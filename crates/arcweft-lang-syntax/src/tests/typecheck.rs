use super::support::*;

#[test]
fn typechecks_flow_signature_parameters_as_locals() {
    let tree = parse_ok(
        r"
pub flow @flow.opening opening(state: GameState) -> Result<FlowExit, FlowError> {
    let _ = state
}
",
    );
    let hir = lower_to_hir(&tree).expect("flow signature fixture lowers");
    assert!(hir.flows()[0].signature().is_some());
    validate_typecheck_ready(&hir).expect("flow signature fixture is typecheck-ready");
    typecheck_hir(&hir, &TypeCheckEnv::new()).expect("flow parameters bind as locals");
}

#[test]
fn typecheck_rejects_try_on_non_result_expression() {
    let tree = parse_ok(
        r"
flow @flow.trying trying {
    let bad = score?
}
",
    );
    let hir = lower_to_hir(&tree).expect("bad try fixture lowers");
    let errors = typecheck_hir(
        &hir,
        &TypeCheckEnv::new().with_symbol("score", TypeKind::Int),
    )
    .expect_err("try on non-result expression is rejected");
    assert!(errors.iter().any(|error| {
        error
            .message()
            .contains("`?` requires Result<T, E> or Option<T>")
    }));
}

#[test]
fn typecheck_rejects_function_return_type_mismatch() {
    let tree = parse_ok(
        r"
fn bad_score() -> Bool {
    1
}
",
    );
    let hir = lower_to_hir(&tree).expect("function lowers");
    let errors = typecheck_hir(&hir, &TypeCheckEnv::new()).expect_err("return mismatch");
    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("function `bad_score` returns"))
    );
}

#[test]
fn typecheck_rejects_unary_not_on_non_bool_expression() {
    let tree = parse_ok(
        r"
flow @flow.branching branching {
    if !state.count {
        goto @flow.ready
    }
}
",
    );
    let hir = lower_to_hir(&tree).expect("unary not fixture lowers");
    let errors = typecheck_hir(
        &hir,
        &TypeCheckEnv::new().with_symbol("state.count", TypeKind::Int),
    )
    .expect_err("unary not on non-bool is rejected");

    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("not operand"))
    );
}

#[test]
fn typecheck_readiness_rejects_raw_dialogue_expressions() {
    let tree = parse_ok(
        r#"
alice[
    #[fmt("夢", color=)]を見た。[p]
]
"#,
    );
    let hir = lower_to_hir(&tree).expect("raw dialogue expression still lowers");
    let errors = validate_typecheck_ready(&hir).expect_err("raw expr blocks type checking");

    assert!(errors[0].message().contains("raw expression"));
}

#[test]
fn typechecks_edge_case_hir_with_explicit_environment() {
    let tree = parse_ok(
        r#"
flow @flow.opening opening {
    show alice normal at=right fade=220ms
    let (actor, (_, voice)) = alice.say(voice=auto)[聞いて。[p]]
    try await load_opening_assets() with { pending p => scene @scene.loading { progress p.ratio } }
    alice[
        #[fmt("夢", color=blue)]を見た。[p]
    ]
    with:
        at(0.42s): alice.stage.face(worried)
    choice @choice.opening.first {
        @choice.opening.listen "聞く" if state.affection[@character.alice] >= 3 -> @flow.alice_intro
    }
    goto @flow.title
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("typecheck fixture lowers");
    let env = TypeCheckEnv::new()
        .with_symbol("alice", TypeKind::Ref(EntityKind::Character))
        .with_symbol("alice.stage", TypeKind::Named("StageActor".to_owned()))
        .with_symbol("auto", TypeKind::Named("VoicePolicy".to_owned()))
        .with_symbol("blue", TypeKind::Named("Color".to_owned()))
        .with_symbol("normal", TypeKind::Named("Pose".to_owned()))
        .with_symbol("right", TypeKind::Named("StagePosition".to_owned()))
        .with_symbol("worried", TypeKind::Named("Face".to_owned()))
        .with_symbol("end", TypeKind::Duration)
        .with_symbol(
            "state.affection",
            TypeKind::Named("Map<Ref<Character>, Int>".to_owned()),
        )
        .with_function("fmt", TypeKind::DisplayText)
        .with_function(
            "load_opening_assets",
            TypeKind::Need {
                ready: Box::new(TypeKind::Unit),
                error: Box::new(TypeKind::Named("AssetError".to_owned())),
            },
        )
        .with_method(
            TypeKind::Ref(EntityKind::Character),
            "say",
            TypeKind::Named("SayBuilder".to_owned()),
        )
        .with_method(
            TypeKind::Named("StageActor".to_owned()),
            "face",
            TypeKind::Named("StageCue".to_owned()),
        )
        .with_index(
            TypeKind::Named("Map<Ref<Character>, Int>".to_owned()),
            TypeKind::Int,
        );

    typecheck_hir(&hir, &env).expect("edge fixture typechecks");
}

#[test]
fn typechecks_fragment_hir_and_include_target() {
    let tree = parse_ok(
        r"
pub fragment @frag.alice_enters alice_enters: FlowFragment {
    alice: おはよう。[p]
}

flow @flow.opening opening {
    include @frag.alice_enters
}
",
    );
    let hir = lower_to_hir(&tree).expect("fragment include fixture lowers");
    let env = TypeCheckEnv::new().with_symbol("alice", TypeKind::Ref(EntityKind::Character));

    assert_eq!(hir.flows()[0].kind(), FlowKind::Fragment);
    typecheck_hir(&hir, &env).expect("fragment include fixture typechecks");
}

#[test]
fn typecheck_reports_wrong_choice_target_kind() {
    let tree = parse_ok(
        r#"
flow @flow.opening opening {
    choice @choice.opening.first {
        @choice.opening.listen "聞く" -> @asset.bg.room
    }
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("bad choice target lowers");
    let errors =
        typecheck_hir(&hir, &TypeCheckEnv::new()).expect_err("choice target must be a flow ref");

    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("choice target"))
    );
}
