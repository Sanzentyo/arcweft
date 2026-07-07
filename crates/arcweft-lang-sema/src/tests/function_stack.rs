use super::support::*;

#[test]
fn records_function_value_call_lowering_evidence() {
    let tree = parse_ok(
        r"
flow @flow.call_value call_value {
    let ok: bool = f(1i64)
    log.info(ok)
}
",
    );
    let hir = lower_to_hir(&tree).expect("function value call fixture lowers");
    validate_typecheck_ready(&hir).expect("function value call fixture is structured");
    let env = TypeCheckEnv::new().with_symbol(
        "f",
        TypeKind::Function {
            params: vec![TypeKind::I64],
            return_type: Box::new(TypeKind::Bool),
        },
    );

    let report = analyze_types(&hir, &env);
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
    let evidence = report
        .typed_lowering_evidence
        .iter()
        .find(|evidence| {
            matches!(
                &evidence.kind,
                TypedLoweringEvidenceKind::FunctionValueCall {
                    callee: Some(callee),
                    callee_ty,
                    result_ty: TypeKind::Bool,
                    arg_count: 1
                } if callee == "f" && callee_ty.function_arity() == Some(1)
            )
        })
        .expect("function value call evidence is recorded");
    assert!(
        report.judgments.iter().any(|judgment| {
            matches!(
                &judgment.subject,
                TypeJudgmentSubject::Expr { id, kind: "call" }
                    if *id == evidence.expression_id && judgment.ty == TypeKind::Bool
            )
        }),
        "function value call evidence is keyed to the call expression judgment"
    );
}

#[test]
fn typechecks_partial_function_value_application() {
    let tree = parse_ok(
        r"
flow @flow.partial_call partial_call {
    let add_one = f(1i64)
    let ok: bool = add_one(2i64)
    log.info(ok)
}
",
    );
    let hir = lower_to_hir(&tree).expect("partial function value fixture lowers");
    validate_typecheck_ready(&hir).expect("partial function value fixture is structured");
    let env = TypeCheckEnv::new().with_symbol(
        "f",
        TypeKind::Function {
            params: vec![TypeKind::I64, TypeKind::I64],
            return_type: Box::new(TypeKind::Bool),
        },
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
                TypedLoweringEvidenceKind::FunctionValueCall {
                    callee: Some(callee),
                    result_ty,
                    arg_count: 1,
                    ..
                } if callee == "f" && result_ty.function_arity() == Some(1)
            )
        }),
        "expected first function value call to return the remaining function"
    );
    assert!(
        report.typed_lowering_evidence.iter().any(|evidence| {
            matches!(
                &evidence.kind,
                TypedLoweringEvidenceKind::FunctionValueCall {
                    callee: Some(callee),
                    result_ty: TypeKind::Bool,
                    arg_count: 1,
                    ..
                } if callee == "add_one"
            )
        }),
        "expected local function value call to return the final result type"
    );
}

#[test]
fn top_level_function_path_typechecks_as_function_value() {
    let tree = parse_ok(
        r"
fn add(lhs: i64, rhs: i64) -> i64 {
    return lhs + rhs
}

flow @flow.local_function_alias local_function_alias {
    let f = add
    let add_two = f(2i64)
    let seven: i64 = add_two(5i64)
    log.info(seven)
}
",
    );
    let hir = lower_to_hir(&tree).expect("top-level function value fixture lowers");
    validate_typecheck_ready(&hir).expect("top-level function value fixture is structured");

    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
    assert!(report.judgments.iter().any(|judgment| {
        matches!(
            &judgment.ty,
            TypeKind::Function {
                params,
                return_type,
            } if params.as_slice() == [TypeKind::I64, TypeKind::I64]
                && return_type.as_ref() == &TypeKind::I64
        )
    }));
    assert!(
        report.typed_lowering_evidence.iter().any(|evidence| {
            matches!(
                &evidence.kind,
                TypedLoweringEvidenceKind::FunctionValueCall {
                    callee: Some(callee),
                    result_ty,
                    arg_count: 1,
                    ..
                } if callee == "f" && result_ty.function_arity() == Some(1)
            )
        }),
        "expected call through top-level function alias to record function-value evidence"
    );
}

