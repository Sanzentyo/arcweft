use std::collections::HashMap;
use std::num::NonZeroU64;
use std::sync::Arc;

use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use super::{
    AstNode, BreakStatementKind, ContinueStatementKind, DeferBlockStatementKind,
    DeferStatementKind, GotoStatementKind, OutStatementKind, RequiredStatementExpressionNode,
    SignalStatementKind,
};
use crate::attachment::{
    AttachedDeferBlockBody, AttachedExpressionNode, GrammarIdentityMap, SyntaxDatabaseId,
    SyntaxLineageId, SyntaxNodeId, SyntaxSnapshotData, SyntaxSnapshotId, attach_typed_tree,
};
use crate::grammar::SyntaxKind;
use crate::parser::{ParseOptions, parse_document};

fn attach(text: &str) -> Arc<SyntaxSnapshotData> {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcw:/keyword-statement-attachment-test").unwrap(),
            SourceName::path("keyword-statement-attachment-test.arcw"),
            text,
        )
        .unwrap(),
    );
    let build = parse_document(&document, ParseOptions::default()).unwrap();
    let database = SyntaxDatabaseId::from_raw_for_test(NonZeroU64::new(211).unwrap());
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

fn statement<K: crate::attachment::node::ExactAstKind>(
    snapshot: &Arc<SyntaxSnapshotData>,
) -> AstNode<K> {
    snapshot
        .nodes()
        .find(|node| node.kind() == K::KIND)
        .unwrap()
        .cast()
        .unwrap()
}

fn is_missing(expression: &RequiredStatementExpressionNode) -> bool {
    matches!(expression, RequiredStatementExpressionNode::Missing(_))
}

#[test]
fn keyword_statement_views_preserve_exact_labels_operands_and_arrow() {
    let snapshot = attach(
        "fn inspect() { out 'exit value; goto target; defer cleanup(); signal ready <- true; break 'outer result; continue 'outer; }\n",
    );

    let out = statement::<OutStatementKind>(&snapshot)
        .semantics()
        .unwrap();
    assert_eq!(out.label().unwrap().value().unwrap().as_str(), "exit");
    assert!(!is_missing(out.value()));

    let goto = statement::<GotoStatementKind>(&snapshot)
        .semantics()
        .unwrap();
    assert!(!is_missing(goto.target()));

    let defer = statement::<DeferStatementKind>(&snapshot)
        .semantics()
        .unwrap();
    assert!(!is_missing(defer.expression()));

    let signal = statement::<SignalStatementKind>(&snapshot)
        .semantics()
        .unwrap();
    assert!(!is_missing(signal.target()));
    assert!(!is_missing(signal.value()));
    assert!(signal.arrow_recovery().is_none());

    let broken = statement::<BreakStatementKind>(&snapshot)
        .semantics()
        .unwrap();
    assert_eq!(broken.label().unwrap().value().unwrap().as_str(), "outer");
    assert!(broken.value().is_some());

    let continued = statement::<ContinueStatementKind>(&snapshot)
        .semantics()
        .unwrap();
    assert_eq!(
        continued.label().unwrap().value().unwrap().as_str(),
        "outer"
    );
    assert!(continued.forbidden_suffix().is_none());
}

