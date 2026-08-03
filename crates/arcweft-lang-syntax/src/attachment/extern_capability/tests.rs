use std::collections::HashMap;
use std::num::NonZeroU64;
use std::sync::Arc;

use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use super::{
    AstNode, AttachedCapabilityMember, AttachedExternCapabilityBody, ExternCapabilityItemKind,
};
use crate::attachment::{
    GrammarIdentityMap, SyntaxDatabaseId, SyntaxLineageId, SyntaxNodeId, SyntaxSnapshotData,
    SyntaxSnapshotId, attach_typed_tree,
};
use crate::grammar::kinds::SyntaxKind;
use crate::parser::{ParseOptions, parse_shadow_document};

fn attach(text: &str) -> Arc<SyntaxSnapshotData> {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcw:/extern-capability-attachment-test").unwrap(),
            SourceName::path("extern-capability-attachment-test.arcw"),
            text,
        )
        .unwrap(),
    );
    let build = parse_shadow_document(&document, ParseOptions::default()).unwrap();
    let database = SyntaxDatabaseId::from_raw_for_test(NonZeroU64::new(193).unwrap());
    let lineage = SyntaxLineageId::from_raw_for_test(database, NonZeroU64::new(1).unwrap());
    let snapshot = SyntaxSnapshotId::new(
        lineage,
        SourceSnapshotId::initial(document.display_name().clone()),
    );
    let identities = build
        .index()
        .entries()
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            (
                entry.path().clone(),
                SyntaxNodeId::new(
                    lineage,
                    NonZeroU64::new(u64::try_from(index).unwrap() + 1).unwrap(),
                ),
            )
        })
        .collect::<HashMap<_, _>>();
    attach_typed_tree(
        &build,
        &GrammarIdentityMap::new(identities),
        snapshot,
        document,
    )
    .unwrap()
}

fn capabilities(snapshot: &Arc<SyntaxSnapshotData>) -> Vec<AstNode<ExternCapabilityItemKind>> {
    snapshot
        .nodes()
        .filter(|node| node.kind() == SyntaxKind::ExternCapabilityItem)
        .map(|node| node.cast().unwrap())
        .collect()
}

#[test]
fn capability_attachment_preserves_interleaved_members_groups_defaults_and_effects() {
    let snapshot = attach(concat!(
        "/// Host boundary\n",
        "#[audit(external)]\n",
        "pub extern capability host {\n",
        "    /// Request payload\n",
        "    #[opaque]\n",
        "    pub type Request<T> = Result<T, HostError>\n",
        "    pub fn send<T>(request: T = fallback())(retry: u32) -> Need<T, HostError>\n",
        "        effects { net.connect, net.send, }\n",
        "    type Response\n",
        "}\n",
    ));
    let declaration = capabilities(&snapshot)[0].semantics().unwrap();

    assert_eq!(
        declaration.prefix().documentation().unwrap().markdown(),
        "Host boundary"
    );
    assert_eq!(declaration.name().value().unwrap().as_str(), "host");
    let AttachedExternCapabilityBody::Braced { members, .. } = declaration.body() else {
        panic!("braced capability body");
    };
    assert_eq!(members.len(), 3);
    assert_eq!(
        members
            .iter()
            .map(AttachedCapabilityMember::source_ordinal)
            .collect::<Vec<_>>(),
        [0, 1, 2]
    );

    let AttachedCapabilityMember::AssociatedType(request) = &members[0] else {
        panic!("associated Request type");
    };
    assert_eq!(request.name().value().unwrap().as_str(), "Request");
    assert_eq!(request.generics().unwrap().parameters().len(), 1);
    assert!(request.value().is_some());
    assert_eq!(
        request.prefix().documentation().unwrap().markdown(),
        "Request payload"
    );
    assert_eq!(request.prefix().attributes().len(), 1);

    let AttachedCapabilityMember::Function(send) = &members[1] else {
        panic!("send function");
    };
    assert_eq!(send.name().value().unwrap().as_str(), "send");
    assert_eq!(send.generics().unwrap().parameters().len(), 1);
    assert_eq!(send.parameter_groups().len(), 2);
    assert_eq!(send.parameters().count(), 2);
    assert!(
        send.parameter_groups()[0].parameters()[0]
            .default()
            .is_some()
    );
    assert!(send.authored_return().is_some());
    let effects = send.effects().unwrap();
    assert_eq!(effects.expressions().len(), 2);
    assert!(!effects.has_recovery());

    let AttachedCapabilityMember::AssociatedType(response) = &members[2] else {
        panic!("associated Response type");
    };
    assert_eq!(response.name().value().unwrap().as_str(), "Response");
    assert!(response.value().is_none());
    assert!(!declaration.has_recovery());
}

