use super::support::*;

fn analyze_registered_function_stack_fixture(
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
    )
    .expect("registered function-stack fixture facts");
    let registered = crate::test_support::character_project::register(
        &project,
        &facts,
        TypeCheckEnv::standard(),
        None,
    )
    .expect("registered function-stack fixture world");
    crate::checker::analyze_registered_project_types(&project.linked_module(), &registered)
}

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
    let env =
        TypeCheckEnv::new().with_symbol("f", TypeKind::function([TypeKind::I64], TypeKind::Bool));

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
                    arg_count: 1,
                    partial: false,
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
        TypeKind::function([TypeKind::I64, TypeKind::I64], TypeKind::Bool),
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
            TypeKind::Function { params, return_type, .. } if params.as_slice() == [TypeKind::I64, TypeKind::I64]
                && return_type.as_ref() == &TypeKind::I64
        )
    }));
    assert!(
        report.typed_lowering_evidence.iter().any(|evidence| {
            matches!(
                &evidence.kind,
                TypedLoweringEvidenceKind::FunctionValueReference { callee, ty }
                    if callee == "add" && ty.function_arity() == Some(2)
            )
        }),
        "expected top-level function path to record function-value reference evidence"
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
        "expected call through top-level function alias to record function-value evidence"
    );
}

#[test]
fn data_last_pipe_through_local_function_value_records_staged_call_evidence() {
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
                    callee: None,
                    result_ty,
                    arg_count: 1,
                    partial: true,
                    ..
                } if result_ty.function_arity() == Some(1)
            )
        }),
        "expected bare pipe RHS to apply the left value as one distinct call group"
    );
    assert!(
        report.typed_lowering_evidence.iter().any(|evidence| {
            matches!(
                &evidence.kind,
                TypedLoweringEvidenceKind::FunctionValueCall {
                    callee: Some(callee),
                    result_ty,
                    arg_count: 1,
                    partial: true,
                    ..
                } if callee == "f" && result_ty.function_arity() == Some(1)
            )
        }),
        "expected f(1i64) to remain a first-stage partial call"
    );
    assert!(
        report.typed_lowering_evidence.iter().any(|evidence| {
            matches!(
                &evidence.kind,
                TypedLoweringEvidenceKind::FunctionValueCall {
                    callee: None,
                    result_ty: TypeKind::I64,
                    arg_count: 1,
                    partial: false,
                    ..
                }
            )
        }),
        "expected the pipe left value to be applied in a second, exact call group"
    );
}

#[test]
fn data_last_pipe_preserves_curried_source_call_groups() {
    let tree = parse_ok(
        r"
fn add(a: i64)(b: i64) -> i64 {
    return a + b
}

flow @flow.curried_pipe curried_pipe {
    let piped: i64 = 2i64 |> add(40i64)
    let grouped: i64 = add(40i64)(2i64)
    log.info(piped)
    log.info(grouped)
}
",
    );
    let hir = lower_to_hir(&tree).expect("curried pipe fixture lowers");
    validate_typecheck_ready(&hir).expect("curried pipe fixture is structured");

    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.is_empty(),
        "staged data-last application must preserve add(40i64)(2i64), got {:?}",
        report.diagnostics
    );
    assert!(report.typed_lowering_evidence.iter().any(|evidence| {
        matches!(
            &evidence.kind,
            TypedLoweringEvidenceKind::FunctionValueCall {
                callee: None,
                result_ty: TypeKind::I64,
                arg_count: 1,
                partial: false,
                ..
            }
        )
    }));
}

#[test]
fn data_last_pipe_does_not_merge_into_a_curried_rhs_group() {
    let tree = parse_ok(
        r"
fn add(a: i64)(b: i64) -> i64 {
    return a + b
}

flow @flow.flattened_curried_pipe flattened_curried_pipe {
    let wrong = 3i64 |> add(1i64, 2i64)
    log.info(wrong)
}
",
    );
    let hir = lower_to_hir(&tree).expect("flattened curried pipe fixture lowers");
    validate_typecheck_ready(&hir).expect("flattened curried pipe fixture is structured");

    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message().contains("curried parameter groups")),
        "RHS call groups must be checked before the pipe applies its left value: {:?}",
        report.diagnostics
    );
}

