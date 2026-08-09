use std::collections::HashMap;
use std::num::NonZeroU64;
use std::sync::Arc;

use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use super::{
    AstNode, AttachedImplBody, AttachedImplMember, AttachedTraitBody, AttachedTraitMember,
    ImplItemKind, TraitItemKind,
};
use crate::attachment::{
    AttachedFunctionBody, AttachedMethodParameter, AttachedMethodReceiverKind, GrammarIdentityMap,
    SyntaxDatabaseId, SyntaxLineageId, SyntaxNodeId, SyntaxSnapshotData, SyntaxSnapshotId,
    attach_typed_tree,
};
use crate::grammar::kinds::SyntaxKind;
use crate::parser::{ParseOptions, parse_document};

fn attach(text: &str) -> Arc<SyntaxSnapshotData> {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcw:/trait-impl-attachment-test").unwrap(),
            SourceName::path("trait-impl-attachment-test.arcw"),
            text,
        )
        .unwrap(),
    );
    let build = parse_document(&document, ParseOptions::default()).unwrap();
    let database = SyntaxDatabaseId::from_raw_for_test(NonZeroU64::new(197).unwrap());
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

fn traits(snapshot: &Arc<SyntaxSnapshotData>) -> Vec<AstNode<TraitItemKind>> {
    snapshot
        .nodes()
        .filter(|node| node.kind() == SyntaxKind::TraitItem)
        .map(|node| node.cast().unwrap())
        .collect()
}

fn impls(snapshot: &Arc<SyntaxSnapshotData>) -> Vec<AstNode<ImplItemKind>> {
    snapshot
        .nodes()
        .filter(|node| node.kind() == SyntaxKind::ImplItem)
        .map(|node| node.cast().unwrap())
        .collect()
}

#[test]
fn trait_and_impl_attachments_preserve_distinct_members_and_method_shapes() {
    let snapshot = attach(concat!(
        "trait SourceLike<T>: Base + Iterable<T> where T: Copyable {\n",
        "    type Item<U> = Result<U, Error>\n",
        "    fn current(&self)(fallback: T) -> T { fallback }\n",
        "    fn required(mut self) -> T\n",
        "}\n",
        "impl<T> SourceLike<T> for Box<T> where T: Copyable {\n",
        "    type Item<U> = U\n",
        "    fn current(&mut self)(fallback: T) -> T { fallback }\n",
        "    fn required(self) -> T\n",
        "}\n",
    ));

    let trait_declaration = traits(&snapshot)[0].semantics().unwrap();
    assert_eq!(
        trait_declaration.name().value().unwrap().as_str(),
        "SourceLike"
    );
    assert_eq!(trait_declaration.generics().unwrap().parameters().len(), 1);
    assert_eq!(trait_declaration.supertraits().len(), 2);
    assert_eq!(trait_declaration.where_clauses().len(), 1);
    let AttachedTraitBody::Braced { members, .. } = trait_declaration.body() else {
        panic!("braced Trait body")
    };
    assert_eq!(members.len(), 3);
    assert_eq!(
        members
            .iter()
            .map(AttachedTraitMember::source_ordinal)
            .collect::<Vec<_>>(),
        [0, 1, 2]
    );
    let AttachedTraitMember::AssociatedType(item) = &members[0] else {
        panic!("Trait associated type")
    };
    assert_eq!(item.name().value().unwrap().as_str(), "Item");
    assert!(item.default().is_some());
    let AttachedTraitMember::Function(current) = &members[1] else {
        panic!("Trait default method")
    };
    assert_eq!(current.parameter_groups().len(), 2);
    let parameters = current.parameters().collect::<Vec<_>>();
    let AttachedMethodParameter::Receiver(receiver) = parameters[0] else {
        panic!("shared receiver")
    };
    assert_eq!(receiver.kind(), AttachedMethodReceiverKind::SharedReference);
    assert!(receiver.ampersand_source().is_some());
    assert!(receiver.mut_keyword_source().is_none());
    assert!(matches!(
        current.body(),
        Some(AttachedFunctionBody::Block { .. })
    ));
    let AttachedTraitMember::Function(required) = &members[2] else {
        panic!("Trait signature")
    };
    let receiver = required
        .parameters()
        .next()
        .and_then(AttachedMethodParameter::receiver)
        .unwrap();
    assert_eq!(receiver.kind(), AttachedMethodReceiverKind::Owned);
    assert!(receiver.mut_keyword_source().is_some());
    assert!(required.body().is_none());
    assert!(!trait_declaration.has_recovery());

    let impl_declaration = impls(&snapshot)[0].semantics().unwrap();
    assert!(impl_declaration.trait_ref().is_some());
    assert_eq!(impl_declaration.where_clauses().len(), 1);
    let AttachedImplBody::Braced { members, .. } = impl_declaration.body() else {
        panic!("braced Impl body")
    };
    assert_eq!(members.len(), 3);
    let AttachedImplMember::AssociatedType(item) = &members[0] else {
        panic!("Impl associated type")
    };
    assert_eq!(item.name().value().unwrap().as_str(), "Item");
    let AttachedImplMember::Function(current) = &members[1] else {
        panic!("Impl method")
    };
    let receiver = current
        .parameters()
        .next()
        .and_then(AttachedMethodParameter::receiver)
        .unwrap();
    assert_eq!(
        receiver.kind(),
        AttachedMethodReceiverKind::MutableReference
    );
    assert!(receiver.ampersand_source().is_some());
    assert!(receiver.mut_keyword_source().is_some());
    let AttachedImplMember::Function(required) = &members[2] else {
        panic!("bodyless Impl method")
    };
    assert!(required.body().is_none());
    assert!(!impl_declaration.has_recovery());
}

