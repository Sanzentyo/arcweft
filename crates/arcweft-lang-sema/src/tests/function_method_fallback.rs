use super::support::*;

fn analyze_registered_method_fixture(
    profile: &str,
    source: &str,
) -> crate::checker::TypeCheckReport {
    let (document, project, world) =
        crate::test_support::character_project::root_project_source(profile, source);
    let facts = crate::registration::ProjectRegistrationFacts::try_new(
        world,
        vec![document],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .expect("registered method fixture facts");
    let registered = crate::test_support::character_project::register(
        &project,
        &facts,
        TypeCheckEnv::standard(),
        None,
    )
    .expect("registered method fixture world");
    crate::checker::analyze_registered_project_types(&project.linked_module(), &registered)
}

#[test]
fn method_chain_falls_back_to_data_last_callable_when_no_method_matches() {
    let tree = parse_ok(
        r"
flow method_fallback {
    let ok: bool = score.above(80i64)
    log.info(ok)
}
",
    );
    let hir = lower_document_to_hir(tree.document(), tree.typed_tree())
        .expect("method fallback fixture lowers");
    validate_typecheck_ready(&hir).expect("method fallback fixture is structured");
    let env = TypeCheckEnv::new()
        .with_symbol("score", TypeKind::I64)
        .with_function_signature(
            "above",
            FunctionSignature::new(
                TypeKind::Bool,
                [
                    FunctionParam::required("min", TypeKind::I64),
                    FunctionParam::required("value", TypeKind::I64),
                ],
            ),
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
                TypedLoweringEvidenceKind::DataLastMethodFallback {
                    method,
                    arg_count,
                    ..
                }
                    if method == "above" && *arg_count == 1
            )
        }),
        "expected method fallback evidence"
    );
}

#[test]
fn method_chain_fallback_preserves_curried_callable_groups() {
    let tree = parse_ok(
        r"
fn above(min: i64)(value: i64) -> bool {
    return value > min
}

flow curried_method_fallback {
    let positional: bool = score.above(80i64)
    let named: bool = score.above(min = 80i64)
    log.info(positional)
    log.info(named)
}
",
    );
    let hir = lower_document_to_hir(tree.document(), tree.typed_tree())
        .expect("curried method fallback fixture lowers");
    validate_typecheck_ready(&hir).expect("curried method fallback fixture is structured");
    let env = TypeCheckEnv::new().with_symbol("score", TypeKind::I64);

    let report = analyze_types(&hir, &env);
    assert!(
        report.diagnostics.is_empty(),
        "method fallback must mean above(min)(score), got {:?}",
        report.diagnostics
    );
    assert_eq!(
        report
            .typed_lowering_evidence
            .iter()
            .filter(|evidence| matches!(
                &evidence.kind,
                TypedLoweringEvidenceKind::DataLastMethodFallback {
                    method,
                    arg_count: 1,
                    arg_order,
                } if method == "above"
                    && arg_order == &[
                        DataLastMethodFallbackArg::CallArg { index: 0 },
                        DataLastMethodFallbackArg::Receiver,
                    ]
            ))
            .count(),
        2,
        "positional and named first-stage calls should share staged fallback evidence"
    );
}

#[test]
fn method_chain_accepts_named_data_last_fallback_and_records_arg_order() {
    let tree = parse_ok(
        r"
flow method_fallback_named {
    let ok: bool = score.above(min = 80i64)
    log.info(ok)
}
",
    );
    let hir = lower_document_to_hir(tree.document(), tree.typed_tree())
        .expect("named method fallback fixture lowers");
    validate_typecheck_ready(&hir).expect("named method fallback fixture is structured");
    let env = TypeCheckEnv::new()
        .with_symbol("score", TypeKind::I64)
        .with_function_signature(
            "above",
            FunctionSignature::new(
                TypeKind::Bool,
                [
                    FunctionParam::required("min", TypeKind::I64),
                    FunctionParam::required("value", TypeKind::I64),
                ],
            ),
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
                TypedLoweringEvidenceKind::DataLastMethodFallback {
                    method,
                    arg_count: 1,
                    arg_order,
                } if method == "above"
                    && arg_order == &[
                        DataLastMethodFallbackArg::CallArg { index: 0 },
                        DataLastMethodFallbackArg::Receiver,
                    ]
            )
        }),
        "expected named fallback order evidence, got {:?}",
        report.typed_lowering_evidence
    );
}

