use std::collections::HashMap;
use std::num::NonZeroU64;
use std::sync::Arc;

use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use super::{
    AstNode, AttachedViewFragmentEntry, AttachedViewPartLocalName, AttachedViewPartPath,
    AttachedViewRequiredKeyword, ViewDeclarationItemKind,
};
use crate::attachment::{
    GrammarIdentityMap, SyntaxDatabaseId, SyntaxLineageId, SyntaxNodeId, SyntaxSnapshotData,
    SyntaxSnapshotId, attach_typed_tree,
};
use crate::grammar::kinds::SyntaxKind;
use crate::parser::{ParseOptions, parse_document};

fn attach(text: &str) -> Arc<SyntaxSnapshotData> {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcw:/view-attachment-test").unwrap(),
            SourceName::path("view-attachment-test.arcw"),
            text,
        )
        .unwrap(),
    );
    let build = parse_document(&document, ParseOptions::default()).unwrap();
    let database = SyntaxDatabaseId::from_raw_for_test(NonZeroU64::new(163).unwrap());
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

fn views(snapshot: &Arc<SyntaxSnapshotData>) -> Vec<AstNode<ViewDeclarationItemKind>> {
    snapshot
        .nodes()
        .filter(|node| node.kind() == SyntaxKind::ViewDeclarationItem)
        .map(|node| node.cast().unwrap())
        .collect()
}

#[test]
fn view_attachment_reuses_callable_parameters_and_owns_exports_and_values() {
    let snapshot = attach(concat!(
        "/// Main View\n",
        "pub view Main(count: u32 = 1) {\n",
        "    export part panel as public.panel\n",
        "    Panel {}\n",
        "    Text(count)\n",
        "}\n",
    ));
    let declaration = views(&snapshot)[0].semantics().unwrap();
    assert_eq!(
        declaration.prefix().documentation().unwrap().markdown(),
        "Main View"
    );
    assert!(!declaration.parameter_group().open_state().is_missing());
    assert!(!declaration.parameter_group().close_state().is_missing());
    let [parameter] = declaration.parameter_group().parameters() else {
        panic!("fixture has one parameter");
    };
    assert_eq!(parameter.source_ordinal(), 0);
    assert!(parameter.default().is_some());

    let exports = declaration.exports().collect::<Vec<_>>();
    assert_eq!(exports.len(), 1);
    assert_eq!(exports[0].source_ordinal(), 0);
    assert!(matches!(
        exports[0].part(),
        AttachedViewRequiredKeyword::Authored(_)
    ));
    let AttachedViewPartPath::Path(local) = exports[0].local_part() else {
        panic!("local part must be a path");
    };
    assert_eq!(local.segments()[0].source_text(), "panel");
    let AttachedViewPartPath::Path(public) = exports[0].public_part() else {
        panic!("public part must be a path");
    };
    assert_eq!(
        public
            .segments()
            .iter()
            .map(super::super::source_file::AttachedPathSegment::source_text)
            .collect::<Vec<_>>(),
        ["public", "panel"]
    );
    assert_eq!(declaration.body().fragment().unwrap().values().count(), 2);
    assert!(!declaration.has_recovery());
}

#[test]
fn view_fragment_preserves_value_export_value_interleaving_and_global_export_ordinals() {
    let snapshot = attach(concat!(
        "view Broken() {\n",
        "    export part first as public_first\n",
        "    Panel {}\n",
        "    export late\n",
        "    Text(1)\n",
        "}\n",
    ));
    let declaration = views(&snapshot)[0].semantics().unwrap();
    let fragment = declaration.body().fragment().unwrap();
    assert!(matches!(
        fragment.entries(),
        [
            AttachedViewFragmentEntry::Value(_),
            AttachedViewFragmentEntry::MisplacedExport(_),
            AttachedViewFragmentEntry::Value(_)
        ]
    ));
    let export = fragment.misplaced_exports().next().unwrap();
    assert_eq!(export.source_ordinal(), 1);
    assert!(export.is_misplaced());
    assert!(export.part().is_missing());
    assert!(export.alias().is_missing());
    assert!(matches!(
        export.public_part(),
        AttachedViewPartPath::Missing(_)
    ));
    assert!(declaration.has_recovery());
}