#[test]
fn data_last_pipe_through_local_function_value_records_call_evidence() {
    let tree = parse_ok(
        r"
#[pure]
fn add(lhs: i64, rhs: i64) -> i64 {
    return lhs + rhs
}

flow @flow.local_pipe local_pipe {
    let f = add
    let partial: i64 -> i64 = 2i64 |> f
    let exact: i64 = 2i64 |> f(1i64)
    log.info(exact)
}
",
    );
    let hir = lower_to_hir(&tree).expect("local pipe fixture lowers");
    validate_typecheck_ready(&hir).expect("local pipe fixture is structured");

    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
    assert!(
        report.typed_lowering_evidence.iter().any(|evidence| {
            matches!(
                &evidence.kind,
                TypedLoweringEvidenceKind::FunctionValueCall {
                    callee: Some(callee),
                    result_ty,
                    arg_count: 1,
                    ..
                } if callee == "f" && result_ty.function_arity() == Some(1)
            )
        }),
        "expected bare pipe RHS through local function to record partial function-value evidence"
    );
    assert!(
        report.typed_lowering_evidence.iter().any(|evidence| {
            matches!(
                &evidence.kind,
                TypedLoweringEvidenceKind::FunctionValueCall {
                    callee: Some(callee),
                    result_ty: TypeKind::I64,
                    arg_count: 2,
                    ..
                } if callee == "f"
            )
        }),
        "expected call pipe RHS through local function to record exact function-value evidence"
    );
}

#[test]
fn curried_function_declaration_preserves_call_group_semantics() {
    let tree = parse_ok(
        r"
fn add(a: i64)(b: i64) -> i64 {
    return a + b
}

flow @flow.curried curried {
    let add_one: i64 -> i64 = add(1i64)
    let three: i64 = add(1i64)(2i64)
    log.info(three)
}
",
    );
    let hir = lower_to_hir(&tree).expect("curried function fixture lowers");
    validate_typecheck_ready(&hir).expect("curried function fixture is structured");

    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
    assert!(
        report.judgments.iter().any(|judgment| {
            matches!(
                &judgment.subject,
                TypeJudgmentSubject::Expr { kind: "call", .. }
                    if matches!(
                        &judgment.ty,
                        TypeKind::Function { params, return_type }
                            if params == &[TypeKind::I64]
                                && return_type.as_ref() == &TypeKind::I64
                    )
            )
        }),
        "add(1i64) should typecheck as the remaining call group function"
    );
}

#[test]
fn pure_function_prefix_call_typechecks_as_partial_application() {
    let tree = parse_ok(
        r"
#[pure]
fn add(lhs: i64, rhs: i64) -> i64 {
    return lhs + rhs
}

flow @flow.partial_pure partial_pure {
    let add_two: i64 -> i64 = add(2i64)
    let via_pipe: i64 -> i64 = 2i64 |> add
    log.info(add_two)
}
",
    );
    let hir = lower_to_hir(&tree).expect("pure partial fixture lowers");
    validate_typecheck_ready(&hir).expect("pure partial fixture is structured");

    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
    let function_judgments = report
        .judgments
        .iter()
        .filter(|judgment| {
            matches!(
                &judgment.ty,
                TypeKind::Function { params, return_type }
                    if params == &[TypeKind::I64] && return_type.as_ref() == &TypeKind::I64
            )
        })
        .count();
    assert!(
        function_judgments >= 2,
        "expected add(2i64) and 2i64 |> add to typecheck as partial applications: {:?}",
        report.judgments
    );
}

#[test]
fn non_pure_function_prefix_call_still_requires_full_signature() {
    let tree = parse_ok(
        r"
fn add(lhs: i64, rhs: i64) -> i64 {
    return lhs + rhs
}

flow @flow.partial_non_pure partial_non_pure {
    let add_two = add(2i64)
}
",
    );
    let hir = lower_to_hir(&tree).expect("non-pure partial fixture lowers");
    validate_typecheck_ready(&hir).expect("non-pure partial fixture is structured");

    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.iter().any(|diagnostic| diagnostic
            .message()
            .contains("function `add` missing required argument `rhs`")),
        "non-pure partial calls should remain rejected until runtime lowering is designed: {:?}",
        report.diagnostics
    );
}