#[test]
fn pipe_left_placeholder_is_one_lexical_value_inside_closure() {
    let tree = parse_ok(
        r"
flow @flow.pipe_capture pipe_capture {
    let reuse = 3i64 |> || ^ + ^
    let six: i64 = reuse()
    log.info(six)
}
",
    );
    let hir = lower_to_hir(&tree).expect("pipe capture fixture lowers");
    validate_typecheck_ready(&hir).expect("pipe capture fixture is structured");

    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.is_empty(),
        "pipe-left value should stay in scope throughout the RHS closure: {:?}",
        report.diagnostics
    );
    assert!(
        report
            .judgments
            .iter()
            .filter(|judgment| {
                matches!(
                    &judgment.subject,
                    TypeJudgmentSubject::Expr {
                        kind: "placeholder",
                        ..
                    }
                ) && judgment.ty == TypeKind::I64
            })
            .count()
            >= 2
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
                        TypeKind::Function { params, return_type, .. }
                            if params == &[TypeKind::I64]
                                && return_type.as_ref() == &TypeKind::I64
                    )
            )
        }),
        "add(1i64) should typecheck as the remaining call group function"
    );
}

#[test]
fn curried_function_value_accepts_fixed_literal_spread_group() {
    let tree = parse_ok(
        r"
fn add(a: i64)(b: i64) -> i64 {
    return a + b
}

flow @flow.curried_spread_literal_group curried_spread_literal_group {
    let add_one = add(1i64)
    let ok: i64 = add_one([2i64]...)
}
",
    );
    let hir = lower_to_hir(&tree).expect("curried literal spread fixture lowers");
    validate_typecheck_ready(&hir).expect("curried literal spread fixture is structured");

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
                    result_ty: TypeKind::I64,
                    arg_count: 1,
                    ..
                } if callee == "add_one"
            )
        }),
        "expected inline fixed literal spread to record function-value apply evidence"
    );
}

#[test]
fn curried_function_value_rejects_later_spread_group_with_structured_diagnostic() {
    let tree = parse_ok(
        r"
fn add(a: i64)(b: i64) -> i64 {
    return a + b
}

flow @flow.curried_spread_later_group curried_spread_later_group {
    let values = [2i64]
    let add_one = add(1i64)
    let bad = add_one(values...)
}
",
    );
    let hir = lower_to_hir(&tree).expect("curried later spread fixture lowers");
    validate_typecheck_ready(&hir).expect("curried later spread fixture is structured");

    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.iter().any(|diagnostic| {
            matches!(
                diagnostic.kind(),
                TypeCheckErrorKind::UnsupportedFunctionValueCall {
                    callee: Some(callee),
                    reason
                } if callee == "add_one" && reason.contains("spread arguments")
            )
        }),
        "expected structured function-value spread diagnostic, got {:?}",
        report.diagnostics
    );
    assert!(
        report.diagnostics.iter().all(|diagnostic| !diagnostic
            .message()
            .contains("do not accept spread arguments")),
        "function-value spread should not degrade into a generic call error: {:?}",
        report.diagnostics
    );
    assert!(
        !report.typed_lowering_evidence.iter().any(|evidence| {
            matches!(
                &evidence.kind,
                TypedLoweringEvidenceKind::FunctionValueCall {
                    callee: Some(callee),
                    ..
                } if callee == "add_one"
            )
        }),
        "rejected function-value spread must not record apply lowering evidence"
    );
}

