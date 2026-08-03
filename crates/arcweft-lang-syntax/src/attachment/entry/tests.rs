use std::collections::HashMap;
use std::num::NonZeroU64;
use std::sync::Arc;

use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use super::{
    AttachedEntryBody, AttachedEntryId, AttachedEntryKind, AttachedEntryMember, AttachedEntryValue,
};
use crate::attachment::node::EntryDeclarationItemKind;
use crate::attachment::{
    AstNode, GrammarIdentityMap, SyntaxDatabaseId, SyntaxLineageId, SyntaxNodeId,
    SyntaxSnapshotData, SyntaxSnapshotId, attach_typed_tree,
};
use crate::grammar::kinds::SyntaxKind;
use crate::parser::{ParseOptions, parse_shadow_document};

fn attach(text: &str) -> Arc<SyntaxSnapshotData> {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcw:/entry-attachment-test").unwrap(),
            SourceName::path("entry-attachment-test.arcw"),
            text,
        )
        .unwrap(),
    );
    let build = parse_shadow_document(&document, ParseOptions::default()).unwrap();
    let database = SyntaxDatabaseId::from_raw_for_test(NonZeroU64::new(181).unwrap());
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

fn entry(snapshot: &Arc<SyntaxSnapshotData>) -> AstNode<EntryDeclarationItemKind> {
    snapshot
        .nodes()
        .find(|node| node.kind() == SyntaxKind::EntryDeclarationItem)
        .expect("entry declaration")
        .cast()
        .unwrap()
}

#[test]
fn entry_attachment_preserves_typed_header_and_complete_member_inventory() {
    let snapshot = attach(concat!(
        "/// Main entry\n",
        "pub entry game @entry.game.main {\n",
        "    state = GameState\n",
        "    initializer = game.initial_state\n",
        "    event = GameEvent\n",
        "    reducer = game.reduce\n",
        "    controller = game.controller\n",
        "    goto @flow.opening\n",
        "    route GET \"/hello/:name\" -> @flow.hello(name = :name)\n",
        "    budget = policy(1)\n",
        "}\n",
    ));
    let declaration = entry(&snapshot).semantics().unwrap();
    assert!(matches!(declaration.kind(), AttachedEntryKind::Game(_)));
    assert!(declaration.id().is_canonical_entry_family());
    assert_eq!(
        declaration.prefix().documentation().unwrap().markdown(),
        "Main entry"
    );
    let AttachedEntryBody::Braced { members, .. } = declaration.body() else {
        panic!("authored entry body")
    };
    assert_eq!(members.len(), 8);
    assert!(matches!(members[0], AttachedEntryMember::StateType(_)));
    assert!(matches!(members[1], AttachedEntryMember::Initializer(_)));
    assert!(matches!(members[2], AttachedEntryMember::EventType(_)));
    assert!(matches!(members[3], AttachedEntryMember::Reducer(_)));
    assert!(matches!(members[4], AttachedEntryMember::Controller(_)));
    assert!(matches!(members[5], AttachedEntryMember::Goto { .. }));
    assert!(matches!(members[6], AttachedEntryMember::Route { .. }));
    assert!(matches!(members[7], AttachedEntryMember::Option { .. }));
    assert!(!declaration.has_recovery());
}

#[test]
fn entry_attachment_retains_current_grammar_recovery_without_text_reinterpretation() {
    let snapshot = attach(concat!(
        "entry game @flow.main trailing {\n",
        "    state GameState\n",
        "    event =\n",
        "    route FETCH \"/hello/:name\" -> @flow.hello(name :name)\n",
    ));
    let declaration = entry(&snapshot).semantics().unwrap();
    assert!(declaration.has_header_trailing_recovery());
    assert!(!declaration.id().is_canonical_entry_family());
    assert!(!declaration.body().is_closed());
    assert!(declaration.has_recovery());
    let members = declaration.body().members();
    assert_eq!(members.len(), 3);
    assert!(members.iter().all(AttachedEntryMember::has_recovery));
    let AttachedEntryMember::EventType(event) = &members[1] else {
        panic!("missing event type remains a typed recovery type")
    };
    assert!(event.value().value().is_some());
}

#[test]
fn entry_attachment_propagates_callable_path_trailing_recovery_to_the_declaration() {
    let snapshot = attach(concat!(
        "entry game @entry.path_recovery {\n",
        "    initializer = server.\n",
        "}\n",
    ));
    let declaration = entry(&snapshot).semantics().unwrap();
    let [member @ AttachedEntryMember::Initializer(binding)] = declaration.body().members() else {
        panic!("one typed initializer member")
    };
    let AttachedEntryValue::Authored(path) = binding.value() else {
        panic!("recovered authored path")
    };

    assert_eq!(
        path.segments()
            .iter()
            .map(super::super::source_file::AttachedPathSegment::source_text)
            .collect::<Vec<_>>(),
        ["server"]
    );
    assert!(!path.has_recovery());
    assert!(binding.has_trailing_recovery());
    assert!(member.has_recovery());
    assert!(declaration.has_recovery());
}

#[test]
fn entry_attachment_propagates_recovered_goto_id_to_the_declaration() {
    let snapshot = attach(concat!(
        "entry game @entry.goto_recovery {\n",
        "    goto @flow.\n",
        "}\n",
    ));
    let declaration = entry(&snapshot).semantics().unwrap();
    let [member @ AttachedEntryMember::Goto { target, .. }] = declaration.body().members() else {
        panic!("one typed goto member")
    };
    let AttachedEntryValue::Authored(expression) = target else {
        panic!("recovered authored goto expression")
    };
    let crate::expressions::ExpressionProjection::EntityReference(reference) =
        expression.projection()
    else {
        panic!("typed goto entity reference")
    };

    assert!(reference.value().is_err());
    assert!(member.has_recovery());
    assert!(declaration.has_recovery());
}

#[test]
fn delimited_entry_id_remains_a_recognized_noncanonical_entry_header() {
    let snapshot = attach("entry game @<entry.foo> {}\n");
    let declaration = entry(&snapshot).semantics().unwrap();

    assert!(matches!(declaration.kind(), AttachedEntryKind::Game(_)));
    let AttachedEntryId::Authored {
        reference,
        canonical_entry_family,
        ..
    } = declaration.id()
    else {
        panic!("typed authored Entry ID")
    };
    assert!(reference.value().is_ok());
    assert!(!canonical_entry_family);
    assert!(declaration.id().has_recovery());
    assert!(declaration.has_recovery());
}