#[test]
fn curried_function_declaration_handles_multi_param_groups_and_tuple_return_samples() {
    let tree = parse_ok(
        r"
fn tuple_tail(a: i64, b: i64)(c: i64) -> (i64, i64, i64) {
    return (a, b, c)
}

fn chain(a: i64)(b: i64)(c: i64, d: i64) -> i64 {
    return a + b + c + d
}

flow @flow.curried_samples curried_samples {
    let tupled: (i64, i64, i64) = tuple_tail(1i64, 2i64)(3i64)
    let sum: i64 = chain(1i64)(2i64)(3i64, 4i64)
    log.info(sum)
}
",
    );
    let hir = lower_to_hir(&tree).expect("curried sample fixture lowers");
    validate_typecheck_ready(&hir).expect("curried sample fixture is structured");

    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
    assert!(
        report.judgments.iter().any(|judgment| {
            matches!(
                &judgment.ty,
                TypeKind::Function { params, return_type }
                    if params == &[TypeKind::I64]
                        && matches!(
                            return_type.as_ref(),
                            TypeKind::Tuple(items)
                                if items == &[TypeKind::I64, TypeKind::I64, TypeKind::I64]
                        )
            )
        }),
        "tuple_tail(1i64, 2i64) should typecheck as c -> (i64, i64, i64)"
    );
    assert!(
        report.judgments.iter().any(|judgment| {
            matches!(
                &judgment.ty,
                TypeKind::Function { params, return_type }
                    if params == &[TypeKind::I64]
                        && matches!(
                            return_type.as_ref(),
                            TypeKind::Function {
                                params: final_params,
                                return_type: final_return_type,
                            }
                                if final_params == &[TypeKind::I64, TypeKind::I64]
                                    && final_return_type.as_ref() == &TypeKind::I64
                        )
            )
        }),
        "chain(1i64) should retain the remaining two call groups"
    );
    assert!(
        report.judgments.iter().any(|judgment| {
            matches!(
                &judgment.ty,
                TypeKind::Tuple(items)
                    if items == &[TypeKind::I64, TypeKind::I64, TypeKind::I64]
            )
        }),
        "tuple_tail(1i64, 2i64)(3i64) should typecheck as the tuple return"
    );
}

#[test]
fn curried_function_declaration_rejects_flattened_call_group() {
    let tree = parse_ok(
        r"
fn add(a: i64)(b: i64) -> i64 {
    return a + b
}

flow @flow.curried_flattened curried_flattened {
    let wrong = add(1i64, 2i64)
    log.info(wrong)
}
",
    );
    let hir = lower_to_hir(&tree).expect("curried flattened fixture lowers");
    validate_typecheck_ready(&hir).expect("curried flattened fixture is structured");

    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message().contains("curried parameter groups")),
        "expected flattened call-group diagnostic, got {:?}",
        report.diagnostics
    );
}

#[test]
fn curried_trait_method_preserves_call_group_semantics() {
    let tree = parse_ok(
        r"
struct Score {}

trait Threshold {
    fn above(self, min: i64)(value: i64) -> bool
}

impl Threshold for Score {
    fn above(self, min: i64)(value: i64) -> bool {
        value >= min
    }
}

flow @flow.curried_trait_method curried_trait_method {
    let predicate: i64 -> bool = score.above(80i64)
    let ok: bool = predicate(81i64)
    let direct: bool = score.above(80i64)(82i64)
    log.info(ok)
    log.info(direct)
}
",
    );
    let hir = lower_to_hir(&tree).expect("curried trait method fixture lowers");
    validate_typecheck_ready(&hir).expect("curried trait method fixture is structured");
    let env = TypeCheckEnv::new().with_symbol("score", TypeKind::Named("Score".to_owned()));

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
                TypeJudgmentSubject::Expr { kind: "method_call", .. }
                    if matches!(
                        &judgment.ty,
                        TypeKind::Function { params, return_type }
                            if params == &[TypeKind::I64]
                                && return_type.as_ref() == &TypeKind::Bool
                    )
            )
        }),
        "score.above(80i64) should typecheck as the remaining call group function"
    );
}

#[test]
fn curried_trait_method_rejects_flattened_call_group() {
    let tree = parse_ok(
        r"
struct Score {}

trait Threshold {
    fn above(self, min: i64)(value: i64) -> bool
}

impl Threshold for Score {
    fn above(self, min: i64)(value: i64) -> bool {
        value >= min
    }
}

flow @flow.curried_trait_flattened curried_trait_flattened {
    let wrong = score.above(80i64, 81i64)
    log.info(wrong)
}
",
    );
    let hir = lower_to_hir(&tree).expect("curried trait flattened fixture lowers");
    validate_typecheck_ready(&hir).expect("curried trait flattened fixture is structured");
    let env = TypeCheckEnv::new().with_symbol("score", TypeKind::Named("Score".to_owned()));

    let report = analyze_types(&hir, &env);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message().contains("curried parameter groups")),
        "expected flattened trait method call-group diagnostic, got {:?}",
        report.diagnostics
    );
}

