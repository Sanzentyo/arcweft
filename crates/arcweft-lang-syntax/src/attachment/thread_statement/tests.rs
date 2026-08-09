use std::collections::HashMap;
use std::num::NonZeroU64;
use std::sync::Arc;

use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use super::{
    AttachedForStatement, AttachedLoopStatement, AttachedWhileLetStatement, AttachedWhileStatement,
};
use crate::attachment::node::{FlowItemKind, ThreadExpressionKind};
use crate::attachment::source_file::AttachedDelimiterState;
use crate::attachment::{
    AttachedFlowStatementBody, AttachedRequiredFlowBody, AttachedRequiredNestedThreadFlowBody,
    AttachedRequiredThreadExpressionBody, AttachedThreadExpressionBody, AttachedThreadFlowItem,
    GrammarIdentityMap, RequiredStatementExpressionNode, SyntaxDatabaseId, SyntaxLineageId,
    SyntaxNodeId, SyntaxSnapshotData, SyntaxSnapshotId, attach_typed_tree,
};
use crate::parser::{ParseOptions, parse_document};
use crate::patterns::PatternSyntaxState;

fn attach(text: &str) -> Arc<SyntaxSnapshotData> {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcw:/thread-loop-statement-attachment-test").unwrap(),
            SourceName::path("thread-loop-statement-attachment-test.arcw"),
            text,
        )
        .unwrap(),
    );
    let build = parse_document(&document, ParseOptions::default()).unwrap();
    let database = SyntaxDatabaseId::from_raw_for_test(NonZeroU64::new(311).unwrap());
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

fn flow_body(text: &str) -> AttachedFlowStatementBody {
    let snapshot = attach(text);
    let flow = snapshot
        .nodes()
        .find(|node| node.kind() == crate::grammar::SyntaxKind::FlowItem)
        .expect("fixture must retain one Flow declaration")
        .cast::<FlowItemKind>()
        .unwrap();
    let declaration = flow.semantics().unwrap();
    let AttachedRequiredFlowBody::Present(body) = declaration.body() else {
        panic!("fixture Flow body must be present");
    };
    body.clone()
}

fn thread_body(text: &str) -> AttachedThreadExpressionBody {
    let snapshot = attach(text);
    let thread = snapshot
        .nodes()
        .find(|node| node.kind() == crate::grammar::SyntaxKind::ThreadExpression)
        .expect("fixture must retain one Thread expression")
        .cast::<ThreadExpressionKind>()
        .unwrap();
    let AttachedRequiredThreadExpressionBody::Present(body) = thread.statement_body().unwrap()
    else {
        panic!("fixture Thread body must be present");
    };
    body
}

fn assert_one_nested_item(body: &AttachedRequiredNestedThreadFlowBody) {
    let AttachedRequiredNestedThreadFlowBody::Present(body) = body else {
        panic!("authored loop-family body must be present");
    };
    assert_eq!(body.items().len(), 1);
    assert!(matches!(
        body.items()[0],
        AttachedThreadFlowItem::Include(_)
    ));
}

#[test]
fn loop_while_while_let_and_for_own_typed_heads_and_thread_flow_bodies() {
    let body = flow_body(concat!(
        "flow loops {\n",
        "    loop { include @flow.looping }\n",
        "    while ready { include @flow.while_ready }\n",
        "    while let item = source when allowed { include @flow.while_item }\n",
        "    for item in source { include @flow.for_item }\n",
        "}\n",
    ));

    let AttachedThreadFlowItem::Loop(statement) = &body.items()[0] else {
        panic!("first item must remain Loop");
    };
    let loop_statement: AttachedLoopStatement = statement.semantics().unwrap();
    assert_one_nested_item(loop_statement.body());
    assert!(!loop_statement.has_recovery());

    let AttachedThreadFlowItem::While(statement) = &body.items()[1] else {
        panic!("second item must remain While");
    };
    let while_statement: AttachedWhileStatement = statement.semantics().unwrap();
    assert!(matches!(
        while_statement.condition(),
        RequiredStatementExpressionNode::Expression(condition)
            if condition.source_text() == "ready"
    ));
    assert_one_nested_item(while_statement.body());
    assert!(!while_statement.has_recovery());

    let AttachedThreadFlowItem::WhileLet(statement) = &body.items()[2] else {
        panic!("third item must remain WhileLet");
    };
    let while_let: AttachedWhileLetStatement = statement.semantics().unwrap();
    assert_eq!(while_let.pattern().syntax().source_text(), "item");
    assert!(matches!(
        while_let.scrutinee(),
        RequiredStatementExpressionNode::Expression(scrutinee)
            if scrutinee.source_text() == "source"
    ));
    assert!(matches!(
        while_let.guard(),
        Some(RequiredStatementExpressionNode::Expression(guard))
            if guard.source_text() == "allowed"
    ));
    assert_one_nested_item(while_let.body());
    assert!(!while_let.has_recovery());

    let AttachedThreadFlowItem::For(statement) = &body.items()[3] else {
        panic!("fourth item must remain For");
    };
    let for_statement: AttachedForStatement = statement.semantics().unwrap();
    assert_eq!(for_statement.pattern().syntax().source_text(), "item");
    assert!(matches!(
        for_statement.source(),
        RequiredStatementExpressionNode::Expression(source) if source.source_text() == "source"
    ));
    assert_one_nested_item(for_statement.body());
    assert!(!for_statement.has_recovery());
}

