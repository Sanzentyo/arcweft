use crate::plan::{RuntimePureHelper, RuntimePureHelperId, RuntimePureHelperOrigin};
use crate::pure::{
    AotPureFunctionBackend, PureFunctionBackend, PureFunctionBackendKind, PureFunctionRequest,
    RuntimePureCallBackend, VmPureFunctionBackend, VmPureFunctionScratch, VmRuntimePureCallBackend,
    compare_pure_function_backend,
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
fn vm_runtime_value_fallback_records_pure_call_stats() {
    let helper = RuntimePureHelper {
        id: RuntimePureHelperId(0),
        name: "echo_label".to_owned(),
        input_names: vec!["label".to_owned()],
        expr: RuntimeExpr::Local("label".to_owned()),
        scalar_eval_supported: false,
        origin: RuntimePureHelperOrigin::Annotated,
    };
    let mut backend = VmRuntimePureCallBackend::default();

    let value = backend
        .call_values(&helper, &[RuntimeValue::String("ready".to_owned())])
        .expect("VM value fallback evaluates");

    assert_eq!(value, RuntimeValue::String("ready".to_owned()));
    assert_eq!(backend.stats().pure_calls, 1);
    assert_eq!(backend.stats().vm_calls, 1);
    assert_eq!(backend.stats().fallbacks, 1);
    assert_eq!(backend.stats().arg_vec_allocations, 0);
    assert_eq!(
        backend.stats().arg_bytes_borrowed,
        std::mem::size_of_val(&[RuntimeValue::String("ready".to_owned())])
    );
}

#[test]
fn vm_runtime_i64_fast_path_records_copy_bytes() {
    let helper = RuntimePureHelper {
        id: RuntimePureHelperId(0),
        name: "score".to_owned(),
        input_names: vec!["base".to_owned(), "bonus".to_owned()],
        expr: RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Add,
            rhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
        },
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Annotated,
    };
    let mut backend = VmRuntimePureCallBackend::default();

    let value = backend
        .call_i64(&helper, crate::pure::RuntimeI64Args::new([3, 4, 0, 0], 2))
        .expect("VM i64 fast path evaluates");

    assert_eq!(value, Some(7));
    assert_eq!(backend.stats().arg_stack_packs, 1);
    assert_eq!(
        backend.stats().arg_bytes_copied,
        2 * std::mem::size_of::<i64>()
    );
    assert_eq!(
        backend.stats().result_bytes_copied,
        std::mem::size_of::<i64>()
    );
}

#[test]
fn vm_pure_scratch_reuses_and_rebuilds_i64_root_bindings() {
    let add = RuntimePureHelper {
        id: RuntimePureHelperId(0),
        name: "add".to_owned(),
        input_names: vec!["base".to_owned(), "bonus".to_owned()],
        expr: RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Add,
            rhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
        },
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Annotated,
    };
    let double = RuntimePureHelper {
        id: RuntimePureHelperId(1),
        name: "double".to_owned(),
        input_names: vec!["value".to_owned()],
        expr: RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("value".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Value(RuntimeValue::Int(2))),
        },
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Annotated,
    };
    let mut scratch = VmPureFunctionScratch::default();

    let first = scratch
        .evaluate_i64_slice(&add, &[3, 4])
        .expect("scratch evaluates first helper");
    let second = scratch
        .evaluate_i64_slice(&add, &[5, 6])
        .expect("scratch reuses matching root bindings");
    let third = scratch
        .evaluate_i64_slice(&double, &[7])
        .expect("scratch rebuilds when helper inputs change");

    assert_eq!(first, RuntimeValue::Int(7));
    assert_eq!(second, RuntimeValue::Int(11));
    assert_eq!(third, RuntimeValue::Int(14));
}

#[test]
fn vm_pure_scratch_reuses_value_root_bindings_without_request_allocation() {
    let echo = RuntimePureHelper {
        id: RuntimePureHelperId(0),
        name: "echo".to_owned(),
        input_names: vec!["label".to_owned()],
        expr: RuntimeExpr::Local("label".to_owned()),
        scalar_eval_supported: false,
        origin: RuntimePureHelperOrigin::Annotated,
    };
    let mut scratch = VmPureFunctionScratch::default();

    let first = scratch
        .evaluate_values(&echo, &[RuntimeValue::String("ready".to_owned())])
        .expect("scratch evaluates first value helper");
    let second = scratch
        .evaluate_values(&echo, &[RuntimeValue::String("done".to_owned())])
        .expect("scratch reuses matching value root binding");

    assert_eq!(first, RuntimeValue::String("ready".to_owned()));
    assert_eq!(second, RuntimeValue::String("done".to_owned()));
}