#[test]
fn method_chain_accepts_fixed_literal_spread_data_last_fallback() {
    let tree = parse_ok(
        r"
flow method_fallback_fixed_spread {
    let direct: bool = score.between([60i64, 90i64]...)
    let mixed: bool = score.between([60i64]..., max = 90i64)
    log.info(direct)
    log.info(mixed)
}
",
    );
    let hir = lower_document_to_hir(tree.document(), tree.typed_tree())
        .expect("fixed spread method fallback fixture lowers");
    validate_typecheck_ready(&hir).expect("fixed spread method fallback fixture is structured");
    let env = TypeCheckEnv::new()
        .with_symbol("score", TypeKind::I64)
        .with_function_signature(
            "between",
            FunctionSignature::new(
                TypeKind::Bool,
                [
                    FunctionParam::required("min", TypeKind::I64),
                    FunctionParam::required("max", TypeKind::I64),
                    FunctionParam::required("value", TypeKind::I64),
                ],
            ),
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
                TypedLoweringEvidenceKind::DataLastMethodFallback {
                    method,
                    arg_count: 1,
                    arg_order,
                } if method == "between"
                    && arg_order == &[
                        DataLastMethodFallbackArg::CallArg { index: 0 },
                        DataLastMethodFallbackArg::Receiver,
                    ]
            )
        }),
        "expected single spread fallback order evidence, got {:?}",
        report.typed_lowering_evidence
    );
    assert!(
        report.typed_lowering_evidence.iter().any(|evidence| {
            matches!(
                &evidence.kind,
                TypedLoweringEvidenceKind::DataLastMethodFallback {
                    method,
                    arg_count: 2,
                    arg_order,
                } if method == "between"
                    && arg_order == &[
                        DataLastMethodFallbackArg::CallArg { index: 0 },
                        DataLastMethodFallbackArg::CallArg { index: 1 },
                        DataLastMethodFallbackArg::Receiver,
                    ]
            )
        }),
        "expected mixed spread/named fallback order evidence, got {:?}",
        report.typed_lowering_evidence
    );
}

#[test]
fn method_chain_reports_spread_data_last_fallback_as_unsupported() {
    let tree = parse_ok(
        r"
flow method_fallback_spread {
    let thresholds = [80i64]
    let wrong = score.above(thresholds...)
    log.info(wrong)
}
",
    );
    let hir = lower_document_to_hir(tree.document(), tree.typed_tree())
        .expect("spread method fallback fixture lowers");
    validate_typecheck_ready(&hir).expect("spread method fallback fixture is structured");
    let env = TypeCheckEnv::new()
        .with_symbol("score", TypeKind::I64)
        .with_function_signature(
            "above",
            FunctionSignature::new(
                TypeKind::Bool,
                [
                    FunctionParam::required("min", TypeKind::I64),
                    FunctionParam::required("value", TypeKind::I64),
                ],
            ),
        );

    let report = analyze_types(&hir, &env);
    assert!(
        report.diagnostics.iter().any(|diagnostic| {
            matches!(
                diagnostic.kind(),
                TypeCheckErrorKind::UnsupportedDataLastMethodFallback { method, reason }
                    if method == "above" && reason.contains("spread arguments")
            )
        }),
        "expected spread fallback diagnostic, got {:?}",
        report.diagnostics
    );
    assert!(
        report
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message().contains("unknown method `above`")),
        "fallback candidate should not degrade to unknown method: {:?}",
        report.diagnostics
    );
}

#[test]
fn method_chain_reports_spread_then_named_data_last_fallback_as_unsupported() {
    let tree = parse_ok(
        r"
flow method_fallback_spread_then_named {
    let thresholds = [80i64]
    let wrong = score.between(thresholds..., max = 99i64)
    log.info(wrong)
}
",
    );
    let hir = lower_document_to_hir(tree.document(), tree.typed_tree())
        .expect("spread then named fallback fixture lowers");
    validate_typecheck_ready(&hir).expect("spread then named fallback fixture is structured");
    let env = TypeCheckEnv::new()
        .with_symbol("score", TypeKind::I64)
        .with_function_signature(
            "between",
            FunctionSignature::new(
                TypeKind::Bool,
                [
                    FunctionParam::required("min", TypeKind::I64),
                    FunctionParam::required("max", TypeKind::I64),
                    FunctionParam::required("value", TypeKind::I64),
                ],
            ),
        );

    let report = analyze_types(&hir, &env);
    assert!(
        report.diagnostics.iter().any(|diagnostic| {
            matches!(
                diagnostic.kind(),
                TypeCheckErrorKind::UnsupportedDataLastMethodFallback { method, reason }
                    if method == "between"
                        && reason.contains("followed by fixed data-last fallback arguments")
            )
        }),
        "expected spread-then-named fallback diagnostic, got {:?}",
        report.diagnostics
    );
    assert!(
        !report.typed_lowering_evidence.iter().any(|evidence| {
            matches!(
                &evidence.kind,
                TypedLoweringEvidenceKind::DataLastMethodFallback { method, .. }
                    if method == "between"
            )
        }),
        "unsupported spread-then-named fallback must not record selected evidence"
    );
}

