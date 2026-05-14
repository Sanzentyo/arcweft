use super::support::*;

#[test]
fn lowers_edge_case_flow_to_hir_without_raw_reparse() {
    let tree = parse_ok(
        r#"
flow @flow.opening opening {
    bg(@asset.bg.room, fade = 300ms)
    let (actor, (_, voice)) = alice.say()[聞いて。[p]]
    try await load_opening_assets() with { pending p => scene @scene.loading { progress p.ratio } }
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

    let hir = lower_to_hir(&tree).expect("edge flow lowers");
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
fn lowering_rejects_unstructured_raw_items() {
    let tree = parse_ok("unknown top level syntax");
    let errors = lower_to_hir(&tree).expect_err("raw item cannot lower");
    assert!(errors[0].message().contains("raw"));
}