#[test]
fn closure_return_type_annotation_checks_body() {
    let tree = parse_ok(
        r"
flow @flow.closure_return closure_return {
    let is_high = |score: i64| -> bool {
        score >= 80i64
    }
    let ok: bool = is_high(81i64)
    log.info(ok)
}
",
    );
    let hir = lower_to_hir(&tree).expect("closure return fixture lowers");
    validate_typecheck_ready(&hir).expect("closure return fixture is structured");

    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
    assert!(
        report.judgments.iter().any(|judgment| {
            matches!(
                &judgment.ty,
                TypeKind::Function { params, return_type }
                    if params == &[TypeKind::I64] && return_type.as_ref() == &TypeKind::Bool
            )
        }),
        "expected closure to typecheck as i64 -> bool"
    );
}

#[test]
fn closure_tuple_pattern_parameter_binds_body_locals() {
    let tree = parse_ok(
        r"
fn closure_tuple_pattern() -> i64 {
    let sum = |(left, right): (i64, i64)| -> i64 {
        left + right
    }
    sum((1i64, 2i64))
}
",
    );
    let hir = lower_to_hir(&tree).expect("closure tuple pattern fixture lowers");
    validate_typecheck_ready(&hir).expect("closure tuple pattern fixture is structured");

    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
    assert!(
        report.judgments.iter().any(|judgment| {
            matches!(
                &judgment.ty,
                TypeKind::Function { params, return_type }
                    if params
                        == &[TypeKind::Tuple(vec![TypeKind::I64, TypeKind::I64])]
                        && return_type.as_ref() == &TypeKind::I64
            )
        }),
        "expected closure to typecheck as (i64, i64) tuple parameter -> i64"
    );
}

#[test]
fn closure_discard_parameter_does_not_require_binding_name() {
    let tree = parse_ok(
        r"
fn closure_discard_parameter() -> i64 {
    let always_one = |_: i64| -> i64 {
        1i64
    }
    always_one(2i64)
}
",
    );
    let hir = lower_to_hir(&tree).expect("closure discard fixture lowers");
    validate_typecheck_ready(&hir).expect("closure discard fixture is structured");

    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
}

#[test]
fn closure_capture_inventory_records_immutable_local_capture() {
    let tree = parse_ok(
        r"
flow @flow.closure_capture_inventory closure_capture_inventory {
    let limit: i64 = 80i64
    let is_high = |score: i64| -> bool {
        score >= limit
    }
    let ok: bool = is_high(81i64)
    log.info(ok)
}
",
    );
    let hir = lower_to_hir(&tree).expect("closure capture fixture lowers");
    validate_typecheck_ready(&hir).expect("closure capture fixture is structured");

    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
    let inventory = report
        .closure_captures
        .iter()
        .find(|inventory| {
            inventory
                .captures
                .iter()
                .any(|capture| capture.name == "limit")
        })
        .expect("closure capture inventory records the outer local");
    assert!(
        inventory
            .captures
            .iter()
            .any(|capture| capture.name == "limit" && capture.ty == TypeKind::I64),
        "expected `limit` to be captured as i64, got {:?}",
        inventory.captures
    );
    assert!(
        inventory
            .captures
            .iter()
            .all(|capture| capture.name != "score"),
        "closure parameter must not be reported as a capture: {:?}",
        inventory.captures
    );
}

#[test]
fn inferred_closure_body_reports_numeric_fallback_warning() {
    let tree = parse_ok(
        r"
flow @flow.closure_numeric_fallback closure_numeric_fallback {
    let fallback = || 1
    log.info(fallback)
}
",
    );
    let hir = lower_to_hir(&tree).expect("closure numeric fallback fixture lowers");
    validate_typecheck_ready(&hir).expect("closure numeric fallback fixture is structured");

    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
    assert!(
        report.warnings.iter().any(|warning| {
            matches!(
                warning.kind(),
                TypeCheckWarningKind::NumericFallbackInInferredClosure {
                    literal_kind,
                    fallback: TypeKind::I32
                } if literal_kind == "integer"
            ) && warning.stable_code() == "sema.numeric.fallback_in_inferred_closure"
        }),
        "expected inferred closure numeric fallback warning, got {:?}",
        report.warnings
    );
}

#[test]
fn explicit_closure_return_type_suppresses_numeric_fallback_warning() {
    let tree = parse_ok(
        r"
flow @flow.closure_numeric_explicit closure_numeric_explicit {
    let explicit = || -> i64 {
        1
    }
    log.info(explicit)
}
",
    );
    let hir = lower_to_hir(&tree).expect("explicit closure numeric fixture lowers");
    validate_typecheck_ready(&hir).expect("explicit closure numeric fixture is structured");

    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
    assert!(
        report.warnings.iter().all(|warning| !matches!(
            warning.kind(),
            TypeCheckWarningKind::NumericFallbackInInferredClosure { .. }
        )),
        "explicit closure return type should suppress fallback warning: {:?}",
        report.warnings
    );
}