#[test]
fn method_chain_reports_multiple_spread_data_last_fallback_as_unsupported() {
    let tree = parse_ok(
        r"
flow method_fallback_multiple_spreads {
    let lows = [60i64]
    let highs = [90i64]
    let wrong = score.between(lows..., highs...)
    log.info(wrong)
}
",
    );
    let hir = lower_document_to_hir(tree.document(), tree.typed_tree())
        .expect("multiple-spread fallback fixture lowers");
    validate_typecheck_ready(&hir).expect("multiple-spread fallback fixture is structured");
    let env = TypeCheckEnv::new()
        .with_symbol("score", TypeKind::I64)
        .with_function_signature(
            "between",
            FunctionSignature::new(
                TypeKind::Bool,
                [
                    FunctionParam::required("min", TypeKind::I64),
                    FunctionParam::required("max", TypeKind::I64),
                    FunctionParam::required("value", TypeKind::I64),
                ],
            ),
        );

    let report = analyze_types(&hir, &env);
    assert!(
        report.diagnostics.iter().any(|diagnostic| {
            matches!(
                diagnostic.kind(),
                TypeCheckErrorKind::UnsupportedDataLastMethodFallback { method, reason }
                    if method == "between" && reason.contains("multiple spread arguments")
            )
        }),
        "expected multiple-spread fallback diagnostic, got {:?}",
        report.diagnostics
    );
    assert!(
        !report.typed_lowering_evidence.iter().any(|evidence| {
            matches!(
                &evidence.kind,
                TypedLoweringEvidenceKind::DataLastMethodFallback { method, .. }
                    if method == "between"
            )
        }),
        "unsupported multiple-spread fallback must not record selected evidence"
    );
}

#[test]
fn method_chain_reports_ambiguous_data_last_fallback_candidates() {
    let tree = parse_ok(
        r"
#[pure]
fn above(min: i64, value: i64) -> bool {
    return value > min
}

flow method_fallback_ambiguous {
    let wrong = score.above(80i64)
}
",
    );
    let hir = lower_document_to_hir(tree.document(), tree.typed_tree())
        .expect("ambiguous method fallback fixture lowers");
    validate_typecheck_ready(&hir).expect("ambiguous method fallback fixture is structured");
    let env = TypeCheckEnv::new()
        .with_symbol("score", TypeKind::I64)
        .with_function_signature(
            "above",
            FunctionSignature::new(
                TypeKind::String,
                [
                    FunctionParam::required("threshold", TypeKind::I64),
                    FunctionParam::required("subject", TypeKind::I64),
                ],
            ),
        );

    let report = analyze_types(&hir, &env);
    assert!(
        report.diagnostics.iter().any(|diagnostic| {
            matches!(
                diagnostic.kind(),
                TypeCheckErrorKind::AmbiguousDataLastMethodFallback {
                    method,
                    receiver: TypeKind::I64,
                    candidates
                } if method == "above"
                    && candidates.len() == 2
                    && candidates.iter().any(|candidate| candidate.contains("module fn `above`"))
                    && candidates
                        .iter()
                        .any(|candidate| candidate.contains("environment fn `above`"))
            )
        }),
        "expected ambiguous fallback diagnostic, got {:?}",
        report.diagnostics
    );
    assert!(
        !report.typed_lowering_evidence.iter().any(|evidence| {
            matches!(
                &evidence.kind,
                TypedLoweringEvidenceKind::DataLastMethodFallback { method, .. }
                    if method == "above"
            )
        }),
        "ambiguous fallback must not record a selected lowering"
    );
}

