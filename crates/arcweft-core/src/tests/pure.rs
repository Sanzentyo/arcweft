use crate::plan::{
    RuntimePureHelper, RuntimePureHelperId, RuntimePureHelperOrigin, RuntimePureInputType,
    RuntimePureOutputType,
};
use crate::pure::{
    AotPureFunctionBackend, PureFunctionBackend, PureFunctionBackendKind, PureFunctionRequest,
    RuntimePureCallBackend, VmPureFunctionBackend, VmPureFunctionScratch, VmRuntimePureCallBackend,
    compare_pure_function_backend,
};
use crate::value::{
    RuntimeBinaryOp, RuntimeBinding, RuntimeEvalError, RuntimeExpr, RuntimeValue,
    runtime_sequence_dense_bool, runtime_sequence_dense_bytes, runtime_sequence_dense_i8,
    runtime_sequence_dense_i16, runtime_sequence_dense_i32, runtime_sequence_dense_i64,
    runtime_sequence_dense_i128, runtime_sequence_dense_isize, runtime_sequence_dense_u8,
    runtime_sequence_dense_u16, runtime_sequence_dense_u32, runtime_sequence_dense_u64,
    runtime_sequence_dense_u128, runtime_sequence_dense_usize, runtime_sequence_values,
};

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
fn vm_pure_backend_sums_local_i64_sequence_by_borrow() {
    let request = PureFunctionRequest::new(
        "sum_scores",
        RuntimeExpr::Sum {
            source: Box::new(RuntimeExpr::Local("scores".to_owned())),
        },
        [RuntimeBinding {
            name: "scores".to_owned(),
            value: runtime_sequence_values(vec![
                RuntimeValue::Int(18),
                RuntimeValue::Int(15),
                RuntimeValue::Int(20),
            ]),
        }],
    );

    let result = VmPureFunctionBackend
        .evaluate(&request)
        .expect("pure helper sums local sequence");

    assert_eq!(result.value, RuntimeValue::Int(53));
}

#[test]
fn vm_pure_backend_sums_dense_i64_sequence_without_materializing_values() {
    let request = PureFunctionRequest::new(
        "sum_scores",
        RuntimeExpr::Sum {
            source: Box::new(RuntimeExpr::Local("scores".to_owned())),
        },
        [RuntimeBinding {
            name: "scores".to_owned(),
            value: runtime_sequence_dense_i64(vec![18, 15, 20]),
        }],
    );

    let result = VmPureFunctionBackend
        .evaluate(&request)
        .expect("pure helper sums dense local sequence");

    assert_eq!(result.value, RuntimeValue::Int(53));
}

#[test]
fn vm_pure_backend_sums_dense_integer_sequences_without_materializing_values() {
    let cases = [
        runtime_sequence_dense_i8(vec![18, 15, 20]),
        runtime_sequence_dense_i16(vec![18, 15, 20]),
        runtime_sequence_dense_i32(vec![18, 15, 20]),
        runtime_sequence_dense_i64(vec![18, 15, 20]),
        runtime_sequence_dense_i128(vec![18, 15, 20]),
        runtime_sequence_dense_isize(vec![18, 15, 20]),
        runtime_sequence_dense_u8(vec![18, 15, 20]),
        runtime_sequence_dense_u16(vec![18, 15, 20]),
        runtime_sequence_dense_u32(vec![18, 15, 20]),
        runtime_sequence_dense_u64(vec![18, 15, 20]),
        runtime_sequence_dense_u128(vec![18, 15, 20]),
        runtime_sequence_dense_usize(vec![18, 15, 20]),
        runtime_sequence_dense_bytes(vec![18, 15, 20]),
    ];

    for value in cases {
        let request = PureFunctionRequest::new(
            "sum_scores",
            RuntimeExpr::Sum {
                source: Box::new(RuntimeExpr::Local("scores".to_owned())),
            },
            [RuntimeBinding {
                name: "scores".to_owned(),
                value,
            }],
        );

        let result = VmPureFunctionBackend
            .evaluate(&request)
            .expect("pure helper sums dense integer-compatible sequence");

        assert_eq!(result.value, RuntimeValue::Int(53));
    }
}

#[test]
fn vm_pure_backend_reads_dense_sequence_len_without_materializing_values() {
    let request = PureFunctionRequest::new(
        "flag_count",
        RuntimeExpr::MethodCall {
            receiver: Box::new(RuntimeExpr::Local("flags".to_owned())),
            method: "len".to_owned(),
            args: Vec::new(),
        },
        [RuntimeBinding {
            name: "flags".to_owned(),
            value: runtime_sequence_dense_bool(vec![true, false, true, true]),
        }],
    );

    let result = VmPureFunctionBackend
        .evaluate(&request)
        .expect("pure helper reads dense sequence length");

    assert_eq!(result.value, RuntimeValue::UInt(4));
    assert_eq!(result.stats.evaluated_method_calls, 1);
}