#[test]
fn capability_attachment_marks_invalid_rest_structure_as_owner_recovery() {
    for (source, rest_count, default_count) in [
        (
            "extern capability host { fn misplaced(values: ...I64, tail: I64) }\n",
            1,
            0,
        ),
        (
            "extern capability host { fn nonfinal(values: ...I64)(tail: I64) }\n",
            1,
            0,
        ),
        (
            "extern capability host { fn duplicate(first: ...I64, second: ...I64) }\n",
            2,
            0,
        ),
        (
            "extern capability host { fn defaulted(values: ...I64 = fallback) }\n",
            1,
            1,
        ),
    ] {
        let snapshot = attach(source);
        let declaration = capabilities(&snapshot)[0].semantics().unwrap();
        let [AttachedCapabilityMember::Function(function)] = declaration.body().members() else {
            panic!("one capability function")
        };

        assert!(function.has_parameter_shape_recovery(), "{source}");
        assert!(function.has_recovery(), "{source}");
        assert!(declaration.has_recovery(), "{source}");
        assert_eq!(
            function
                .parameters()
                .filter(|parameter| parameter.is_rest())
                .count(),
            rest_count
        );
        assert_eq!(
            function
                .parameters()
                .filter(|parameter| parameter.default().is_some())
                .count(),
            default_count
        );
    }
}

#[test]
fn capability_attachment_retains_missing_name_body_and_member_recovery() {
    let snapshot = attach(concat!(
        "extern capability {}\n",
        "extern capability host\n",
        "extern capability recovered {\n",
        "    unsupported member\n",
        "    type\n",
        "    fn broken() effects net.read\n",
        "}\n",
    ));
    let declarations = capabilities(&snapshot);
    assert_eq!(declarations.len(), 3);

    let missing_name = declarations[0].semantics().unwrap();
    assert!(missing_name.name().is_missing());
    assert!(matches!(
        missing_name.body(),
        AttachedExternCapabilityBody::Braced { .. }
    ));

    let missing_body = declarations[1].semantics().unwrap();
    assert!(matches!(
        missing_body.body(),
        AttachedExternCapabilityBody::Missing(_)
    ));

    let recovered = declarations[2].semantics().unwrap();
    let members = recovered.body().members();
    assert_eq!(members.len(), 3);
    assert!(matches!(members[0], AttachedCapabilityMember::Error { .. }));
    let AttachedCapabilityMember::AssociatedType(associated) = &members[1] else {
        panic!("recovered associated type");
    };
    assert!(associated.name().is_missing());
    let AttachedCapabilityMember::Function(function) = &members[2] else {
        panic!("recovered capability function");
    };
    assert!(function.effects().unwrap().has_recovery());
    assert!(!function.trailing_recovery().is_empty());
    assert!(recovered.has_recovery());
}

#[test]
fn capability_attachment_retains_unclosed_effects_and_outer_body_independently() {
    let snapshot = attach(concat!(
        "extern capability host {\n",
        "    fn send() effects { net.send\n",
        "    fn finish() -> Unit\n",
    ));
    let declaration = capabilities(&snapshot)[0].semantics().unwrap();
    let AttachedExternCapabilityBody::Braced { members, .. } = declaration.body() else {
        panic!("braced capability body");
    };
    let AttachedCapabilityMember::Function(send) = &members[0] else {
        panic!("send function");
    };
    assert!(send.effects().unwrap().has_recovery());
    assert!(declaration.body().is_unclosed());
    assert!(declaration.has_recovery());
}
