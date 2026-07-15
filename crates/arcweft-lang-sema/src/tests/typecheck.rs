use super::support::*;
use crate::check::{
    ForIterationEvidenceFamily, NumericFallbackKind, StandardIteratorFamily, TypeCheckReport,
};
use arcweft_data::DataFormat;

#[test]
fn integer_literal_bounds_cover_expected_suffix_and_signed_minimum() {
    let tree = parse_ok(
        r"
flow numeric_bounds {
    let max_u128: u128 = 340282366920938463463374607431768211455
    let min_i8: i8 = -128
    let min_i128: i128 = -170141183460469231731687303715884105728
    let too_large: u8 = 256
    let too_negative: i8 = -129
    let unsigned_negative = -1u8
}
",
    );
    let hir = lower_to_hir(&tree).expect("numeric bounds fixture lowers");
    validate_typecheck_ready(&hir).expect("numeric bounds fixture is structured");
    let report = analyze_types(&hir, &TypeCheckEnv::new());

    let range_errors = report
        .diagnostics
        .iter()
        .filter(|error| {
            matches!(
                error.kind(),
                TypeCheckErrorKind::IntegerLiteralOutOfRange { .. }
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        range_errors.len(),
        2,
        "diagnostics: {:?}",
        report.diagnostics
    );
    assert!(
        range_errors
            .iter()
            .any(|error| error.message().contains("256"))
    );
    assert!(
        range_errors
            .iter()
            .any(|error| error.message().contains("129"))
    );
    assert!(report.diagnostics.iter().any(|error| {
        error
            .message()
            .contains("negation operand must be a signed numeric type or Duration")
    }));
}

#[test]
fn float_literal_bounds_follow_the_resolved_ieee_width() {
    let tree = parse_ok(
        r"
flow float_bounds {
    let max_f32: f32 = 3.4028235e38
    let f32_overflow: f32 = 3.5e38
    let f64_overflow: f64 = 1.8e308
}
",
    );
    let hir = lower_to_hir(&tree).expect("float bounds fixture lowers");
    let report = analyze_types(&hir, &TypeCheckEnv::new());
    let overflow = report
        .diagnostics
        .iter()
        .filter(|error| {
            matches!(
                error.kind(),
                TypeCheckErrorKind::FloatLiteralOutOfRange { .. }
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(overflow.len(), 2, "{:?}", report.diagnostics);
    assert!(overflow.iter().any(|error| error.message().contains("f32")));
    assert!(overflow.iter().any(|error| error.message().contains("f64")));
}

#[test]
fn numeric_resolution_exports_exact_lowering_and_fallback_evidence() {
    let tree = parse_ok(
        r"
flow numeric_evidence {
    let wide: u128 = 340282366920938463463374607431768211455
    let precise: f32 = 1_2.5_0
    let values: Vec<u64> = [4294967296, 4294967297]
    let sum: u128 = 1 + 2
    let scaled: f32 = 1.0 * 2.0
    let fallback_int = 7
    let fallback_float = 0.5
}
",
    );
    let hir = lower_to_hir(&tree).expect("numeric evidence fixture lowers");
    validate_typecheck_ready(&hir).expect("numeric evidence fixture is structured");
    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);

    let targets = report
        .typed_lowering_evidence
        .iter()
        .filter_map(|evidence| match &evidence.kind {
            TypedLoweringEvidenceKind::ResolvedNumericType { target } => Some(target),
            _ => None,
        })
        .collect::<Vec<_>>();
    for expected in [
        &TypeKind::U128,
        &TypeKind::F32,
        &TypeKind::U64,
        &TypeKind::I32,
        &TypeKind::F64,
    ] {
        assert!(
            targets.contains(&expected),
            "missing {expected:?} in {targets:?}"
        );
    }
    assert!(report.numeric_fallbacks.iter().any(|fallback| {
        fallback.kind == NumericFallbackKind::IntegerLiteral
            && fallback.fallback == TypeKind::I32
            && !fallback.inferred_contract
    }));
    assert!(report.numeric_fallbacks.iter().any(|fallback| {
        fallback.kind == NumericFallbackKind::FloatLiteral
            && fallback.fallback == TypeKind::F64
            && !fallback.inferred_contract
    }));
    assert_eq!(
        report.numeric_fallbacks.len(),
        2,
        "expected types must reach arithmetic operands: {:?}",
        report.numeric_fallbacks
    );
}

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
fn typechecks_view_mount_builtin() {
    let tree = parse_ok(
        r#"
view Panel() {
  TextField(@input:.name)
    .label("Name")
}

flow main {
  view(@view:.Panel)
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("view mount fixture lowers");
    validate_hir_references(&hir, &registry_from_hir(&hir)).expect("view mount references resolve");
    typecheck_hir(&hir, &TypeCheckEnv::standard()).expect("view mount builtin typechecks");
}

#[test]
fn typechecks_view_handle_release() {
    let tree = parse_ok(
        r#"
view Panel() {
  Text("Ready")
}

flow main {
  let panel = view(@view:.Panel)
  panel.release()
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("view handle fixture lowers");
    validate_hir_references(&hir, &registry_from_hir(&hir))
        .expect("view handle references resolve");
    typecheck_hir(&hir, &TypeCheckEnv::standard()).expect("view handle release typechecks");
}

#[test]
fn rejects_unknown_view_handle_method() {
    let tree = parse_ok(
        r#"
view Panel() {
  Text("Ready")
}

flow main {
  let panel = view(@view:.Panel)
  panel.mystery()
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("view handle fixture lowers");
    validate_hir_references(&hir, &registry_from_hir(&hir))
        .expect("view handle references resolve");
    let errors = typecheck_hir(&hir, &TypeCheckEnv::standard())
        .expect_err("unknown ViewHandle method rejects");
    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("unknown method `mystery`")),
        "{errors:#?}"
    );
}

#[test]
fn typechecks_overlay_handle_pop() {
    let tree = parse_ok(
        r#"
view MenuOverlay() {
  Panel {
    Text("Menu")
  }
}

flow main {
  let overlay_handle = overlay(@view:.MenuOverlay)
  overlay_handle.pop()
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("overlay handle fixture lowers");
    validate_hir_references(&hir, &registry_from_hir(&hir))
        .expect("overlay handle references resolve");
    typecheck_hir(&hir, &TypeCheckEnv::standard()).expect("overlay pop typechecks");
}

#[test]
fn typechecks_receive_action_event_value_projection() {
    let tree = parse_ok(
        r"
pub action feedback.submit(value: String)

flow action_wait {
  let event = receive action(@action:.feedback.submit)
  let value: String = event.value
  return value
}
",
    );
    let hir = lower_to_hir(&tree).expect("receive action fixture lowers");
    validate_hir_references(&hir, &registry_from_hir(&hir))
        .expect("receive action target resolves");
    validate_typecheck_ready(&hir).expect("receive action fixture is typecheck-ready");
    typecheck_hir(&hir, &TypeCheckEnv::standard())
        .expect("receive action event value projects as String");
}

#[test]
fn typechecks_view_action_invoke_payload_signature() {
    let tree = parse_ok(
        r#"
pub action feedback.submit(value: String)
pub action feedback.label(name: String)

view FeedbackForm() {
  Button("Continue")
    .on_click {
      action.invoke(@action:.feedback.submit, value = "ready")
    }
  Button("Label")
    .on_click {
      action.invoke(@action:.feedback.label, name = "Ada")
    }
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("view action fixture lowers");
    validate_typecheck_ready(&hir).expect("view action fixture is typecheck-ready");
    typecheck_hir(&hir, &TypeCheckEnv::standard())
        .expect("view action payload matches declaration signature");
}

#[test]
fn typechecks_view_action_invoke_without_payload() {
    let tree = parse_ok(
        r#"
pub action settings.close

view SettingsPanel() {
  Button("Close")
    .on_click {
      action.invoke(@action:.settings.close)
    }
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("view action fixture lowers");
    validate_typecheck_ready(&hir).expect("view action fixture is typecheck-ready");
    typecheck_hir(&hir, &TypeCheckEnv::standard())
        .expect("view action without payload matches declaration signature");
}

#[test]
fn typechecks_generic_view_callback_action_invoke_payload_signature() {
    let tree = parse_ok(
        r#"
pub action feedback.focus(value: String)

view FeedbackForm() {
  Button("Continue")
    .on_focus {
      action.invoke(@action:.feedback.focus, value = "focused")
    }
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("generic callback action fixture lowers");
    validate_typecheck_ready(&hir).expect("generic callback action fixture is typecheck-ready");
    typecheck_hir(&hir, &TypeCheckEnv::standard())
        .expect("generic callback action payload matches declaration signature");
}

#[test]
fn rejects_generic_view_callback_action_invoke_payload_signature() {
    let tree = parse_ok(
        r#"
pub action feedback.focus(value: String)

view FeedbackForm() {
  Button("Continue")
    .on_focus {
      action.invoke(@action:.feedback.focus, label = "focused")
    }
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("generic callback action fixture lowers");
    validate_typecheck_ready(&hir).expect("generic callback action fixture is typecheck-ready");
    let errors = typecheck_hir(&hir, &TypeCheckEnv::standard())
        .expect_err("generic callback action payload name mismatch is rejected");
    assert!(errors.iter().any(|error| {
        error
            .message()
            .contains("action `action.feedback.focus` does not declare payload `label`")
    }));
}

#[test]
fn for_iteration_evidence_is_trait_resolved_for_runtime_flows() {
    let tree = parse_ok(
        r"
fn helper() -> Unit {
    for n in 0i32..1i32 {
        let _ = n
    }
}

flow @flow.iter iter {
    for n in 0i32..2i32 {
        let _ = n
    }
    for c in [1i32, 2i32] {
        let _ = c
    }
}
",
    );
    let hir = lower_to_hir(&tree).expect("iterator evidence fixture lowers");
    let report = analyze_types(&hir, &TypeCheckEnv::new());

    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    let families = report
        .for_iteration_evidence
        .iter()
        .map(|evidence| evidence.family.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        families,
        vec![
            ForIterationEvidenceFamily::Builtin(StandardIteratorFamily::Range),
            ForIterationEvidenceFamily::Builtin(StandardIteratorFamily::Vec),
        ]
    );
    assert!(
        report
            .for_iteration_evidence
            .iter()
            .all(|evidence| evidence.item_ty == TypeKind::I32)
    );
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
fn typechecks_data_codec_builtins_with_format_enum() {
    let tree = parse_ok(
        r#"
flow @flow.opening opening {
    let payload = ["hello"]
    let bytes: Bytes = data.encode(payload, .Json)
    let decoded: AgentValue = data.decode(bytes, .Json)
    let shape: DataShape = data.shape(decoded)
    let shaped: AgentValue = data.decode(bytes, .Json, shape)
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("data codec fixture lowers");
    validate_typecheck_ready(&hir).expect("data codec fixture is typecheck-ready");
    typecheck_hir(&hir, &TypeCheckEnv::standard()).expect("data codec builtins typecheck");
}

#[test]
fn typechecks_every_authoritative_data_format_variant() {
    for format in DataFormat::ALL {
        let variant = format.variant_name();
        let source = format!(
            r#"
flow formats {{
    let short: Bytes = data.encode(["hello"], .{variant})
    let qualified: Bytes = data.encode(["hello"], DataFormat.{variant})
}}
"#
        );
        let tree = parse_ok(&source);
        let hir = lower_to_hir(&tree)
            .unwrap_or_else(|errors| panic!("DataFormat.{variant} lowers: {errors:?}"));
        validate_typecheck_ready(&hir)
            .unwrap_or_else(|errors| panic!("DataFormat.{variant} is typecheck-ready: {errors:?}"));
        typecheck_hir(&hir, &TypeCheckEnv::standard())
            .unwrap_or_else(|errors| panic!("DataFormat.{variant} typechecks: {errors:?}"));
    }
}

#[test]
fn typechecks_content_availability_builtins() {
    let tree = parse_ok(
        r"
content chapter_two {
    roots = [
        @flow.chapter_two,
    ]
}

flow menu_open
effects { content.load, content.release, control.suspend }
{
    content.prefetch(@content.chapter_two)
    let _ = await content.ensure(@content.chapter_two)
    content.release(@content.chapter_two)
}

flow chapter_two {}
",
    );
    let hir = lower_to_hir(&tree).expect("content availability fixture lowers");
    validate_typecheck_ready(&hir).expect("content availability fixture is typecheck-ready");
    typecheck_hir(&hir, &TypeCheckEnv::standard())
        .expect("content availability builtins typecheck");

    let bad = parse_ok(
        r"
asset chapter_two {
}

flow bad_content_ref {
    content.prefetch(@asset:.chapter_two)
}
",
    );
    let bad_hir = lower_to_hir(&bad).expect("bad content ref fixture lowers");
    let errors = typecheck_hir(&bad_hir, &TypeCheckEnv::standard())
        .expect_err("content builtins reject non-content ids");
    assert!(errors.iter().any(|error| matches!(
        error.kind(),
        TypeCheckErrorKind::ArgumentTypeMismatch {
            function,
            argument,
            expected,
            actual,
        } if function == "content.prefetch"
            && argument == "unit"
            && expected == &TypeKind::entity_ref(EntityKind::Content)
            && actual == &TypeKind::entity_ref(EntityKind::Asset)
    )));
}

#[test]
fn unregistered_generic_type_names_remain_open_nominal_types() {
    let tree = parse_ok(
        r"
fn inspect_collection(route: ProjectCollection<Asset>) {
    let _ = route
}
",
    );
    let hir = lower_to_hir(&tree).expect("open nominal fixture lowers");
    validate_typecheck_ready(&hir).expect("open nominal fixture is typecheck-ready");

    typecheck_hir(&hir, &TypeCheckEnv::new())
        .expect("unregistered generic names have no special source-level meaning");
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
fn explicit_empty_effect_bound_rejects_adapter_effect() {
    let tree = parse_ok(
        r#"
flow @flow.opening opening
effects { }
{
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

    let errors = typecheck_hir(&hir, &env).expect_err("excess adapter effect is rejected");
    assert!(errors.iter().any(|error| {
        matches!(
            error.kind(),
            TypeCheckErrorKind::Effect { diagnostic }
                if diagnostic.code()
                    == crate::effect_diagnostics::EffectDiagnosticCode::UpperBoundExceeded
                    && diagnostic.message().contains("flow.opening")
                    && diagnostic.message().contains("fs.read")
        )
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
fn effect_closure_reaches_callers_through_user_helpers() {
    let tree = parse_ok(
        r#"
fn load_profile() -> String
effects { fs.read }
{
    adapter.read_text(path = "profile.json")
}

flow @flow.opening opening
effects { }
{
    let profile = load_profile()
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("transitive effect fixture lowers");
    let env = TypeCheckEnv::new()
        .with_function_signature(
            "adapter.read_text",
            FunctionSignature::new(
                TypeKind::String,
                [FunctionParam::required("path", TypeKind::String)],
            ),
        )
        .with_function_effects("adapter.read_text", ["fs.read".to_owned()]);

    let errors = typecheck_hir(&hir, &env).expect_err("caller upper-bound excess is rejected");
    assert!(errors.iter().any(|error| {
        matches!(error.kind(), TypeCheckErrorKind::Effect { .. })
            && error.message().contains("exceeding explicit upper bound")
            && error.message().contains("fs.read")
    }));
}

#[test]
fn environment_capability_does_not_replace_source_effect_declaration() {
    let tree = parse_ok(
        r#"
flow @flow.opening opening
effects { }
{
    let body = adapter.read_text(path = "story.arcw")
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("availability separation fixture lowers");
    let env = TypeCheckEnv::new()
        .with_function_signature(
            "adapter.read_text",
            FunctionSignature::new(
                TypeKind::String,
                [FunctionParam::required("path", TypeKind::String)],
            ),
        )
        .with_function_effects("adapter.read_text", ["fs.read".to_owned()])
        .with_capability("fs.read");

    let errors =
        typecheck_hir(&hir, &env).expect_err("host availability must not change source bound");
    assert!(errors.iter().any(|error| {
        matches!(error.kind(), TypeCheckErrorKind::Effect { .. })
            && error.message().contains("exceeding explicit upper bound")
            && error.message().contains("fs.read")
    }));
}

#[test]
fn omitted_entry_target_flow_effects_are_inferred_without_source_upper_bound() {
    let tree = parse_ok(
        r#"
extern capability cli { fn stdout(text: String) effects { stdio.write } }
entry cli @entry.main { goto @flow.main }
flow @flow.main main {
    cli.stdout("missing effects declaration")
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("entry effect fixture lowers");

    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.is_empty(),
        "omitted effects should infer without imposing a bound: {:?}",
        report.diagnostics
    );
    let summary = report
        .effects
        .summary(&crate::effect_model::CallableId::new("flow.main"))
        .expect("entry target flow has an effect summary");
    assert!(summary.declared().is_none());
    assert!(
        summary
            .inferred()
            .contains(&crate::effects::EffectId::parse("stdio.write").expect("valid effect"))
    );
}

#[test]
fn target_effect_availability_is_separate_from_checker_capabilities() {
    let tree = parse_ok(
        r#"
flow @flow.opening opening
effects { fs.read }
{
    let body = adapter.read_text(path = "story.arcw")
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("availability fixture lowers");
    let env = TypeCheckEnv::new()
        .with_function_signature(
            "adapter.read_text",
            FunctionSignature::new(
                TypeKind::String,
                [FunctionParam::required("path", TypeKind::String)],
            ),
        )
        .with_function_effects("adapter.read_text", ["fs.read".to_owned()])
        .with_available_effects(["fs.write"]);

    let errors = typecheck_hir(&hir, &env).expect_err("unavailable target effect is rejected");
    assert!(errors.iter().any(|error| {
        matches!(
            error.kind(),
            TypeCheckErrorKind::Effect { diagnostic }
                if diagnostic.code()
                    == crate::effect_diagnostics::EffectDiagnosticCode::CapabilityUnavailable
                    && diagnostic.message().contains("fs.read")
        )
    }));
}

#[test]
fn target_effect_availability_covers_same_path_scoped_effects_only() {
    let read_tree = parse_ok(
        r#"
flow @flow.opening opening
effects { fs.read(save) }
{
    let body = adapter.read_text(path = "story.arcw")
}
"#,
    );
    let read_hir = lower_to_hir(&read_tree).expect("scoped availability fixture lowers");
    let read_env = TypeCheckEnv::new()
        .with_function_signature(
            "adapter.read_text",
            FunctionSignature::new(
                TypeKind::String,
                [FunctionParam::required("path", TypeKind::String)],
            ),
        )
        .with_function_effects("adapter.read_text", ["fs.read(save)".to_owned()])
        .with_available_effects(["fs.read(save)"]);

    typecheck_hir(&read_hir, &read_env)
        .expect("same-path scoped availability covers unscoped inferred effect");

    let asset_env = TypeCheckEnv::new()
        .with_function_signature(
            "adapter.read_text",
            FunctionSignature::new(
                TypeKind::String,
                [FunctionParam::required("path", TypeKind::String)],
            ),
        )
        .with_function_effects("adapter.read_text", ["fs.read(save)".to_owned()])
        .with_available_effects(["fs.read(asset)"]);

    let errors = typecheck_hir(&read_hir, &asset_env)
        .expect_err("different scoped availability does not cover save reads");
    assert!(errors.iter().any(|error| {
        matches!(
            error.kind(),
            TypeCheckErrorKind::Effect { diagnostic }
                if diagnostic.code()
                    == crate::effect_diagnostics::EffectDiagnosticCode::CapabilityUnavailable
                    && diagnostic.message().contains("fs.read")
        )
    }));
}

#[test]
fn no_effect_rejects_transitive_helper_effect() {
    let tree = parse_ok(
        r#"
fn load_profile() -> String
effects { fs.read }
{
    adapter.read_text(path = "profile.json")
}

flow @flow.opening opening
effects { fs.read }
ensures no_effect fs.read
{
    let profile = load_profile()
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("no_effect fixture lowers");
    let env = TypeCheckEnv::new()
        .with_function_signature(
            "adapter.read_text",
            FunctionSignature::new(
                TypeKind::String,
                [FunctionParam::required("path", TypeKind::String)],
            ),
        )
        .with_function_effects("adapter.read_text", ["fs.read".to_owned()]);

    let errors = typecheck_hir(&hir, &env).expect_err("forbidden transitive effect is rejected");
    assert!(errors.iter().any(|error| {
        matches!(error.kind(), TypeCheckErrorKind::Effect { .. })
            && error.message().contains("forbids effect `fs.read`")
    }));
}

#[test]
fn unused_explicit_upper_bound_is_not_reported_as_warning() {
    let tree = parse_ok(
        r#"
flow @flow.opening opening
effects { fs.read }
{
    return "ok"
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("overdeclared effect fixture lowers");
    let report = analyze_types(&hir, &TypeCheckEnv::new());

    assert!(
        report.diagnostics.is_empty(),
        "unused upper bound should not fail: {:?}",
        report.diagnostics
    );
    assert!(
        report.warnings.is_empty(),
        "unused upper bound should not warn: {:?}",
        report.warnings
    );
    let summary = report
        .effects
        .summary(&crate::effect_model::CallableId::new("flow.opening"))
        .expect("flow effect summary");
    assert!(summary.inferred().is_empty());
}

#[test]
fn typecheck_report_counts_type_and_borrow_work() {
    let tree = parse_ok(
        r#"
flow @flow.borrow_stats borrow_stats {
    let pixels: &'asset [Rgba8] = pixels()
    let alias = pixels
    drop(pixels)
    return "done"
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("borrow stats fixture lowers");
    validate_typecheck_ready(&hir).expect("borrow stats fixture is typecheck-ready");
    let report = analyze_types(
        &hir,
        &TypeCheckEnv::new().with_function("pixels", pixel_borrow_ty()),
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
fn expected_type_reaches_nested_value_expressions() {
    let tree = parse_ok(
        r"
flow @flow.expected_values expected_values {
    let block_value: i64 = { 1 }
    let if_value: i64 = if cond {
        1
    } else {
        2
    }
    let if_let_value: i64 = if let .Some(value) = maybe {
        value
    } else {
        2
    }
    let match_value: i64 = match cond {
        true => 1
        false => 2
    }
}
",
    );
    let hir = lower_to_hir(&tree).expect("expected propagation fixture lowers");
    typecheck_hir(
        &hir,
        &TypeCheckEnv::new()
            .with_symbol("cond", TypeKind::Bool)
            .with_symbol("maybe", TypeKind::Option(Box::new(TypeKind::I64))),
    )
    .expect("expected types reach block, if-let, and match branches");
}

#[test]
fn expected_type_resolves_user_enum_short_variant() {
    let tree = parse_ok(
        r#"
enum Mood {
    Calm,
    Alert,
    WithScore(i64),
    WithMeta { label: String },
}

fn echo_mood(mood: Mood) -> Mood {
    return mood
}

flow @flow.enum_shorthand enum_shorthand {
    let mood: Mood = .Alert
    let echoed: Mood = echo_mood(.Calm)
    let nested: Mood = { .Alert }
    let scored: Mood = .WithScore(7i64)
    let meta: Mood = WithMeta { label = "ready" }
    let _ = (mood, echoed, nested, scored, meta)
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("user enum shorthand fixture lowers");
    validate_typecheck_ready(&hir).expect("user enum shorthand fixture is typecheck-ready");
    typecheck_hir(&hir, &TypeCheckEnv::new())
        .expect("expected type resolves user enum short variants");
}

#[test]
fn result_constructors_use_expected_payload_types() {
    let tree = parse_ok(
        r"
fn result_constructors(cond: bool) -> Result<i64, i64> {
    let ok: Result<i64, i64> = Ok(1)
    let err: Result<i64, i64> = Err(2)
    if cond {
        return ok
    }
    return err
}
",
    );
    let hir = lower_to_hir(&tree).expect("result constructor fixture lowers");
    typecheck_hir(&hir, &TypeCheckEnv::new())
        .expect("Ok and Err payloads use expected Result view types");
}

#[test]
fn option_try_requires_option_return_context() {
    let tree = parse_ok(
        r"
fn bad_option_try(maybe: Option<i64>) -> Result<i64, i64> {
    let value: i64 = maybe?
    return Ok(value)
}
",
    );
    let hir = lower_to_hir(&tree).expect("bad option try fixture lowers");
    let errors = typecheck_hir(&hir, &TypeCheckEnv::new())
        .expect_err("Option ? outside Option return is rejected");
    assert!(errors.iter().any(|error| {
        error
            .message()
            .contains("`?` on Option<T> requires an enclosing Option return")
    }));
}

#[test]
fn option_try_returns_inner_type_in_option_context() {
    let tree = parse_ok(
        r"
fn option_try(maybe: Option<i64>) -> Option<i64> {
    let value: i64 = maybe?
    return Some(value)
}
",
    );
    let hir = lower_to_hir(&tree).expect("option try fixture lowers");
    typecheck_hir(&hir, &TypeCheckEnv::new()).expect("Option ? typechecks in Option return");
}

#[test]
fn numeric_bracket_sequence_uses_expected_choice_item_type() {
    let tree = parse_ok(
        r"
fn numeric_choice_items() -> Unit {
    let values: Vec<String | i64> = [1, 2, 3]
}
",
    );
    let hir = lower_to_hir(&tree).expect("numeric choice sequence fixture lowers");
    typecheck_hir(&hir, &TypeCheckEnv::new())
        .expect("integer sequence uses the unique numeric expected item alternative");
}

#[test]
fn numeric_bracket_sequence_rejects_ambiguous_choice_item_type() {
    let tree = parse_ok(
        r"
fn ambiguous_numeric_choice_items() -> Unit {
    let values: Vec<i64 | u64> = [1, 2, 3]
}
",
    );
    let hir = lower_to_hir(&tree).expect("ambiguous numeric choice fixture lowers");
    let errors = typecheck_hir(&hir, &TypeCheckEnv::new())
        .expect_err("ambiguous numeric choice item is rejected");
    assert!(errors.iter().any(|error| {
        error
            .message()
            .contains("unsuffixed integer sequence literal requires an expected integer item type")
    }));
}

#[test]
fn comparison_operators_require_compatible_operands() {
    let tree = parse_ok(
        r#"
fn bad_comparisons() -> Unit {
    let bad_eq = "score" == 1i64
    let bad_order = "score" < 1i64
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("bad comparison fixture lowers");
    let errors = typecheck_hir(&hir, &TypeCheckEnv::new())
        .expect_err("incompatible comparison operands are rejected");
    assert!(errors.iter().any(|error| {
        error
            .message()
            .contains("equality operands must be compatible")
    }));
    assert!(errors.iter().any(|error| {
        error
            .message()
            .contains("ordering operands must have the same ordered scalar type")
    }));
}

#[test]
fn borrow_branch_merge_records_delta_without_full_clone() {
    let tree = parse_ok(
        r#"
flow @flow.borrow_branch_delta borrow_branch_delta {
    let pixels: &'asset [Rgba8] = pixels()
    if ready {
        drop(pixels)
    }
    drop(pixels)
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
            .with_function("pixels", pixel_borrow_ty()),
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
    assert_eq!(TypeKind::primitive_name("Unit"), Some(TypeKind::Unit));
    assert_eq!(TypeKind::primitive_name("Never"), Some(TypeKind::Never));
    assert_eq!(TypeKind::Unit.source_label(), "Unit");
    assert_eq!(TypeKind::Never.source_label(), "Never");
    assert_eq!(
        crate::checker::helpers::type_ref_kind(&parse_type_ref("!").expect("! parses")),
        TypeKind::Never
    );
    assert_eq!(
        crate::checker::helpers::type_ref_kind(&parse_type_ref("Never").expect("Never parses")),
        TypeKind::Never
    );
    assert_eq!(
        TypeKind::function([], TypeKind::Unit).source_label(),
        "() -> Unit"
    );
    assert_eq!(
        TypeKind::function(
            [TypeKind::I64],
            TypeKind::function([TypeKind::String], TypeKind::Bool),
        )
        .source_label(),
        "i64 -> String -> bool"
    );
    assert_eq!(
        TypeKind::function([TypeKind::I64, TypeKind::String], TypeKind::Bool).source_label(),
        "(i64, String) -> bool"
    );
    assert_eq!(TypeKind::primitive_name("()"), None);
    assert_eq!(TypeKind::primitive_name("!"), None);
    assert_eq!(TypeKind::primitive_name("Bool"), None);
    assert_eq!(TypeKind::primitive_name("Char"), None);
    assert_eq!(TypeKind::primitive_name("int"), None);
    assert_eq!(TypeKind::primitive_name("uint"), None);
    assert_eq!(TypeKind::primitive_name("float"), None);
    assert_eq!(TypeKind::primitive_name("Number"), None);
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
fn unsuffixed_numeric_literals_default_to_stable_widths() {
    let tree = parse_ok(
        r"
flow @flow.good good {
    let n = 1
    let f = 1.0
}
",
    );
    let hir = lower_to_hir(&tree).expect("unsuffixed literal fixture lowers");
    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
    assert!(report.judgments.iter().any(|judgment| {
        matches!(
            (&judgment.subject, judgment.rule, &judgment.ty),
            (
                TypeJudgmentSubject::Expr { kind, .. },
                TypeJudgmentRule::Expr,
                TypeKind::I32
            )
                if *kind == "literal"
        )
    }));
    assert!(report.judgments.iter().any(|judgment| {
        matches!(
            (&judgment.subject, judgment.rule, &judgment.ty),
            (
                TypeJudgmentSubject::Expr { kind, .. },
                TypeJudgmentRule::Expr,
                TypeKind::F64
            )
                if *kind == "literal"
        )
    }));
}

#[test]
fn let_rhs_type_judgments_carry_source_ranges() {
    let source = r"
flow @flow.source_ranges source_ranges {
    let total = 1i32 + 2i32
}
";
    let tree = parse_ok(source);
    let hir = lower_to_hir(&tree).expect("source range fixture lowers");
    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );

    let judgment = report
        .judgments
        .iter()
        .find(|judgment| {
            matches!(
                (&judgment.subject, judgment.rule, &judgment.ty),
                (
                    TypeJudgmentSubject::Expr { kind, .. },
                    TypeJudgmentRule::Expr,
                    TypeKind::I32
                ) if *kind == "binary"
            )
        })
        .expect("binary let RHS should be judged");
    let range = judgment
        .source_range
        .expect("let RHS expression judgment should retain its source range");
    assert_eq!(&source[range.as_range()], "1i32 + 2i32");
    let binding_judgment = report
        .judgments
        .iter()
        .find(|judgment| {
            matches!(
                (&judgment.subject, judgment.rule, &judgment.ty),
                (
                    TypeJudgmentSubject::LetBinding { pattern },
                    TypeJudgmentRule::LetBinding,
                    TypeKind::I32
                ) if pattern == "Ident(\"total\")"
            )
        })
        .expect("let binding should be judged");
    let binding_range = binding_judgment
        .source_range
        .expect("let binding judgment should retain its RHS source range");
    assert_eq!(&source[binding_range.as_range()], "1i32 + 2i32");
    assert!(
        report.judgments.iter().any(|judgment| {
            matches!(
                (&judgment.subject, judgment.rule, &judgment.ty),
                (
                    TypeJudgmentSubject::Expr { kind, .. },
                    TypeJudgmentRule::Expr,
                    TypeKind::I32
                ) if *kind == "literal"
                    && judgment
                        .source_range
                        .is_some_and(|range| &source[range.as_range()] == "1i32")
            )
        }),
        "child literal judgments should carry their own source ranges"
    );
}

#[test]
fn function_like_body_value_judgments_carry_source_ranges() {
    let source = r"
fn add(lhs: i64, rhs: i64) -> i64 {
    lhs + rhs
}

impl i64 {
    fn plus(self, delta: i64) -> i64 {
        self + delta
    }
}
";
    let tree = parse_ok(source);
    let hir = lower_to_hir(&tree).expect("function-like body source range fixture lowers");
    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );

    assert_expr_source_judgment(
        &report,
        source,
        "binary",
        "lhs + rhs",
        |ty| matches!(ty, TypeKind::I64),
        "top-level function body value should retain its source range",
    );
    assert_expr_source_judgment(
        &report,
        source,
        "binary",
        "self + delta",
        |ty| matches!(ty, TypeKind::I64),
        "impl method body value should retain its source range",
    );
}

#[test]
fn nested_let_rhs_expression_judgments_carry_source_ranges() {
    let source = r"
fn add(lhs: i64, rhs: i64) -> i64 {
    lhs + rhs
}

flow @flow.nested_source_ranges nested_source_ranges {
    let base = 2i64
    let total = add(1i64, base + 3i64)
    let piped = base |> add(4i64)
}
";
    let tree = parse_ok(source);
    let hir = lower_to_hir(&tree).expect("nested source range fixture lowers");
    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
    assert!(
        report.judgments.iter().any(|judgment| {
            matches!(
                (&judgment.subject, &judgment.ty),
                (
                    TypeJudgmentSubject::Expr { kind, .. },
                    TypeKind::I64
                ) if *kind == "binary"
                    && judgment
                        .source_range
                        .is_some_and(|range| &source[range.as_range()] == "base + 3i64")
            )
        }),
        "nested call argument expression should carry its own range"
    );
    assert!(
        report.judgments.iter().any(|judgment| {
            matches!(
                (&judgment.subject, &judgment.ty),
                (
                    TypeJudgmentSubject::Expr { kind, .. },
                    TypeKind::Function {
                        params,
                        return_type,
                        ..
                    }
                ) if *kind == "call"
                    && params == &[TypeKind::I64]
                    && matches!(return_type.as_ref(), TypeKind::I64)
                    && judgment
                        .source_range
                        .is_some_and(|range| &source[range.as_range()] == "add(4i64)")
            )
        }),
        "data-last pipe RHS partial call should keep its authored RHS range"
    );
    assert!(
        report.judgments.iter().any(|judgment| {
            matches!(
                (&judgment.subject, &judgment.ty),
                (
                    TypeJudgmentSubject::Expr { kind, .. },
                    TypeKind::I64
                ) if *kind == "pipe"
                    && judgment
                        .source_range
                        .is_some_and(|range| &source[range.as_range()] == "base |> add(4i64)")
            )
        }),
        "pipe expression should retain the full authored pipe range"
    );
}

#[test]
fn numeric_bracket_sequence_judgments_carry_source_ranges() {
    let source = r"
flow @flow.numeric_source_ranges numeric_source_ranges {
    let values = [1, 2, 3]
}
";
    let tree = parse_ok(source);
    let hir = lower_to_hir(&tree).expect("numeric bracket source range fixture lowers");
    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
    assert_expr_source_judgment(
        &report,
        source,
        "numeric_bracket_seq",
        "[1, 2, 3]",
        |ty| matches!(ty, TypeKind::Vec(item) if item.as_ref() == &TypeKind::I32),
        "numeric bracket sequence root should retain its authored range",
    );
}

#[test]
fn thread_expression_body_judgments_carry_source_ranges() {
    let source = r"
flow @flow.thread_source_ranges thread_source_ranges {
    let score_task = thread compute_score { route_score(state) }
}
";
    let tree = parse_ok(source);
    let hir = lower_to_hir(&tree).expect("thread expression source range fixture lowers");
    validate_typecheck_ready(&hir).expect("thread expression source range fixture is structured");
    let env = TypeCheckEnv::new()
        .with_symbol("state", TypeKind::String)
        .with_function_signature(
            "route_score",
            FunctionSignature::new(
                TypeKind::I64,
                [FunctionParam::required("state", TypeKind::String)],
            ),
        );
    let report = analyze_types(&hir, &env);
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
    assert_expr_source_judgment(
        &report,
        source,
        "thread",
        "thread compute_score { route_score(state) }",
        |ty| matches!(ty, TypeKind::ThreadHandle(item) if item.as_ref() == &TypeKind::Unit),
        "thread expression root should retain its authored range",
    );
    assert_expr_source_judgment(
        &report,
        source,
        "call",
        "route_score(state)",
        |ty| matches!(ty, TypeKind::I64),
        "thread expression body call should retain its authored body range",
    );
}

#[test]
fn desugared_function_stack_expression_judgments_keep_authored_source_ranges() {
    let source = r#"
struct Choice {
    label: String,
    enabled: bool,
}

fn add(lhs: i64, rhs: i64) -> i64 {
    lhs + rhs
}

fn above(min: i64, value: i64) -> bool {
    value >= min
}

flow @flow.desugared_source_ranges desugared_source_ranges {
    let threshold = 80i64
    let values: Vec<i64> = [79i64, 81i64]
    let choice = Choice { label: "Start", enabled: true }
    let label = choice.label
    let high = values.filter(_ > threshold)
    let mapped = values.map(|value: i64| value + 1i64)
    let pipe_with_placeholder = threshold |> add(^, 11i64)
    let pipe_data_last = threshold |> add(22i64)
    let method_fallback = threshold.above(70i64)
}
"#;
    let tree = parse_ok(source);
    let hir = lower_to_hir(&tree).expect("desugared source range fixture lowers");
    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
    assert_expr_source_judgment(
        &report,
        source,
        "binary",
        "_ > threshold",
        |ty| matches!(ty, TypeKind::Bool | TypeKind::Function { .. }),
        "partial-placeholder abstraction body should keep the authored body range",
    );
    assert_expr_source_judgment(
        &report,
        source,
        "select",
        "choice.label",
        |ty| matches!(ty, TypeKind::String),
        "ordinary selector expressions should retain the full visible selector range",
    );
    assert_expr_source_judgment(
        &report,
        source,
        "path",
        "choice",
        |ty| matches!(ty, TypeKind::Named(name) if name == "Choice"),
        "selector target expressions should retain their authored receiver range",
    );
    assert_expr_source_judgment(
        &report,
        source,
        "binary",
        "value + 1i64",
        |ty| matches!(ty, TypeKind::I64),
        "closure body expression should keep its authored body range",
    );
    assert_expr_source_judgment(
        &report,
        source,
        "literal",
        "11i64",
        |ty| matches!(ty, TypeKind::I64),
        "pipe RHS `^` substitution should keep source ranges for authored RHS children",
    );
    assert_expr_source_judgment(
        &report,
        source,
        "literal",
        "22i64",
        |ty| matches!(ty, TypeKind::I64),
        "data-last pipe rewriting should keep source ranges for authored RHS children",
    );
    assert_expr_source_judgment(
        &report,
        source,
        "call",
        "threshold.above(70i64)",
        |ty| matches!(ty, TypeKind::Bool),
        "method-chain data-last fallback should retain the visible method-call range",
    );
    assert!(
        !report.judgments.iter().any(|judgment| {
            judgment
                .source_range
                .is_some_and(|range| &source[range.as_range()] == "^")
        }),
        "substituted pipe LHS must not pretend the `^` token is the authored LHS range"
    );
}

fn assert_expr_source_judgment(
    report: &TypeCheckReport,
    source: &str,
    kind: &str,
    source_snippet: &str,
    ty_matches: impl Fn(&TypeKind) -> bool,
    message: &str,
) {
    assert!(
        report.judgments.iter().any(|judgment| {
            matches!(
                &judgment.subject,
                TypeJudgmentSubject::Expr { kind: actual_kind, .. } if *actual_kind == kind
            ) && ty_matches(&judgment.ty)
                && judgment
                    .source_range
                    .is_some_and(|range| &source[range.as_range()] == source_snippet)
        }),
        "{message}; candidates: {:?}",
        report
            .judgments
            .iter()
            .filter_map(|judgment| {
                let TypeJudgmentSubject::Expr { kind, .. } = &judgment.subject else {
                    return None;
                };
                let source_range = judgment.source_range?;
                Some((
                    *kind,
                    judgment.ty.source_label(),
                    source[source_range.as_range()].to_owned(),
                ))
            })
            .collect::<Vec<_>>()
    );
}

fn assert_expression_source_stats_match_report(report: &TypeCheckReport) {
    let source_backed = report
        .judgments
        .iter()
        .filter(|judgment| {
            matches!(judgment.subject, TypeJudgmentSubject::Expr { .. })
                && judgment.source_range.is_some()
        })
        .count();
    let source_missing = report
        .judgments
        .iter()
        .filter(|judgment| {
            matches!(judgment.subject, TypeJudgmentSubject::Expr { .. })
                && judgment.source_range.is_none()
        })
        .count();
    assert_eq!(
        report.stats.source_backed_expr_judgments, source_backed,
        "source-backed expression judgment stats should match the report"
    );
    assert_eq!(
        report.stats.source_missing_expr_judgments, source_missing,
        "source-missing expression judgment stats should match the report"
    );
}

#[test]
fn assignment_statement_rhs_judgments_carry_source_ranges() {
    let source = r"
struct Counter {
    value: i64,
}

flow @flow.assignment_source_ranges assignment_source_ranges {
    let counter = Counter { value: 1i64 }
    counter.value = counter.value + 2i64
}
";
    let tree = parse_ok(source);
    let hir = lower_to_hir(&tree).expect("assignment source range fixture lowers");
    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
    assert_expr_source_judgment(
        &report,
        source,
        "binary",
        "counter.value + 2i64",
        |ty| matches!(ty, TypeKind::I64),
        "assignment RHS root should carry its full authored range",
    );
    assert_expr_source_judgment(
        &report,
        source,
        "literal",
        "2i64",
        |ty| matches!(ty, TypeKind::I64),
        "assignment RHS literal should carry its own authored range",
    );
}

#[test]
fn typed_branch_statement_judgments_carry_source_ranges() {
    let source = r"
fn branch_source_ranges(maybe: Option<i64>, ready: bool) -> i64 {
    let .Some(value) = maybe else {
        return 0i64
    }
    while let .Some(item) = maybe when item > 0i64 {
        break
    }
    match ready {
        true when ready && false => { let first = value + 1i64 }
        false => { let second = value + 2i64 }
    }
    return value
}
";
    let tree = parse_ok(source);
    let hir = lower_to_hir(&tree).expect("typed branch source range fixture lowers");
    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
    assert_expression_source_stats_match_report(&report);
    assert_expr_source_judgment(
        &report,
        source,
        "path",
        "maybe",
        |ty| matches!(ty, TypeKind::Option(item) if item.as_ref() == &TypeKind::I64),
        "let-else RHS should carry its authored range",
    );
    assert_expr_source_judgment(
        &report,
        source,
        "binary",
        "item > 0i64",
        |ty| matches!(ty, TypeKind::Bool),
        "statement while-let guard should carry its authored range",
    );
    assert_expr_source_judgment(
        &report,
        source,
        "binary",
        "ready && false",
        |ty| matches!(ty, TypeKind::Bool),
        "statement match arm guard should carry its authored range",
    );
    assert_expr_source_judgment(
        &report,
        source,
        "binary",
        "value + 2i64",
        |ty| matches!(ty, TypeKind::I64),
        "statement match arm body should carry its nested statement range",
    );
}

#[test]
fn lifetime_set_statement_value_judgments_carry_source_ranges() {
    let source = r"
flow @flow.lifetime_set_source_ranges lifetime_set_source_ranges {
    let score = 2i64
    'flow.flags.score <- score + 1i64
}
";
    let tree = parse_ok(source);
    let hir = lower_to_hir(&tree).expect("lifetime-set source range fixture lowers");
    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
    assert_expr_source_judgment(
        &report,
        source,
        "binary",
        "score + 1i64",
        |ty| matches!(ty, TypeKind::I64),
        "lifetime-set value root should carry its full authored range",
    );
    assert_expr_source_judgment(
        &report,
        source,
        "literal",
        "1i64",
        |ty| matches!(ty, TypeKind::I64),
        "lifetime-set value literal should carry its own authored range",
    );
}

#[test]
fn action_receive_and_defer_judgments_carry_source_ranges() {
    let source = r"
pub action feedback.submit(value: String)

flow @flow.action_defer_source_ranges action_defer_source_ranges {
    let event = receive action(@action:.feedback.submit)
    defer 3i64 + 4i64
}
";
    let tree = parse_ok(source);
    let hir = lower_to_hir(&tree).expect("action/defer source range fixture lowers");
    validate_hir_references(&hir, &registry_from_hir(&hir))
        .expect("action/defer source range references resolve");
    let report = analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
    assert_expr_source_judgment(
        &report,
        source,
        "entity_ref",
        "@action:.feedback.submit",
        |ty| ty.is_entity_ref_kind(&EntityKind::Action),
        "receive action target should carry its authored range",
    );
    assert_expr_source_judgment(
        &report,
        source,
        "binary",
        "3i64 + 4i64",
        |ty| matches!(ty, TypeKind::I64),
        "defer expression root should carry its authored range",
    );
    assert_expr_source_judgment(
        &report,
        source,
        "literal",
        "4i64",
        |ty| matches!(ty, TypeKind::I64),
        "defer expression child literal should carry its own range",
    );
}

#[test]
fn return_and_expression_statement_judgments_carry_source_ranges() {
    let source = r"
fn ret() -> i64 {
    3i64 + 4i64
    return 1i64 + 2i64
}
";
    let tree = parse_ok(source);
    let hir = lower_to_hir(&tree).expect("statement source range fixture lowers");
    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );

    let return_judgment = report
        .judgments
        .iter()
        .find(|judgment| {
            matches!(
                (&judgment.subject, judgment.rule, &judgment.ty),
                (
                    TypeJudgmentSubject::Return { context },
                    TypeJudgmentRule::Return,
                    TypeKind::I64
                ) if context == "tail block expression"
            )
        })
        .expect("return statement should be judged");
    let return_range = return_judgment
        .source_range
        .expect("return judgment should retain its expression source range");
    assert_eq!(&source[return_range.as_range()], "1i64 + 2i64");

    assert!(
        report.judgments.iter().any(|judgment| {
            matches!(
                (&judgment.subject, &judgment.ty),
                (
                    TypeJudgmentSubject::Expr { kind, .. },
                    TypeKind::I64
                ) if *kind == "binary"
                    && judgment
                        .source_range
                        .is_some_and(|range| &source[range.as_range()] == "3i64 + 4i64")
            )
        }),
        "expression statement root should carry its full authored range"
    );
    assert!(
        report.judgments.iter().any(|judgment| {
            matches!(
                (&judgment.subject, &judgment.ty),
                (
                    TypeJudgmentSubject::Expr { kind, .. },
                    TypeKind::I64
                ) if *kind == "literal"
                    && judgment
                        .source_range
                        .is_some_and(|range| &source[range.as_range()] == "4i64")
            )
        }),
        "expression statement child literal should carry its own range"
    );
}

#[test]
fn control_transfer_statement_judgments_carry_source_ranges() {
    let source = r"
flow @flow.control_source_ranges control_source_ranges {
    goto @flow.next
    close @flow.next
    let chosen = loop {
        break 8i64 + 9i64
    }
    let _line = alice.say()[Pick one.] with {
        select @choice.primary
        wait(0.35s)
    }
}

stream fn sample_stream(frames: Stream<i64, String>) -> Stream<i64, String> {
    yield 1i64 + 2i64
}
";
    let tree = parse_ok(source);
    let hir = lower_to_hir(&tree).expect("control-transfer source range fixture lowers");
    let report = analyze_types(
        &hir,
        &TypeCheckEnv::new().with_symbol("alice", TypeKind::entity_ref(EntityKind::Character)),
    );
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
    assert!(
        report.judgments.iter().any(|judgment| {
            matches!(
                (&judgment.subject, &judgment.ty),
                (
                    TypeJudgmentSubject::Expr { kind, .. },
                    ty
                ) if *kind == "entity_ref"
                    && ty.is_entity_ref_kind(&EntityKind::Flow)
                    && judgment
                        .source_range
                        .is_some_and(|range| &source[range.as_range()] == "@flow.next")
            )
        }),
        "goto destination should retain its authored range"
    );
    assert!(
        report.judgments.iter().any(|judgment| {
            matches!(
                (&judgment.subject, &judgment.ty),
                (
                    TypeJudgmentSubject::Expr { kind, .. },
                    ty
                ) if *kind == "entity_ref"
                    && ty.is_entity_ref_kind(&EntityKind::Choice)
                    && judgment
                        .source_range
                        .is_some_and(|range| &source[range.as_range()] == "@choice.primary")
            )
        }),
        "line-plan select target should retain its authored range"
    );
    assert_expr_source_judgment(
        &report,
        source,
        "binary",
        "8i64 + 9i64",
        |ty| matches!(ty, TypeKind::I64),
        "break value expression should retain its authored range",
    );
    assert_expr_source_judgment(
        &report,
        source,
        "literal",
        "0.35s",
        |ty| matches!(ty, TypeKind::Duration),
        "wait duration should retain its authored range inside wait(...)",
    );
    assert!(
        report.judgments.iter().any(|judgment| {
            matches!(
                (&judgment.subject, &judgment.ty),
                (
                    TypeJudgmentSubject::Expr { kind, .. },
                    TypeKind::I64
                ) if *kind == "binary"
                    && judgment
                        .source_range
                        .is_some_and(|range| &source[range.as_range()] == "1i64 + 2i64")
            )
        }),
        "yield expression should retain its authored range"
    );
}

#[test]
fn control_statement_expression_judgments_carry_source_ranges() {
    let source = r"
flow @flow.control_stmt_source_ranges control_stmt_source_ranges {
    let ready = true
    let keep_going = false
    let selected = true
    let values: Vec<i64> = [1i64, 2i64]
    if ready && true {
        let then_value = 1i64
    }
    while keep_going {
        break
    }
    for value in values {
        let copy = value
    }
    match selected {
        true => let truthy = 1i64
        false => let falsy = 0i64
    }
}
";
    let tree = parse_ok(source);
    let hir = lower_to_hir(&tree).expect("control statement source range fixture lowers");
    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
    assert!(
        report.judgments.iter().any(|judgment| {
            matches!(
                (&judgment.subject, &judgment.ty),
                (
                    TypeJudgmentSubject::Expr { kind, .. },
                    TypeKind::Bool
                ) if *kind == "binary"
                    && judgment
                        .source_range
                        .is_some_and(|range| &source[range.as_range()] == "ready && true")
            )
        }),
        "if condition root expression should carry its source range"
    );
    assert!(
        report.judgments.iter().any(|judgment| {
            matches!(
                (&judgment.subject, &judgment.ty),
                (
                    TypeJudgmentSubject::Expr { kind, .. },
                    TypeKind::Bool
                ) if *kind == "path"
                    && judgment
                        .source_range
                        .is_some_and(|range| &source[range.as_range()] == "keep_going")
            )
        }),
        "while condition expression should carry its source range"
    );
    assert!(
        report.judgments.iter().any(|judgment| {
            matches!(
                (&judgment.subject, &judgment.ty),
                (
                    TypeJudgmentSubject::Expr { kind, .. },
                    TypeKind::Vec(_)
                ) if *kind == "path"
                    && judgment
                        .source_range
                        .is_some_and(|range| &source[range.as_range()] == "values")
            )
        }),
        "for source expression should carry its source range"
    );
    assert!(
        report.judgments.iter().any(|judgment| {
            matches!(
                (&judgment.subject, &judgment.ty),
                (
                    TypeJudgmentSubject::Expr { kind, .. },
                    TypeKind::Bool
                ) if *kind == "path"
                    && judgment
                        .source_range
                        .is_some_and(|range| &source[range.as_range()] == "selected")
            )
        }),
        "match scrutinee expression should carry its source range"
    );
}

#[test]
fn dialogue_interpolation_judgments_carry_source_ranges() {
    let source = r"
flow @flow.dialogue_source_ranges dialogue_source_ranges {
    alice: Score #[score + 1i64] / $( player_name )[p]
}
";
    let tree = parse_ok(source);
    let hir = lower_to_hir(&tree).expect("dialogue source range fixture lowers");
    let env = TypeCheckEnv::new()
        .with_symbol("alice", TypeKind::entity_ref(EntityKind::Character))
        .with_symbol("score", TypeKind::I64)
        .with_symbol("player_name", TypeKind::String);
    let report = analyze_types(&hir, &env);
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );

    assert!(
        report.judgments.iter().any(|judgment| {
            matches!(
                (&judgment.subject, &judgment.ty),
                (
                    TypeJudgmentSubject::Expr { kind, .. },
                    TypeKind::I64
                ) if *kind == "binary"
                    && judgment
                        .source_range
                        .is_some_and(|range| &source[range.as_range()] == "score + 1i64")
            )
        }),
        "dialogue binary interpolation should retain its authored range"
    );
    assert!(
        report.judgments.iter().any(|judgment| {
            matches!(
                (&judgment.subject, &judgment.ty),
                (
                    TypeJudgmentSubject::Expr { kind, .. },
                    TypeKind::String
                ) if *kind == "path"
                    && judgment
                        .source_range
                        .is_some_and(|range| &source[range.as_range()] == "player_name")
            )
        }),
        "dialogue path interpolation should retain its trimmed authored range"
    );
}

#[test]
fn multiline_dialogue_interpolation_judgments_project_lf_and_crlf_ranges() {
    let source_lf = "flow @flow.dialogue_multiline_ranges dialogue_multiline_ranges {\n    alice: Score #[\n        score + 1i64\n    ][p]\n}\n";
    for source in [source_lf.to_owned(), source_lf.replace('\n', "\r\n")] {
        let tree = parse_ok(&source);
        let hir = lower_to_hir(&tree).expect("multiline dialogue range fixture lowers");
        let env = TypeCheckEnv::new()
            .with_symbol("alice", TypeKind::entity_ref(EntityKind::Character))
            .with_symbol("score", TypeKind::I64);
        let report = analyze_types(&hir, &env);
        assert!(
            report.diagnostics.is_empty(),
            "unexpected diagnostics: {:?}",
            report.diagnostics
        );
        assert!(
            report.judgments.iter().any(|judgment| {
                matches!(
                    (&judgment.subject, &judgment.ty),
                    (TypeJudgmentSubject::Expr { kind, .. }, TypeKind::I64)
                        if *kind == "binary"
                            && judgment.source_range.is_some_and(
                                |range| &source[range.as_range()] == "score + 1i64"
                            )
                )
            }),
            "later-line binary judgment should retain its authored range"
        );
    }
}

#[test]
fn dialogue_call_line_plan_expression_judgments_carry_source_ranges() {
    let source = r"
flow @flow.dialogue_call_plan_source_ranges dialogue_call_plan_source_ranges {
    let result = alice.say()[Pick one.] with { out score + 1i64 }
    let second = alice.say()[Choose again.]
    with:
        let cue = at(0.42s):
            score + 3i64
        out score + 2i64
}
";
    let tree = parse_ok(source);
    let hir = lower_to_hir(&tree).expect("dialogue call line-plan source range fixture lowers");
    let report = analyze_types(
        &hir,
        &TypeCheckEnv::new()
            .with_symbol("alice", TypeKind::entity_ref(EntityKind::Character))
            .with_symbol("score", TypeKind::I64),
    );
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
    assert_expr_source_judgment(
        &report,
        source,
        "binary",
        "score + 1i64",
        |ty| matches!(ty, TypeKind::I64),
        "dialogue call line-plan out expression should carry its authored range",
    );
    assert_expr_source_judgment(
        &report,
        source,
        "literal",
        "1i64",
        |ty| matches!(ty, TypeKind::I64),
        "dialogue call line-plan child literal should carry its own authored range",
    );
    assert_expr_source_judgment(
        &report,
        source,
        "binary",
        "score + 2i64",
        |ty| matches!(ty, TypeKind::I64),
        "following-line dialogue call line-plan out expression should carry its authored range",
    );
    assert_expr_source_judgment(
        &report,
        source,
        "literal",
        "2i64",
        |ty| matches!(ty, TypeKind::I64),
        "following-line dialogue call line-plan child literal should carry its own range",
    );
    assert_expr_source_judgment(
        &report,
        source,
        "binary",
        "score + 3i64",
        |ty| matches!(ty, TypeKind::I64),
        "line-plan named cue body root should carry its authored range",
    );
    assert_expr_source_judgment(
        &report,
        source,
        "path",
        "score",
        |ty| matches!(ty, TypeKind::I64),
        "line-plan named cue body path should carry its authored range",
    );
    assert_expr_source_judgment(
        &report,
        source,
        "literal",
        "3i64",
        |ty| matches!(ty, TypeKind::I64),
        "line-plan named cue body literal should carry its authored range",
    );
}

#[test]
fn container_and_control_expression_judgments_carry_source_ranges() {
    let source = r#"
struct Choice {
    label: String,
    enabled: bool,
}

flow @flow.container_source_ranges container_source_ranges {
    let ready = true
    let limit = 8i64
    let pair = (101i64, 202i64)
    let numbers: Vec<i64> = [303i64, 404i64]
    let repeated: Array<i64, 2> = [505i64; 2]
    let picked = numbers[1i64]
    let bounded = 1i64..=limit
    let choice = Choice { label: "Start", enabled: ready }
    let block_value = { 606i64 + 707i64 }
    let if_value = if ready {
        808i64
    } else {
        909i64
    }
    let if_let_value = if let .Some(value) = maybe when ready && true {
        value
    } else {
        0i64
    }
    let match_value = match ready {
        true => 1001i64
        false => 1002i64
    }
}
"#;
    let tree = parse_ok(source);
    let hir = lower_to_hir(&tree).expect("container/control source range fixture lowers");
    let report = analyze_types(
        &hir,
        &TypeCheckEnv::new().with_symbol("maybe", TypeKind::Option(Box::new(TypeKind::I64))),
    );
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
    assert_container_expression_ranges(&report, source);
    assert_control_expression_ranges(&report, source);
}

fn assert_container_expression_ranges(report: &TypeCheckReport, source: &str) {
    assert_expr_source_judgment(
        report,
        source,
        "tuple",
        "(101i64, 202i64)",
        |ty| matches!(ty, TypeKind::Tuple(items) if items == &[TypeKind::I64, TypeKind::I64]),
        "tuple root should carry its full authored range",
    );
    assert_expr_source_judgment(
        report,
        source,
        "numeric_bracket_seq",
        "[303i64, 404i64]",
        |ty| matches!(ty, TypeKind::Vec(item) if item.as_ref() == &TypeKind::I64),
        "bracket sequence root should carry its full authored range",
    );
    assert_expr_source_judgment(
        report,
        source,
        "array_repeat",
        "[505i64; 2]",
        |ty| matches!(ty, TypeKind::Array { item, len } if item.as_ref() == &TypeKind::I64 && len == "2"),
        "array repeat root should carry its full authored range",
    );
    assert_expr_source_judgment(
        report,
        source,
        "literal",
        "505i64",
        |ty| matches!(ty, TypeKind::I64),
        "array repeat value should carry its own authored range",
    );
    assert_expr_source_judgment(
        report,
        source,
        "index",
        "numbers[1i64]",
        |ty| matches!(ty, TypeKind::I64),
        "index root should carry its full authored range",
    );
    assert_expr_source_judgment(
        report,
        source,
        "range",
        "1i64..=limit",
        |ty| matches!(ty, TypeKind::Range(item) if item.as_ref() == &TypeKind::I64),
        "range root should carry its full authored range",
    );
    assert_expr_source_judgment(
        report,
        source,
        "record",
        r#"Choice { label: "Start", enabled: ready }"#,
        |ty| matches!(ty, TypeKind::Named(name) if name == "Choice"),
        "nominal record constructor should carry its full authored range",
    );
    assert_expr_source_judgment(
        report,
        source,
        "path",
        "ready",
        |ty| matches!(ty, TypeKind::Bool),
        "record field values should carry their authored ranges",
    );
    assert_expr_source_judgment(
        report,
        source,
        "block",
        "{ 606i64 + 707i64 }",
        |ty| matches!(ty, TypeKind::I64),
        "block expression root should carry its full authored range",
    );
    assert_expr_source_judgment(
        report,
        source,
        "binary",
        "606i64 + 707i64",
        |ty| matches!(ty, TypeKind::I64),
        "block final value should carry its authored range",
    );
}

fn assert_control_expression_ranges(report: &TypeCheckReport, source: &str) {
    assert_expr_source_judgment(
        report,
        source,
        "if",
        "if ready {\n        808i64\n    } else {\n        909i64\n    }",
        |ty| matches!(ty, TypeKind::I64),
        "if expression root should carry its full authored range",
    );
    assert_expr_source_judgment(
        report,
        source,
        "if_let",
        "if let .Some(value) = maybe when ready && true {\n        value\n    } else {\n        0i64\n    }",
        |ty| matches!(ty, TypeKind::I64),
        "if-let expression root should carry its full authored range",
    );
    assert_expr_source_judgment(
        report,
        source,
        "path",
        "maybe",
        |ty| matches!(ty, TypeKind::Option(item) if item.as_ref() == &TypeKind::I64),
        "if-let scrutinee should not absorb the guard source",
    );
    assert_expr_source_judgment(
        report,
        source,
        "binary",
        "ready && true",
        |ty| matches!(ty, TypeKind::Bool),
        "if-let guard should carry its own authored range",
    );
    assert!(
        !report.judgments.iter().any(|judgment| {
            judgment
                .source_range
                .is_some_and(|range| &source[range.as_range()] == "maybe when ready && true")
        }),
        "if-let scrutinee range must stop before `when` guard source"
    );
    assert_expr_source_judgment(
        report,
        source,
        "match",
        "match ready {\n        true => 1001i64\n        false => 1002i64\n    }",
        |ty| matches!(ty, TypeKind::I64),
        "match expression root should carry its full authored range",
    );
    assert_expr_source_judgment(
        report,
        source,
        "literal",
        "1002i64",
        |ty| matches!(ty, TypeKind::I64),
        "match arm values should carry their authored ranges",
    );
}

#[test]
fn container_child_expression_judgments_carry_source_ranges() {
    let source = r#"
flow @flow.container_child_source_ranges container_child_source_ranges {
    let numbers: Vec<i64> = [10i64, 20i64]
    let limit = 99i64
    let repeated: Array<i64, 12> = [505i64; 12]
    let picked = numbers[6i64]
    let bounded = 13i64..=limit
    let literal_record = accept_record({ title = "Loose", enabled = !false })
}
"#;
    let tree = parse_ok(source);
    let hir = lower_to_hir(&tree).expect("container child source range fixture lowers");
    let record_type = TypeKind::Named("Record".to_owned());
    let report = analyze_types(
        &hir,
        &TypeCheckEnv::new().with_function_signature(
            "accept_record",
            FunctionSignature::new(
                record_type.clone(),
                [FunctionParam::required("input", record_type)],
            ),
        ),
    );
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
    for (kind, snippet, message) in [
        ("literal", "12", "array repeat length"),
        ("literal", "6i64", "index expression"),
        ("literal", "13i64", "range start bound"),
        ("path", "limit", "range end bound"),
        ("literal", r#""Loose""#, "record literal string field"),
        ("unary", "!false", "record literal unary field"),
    ] {
        assert_expr_source_judgment(
            &report,
            source,
            kind,
            snippet,
            |_| true,
            &format!("{message} should carry its authored range"),
        );
    }
    assert_expr_source_judgment(
        &report,
        source,
        "record_literal",
        r#"{ title = "Loose", enabled = !false }"#,
        |ty| matches!(ty, TypeKind::Named(name) if name == "Record"),
        "anonymous record literal should carry its full authored range",
    );
}

#[test]
fn computation_and_braced_closure_judgments_carry_source_ranges() {
    let source = r"
flow @flow.block_value_source_ranges block_value_source_ranges {
    let from_result = result {
        141i64 + 142i64
    }
    let from_task = task {
        151i64 + 152i64
    }
    let make_value = || -> i64 {
        161i64 + 162i64
    }
    let from_closure = make_value()
}
";
    let tree = parse_ok(source);
    let hir = lower_to_hir(&tree).expect("block value source range fixture lowers");
    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
    assert_expr_source_judgment(
        &report,
        source,
        "computation_block",
        "result {\n        141i64 + 142i64\n    }",
        |ty| matches!(ty, TypeKind::I64),
        "result computation block root should retain its full authored range",
    );
    assert_expr_source_judgment(
        &report,
        source,
        "binary",
        "141i64 + 142i64",
        |ty| matches!(ty, TypeKind::I64),
        "result computation block value should retain its authored range",
    );
    assert_expr_source_judgment(
        &report,
        source,
        "binary",
        "151i64 + 152i64",
        |ty| matches!(ty, TypeKind::I64),
        "task computation block value should retain its authored range",
    );
    assert_expr_source_judgment(
        &report,
        source,
        "closure",
        "|| -> i64 {\n        161i64 + 162i64\n    }",
        |ty| {
            matches!(
                ty,
                TypeKind::Function { params, return_type, .. } if params.is_empty() && return_type.as_ref() == &TypeKind::I64
            )
        },
        "braced closure root should retain its full authored range",
    );
    assert_expr_source_judgment(
        &report,
        source,
        "binary",
        "161i64 + 162i64",
        |ty| matches!(ty, TypeKind::I64),
        "braced closure body value should retain its authored range",
    );
}

#[test]
fn memo_block_option_expression_judgments_carry_source_ranges() {
    let source = r"
flow @flow.memo_option_source_ranges memo_option_source_ranges {
    let cached = memo(scope=scene, key=score + 1i64) {
        score
    }
}
";
    let tree = parse_ok(source);
    let hir = lower_to_hir(&tree).expect("memo option source range fixture lowers");
    let report = analyze_types(
        &hir,
        &TypeCheckEnv::new()
            .with_symbol("scene", TypeKind::Named("MemoScope".to_owned()))
            .with_symbol("score", TypeKind::I64),
    );
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
    assert_expr_source_judgment(
        &report,
        source,
        "path",
        "scene",
        |ty| matches!(ty, TypeKind::Named(name) if name == "MemoScope"),
        "memo scope option expression should carry its authored range",
    );
    assert_expr_source_judgment(
        &report,
        source,
        "binary",
        "score + 1i64",
        |ty| matches!(ty, TypeKind::I64),
        "memo key option expression should carry its authored range",
    );
    assert_expr_source_judgment(
        &report,
        source,
        "literal",
        "1i64",
        |ty| matches!(ty, TypeKind::I64),
        "memo key child literal should carry its own authored range",
    );
}

#[test]
fn effect_and_prefix_expression_judgments_carry_source_ranges() {
    let source = r"
flow @flow.await_question_source_ranges await_question_source_ranges {
    let bg = await? load_bg()
}

fn option_source_ranges(maybe: Option<i64>, flag: bool) -> Option<i64> {
    let unwrapped = maybe?
    let prefix = try Some(unwrapped)
    let negated = -unwrapped
    let inverted = !flag
    return Some(prefix + negated)
}
";
    let tree = parse_ok(source);
    let hir = lower_to_hir(&tree).expect("effect/prefix source range fixture lowers");
    let env = TypeCheckEnv::new().with_function(
        "load_bg",
        TypeKind::Need {
            ready: Box::new(TypeKind::Named("Image".to_owned())),
            error: Box::new(TypeKind::Named("AssetError".to_owned())),
        },
    );
    let report = analyze_types(&hir, &env);
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );

    assert_expr_source_judgment(
        &report,
        source,
        "await",
        "await? load_bg()",
        |ty| matches!(ty, TypeKind::Named(name) if name == "Image"),
        "await? root should carry its full authored range",
    );
    assert_expr_source_judgment(
        &report,
        source,
        "call",
        "load_bg()",
        |ty| matches!(ty, TypeKind::Need { ready, .. } if ready.as_ref() == &TypeKind::Named("Image".to_owned())),
        "await? inner call should start after the question marker",
    );
    assert_expr_source_judgment(
        &report,
        source,
        "try",
        "maybe?",
        |ty| matches!(ty, TypeKind::I64),
        "postfix try root should carry the full question expression range",
    );
    assert_expr_source_judgment(
        &report,
        source,
        "path",
        "maybe",
        |ty| matches!(ty, TypeKind::Option(item) if item.as_ref() == &TypeKind::I64),
        "postfix try operand should keep its own source range",
    );
    assert_expr_source_judgment(
        &report,
        source,
        "try",
        "try Some(unwrapped)",
        |ty| matches!(ty, TypeKind::I64),
        "prefix try root should carry the full authored range",
    );
    assert_expr_source_judgment(
        &report,
        source,
        "call",
        "Some(unwrapped)",
        |ty| matches!(ty, TypeKind::Option(item) if item.as_ref() == &TypeKind::I64),
        "prefix try operand should keep its own source range",
    );
    assert_expr_source_judgment(
        &report,
        source,
        "unary",
        "-unwrapped",
        |ty| matches!(ty, TypeKind::I64),
        "numeric unary expression should carry its full authored range",
    );
    assert_expr_source_judgment(
        &report,
        source,
        "unary",
        "!flag",
        |ty| matches!(ty, TypeKind::Bool),
        "boolean unary expression should carry its full authored range",
    );
    assert!(!report.judgments.iter().any(|judgment| {
        judgment
            .source_range
            .is_some_and(|range| &source[range.as_range()] == "? load_bg()")
    }));
}

#[test]
fn range_expression_infers_item_type_from_bound() {
    let tree = parse_ok(
        r"
flow @flow.range range {
    let a = 2
    for i in 0..a {
        let j = i
    }
}
",
    );
    let hir = lower_to_hir(&tree).expect("range fixture lowers");
    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
    assert!(report.judgments.iter().any(|judgment| {
        matches!(
            (&judgment.subject, &judgment.ty),
            (TypeJudgmentSubject::Expr { kind, .. }, TypeKind::Range(item))
                if *kind == "range" && item.as_ref() == &TypeKind::I32
        )
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
                TypeJudgmentSubject::Expr { kind, .. },
                TypeJudgmentRule::Expected,
                Some(TypeKind::I32)
            ) if *kind == "literal"
        )
    }));
    assert!(report.judgments.iter().any(|judgment| {
        matches!(
            (&judgment.subject, judgment.rule, &judgment.expected),
            (
                TypeJudgmentSubject::Expr { kind, .. },
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
            .contains("let annotation expects i32, but expression has u64")
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
fn bad_score() -> bool {
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
fn typecheck_accepts_plain_string_inline_function_without_failure_policy() {
    let tree = parse_ok(
        r"
character @character.alice Alice as alice {}

flow @flow.opening opening {
    alice: #[i_to_string(score)]点[p]
}
",
    );
    let hir = lower_to_hir(&tree).expect("inline string function fixture lowers");

    typecheck_hir(
        &hir,
        &TypeCheckEnv::new()
            .with_symbol("alice", TypeKind::entity_ref(EntityKind::Character))
            .with_symbol("score", TypeKind::I32)
            .with_function("i_to_string", TypeKind::String),
    )
    .expect("plain string inline function does not need an inline failure policy");
}

#[test]
fn typecheck_accepts_diverging_if_else_function_body_for_declared_return() {
    let tree = parse_ok(
        r#"
character @character.alice Alice as alice {}

fn i_to_string(i: i32) -> String {
    if i == 0 {
        return "first"
    } else if i == 1 {
        return "second"
    } else {
        return "last"
    }
}

flow @flow.opening opening {
    alice: #[i_to_string(1i32)][p]
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("diverging if-else fixture lowers");

    typecheck_hir(
        &hir,
        &TypeCheckEnv::new().with_symbol("alice", TypeKind::entity_ref(EntityKind::Character)),
    )
    .expect("diverging if-else function body satisfies declared return type");
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
    let room = bg(@asset:.bg.room, target = @target.scene, slot = @slot.background.main)
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
asset bg.room {
    file = "bg/room.png"
    kind = image
}

asset bg.pulse {
    file = "bg/pulse.gif"
    kind = image
    animation = true
}

image @image.sample.pulse {
    asset = @asset:.bg.pulse
    target = @target.sample.pulse
    layer = @layer.foreground
    x = 96px
    y = 72px
    width = 360px
    height = 180px
}

flow @flow.opening opening {
    let room = bg(@asset:.bg.room, fit = "intrinsic", alignment.x = 1, alignment.y = 0.5, opacity = 0.75, playback.rate = 0.5, playback.local_time = 50ms)
    let pulse = image(asset = @asset:.bg.pulse, id = "image.sample.pulse", target = "target.sample.pulse", layer = "layer.foreground", x = 96px, y = 72px, width = 360px, height = 180px, fit = "stretch", alignment.x = 0.25, alignment.y = 750, opacity = 0.5, playback.start = 0.1, playback.rate = 0.5, transform.tx = 24px, transform.ty = 12px, transform.m11 = 1000, transform.m22 = 1000, depth = 2500, enabled = true, visible = true, action = "action.inspect.pulse", param.role = "animated-hotspot", proxy.id = "proxy.pulse.hotspot", proxy.type = "PulseHotspot", proxy.role = "inspect", proxy.layer = "layer.hit", proxy.depth = 2600, proxy.hit_test = true, proxy.param.channel = "preview")
    let declared = image(@image.sample.pulse)
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("presentation image fixture lowers");

    validate_hir_references(&hir, &registry_from_hir(&hir)).expect("declared image assets resolve");
    typecheck_hir(&hir, &TypeCheckEnv::new()).expect("presentation image call typechecks");
}

#[test]
fn typechecks_family_relative_asset_references_in_asset_expected_calls() {
    let tree = parse_ok(
        r#"
asset room {
    file = "bg/room.png"
    kind = image
}

asset pulse {
    file = "bg/pulse.gif"
    kind = image
    animation = true
}

flow @flow.opening opening {
    let room = bg(@asset:.room)
    let pulse = image(asset = @asset:.pulse, id = "image.sample.pulse")
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("family-relative asset refs lower");

    typecheck_hir(&hir, &TypeCheckEnv::new()).expect("family-relative asset refs typecheck");
}

#[test]
fn typecheck_rejects_presentation_slot_family_mismatch() {
    let tree = parse_ok(
        r"
flow @flow.opening opening {
    let room = bg(@asset:.bg.room, slot = @slot.character.alice.main)
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
    let room = bg(@asset:.bg.room)
    let evening = bg(@asset:.bg.evening)
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
fn typechecks_included_flow_target() {
    let tree = parse_ok(
        r"
pub flow alice_enters {
    alice: おはよう。[p]
}

flow @flow.opening opening {
    include @flow.alice_enters
}
",
    );
    let hir = lower_to_hir(&tree).expect("flow include fixture lowers");
    let env = TypeCheckEnv::new().with_symbol("alice", TypeKind::entity_ref(EntityKind::Character));

    typecheck_hir(&hir, &env).expect("flow include fixture typechecks");
}

#[test]
fn typecheck_reports_wrong_choice_target_kind() {
    let tree = parse_ok(
        r#"
flow @flow.opening opening {
    choice @choice.opening.first {
        @choice.opening.listen "聞く" -> @asset:.bg.room
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
fn typecheck_tracks_lifetime_registry_writes_as_inferred_effects() {
    let tree = parse_ok(
        r"
flow @flow.registry registry {
    'flow.flags.seen <- 1i32
}
",
    );
    let hir = lower_to_hir(&tree).expect("flow registry write lowers");
    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.is_empty(),
        "omitted effects infer lifetime writes without imposing a source bound: {:?}",
        report.diagnostics
    );
    let summary = report
        .effects
        .summary(&crate::effect_model::CallableId::new("flow.registry"))
        .expect("flow effect summary");
    assert!(
        summary
            .inferred()
            .contains(&crate::effects::EffectId::parse("state.write(flow)").expect("valid effect"))
    );

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
        .expect("matching source upper bound permits flow lifetime registry writes");

    let missing = parse_ok(
        r"
flow @flow.registry_contract registry_contract
effects {}
{
    'flow.flags.seen <- 1i32
}
",
    );
    let missing_hir = lower_to_hir(&missing).expect("empty-bound lifetime write lowers");
    let errors =
        typecheck_hir(&missing_hir, &TypeCheckEnv::new()).expect_err("empty bound is exceeded");
    assert!(errors.iter().any(|error| {
        matches!(
            error.kind(),
            TypeCheckErrorKind::Effect { diagnostic }
                if diagnostic.code()
                    == crate::effect_diagnostics::EffectDiagnosticCode::UpperBoundExceeded
                    && diagnostic.message().contains("state.write(flow)")
        )
    }));
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
    let ch: char = "あ"c
}
"#,
    );
    let hir = lower_to_hir(&ok).expect("char literal fixture lowers");
    typecheck_hir(&hir, &TypeCheckEnv::new()).expect("char literal typechecks");

    let bad = parse_ok(
        r#"
flow @flow.char_literal_bad char_literal_bad {
    let ch: char = "a"
}
"#,
    );
    let hir = lower_to_hir(&bad).expect("string literal fixture lowers");
    let errors = typecheck_hir(&hir, &TypeCheckEnv::new()).expect_err("string is not char");
    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("let annotation expects char"))
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
    let flags: Vec<bool> = nums.map(|item| item > 1i64)
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
fn typechecks_partial_placeholder_function_and_vec_map() {
    let tree = parse_ok(
        r"
struct Choice {
    label: String,
    score: i64,
    enabled: bool,
}

flow @flow.partial partial {
    let high: i64 -> bool = _ > 80i64
    let labels: Vec<String> = choices.map(_.label)
    let flags: Vec<bool> = choices.map(_.enabled)
    let enabled_labels: Vec<String> = choices.filter(_.enabled).map(_.label)
    log.info(high)
    log.info(labels)
    log.info(flags)
    log.info(enabled_labels)
}
",
    );
    let hir = lower_to_hir(&tree).expect("partial placeholder fixture lowers");
    validate_typecheck_ready(&hir).expect("partial placeholder fixture is structured");
    let env = TypeCheckEnv::new().with_symbol(
        "choices",
        TypeKind::Vec(Box::new(TypeKind::Named("Choice".to_owned()))),
    );

    let report = analyze_types(&hir, &env);
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
    assert!(
        report.typed_lowering_evidence.iter().any(|evidence| {
            matches!(
                &evidence.kind,
                TypedLoweringEvidenceKind::ExpectedFunctionValue {
                    expected_ty,
                    actual_ty,
                    arity: 1
                } if expected_ty == actual_ty
                    && expected_ty.function_arity() == Some(1)
            )
        }),
        "expected partial placeholder to record expected-function lowering evidence"
    );
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
    let flags: Vec<bool> = [true, false, true]
    let letters: Vec<char> = ["a"c, "b"c]
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
    let flags: Vec<bool> = nums.map(|item| item > 1i64)
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
fn typecheck_defers_unsafe_lifetime_audit_metadata_to_verifier() {
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
    typecheck_hir(&hir, &TypeCheckEnv::new())
        .expect("missing audit metadata is a verifier-owned policy obligation");
}

#[test]
fn typecheck_still_rejects_unrelated_errors_inside_unsafe_lifetime() {
    let tree = parse_ok(
        r#"
flow @flow.audit audit {
    unsafe lifetime @unsafe.cache_last_line {
        let summary: i32 = "not an int"
    }
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("unsafe lifetime block lowers");
    let errors = typecheck_hir(&hir, &TypeCheckEnv::new())
        .expect_err("ordinary type errors still stop before verifier repair actions");

    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("let annotation expects"))
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
