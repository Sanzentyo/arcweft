use super::support::*;

#[test]
fn source_function_type_effect_row_becomes_closed_semantic_row() {
    let tree = parse_ok(
        r#"
flow @flow.source_effect_row source_effect_row
effects { fs.read }
{
    let loader: String -> String effects { fs.read } = load
    let text: String = loader("avatar")
    log.info(text)
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("source function effect row fixture lowers");
    validate_typecheck_ready(&hir).expect("source function effect row fixture is structured");
    let fs_read = crate::effects::EffectSet::from_labels(["fs.read"]).expect("valid effect set");
    let env = TypeCheckEnv::new()
        .with_symbol(
            "load",
            TypeKind::function_with_effects(
                [TypeKind::String],
                TypeKind::String,
                crate::effect_row::EffectRow::closed(fs_read.clone()),
            ),
        )
        .with_function("log.info", TypeKind::Unit);

    let report = analyze_types(&hir, &env);
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
    assert!(
        report.judgments.iter().any(|judgment| {
            matches!(
                &judgment.subject,
                TypeJudgmentSubject::LetBinding { pattern } if pattern.contains("loader")
            ) && matches!(
                &judgment.ty,
                TypeKind::Function { effects, .. }
                    if effects.tail() == crate::effect_row::EffectRowTail::Closed
                        && effects.concrete() == &fs_read
            )
        }),
        "let annotation should preserve the closed source effect row"
    );
}

#[test]
fn inferred_source_function_value_uses_an_open_row_that_closes_from_its_body() {
    let tree = parse_ok(
        r"
fn load_story(path: String) -> String {
    return adapter.read_text(path = path)
}

flow @flow.source_function_value_row source_function_value_row
effects { }
{
    let loader = load_story
}
",
    );
    let hir = lower_to_hir(&tree).expect("source function open-row fixture lowers");
    validate_typecheck_ready(&hir).expect("source function open-row fixture is structured");

    let report = analyze_types(&hir, &read_text_env());
    assert!(
        report.diagnostics.is_empty(),
        "function-value creation should remain effect-free: {:?}",
        report.diagnostics
    );
    let row = report
        .judgments
        .iter()
        .find_map(|judgment| {
            let TypeJudgmentSubject::LetBinding { pattern } = &judgment.subject else {
                return None;
            };
            let TypeKind::Function { effects, .. } = &judgment.ty else {
                return None;
            };
            pattern.contains("loader").then_some(effects)
        })
        .expect("loader function row");
    assert!(matches!(
        row.tail(),
        crate::effect_row::EffectRowTail::Variable(_)
    ));
    let callable = crate::effect_model::CallableId::new("fn.load_story");
    let raw = report
        .effects
        .effect_rows()
        .summary(&callable)
        .expect("source function raw row");
    assert_eq!(raw.inferred().concrete().to_labels(), ["fs.read"]);
    assert!(matches!(
        raw.inferred().tail(),
        crate::effect_row::EffectRowTail::Variable(_)
    ));
    let closed = report
        .effects
        .closed_effect_rows()
        .expect("source function inferred row closes");
    assert_eq!(
        closed
            .summary(&callable)
            .expect("closed source function row")
            .inferred()
            .to_labels(),
        ["fs.read"]
    );
}

#[test]
fn environment_function_value_type_carries_closed_effect_row() {
    let tree = parse_ok(
        r"
flow @flow.env_function_value_effect_row env_function_value_effect_row {
    let reader = read_text
}
",
    );
    let hir = lower_to_hir(&tree).expect("environment function value fixture lowers");
    validate_typecheck_ready(&hir).expect("environment function value fixture is structured");
    let env = TypeCheckEnv::new()
        .with_function_signature(
            "read_text",
            FunctionSignature::new(
                TypeKind::String,
                [FunctionParam::required("path", TypeKind::String)],
            ),
        )
        .with_function_effects("read_text", ["fs.read".to_owned()]);

    let report = analyze_types(&hir, &env);
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
    let Some(TypeKind::Function {
        params,
        return_type,
        effects,
    }) = report.judgments.iter().find_map(|judgment| {
        matches!(
            &judgment.subject,
            TypeJudgmentSubject::LetBinding { pattern } if pattern.contains("reader")
        )
        .then_some(&judgment.ty)
    })
    else {
        panic!("expected reader binding to have function type judgment");
    };
    assert_eq!(params.as_slice(), [TypeKind::String]);
    assert_eq!(return_type.as_ref(), &TypeKind::String);
    assert_eq!(
        effects.tail(),
        crate::effect_row::EffectRowTail::Closed,
        "environment callable effects should be captured as a closed function row"
    );
    assert_eq!(effects.concrete().to_labels(), ["fs.read"]);
    assert_eq!(
        report
            .typed_lowering_evidence
            .iter()
            .find_map(|evidence| {
                let TypedLoweringEvidenceKind::FunctionValueReference { callee, ty } =
                    &evidence.kind
                else {
                    return None;
                };
                (callee == "read_text").then_some(ty.source_label())
            })
            .as_deref(),
        Some("String -> String effects { fs.read }")
    );
}

#[test]
fn closure_body_effects_do_not_leak_on_function_value_creation() {
    let tree = parse_ok(
        r#"
flow @flow.closure_effect_creation closure_effect_creation
effects { }
{
    let later = || -> String {
        adapter.read_text(path = "story.arcw")
    }
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("closure effect fixture lowers");
    validate_typecheck_ready(&hir).expect("closure effect fixture is structured");

    let report = analyze_types(&hir, &read_text_env());
    assert!(
        report.diagnostics.is_empty(),
        "closure creation should not perform the closure body effects: {:?}",
        report.diagnostics
    );
    let flow_summary = report
        .effects
        .summary(&crate::effect_model::CallableId::new(
            "flow.closure_effect_creation",
        ))
        .expect("flow summary");
    assert!(
        flow_summary.inferred().is_empty(),
        "flow should not infer closure body effects at creation: {flow_summary:?}"
    );
    assert!(
        report.effects.summaries().any(|(callable, summary)| {
            callable.as_str().starts_with("closure.expr.")
                && summary
                    .inferred()
                    .contains(&crate::effects::EffectId::parse("fs.read").expect("valid effect"))
        }),
        "closure body effect should be tracked on a synthetic closure callable: {:?}",
        report.effects
    );
}

#[test]
fn closure_effect_rows_project_closed_report_evidence() {
    let tree = parse_ok(
        r#"
flow @flow.closure_row_projection closure_row_projection
effects { }
{
    let later = || -> String {
        adapter.read_text(path = "story.arcw")
    }
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("closure row fixture lowers");
    validate_typecheck_ready(&hir).expect("closure row fixture is structured");

    let report = analyze_types(&hir, &read_text_env());
    assert!(
        report.diagnostics.is_empty(),
        "closure creation should stay effect-free for the caller: {:?}",
        report.diagnostics
    );
    let rows = report
        .effects
        .closed_effect_rows()
        .expect("closed effect-row report resolves");
    let flow_row = rows
        .summary(&crate::effect_model::CallableId::new(
            "flow.closure_row_projection",
        ))
        .expect("flow row is projected");
    assert!(
        flow_row.inferred().is_empty(),
        "closure creation must not add body effects to caller row: {flow_row:?}"
    );
    let closure_row = rows
        .summaries()
        .find(|(callable, _)| callable.as_str().starts_with("closure.expr."))
        .map(|(_, row)| row)
        .expect("closure synthetic row is projected");
    assert_eq!(
        closure_row.inferred().to_labels(),
        vec!["fs.read"],
        "closure body effects should live on the closure row"
    );
}

#[test]
fn analyzable_closure_type_and_report_use_a_resolved_open_effect_row() {
    let tree = parse_ok(
        r"
flow @flow.open_closure_row open_closure_row
effects { }
{
    let later = |path: String| -> String {
        adapter.read_text(path = path)
    }
}
",
    );
    let hir = lower_to_hir(&tree).expect("open closure row fixture lowers");
    validate_typecheck_ready(&hir).expect("open closure row fixture is structured");

    let report = analyze_types(&hir, &read_text_env());
    assert!(
        report.diagnostics.is_empty(),
        "closure creation should remain effect-free: {:?}",
        report.diagnostics
    );
    let (expression_id, effects) = report
        .judgments
        .iter()
        .find_map(|judgment| {
            let TypeJudgmentSubject::Expr {
                id,
                kind: "closure",
            } = &judgment.subject
            else {
                return None;
            };
            let TypeKind::Function { effects, .. } = &judgment.ty else {
                return None;
            };
            Some((*id, effects))
        })
        .expect("closure function judgment");
    assert!(matches!(
        effects.tail(),
        crate::effect_row::EffectRowTail::Variable(_)
    ));
    let callable = report
        .function_effect_callable_for_expression(expression_id)
        .expect("closure effect callable");
    let raw = report
        .effects
        .effect_rows()
        .summary(callable)
        .expect("raw closure effect row");
    assert!(matches!(
        raw.inferred().tail(),
        crate::effect_row::EffectRowTail::Variable(_)
    ));
    assert_eq!(raw.inferred().concrete().to_labels(), ["fs.read"]);
    let closed = report
        .effects
        .closed_effect_rows()
        .expect("inferred closure tail resolves");
    assert_eq!(
        closed
            .summary(callable)
            .expect("closed closure row")
            .inferred()
            .to_labels(),
        ["fs.read"]
    );
    let closure_type = report
        .judgments
        .iter()
        .find_map(|judgment| {
            matches!(
                judgment.subject,
                TypeJudgmentSubject::Expr {
                    id,
                    kind: "closure"
                } if id == expression_id
            )
            .then_some(&judgment.ty)
        })
        .expect("closure type judgment");
    assert_eq!(
        report
            .resolved_type(closure_type)
            .expect("closure type row resolves")
            .source_label(),
        "String -> String effects { fs.read }"
    );
}

#[test]
fn effect_analysis_report_owns_effect_row_report_boundary() {
    let tree = parse_ok(
        r#"
flow @flow.owned_row_report owned_row_report
effects { fs.read }
{
    let body = adapter.read_text(path = "story.arcw")
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("effect-row report fixture lowers");
    validate_typecheck_ready(&hir).expect("effect-row report fixture is structured");

    let report = analyze_types(&hir, &read_text_env());
    assert!(
        report.diagnostics.is_empty(),
        "declared effect row should cover the adapter call: {:?}",
        report.diagnostics
    );
    let callable = crate::effect_model::CallableId::new("flow.owned_row_report");
    let row = report
        .effects
        .effect_rows()
        .summary(&callable)
        .expect("analysis report should own a row summary for the flow");
    assert_eq!(
        row.inferred().concrete().to_labels(),
        vec!["fs.read"],
        "owned row report should preserve inferred effects before closed projection"
    );
    assert_eq!(
        row.upper_bound()
            .expect("explicit effect row should become the upper bound")
            .concrete()
            .to_labels(),
        vec!["fs.read"]
    );

    let closed = report
        .effects
        .closed_effect_rows()
        .expect("owned row report should resolve through the report's substitutions");
    let closed_row = closed.summary(&callable).expect("closed row summary");
    assert_eq!(closed_row.inferred().to_labels(), vec!["fs.read"]);
    assert_eq!(
        closed_row
            .upper_bound()
            .expect("closed upper bound")
            .to_labels(),
        vec!["fs.read"]
    );
}

#[test]
fn closure_expected_function_type_effect_row_sets_closed_upper_bound() {
    let tree = parse_ok(
        r"
flow @flow.closure_expected_row closure_expected_row
effects { }
{
    let later: String -> String effects { fs.read } =
        |path: String| -> String {
            adapter.read_text(path = path)
        }
}
",
    );
    let hir = lower_to_hir(&tree).expect("closure expected row fixture lowers");
    validate_typecheck_ready(&hir).expect("closure expected row fixture is structured");

    let report = analyze_types(&hir, &read_text_env());
    assert!(
        report.diagnostics.is_empty(),
        "expected effect row should cover closure body effects: {:?}",
        report.diagnostics
    );
    let fs_read = crate::effects::EffectSet::from_labels(["fs.read"]).expect("valid effect set");
    assert!(
        report.judgments.iter().any(|judgment| {
            matches!(
                &judgment.subject,
                TypeJudgmentSubject::Expr {
                    kind: "closure",
                    ..
                }
            ) && matches!(
                &judgment.ty,
                TypeKind::Function { effects, .. }
                    if effects.tail() == crate::effect_row::EffectRowTail::Closed
                        && effects.concrete() == &fs_read
            )
        }),
        "closure expression judgment should preserve the expected closed effect row"
    );
    let rows = report
        .effects
        .closed_effect_rows()
        .expect("closed effect rows resolve");
    let closure_row = rows
        .summaries()
        .find(|(callable, _)| callable.as_str().starts_with("closure.expr."))
        .map(|(_, row)| row)
        .expect("closure row is present");
    assert_eq!(
        closure_row.inferred().to_labels(),
        vec!["fs.read"],
        "closure inferred row should still come from the body"
    );
    assert_eq!(
        closure_row
            .upper_bound()
            .expect("expected row should become the closure upper bound")
            .to_labels(),
        vec!["fs.read"]
    );
}

#[test]
fn closure_expected_empty_effect_row_rejects_body_effect() {
    let tree = parse_ok(
        r"
flow @flow.closure_empty_expected_row closure_empty_expected_row
effects { }
{
    let later: String -> String effects { } =
        |path: String| -> String {
            adapter.read_text(path = path)
        }
}
",
    );
    let hir = lower_to_hir(&tree).expect("closure empty expected row fixture lowers");
    validate_typecheck_ready(&hir).expect("closure empty expected row fixture is structured");

    let errors = typecheck_hir(&hir, &read_text_env())
        .expect_err("closure body effects must not exceed expected empty row");
    assert!(
        errors.iter().any(|error| {
            matches!(
                error.kind(),
                TypeCheckErrorKind::Effect { diagnostic }
                    if diagnostic.code()
                        == crate::effect_diagnostics::EffectDiagnosticCode::UpperBoundExceeded
                        && diagnostic.callable().as_str().starts_with("closure.expr.")
            )
        }),
        "expected closure upper-bound diagnostic, got {errors:?}"
    );
}

#[test]
fn closure_effect_callable_evidence_joins_type_judgment_to_closed_row() {
    let tree = parse_ok(
        r"
flow @flow.closure_row_join closure_row_join
effects { }
{
    let later = |path: String| -> String {
        adapter.read_text(path = path)
    }
}
",
    );
    let hir = lower_to_hir(&tree).expect("closure effect callable fixture lowers");
    validate_typecheck_ready(&hir).expect("closure effect callable fixture is structured");

    let report = analyze_types(&hir, &read_text_env());
    assert!(
        report.diagnostics.is_empty(),
        "closure creation should stay effect-free for the caller: {:?}",
        report.diagnostics
    );
    let expression_id = report
        .judgments
        .iter()
        .find_map(|judgment| {
            let TypeJudgmentSubject::Expr {
                id,
                kind: "closure",
            } = &judgment.subject
            else {
                return None;
            };
            Some(*id)
        })
        .expect("closure expression judgment should be recorded");
    let callable = report
        .function_effect_callable_for_expression(expression_id)
        .expect("closure expression should export effect-callable evidence")
        .clone();
    assert!(
        report.judgments.iter().any(|judgment| {
            matches!(
                &judgment.subject,
                TypeJudgmentSubject::Expr {
                    id,
                    kind: "closure"
                } if *id == expression_id && matches!(judgment.ty, TypeKind::Function { .. })
            )
        }),
        "closure effect-callable evidence should be keyed to the closure expression judgment"
    );

    let rows = report
        .effects
        .closed_effect_rows()
        .expect("closed effect-row report resolves");
    let closure_row = rows
        .summary(&callable)
        .expect("closure evidence callable should have a closed row summary");
    assert_eq!(
        closure_row.inferred().to_labels(),
        vec!["fs.read"],
        "closure evidence callable should point at the closure body effect row"
    );
}

#[test]
fn borrowed_closure_capture_keeps_effect_row_evidence_at_await_boundary() {
    let tree = parse_ok(
        r#"
flow @flow.borrowed_capture_effect_row borrowed_capture_effect_row
effects { }
{
    let pixels: &'asset [Rgba8] = bg.pixels()
    let later = || -> String {
        let loaded = await load_avatar()
        log.info(pixels)
        adapter.read_text(path = "story.arcw")
    }
    log.info(later)
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("borrowed capture effect-row fixture lowers");
    validate_typecheck_ready(&hir).expect("borrowed capture effect-row fixture is structured");

    let report = analyze_types(&hir, &borrow_capture_read_text_env());
    assert!(
        report.diagnostics.iter().any(|diagnostic| {
            matches!(
                diagnostic.kind(),
                TypeCheckErrorKind::BorrowedClosureCaptureCrossesBoundary {
                    capture,
                    lifetimes,
                    boundary,
                    ..
                } if capture == "pixels"
                    && lifetimes.len() == 1
                    && lifetimes[0] == "asset"
                    && boundary == "await suspension boundary"
            ) && diagnostic.stable_code()
                == "sema.typecheck.borrowed_closure_capture_crosses_boundary"
        }),
        "expected borrowed closure capture diagnostic, got {:?}",
        report.diagnostics
    );

    let rows = report
        .effects
        .closed_effect_rows()
        .expect("closed effect-row report resolves");
    let closure_rows = rows
        .summaries()
        .filter(|(callable, _)| callable.as_str().starts_with("closure.expr."))
        .map(|(_, row)| row.inferred().to_labels())
        .collect::<Vec<_>>();
    assert!(
        closure_rows
            .iter()
            .any(|labels| labels.iter().any(|label| label == "fs.read")),
        "borrowed capture diagnostic must not drop closure effect-row evidence: {closure_rows:?}"
    );
}

#[test]
fn suspending_callable_kinds_do_not_claim_ordinary_open_rows() {
    let tree = parse_ok(
        r"
task fn task_label(prefix: String)(name: String) -> String {
    return name
}

dialogue fn dialogue_label(prefix: String)(name: String) -> String {
    return name
}

stream fn stream_values(prefix: String)(values: Stream<i64, String>) -> Stream<i64, String> {
    for value in values {
        yield value
    }
}

flow @flow.suspending_callable_values suspending_callable_values {
    let task_value = task_label
    let dialogue_value = dialogue_label
    let stream_value = stream_values
}
",
    );
    let hir = lower_to_hir(&tree).expect("suspending callable row fixture lowers");
    validate_typecheck_ready(&hir).expect("suspending callable row fixture is structured");

    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    for name in ["task_label", "dialogue_label", "stream_values"] {
        let raw = report
            .effects
            .effect_rows()
            .summary(&crate::effect_model::CallableId::new(format!("fn.{name}")))
            .expect("suspending callable row summary");
        assert_eq!(
            raw.inferred().tail(),
            crate::effect_row::EffectRowTail::Closed,
            "{name} must not claim the ordinary function open-row ABI"
        );
    }
    assert!(report.judgments.iter().any(|judgment| {
        matches!(
            &judgment.subject,
            TypeJudgmentSubject::LetBinding { pattern } if pattern.contains("task_value")
        ) && matches!(
            &judgment.ty,
            TypeKind::Function { effects, .. }
                if effects.tail() == crate::effect_row::EffectRowTail::Unknown
        )
    }));
}
