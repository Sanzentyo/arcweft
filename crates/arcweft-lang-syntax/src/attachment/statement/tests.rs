use std::collections::HashMap;
use std::num::NonZeroU64;
use std::sync::Arc;

use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use super::{
    AstNode, BreakStatementKind, ContinueStatementKind, DeferStatementKind, GotoStatementKind,
    OutStatementKind, RequiredStatementExpressionNode, SignalStatementKind,
};
use crate::attachment::{
    GrammarIdentityMap, SyntaxDatabaseId, SyntaxLineageId, SyntaxNodeId, SyntaxSnapshotData,
    SyntaxSnapshotId, attach_typed_tree,
};
use crate::grammar::SyntaxKind;
use crate::parser::{ParseOptions, parse_shadow_document};

fn attach(text: &str) -> Arc<SyntaxSnapshotData> {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcw:/keyword-statement-attachment-test").unwrap(),
            SourceName::path("keyword-statement-attachment-test.arcw"),
            text,
        )
        .unwrap(),
    );
    let build = parse_shadow_document(&document, ParseOptions::default()).unwrap();
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