#[test]
fn missing_impl_target_and_invalid_members_remain_typed_recovery() {
    let snapshot = attach(concat!(
        "impl SourceLike for Broken {\n",
        "    type Item\n",
        "    unsupported member\n",
        "    fn current(self) -> T\n",
        "}\n",
    ));
    let declaration = impls(&snapshot)[0].semantics().unwrap();
    let AttachedImplBody::Braced { members, .. } = declaration.body() else {
        panic!("braced Impl body")
    };
    assert_eq!(members.len(), 3);
    let AttachedImplMember::AssociatedType(item) = &members[0] else {
        panic!("recovered associated type")
    };
    assert_eq!(
        item.target().family(),
        crate::attachment::AttachedTypeFamily::Recovery
    );
    assert!(matches!(members[1], AttachedImplMember::Error { .. }));
    assert!(declaration.has_recovery());
}

#[test]
fn every_receiver_form_has_a_pattern_and_never_a_fabricated_type() {
    let snapshot = attach(concat!(
        "trait ReceiverForms {\n",
        "    fn owned(self)\n",
        "    fn owned_mut(mut self)\n",
        "    fn shared(&self)\n",
        "    fn exclusive(&mut self)\n",
        "}\n",
    ));
    let declaration = traits(&snapshot)[0].semantics().unwrap();
    let kinds = declaration
        .body()
        .members()
        .iter()
        .map(|member| {
            let AttachedTraitMember::Function(function) = member else {
                panic!("method member")
            };
            let receiver = function
                .parameters()
                .next()
                .and_then(AttachedMethodParameter::receiver)
                .unwrap();
            assert!(receiver.pattern().syntax().kind().is_pattern_node());
            receiver.kind()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        kinds,
        [
            AttachedMethodReceiverKind::Owned,
            AttachedMethodReceiverKind::Owned,
            AttachedMethodReceiverKind::SharedReference,
            AttachedMethodReceiverKind::MutableReference,
        ]
    );
    assert_eq!(
        snapshot
            .nodes()
            .filter(|node| {
                node.parent()
                    .is_some_and(|parent| parent.kind() == SyntaxKind::Parameter)
                    && node.kind().is_type_node()
            })
            .count(),
        0
    );
}