#[test]
fn for_source_accepts_transparent_group_delimiters_without_a_second_expression_identity() {
    let body = flow_body(concat!(
        "flow grouped_for {\n",
        "    for item in (Counter { start: 0, end: 3 }) { include @flow.consume }\n",
        "}\n",
    ));
    let [AttachedThreadFlowItem::For(statement)] = body.items() else {
        panic!("fixture must retain one For statement");
    };
    let statement = statement.semantics().expect("grouped For source attaches");
    assert!(matches!(
        statement.source(),
        RequiredStatementExpressionNode::Expression(source)
            if source.source_text() == "Counter { start: 0, end: 3 }"
    ));
    assert_one_nested_item(statement.body());
    assert!(!statement.has_recovery());
}

#[test]
fn thread_expression_reuses_the_same_loop_family_owners() {
    let body = thread_body(concat!(
        "flow host {\n",
        "    let worker = thread {\n",
        "        loop {}\n",
        "        while ready {}\n",
        "        while let item = source {}\n",
        "        for item in source {}\n",
        "    }\n",
        "}\n",
    ));

    assert!(matches!(
        &body.items()[0],
        AttachedThreadFlowItem::Loop(statement) if statement.semantics().is_ok()
    ));
    assert!(matches!(
        &body.items()[1],
        AttachedThreadFlowItem::While(statement) if statement.semantics().is_ok()
    ));
    assert!(matches!(
        &body.items()[2],
        AttachedThreadFlowItem::WhileLet(statement) if statement.semantics().is_ok()
    ));
    assert!(matches!(
        &body.items()[3],
        AttachedThreadFlowItem::For(statement) if statement.semantics().is_ok()
    ));
}

#[test]
fn malformed_loop_family_heads_keep_typed_slots_and_exact_missing_bodies() {
    let body = flow_body(concat!(
        "flow recovered {\n",
        "    loop\n",
        "    while\n",
        "    while let = when\n",
        "    for in\n",
        "}\n",
    ));

    let AttachedThreadFlowItem::Loop(statement) = &body.items()[0] else {
        panic!("first recovery item must remain Loop");
    };
    let statement = statement.semantics().unwrap();
    assert!(matches!(
        statement.body(),
        AttachedRequiredNestedThreadFlowBody::Missing(missing) if missing.range().is_empty()
    ));

    let AttachedThreadFlowItem::While(statement) = &body.items()[1] else {
        panic!("second recovery item must remain While");
    };
    let statement = statement.semantics().unwrap();
    assert!(matches!(
        statement.condition(),
        RequiredStatementExpressionNode::Missing(missing) if missing.range().is_empty()
    ));
    assert!(matches!(
        statement.body(),
        AttachedRequiredNestedThreadFlowBody::Missing(missing) if missing.range().is_empty()
    ));
    assert!(statement.has_recovery());

    let AttachedThreadFlowItem::WhileLet(statement) = &body.items()[2] else {
        panic!("third recovery item must remain WhileLet");
    };
    let statement = statement.semantics().unwrap();
    assert!(matches!(
        statement.pattern().value().state(),
        PatternSyntaxState::Recovered(_)
    ));
    assert!(matches!(
        statement.scrutinee(),
        RequiredStatementExpressionNode::Missing(missing) if missing.range().is_empty()
    ));
    assert!(matches!(
        statement.guard(),
        Some(RequiredStatementExpressionNode::Missing(missing)) if missing.range().is_empty()
    ));
    assert!(matches!(
        statement.body(),
        AttachedRequiredNestedThreadFlowBody::Missing(missing) if missing.range().is_empty()
    ));
    assert!(statement.has_recovery());

    let AttachedThreadFlowItem::For(statement) = &body.items()[3] else {
        panic!("fourth recovery item must remain For");
    };
    let statement = statement.semantics().unwrap();
    assert!(matches!(
        statement.pattern().value().state(),
        PatternSyntaxState::Recovered(_)
    ));
    assert!(matches!(
        statement.source(),
        RequiredStatementExpressionNode::Missing(missing) if missing.range().is_empty()
    ));
    assert!(matches!(
        statement.body(),
        AttachedRequiredNestedThreadFlowBody::Missing(missing) if missing.range().is_empty()
    ));
    assert!(statement.has_recovery());
}

#[test]
fn loop_family_unclosed_bodies_share_the_typed_missing_close_owner() {
    for head in [
        "loop",
        "while ready",
        "while let item = source",
        "for item in source",
    ] {
        let body = flow_body(&format!(
            "flow unclosed {{\n    {head} {{\n        include @flow.shared\n"
        ));
        let nested = match &body.items()[0] {
            AttachedThreadFlowItem::Loop(statement) => {
                statement.semantics().unwrap().body().clone()
            }
            AttachedThreadFlowItem::While(statement) => {
                statement.semantics().unwrap().body().clone()
            }
            AttachedThreadFlowItem::WhileLet(statement) => {
                statement.semantics().unwrap().body().clone()
            }
            AttachedThreadFlowItem::For(statement) => statement.semantics().unwrap().body().clone(),
            item => panic!("unexpected loop-family item: {:?}", item.family()),
        };
        let AttachedRequiredNestedThreadFlowBody::Present(nested) = nested else {
            panic!("authored open brace must retain a present nested body for {head}");
        };
        assert!(matches!(
            nested.close_state(),
            AttachedDelimiterState::Missing(missing) if missing.range().is_empty()
        ));
        assert!(nested.has_recovery());
    }
}
