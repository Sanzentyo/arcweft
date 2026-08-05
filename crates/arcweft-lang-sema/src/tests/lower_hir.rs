use super::support::*;

#[test]
fn lowers_edge_case_flow_to_hir_without_raw_reparse() {
    let tree = parse_ok(
        r#"
flow opening {
    bg(@asset:.bg.room, fade = 300ms)
    let (actor, (_, voice)) = alice.say()[聞いて。[p]]
    try await load_opening_assets() with { pending p => progress.set(p.ratio) }
    alice[
        今日は｜変な夢《へんなゆめ》を見たんだ。[p]
    ]
    with:
        at(end-250ms): alice.stage.face(worried)
    choice @choice.opening.first {
        @choice.opening.listen "聞いてみる" if state.affection[@character.alice] >= 3 -> @flow.alice_intro
    }
    goto @flow.title
}
"#,
    );

    let hir = lower_document_to_hir(tree.document(), tree.typed_tree()).expect("edge flow lowers");
    let flow = &hir.flows()[0];
    assert!(
        flow.body()
            .iter()
            .any(|item| matches!(item, HirFlowItem::Stmt(Stmt::Let { .. })))
    );
    assert!(
        flow.body()
            .iter()
            .any(|item| matches!(item, HirFlowItem::Await(await_with) if await_with.applies_try()))
    );
    assert!(flow.body().iter().any(
        |item| matches!(item, HirFlowItem::Dialogue(dialogue) if dialogue.callee() == "alice")
    ));
    assert!(
        flow.body().iter().any(
            |item| matches!(item, HirFlowItem::Dialogue(dialogue) if dialogue.plan().is_some())
        )
    );
    assert!(flow
            .body()
            .iter()
            .any(|item| matches!(item, HirFlowItem::Choice(choice) if choice.options()[0].condition().is_some())));
}

#[test]
fn parser_rejects_unstructured_top_level_syntax() {
    let errors = parse_errors("unknown top level syntax");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message(), "unexpected top-level item");
}

#[test]
fn lowering_rejects_flow_recovery_nodes_with_span() {
    let parsed = parse_recovered(
        r"
flow @flow.raw_example {
    unknown surface form
}
",
    );
    assert_eq!(parsed.errors().len(), 1);
    assert_eq!(parsed.errors()[0].message(), "unsupported flow item");
    let errors = lower_document_to_hir(parsed.document(), parsed.typed_tree())
        .expect_err("raw flow item cannot lower");
    assert!(errors[0].message().contains("FlowItem"));
    assert!(errors[0].range().is_some());
}