#[test]
fn method_chain_reports_ambiguous_spread_data_last_fallback_candidates() {
    let tree = parse_ok(
        r"
#[pure]
fn above(min: i64, value: i64) -> bool {
    return value > min
}

flow method_fallback_spread_ambiguous {
    let thresholds = [80i64]
    let wrong = score.above(thresholds...)
}
",
    );
    let hir = lower_document_to_hir(tree.document(), tree.typed_tree())
        .expect("ambiguous spread fallback fixture lowers");
    validate_typecheck_ready(&hir).expect("ambiguous spread fallback fixture is structured");
    let env = TypeCheckEnv::new()
        .with_symbol("score", TypeKind::I64)
        .with_function_signature(
            "above",
            FunctionSignature::new(
                TypeKind::String,
                [
                    FunctionParam::required("threshold", TypeKind::I64),
                    FunctionParam::required("subject", TypeKind::I64),
                ],
            ),
        );

    let report = analyze_types(&hir, &env);
    assert!(
        report.diagnostics.iter().any(|diagnostic| {
            matches!(
                diagnostic.kind(),
                TypeCheckErrorKind::AmbiguousDataLastMethodFallback {
                    method,
                    receiver: TypeKind::I64,
                    candidates
                } if method == "above"
                    && candidates.len() == 2
                    && candidates.iter().any(|candidate| candidate.contains("module fn `above`"))
                    && candidates
                        .iter()
                        .any(|candidate| candidate.contains("environment fn `above`"))
            )
        }),
        "expected ambiguous spread fallback diagnostic, got {:?}",
        report.diagnostics
    );
    assert!(
        report.diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message()
                .contains("unsupported data-last fallback")
                && !diagnostic.message().contains("unknown method `above`")
        }),
        "ambiguous spread fallback should report candidate ambiguity first: {:?}",
        report.diagnostics
    );
    assert!(
        !report.typed_lowering_evidence.iter().any(|evidence| {
            matches!(
                &evidence.kind,
                TypedLoweringEvidenceKind::DataLastMethodFallback { method, .. }
                    if method == "above"
            )
        }),
        "ambiguous spread fallback must not record a selected lowering"
    );
}

#[test]
fn method_chain_prefers_real_method_over_data_last_callable_fallback() {
    let tree = parse_ok(
        r"
flow method_priority {
    let text: String = score.above(80i64)
    log.info(text)
}
",
    );
    let hir = lower_document_to_hir(tree.document(), tree.typed_tree())
        .expect("method priority fixture lowers");
    validate_typecheck_ready(&hir).expect("method priority fixture is structured");
    let env = TypeCheckEnv::new()
        .with_symbol("score", TypeKind::I64)
        .with_method_signature(
            TypeKind::I64,
            "above",
            FunctionSignature::new(
                TypeKind::String,
                [FunctionParam::required("min", TypeKind::I64)],
            ),
        )
        .with_function_signature(
            "above",
            FunctionSignature::new(
                TypeKind::Bool,
                [
                    FunctionParam::required("min", TypeKind::I64),
                    FunctionParam::required("value", TypeKind::I64),
                ],
            ),
        );

    let report = analyze_types(&hir, &env);
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
    assert!(
        report.warnings.iter().any(|warning| {
            matches!(
                warning.kind(),
                TypeCheckWarningKind::ShadowedDataLastMethodFallback {
                    method,
                    receiver: TypeKind::I64,
                    selected,
                    fallbacks
                } if method == "above"
                    && selected.contains("environment method `i64.above`")
                    && fallbacks.len() == 1
                    && fallbacks[0].contains("environment fn `above`")
            ) && warning.stable_code() == "sema.typecheck.shadowed_data_last_method_fallback"
        }),
        "expected shadowed fallback warning, got {:?}",
        report.warnings
    );
    assert!(
        !report.typed_lowering_evidence.iter().any(|evidence| {
            matches!(
                &evidence.kind,
                TypedLoweringEvidenceKind::DataLastMethodFallback { method, .. }
                    if method == "above"
            )
        }),
        "real method calls must not record data-last fallback evidence"
    );
}

