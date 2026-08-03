use std::fmt::Write;

use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceName, SourceRange};

use super::{source_document, source_edit, syntax_database};
use crate::attachment::node::FlowItemKind;
use crate::attachment::{
    AttachedChoiceIf, AttachedChoiceItem, AttachedRequiredChoiceBody, AttachedRequiredFlowBody,
    AttachedThreadFlowItem, RequiredStatementExpressionNode,
};
use crate::grammar::kinds::SyntaxKind;
use crate::incremental::ParsedSource;
use crate::parser::ParseOptions;

fn choice_source(branch_count: usize) -> String {
    let mut source = String::from("flow reconciled_chain {\n    choice {\n");
    for index in 0..branch_count {
        if index == 0 {
            writeln!(source, "        if condition_{index} {{").unwrap();
        } else {
            writeln!(source, "        else if condition_{index} {{").unwrap();
        }
        writeln!(
            source,
            "            @.branch \"Branch\" => unit\n        }}"
        )
        .unwrap();
    }
    source.push_str(
        "        else {\n            @.fallback \"Fallback\" => unit\n        }\n    }\n}\n",
    );
    source
}

fn first_conditional(source: &ParsedSource) -> AttachedChoiceIf {
    let flow = source
        .attached()
        .nodes()
        .find(|node| node.kind() == SyntaxKind::FlowItem)
        .unwrap()
        .cast::<FlowItemKind>()
        .unwrap()
        .semantics()
        .unwrap();
    let AttachedRequiredFlowBody::Present(body) = flow.body() else {
        panic!("Choice fixture requires a Flow body");
    };
    let AttachedThreadFlowItem::Choice(choice) = &body.items()[0] else {
        panic!("first Flow item must remain Choice");
    };
    let choice = choice.semantics().unwrap();
    let AttachedRequiredChoiceBody::Present(body) = choice.expression().body() else {
        panic!("Choice body must remain present");
    };
    let [AttachedChoiceItem::If(conditional)] = body.items() else {
        panic!("the complete chain must remain one Choice item");
    };
    conditional.clone()
}

#[test]
fn flat_choice_if_chain_survives_initial_parse_and_reparse_authority() {
    const BRANCH_COUNT: usize = 256;

    let name = SourceName::path("choice-chain.arcw");
    let mut database = syntax_database();
    let text = choice_source(BRANCH_COUNT);
    let document = source_document(&name, text.clone());
    let initial = database
        .parse_initial(
            SourceSnapshotId::initial(name),
            document,
            ParseOptions::default(),
        )
        .expect("initial Choice chain parse");
    let initial_conditional = first_conditional(&initial);
    assert_eq!(initial_conditional.branches().len(), BRANCH_COUNT);
    assert!(initial_conditional.else_body().is_some());
    assert!(
        initial_conditional
            .syntax()
            .syntax()
            .children()
            .iter()
            .filter(|child| child.kind() == SyntaxKind::ChoiceIfBranch)
            .count()
            == BRANCH_COUNT
    );

    let target = "condition_128";
    let start = text.find(target).expect("edited branch condition");
    let reparsed = database
        .reparse(
            &initial,
            &[source_edit(
                &initial,
                SourceRange::new(start, start + target.len()),
                "condition_128_reparsed",
            )],
            ParseOptions::default(),
        )
        .expect("Choice chain reparse");
    let reparsed_conditional = first_conditional(&reparsed);
    assert_eq!(reparsed_conditional.branches().len(), BRANCH_COUNT);
    assert!(reparsed_conditional.else_body().is_some());
    let RequiredStatementExpressionNode::Expression(condition) =
        reparsed_conditional.branches()[128].condition()
    else {
        panic!("reparsed condition must remain authored");
    };
    assert_eq!(condition.syntax().source_text(), "condition_128_reparsed");
}