#[test]
fn curried_function_rejects_first_group_spread_partial_with_structured_diagnostic() {
    let tree = parse_ok(
        r"
fn add(a: i64)(b: i64) -> i64 {
    return a + b
}

flow @flow.curried_spread_first_group curried_spread_first_group {
    let values = [1i64]
    let add_values = add(values...)
}
",
    );
    let hir = lower_to_hir(&tree).expect("curried first-group spread fixture lowers");
    validate_typecheck_ready(&hir).expect("curried first-group spread fixture is structured");

    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.iter().any(|diagnostic| {
            matches!(
                diagnostic.kind(),
                TypeCheckErrorKind::UnsupportedSignaturePartialCall { function, reason }
                    if function == "add" && reason.contains("missing-input partial")
            )
        }),
        "expected structured curried first-group spread diagnostic, got {:?}",
        report.diagnostics
    );
    assert!(
        report.diagnostics.iter().all(|diagnostic| {
            !diagnostic.message().contains("does not accept spread")
                && !diagnostic.message().contains("missing required argument")
        }),
        "curried first-group spread should not degrade into generic call errors: {:?}",
        report.diagnostics
    );
    assert!(
        !report.typed_lowering_evidence.iter().any(|evidence| {
            matches!(
                &evidence.kind,
                TypedLoweringEvidenceKind::SignaturePartialCall { callee, .. }
                    if callee == "add"
            )
        }),
        "rejected curried first-group spread must not record partial lowering evidence"
    );
}

#[test]
fn function_value_rejects_named_arguments_with_structured_diagnostic() {
    let tree = parse_ok(
        r"
flow @flow.function_value_named_arg function_value_named_arg {
    let bad = f(value = 1i64)
}
",
    );
    let hir = lower_to_hir(&tree).expect("function-value named arg fixture lowers");
    validate_typecheck_ready(&hir).expect("function-value named arg fixture is structured");
    let env =
        TypeCheckEnv::new().with_symbol("f", TypeKind::function([TypeKind::I64], TypeKind::Bool));

    let report = analyze_types(&hir, &env);
    assert!(
        report.diagnostics.iter().any(|diagnostic| {
            matches!(
                diagnostic.kind(),
                TypeCheckErrorKind::UnsupportedFunctionValueCall {
                    callee: Some(callee),
                    reason
                } if callee == "f" && reason.contains("named argument `value`")
            )
        }),
        "expected structured function-value named-argument diagnostic, got {:?}",
        report.diagnostics
    );
    assert!(
        report.diagnostics.iter().all(|diagnostic| !diagnostic
            .message()
            .contains("do not accept named argument")),
        "function-value named arg should not degrade into a generic call error: {:?}",
        report.diagnostics
    );
    assert!(
        !report.typed_lowering_evidence.iter().any(|evidence| {
            matches!(
                &evidence.kind,
                TypedLoweringEvidenceKind::FunctionValueCall {
                    callee: Some(callee),
                    ..
                } if callee == "f"
            )
        }),
        "rejected function-value named arg must not record apply lowering evidence"
    );
}

#[test]
fn function_value_reports_arity_mismatch_with_structured_diagnostic() {
    let tree = parse_ok(
        r"
flow @flow.function_value_arity function_value_arity {
    let bad = f(1i64, 2i64)
}
",
    );
    let hir = lower_to_hir(&tree).expect("function-value arity fixture lowers");
    validate_typecheck_ready(&hir).expect("function-value arity fixture is structured");
    let env =
        TypeCheckEnv::new().with_symbol("f", TypeKind::function([TypeKind::I64], TypeKind::Bool));

    let report = analyze_types(&hir, &env);
    assert!(
        report.diagnostics.iter().any(|diagnostic| {
            matches!(
                diagnostic.kind(),
                TypeCheckErrorKind::FunctionValueArityMismatch {
                    callee: Some(callee),
                    expected: 1,
                    actual: 2
                } if callee == "f"
            ) && diagnostic.stable_code() == "sema.typecheck.function_value_arity_mismatch"
        }),
        "expected structured function-value arity diagnostic, got {:?}",
        report.diagnostics
    );
    assert!(
        !report.typed_lowering_evidence.iter().any(|evidence| {
            matches!(
                &evidence.kind,
                TypedLoweringEvidenceKind::FunctionValueCall {
                    callee: Some(callee),
                    ..
                } if callee == "f"
            )
        }),
        "rejected function-value arity mismatch must not record apply lowering evidence"
    );
}