#[test]
fn method_chain_prefers_trait_method_over_data_last_callable_fallback() {
    let report = analyze_registered_method_fixture(
        "trait-method-priority",
        r#"
struct Score {}

trait Threshold {
    fn above(self, min: i64) -> String
}

impl Threshold for Score {
    fn above(self, min: i64) -> String {
        return "trait"
    }
}

fn above(min: i64, value: Score) -> bool {
    true
}

flow method_trait_priority(score: Score) {
    let text: String = score.above(80i64)
    log.info(text)
}
"#,
    );
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
    assert!(
        report.retained_call_target_facts().any(|facts| {
            matches!(
                facts.target(),
                crate::callable::CallTargetFact::Selected { selected, .. }
                    if selected.id().family() == crate::callable::CallableFamily::TraitMethod
            )
        }),
        "accepted project checking must select the trait method before project data-last fallback"
    );
    assert!(
        !report.typed_lowering_evidence.iter().any(|evidence| {
            matches!(
                &evidence.kind,
                TypedLoweringEvidenceKind::DataLastMethodFallback { method, .. }
                    if method == "above"
            )
        }),
        "trait method calls must not record data-last fallback evidence"
    );
}

#[test]
fn trait_method_value_reference_reports_unsupported_method_value() {
    let report = analyze_registered_method_fixture(
        "trait-method-value",
        r#"
struct Score {}

trait Threshold {
    fn above(self, min: i64) -> String
}

impl Threshold for Score {
    fn above(self, min: i64) -> String {
        return "trait"
    }
}

flow method_trait_value(score: Score) {
    let method = score.above
}
"#,
    );
    assert!(
        report.diagnostics.iter().any(|diagnostic| {
            matches!(
                diagnostic.kind(),
                TypeCheckErrorKind::UnsupportedMethodValueReference {
                    receiver: TypeKind::ProjectNominal(receiver),
                    method,
                    reason
                } if receiver.declaration().name().as_str() == "Score"
                    && method == "above"
                    && reason.contains("receiver-binding contract")
            ) && diagnostic.stable_code() == "sema.typecheck.unsupported_method_value_reference"
        }),
        "expected unsupported method-value diagnostic, got {:?}",
        report.diagnostics
    );
    assert!(
        report.diagnostics.iter().all(|diagnostic| {
            !diagnostic.message().contains("unknown method `above`")
                && !diagnostic.message().contains("unknown field `above`")
        }),
        "method value references should not fall through to unknown-field/method diagnostics: {:?}",
        report.diagnostics
    );
}

#[test]
fn environment_method_value_reference_reports_unsupported_method_value() {
    let tree = parse_ok(
        r"
flow env_method_value {
    let method = score.above
}
",
    );
    let hir = lower_document_to_hir(tree.document(), tree.typed_tree())
        .expect("environment method value fixture lowers");
    validate_typecheck_ready(&hir).expect("environment method value fixture is structured");
    let env = TypeCheckEnv::new()
        .with_symbol("score", TypeKind::I64)
        .with_method_signature(
            TypeKind::I64,
            "above",
            FunctionSignature::new(
                TypeKind::String,
                [FunctionParam::required("min", TypeKind::I64)],
            ),
        );

    let report = analyze_types(&hir, &env);
    assert!(
        report.diagnostics.iter().any(|diagnostic| {
            matches!(
                diagnostic.kind(),
                TypeCheckErrorKind::UnsupportedMethodValueReference {
                    receiver: TypeKind::I64,
                    method,
                    reason
                } if method == "above" && reason.contains("receiver-binding contract")
            ) && diagnostic.stable_code() == "sema.typecheck.unsupported_method_value_reference"
        }),
        "expected unsupported method-value diagnostic, got {:?}",
        report.diagnostics
    );
}

#[test]
fn data_last_function_receiver_composes_when_curried_body_invokes_it() {
    let tree = parse_ok(
        r#"
fn use_loader(label: String, load: String -> String)(path: String) -> String {
    return load(path)
}

flow data_last_receiver_stage
effects { }
{
    let loader = |path: String| -> String {
        adapter.read_text(path = path)
    }
    let staged = loader.use_loader("story")
}

flow data_last_receiver_call
effects { }
{
    let loader = |path: String| -> String {
        adapter.read_text(path = path)
    }
    let staged = loader.use_loader("story")
    let body = staged("story.arcw")
}
"#,
    );
    let hir = lower_document_to_hir(tree.document(), tree.typed_tree())
        .expect("data-last function receiver fixture lowers");
    validate_typecheck_ready(&hir).expect("data-last function receiver fixture is structured");

    let report = analyze_types(&hir, &read_text_env());
    let effect = crate::effects::EffectId::parse("fs.read").expect("valid effect");
    assert!(
        report
            .effects
            .summary(&crate::effect_model::CallableId::new(
                "flow.data_last_receiver_stage",
            ))
            .expect("receiver stage summary")
            .inferred()
            .is_empty()
    );
    assert!(
        report
            .effects
            .summary(&crate::effect_model::CallableId::new(
                "flow.data_last_receiver_call",
            ))
            .expect("receiver final summary")
            .inferred()
            .contains(&effect)
    );
}