#[test]
fn vm_runtime_value_fallback_records_pure_call_stats() {
    let helper = RuntimePureHelper {
        id: RuntimePureHelperId(0),
        name: "echo_label".to_owned(),
        input_names: vec!["label".to_owned()],
        input_types: vec![RuntimePureInputType::Value],
        output_type: RuntimePureOutputType::Value,
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
        input_types: vec![RuntimePureInputType::I64, RuntimePureInputType::I64],
        output_type: RuntimePureOutputType::I64,
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
    assert_eq!(backend.stats().result_bytes_copied, 0);
}

#[test]
fn vm_runtime_i64_slice_fast_path_records_borrowed_bytes() {
    let helper = RuntimePureHelper {
        id: RuntimePureHelperId(0),
        name: "score".to_owned(),
        input_names: vec!["base".to_owned(), "bonus".to_owned()],
        input_types: vec![RuntimePureInputType::I64, RuntimePureInputType::I64],
        output_type: RuntimePureOutputType::I64,
        expr: RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Add,
            rhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
        },
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Annotated,
    };
    let mut backend = VmRuntimePureCallBackend::default();
    let args = [3, 4];

    let value = backend
        .call_i64_slice(&helper, &args)
        .expect("VM i64 slice fast path evaluates");

    assert_eq!(value, Some(7));
    assert_eq!(backend.stats().arg_stack_packs, 0);
    assert_eq!(backend.stats().arg_bytes_copied, 0);
    assert_eq!(
        backend.stats().arg_bytes_borrowed,
        2 * std::mem::size_of::<i64>()
    );
    assert_eq!(backend.stats().result_bytes_copied, 0);
}

#[test]
fn vm_pure_scratch_reuses_and_rebuilds_i64_root_bindings() {
    let add = RuntimePureHelper {
        id: RuntimePureHelperId(0),
        name: "add".to_owned(),
        input_names: vec!["base".to_owned(), "bonus".to_owned()],
        input_types: vec![RuntimePureInputType::I64, RuntimePureInputType::I64],
        output_type: RuntimePureOutputType::I64,
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
        input_types: vec![RuntimePureInputType::I64],
        output_type: RuntimePureOutputType::I64,
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
        input_types: vec![RuntimePureInputType::Value],
        output_type: RuntimePureOutputType::Value,
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
        input_types: vec![RuntimePureInputType::I64, RuntimePureInputType::I64],
        output_type: RuntimePureOutputType::I64,
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
    assert_eq!(backend.stats().flat_batch_calls, 0);
    assert_eq!(backend.stats().flat_batch_items, 0);
    assert_eq!(backend.stats().flatten_materializations, 0);
    assert_eq!(backend.stats().flatten_bytes_copied, 0);
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
        input_types: vec![RuntimePureInputType::I64, RuntimePureInputType::I64],
        output_type: RuntimePureOutputType::I64,
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
    assert_eq!(backend.stats().flat_batch_calls, 1);
    assert_eq!(backend.stats().flat_batch_items, 2);
    assert_eq!(
        backend.stats().flat_batch_bytes_borrowed,
        std::mem::size_of_val(&inputs)
    );
    assert_eq!(backend.stats().flatten_materializations, 0);
    assert_eq!(backend.stats().flatten_bytes_copied, 0);
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
fn vm_runtime_i32_flat_batch_preserves_input_and_output_width() {
    let helper = RuntimePureHelper {
        id: RuntimePureHelperId(0),
        name: "score_i32".to_owned(),
        input_names: vec!["base".to_owned(), "bonus".to_owned()],
        input_types: vec![RuntimePureInputType::I32, RuntimePureInputType::I32],
        output_type: RuntimePureOutputType::I32,
        expr: RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
        },
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Annotated,
    };
    let mut backend = VmRuntimePureCallBackend::default();
    let inputs = [3_i32, 4, 5, 6];
    let mut out = [0_i32; 2];

    backend
        .call_i32_flat_batch(&helper, &inputs, 2, &mut out)
        .expect("VM flat i32 batch evaluates");

    assert_eq!(out, [12, 30]);
    assert_eq!(backend.stats().batch_calls, 1);
    assert_eq!(backend.stats().batch_items, 2);
    assert_eq!(backend.stats().flat_batch_calls, 1);
    assert_eq!(backend.stats().flat_batch_items, 2);
    assert_eq!(
        backend.stats().flat_batch_bytes_borrowed,
        std::mem::size_of_val(&inputs)
    );
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
fn vm_runtime_f32_and_f64_slice_calls_preserve_float_width() {
    let f32_helper = RuntimePureHelper {
        id: RuntimePureHelperId(0),
        name: "score_f32".to_owned(),
        input_names: vec!["base".to_owned(), "gain".to_owned()],
        input_types: vec![RuntimePureInputType::F32, RuntimePureInputType::F32],
        output_type: RuntimePureOutputType::F32,
        expr: RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Local("gain".to_owned())),
        },
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Annotated,
    };
    let f64_helper = RuntimePureHelper {
        id: RuntimePureHelperId(1),
        name: "score_f64".to_owned(),
        input_names: vec!["base".to_owned(), "gain".to_owned()],
        input_types: vec![RuntimePureInputType::F64, RuntimePureInputType::F64],
        output_type: RuntimePureOutputType::F64,
        expr: RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Add,
            rhs: Box::new(RuntimeExpr::Local("gain".to_owned())),
        },
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Annotated,
    };
    let mut backend = VmRuntimePureCallBackend::default();
    let f32_args = [1.5_f32, 2.0];
    let f64_args = [1.5_f64, 2.0];

    let f32_value = backend
        .call_f32_slice(&f32_helper, &f32_args)
        .expect("f32 pure call evaluates")
        .expect("f32 pure call returns a value");
    let f64_value = backend
        .call_f64_slice(&f64_helper, &f64_args)
        .expect("f64 pure call evaluates")
        .expect("f64 pure call returns a value");

    assert_eq!(f32_value.to_bits(), 3.0_f32.to_bits());
    assert_eq!(f64_value.to_bits(), 3.5_f64.to_bits());
    assert_eq!(backend.stats().pure_calls, 2);
    assert_eq!(backend.stats().arg_vec_allocations, 0);
    assert_eq!(backend.stats().arg_bytes_copied, 0);
    assert_eq!(
        backend.stats().arg_bytes_borrowed,
        std::mem::size_of_val(&f32_args) + std::mem::size_of_val(&f64_args)
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