#[test]
fn function_value_reports_argument_type_mismatch_structurally() {
    let tree = parse_ok(
        r#"
flow @flow.function_value_arg_type function_value_arg_type {
    let bad = f("wrong")
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("function-value argument fixture lowers");
    validate_typecheck_ready(&hir).expect("function-value argument fixture is structured");
    let env =
        TypeCheckEnv::new().with_symbol("f", TypeKind::function([TypeKind::I64], TypeKind::Bool));

    let report = analyze_types(&hir, &env);
    assert!(
        report.diagnostics.iter().any(|diagnostic| {
            matches!(
                diagnostic.kind(),
                TypeCheckErrorKind::ArgumentTypeMismatch {
                    function,
                    argument,
                    expected: TypeKind::I64,
                    actual: TypeKind::String,
                } if function == "function value `f`" && argument == "#0"
            ) && diagnostic.stable_code() == "sema.typecheck.argument_type_mismatch"
        }),
        "expected structured function-value argument diagnostic, got {:?}",
        report.diagnostics
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
                TypeKind::Function { params, return_type, .. }
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
fn non_annotated_function_prefix_call_typechecks_as_partial_application() {
    let tree = parse_ok(
        r"
fn add(lhs: i64, rhs: i64) -> i64 {
    return lhs + rhs
}

flow @flow.partial_non_pure partial_non_pure {
    let add_two = add(2i64)
    let seven: i64 = add_two(5i64)
}
",
    );
    let hir = lower_to_hir(&tree).expect("non-pure partial fixture lowers");
    validate_typecheck_ready(&hir).expect("non-pure partial fixture is structured");

    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
    assert!(
        report.judgments.iter().any(|judgment| matches!(
            &judgment.ty,
            TypeKind::Function { params, return_type, .. }
                if params == &[TypeKind::I64] && return_type.as_ref() == &TypeKind::I64
        )),
        "add(2i64) should typecheck as a function awaiting the missing rhs argument: {:?}",
        report.judgments
    );
    assert!(
        report
            .typed_lowering_evidence
            .iter()
            .any(|evidence| matches!(
                &evidence.kind,
                TypedLoweringEvidenceKind::SignaturePartialCall {
                    callee,
                    result_ty,
                    arg_count: 1,
                } if callee == "add"
                    && matches!(
                        result_ty,
                        TypeKind::Function { params, return_type, .. }
                            if params.as_slice() == [TypeKind::I64]
                                && return_type.as_ref() == &TypeKind::I64
                    )
            )),
        "direct top-level partial call should record runtime lowering evidence: {:?}",
        report.typed_lowering_evidence
    );
}

#[test]
fn fixed_literal_spread_signature_call_typechecks_as_exact_and_partial_application() {
    let tree = parse_ok(
        r"
fn add(left: i64, right: i64) -> i64 {
    return left + right
}

flow @flow.fixed_literal_spread_signature_call fixed_literal_spread_signature_call {
    let exact_a: i64 = add([1i64, 2i64]...)
    let exact_b: i64 = add([1i64]..., 2i64)
    let add_one = add([1i64]...)
    let three: i64 = add_one(2i64)
}
",
    );
    let hir = lower_to_hir(&tree).expect("fixed literal spread signature fixture lowers");
    validate_typecheck_ready(&hir).expect("fixed literal spread signature fixture is structured");

    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
    assert!(
        report.judgments.iter().any(|judgment| matches!(
            &judgment.ty,
            TypeKind::Function { params, return_type, .. }
                if params.as_slice() == [TypeKind::I64]
                    && return_type.as_ref() == &TypeKind::I64
        )),
        "add([1i64]...) should typecheck as a function awaiting the missing right argument: {:?}",
        report.judgments
    );
    assert!(
        report
            .typed_lowering_evidence
            .iter()
            .any(|evidence| matches!(
                &evidence.kind,
                TypedLoweringEvidenceKind::SignaturePartialCall {
                    callee,
                    result_ty,
                    arg_count: 1,
                } if callee == "add"
                    && matches!(
                        result_ty,
                        TypeKind::Function { params, return_type, .. }
                            if params.as_slice() == [TypeKind::I64]
                                && return_type.as_ref() == &TypeKind::I64
                    )
            )),
        "fixed literal spread partial call should record signature partial evidence: {:?}",
        report.typed_lowering_evidence
    );
}

#[test]
fn bare_signature_prefix_call_statement_reports_missing_argument() {
    let tree = parse_ok(
        r"
fn add(lhs: i64, rhs: i64) -> i64 {
    return lhs + rhs
}

flow @flow.bad_partial_statement bad_partial_statement {
    add(2i64)
}
",
    );
    let hir = lower_to_hir(&tree).expect("bare partial statement fixture lowers");
    validate_typecheck_ready(&hir).expect("bare partial statement fixture is structured");

    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.iter().any(|diagnostic| diagnostic
            .message()
            .contains("missing required argument `rhs`")),
        "bare statement partial call should report the missing fixed argument: {:?}",
        report.diagnostics
    );
    assert!(
        !report
            .typed_lowering_evidence
            .iter()
            .any(|evidence| matches!(
                evidence.kind,
                TypedLoweringEvidenceKind::SignaturePartialCall { .. }
            )),
        "bare statement partial call should not lower as a function value: {:?}",
        report.typed_lowering_evidence
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
                TypeKind::Function { params, return_type, .. }
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
                TypeKind::Function { params, return_type, .. }
                    if params == &[TypeKind::I64]
                        && matches!(
                            return_type.as_ref(),
                            TypeKind::Function { params: final_params, return_type: final_return_type, .. }
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
fn curried_task_dialogue_and_stream_functions_preserve_param_groups() {
    let tree = parse_ok(
        r#"
task fn task_label(prefix: String)(name: String) -> String {
    return name
}

dialogue fn dialogue_label(prefix: String)(name: String) -> String {
    return name
}

stream fn stream_passthrough(prefix: String)(frames: Stream<i64, String>) -> Stream<i64, String> {
    for frame in frames {
        yield frame
    }
}

flow @flow.curried_function_kind_calls curried_function_kind_calls {
    let task_partial = task_label("prefix")
    let task_value: String = task_partial("name")
    let dialogue_value: String = dialogue_label("prefix")("name")
    log.info(task_value)
    log.info(dialogue_value)
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("curried function-kind fixture lowers");
    validate_typecheck_ready(&hir).expect("curried function-kind fixture is structured");
    assert_eq!(
        hir.functions()
            .iter()
            .map(|function| function.signature().param_groups().len())
            .collect::<Vec<_>>(),
        vec![2, 2, 2],
        "all non-flow function kinds should preserve curried parameter groups"
    );
    assert_eq!(hir.functions()[0].kind(), FunctionKind::Task);
    assert_eq!(hir.functions()[1].kind(), FunctionKind::Dialogue);
    assert_eq!(hir.functions()[2].kind(), FunctionKind::Stream);

    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.is_empty(),
        "curried task/dialogue/stream functions should typecheck, got {:?}",
        report.diagnostics
    );
    assert!(
        report.judgments.iter().any(|judgment| {
            matches!(
                &judgment.ty,
                TypeKind::Function { params, return_type, .. }
                    if params == &[TypeKind::String] && return_type.as_ref() == &TypeKind::String
            )
        }),
        "curried task/dialogue first calls should expose the staged function type"
    );
}

#[test]
fn curried_trait_method_preserves_call_group_semantics() {
    let report = analyze_registered_function_stack_fixture(
        "curried-trait-method",
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

flow @flow.curried_trait_method curried_trait_method(score: Score) {
    let predicate: i64 -> bool = score.above(80i64)
    let ok: bool = predicate(81i64)
    let direct: bool = score.above(80i64)(82i64)
    log.info(ok)
    log.info(direct)
}
",
    );
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
                        TypeKind::Function { params, return_type, .. }
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
    let report = analyze_registered_function_stack_fixture(
        "curried-trait-flattened",
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

flow @flow.curried_trait_flattened curried_trait_flattened(score: Score) {
    let wrong = score.above(80i64, 81i64)
    log.info(wrong)
}
",
    );
    assert!(
        report.retained_call_target_facts().any(|facts| {
            matches!(
                facts.target(),
                crate::callable::CallTargetFact::Rejected { candidates }
                    if candidates.iter().any(|candidate| {
                        candidate.id().family() == crate::callable::CallableFamily::TraitMethod
                            && candidate.schema().groups().len() == 2
                    })
            ) && facts.diagnostics().iter().any(|diagnostic| {
                diagnostic.code()
                    == crate::callable::CallableDiagnosticCode::TooManyPositionalArguments
            })
        }),
        "flattened trait-method arguments must be rejected against the first of two retained call groups: {:?}",
        report.diagnostics,
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
                TypeKind::Function { params, return_type, .. }
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
                TypeKind::Function { params, return_type, .. }
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
                TypeKind::Function { params, return_type, .. }
                    if params == &[TypeKind::I64]
                        && matches!(
                            return_type.as_ref(),
                            TypeKind::Function { params: inner_params, return_type: inner_return_type, .. } if inner_params == &[TypeKind::I64]
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
            TypeKind::Function { params, return_type, .. } if params.as_slice() == [TypeKind::I64]
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
                        TypeKind::Function { params, return_type, .. }
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
            TypeKind::Function { params, return_type, .. } if params.as_slice() == [TypeKind::I64]
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
            TypeKind::Function { params, return_type, .. } if params.as_slice() == [TypeKind::I64]
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
            TypeKind::Function { params, return_type, .. } if params.as_slice() == [TypeKind::I64]
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
                        TypeKind::Function { params, return_type, .. }
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
            TypeKind::Function { params, return_type, .. } if params.as_slice() == [TypeKind::I64]
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
                        TypeKind::Function { params, return_type, .. }
                            if params.as_slice() == [TypeKind::I64]
                                && return_type.as_ref() == &TypeKind::I64
                    )
            )
        }),
        "expected named partial call placeholder to record one generated parameter"
    );
}

#[test]
fn rejects_partial_call_placeholder_mixed_with_spread() {
    let tree = parse_ok(
        r"
#[pure]
fn add(left: i64, right: i64) -> i64 {
    return left + right
}

flow @flow.partial_call_spread_placeholder partial_call_spread_placeholder {
    let values = [1i64]
    let add_later = add(_, values...)
}
",
    );
    let hir = lower_to_hir(&tree).expect("spread placeholder partial fixture lowers");
    validate_typecheck_ready(&hir).expect("spread placeholder partial fixture is structured");

    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.iter().any(|diagnostic| {
            matches!(
                diagnostic.kind(),
                TypeCheckErrorKind::UnsupportedSignaturePartialCall { function, reason }
                    if function == "add" && reason.contains("`_` placeholder")
            )
        }),
        "expected structured spread partial diagnostic, got {:?}",
        report.diagnostics
    );
    assert!(
        report.diagnostics.iter().all(|diagnostic| {
            !diagnostic.message().contains("does not accept spread")
                && !diagnostic.message().contains("missing required argument")
        }),
        "spread partial diagnostic should not degrade into generic call errors: {:?}",
        report.diagnostics
    );
    assert!(
        !report.typed_lowering_evidence.iter().any(|evidence| {
            matches!(
                &evidence.kind,
                TypedLoweringEvidenceKind::SignaturePartialCall { callee, .. }
                    if callee == "add"
            )
        }),
        "rejected spread partial must not record signature partial lowering evidence"
    );
}

#[test]
fn rejects_fixed_signature_partial_call_with_spread() {
    let tree = parse_ok(
        r"
fn add(left: i64, right: i64) -> i64 {
    return left + right
}

flow @flow.partial_call_spread_only partial_call_spread_only {
    let values = [1i64]
    let add_later = add(values...)
}
",
    );
    let hir = lower_to_hir(&tree).expect("spread-only partial fixture lowers");
    validate_typecheck_ready(&hir).expect("spread-only partial fixture is structured");

    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.iter().any(|diagnostic| {
            matches!(
                diagnostic.kind(),
                TypeCheckErrorKind::UnsupportedSignaturePartialCall { function, reason }
                    if function == "add" && reason.contains("missing-input partial")
            )
        }),
        "expected structured spread-only partial diagnostic, got {:?}",
        report.diagnostics
    );
    assert!(
        report.diagnostics.iter().all(|diagnostic| {
            !diagnostic.message().contains("does not accept spread")
                && !diagnostic.message().contains("missing required argument")
        }),
        "spread-only partial diagnostic should not degrade into generic call errors: {:?}",
        report.diagnostics
    );
    assert!(
        !report.typed_lowering_evidence.iter().any(|evidence| {
            matches!(
                &evidence.kind,
                TypedLoweringEvidenceKind::SignaturePartialCall { callee, .. }
                    if callee == "add"
            )
        }),
        "rejected spread-only partial must not record signature partial lowering evidence"
    );
}

#[test]
fn rejects_named_missing_input_partial_call_mixed_with_spread() {
    let tree = parse_ok(
        r"
fn add(left: i64, right: i64) -> i64 {
    return left + right
}

flow @flow.partial_call_named_spread partial_call_named_spread {
    let values = [2i64]
    let add_to_right = add(right = 1i64, values...)
}
",
    );
    let hir = lower_to_hir(&tree).expect("named spread partial fixture lowers");
    validate_typecheck_ready(&hir).expect("named spread partial fixture is structured");

    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.iter().any(|diagnostic| {
            matches!(
                diagnostic.kind(),
                TypeCheckErrorKind::UnsupportedSignaturePartialCall { function, reason }
                    if function == "add" && reason.contains("missing-input partial")
            )
        }),
        "expected structured named spread partial diagnostic, got {:?}",
        report.diagnostics
    );
    assert!(
        report.diagnostics.iter().all(|diagnostic| {
            !diagnostic.message().contains("does not accept spread")
                && !diagnostic.message().contains("missing required argument")
        }),
        "named spread partial diagnostic should not degrade into generic call errors: {:?}",
        report.diagnostics
    );
    assert!(
        !report.typed_lowering_evidence.iter().any(|evidence| {
            matches!(
                &evidence.kind,
                TypedLoweringEvidenceKind::SignaturePartialCall { callee, .. }
                    if callee == "add"
            )
        }),
        "rejected named spread partial must not record signature partial lowering evidence"
    );
}

#[test]
fn rejects_partial_call_spread_before_positional_fixed_arg() {
    let tree = parse_ok(
        r"
fn add(left: i64, right: i64) -> i64 {
    return left + right
}

flow @flow.partial_call_spread_then_positional partial_call_spread_then_positional {
    let values = [1i64]
    let add_later = add(values..., 2i64)
}
",
    );
    let hir = lower_to_hir(&tree).expect("spread-before-positional partial fixture lowers");
    validate_typecheck_ready(&hir).expect("spread-before-positional partial fixture is structured");

    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.iter().any(|diagnostic| {
            matches!(
                diagnostic.kind(),
                TypeCheckErrorKind::UnsupportedSignaturePartialCall { function, reason }
                    if function == "add" && reason.contains("followed by fixed partial-call arguments")
            )
        }),
        "expected structured spread-before-positional diagnostic, got {:?}",
        report.diagnostics
    );
    assert!(
        report.diagnostics.iter().all(|diagnostic| {
            !diagnostic.message().contains("does not accept spread")
                && !diagnostic.message().contains("missing required argument")
        }),
        "spread-before-positional diagnostic should not degrade into generic call errors: {:?}",
        report.diagnostics
    );
    assert!(
        !report.typed_lowering_evidence.iter().any(|evidence| {
            matches!(
                &evidence.kind,
                TypedLoweringEvidenceKind::SignaturePartialCall { callee, .. }
                    if callee == "add"
            )
        }),
        "rejected spread-before-positional partial must not record signature partial evidence"
    );
}

