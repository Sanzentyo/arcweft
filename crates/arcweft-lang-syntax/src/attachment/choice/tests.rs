use std::collections::HashMap;
use std::fmt::Write;
use std::num::NonZeroU64;
use std::sync::Arc;

use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use super::{AttachedChoiceExpression, AttachedChoiceItem, AttachedRequiredChoiceBody};
use crate::attachment::node::FlowItemKind;
use crate::attachment::{
    AttachedRequiredChoiceOptionBody, AttachedRequiredFlowBody, AttachedThreadFlowItem,
    GrammarIdentityMap, RequiredStatementExpressionNode, SyntaxDatabaseId, SyntaxLineageId,
    SyntaxNodeId, SyntaxSnapshotData, SyntaxSnapshotId, attach_typed_tree,
};
use crate::grammar::kinds::SyntaxKind;
use crate::parser::{ParseOptions, parse_shadow_document};

fn attach(text: &str) -> Arc<SyntaxSnapshotData> {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcw:/choice-attachment-test").unwrap(),
            SourceName::path("choice-attachment-test.arcw"),
            text,
        )
        .unwrap(),
    );
    let build = parse_shadow_document(&document, ParseOptions::default()).unwrap();
    let database = SyntaxDatabaseId::from_raw_for_test(NonZeroU64::new(223).unwrap());
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

fn first_choice(snapshot: &Arc<SyntaxSnapshotData>) -> AttachedChoiceExpression {
    let flow = snapshot
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
    choice.semantics().unwrap().expression().clone()
}

#[test]
fn long_else_if_chain_attaches_as_flat_source_ordered_branches() {
    const BRANCH_COUNT: usize = 512;

    let mut source = String::from("flow long_chain {\n    choice {\n");
    for index in 0..BRANCH_COUNT {
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

    let snapshot = attach(&source);
    let choice = first_choice(&snapshot);
    let AttachedRequiredChoiceBody::Present(body) = choice.body() else {
        panic!("Choice body must remain present");
    };
    let [AttachedChoiceItem::If(conditional)] = body.items() else {
        panic!("the complete chain must remain one Choice item");
    };
    assert_eq!(conditional.branches().len(), BRANCH_COUNT);
    assert!(matches!(
        conditional.else_body(),
        Some(AttachedRequiredChoiceBody::Present(_))
    ));
    assert!(!choice.has_recovery());
}

#[test]
fn terminal_else_stops_before_duplicate_else_recovery() {
    let snapshot = attach(concat!(
        "flow duplicate_else {\n",
        "    choice {\n",
        "        if first { @.first \"First\" => unit }\n",
        "        else if second { @.second \"Second\" => unit }\n",
        "        else if third { @.third \"Third\" => unit }\n",
        "        else { @.fallback \"Fallback\" => unit } else { @.duplicate \"Duplicate\" => unit }\n",
        "        @.later \"Later\" => unit\n",
        "    }\n",
        "}\n",
    ));
    let choice = first_choice(&snapshot);
    let AttachedRequiredChoiceBody::Present(body) = choice.body() else {
        panic!("Choice body must remain present");
    };
    assert_eq!(body.items().len(), 3);
    let AttachedChoiceItem::If(conditional) = &body.items()[0] else {
        panic!("the valid if chain must remain first");
    };
    assert_eq!(conditional.branches().len(), 3);
    assert!(conditional.else_body().is_some());
    assert!(matches!(body.items()[1], AttachedChoiceItem::Recovered(_)));
    assert!(matches!(body.items()[2], AttachedChoiceItem::CompactArm(_)));
}

#[test]
fn postfix_record_heads_are_not_reclassified_as_choice_bodies() {
    let snapshot = attach(concat!(
        "flow postfix_heads {\n",
        "    choice {\n",
        "        option Route { key: \"main\" }.id\n",
        "        for route in Routes { active: true }.visible\n",
        "    }\n",
        "}\n",
    ));
    let choice = first_choice(&snapshot);
    let AttachedRequiredChoiceBody::Present(body) = choice.body() else {
        panic!("Choice body must remain present");
    };

    let AttachedChoiceItem::Option(option) = &body.items()[0] else {
        panic!("first item must remain a full option");
    };
    let RequiredStatementExpressionNode::Expression(id) = option.id() else {
        panic!("postfix option ID must remain an expression");
    };
    assert_eq!(id.syntax().source_text(), "Route { key: \"main\" }.id");
    assert!(matches!(
        option.body(),
        AttachedRequiredChoiceOptionBody::Missing(_)
    ));

    let AttachedChoiceItem::For(loop_item) = &body.items()[1] else {
        panic!("second item must remain Choice For");
    };
    let RequiredStatementExpressionNode::Expression(source) = loop_item.source() else {
        panic!("postfix source must remain an expression");
    };
    assert_eq!(
        source.syntax().source_text(),
        "Routes { active: true }.visible"
    );
    assert!(matches!(
        loop_item.body(),
        AttachedRequiredChoiceBody::Missing(_)
    ));
}

#[test]
fn multiline_record_heads_keep_the_final_brace_as_the_body() {
    let snapshot = attach(concat!(
        "flow multiline_heads {\n",
        "    choice {\n",
        "        option Route {\n",
        "            key: \"main\"\n",
        "        } {\n",
        "            label = \"Route\"\n",
        "        }\n",
        "        for route in Routes {\n",
        "            active: true\n",
        "        } {\n",
        "            @.route \"Route\" => unit\n",
        "        }\n",
        "    }\n",
        "}\n",
    ));
    let choice = first_choice(&snapshot);
    let AttachedRequiredChoiceBody::Present(body) = choice.body() else {
        panic!("Choice body must remain present");
    };

    let AttachedChoiceItem::Option(option) = &body.items()[0] else {
        panic!("first item must remain a full option");
    };
    let RequiredStatementExpressionNode::Expression(id) = option.id() else {
        panic!("multiline record option ID must remain an expression");
    };
    assert_eq!(
        id.syntax().source_text(),
        "Route {\n            key: \"main\"\n        }"
    );
    assert!(matches!(
        option.body(),
        AttachedRequiredChoiceOptionBody::Present(_)
    ));

    let AttachedChoiceItem::For(loop_item) = &body.items()[1] else {
        panic!("second item must remain Choice For");
    };
    let RequiredStatementExpressionNode::Expression(source) = loop_item.source() else {
        panic!("multiline record source must remain an expression");
    };
    assert_eq!(
        source.syntax().source_text(),
        "Routes {\n            active: true\n        }"
    );
    assert!(matches!(
        loop_item.body(),
        AttachedRequiredChoiceBody::Present(_)
    ));
}
