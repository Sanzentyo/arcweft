use std::collections::HashMap;
use std::num::NonZeroU64;
use std::sync::Arc;

use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange};

use super::{AttachedDialogueLinePlanInit, AttachedDialogueLinePlanItem};
use crate::attachment::{
    AttachedExpressionNode, GrammarIdentityMap, SyntaxDatabaseId, SyntaxLineageId, SyntaxNodeId,
    SyntaxSnapshotData, SyntaxSnapshotId, attach_typed_tree,
};
use crate::grammar::SyntaxKind;
use crate::parser::{ParseOptions, parse_document};

fn attach(source: &str) -> Arc<SyntaxSnapshotData> {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("memory:dialogue-line-plan-init").unwrap(),
            SourceName::Memory,
            source,
        )
        .unwrap(),
    );
    let build = parse_document(&document, ParseOptions::default()).unwrap();
    let database = SyntaxDatabaseId::from_raw_for_test(NonZeroU64::new(331).unwrap());
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

fn line_plan(source: &str) -> crate::attachment::AttachedDialogueLinePlan {
    let snapshot = attach(source);
    let application = snapshot
        .nodes()
        .find(|node| node.kind() == SyntaxKind::AttachedContentApplicationExpression)
        .unwrap_or_else(|| {
            panic!(
                "fixture contains one attached Dialogue application: {:?}",
                snapshot.nodes().map(|node| node.kind()).collect::<Vec<_>>()
            )
        });
    AttachedExpressionNode::from_syntax(application)
        .unwrap()
        .dialogue_line_plan()
        .unwrap()
        .expect("Dialogue application retains its line plan")
}

fn expect_init(item: &AttachedDialogueLinePlanItem) -> &AttachedDialogueLinePlanInit {
    let AttachedDialogueLinePlanItem::Init(init) = item else {
        panic!("expected the typed Init item")
    };
    init
}

#[test]
fn braced_init_is_one_typed_scoped_item_with_ordered_statements() {
    let source = concat!(
        "flow main() {\n",
        "    alice()[本文。[p]] with {\n",
        "        init {\n",
        "            let word = \"setup\"\n",
        "            log.info(word)\n",
        "        }\n",
        "        out ()\n",
        "    }\n",
        "}\n",
    );
    let plan = line_plan(source);
    assert_eq!(plan.body().items().len(), 2);
    let init = expect_init(&plan.body().items()[0]);
    assert_eq!(init.body().kind(), SyntaxKind::Block);
    assert_eq!(init.statements().len(), 2);
    assert_eq!(init.statements()[0].kind(), SyntaxKind::LetStatement);
    assert_eq!(init.statements()[1].kind(), SyntaxKind::ExpressionStatement);
    assert!(!init.has_recovery());
    assert_eq!(plan.body().items()[1].kind(), SyntaxKind::OutStatement);
}

#[test]
fn colon_init_is_one_typed_scoped_item_and_does_not_consume_plan_sibling() {
    let source = concat!(
        "flow main() {\n",
        "    alice()[本文。[p]] with {\n",
        "        init:\n",
        "            log.info(\"setup\")\n",
        "        out ()\n",
        "    }\n",
        "}\n",
    );
    let plan = line_plan(source);
    assert_eq!(plan.body().items().len(), 2);
    let init = expect_init(&plan.body().items()[0]);
    assert_eq!(init.statements().len(), 1);
    assert_eq!(init.statements()[0].kind(), SyntaxKind::ExpressionStatement);
    assert_eq!(plan.body().items()[1].kind(), SyntaxKind::OutStatement);
}

#[test]
fn init_colon_form_in_indented_plan_keeps_the_following_item_outside_init() {
    let source = concat!(
        "flow main() {\n",
        "    alice()[本文。[p]]\n",
        "    with:\n",
        "        init:\n",
        "            log.info(\"setup\")\n",
        "        out ()\n",
        "}\n",
    );
    let plan = line_plan(source);
    assert_eq!(plan.body().items().len(), 2);
    let init = expect_init(&plan.body().items()[0]);
    assert_eq!(init.statements().len(), 1);
    assert_eq!(plan.body().items()[1].kind(), SyntaxKind::OutStatement);
}

#[test]
fn init_without_a_body_has_a_bounded_recovery_span() {
    let source = concat!(
        "flow main() {\n",
        "    alice()[本文。[p]] with {\n",
        "        init\n",
        "    }\n",
        "}\n",
    );
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("memory:dialogue-line-plan-init-recovery").unwrap(),
            SourceName::Memory,
            source,
        )
        .unwrap(),
    );
    let build = parse_document(&document, ParseOptions::default()).unwrap();
    let [diagnostic] = build.diagnostics() else {
        panic!("missing Init body reports one bounded recovery diagnostic")
    };
    let init_start = source.find("init").unwrap();
    assert_eq!(
        diagnostic.code(),
        "syntax.dialogue.line_plan_init_missing_body"
    );
    assert_eq!(
        diagnostic.range(),
        SourceRange::new(init_start, init_start + "init".len())
    );

    let plan = line_plan(source);
    assert!(plan.body().has_recovery());
    let [AttachedDialogueLinePlanItem::Statement(error)] = plan.body().items() else {
        panic!("invalid Init remains one recovered line-plan item")
    };
    assert_eq!(error.kind(), SyntaxKind::ErrorStatement);
}

#[test]
fn init_out_followed_by_a_statement_remains_typed_for_semantic_rejection() {
    let source = concat!(
        "flow main() {\n",
        "    alice()[本文。[p]] with {\n",
        "        init {\n",
        "            out ()\n",
        "            log.info(\"unreachable\")\n",
        "        }\n",
        "    }\n",
        "}\n",
    );
    let plan = line_plan(source);
    let [AttachedDialogueLinePlanItem::Init(init)] = plan.body().items() else {
        panic!("line plan retains the typed Init item")
    };
    assert_eq!(init.statements().len(), 2);
    assert_eq!(init.statements()[0].kind(), SyntaxKind::OutStatement);
    assert_eq!(init.statements()[1].kind(), SyntaxKind::ExpressionStatement);
    assert!(!plan.body().has_recovery());
}