#[test]
fn data_last_callable_alias_retains_source_body_and_callback_identity() {
    let tree = parse_ok(
        r#"
fn audited_load(load: String -> String)(path: String) -> String {
    let audited = adapter.audit(path = path)
    return load(path)
}

flow data_last_alias_identity
effects { }
{
    let invoke = audited_load
    let body = "story.arcw".invoke(|path: String| -> String {
        adapter.read_text(path = path)
    })
}
"#,
    );
    let hir = lower_document_to_hir(tree.document(), tree.typed_tree())
        .expect("data-last alias identity fixture lowers");
    validate_typecheck_ready(&hir).expect("data-last alias identity fixture is structured");
    let env = read_text_env()
        .with_function_signature(
            "adapter.audit",
            FunctionSignature::new(
                TypeKind::Unit,
                [FunctionParam::required("path", TypeKind::String)],
            ),
        )
        .with_function_effects("adapter.audit", ["fs.audit".to_owned()]);

    let report = analyze_types(&hir, &env);
    let summary = report
        .effects
        .summary(&crate::effect_model::CallableId::new(
            "flow.data_last_alias_identity",
        ))
        .expect("alias flow summary");
    assert_eq!(summary.inferred().to_labels(), ["fs.audit", "fs.read"]);
}

#[test]
fn method_chain_data_last_fallback_composes_higher_order_callback_effects() {
    let tree = parse_ok(
        r#"
fn use_loader(load: String -> String, path: String) -> String {
    return load(path)
}

flow method_fallback_callback_effect
effects { }
{
    let body = "story.arcw".use_loader(|path: String| -> String {
        adapter.read_text(path = path)
    })
}
"#,
    );
    let hir = lower_document_to_hir(tree.document(), tree.typed_tree())
        .expect("method fallback callback fixture lowers");
    validate_typecheck_ready(&hir).expect("method fallback callback fixture is structured");

    let errors = typecheck_hir(&hir, &read_text_env())
        .expect_err("data-last fallback callback invocation must compose body effects");
    assert!(
        errors.iter().any(|error| {
            matches!(error.kind(), TypeCheckErrorKind::Effect { .. })
                && error
                    .message()
                    .contains("flow.method_fallback_callback_effect")
                && error.message().contains("fs.read")
        }),
        "expected data-last fallback callback effect diagnostic, got {errors:?}"
    );
}

#[test]
fn curried_method_fallback_defers_first_group_callback_until_final_call() {
    let tree = parse_ok(
        r#"
fn use_loader(load: String -> String, context: String)(path: String) -> String {
    return load(path)
}

flow curried_method_callback_stage
effects { }
{
    let staged = "context".use_loader(|path: String| -> String {
        adapter.read_text(path = path)
    })
}

flow curried_method_callback_final
effects { }
{
    let staged = "context".use_loader(|path: String| -> String {
        adapter.read_text(path = path)
    })
    let body = staged("story.arcw")
}
"#,
    );
    let hir = lower_document_to_hir(tree.document(), tree.typed_tree())
        .expect("curried method callback fixture lowers");
    validate_typecheck_ready(&hir).expect("curried method callback fixture is structured");

    let report = analyze_types(&hir, &read_text_env());
    let effect = crate::effects::EffectId::parse("fs.read").expect("valid effect");
    let staged = report
        .effects
        .summary(&crate::effect_model::CallableId::new(
            "flow.curried_method_callback_stage",
        ))
        .expect("staged method callback summary");
    assert!(
        staged.inferred().is_empty(),
        "completing the first group must not invoke the callback: {staged:?}"
    );
    let final_call = report
        .effects
        .summary(&crate::effect_model::CallableId::new(
            "flow.curried_method_callback_final",
        ))
        .expect("final method callback summary");
    assert!(final_call.inferred().contains(&effect));
}
