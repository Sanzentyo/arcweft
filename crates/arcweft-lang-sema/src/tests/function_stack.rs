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
    let env = TypeCheckEnv::new().with_symbol(
        "f",
        TypeKind::Function {
            params: vec![TypeKind::I64],
            return_type: Box::new(TypeKind::Bool),
        },
    );

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
    let env = TypeCheckEnv::new().with_symbol(
        "f",
        TypeKind::Function {
            params: vec![TypeKind::I64],
            return_type: Box::new(TypeKind::Bool),
        },
    );

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
    let env = TypeCheckEnv::new().with_symbol(
        "f",
        TypeKind::Function {
            params: vec![TypeKind::I64],
            return_type: Box::new(TypeKind::Bool),
        },
    );

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
            TypeKind::Function { params, return_type }
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
                        TypeKind::Function { params, return_type }
                            if params.as_slice() == [TypeKind::I64]
                                && return_type.as_ref() == &TypeKind::I64
                    )
            )),
        "direct top-level partial call should record runtime lowering evidence: {:?}",
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
                TypeKind::Function { params, return_type }
                    if params == &[TypeKind::String] && return_type.as_ref() == &TypeKind::String
            )
        }),
        "curried task/dialogue first calls should expose the staged function type"
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
                TypeJudgmentSubject::Expr { kind: "call", .. }
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
    let rows = report.effects.closed_effect_rows();
    let empty_substitution = crate::effect_row::EffectSubstitution::new();
    let flow_row = rows
        .summary(&crate::effect_model::CallableId::new(
            "flow.closure_row_projection",
        ))
        .expect("flow row is projected");
    assert!(
        flow_row
            .inferred()
            .resolve(&empty_substitution)
            .expect("closed flow row resolves")
            .is_empty(),
        "closure creation must not add body effects to caller row: {flow_row:?}"
    );
    let closure_row = rows
        .summaries()
        .find(|(callable, _)| callable.as_str().starts_with("closure.expr."))
        .map(|(_, row)| row)
        .expect("closure synthetic row is projected");
    assert_eq!(
        closure_row
            .inferred()
            .resolve(&empty_substitution)
            .expect("closed closure row resolves")
            .to_labels(),
        vec!["fs.read"],
        "closure body effects should live on the closure row"
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
        EnumVariantPayload::tuple([TypeKind::Function {
            params: vec![TypeKind::String],
            return_type: Box::new(TypeKind::String),
        }]),
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
            TypeKind::Function {
                params: vec![TypeKind::String],
                return_type: Box::new(TypeKind::String),
            },
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
fn method_chain_data_last_fallback_composes_higher_order_callback_effects() {
    let tree = parse_ok(
        r#"
fn use_loader(load: String -> String, path: String) -> String {
    return load(path)
}

flow @flow.method_fallback_callback_effect method_fallback_callback_effect
effects { }
{
    let body = "story.arcw".use_loader(|path: String| -> String {
        adapter.read_text(path = path)
    })
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("method fallback callback fixture lowers");
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
fn method_chain_reports_spread_then_named_data_last_fallback_as_unsupported() {
    let tree = parse_ok(
        r"
flow @flow.method_fallback_spread_then_named method_fallback_spread_then_named {
    let thresholds = [80i64]
    let wrong = score.between(thresholds..., max = 99i64)
    log.info(wrong)
}
",
    );
    let hir = lower_to_hir(&tree).expect("spread then named fallback fixture lowers");
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
flow @flow.method_fallback_multiple_spreads method_fallback_multiple_spreads {
    let lows = [60i64]
    let highs = [90i64]
    let wrong = score.between(lows..., highs...)
    log.info(wrong)
}
",
    );
    let hir = lower_to_hir(&tree).expect("multiple-spread fallback fixture lowers");
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
fn method_chain_reports_ambiguous_spread_data_last_fallback_candidates() {
    let tree = parse_ok(
        r"
#[pure]
fn above(min: i64, value: i64) -> bool {
    return value > min
}

flow @flow.method_fallback_spread_ambiguous method_fallback_spread_ambiguous {
    let thresholds = [80i64]
    let wrong = score.above(thresholds...)
}
",
    );
    let hir = lower_to_hir(&tree).expect("ambiguous spread fallback fixture lowers");
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