#[test]
fn closure_borrow_capture_rejects_await_boundary_crossing() {
    let tree = parse_ok(
        r"
flow @flow.closure_borrow_capture closure_borrow_capture {
    let pixels: &'asset [Rgba8] = bg.pixels()
    let bad = || -> Unit {
        let loaded = await load_avatar()
        log.info(pixels)
    }
    log.info(bad)
}
",
    );
    let hir = lower_to_hir(&tree).expect("closure borrowed capture fixture lowers");
    validate_typecheck_ready(&hir).expect("closure borrowed capture fixture is structured");

    let report = analyze_types(&hir, &borrow_capture_env());
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
        "expected borrowed closure capture await diagnostic, got {:?}",
        report.diagnostics
    );
}

#[test]
fn closure_non_borrow_capture_may_cross_await_boundary() {
    let tree = parse_ok(
        r"
flow @flow.closure_value_capture closure_value_capture {
    let limit: i64 = 80i64
    let ok = || -> Unit {
        let loaded = await load_avatar()
        log.info(limit)
    }
    log.info(ok)
}
",
    );
    let hir = lower_to_hir(&tree).expect("closure value capture fixture lowers");
    validate_typecheck_ready(&hir).expect("closure value capture fixture is structured");
    let env = TypeCheckEnv::new().with_function("load_avatar", load_avatar_need_ty());

    let report = analyze_types(&hir, &env);
    assert!(
        report
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message().contains("closure capture `limit`")),
        "non-borrowed closure capture must not be rejected: {:?}",
        report.diagnostics
    );
}

#[test]
fn closure_borrow_capture_reports_thread_and_defer_boundaries() {
    let tree = parse_ok(
        r"
flow @flow.closure_borrow_capture_boundaries closure_borrow_capture_boundaries {
    let pixels: &'asset [Rgba8] = bg.pixels()
    let bad = || -> Unit {
        thread worker { log.info(pixels) }
        defer { log.info(pixels) }
    }
    log.info(bad)
}
",
    );
    let hir = lower_to_hir(&tree).expect("closure borrowed boundary fixture lowers");
    validate_typecheck_ready(&hir).expect("closure borrowed boundary fixture is structured");

    let report = analyze_types(&hir, &borrow_capture_env());
    for boundary in ["thread boundary", "defer cleanup boundary"] {
        assert!(
            report.diagnostics.iter().any(|diagnostic| {
                matches!(
                    diagnostic.kind(),
                    TypeCheckErrorKind::BorrowedClosureCaptureCrossesBoundary {
                        capture,
                        boundary: actual_boundary,
                        ..
                    } if capture == "pixels" && actual_boundary == boundary
                )
            }),
            "expected borrowed closure capture diagnostic for {boundary}, got {:?}",
            report.diagnostics
        );
    }
}

