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