#[test]
fn rejects_partial_call_spread_before_named_fixed_arg() {
    let tree = parse_ok(
        r"
fn add(left: i64, right: i64) -> i64 {
    return left + right
}

flow @flow.partial_call_spread_then_named partial_call_spread_then_named {
    let values = [1i64]
    let add_later = add(values..., right = 2i64)
}
",
    );
    let hir = lower_to_hir(&tree).expect("spread-before-named partial fixture lowers");
    validate_typecheck_ready(&hir).expect("spread-before-named partial fixture is structured");

    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.iter().any(|diagnostic| {
            matches!(
                diagnostic.kind(),
                TypeCheckErrorKind::UnsupportedSignaturePartialCall { function, reason }
                    if function == "add" && reason.contains("followed by fixed partial-call arguments")
            )
        }),
        "expected structured spread-before-named diagnostic, got {:?}",
        report.diagnostics
    );
    assert!(
        report.diagnostics.iter().all(|diagnostic| {
            !diagnostic.message().contains("does not accept spread")
                && !diagnostic.message().contains("missing required argument")
        }),
        "spread-before-named diagnostic should not degrade into generic call errors: {:?}",
        report.diagnostics
    );
    assert!(
        !report.typed_lowering_evidence.iter().any(|evidence| {
            matches!(
                &evidence.kind,
                TypedLoweringEvidenceKind::SignaturePartialCall { callee, .. }
                    if callee == "add"
            )
        }),
        "rejected spread-before-named partial must not record signature partial evidence"
    );
}