#[test]
fn vm_runtime_i64_batch_records_batch_stats() {
    let helper = RuntimePureHelper {
        id: RuntimePureHelperId(0),
        name: "score".to_owned(),
        input_names: vec!["base".to_owned(), "bonus".to_owned()],
        expr: RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
        },
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Annotated,
    };
    let mut backend = VmRuntimePureCallBackend::default();
    let rows = [
        crate::pure::RuntimeI64Args::new([3, 4, 0, 0], 2),
        crate::pure::RuntimeI64Args::new([5, 6, 0, 0], 2),
    ];
    let mut out = [0; 2];

    backend
        .call_i64_batch(&helper, &rows, &mut out)
        .expect("VM i64 batch evaluates");

    assert_eq!(out, [12, 30]);
    assert_eq!(backend.stats().batch_calls, 1);
    assert_eq!(backend.stats().batch_items, 2);
    assert_eq!(backend.stats().pure_calls, 2);
    assert_eq!(backend.stats().vm_calls, 2);
    assert_eq!(backend.stats().arg_stack_packs, 2);
    assert_eq!(backend.stats().arg_vec_allocations, 0);
    assert_eq!(
        backend.stats().arg_bytes_copied,
        4 * std::mem::size_of::<i64>()
    );
    assert_eq!(
        backend.stats().result_bytes_copied,
        2 * std::mem::size_of::<i64>()
    );
}

#[test]
fn vm_runtime_i64_flat_batch_borrows_input_slice() {
    let helper = RuntimePureHelper {
        id: RuntimePureHelperId(0),
        name: "score".to_owned(),
        input_names: vec!["base".to_owned(), "bonus".to_owned()],
        expr: RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
        },
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Annotated,
    };
    let mut backend = VmRuntimePureCallBackend::default();
    let inputs = [3, 4, 5, 6];
    let mut out = [0; 2];

    backend
        .call_i64_flat_batch(&helper, &inputs, 2, &mut out)
        .expect("VM flat i64 batch evaluates");

    assert_eq!(out, [12, 30]);
    assert_eq!(backend.stats().batch_calls, 1);
    assert_eq!(backend.stats().batch_items, 2);
    assert_eq!(backend.stats().pure_calls, 2);
    assert_eq!(backend.stats().vm_calls, 2);
    assert_eq!(backend.stats().arg_stack_packs, 0);
    assert_eq!(backend.stats().arg_vec_allocations, 0);
    assert_eq!(backend.stats().arg_bytes_copied, 0);
    assert_eq!(
        backend.stats().arg_bytes_borrowed,
        std::mem::size_of_val(&inputs)
    );
    assert_eq!(
        backend.stats().result_bytes_copied,
        std::mem::size_of_val(&out)
    );
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
fn aot_compiled_i64_plan_accepts_runtime_inputs() {
    let request = PureFunctionRequest::new(
        "score_inputs",
        RuntimeExpr::If {
            condition: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                op: RuntimeBinaryOp::Ge,
                rhs: Box::new(RuntimeExpr::Value(RuntimeValue::Int(3))),
            }),
            then_expr: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                op: RuntimeBinaryOp::Mul,
                rhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
            }),
            else_expr: Box::new(RuntimeExpr::Value(RuntimeValue::Int(0))),
        },
        [int_binding("base", 0), int_binding("bonus", 0)],
    );

    let plan = AotPureFunctionBackend::new()
        .compile_i64_with_inputs(&request, ["base", "bonus"])
        .expect("AOT compiles parameterized helper");

    assert_eq!(plan.call_with_inputs(&[3, 4]).expect("call succeeds").0, 12);
    assert_eq!(plan.call_with_inputs(&[2, 99]).expect("call succeeds").0, 0);
    let mut slots = Vec::new();
    assert_eq!(
        plan.call_with_inputs_scratch(&[5, 6], &mut slots)
            .expect("scratch call succeeds")
            .0,
        30
    );
    let slot_capacity = slots.capacity();
    let slot_len = slots.len();
    assert_eq!(
        plan.call_with_inputs_scratch(&[1, 9], &mut slots)
            .expect("scratch call succeeds")
            .0,
        0
    );
    assert_eq!(slots.capacity(), slot_capacity);
    assert_eq!(slots.len(), slot_len);
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