#[test]
fn missing_view_signature_and_body_remain_typed_recovery() {
    let snapshot = attach("view Missing\n");
    let declaration = views(&snapshot)[0].semantics().unwrap();
    assert!(declaration.parameter_group().open_state().is_missing());
    assert!(declaration.parameter_group().close_state().is_missing());
    assert!(declaration.body().is_missing());
    assert!(declaration.has_recovery());
}

#[test]
fn header_and_trailing_recoveries_keep_distinct_source_owned_slots() {
    let snapshot = attach("view Broken() -> View { Panel {} } trailing\n");
    let declaration = views(&snapshot)[0].semantics().unwrap();
    assert!(declaration.header_recovery().is_some());
    assert!(declaration.trailing_recovery().is_some());
    assert!(
        declaration.header_recovery().unwrap().range().end()
            <= declaration.body().syntax().range().start()
    );
    assert!(
        declaration.trailing_recovery().unwrap().range().start()
            >= declaration.body().syntax().range().end()
    );
}

#[test]
fn keyword_and_destructuring_parameters_poison_view_without_losing_typed_patterns() {
    let snapshot = attach(concat!(
        "view Keyword(view: u32) { Panel {} }\n",
        "view Tuple((left, right): Pair) { Panel {} }\n",
    ));
    let declarations = views(&snapshot)
        .iter()
        .map(AstNode::<ViewDeclarationItemKind>::semantics)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert!(
        declarations
            .iter()
            .all(super::AttachedViewDeclaration::has_recovery)
    );
    assert_eq!(declarations[0].parameter_group().parameters().len(), 1);
    assert_eq!(declarations[1].parameter_group().parameters().len(), 1);
}

#[test]
fn view_fragment_owns_part_modifier_roles_without_detached_view_reparse() {
    let source = concat!(
        "view Card() {\n",
        "    Column {\n",
        "        Text(\"Body\").part( body )\n",
        "        Text(\"Title\")\n",
        "            .part( header.title )\n",
        "    }\n",
        "}\n",
    );
    let snapshot = attach(source);
    let declaration = views(&snapshot)[0].semantics().unwrap();
    let modifiers = declaration.body().fragment().unwrap().part_modifiers();
    assert_eq!(modifiers.len(), 2);
    for (ordinal, (modifier, expected)) in
        modifiers.iter().zip(["body", "header.title"]).enumerate()
    {
        assert_eq!(modifier.source_ordinal(), u32::try_from(ordinal).unwrap());
        assert!(!modifier.has_recovery());
        let AttachedViewPartLocalName::Present(local_name) = modifier.local_name() else {
            panic!("clean View part modifier must own a present local name");
        };
        assert_eq!(&source[local_name.range().as_range()], expected);
        assert_eq!(&source[modifier.name().range().as_range()], "part");
        assert_eq!(&source[modifier.dot().range().as_range()], ".");
        assert_eq!(&source[modifier.open().range().as_range()], "(");
        assert_eq!(&source[modifier.close().unwrap().range().as_range()], ")");
    }
}

#[test]
fn malformed_part_modifier_is_typed_recovery_and_not_a_clean_local_name() {
    let source = "view Broken() { Text(\"Body\").part( header..title ) }\n";
    let snapshot = attach(source);
    let declaration = views(&snapshot)[0].semantics().unwrap();
    let [modifier] = declaration.body().fragment().unwrap().part_modifiers() else {
        panic!("fixture has one View part modifier");
    };
    assert!(modifier.has_recovery());
    assert!(matches!(
        modifier.local_name(),
        AttachedViewPartLocalName::Invalid(_)
    ));
    assert!(declaration.has_recovery());
}
