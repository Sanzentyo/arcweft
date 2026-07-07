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