#[test]
fn closure_borrow_capture_reports_yield_boundary() {
    let tree = parse_ok(
        r"
flow @flow.closure_borrow_capture_yield closure_borrow_capture_yield {
    let pixels: &'asset [Rgba8] = bg.pixels()
    let bad = || -> Seq<Frame> {
        seq {
            yield frame
            log.info(pixels)
        }
    }
    log.info(bad)
}
",
    );
    let hir = lower_to_hir(&tree).expect("closure borrowed yield fixture lowers");
    validate_typecheck_ready(&hir).expect("closure borrowed yield fixture is structured");

    let report = analyze_types(
        &hir,
        &borrow_capture_env().with_symbol("frame", TypeKind::Named("Frame".to_owned())),
    );
    assert!(
        report.diagnostics.iter().any(|diagnostic| {
            matches!(
                diagnostic.kind(),
                TypeCheckErrorKind::BorrowedClosureCaptureCrossesBoundary {
                    capture,
                    boundary,
                    ..
                } if capture == "pixels" && boundary == "yield suspension boundary"
            ) && diagnostic.stable_code()
                == "sema.typecheck.borrowed_closure_capture_crosses_boundary"
        }),
        "expected borrowed closure capture diagnostic for yield, got {:?}",
        report.diagnostics
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
fn closure_return_type_annotation_rejects_body_mismatch() {
    let tree = parse_ok(
        r"
flow @flow.closure_return_mismatch closure_return_mismatch {
    let bad = |score: i64| -> bool {
        score + 1i64
    }
    log.info(bad(1i64))
}
",
    );
    let hir = lower_to_hir(&tree).expect("closure mismatch fixture lowers");
    validate_typecheck_ready(&hir).expect("closure mismatch fixture is structured");

    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.iter().any(|diagnostic| diagnostic
            .message()
            .contains("closure body must return bool, found i64")),
        "expected closure return mismatch diagnostic, got {:?}",
        report.diagnostics
    );
}

fn borrow_capture_env() -> TypeCheckEnv {
    TypeCheckEnv::new()
        .with_symbol("bg", TypeKind::Named("ImageHandle".to_owned()))
        .with_method(
            TypeKind::Named("ImageHandle".to_owned()),
            "pixels",
            pixel_borrow_ty(),
        )
        .with_function("load_avatar", load_avatar_need_ty())
}

fn pixel_borrow_ty() -> TypeKind {
    TypeKind::BorrowRef {
        lifetime: Some(LifetimeScopeKind::Named("asset".to_owned())),
        inner: Box::new(TypeKind::Slice(Box::new(TypeKind::Named(
            "Rgba8".to_owned(),
        )))),
    }
}

fn load_avatar_need_ty() -> TypeKind {
    TypeKind::Need {
        ready: Box::new(TypeKind::Unit),
        error: Box::new(TypeKind::Named("AssetError".to_owned())),
    }
}

fn read_text_env() -> TypeCheckEnv {
    TypeCheckEnv::new()
        .with_function_signature(
            "adapter.read_text",
            FunctionSignature::new(
                TypeKind::String,
                [FunctionParam::required("path", TypeKind::String)],
            ),
        )
        .with_function_effects("adapter.read_text", ["fs.read".to_owned()])
}

#[test]
fn closure_return_statement_uses_nearest_closure_boundary() {
    let tree = parse_ok(
        r"
fn closure_boundary() -> i64 {
    let returns_bool = || -> bool {
        return true
    }
    return 1i64
}
",
    );
    let hir = lower_to_hir(&tree).expect("closure boundary fixture lowers");
    validate_typecheck_ready(&hir).expect("closure boundary fixture is structured");

    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
}

#[test]
fn closure_return_statement_checks_declared_return_type() {
    let tree = parse_ok(
        r"
flow @flow.closure_return_statement_mismatch closure_return_statement_mismatch {
    let bad = || -> bool {
        return 1i64
    }
    log.info(bad())
}
",
    );
    let hir = lower_to_hir(&tree).expect("closure return statement fixture lowers");
    validate_typecheck_ready(&hir).expect("closure return statement fixture is structured");

    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.iter().any(|diagnostic| diagnostic
            .message()
            .contains("return value must have type bool, found i64")),
        "expected closure return statement mismatch diagnostic, got {:?}",
        report.diagnostics
    );
}

#[test]
fn curried_closure_return_type_annotation_preserves_remaining_function() {
    let tree = parse_ok(
        r"
flow @flow.curried_closure_return curried_closure_return {
    let at_least = |min: i64| |value: i64| -> bool {
        value >= min
    }
    let adult = at_least(18i64)
    let ok: bool = adult(21i64)
    log.info(ok)
}
",
    );
    let hir = lower_to_hir(&tree).expect("curried closure return fixture lowers");
    validate_typecheck_ready(&hir).expect("curried closure return fixture is structured");

    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
    assert!(
        report.judgments.iter().any(|judgment| {
            matches!(
                &judgment.ty,
                TypeKind::Function { params, return_type }
                    if params == &[TypeKind::I64]
                        && matches!(
                            return_type.as_ref(),
                            TypeKind::Function {
                                params: inner_params,
                                return_type: inner_return_type,
                            } if inner_params == &[TypeKind::I64]
                                && inner_return_type.as_ref() == &TypeKind::Bool
                        )
            )
        }),
        "expected outer closure to typecheck as i64 -> (i64 -> bool)"
    );
    let inventory = report
        .closure_captures
        .iter()
        .find(|inventory| {
            inventory
                .captures
                .iter()
                .any(|capture| capture.name == "min")
        })
        .expect("inner closure should capture the outer closure parameter");
    assert!(
        inventory
            .captures
            .iter()
            .any(|capture| capture.name == "min" && capture.ty == TypeKind::I64),
        "expected `min` to be captured as i64, got {:?}",
        inventory.captures
    );
    assert!(
        inventory
            .captures
            .iter()
            .all(|capture| capture.name != "value"),
        "inner closure parameter must not be reported as a capture: {:?}",
        inventory.captures
    );
}

