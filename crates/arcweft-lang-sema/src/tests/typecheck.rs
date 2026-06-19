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
fn typechecks_canonical_bare_character_as_dialogue_callee() {
    let tree = parse_ok(
        r#"
pub character alice {
    display = "Alice"
}

flow opening {
    alice: おはよう。[p]
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("canonical character fixture lowers");
    validate_typecheck_ready(&hir).expect("canonical character fixture is typecheck-ready");
    typecheck_hir(&hir, &TypeCheckEnv::new())
        .expect("bare character identity suffix binds as dialogue callee");
}

#[test]
fn adapter_function_signature_checks_arguments() {
    let tree = parse_ok(
        r"
flow @flow.opening opening {
    let rank = mini_games.truck.score_to_rank(score = 42i32)
}
",
    );
    let hir = lower_to_hir(&tree).expect("adapter signature fixture lowers");
    let env = TypeCheckEnv::new().with_function_signature(
        "mini_games.truck.score_to_rank",
        FunctionSignature::new(
            TypeKind::Named("Rank".to_owned()),
            [FunctionParam::required("score", TypeKind::I32)],
        ),
    );
    typecheck_hir(&hir, &env).expect("adapter signature typechecks");

    let bad = parse_ok(
        r#"
flow @flow.opening opening {
    let rank = mini_games.truck.score_to_rank(score = "bad")
}
"#,
    );
    let bad_hir = lower_to_hir(&bad).expect("bad adapter signature fixture lowers");
    let errors = typecheck_hir(&bad_hir, &env).expect_err("argument mismatch is rejected");
    assert!(errors.iter().any(|error| matches!(
        error.kind(),
        TypeCheckErrorKind::ArgumentTypeMismatch {
            function,
            argument,
            expected: TypeKind::I32,
            actual: TypeKind::String,
        } if function == "mini_games.truck.score_to_rank" && argument == "score"
    )));
}

#[test]
fn adapter_function_effects_require_matching_capability() {
    let tree = parse_ok(
        r#"
flow @flow.opening opening {
    let body = adapter.read_text(path = "story.arcw")
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("adapter effect fixture lowers");
    let env = TypeCheckEnv::new()
        .with_function_signature(
            "adapter.read_text",
            FunctionSignature::new(
                TypeKind::String,
                [FunctionParam::required("path", TypeKind::String)],
            ),
        )
        .with_function_effects("adapter.read_text", ["fs.read".to_owned()]);

    let errors = typecheck_hir(&hir, &env).expect_err("missing adapter effect is rejected");
    assert!(errors.iter().any(|error| {
        error
            .message()
            .contains("calling `adapter.read_text` requires effect capability `fs.read`")
    }));

    let allowed_tree = parse_ok(
        r#"
flow @flow.opening opening
effects { fs.read }
{
    let body = adapter.read_text(path = "story.arcw")
}
"#,
    );
    let allowed_hir = lower_to_hir(&allowed_tree).expect("allowed adapter effect fixture lowers");
    typecheck_hir(&allowed_hir, &env).expect("flow effect contract grants adapter call");
}

#[test]
fn typecheck_report_counts_type_and_borrow_work() {
    let tree = parse_ok(
        r#"
flow @flow.borrow_stats borrow_stats {
    borrow pixels() as pixels: &'asset [Rgba8] {
        let alias = pixels
        drop(pixels)
    }
    return "done"
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("borrow stats fixture lowers");
    validate_typecheck_ready(&hir).expect("borrow stats fixture is typecheck-ready");
    let report = analyze_types(
        &hir,
        &TypeCheckEnv::new().with_function(
            "pixels",
            TypeKind::Shared(Box::new(TypeKind::Named("Rgba8".to_owned()))),
        ),
    );
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
    assert_eq!(report.stats.flows, 1);
    assert!(report.stats.statements >= 3);
    assert!(report.stats.expressions >= 4);
    assert!(report.stats.borrow_binding_groups >= 1);
    assert!(report.stats.borrow_bindings >= 1);
    assert!(report.stats.active_borrow_removes >= 1);
    assert!(report.stats.max_active_borrows >= 1);
    assert_eq!(report.stats.borrow_state_full_clones, 0);
    assert_eq!(report.stats.borrow_state_cloned_bindings, 0);
    assert_eq!(report.stats.judgments, report.judgments.len());
    assert_eq!(
        report.stats.judgments,
        report.stats.expr_judgments
            + report.stats.expected_judgments
            + report.stats.let_binding_judgments
            + report.stats.return_judgments
    );
    assert!(
        report
            .judgments
            .iter()
            .any(|judgment| matches!(&judgment.subject, TypeJudgmentSubject::LetBinding { .. }))
    );
    assert!(
        report
            .judgments
            .iter()
            .any(|judgment| matches!(&judgment.subject, TypeJudgmentSubject::Return { .. }))
    );
}

#[test]
fn typechecks_std_float_constants_and_functions() {
    let tree = parse_ok(
        r"
flow @flow.float_std float_std {
    let root = std.f32.sqrt(4.0f32)
    let exact = std.f32.to_bits(std.f32.nan)
    let restored = std.f32.from_bits(exact)
    let widened = std.f32.to_f64(root)
    let narrowed = std.f64.to_f32(widened)
    let ok = std.f64.is_nan(std.f64.nan)
    return narrowed
}
",
    );
    let hir = lower_to_hir(&tree).expect("std float fixture lowers");
    typecheck_hir(&hir, &TypeCheckEnv::new()).expect("std float calls typecheck");
}

#[test]
fn borrow_branch_merge_records_delta_without_full_clone() {
    let tree = parse_ok(
        r#"
flow @flow.borrow_branch_delta borrow_branch_delta {
    borrow pixels() as pixels: &'asset [Rgba8] {
        if ready {
            drop(pixels)
        }
        drop(pixels)
    }
    return "done"
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("borrow branch fixture lowers");
    validate_typecheck_ready(&hir).expect("borrow branch fixture is typecheck-ready");
    let report = analyze_types(
        &hir,
        &TypeCheckEnv::new()
            .with_symbol("ready", TypeKind::Bool)
            .with_function(
                "pixels",
                TypeKind::Shared(Box::new(TypeKind::Named("Rgba8".to_owned()))),
            ),
    );
    assert!(
        report
            .diagnostics
            .iter()
            .any(|error| error.message().contains("may already have been dropped")),
        "conditional drop should remain a borrow-state error: {:?}",
        report.diagnostics
    );
    assert!(report.stats.borrow_state_snapshots >= 1);
    assert!(report.stats.borrow_state_delta_entries >= 1);
    assert!(report.stats.borrow_state_merge_keys >= 1);
    assert_eq!(report.stats.borrow_state_full_clones, 0);
    assert_eq!(report.stats.borrow_state_cloned_bindings, 0);
}

#[test]
fn typecheck_for_loop_binds_sequence_item_type() {
    let tree = parse_ok(
        r#"
#[pure]
fn score(base: i64, bonus: i64) -> i64 {
    return base * (bonus + 2i64)
}

flow @flow.for_pure for_pure {
    let values: Vec<i64> = [1i64, 2i64, 3i64, 4i64]
    for item in values {
        let scored = score(item, 2i64)
        log.info(scored)
    }
    return "done"
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("for pure fixture lowers");
    validate_typecheck_ready(&hir).expect("for pure fixture is typecheck-ready");
    typecheck_hir(&hir, &TypeCheckEnv::new()).expect("for item binds as i64");
}

#[test]
fn typecheck_for_loop_binds_stream_item_type() {
    let tree = parse_ok(
        r"
stream fn passthrough(frames: Stream<IteratorItem, CaptureError>) -> Stream<IteratorItem, CaptureError> {
    for frame in frames {
        yield frame
    }
}
",
    );
    let hir = lower_to_hir(&tree).expect("stream for fixture lowers");
    validate_typecheck_ready(&hir).expect("stream for fixture is typecheck-ready");
    typecheck_hir(&hir, &TypeCheckEnv::new()).expect("stream for item binds as stream item");
}

#[test]
fn numeric_primitive_types_keep_explicit_widths() {
    assert_eq!(TypeKind::primitive_name("i32"), Some(TypeKind::I32));
    assert_eq!(TypeKind::primitive_name("i64"), Some(TypeKind::I64));
    assert_eq!(TypeKind::primitive_name("usize"), Some(TypeKind::USize));
    assert_eq!(TypeKind::primitive_name("f32"), Some(TypeKind::F32));
    assert_ne!(
        TypeKind::primitive_name("i32"),
        TypeKind::primitive_name("usize")
    );
    assert_ne!(
        TypeKind::primitive_name("f32"),
        TypeKind::primitive_name("f64")
    );
}

#[test]
fn unsuffixed_numeric_literals_require_expected_type() {
    let tree = parse_ok(
        r"
flow @flow.bad bad {
    let n = 1
}
",
    );
    let hir = lower_to_hir(&tree).expect("unsuffixed literal fixture lowers");
    let errors =
        typecheck_hir(&hir, &TypeCheckEnv::new()).expect_err("untyped integer is rejected");
    assert!(errors.iter().any(|error| {
        error
            .message()
            .contains("unsuffixed integer literal requires an expected integer type")
    }));
}

#[test]
fn unsuffixed_numeric_literals_use_annotations_and_return_context() {
    let tree = parse_ok(
        r"
fn value() -> i32 {
    return 1
}

flow @flow.good good(input: i32) -> i32 {
    let annotated: i32 = 2
    if input > 0 {
        return annotated
    } else {
        return 0
    }
}
",
    );
    let hir = lower_to_hir(&tree).expect("expected numeric fixture lowers");
    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
    assert!(report.judgments.iter().any(|judgment| {
        matches!(
            (&judgment.subject, judgment.rule, judgment.expected_type()),
            (
                TypeJudgmentSubject::Expr { kind },
                TypeJudgmentRule::Expected,
                Some(TypeKind::I32)
            ) if *kind == "literal"
        )
    }));
    assert!(report.judgments.iter().any(|judgment| {
        matches!(
            (&judgment.subject, judgment.rule, &judgment.expected),
            (
                TypeJudgmentSubject::Expr { kind },
                TypeJudgmentRule::Expected,
                Some(TypeJudgmentExpected::SameAsJudgment)
            ) if *kind == "literal"
        )
    }));
    assert!(
        report.stats.type_compatibility_checks > 0,
        "expected type compatibility checks to be counted"
    );
}

#[test]
fn numeric_literal_suffixes_are_checked_against_annotations() {
    let ok = parse_ok(
        r"
flow @flow.ok ok {
    let n: i32 = 1i32
    let f: f32 = 1.0f32
}
",
    );
    let hir = lower_to_hir(&ok).expect("suffixed numeric fixture lowers");
    typecheck_hir(&hir, &TypeCheckEnv::new()).expect("matching numeric suffixes typecheck");

    let bad = parse_ok(
        r"
flow @flow.bad bad {
    let n: i32 = 1u64
}
",
    );
    let hir = lower_to_hir(&bad).expect("mismatched suffix fixture lowers");
    let errors =
        typecheck_hir(&hir, &TypeCheckEnv::new()).expect_err("mismatched suffix is rejected");
    assert!(errors.iter().any(|error| {
        error
            .message()
            .contains("let annotation expects I32, but expression has U64")
    }));
}

#[test]
fn numeric_sequence_literals_use_expected_item_fast_path() {
    let values = (0..64)
        .map(|value| format!("{value}i64"))
        .collect::<Vec<_>>()
        .join(", ");
    let tree = parse_ok(format!(
        r#"
flow @flow.numeric_seq numeric_seq {{
    let values: Vec<i64> = [{values}]
    return "done"
}}
"#
    ));
    let hir = lower_to_hir(&tree).expect("numeric sequence fixture lowers");
    validate_typecheck_ready(&hir).expect("numeric sequence fixture is typecheck-ready");
    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
    assert!(
        report.stats.expressions < 16,
        "numeric sequence should not recursively typecheck each literal: {:?}",
        report.stats
    );

    let bad = parse_ok(
        r"
flow @flow.bad bad {
    let values: Vec<i64> = [1i64, 2u64]
}
",
    );
    let hir = lower_to_hir(&bad).expect("mismatched sequence fixture lowers");
    let errors =
        typecheck_hir(&hir, &TypeCheckEnv::new()).expect_err("mismatched item is rejected");
    assert!(errors.iter().any(|error| {
        error
            .message()
            .contains("sequence literal items must have the same type")
    }));
}

#[test]
fn typechecks_explicit_route_parameter_bindings() {
    let tree = parse_ok(
        r#"
entry server @entry.http {
    route GET "/hello/:name" -> @flow.hello(name = :name)
}

flow @flow.hello hello(name: String) -> String {
    return name
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("route fixture lowers");
    validate_typecheck_ready(&hir).expect("route fixture is typecheck-ready");
    typecheck_hir(&hir, &TypeCheckEnv::new()).expect("explicit route binding typechecks");
}

#[test]
fn typecheck_rejects_route_parameter_mismatches() {
    let tree = parse_ok(
        r#"
entry server @entry.http {
    route GET "/hello/:name" -> @flow.hello(person = :missing)
}

flow @flow.hello hello(name: String) -> String {
    return name
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("bad route fixture lowers");
    let errors = typecheck_hir(&hir, &TypeCheckEnv::new()).expect_err("route mismatch is rejected");
    assert!(errors.iter().any(|error| {
        error
            .message()
            .contains("route binding `person` references missing path parameter `:missing`")
    }));
    assert!(errors.iter().any(|error| {
        error
            .message()
            .contains("route target `flow.hello` has no flow parameter named `person`")
    }));
    assert!(errors.iter().any(|error| {
        error
            .message()
            .contains("requires explicit binding for flow parameter `name`")
    }));
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
        &TypeCheckEnv::new().with_symbol("score", TypeKind::I64),
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
        &TypeCheckEnv::new().with_symbol("state.count", TypeKind::I64),
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
    show(@character.alice, .normal, at = .right, fade = 220ms)
    let (actor, (_, voice)) = alice.say(voice=auto)[聞いて。[p]]
    try await load_opening_assets() with { pending p => progress.set(p.ratio) }
    alice[
        #[fmt("夢", color=blue, on_error=.discard)]を見た。[p]
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
        .with_symbol("alice", TypeKind::entity_ref(EntityKind::Character))
        .with_symbol("alice.stage", TypeKind::Named("StageActor".to_owned()))
        .with_symbol("auto", TypeKind::Named("VoicePolicy".to_owned()))
        .with_symbol("blue", TypeKind::Named("Color".to_owned()))
        .with_symbol(".normal", TypeKind::Named("Pose".to_owned()))
        .with_symbol(".right", TypeKind::Named("StagePosition".to_owned()))
        .with_symbol("normal", TypeKind::Named("Pose".to_owned()))
        .with_symbol("right", TypeKind::Named("StagePosition".to_owned()))
        .with_symbol("worried", TypeKind::Named("Face".to_owned()))
        .with_symbol("end", TypeKind::Duration)
        .with_symbol(
            "state.affection",
            TypeKind::Named("OrderedMap<Ref<Character>, i64>".to_owned()),
        )
        .with_function("show", TypeKind::Unit)
        .with_function("fmt", TypeKind::DisplayText)
        .with_function(
            "load_opening_assets",
            TypeKind::Need {
                ready: Box::new(TypeKind::Unit),
                error: Box::new(TypeKind::Named("AssetError".to_owned())),
            },
        )
        .with_method(
            TypeKind::entity_ref(EntityKind::Character),
            "say",
            TypeKind::Named("SayBuilder".to_owned()),
        )
        .with_method(
            TypeKind::Named("StageActor".to_owned()),
            "face",
            TypeKind::Named("StageCue".to_owned()),
        )
        .with_index(
            TypeKind::Named("OrderedMap<Ref<Character>, i64>".to_owned()),
            TypeKind::I64,
        );

    typecheck_hir(&hir, &env).expect("edge fixture typechecks");
}

#[test]
fn typecheck_requires_inline_function_failure_policy() {
    let tree = parse_ok(
        r#"
character @character.alice Alice as alice {}

flow @flow.opening opening {
    alice: #[fmt(score, style="number")]点[p]
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("inline policy fixture lowers");
    let errors = typecheck_hir(
        &hir,
        &TypeCheckEnv::new()
            .with_symbol("alice", TypeKind::entity_ref(EntityKind::Character))
            .with_symbol("score", TypeKind::I64)
            .with_function("fmt", TypeKind::DisplayText),
    )
    .expect_err("inline function call without policy is rejected");

    assert!(errors.iter().any(|error| {
        matches!(
            error.kind(),
            crate::diagnostics::TypeCheckErrorKind::InlineCallErrorPolicyMissing { function }
                if function == "fmt"
        )
    }));
}

#[test]
fn typecheck_accepts_inline_function_failure_policy() {
    let tree = parse_ok(
        r#"
character @character.alice Alice as alice {}

flow @flow.opening opening {
    alice: #[fmt(score, style="number", on_error=InlineFailure.fallback("?"))]点[p]
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("inline policy fixture lowers");

    typecheck_hir(
        &hir,
        &TypeCheckEnv::new()
            .with_symbol("alice", TypeKind::entity_ref(EntityKind::Character))
            .with_symbol("score", TypeKind::I64)
            .with_function("fmt", TypeKind::DisplayText),
    )
    .expect("inline function call with policy typechecks");
}

#[test]
fn typecheck_accepts_line_default_inline_failure_policy() {
    let tree = parse_ok(
        r#"
character @character.alice Alice as alice {}

flow @flow.opening opening {
    alice(inline_error=.fail): #[fmt(score, style="number")]点[p]
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("inline default policy fixture lowers");

    typecheck_hir(
        &hir,
        &TypeCheckEnv::new()
            .with_symbol("alice", TypeKind::entity_ref(EntityKind::Character))
            .with_symbol("score", TypeKind::I64)
            .with_function("fmt", TypeKind::DisplayText),
    )
    .expect("line default inline failure policy typechecks");
}

#[test]
fn typecheck_rejects_conflicting_inline_failure_policies() {
    let tree = parse_ok(
        r#"
character @character.alice Alice as alice {}

flow @flow.opening opening {
    alice: #[fmt(score, on_error=.fail, fallback="?")]点[p]
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("inline policy fixture lowers");
    let errors = typecheck_hir(
        &hir,
        &TypeCheckEnv::new()
            .with_symbol("alice", TypeKind::entity_ref(EntityKind::Character))
            .with_symbol("score", TypeKind::I64)
            .with_function("fmt", TypeKind::DisplayText),
    )
    .expect_err("conflicting inline policies are rejected");

    assert!(errors.iter().any(|error| {
        matches!(
            error.kind(),
            crate::diagnostics::TypeCheckErrorKind::InlineFailurePolicyConflict { function }
                if function == "fmt"
        )
    }));
}

#[test]
fn typecheck_rejects_unknown_inline_failure_policy() {
    let tree = parse_ok(
        r"
character @character.alice Alice as alice {}

flow @flow.opening opening {
    alice: #[fmt(score, on_error=.explode)]点[p]
}
",
    );
    let hir = lower_to_hir(&tree).expect("inline policy fixture lowers");
    let errors = typecheck_hir(
        &hir,
        &TypeCheckEnv::new()
            .with_symbol("alice", TypeKind::entity_ref(EntityKind::Character))
            .with_symbol("score", TypeKind::I64)
            .with_function("fmt", TypeKind::DisplayText),
    )
    .expect_err("unknown inline policy is rejected");

    assert!(errors.iter().any(|error| {
        matches!(
            error.kind(),
            crate::diagnostics::TypeCheckErrorKind::UnknownInlineFailurePolicy {
                function,
                policy,
            } if function == "fmt" && policy == ".explode"
        )
    }));
}

#[test]
fn typechecks_presentation_handle_calls_and_slot_refs() {
    let tree = parse_ok(
        r"
flow @flow.opening opening {
    let room = bg(@asset.bg.room, target = @target.scene, slot = @slot.background.main)
    let alice_on_stage = show(@character.alice, .normal, slot = @slot.character.alice.main)
    let current_room = bg.ref(target = @target.scene, slot = @slot.background.main)
    let cleared_room = bg.clear(target = @target.scene, slot = @slot.background.main)
    let current_alice = show.ref(@character.alice, slot = @slot.character.alice.main)
    let hidden_alice = hide(@character.alice, slot = @slot.character.alice.main)
}
",
    );
    let hir = lower_to_hir(&tree).expect("presentation fixture lowers");

    typecheck_hir(
        &hir,
        &TypeCheckEnv::new()
            .with_symbol("bg", TypeKind::Named("PresentationSlotApi".to_owned()))
            .with_symbol("show", TypeKind::Named("PresentationSlotApi".to_owned()))
            .with_method(
                TypeKind::Named("PresentationSlotApi".to_owned()),
                "ref",
                TypeKind::Named("PresentationHandle".to_owned()),
            )
            .with_method(
                TypeKind::Named("PresentationSlotApi".to_owned()),
                "clear",
                TypeKind::Named("PresentationHandle".to_owned()),
            ),
    )
    .expect("presentation calls typecheck");
}

#[test]
fn typechecks_presentation_image_object_call_with_named_asset_and_bounds() {
    let tree = parse_ok(
        r#"
asset @asset.bg.room {
    file = "bg/room.png"
    kind = image
}

asset @asset.bg.pulse {
    file = "bg/pulse.gif"
    kind = image
    animation = true
}

image @image.sample.pulse {
    asset = @asset.bg.pulse
    target = @target.sample.pulse
    layer = @layer.foreground
    x = 96px
    y = 72px
    width = 360px
    height = 180px
}

flow @flow.opening opening {
    let room = bg(@asset.bg.room, fit = "intrinsic", alignment.x = 1, alignment.y = 0.5, opacity = 0.75, playback.rate = 0.5, playback.local_time = 50ms)
    let pulse = image(asset = @asset.bg.pulse, id = "image.sample.pulse", target = "target.sample.pulse", layer = "layer.foreground", x = 96px, y = 72px, width = 360px, height = 180px, fit = "stretch", alignment.x = 0.25, alignment.y = 750, opacity = 0.5, playback.start = 0.1, playback.rate = 0.5, transform.tx = 24px, transform.ty = 12px, transform.m11 = 1000, transform.m22 = 1000, depth = 2500, enabled = true, visible = true, action = "action.inspect.pulse", param.role = "animated-hotspot", proxy.id = "proxy.pulse.hotspot", proxy.type = "PulseHotspot", proxy.role = "inspect", proxy.layer = "layer.hit", proxy.depth = 2600, proxy.hit_test = true, proxy.param.channel = "preview")
    let declared = image(@image.sample.pulse)
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("presentation image fixture lowers");

    validate_hir_references(&hir, &registry_from_hir(&hir)).expect("declared image assets resolve");
    typecheck_hir(&hir, &TypeCheckEnv::new()).expect("presentation image call typechecks");
}

#[test]
fn typecheck_rejects_presentation_slot_family_mismatch() {
    let tree = parse_ok(
        r"
flow @flow.opening opening {
    let room = bg(@asset.bg.room, slot = @slot.character.alice.main)
}
",
    );
    let hir = lower_to_hir(&tree).expect("presentation slot fixture lowers");
    let errors = typecheck_hir(&hir, &TypeCheckEnv::new()).expect_err("wrong slot family");

    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("@slot.background.*"))
    );
}

#[test]
fn typecheck_requires_explicit_slots_for_simultaneous_defaults() {
    let tree = parse_ok(
        r"
flow @flow.opening opening {
    let room = bg(@asset.bg.room)
    let evening = bg(@asset.bg.evening)
}
",
    );
    let hir = lower_to_hir(&tree).expect("presentation default fixture lowers");
    let errors = typecheck_hir(&hir, &TypeCheckEnv::new()).expect_err("duplicate default slot");

    assert!(errors.iter().any(|error| {
        error
            .message()
            .contains("default slot already has live handle")
    }));
}

#[test]
fn type_ref_keeps_explicit_map_kind() {
    let ordered = crate::checker::helpers::type_ref_kind(
        &parse_type_ref("OrderedMap<Ref<Character>, i64>").expect("ordered map type parses"),
    );
    let sorted = crate::checker::helpers::type_ref_kind(
        &parse_type_ref("SortedMap<Ref<Character>, i64>").expect("sorted map type parses"),
    );
    let btree = crate::checker::helpers::type_ref_kind(
        &parse_type_ref("BTreeMap<Ref<Character>, i64>").expect("btree map type parses"),
    );
    assert!(matches!(
        ordered,
        TypeKind::Map {
            kind: MapKind::Ordered,
            ..
        }
    ));
    assert!(matches!(
        sorted,
        TypeKind::Map {
            kind: MapKind::Sorted,
            ..
        }
    ));
    assert!(matches!(
        btree,
        TypeKind::Map {
            kind: MapKind::BTree,
            ..
        }
    ));
}

#[test]
fn named_iter_item_type_extracts_sequence_items() {
    assert_eq!(
        crate::checker::helpers::named_iter_item_type("Vec<Foo>").as_deref(),
        Some("Foo")
    );
    assert_eq!(
        crate::checker::helpers::named_iter_item_type("Seq<Foo>").as_deref(),
        Some("Foo")
    );
    assert_eq!(
        crate::checker::helpers::named_iter_item_type("Slice<Foo>").as_deref(),
        Some("Foo")
    );
    assert_eq!(
        crate::checker::helpers::named_iter_item_type("Array<Foo, 3>").as_deref(),
        Some("Foo")
    );
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
    let env = TypeCheckEnv::new().with_symbol("alice", TypeKind::entity_ref(EntityKind::Character));

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

#[test]
fn typecheck_tracks_lifetime_registry_scope_and_write_capabilities() {
    let tree = parse_ok(
        r"
flow @flow.registry registry {
    'flow.flags.seen <- 1i32
}
",
    );
    let hir = lower_to_hir(&tree).expect("flow registry write lowers");
    let errors =
        typecheck_hir(&hir, &TypeCheckEnv::new()).expect_err("flow writes need capability");
    assert!(errors.iter().any(|error| {
        error
            .message()
            .contains("requires effect capability `state.write(flow)`")
    }));

    typecheck_hir(
        &hir,
        &TypeCheckEnv::new().with_capability("state.write(flow)"),
    )
    .expect("capability permits flow lifetime registry writes");

    let tree = parse_ok(
        r"
flow @flow.registry_contract registry_contract
effects { state.write('flow) }
{
    'flow.flags.seen <- 1i32
}
",
    );
    let hir = lower_to_hir(&tree).expect("flow registry write with effects lowers");
    typecheck_hir(&hir, &TypeCheckEnv::new())
        .expect("effects clause permits flow lifetime registry writes");
}

#[test]
fn typecheck_rejects_borrowed_block_final_value_escape() {
    let tree = parse_ok(
        r"
flow @flow.borrow_escape borrow_escape {
    let escaped = {
        let pixels: &'asset [Rgba8] = bg.pixels()
        pixels
    }
}
",
    );
    let hir = lower_to_hir(&tree).expect("borrow escape fixture lowers");
    let env = TypeCheckEnv::new()
        .with_symbol("bg", TypeKind::Named("ImageHandle".to_owned()))
        .with_method(
            TypeKind::Named("ImageHandle".to_owned()),
            "pixels",
            TypeKind::BorrowRef {
                lifetime: Some(LifetimeScopeKind::Named("asset".to_owned())),
                inner: Box::new(TypeKind::Slice(Box::new(TypeKind::Named(
                    "Rgba8".to_owned(),
                )))),
            },
        );
    let errors = typecheck_hir(&hir, &env).expect_err("borrowed final value cannot escape block");
    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("block final value"))
    );
}

#[test]
fn typecheck_rejects_borrowed_value_written_to_upper_lifetime() {
    let tree = parse_ok(
        r"
flow @flow.borrow_registry borrow_registry
effects { state.write('flow) }
{
    let pixels: &'asset [Rgba8] = bg.pixels()
    'flow.cache.pixels <- pixels
}
",
    );
    let hir = lower_to_hir(&tree).expect("borrow registry fixture lowers");
    let env = TypeCheckEnv::new()
        .with_symbol("bg", TypeKind::Named("ImageHandle".to_owned()))
        .with_method(
            TypeKind::Named("ImageHandle".to_owned()),
            "pixels",
            TypeKind::BorrowRef {
                lifetime: Some(LifetimeScopeKind::Named("asset".to_owned())),
                inner: Box::new(TypeKind::Slice(Box::new(TypeKind::Named(
                    "Rgba8".to_owned(),
                )))),
            },
        );
    let errors = typecheck_hir(&hir, &env).expect_err("borrowed value cannot escape to flow scope");
    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("upper lifetime registry write"))
    );
}

#[test]
fn typecheck_rejects_line_lifetime_use_outside_line_scope() {
    let tree = parse_ok(
        r"
flow @flow.registry registry {
    let focus = 'line.focus?
}
",
    );
    let hir = lower_to_hir(&tree).expect("line registry read lowers");
    let errors =
        typecheck_hir(&hir, &TypeCheckEnv::new()).expect_err("line lifetime is not in scope");
    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("lifetime `line` is not available"))
    );
}

#[test]
fn typecheck_rejects_line_lifetime_capture_across_thread_boundary() {
    let tree = parse_ok(
        r"
flow @flow.thread_capture thread_capture {
    alice(focus=.soft)[待って。[p]]
    with:
        thread motion:
            let focus = 'line.focus?
}
",
    );
    let hir = lower_to_hir(&tree).expect("thread capture fixture lowers");
    let env = TypeCheckEnv::new()
        .with_symbol("alice", TypeKind::entity_ref(EntityKind::Character))
        .with_symbol(".soft", TypeKind::FocusPatch);
    let errors = typecheck_hir(&hir, &env).expect_err("thread cannot capture line lifetime");
    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("lifetime `line` is not available"))
    );
}

#[test]
fn typechecks_patch_merge_operator_for_same_patch_family() {
    let tree = parse_ok(
        r"
flow @flow.patch patch {
    let look = .smile & .casual
    let focus = .soft & .near
}
",
    );
    let hir = lower_to_hir(&tree).expect("patch merge fixture lowers");
    let env = TypeCheckEnv::new()
        .with_symbol(".smile", TypeKind::CharacterPatch(EntityKind::Character))
        .with_symbol(".casual", TypeKind::CharacterPatch(EntityKind::Character))
        .with_symbol(".soft", TypeKind::FocusPatch)
        .with_symbol(".near", TypeKind::FocusPatch);
    typecheck_hir(&hir, &env).expect("compatible patch merges typecheck");
}

#[test]
fn typechecks_expression_thread_without_raw_hir_body() {
    let tree = parse_ok(
        r"
flow @flow.thread_expr thread_expr {
    let score_task = thread compute_score { route_score(state) }
}
",
    );
    let hir = lower_to_hir(&tree).expect("thread expression fixture lowers");
    validate_typecheck_ready(&hir).expect("thread expression body is structured");
    let env = TypeCheckEnv::new()
        .with_symbol("state", TypeKind::Named("GameState".to_owned()))
        .with_function("route_score", TypeKind::I64);
    typecheck_hir(&hir, &env).expect("thread expression typechecks");
}

#[test]
fn typechecks_char_literal_and_rejects_string_annotation_mismatch() {
    let ok = parse_ok(
        r#"
flow @flow.char_literal char_literal {
    let ch: Char = "あ"c
}
"#,
    );
    let hir = lower_to_hir(&ok).expect("char literal fixture lowers");
    typecheck_hir(&hir, &TypeCheckEnv::new()).expect("char literal typechecks");

    let bad = parse_ok(
        r#"
flow @flow.char_literal_bad char_literal_bad {
    let ch: Char = "a"
}
"#,
    );
    let hir = lower_to_hir(&bad).expect("string literal fixture lowers");
    let errors = typecheck_hir(&hir, &TypeCheckEnv::new()).expect_err("string is not Char");
    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("let annotation expects Char"))
    );
}

#[test]
fn typechecks_structured_collection_and_capacity_trait_methods() {
    let tree = parse_ok(
        r"
flow @flow.collections collections {
    let nums: Vec<i32> = [1i32, 2i32, 3i32]
    let first: i32 = nums[0i64]
    let fixed: Array<i32, 3> = [1i32, 2i32, 3i32]
    let zeros: Array<i32, 4> = [0i32; 4i64]
    let _ = nums.reserve(4i64)
    let _ = nums.shrink()
    let _ = nums.shrink_to(1i64)
    let text = String.with_capacity(16usize)
}
",
    );
    let hir = lower_to_hir(&tree).expect("collection fixture lowers");
    typecheck_hir(&hir, &TypeCheckEnv::new()).expect("collection fixture typechecks");
}

#[test]
fn typechecks_vec_map_closure_result_and_sum() {
    let tree = parse_ok(
        r"
flow @flow.map_types map_types {
    let nums: Vec<i64> = [1i64, 2i64, 3i64]
    let shifted: Vec<i64> = nums.map(|item| item + 1i64)
    let flags: Vec<Bool> = nums.map(|item| item > 1i64)
    let total: i64 = shifted.sum()
    log.info(total)
    log.info(flags)
}
",
    );
    let hir = lower_to_hir(&tree).expect("map type fixture lowers");
    typecheck_hir(&hir, &TypeCheckEnv::new()).expect("map closure result typechecks");
}

#[test]
fn typechecks_array_map_closure_result_and_sum() {
    let tree = parse_ok(
        r"
#[pure]
fn score(base: i64, bonus: i64) -> i64 {
    return base * (bonus + 2i64)
}

flow @flow.arrays arrays {
    let values: Array<i64, 4> = [2i64; 4]
    let shifted: Vec<i64> = values.map(|item| score(item, 2i64))
    let total: i64 = shifted.sum()
    return total
}
",
    );
    let hir = lower_to_hir(&tree).expect("array map fixture lowers");
    validate_typecheck_ready(&hir).expect("array map fixture is typecheck-ready");

    typecheck_hir(&hir, &TypeCheckEnv::new()).expect("array map closure result typechecks");
}

#[test]
fn typechecks_sequence_len_as_usize() {
    let tree = parse_ok(
        r#"
flow @flow.sequence_len sequence_len {
    let flags: Vec<Bool> = [true, false, true]
    let letters: Vec<Char> = ["a"c, "b"c]
    let delays: Vec<Duration> = [1ms, 2ms]
    let total: usize = flags.len() + letters.len() + delays.len()
    return "done"
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("sequence len fixture lowers");
    validate_typecheck_ready(&hir).expect("sequence len fixture is typecheck-ready");

    typecheck_hir(&hir, &TypeCheckEnv::new()).expect("sequence len typechecks as usize");
}

#[test]
fn typecheck_rejects_sum_on_non_integer_vec() {
    let tree = parse_ok(
        r"
flow @flow.bad_sum bad_sum {
    let nums: Vec<i64> = [1i64, 2i64, 3i64]
    let flags: Vec<Bool> = nums.map(|item| item > 1i64)
    let total: i64 = flags.sum()
}
",
    );
    let hir = lower_to_hir(&tree).expect("bad sum fixture lowers");
    let errors = typecheck_hir(&hir, &TypeCheckEnv::new()).expect_err("bool sum is rejected");
    assert!(errors.iter().any(|error| {
        error
            .message()
            .contains("sum receiver items must be integers")
    }));
}

#[test]
fn typecheck_rejects_array_literal_length_mismatch() {
    let tree = parse_ok(
        r"
flow @flow.array_mismatch array_mismatch {
    let fixed: Array<i32, 2> = [1, 2, 3]
}
",
    );
    let hir = lower_to_hir(&tree).expect("array mismatch fixture lowers");
    let errors = typecheck_hir(&hir, &TypeCheckEnv::new()).expect_err("length mismatch");
    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("array literal length mismatch"))
    );
}

#[test]
fn typecheck_rejects_array_repeat_length_mismatch() {
    let tree = parse_ok(
        r"
flow @flow.array_repeat_mismatch array_repeat_mismatch {
    let fixed: Array<i32, 2> = [0; 3]
}
",
    );
    let hir = lower_to_hir(&tree).expect("array repeat mismatch fixture lowers");
    let errors = typecheck_hir(&hir, &TypeCheckEnv::new()).expect_err("length mismatch");
    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("array repeat length mismatch"))
    );
}

#[test]
fn typechecks_unsafe_lifetime_audit_block_shape() {
    let tree = parse_ok(
        r#"
flow @flow.audit audit {
    unsafe lifetime @unsafe.cache_last_line
    reason = "owned summary is cloned before line scope exits"
    {
        /// SAFETY:
        /// The summary is owned and no line-scoped handle escapes.
        let summary: String = "ok"
        let _ = summary
    }
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("unsafe lifetime block lowers as a structured stmt");
    typecheck_hir(&hir, &TypeCheckEnv::new()).expect("audit metadata is complete");

    let tree = parse_ok(
        r#"
flow @flow.audit audit {
    unsafe lifetime @unsafe.cache_last_line {
        let summary: String = "ok"
    }
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("unsafe lifetime block lowers");
    let errors = typecheck_hir(&hir, &TypeCheckEnv::new()).expect_err("audit metadata is required");
    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("requires a reason"))
    );
    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("SAFETY doc comment"))
    );
}

#[test]
fn typecheck_does_not_leak_on_handler_locals_into_line_scope() {
    let tree = parse_ok(
        r"
flow @flow.handler_leak handler_leak {
    alice[待って。[mark .seen][p]]
    with:
        on mark(.seen):
            let handler_local = 1
        let later = handler_local
}
",
    );
    let hir = lower_to_hir(&tree).expect("handler leak fixture lowers");
    let env = TypeCheckEnv::new().with_symbol("alice", TypeKind::entity_ref(EntityKind::Character));
    let errors = typecheck_hir(&hir, &env).expect_err("handler locals must not leak");
    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("unknown symbol `handler_local`"))
    );
}

#[test]
fn typecheck_does_not_leak_thread_locals_into_line_scope() {
    let tree = parse_ok(
        r"
flow @flow.thread_leak thread_leak {
    alice[待って。[p]]
    with:
        thread worker:
            let worker_local = 1
        let later = worker_local
}
",
    );
    let hir = lower_to_hir(&tree).expect("thread leak fixture lowers");
    let env = TypeCheckEnv::new().with_symbol("alice", TypeKind::entity_ref(EntityKind::Character));
    let errors = typecheck_hir(&hir, &env).expect_err("thread locals must not leak");
    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("unknown symbol `worker_local`"))
    );
}

#[test]
fn typecheck_line_lifetime_guarantees_are_per_line() {
    let tree = parse_ok(
        r"
flow @flow.line_scope line_scope {
    alice(focus=.soft)[一行目。[p]]
    with:
        let focus = 'line.focus
    alice[二行目。[p]]
    with:
        let leaked = 'line.focus
}
",
    );
    let hir = lower_to_hir(&tree).expect("line scope fixture lowers");
    let env = TypeCheckEnv::new()
        .with_symbol("alice", TypeKind::entity_ref(EntityKind::Character))
        .with_symbol(".soft", TypeKind::FocusPatch);
    let errors = typecheck_hir(&hir, &env).expect_err("line guarantee must not leak");
    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("not statically guaranteed"))
    );
}

#[test]
fn typecheck_line_lifetime_drop_state_does_not_leak_to_next_line() {
    let tree = parse_ok(
        r"
flow @flow.line_drop line_drop {
    alice(focus=.soft)[一行目。[p]]
    with:
        'line.focus |> drop
    alice(focus=.soft)[二行目。[p]]
    with:
        let focus = 'line.focus
}
",
    );
    let hir = lower_to_hir(&tree).expect("line drop fixture lowers");
    let env = TypeCheckEnv::new()
        .with_symbol("alice", TypeKind::entity_ref(EntityKind::Character))
        .with_symbol(".soft", TypeKind::FocusPatch);
    typecheck_hir(&hir, &env).expect("line drop state is isolated per line");
}
