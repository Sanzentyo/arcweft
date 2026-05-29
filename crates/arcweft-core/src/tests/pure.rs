use crate::pure::{
    AotPureFunctionBackend, PureFunctionBackend, PureFunctionBackendKind, PureFunctionRequest,
    VmPureFunctionBackend, compare_pure_function_backend,
};
use crate::value::{RuntimeBinaryOp, RuntimeBinding, RuntimeEvalError, RuntimeExpr, RuntimeValue};

fn int_binding(name: &str, value: i64) -> RuntimeBinding {
    RuntimeBinding {
        name: name.to_owned(),
        value: RuntimeValue::Int(value),
    }
}

#[test]
fn vm_pure_backend_evaluates_deterministic_helper_expr() {
    let request = PureFunctionRequest::new(
        "score",
        RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Add,
            rhs: Box::new(RuntimeExpr::Call {
                callee: "add".to_owned(),
                args: vec![
                    RuntimeExpr::Local("bonus".to_owned()),
                    RuntimeExpr::Value(RuntimeValue::Int(2)),
                ],
            }),
        },
        [int_binding("base", 3), int_binding("bonus", 4)],
    );

    let result = VmPureFunctionBackend
        .evaluate(&request)
        .expect("pure helper evaluates");

    assert_eq!(result.backend, PureFunctionBackendKind::Vm);
    assert_eq!(result.value, RuntimeValue::Int(9));
    assert_eq!(result.stats.evaluated_calls, 1);
    assert_eq!(result.stats.evaluated_binary_ops, 1);
    assert!(result.stats.evaluated_exprs >= 5);
}

#[test]
fn vm_pure_backend_evaluates_lexical_let_expr() {
    let request = PureFunctionRequest::new(
        "score_with_local",
        RuntimeExpr::Let {
            name: "boosted".to_owned(),
            expr: Box::new(RuntimeExpr::Call {
                callee: "add".to_owned(),
                args: vec![
                    RuntimeExpr::Local("bonus".to_owned()),
                    RuntimeExpr::Value(RuntimeValue::Int(2)),
                ],
            }),
            body: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                op: RuntimeBinaryOp::Mul,
                rhs: Box::new(RuntimeExpr::Local("boosted".to_owned())),
            }),
        },
        [int_binding("base", 3), int_binding("bonus", 4)],
    );

    let result = VmPureFunctionBackend
        .evaluate(&request)
        .expect("pure helper evaluates lexical let");

    assert_eq!(result.value, RuntimeValue::Int(18));
    assert_eq!(result.stats.evaluated_calls, 1);
    assert_eq!(result.stats.evaluated_binary_ops, 1);
}

#[test]
fn aot_pure_backend_candidate_matches_vm_result() {
    let request = PureFunctionRequest::new(
        "score_branch",
        RuntimeExpr::Let {
            name: "boosted".to_owned(),
            expr: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
                op: RuntimeBinaryOp::Add,
                rhs: Box::new(RuntimeExpr::Value(RuntimeValue::Int(2))),
            }),
            body: Box::new(RuntimeExpr::If {
                condition: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                    op: RuntimeBinaryOp::Ge,
                    rhs: Box::new(RuntimeExpr::Value(RuntimeValue::Int(3))),
                }),
                then_expr: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                    op: RuntimeBinaryOp::Mul,
                    rhs: Box::new(RuntimeExpr::Local("boosted".to_owned())),
                }),
                else_expr: Box::new(RuntimeExpr::Value(RuntimeValue::Int(0))),
            }),
        },
        [int_binding("base", 3), int_binding("bonus", 4)],
    );

    let conformance = compare_pure_function_backend(
        &VmPureFunctionBackend,
        &AotPureFunctionBackend::new(),
        &request,
    )
    .expect("pure backends evaluate");

    assert!(conformance.matches_vm);
    assert_eq!(conformance.vm.backend, PureFunctionBackendKind::Vm);
    assert_eq!(conformance.candidate.backend, PureFunctionBackendKind::Aot);
    assert_eq!(conformance.candidate.value, RuntimeValue::Int(18));
    assert_eq!(conformance.candidate.stats.evaluated_binary_ops, 3);
}

#[test]
fn aot_compiled_i64_plan_can_be_called_repeatedly() {
    let request = PureFunctionRequest::new(
        "score",
        RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Value(RuntimeValue::Int(20))),
            op: RuntimeBinaryOp::Add,
            rhs: Box::new(RuntimeExpr::Value(RuntimeValue::Int(22))),
        },
        [],
    );

    let plan = AotPureFunctionBackend::new()
        .compile_i64(&request)
        .expect("AOT compiles i64 helper");

    assert_eq!(plan.name(), "score");
    assert_eq!(plan.call().0, 42);
    assert_eq!(plan.call().0, 42);
}

#[test]
fn aot_pure_backend_rejects_non_integer_helpers() {
    let request = PureFunctionRequest::new(
        "trim_label",
        RuntimeExpr::MethodCall {
            receiver: Box::new(RuntimeExpr::Value(RuntimeValue::String(
                "  menu item  ".to_owned(),
            ))),
            method: "trim".to_owned(),
            args: Vec::new(),
        },
        [],
    );

    let error = AotPureFunctionBackend::new()
        .evaluate(&request)
        .expect_err("string-heavy helpers are outside the AOT i64 subset");

    assert!(matches!(error, RuntimeEvalError::UnsupportedPure { .. }));
}

#[test]
fn pure_backend_rejects_unregistered_effectful_calls() {
    let request = PureFunctionRequest::new(
        "effectful",
        RuntimeExpr::Call {
            callee: "play_audio".to_owned(),
            args: Vec::new(),
        },
        [],
    );

    let error = VmPureFunctionBackend
        .evaluate(&request)
        .expect_err("effectful call is outside the pure helper subset");

    assert_eq!(
        error,
        RuntimeEvalError::UnsupportedPure {
            name: "play_audio".to_owned(),
            reason: "call is not registered as a pure helper".to_owned()
        }
    );
}