#[test]
fn infers_partial_placeholder_function_without_expected_type() {
    let tree = parse_ok(
        r"
flow @flow.partial_infer partial_infer {
    let high = _ > 80i64
    log.info(high)
}
",
    );
    let hir = lower_to_hir(&tree).expect("inferred partial placeholder fixture lowers");
    validate_typecheck_ready(&hir).expect("inferred partial placeholder fixture is structured");

    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
    assert!(report.judgments.iter().any(|judgment| {
        matches!(
            &judgment.ty,
            TypeKind::Function {
                params,
                return_type,
            } if params.as_slice() == [TypeKind::I64]
                && return_type.as_ref() == &TypeKind::Bool
        )
    }));
    assert!(
        report.typed_lowering_evidence.iter().any(|evidence| {
            matches!(
                &evidence.kind,
                TypedLoweringEvidenceKind::ExpectedFunctionValue {
                    expected_ty,
                    actual_ty,
                    arity: 1
                } if expected_ty == actual_ty
                    && matches!(
                        expected_ty,
                        TypeKind::Function { params, return_type }
                            if params.as_slice() == [TypeKind::I64]
                                && return_type.as_ref() == &TypeKind::Bool
                    )
            )
        }),
        "expected inferred partial placeholder to record function lowering evidence"
    );
}

#[test]
fn infers_parenthesized_partial_placeholder_function_without_expected_type() {
    let tree = parse_ok(
        r"
flow @flow.partial_infer_grouped partial_infer_grouped {
    let high = (_ > 80i64)
    log.info(high)
}
",
    );
    let hir = lower_to_hir(&tree).expect("grouped inferred partial placeholder fixture lowers");
    validate_typecheck_ready(&hir).expect("grouped partial placeholder fixture is structured");

    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
    assert!(report.judgments.iter().any(|judgment| {
        matches!(
            &judgment.ty,
            TypeKind::Function {
                params,
                return_type,
            } if params.as_slice() == [TypeKind::I64]
                && return_type.as_ref() == &TypeKind::Bool
        )
    }));
}

#[test]
fn infers_partial_call_abstraction_without_expected_type() {
    let tree = parse_ok(
        r"
#[pure]
fn add(left: i64, right: i64) -> i64 {
    return left + right
}

flow @flow.partial_call partial_call {
    let add_one = add(_, 1i64)
    log.info(add_one)
}
",
    );
    let hir = lower_to_hir(&tree).expect("partial call fixture lowers");
    validate_typecheck_ready(&hir).expect("partial call fixture is structured");

    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
    assert!(report.judgments.iter().any(|judgment| {
        matches!(
            &judgment.ty,
            TypeKind::Function {
                params,
                return_type,
            } if params.as_slice() == [TypeKind::I64]
                && return_type.as_ref() == &TypeKind::I64
        )
    }));
}

#[test]
fn infers_repeated_partial_call_placeholders_as_one_parameter() {
    let tree = parse_ok(
        r"
#[pure]
fn add(left: i64, right: i64) -> i64 {
    return left + right
}

flow @flow.partial_call_repeated partial_call_repeated {
    let double = add(_, _)
    log.info(double)
}
",
    );
    let hir = lower_to_hir(&tree).expect("repeated partial call fixture lowers");
    validate_typecheck_ready(&hir).expect("repeated partial call fixture is structured");

    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
    assert!(report.judgments.iter().any(|judgment| {
        matches!(
            &judgment.ty,
            TypeKind::Function {
                params,
                return_type,
            } if params.as_slice() == [TypeKind::I64]
                && return_type.as_ref() == &TypeKind::I64
        )
    }));
    assert!(
        report.typed_lowering_evidence.iter().any(|evidence| {
            matches!(
                &evidence.kind,
                TypedLoweringEvidenceKind::ExpectedFunctionValue {
                    expected_ty,
                    actual_ty,
                    arity: 1
                } if expected_ty == actual_ty
                    && matches!(
                        expected_ty,
                        TypeKind::Function { params, return_type }
                            if params.as_slice() == [TypeKind::I64]
                                && return_type.as_ref() == &TypeKind::I64
                    )
            )
        }),
        "expected repeated partial placeholders to record one generated parameter"
    );
}

