use super::support::*;

#[test]
fn curried_source_body_effects_begin_only_at_the_final_call_group() {
    let tree = parse_ok(
        r#"
fn read_with_prefix(prefix: String)(path: String) -> String
{
    return adapter.read_text(path = path)
}

flow @flow.curried_source_stage curried_source_stage
effects { }
{
    let staged = read_with_prefix("assets")
}

flow @flow.curried_source_final curried_source_final
effects { }
{
    let body = read_with_prefix("assets")("story.arcw")
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("curried source effect fixture lowers");
    validate_typecheck_ready(&hir).expect("curried source effect fixture is structured");

    let report = analyze_types(&hir, &read_text_env());
    let effect = crate::effects::EffectId::parse("fs.read").expect("valid effect");
    let stage = report
        .effects
        .summary(&crate::effect_model::CallableId::new(
            "flow.curried_source_stage",
        ))
        .expect("stage flow summary");
    assert!(
        stage.inferred().is_empty(),
        "supplying only the first source group must be effect-free: {stage:?}"
    );
    let final_call = report
        .effects
        .summary(&crate::effect_model::CallableId::new(
            "flow.curried_source_final",
        ))
        .expect("final flow summary");
    assert!(final_call.inferred().contains(&effect));
    assert!(report.diagnostics.iter().any(|error| {
        matches!(
            error.kind(),
            TypeCheckErrorKind::Effect { diagnostic }
                if diagnostic.callable().as_str() == "flow.curried_source_final"
                    && diagnostic.message().contains("fs.read")
        )
    }));
    assert!(!report.diagnostics.iter().any(|error| {
        matches!(
            error.kind(),
            TypeCheckErrorKind::Effect { diagnostic }
                if diagnostic.callable().as_str() == "flow.curried_source_stage"
                    && diagnostic.message().contains("fs.read")
        )
    }));
}

#[test]
fn curried_source_function_alias_preserves_final_group_effect_timing() {
    let tree = parse_ok(
        r#"
fn read_with_prefix(prefix: String)(path: String) -> String
{
    return adapter.read_text(path = path)
}

flow @flow.curried_source_alias_stage curried_source_alias_stage
effects { }
{
    let reader = read_with_prefix
    let staged = reader("assets")
}

flow @flow.curried_source_alias_final curried_source_alias_final
effects { }
{
    let reader = read_with_prefix
    let staged = reader("assets")
    let body = staged("story.arcw")
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("curried source alias effect fixture lowers");
    validate_typecheck_ready(&hir).expect("curried source alias effect fixture is structured");

    let report = analyze_types(&hir, &read_text_env());
    let effect = crate::effects::EffectId::parse("fs.read").expect("valid effect");
    assert!(report.judgments.iter().any(|judgment| {
        matches!(
            &judgment.subject,
            TypeJudgmentSubject::LetBinding { pattern } if pattern.contains("reader")
        ) && matches!(
            &judgment.ty,
            TypeKind::Function {
                effects: outer_effects,
                return_type,
                ..
            } if outer_effects.tail() == crate::effect_row::EffectRowTail::Closed
                && outer_effects.concrete().is_empty()
                && matches!(
                    return_type.as_ref(),
                    TypeKind::Function { effects, .. }
                        if matches!(
                            effects.tail(),
                            crate::effect_row::EffectRowTail::Variable(_)
                        )
                )
        )
    }));
    let stage = report
        .effects
        .summary(&crate::effect_model::CallableId::new(
            "flow.curried_source_alias_stage",
        ))
        .expect("alias stage flow summary");
    assert!(stage.inferred().is_empty());
    let final_call = report
        .effects
        .summary(&crate::effect_model::CallableId::new(
            "flow.curried_source_alias_final",
        ))
        .expect("alias final flow summary");
    assert!(final_call.inferred().contains(&effect));
}

#[test]
fn partial_first_curried_group_preserves_final_group_effect_timing() {
    let tree = parse_ok(
        r#"
fn read_with_prefix(prefix: String, base: String)(path: String) -> String {
    return adapter.read_text(path = path)
}

flow @flow.partial_first_group_stage partial_first_group_stage
effects { }
{
    let partial = read_with_prefix("assets")
    let staged = partial("base")
}

flow @flow.partial_first_group_final partial_first_group_final
effects { }
{
    let partial = read_with_prefix("assets")
    let staged = partial("base")
    let body = staged("story.arcw")
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("partial first curried group fixture lowers");
    validate_typecheck_ready(&hir).expect("partial first curried group fixture is structured");

    let report = analyze_types(&hir, &read_text_env());
    let effect = crate::effects::EffectId::parse("fs.read").expect("valid effect");
    let staged = report
        .effects
        .summary(&crate::effect_model::CallableId::new(
            "flow.partial_first_group_stage",
        ))
        .expect("partial stage summary");
    assert!(
        staged.inferred().is_empty(),
        "completing a non-final curried group must remain effect-free: {staged:?}"
    );
    let final_call = report
        .effects
        .summary(&crate::effect_model::CallableId::new(
            "flow.partial_first_group_final",
        ))
        .expect("partial final summary");
    assert!(final_call.inferred().contains(&effect));
}

#[test]
fn first_group_callback_effect_is_deferred_until_curried_body_invocation() {
    let tree = parse_ok(
        r#"
fn use_loader(load: String -> String)(path: String) -> String {
    return load(path)
}

flow @flow.first_group_callback_stage first_group_callback_stage
effects { }
{
    let staged = use_loader(|path: String| -> String {
        adapter.read_text(path = path)
    })
}

flow @flow.first_group_callback_final first_group_callback_final
effects { }
{
    let staged = use_loader(|path: String| -> String {
        adapter.read_text(path = path)
    })
    let body = staged("story.arcw")
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("first-group callback effect fixture lowers");
    validate_typecheck_ready(&hir).expect("first-group callback effect fixture is structured");

    let report = analyze_types(&hir, &read_text_env());
    let effect = crate::effects::EffectId::parse("fs.read").expect("valid effect");
    assert!(
        report
            .effects
            .summary(&crate::effect_model::CallableId::new(
                "flow.first_group_callback_stage",
            ))
            .expect("callback stage summary")
            .inferred()
            .is_empty(),
        "capturing a callback in an intermediate group must remain effect-free"
    );
    assert!(
        report
            .effects
            .summary(&crate::effect_model::CallableId::new(
                "flow.first_group_callback_final",
            ))
            .expect("callback final summary")
            .inferred()
            .contains(&effect)
    );
}

#[test]
fn partial_uncurried_callback_effect_is_deferred_until_exact_application() {
    let tree = parse_ok(
        r#"
fn use_loader(load: String -> String, path: String) -> String {
    return load(path)
}

flow @flow.partial_callback_stage partial_callback_stage
effects { }
{
    let partial = use_loader(|path: String| -> String {
        adapter.read_text(path = path)
    })
}

flow @flow.partial_callback_final partial_callback_final
effects { }
{
    let partial = use_loader(|path: String| -> String {
        adapter.read_text(path = path)
    })
    let body = partial("story.arcw")
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("partial callback effect fixture lowers");
    validate_typecheck_ready(&hir).expect("partial callback effect fixture is structured");

    let report = analyze_types(&hir, &read_text_env());
    let effect = crate::effects::EffectId::parse("fs.read").expect("valid effect");
    assert!(
        report
            .effects
            .summary(&crate::effect_model::CallableId::new(
                "flow.partial_callback_stage",
            ))
            .expect("partial callback stage summary")
            .inferred()
            .is_empty(),
        "partial application must not invoke the callback"
    );
    assert!(
        report
            .effects
            .summary(&crate::effect_model::CallableId::new(
                "flow.partial_callback_final",
            ))
            .expect("partial callback final summary")
            .inferred()
            .contains(&effect)
    );
}

#[test]
fn local_closure_call_composes_body_effects_into_caller() {
    let tree = parse_ok(
        r#"
flow @flow.closure_effect_call closure_effect_call
effects { }
{
    let later = || -> String {
        adapter.read_text(path = "story.arcw")
    }
    let body = later()
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("closure effect call fixture lowers");
    validate_typecheck_ready(&hir).expect("closure effect call fixture is structured");

    let errors = typecheck_hir(&hir, &read_text_env())
        .expect_err("calling the closure must compose body effects into the caller");
    assert!(
        errors.iter().any(|error| {
            matches!(error.kind(), TypeCheckErrorKind::Effect { .. })
                && error.message().contains("flow.closure_effect_call")
                && error.message().contains("fs.read")
        }),
        "expected closure call effect upper-bound diagnostic, got {errors:?}"
    );
}

#[test]
fn no_effect_rejects_local_closure_effect_when_called() {
    let tree = parse_ok(
        r#"
flow @flow.no_effect_closure_call no_effect_closure_call
effects { fs.read }
ensures no_effect fs.read
{
    let later = || -> String {
        adapter.read_text(path = "story.arcw")
    }
    let body = later()
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("no-effect closure fixture lowers");
    validate_typecheck_ready(&hir).expect("no-effect closure fixture is structured");

    let errors = typecheck_hir(&hir, &read_text_env())
        .expect_err("no_effect must reject closure body effects when the value is called");
    assert!(
        errors.iter().any(|error| {
            matches!(error.kind(), TypeCheckErrorKind::Effect { .. })
                && error.message().contains("flow.no_effect_closure_call")
                && error.message().contains("forbids effect `fs.read`")
        }),
        "expected no_effect closure call diagnostic, got {errors:?}"
    );
}

#[test]
fn immediate_closure_call_composes_body_effects_into_caller() {
    let tree = parse_ok(
        r#"
flow @flow.immediate_closure_effect_call immediate_closure_effect_call
effects { }
{
    let body = (|| -> String {
        adapter.read_text(path = "story.arcw")
    })()
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("immediate closure effect call fixture lowers");
    validate_typecheck_ready(&hir).expect("immediate closure effect call fixture is structured");

    let errors = typecheck_hir(&hir, &read_text_env())
        .expect_err("calling an immediate closure must compose body effects into the caller");
    assert!(
        errors.iter().any(|error| {
            matches!(error.kind(), TypeCheckErrorKind::Effect { .. })
                && error
                    .message()
                    .contains("flow.immediate_closure_effect_call")
                && error.message().contains("fs.read")
        }),
        "expected immediate closure call effect upper-bound diagnostic, got {errors:?}"
    );
}

#[test]
fn partial_local_closure_application_does_not_compose_until_called() {
    let tree = parse_ok(
        r#"
flow @flow.partial_closure_effect_creation partial_closure_effect_creation
effects { }
{
    let later = |path: String, suffix: String| -> String {
        adapter.read_text(path = path)
    }
    let suffixer = later("story.arcw")
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("partial closure effect fixture lowers");
    validate_typecheck_ready(&hir).expect("partial closure effect fixture is structured");

    let report = analyze_types(&hir, &read_text_env());
    assert!(
        report.diagnostics.is_empty(),
        "partial closure application should not perform the closure body effects: {:?}",
        report.diagnostics
    );
    let flow_summary = report
        .effects
        .summary(&crate::effect_model::CallableId::new(
            "flow.partial_closure_effect_creation",
        ))
        .expect("flow summary");
    assert!(
        flow_summary.inferred().is_empty(),
        "flow should not infer partial closure body effects at creation: {flow_summary:?}"
    );
}

#[test]
fn partial_immediate_closure_application_does_not_compose_until_called() {
    let tree = parse_ok(
        r#"
flow @flow.partial_immediate_closure_effect_creation partial_immediate_closure_effect_creation
effects { }
{
    let suffixer = (|path: String, suffix: String| -> String {
        adapter.read_text(path = path)
    })("story.arcw")
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("partial immediate closure effect fixture lowers");
    validate_typecheck_ready(&hir).expect("partial immediate closure effect fixture is structured");

    let report = analyze_types(&hir, &read_text_env());
    assert!(
        report.diagnostics.is_empty(),
        "partial immediate closure application should not perform body effects: {:?}",
        report.diagnostics
    );
    let flow_summary = report
        .effects
        .summary(&crate::effect_model::CallableId::new(
            "flow.partial_immediate_closure_effect_creation",
        ))
        .expect("flow summary");
    assert!(
        flow_summary.inferred().is_empty(),
        "flow should not infer partial immediate closure body effects at creation: {flow_summary:?}"
    );
}

#[test]
fn partial_local_closure_alias_composes_body_effects_when_called() {
    let tree = parse_ok(
        r#"
flow @flow.partial_closure_effect_call partial_closure_effect_call
effects { }
{
    let later = |path: String, suffix: String| -> String {
        adapter.read_text(path = path)
    }
    let suffixer = later("story.arcw")
    let body = suffixer(".bak")
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("partial closure effect call fixture lowers");
    validate_typecheck_ready(&hir).expect("partial closure effect call fixture is structured");

    let errors = typecheck_hir(&hir, &read_text_env())
        .expect_err("calling the partial closure alias must compose body effects");
    assert!(
        errors.iter().any(|error| {
            matches!(error.kind(), TypeCheckErrorKind::Effect { .. })
                && error.message().contains("flow.partial_closure_effect_call")
                && error.message().contains("fs.read")
        }),
        "expected partial closure call effect upper-bound diagnostic, got {errors:?}"
    );
}

#[test]
fn no_effect_rejects_partial_closure_alias_effect_when_called() {
    let tree = parse_ok(
        r#"
flow @flow.no_effect_partial_closure_call no_effect_partial_closure_call
effects { fs.read }
ensures no_effect fs.read
{
    let later = |path: String, suffix: String| -> String {
        adapter.read_text(path = path)
    }
    let suffixer = later("story.arcw")
    let body = suffixer(".bak")
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("no-effect partial closure fixture lowers");
    validate_typecheck_ready(&hir).expect("no-effect partial closure fixture is structured");

    let errors = typecheck_hir(&hir, &read_text_env())
        .expect_err("no_effect must reject partial closure body effects when called");
    assert!(
        errors.iter().any(|error| {
            matches!(error.kind(), TypeCheckErrorKind::Effect { .. })
                && error
                    .message()
                    .contains("flow.no_effect_partial_closure_call")
                && error.message().contains("forbids effect `fs.read`")
        }),
        "expected no_effect partial closure call diagnostic, got {errors:?}"
    );
}

#[test]
fn partial_immediate_closure_alias_composes_body_effects_when_called() {
    let tree = parse_ok(
        r#"
flow @flow.partial_immediate_closure_effect_call partial_immediate_closure_effect_call
effects { }
{
    let suffixer = (|path: String, suffix: String| -> String {
        adapter.read_text(path = path)
    })("story.arcw")
    let body = suffixer(".bak")
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("partial immediate closure effect call fixture lowers");
    validate_typecheck_ready(&hir)
        .expect("partial immediate closure effect call fixture is structured");

    let errors = typecheck_hir(&hir, &read_text_env())
        .expect_err("calling the partial immediate closure alias must compose body effects");
    assert!(
        errors.iter().any(|error| {
            matches!(error.kind(), TypeCheckErrorKind::Effect { .. })
                && error
                    .message()
                    .contains("flow.partial_immediate_closure_effect_call")
                && error.message().contains("fs.read")
        }),
        "expected partial immediate closure call effect upper-bound diagnostic, got {errors:?}"
    );
}

#[test]
fn map_closure_argument_composes_body_effects_into_caller() {
    let tree = parse_ok(
        r#"
flow @flow.map_closure_effect_arg map_closure_effect_arg
effects { }
{
    let paths: Vec<String> = ["story.arcw"]
    let bodies: Vec<String> = paths.map(|path: String| -> String {
        adapter.read_text(path = path)
    })
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("map closure effect arg fixture lowers");
    validate_typecheck_ready(&hir).expect("map closure effect arg fixture is structured");

    let errors = typecheck_hir(&hir, &read_text_env())
        .expect_err("map must compose closure argument body effects into the caller");
    assert!(
        errors.iter().any(|error| {
            matches!(error.kind(), TypeCheckErrorKind::Effect { .. })
                && error.message().contains("flow.map_closure_effect_arg")
                && error.message().contains("fs.read")
        }),
        "expected map closure argument effect upper-bound diagnostic, got {errors:?}"
    );
}

#[test]
fn map_local_closure_alias_composes_body_effects_into_caller() {
    let tree = parse_ok(
        r#"
flow @flow.map_closure_alias_effect_arg map_closure_alias_effect_arg
effects { }
{
    let paths: Vec<String> = ["story.arcw"]
    let load = |path: String| -> String {
        adapter.read_text(path = path)
    }
    let bodies: Vec<String> = paths.map(load)
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("map closure alias effect arg fixture lowers");
    validate_typecheck_ready(&hir).expect("map closure alias effect arg fixture is structured");

    let errors = typecheck_hir(&hir, &read_text_env())
        .expect_err("map must compose local closure alias body effects into the caller");
    assert!(
        errors.iter().any(|error| {
            matches!(error.kind(), TypeCheckErrorKind::Effect { .. })
                && error
                    .message()
                    .contains("flow.map_closure_alias_effect_arg")
                && error.message().contains("fs.read")
        }),
        "expected map closure alias effect upper-bound diagnostic, got {errors:?}"
    );
}

#[test]
fn map_partial_closure_alias_composes_body_effects_into_caller() {
    let tree = parse_ok(
        r#"
flow @flow.map_partial_closure_effect_arg map_partial_closure_effect_arg
effects { }
{
    let suffixes: Vec<String> = [".bak"]
    let load = |path: String, suffix: String| -> String {
        adapter.read_text(path = path)
    }
    let suffixer = load("story.arcw")
    let bodies: Vec<String> = suffixes.map(suffixer)
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("map partial closure effect arg fixture lowers");
    validate_typecheck_ready(&hir).expect("map partial closure effect arg fixture is structured");

    let errors = typecheck_hir(&hir, &read_text_env())
        .expect_err("map must compose partial closure alias body effects into the caller");
    assert!(
        errors.iter().any(|error| {
            matches!(error.kind(), TypeCheckErrorKind::Effect { .. })
                && error
                    .message()
                    .contains("flow.map_partial_closure_effect_arg")
                && error.message().contains("fs.read")
        }),
        "expected map partial closure alias effect upper-bound diagnostic, got {errors:?}"
    );
}

#[test]
fn filter_closure_argument_composes_body_effects_into_caller() {
    let tree = parse_ok(
        r#"
flow @flow.filter_closure_effect_arg filter_closure_effect_arg
effects { }
{
    let paths: Vec<String> = ["story.arcw"]
    let kept: Vec<String> = paths.filter(|path: String| -> bool {
        adapter.read_text(path = path) == "ok"
    })
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("filter closure effect arg fixture lowers");
    validate_typecheck_ready(&hir).expect("filter closure effect arg fixture is structured");

    let errors = typecheck_hir(&hir, &read_text_env())
        .expect_err("filter must compose closure argument body effects into the caller");
    assert!(
        errors.iter().any(|error| {
            matches!(error.kind(), TypeCheckErrorKind::Effect { .. })
                && error.message().contains("flow.filter_closure_effect_arg")
                && error.message().contains("fs.read")
        }),
        "expected filter closure argument effect upper-bound diagnostic, got {errors:?}"
    );
}

#[test]
fn user_higher_order_function_argument_composes_when_param_is_called() {
    let tree = parse_ok(
        r#"
fn use_loader(path: String, load: String -> String) -> String {
    return load(path)
}

flow @flow.user_higher_order_closure_effect user_higher_order_closure_effect
effects { }
{
    let body = use_loader("story.arcw", |path: String| -> String {
        adapter.read_text(path = path)
    })
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("user higher-order effect fixture lowers");
    validate_typecheck_ready(&hir).expect("user higher-order effect fixture is structured");

    let errors = typecheck_hir(&hir, &read_text_env())
        .expect_err("helper callback invocation must compose body effects into the caller");
    assert!(
        errors.iter().any(|error| {
            matches!(error.kind(), TypeCheckErrorKind::Effect { .. })
                && error
                    .message()
                    .contains("flow.user_higher_order_closure_effect")
                && error.message().contains("fs.read")
        }),
        "expected user higher-order callback effect diagnostic, got {errors:?}"
    );
}

#[test]
fn user_higher_order_function_argument_does_not_compose_when_param_is_not_called() {
    let tree = parse_ok(
        r"
fn keep_loader(load: String -> String) -> Unit {
    let _ = load
}

flow @flow.user_higher_order_kept_closure user_higher_order_kept_closure
effects { }
{
    let load = |path: String| -> String {
        adapter.read_text(path = path)
    }
    let kept = keep_loader(load)
}
",
    );
    let hir = lower_to_hir(&tree).expect("kept higher-order effect fixture lowers");
    validate_typecheck_ready(&hir).expect("kept higher-order effect fixture is structured");

    let report = analyze_types(&hir, &read_text_env());
    assert!(
        report.diagnostics.is_empty(),
        "kept callback should not compose body effects into the caller: {:?}",
        report.diagnostics
    );
    let summary = report
        .effects
        .summary(&crate::effect_model::CallableId::new(
            "flow.user_higher_order_kept_closure",
        ))
        .expect("flow summary");
    assert!(
        summary.inferred().is_empty(),
        "flow should not infer kept callback effects: {summary:?}"
    );
}

#[test]
fn returned_closure_callback_does_not_compose_until_closure_is_called() {
    let tree = parse_ok(
        r#"
fn make_loader(load: String -> String) -> Unit -> String {
    return |_unit: Unit| -> String { load("story.arcw") }
}

flow @flow.returned_closure_callback_creation returned_closure_callback_creation
effects { }
{
    let loader = make_loader(|path: String| -> String {
        adapter.read_text(path = path)
    })
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("returned closure creation fixture lowers");
    validate_typecheck_ready(&hir).expect("returned closure creation fixture is structured");

    let report = analyze_types(&hir, &read_text_env());
    assert!(
        report.diagnostics.is_empty(),
        "creating the returned closure should not compose callback body effects: {:?}",
        report.diagnostics
    );
    let flow_summary = report
        .effects
        .summary(&crate::effect_model::CallableId::new(
            "flow.returned_closure_callback_creation",
        ))
        .expect("flow summary");
    assert!(
        flow_summary.inferred().is_empty(),
        "flow should not infer returned closure callback effects at creation: {flow_summary:?}"
    );
}

#[test]
fn returned_closure_callback_composes_when_returned_closure_is_called() {
    let tree = parse_ok(
        r#"
fn make_loader(load: String -> String) -> Unit -> String {
    return |_unit: Unit| -> String { load("story.arcw") }
}

flow @flow.returned_closure_callback_call returned_closure_callback_call
effects { }
{
    let loader = make_loader(|path: String| -> String {
        adapter.read_text(path = path)
    })
    let body = loader(())
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("returned closure call fixture lowers");
    validate_typecheck_ready(&hir).expect("returned closure call fixture is structured");

    let errors = typecheck_hir(&hir, &read_text_env())
        .expect_err("calling the returned closure must compose callback body effects");
    assert!(
        errors.iter().any(|error| {
            matches!(error.kind(), TypeCheckErrorKind::Effect { .. })
                && error
                    .message()
                    .contains("flow.returned_closure_callback_call")
                && error.message().contains("fs.read")
        }),
        "expected returned closure callback effect diagnostic, got {errors:?}"
    );
}

#[test]
fn effect_trace_report_records_returned_closure_callback_origin() {
    let tree = parse_ok(
        r#"
fn make_loader(load: String -> String) -> Unit -> String {
    return |_unit: Unit| -> String { load("story.arcw") }
}

flow @flow.returned_closure_callback_trace returned_closure_callback_trace
effects { }
{
    let loader = make_loader(|path: String| -> String {
        adapter.read_text(path = path)
    })
    let body = loader(())
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("returned closure trace fixture lowers");
    validate_typecheck_ready(&hir).expect("returned closure trace fixture is structured");

    let report = analyze_types(&hir, &read_text_env());
    let callable = crate::effect_model::CallableId::new("flow.returned_closure_callback_trace");
    let effect = crate::effects::EffectId::parse("fs.read").expect("valid effect");
    let trace = report
        .effects
        .effect_traces()
        .trace(&callable, &effect)
        .expect("flow effect trace is available from the analysis report");

    assert_eq!(trace.effect(), &effect);
    assert!(
        report
            .effects
            .effect_traces()
            .traces_for(&callable)
            .iter()
            .any(|summary| summary.callable() == &callable && summary.effect() == &effect),
        "trace summary should retain the owning callable"
    );
    assert!(
        trace.steps().iter().any(|step| matches!(
            step,
            crate::effect_diagnostics::EffectTraceStep::Call { .. }
        )),
        "returned callback origin should include at least one local callable edge: {trace:?}"
    );
    assert!(
        trace.steps().iter().any(|step| {
            matches!(
                step,
                crate::effect_diagnostics::EffectTraceStep::ExternalCall { callee, .. }
                    if callee == "adapter.read_text"
            )
        }),
        "returned callback origin should end at the adapter read: {trace:?}"
    );
}

#[test]
fn no_effect_rejects_returned_closure_callback_when_called() {
    let tree = parse_ok(
        r#"
fn make_loader(load: String -> String) -> Unit -> String {
    return |_unit: Unit| -> String { load("story.arcw") }
}

flow @flow.no_effect_returned_closure_callback_call no_effect_returned_closure_callback_call
effects { fs.read }
ensures no_effect fs.read
{
    let loader = make_loader(|path: String| -> String {
        adapter.read_text(path = path)
    })
    let body = loader(())
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("no-effect returned closure fixture lowers");
    validate_typecheck_ready(&hir).expect("no-effect returned closure fixture is structured");

    let errors = typecheck_hir(&hir, &read_text_env())
        .expect_err("no_effect must reject returned closure callback effects when called");
    assert!(
        errors.iter().any(|error| {
            matches!(error.kind(), TypeCheckErrorKind::Effect { .. })
                && error
                    .message()
                    .contains("flow.no_effect_returned_closure_callback_call")
                && error.message().contains("forbids effect `fs.read`")
        }),
        "expected no_effect returned closure callback diagnostic, got {errors:?}"
    );
}

#[test]
fn stored_returned_closure_callback_composes_when_returned_closure_is_called() {
    let tree = parse_ok(
        r#"
fn make_loader(load: String -> String) -> Unit -> String {
    let runner = |_unit: Unit| -> String { load("story.arcw") }
    return runner
}

flow @flow.stored_returned_closure_callback_call stored_returned_closure_callback_call
effects { }
{
    let loader = make_loader(|path: String| -> String {
        adapter.read_text(path = path)
    })
    let body = loader(())
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("stored returned closure call fixture lowers");
    validate_typecheck_ready(&hir).expect("stored returned closure call fixture is structured");

    let errors = typecheck_hir(&hir, &read_text_env())
        .expect_err("calling the stored returned closure must compose callback body effects");
    assert!(
        errors.iter().any(|error| {
            matches!(error.kind(), TypeCheckErrorKind::Effect { .. })
                && error
                    .message()
                    .contains("flow.stored_returned_closure_callback_call")
                && error.message().contains("fs.read")
        }),
        "expected stored returned closure callback effect diagnostic, got {errors:?}"
    );
}

#[test]
fn returned_closure_captured_function_alias_does_not_compose_until_called() {
    let tree = parse_ok(
        r#"
fn make_loader(load: String -> String) -> Unit -> String {
    let alias = load
    let selected = alias
    return |_unit: Unit| -> String { selected("story.arcw") }
}

flow @flow.returned_closure_alias_creation returned_closure_alias_creation
effects { }
{
    let loader = make_loader(|path: String| -> String {
        adapter.read_text(path = path)
    })
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("returned closure alias creation fixture lowers");
    validate_typecheck_ready(&hir).expect("returned closure alias creation fixture is structured");

    let report = analyze_types(&hir, &read_text_env());
    assert!(
        report.diagnostics.is_empty(),
        "creating a returned closure through a function alias should not compose effects: {:?}",
        report.diagnostics
    );
    let flow_summary = report
        .effects
        .summary(&crate::effect_model::CallableId::new(
            "flow.returned_closure_alias_creation",
        ))
        .expect("flow summary");
    assert!(
        flow_summary.inferred().is_empty(),
        "flow should not infer aliased returned closure callback effects at creation: {flow_summary:?}"
    );
}

#[test]
fn returned_closure_captured_function_alias_composes_when_called() {
    let tree = parse_ok(
        r#"
fn make_loader(load: String -> String) -> Unit -> String {
    let alias = load
    let selected = alias
    return |_unit: Unit| -> String { selected("story.arcw") }
}

flow @flow.returned_closure_alias_call returned_closure_alias_call
effects { }
{
    let loader = make_loader(|path: String| -> String {
        adapter.read_text(path = path)
    })
    let body = loader(())
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("returned closure alias call fixture lowers");
    validate_typecheck_ready(&hir).expect("returned closure alias call fixture is structured");

    let errors = typecheck_hir(&hir, &read_text_env())
        .expect_err("calling the returned closure alias must compose callback body effects");
    assert!(
        errors.iter().any(|error| {
            matches!(error.kind(), TypeCheckErrorKind::Effect { .. })
                && error.message().contains("flow.returned_closure_alias_call")
                && error.message().contains("fs.read")
        }),
        "expected returned closure alias callback effect diagnostic, got {errors:?}"
    );
}

#[test]
fn destructured_higher_order_tuple_argument_composes_when_binding_is_called() {
    let tree = parse_ok(
        r#"
fn use_loader((path, load): (String, String -> String)) -> String {
    return load(path)
}

flow @flow.destructured_higher_order_tuple_effect destructured_higher_order_tuple_effect
effects { }
{
    let load = |path: String| -> String {
        adapter.read_text(path = path)
    }
    let body = use_loader(("story.arcw", load))
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("destructured higher-order fixture lowers");
    validate_typecheck_ready(&hir).expect("destructured higher-order fixture is structured");

    let errors = typecheck_hir(&hir, &read_text_env())
        .expect_err("destructured callback invocation must compose body effects");
    assert!(
        errors.iter().any(|error| {
            matches!(error.kind(), TypeCheckErrorKind::Effect { .. })
                && error
                    .message()
                    .contains("flow.destructured_higher_order_tuple_effect")
                && error.message().contains("fs.read")
        }),
        "expected destructured higher-order callback effect diagnostic, got {errors:?}"
    );
}

#[test]
fn destructured_higher_order_tuple_argument_does_not_compose_when_binding_is_kept() {
    let tree = parse_ok(
        r#"
fn keep_loader((_path, load): (String, String -> String)) -> Unit {
    let _ = load
}

flow @flow.destructured_higher_order_tuple_kept destructured_higher_order_tuple_kept
effects { }
{
    let load = |path: String| -> String {
        adapter.read_text(path = path)
    }
    let kept = keep_loader(("story.arcw", load))
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("destructured kept fixture lowers");
    validate_typecheck_ready(&hir).expect("destructured kept fixture is structured");

    let report = analyze_types(&hir, &read_text_env());
    assert!(
        report.diagnostics.is_empty(),
        "kept destructured callback should not compose effects: {:?}",
        report.diagnostics
    );
    let summary = report
        .effects
        .summary(&crate::effect_model::CallableId::new(
            "flow.destructured_higher_order_tuple_kept",
        ))
        .expect("flow summary");
    assert!(
        summary.inferred().is_empty(),
        "flow should not infer kept destructured callback effects: {summary:?}"
    );
}

#[test]
fn destructured_nested_tuple_inline_closure_composes_when_binding_is_called() {
    let tree = parse_ok(
        r#"
fn use_loader(((load, suffix), path): ((String -> String, String), String)) -> String {
    return load(path)
}

flow @flow.destructured_nested_tuple_inline_closure_effect destructured_nested_tuple_inline_closure_effect
effects { }
{
    let body = use_loader(((|path: String| -> String {
        adapter.read_text(path = path)
    }, ".bak"), "story.arcw"))
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("nested destructured higher-order fixture lowers");
    validate_typecheck_ready(&hir).expect("nested destructured higher-order fixture is structured");

    let errors = typecheck_hir(&hir, &read_text_env())
        .expect_err("nested destructured inline callback must compose body effects");
    assert!(
        errors.iter().any(|error| {
            matches!(error.kind(), TypeCheckErrorKind::Effect { .. })
                && error
                    .message()
                    .contains("flow.destructured_nested_tuple_inline_closure_effect")
                && error.message().contains("fs.read")
        }),
        "expected nested destructured inline callback effect diagnostic, got {errors:?}"
    );
}

#[test]
fn destructured_record_inline_closure_composes_when_binding_is_called() {
    let tree = parse_ok(
        r#"
struct LoaderSpec {
    load: String -> String,
    path: String,
}

fn use_loader(LoaderSpec { load: load: String -> String, path }: LoaderSpec) -> String {
    return load(path)
}

flow @flow.destructured_record_inline_closure_effect destructured_record_inline_closure_effect
effects { }
{
    let body = use_loader(LoaderSpec {
        load: |path: String| -> String {
            adapter.read_text(path = path)
        },
        path: "story.arcw",
    })
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("record destructured higher-order fixture lowers");
    validate_typecheck_ready(&hir).expect("record destructured higher-order fixture is structured");

    let errors = typecheck_hir(&hir, &read_text_env())
        .expect_err("record destructured inline callback must compose body effects");
    assert!(
        errors.iter().any(|error| {
            matches!(error.kind(), TypeCheckErrorKind::Effect { .. })
                && error
                    .message()
                    .contains("flow.destructured_record_inline_closure_effect")
                && error.message().contains("fs.read")
        }),
        "expected record destructured inline callback effect diagnostic, got {errors:?}"
    );
}

#[test]
fn untyped_destructured_record_callback_uses_nominal_field_type() {
    let tree = parse_ok(
        r#"
struct LoaderSpec {
    load: String -> String,
    path: String,
}

fn use_loader(LoaderSpec { load, path }: LoaderSpec) -> String {
    return load(path)
}

flow @flow.untyped_destructured_record_callback_effect untyped_destructured_record_callback_effect
effects { }
{
    let body = use_loader(LoaderSpec {
        load: |path: String| -> String {
            adapter.read_text(path = path)
        },
        path: "story.arcw",
    })
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("untyped record destructured fixture lowers");
    validate_typecheck_ready(&hir).expect("untyped record destructured fixture is structured");

    let errors = typecheck_hir(&hir, &read_text_env())
        .expect_err("untyped record destructured callback must compose body effects");
    assert!(
        errors.iter().any(|error| {
            matches!(error.kind(), TypeCheckErrorKind::Effect { .. })
                && error
                    .message()
                    .contains("flow.untyped_destructured_record_callback_effect")
                && error.message().contains("fs.read")
        }),
        "expected untyped record destructured callback effect diagnostic, got {errors:?}"
    );
}

#[test]
fn option_variant_destructured_callback_composes_when_payload_is_called() {
    let tree = parse_ok(
        r#"
fn use_loader(.Some(load): Option<String -> String>) -> String {
    return load("story.arcw")
}

flow @flow.option_variant_destructured_callback_effect option_variant_destructured_callback_effect
effects { }
{
    let body = use_loader(Some(|path: String| -> String {
        adapter.read_text(path = path)
    }))
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("option variant destructured fixture lowers");
    validate_typecheck_ready(&hir).expect("option variant destructured fixture is structured");

    let errors = typecheck_hir(&hir, &read_text_env())
        .expect_err("option variant destructured callback must compose body effects");
    assert!(
        errors.iter().any(|error| {
            matches!(error.kind(), TypeCheckErrorKind::Effect { .. })
                && error
                    .message()
                    .contains("flow.option_variant_destructured_callback_effect")
                && error.message().contains("fs.read")
        }),
        "expected option variant destructured callback effect diagnostic, got {errors:?}"
    );
}

#[test]
fn result_err_variant_destructured_callback_composes_when_payload_is_called() {
    let tree = parse_ok(
        r#"
fn use_loader(.Err(load): Result<String, String -> String>) -> String {
    return load("story.arcw")
}

flow @flow.result_err_variant_destructured_callback_effect result_err_variant_destructured_callback_effect
effects { }
{
    let body = use_loader(Err(|path: String| -> String {
        adapter.read_text(path = path)
    }))
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("result variant destructured fixture lowers");
    validate_typecheck_ready(&hir).expect("result variant destructured fixture is structured");

    let errors = typecheck_hir(&hir, &read_text_env())
        .expect_err("result variant destructured callback must compose body effects");
    assert!(
        errors.iter().any(|error| {
            matches!(error.kind(), TypeCheckErrorKind::Effect { .. })
                && error
                    .message()
                    .contains("flow.result_err_variant_destructured_callback_effect")
                && error.message().contains("fs.read")
        }),
        "expected result variant destructured callback effect diagnostic, got {errors:?}"
    );
}

#[test]
fn user_enum_tuple_variant_destructured_callback_composes_when_payload_is_called() {
    let tree = parse_ok(
        r#"
enum LoaderSpec {
    WithLoad(String -> String),
}

fn use_loader(.WithLoad(load): LoaderSpec) -> String {
    return load("story.arcw")
}

flow @flow.user_enum_tuple_variant_destructured_callback_effect user_enum_tuple_variant_destructured_callback_effect
effects { }
{
    let body = use_loader(LoaderSpec.WithLoad(|path: String| -> String {
        adapter.read_text(path = path)
    }))
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("user enum tuple variant fixture lowers");
    validate_typecheck_ready(&hir).expect("user enum tuple variant fixture is structured");

    let errors = typecheck_hir(&hir, &read_text_env())
        .expect_err("user enum tuple variant destructured callback must compose body effects");
    assert!(
        errors.iter().any(|error| {
            matches!(error.kind(), TypeCheckErrorKind::Effect { .. })
                && error
                    .message()
                    .contains("flow.user_enum_tuple_variant_destructured_callback_effect")
                && error.message().contains("fs.read")
        }),
        "expected user enum tuple variant callback effect diagnostic, got {errors:?}"
    );
}

#[test]
fn user_enum_record_variant_destructured_callback_composes_when_payload_is_called() {
    let tree = parse_ok(
        r#"
enum LoaderRecordSpec {
    WithLoad { load: String -> String },
}

fn use_loader(.WithLoad { load }: LoaderRecordSpec) -> String {
    return load("story.arcw")
}

flow @flow.user_enum_record_variant_destructured_callback_effect user_enum_record_variant_destructured_callback_effect
effects { }
{
    let body = use_loader(WithLoad { load: |path: String| -> String { adapter.read_text(path = path) } })
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("user enum record variant fixture lowers");
    validate_typecheck_ready(&hir).expect("user enum record variant fixture is structured");

    let errors = typecheck_hir(&hir, &read_text_env())
        .expect_err("user enum record variant destructured callback must compose body effects");
    assert!(
        errors.iter().any(|error| {
            matches!(error.kind(), TypeCheckErrorKind::Effect { .. })
                && error
                    .message()
                    .contains("flow.user_enum_record_variant_destructured_callback_effect")
                && error.message().contains("fs.read")
        }),
        "expected user enum record variant callback effect diagnostic, got {errors:?}"
    );
}

#[test]
fn env_enum_tuple_variant_destructured_callback_composes_when_payload_is_called() {
    let tree = parse_ok(
        r#"
fn use_loader(.WithLoad(load): ExternalLoaderSpec) -> String {
    return load("story.arcw")
}

flow @flow.env_enum_tuple_variant_destructured_callback_effect env_enum_tuple_variant_destructured_callback_effect
effects { }
{
    let body = use_loader(ExternalLoaderSpec.WithLoad(|path: String| -> String {
        adapter.read_text(path = path)
    }))
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("env enum tuple variant fixture lowers");
    validate_typecheck_ready(&hir).expect("env enum tuple variant fixture is structured");
    let env = read_text_env().with_enum_variant_payload(
        TypeKind::Named("ExternalLoaderSpec".to_owned()),
        "WithLoad",
        EnumVariantPayload::tuple([TypeKind::function([TypeKind::String], TypeKind::String)]),
    );

    let errors = typecheck_hir(&hir, &env)
        .expect_err("env enum tuple variant destructured callback must compose body effects");
    assert!(
        errors.iter().any(|error| {
            matches!(error.kind(), TypeCheckErrorKind::Effect { .. })
                && error
                    .message()
                    .contains("flow.env_enum_tuple_variant_destructured_callback_effect")
                && error.message().contains("fs.read")
        }),
        "expected env enum tuple variant callback effect diagnostic, got {errors:?}"
    );
}

#[test]
fn env_enum_record_variant_destructured_callback_composes_when_payload_is_called() {
    let tree = parse_ok(
        r#"
fn use_loader(.WithLoad { load }: ExternalLoaderRecordSpec) -> String {
    return load("story.arcw")
}

flow @flow.env_enum_record_variant_destructured_callback_effect env_enum_record_variant_destructured_callback_effect
effects { }
{
    let body = use_loader(WithLoad { load: |path: String| -> String { adapter.read_text(path = path) } })
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("env enum record variant fixture lowers");
    validate_typecheck_ready(&hir).expect("env enum record variant fixture is structured");
    let env = read_text_env().with_enum_variant_payload(
        TypeKind::Named("ExternalLoaderRecordSpec".to_owned()),
        "WithLoad",
        EnumVariantPayload::record([(
            "load",
            TypeKind::function([TypeKind::String], TypeKind::String),
        )]),
    );

    let errors = typecheck_hir(&hir, &env)
        .expect_err("env enum record variant destructured callback must compose body effects");
    assert!(
        errors.iter().any(|error| {
            matches!(error.kind(), TypeCheckErrorKind::Effect { .. })
                && error
                    .message()
                    .contains("flow.env_enum_record_variant_destructured_callback_effect")
                && error.message().contains("fs.read")
        }),
        "expected env enum record variant callback effect diagnostic, got {errors:?}"
    );
}

#[test]
fn curried_higher_order_function_argument_composes_when_later_group_param_is_called() {
    let tree = parse_ok(
        r#"
fn use_loader(path: String)(load: String -> String) -> String {
    return load(path)
}

flow @flow.curried_higher_order_closure_effect curried_higher_order_closure_effect
effects { }
{
    let body = use_loader("story.arcw")(|path: String| -> String {
        adapter.read_text(path = path)
    })
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("curried higher-order effect fixture lowers");
    validate_typecheck_ready(&hir).expect("curried higher-order effect fixture is structured");

    let errors = typecheck_hir(&hir, &read_text_env())
        .expect_err("curried callback invocation must compose body effects into the caller");
    assert!(
        errors.iter().any(|error| {
            matches!(error.kind(), TypeCheckErrorKind::Effect { .. })
                && error
                    .message()
                    .contains("flow.curried_higher_order_closure_effect")
                && error.message().contains("fs.read")
        }),
        "expected curried higher-order callback effect diagnostic, got {errors:?}"
    );
}

#[test]
fn curried_higher_order_function_alias_composes_when_later_group_param_is_called() {
    let tree = parse_ok(
        r#"
fn use_loader(path: String)(load: String -> String) -> String {
    return load(path)
}

flow @flow.curried_higher_order_alias_effect curried_higher_order_alias_effect
effects { }
{
    let stage = use_loader("story.arcw")
    let body = stage(|path: String| -> String {
        adapter.read_text(path = path)
    })
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("curried alias higher-order fixture lowers");
    validate_typecheck_ready(&hir).expect("curried alias higher-order fixture is structured");

    let errors = typecheck_hir(&hir, &read_text_env())
        .expect_err("curried callback alias invocation must compose body effects");
    assert!(
        errors.iter().any(|error| {
            matches!(error.kind(), TypeCheckErrorKind::Effect { .. })
                && error
                    .message()
                    .contains("flow.curried_higher_order_alias_effect")
                && error.message().contains("fs.read")
        }),
        "expected curried alias callback effect diagnostic, got {errors:?}"
    );
}

#[test]
fn partial_curried_higher_order_callback_does_not_compose_until_final_call() {
    let tree = parse_ok(
        r#"
fn use_loader(path: String)(load: String -> String, suffix: String) -> String {
    return load(path)
}

flow @flow.partial_curried_higher_order_creation partial_curried_higher_order_creation
effects { }
{
    let stage = use_loader("story.arcw")
    let partial = stage(|path: String| -> String {
        adapter.read_text(path = path)
    })
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("partial curried creation fixture lowers");
    validate_typecheck_ready(&hir).expect("partial curried creation fixture is structured");

    let report = analyze_types(&hir, &read_text_env());
    assert!(
        report.diagnostics.is_empty(),
        "partial curried callback creation should not compose effects: {:?}",
        report.diagnostics
    );
    let summary = report
        .effects
        .summary(&crate::effect_model::CallableId::new(
            "flow.partial_curried_higher_order_creation",
        ))
        .expect("flow summary");
    assert!(
        summary.inferred().is_empty(),
        "flow should not infer partial curried callback effects at creation: {summary:?}"
    );
}

#[test]
fn partial_curried_higher_order_callback_composes_on_final_call() {
    let tree = parse_ok(
        r#"
fn use_loader(path: String)(load: String -> String, suffix: String) -> String {
    return load(path)
}

flow @flow.partial_curried_higher_order_call partial_curried_higher_order_call
effects { }
{
    let stage = use_loader("story.arcw")
    let partial = stage(|path: String| -> String {
        adapter.read_text(path = path)
    })
    let body = partial(".bak")
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("partial curried call fixture lowers");
    validate_typecheck_ready(&hir).expect("partial curried call fixture is structured");

    let errors = typecheck_hir(&hir, &read_text_env())
        .expect_err("final partial curried call must compose callback body effects");
    assert!(
        errors.iter().any(|error| {
            matches!(error.kind(), TypeCheckErrorKind::Effect { .. })
                && error
                    .message()
                    .contains("flow.partial_curried_higher_order_call")
                && error.message().contains("fs.read")
        }),
        "expected partial curried callback effect diagnostic, got {errors:?}"
    );
}

#[test]
fn no_effect_rejects_partial_curried_higher_order_callback_on_final_call() {
    let tree = parse_ok(
        r#"
fn use_loader(path: String)(load: String -> String, suffix: String) -> String {
    return load(path)
}

flow @flow.no_effect_partial_curried_higher_order_call no_effect_partial_curried_higher_order_call
effects { fs.read }
ensures no_effect fs.read
{
    let stage = use_loader("story.arcw")
    let partial = stage(|path: String| -> String {
        adapter.read_text(path = path)
    })
    let body = partial(".bak")
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("no-effect partial curried call fixture lowers");
    validate_typecheck_ready(&hir).expect("no-effect partial curried call fixture is structured");

    let errors = typecheck_hir(&hir, &read_text_env())
        .expect_err("no_effect must reject partial curried callback effects at final call");
    assert!(
        errors.iter().any(|error| {
            matches!(error.kind(), TypeCheckErrorKind::Effect { .. })
                && error
                    .message()
                    .contains("flow.no_effect_partial_curried_higher_order_call")
                && error.message().contains("forbids effect `fs.read`")
        }),
        "expected no_effect partial curried callback diagnostic, got {errors:?}"
    );
}

#[test]
fn partial_curried_higher_order_callback_composes_on_immediate_final_call() {
    let tree = parse_ok(
        r#"
fn use_loader(path: String)(load: String -> String, suffix: String) -> String {
    return load(path)
}

flow @flow.partial_curried_higher_order_immediate_call partial_curried_higher_order_immediate_call
effects { }
{
    let stage = use_loader("story.arcw")
    let body = stage(|path: String| -> String {
        adapter.read_text(path = path)
    })(".bak")
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("immediate partial curried call fixture lowers");
    validate_typecheck_ready(&hir).expect("immediate partial curried call fixture is structured");

    let errors = typecheck_hir(&hir, &read_text_env())
        .expect_err("immediate final partial curried call must compose callback body effects");
    assert!(
        errors.iter().any(|error| {
            matches!(error.kind(), TypeCheckErrorKind::Effect { .. })
                && error
                    .message()
                    .contains("flow.partial_curried_higher_order_immediate_call")
                && error.message().contains("fs.read")
        }),
        "expected immediate partial curried callback effect diagnostic, got {errors:?}"
    );
}

#[test]
fn curried_source_returned_closure_keeps_delayed_effect_proxy() {
    let tree = parse_ok(
        r#"
fn make_loader(prefix: String)(suffix: String) -> (String -> String) {
    return |path: String| -> String {
        adapter.read_text(path = path)
    }
}

flow @flow.curried_returned_closure_stage curried_returned_closure_stage
effects { }
{
    let loader = make_loader("assets")(".arcw")
}

flow @flow.curried_returned_closure_call curried_returned_closure_call
effects { }
{
    let loader = make_loader("assets")(".arcw")
    let body = loader("story")
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("curried returned closure fixture lowers");
    validate_typecheck_ready(&hir).expect("curried returned closure fixture is structured");

    let report = analyze_types(&hir, &read_text_env());
    let effect = crate::effects::EffectId::parse("fs.read").expect("valid effect");
    let stage = report
        .effects
        .summary(&crate::effect_model::CallableId::new(
            "flow.curried_returned_closure_stage",
        ))
        .expect("returned closure stage summary");
    assert!(stage.inferred().is_empty());
    let call = report
        .effects
        .summary(&crate::effect_model::CallableId::new(
            "flow.curried_returned_closure_call",
        ))
        .expect("returned closure invocation summary");
    assert!(call.inferred().contains(&effect));
}