#[test]
fn keyword_statement_views_keep_required_slots_and_typed_recovery() {
    let snapshot = attach(
        "fn inspect() { out; goto; defer; signal; break; continue extra; out 'line.focus; continue 'events?; }\n",
    );
    let mut nodes = snapshot.nodes();

    let out = nodes
        .find(|node| node.kind() == SyntaxKind::OutStatement)
        .unwrap()
        .cast::<OutStatementKind>()
        .unwrap()
        .semantics()
        .unwrap();
    assert!(out.label().is_none());
    assert!(is_missing(out.value()));

    assert!(is_missing(
        statement::<GotoStatementKind>(&snapshot)
            .semantics()
            .unwrap()
            .target()
    ));
    assert!(is_missing(
        statement::<DeferStatementKind>(&snapshot)
            .semantics()
            .unwrap()
            .expression()
    ));

    let signal = statement::<SignalStatementKind>(&snapshot)
        .semantics()
        .unwrap();
    assert!(is_missing(signal.target()));
    assert!(is_missing(signal.value()));
    assert!(signal.arrow_recovery().is_some());

    assert!(
        statement::<BreakStatementKind>(&snapshot)
            .semantics()
            .unwrap()
            .value()
            .is_none()
    );
    assert!(
        statement::<ContinueStatementKind>(&snapshot)
            .semantics()
            .unwrap()
            .forbidden_suffix()
            .is_some()
    );

    let recovered_labels = snapshot
        .nodes()
        .filter(|node| {
            node.kind() == SyntaxKind::OutStatement || node.kind() == SyntaxKind::ContinueStatement
        })
        .filter_map(|node| match node.kind() {
            SyntaxKind::OutStatement => node
                .cast::<OutStatementKind>()
                .ok()?
                .semantics()
                .ok()?
                .label()
                .cloned(),
            SyntaxKind::ContinueStatement => node
                .cast::<ContinueStatementKind>()
                .ok()?
                .semantics()
                .ok()?
                .label()
                .cloned(),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(recovered_labels.len(), 2);
    assert!(
        recovered_labels
            .iter()
            .all(super::AttachedControlLabel::is_recovered)
    );
}

#[test]
fn line_plan_defer_attaches_outcome_body_and_authored_spans() {
    let source = concat!(
        "flow line_defer() -> String {\n",
        "    alice(voice=auto):\n",
        "        Hello[p]\n",
        "    with:\n",
        "        defer on completed { cleanup() }\n",
        "        defer on failed:\n",
        "            cleanup()\n",
        "    return \"done\"\n",
        "}\n",
    );
    let snapshot = attach(source);
    let defers = snapshot
        .nodes()
        .filter(|node| node.kind() == SyntaxKind::DeferBlockStatement)
        .map(|node| {
            node.cast::<DeferBlockStatementKind>()
                .unwrap()
                .semantics()
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        defers.len(),
        2,
        "kinds={:#?}",
        snapshot.nodes().map(|node| node.kind()).collect::<Vec<_>>()
    );

    assert_eq!(
        defers[0].outcome(),
        crate::ast::line_plan::DeferOutcome::Completed
    );
    let completed = defers[0].outcome_source_span().unwrap();
    assert_eq!(
        completed.range(),
        source
            .find("completed")
            .map(|start| { arcweft_source::SourceRange::new(start, start + "completed".len()) })
            .unwrap()
    );
    let AttachedDeferBlockBody::Expression(body) = defers[0].body() else {
        panic!("braced defer body must be a Block expression");
    };
    assert!(matches!(
        body.projection(),
        crate::expressions::ExpressionProjection::Block
    ));
    assert_eq!(body.syntax().source_text(), "{ cleanup() }");

    assert_eq!(
        defers[1].outcome(),
        crate::ast::line_plan::DeferOutcome::Failed
    );
    let failed = defers[1].outcome_source_span().unwrap();
    let failed_start = source.find("failed").unwrap();
    assert_eq!(
        failed.range(),
        arcweft_source::SourceRange::new(failed_start, failed_start + "failed".len())
    );
    let AttachedDeferBlockBody::Expression(body) = defers[1].body() else {
        panic!("indented defer body must be a Block expression");
    };
    assert!(matches!(
        body.projection(),
        crate::expressions::ExpressionProjection::Block
    ));
    assert_eq!(body.syntax().source_text(), "cleanup()");
}

#[test]
fn line_plan_defer_rejects_unknown_outcomes_and_missing_indented_bodies() {
    let source = concat!(
        "flow malformed_defer() -> String {\n",
        "    alice(voice=auto):\n",
        "        Hello[p]\n",
        "    with:\n",
        "        defer on succeeded { cleanup() }\n",
        "        defer on completed:\n",
        "    return \"done\"\n",
        "}\n",
    );
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcw:/malformed-defer-test").unwrap(),
            SourceName::path("malformed-defer-test.arcw"),
            source,
        )
        .unwrap(),
    );
    let build = parse_document(&document, ParseOptions::default()).unwrap();
    assert_eq!(build.green().to_string(), source);
    assert!(
        build
            .diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.code() == "syntax.dialogue.line_plan_invalid_defer" })
    );
    assert!(
        build
            .diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.code() == "syntax.statement.missing_defer_body" })
    );
    assert_eq!(
        build
            .index()
            .entries()
            .iter()
            .filter(|entry| entry.kind() == SyntaxKind::DeferBlockStatement)
            .count(),
        1
    );
    let snapshot = attach(source);
    let missing_body = snapshot
        .nodes()
        .find(|node| node.kind() == SyntaxKind::DeferBlockStatement)
        .unwrap()
        .cast::<DeferBlockStatementKind>()
        .unwrap()
        .semantics()
        .unwrap();
    assert_eq!(
        missing_body.outcome(),
        crate::ast::line_plan::DeferOutcome::Completed
    );
    assert!(matches!(
        missing_body.body(),
        AttachedDeferBlockBody::Missing(_)
    ));
}

#[test]
fn inline_colon_dialogue_keeps_same_indent_line_plan_after_content() {
    let source = concat!(
        "flow main() -> String {\n",
        "    alice: hello[mark @.end]\n",
        "    with:\n",
        "        on mark(@.end) => log.info(\"end\")\n",
        "    return \"done\"\n",
        "}\n",
    );
    let snapshot = attach(source);
    let application = snapshot
        .nodes()
        .find(|node| node.kind() == SyntaxKind::AttachedContentApplicationExpression)
        .unwrap();
    let application = AttachedExpressionNode::from_syntax(application).unwrap();
    let plan = application
        .dialogue_line_plan()
        .unwrap()
        .expect("same-indent with plan follows inline colon Dialogue content");
    assert_eq!(plan.body().items().len(), 1);
    assert_eq!(plan.body().items()[0].kind(), SyntaxKind::OnStatement);
}