#[test]
fn infers_named_partial_call_placeholder_from_signature_parameter() {
    let tree = parse_ok(
        r"
#[pure]
fn add(left: i64, right: i64) -> i64 {
    return left + right
}

flow @flow.partial_call_named partial_call_named {
    let add_to_one = add(right = _, left = 1i64)
    log.info(add_to_one)
}
",
    );
    let hir = lower_to_hir(&tree).expect("named partial call fixture lowers");
    validate_typecheck_ready(&hir).expect("named partial call fixture is structured");

    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
    assert!(report.judgments.iter().any(|judgment| {
        matches!(
            &judgment.ty,
            TypeKind::Function {
                params,
                return_type,
            } if params.as_slice() == [TypeKind::I64]
                && return_type.as_ref() == &TypeKind::I64
        )
    }));
    assert!(
        report.typed_lowering_evidence.iter().any(|evidence| {
            matches!(
                &evidence.kind,
                TypedLoweringEvidenceKind::ExpectedFunctionValue {
                    expected_ty,
                    actual_ty,
                    arity: 1
                } if expected_ty == actual_ty
                    && matches!(
                        expected_ty,
                        TypeKind::Function { params, return_type }
                            if params.as_slice() == [TypeKind::I64]
                                && return_type.as_ref() == &TypeKind::I64
                    )
            )
        }),
        "expected named partial call placeholder to record one generated parameter"
    );
}

#[test]
fn pure_function_named_missing_input_typechecks_as_partial_application() {
    let tree = parse_ok(
        r"
#[pure]
fn add(left: i64, right: i64) -> i64 {
    return left + right
}

flow @flow.partial_named_missing partial_named_missing {
    let add_to_one = add(right = 1i64)
    let value: i64 = add_to_one(2i64)
    log.info(value)
}
",
    );
    let hir = lower_to_hir(&tree).expect("named missing partial fixture lowers");
    validate_typecheck_ready(&hir).expect("named missing partial fixture is structured");

    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
    assert!(report.judgments.iter().any(|judgment| {
        matches!(
            &judgment.ty,
            TypeKind::Function {
                params,
                return_type,
            } if params.as_slice() == [TypeKind::I64]
                && return_type.as_ref() == &TypeKind::I64
        )
    }));
}

#[test]
fn method_chain_falls_back_to_data_last_callable_when_no_method_matches() {
    let tree = parse_ok(
        r"
flow @flow.method_fallback method_fallback {
    let ok: bool = score.above(80i64)
    log.info(ok)
}
",
    );
    let hir = lower_to_hir(&tree).expect("method fallback fixture lowers");
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
fn method_chain_accepts_named_data_last_fallback_and_records_arg_order() {
    let tree = parse_ok(
        r"
flow @flow.method_fallback_named method_fallback_named {
    let ok: bool = score.above(min = 80i64)
    log.info(ok)
}
",
    );
    let hir = lower_to_hir(&tree).expect("named method fallback fixture lowers");
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
fn method_chain_reports_spread_data_last_fallback_as_unsupported() {
    let tree = parse_ok(
        r"
flow @flow.method_fallback_spread method_fallback_spread {
    let thresholds = [80i64]
    let wrong = score.above(thresholds...)
    log.info(wrong)
}
",
    );
    let hir = lower_to_hir(&tree).expect("spread method fallback fixture lowers");
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
fn method_chain_reports_ambiguous_data_last_fallback_candidates() {
    let tree = parse_ok(
        r"
#[pure]
fn above(min: i64, value: i64) -> bool {
    return value > min
}

flow @flow.method_fallback_ambiguous method_fallback_ambiguous {
    let wrong = score.above(80i64)
}
",
    );
    let hir = lower_to_hir(&tree).expect("ambiguous method fallback fixture lowers");
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
fn method_chain_prefers_real_method_over_data_last_callable_fallback() {
    let tree = parse_ok(
        r"
flow @flow.method_priority method_priority {
    let text: String = score.above(80i64)
    log.info(text)
}
",
    );
    let hir = lower_to_hir(&tree).expect("method priority fixture lowers");
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
    let tree = parse_ok(
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

flow @flow.method_trait_priority method_trait_priority {
    let text: String = score.above(80i64)
    log.info(text)
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("trait method priority fixture lowers");
    validate_typecheck_ready(&hir).expect("trait method priority fixture is structured");
    let env = TypeCheckEnv::new()
        .with_symbol("score", TypeKind::Named("Score".to_owned()))
        .with_function_signature(
            "above",
            FunctionSignature::new(
                TypeKind::Bool,
                [
                    FunctionParam::required("min", TypeKind::I64),
                    FunctionParam::required("value", TypeKind::Named("Score".to_owned())),
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
                    receiver: TypeKind::Named(receiver),
                    selected,
                    fallbacks
                } if method == "above"
                    && receiver == "Score"
                    && selected.contains("trait `Threshold` method `Score.above`")
                    && fallbacks.len() == 1
                    && fallbacks[0].contains("environment fn `above`")
            )
        }),
        "expected trait-method shadowed fallback warning, got {:?}",
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
        "trait method calls must not record data-last fallback evidence"
    );
}