#[test]
fn rejects_partial_call_multiple_spreads_with_structured_diagnostic() {
    let tree = parse_ok(
        r"
fn add(left: i64, right: i64) -> i64 {
    return left + right
}

flow @flow.partial_call_multiple_spreads partial_call_multiple_spreads {
    let lefts = [1i64]
    let rights = [2i64]
    let add_later = add(lefts..., rights...)
}
",
    );
    let hir = lower_to_hir(&tree).expect("multiple-spread partial fixture lowers");
    validate_typecheck_ready(&hir).expect("multiple-spread partial fixture is structured");

    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.iter().any(|diagnostic| {
            matches!(
                diagnostic.kind(),
                TypeCheckErrorKind::UnsupportedSignaturePartialCall { function, reason }
                    if function == "add" && reason.contains("multiple spread arguments")
            )
        }),
        "expected structured multiple-spread partial diagnostic, got {:?}",
        report.diagnostics
    );
    assert!(
        report.diagnostics.iter().all(|diagnostic| {
            !diagnostic.message().contains("does not accept spread")
                && !diagnostic.message().contains("missing required argument")
        }),
        "multiple-spread partial diagnostic should not degrade into generic call errors: {:?}",
        report.diagnostics
    );
    assert!(
        !report.typed_lowering_evidence.iter().any(|evidence| {
            matches!(
                &evidence.kind,
                TypedLoweringEvidenceKind::SignaturePartialCall { callee, .. }
                    if callee == "add"
            )
        }),
        "rejected multiple-spread partial must not record signature partial evidence"
    );
}

#[test]
fn non_annotated_function_named_missing_input_typechecks_as_partial_application() {
    let tree = parse_ok(
        r"
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
            TypeKind::Function { params, return_type, .. } if params.as_slice() == [TypeKind::I64]
                && return_type.as_ref() == &TypeKind::I64
        )
    }));
}
