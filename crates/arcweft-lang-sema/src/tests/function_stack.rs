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
                TypedLoweringEvidenceKind::DataLastMethodFallback { method, arg_count }
                    if method == "above" && *arg_count == 1
            )
        }),
        "expected method fallback evidence"
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
