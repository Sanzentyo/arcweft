use crate::math::{DenseMatrixF32, DenseMatrixF64, DenseTensorF32, DenseTensorF64};
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
    RuntimeBinaryOp, RuntimeBinding, RuntimeCallTarget, RuntimeEvalError, RuntimeExpr,
    RuntimeFieldExpr, RuntimeFieldValue, RuntimeIntrinsic, RuntimeSeq, RuntimeValue,
    runtime_sequence_dense_bool, runtime_sequence_dense_bytes, runtime_sequence_dense_i8,
    runtime_sequence_dense_i16, runtime_sequence_dense_i32, runtime_sequence_dense_i64,
    runtime_sequence_dense_i128, runtime_sequence_dense_isize, runtime_sequence_dense_u8,
    runtime_sequence_dense_u16, runtime_sequence_dense_u32, runtime_sequence_dense_u64,
    runtime_sequence_dense_u128, runtime_sequence_dense_usize,
    runtime_sequence_from_literal_values, runtime_sequence_values,
};

fn int_binding(name: &str, value: i64) -> RuntimeBinding {
    RuntimeBinding {
        name: name.to_owned(),
        value: RuntimeValue::i64(value),
    }
}

fn value_helper(expr: RuntimeExpr) -> RuntimePureHelper {
    RuntimePureHelper {
        id: RuntimePureHelperId(0),
        name: "project".to_owned(),
        input_names: vec!["row".to_owned()],
        input_types: vec![RuntimePureInputType::Value],
        output_type: RuntimePureOutputType::Value,
        expr,
        scalar_eval_supported: false,
        origin: RuntimePureHelperOrigin::Annotated,
    }
}

#[test]
fn vm_pure_backend_projects_record_columns_by_ordinal() {
    let helper = value_helper(RuntimeExpr::ProjectRecord {
        target: Box::new(RuntimeExpr::Local("row".to_owned())),
        ordinal: 0,
    });
    let rows = runtime_sequence_from_literal_values(vec![
        RuntimeValue::Record(vec![RuntimeFieldValue {
            name: "score".to_owned(),
            value: RuntimeValue::i64(1),
        }]),
        RuntimeValue::Record(vec![RuntimeFieldValue {
            name: "score".to_owned(),
            value: RuntimeValue::i64(2),
        }]),
    ]);
    let mut backend = VmRuntimePureCallBackend::default();

    let value = backend
        .call_values(&helper, &[rows])
        .expect("projection evaluates");

    assert!(matches!(
        value,
        RuntimeValue::Seq(seq) if seq.as_i64_slice() == Some([1, 2].as_slice())
    ));
}

#[test]
fn vm_pure_backend_projects_tuple_columns_by_ordinal() {
    let helper = value_helper(RuntimeExpr::ProjectTuple {
        target: Box::new(RuntimeExpr::Local("row".to_owned())),
        ordinal: 1,
    });
    let rows = runtime_sequence_from_literal_values(vec![
        RuntimeValue::Tuple(vec![RuntimeValue::i64(1), RuntimeValue::Bool(true)]),
        RuntimeValue::Tuple(vec![RuntimeValue::i64(2), RuntimeValue::Bool(false)]),
    ]);
    let mut backend = VmRuntimePureCallBackend::default();

    let value = backend
        .call_values(&helper, &[rows])
        .expect("projection evaluates");

    assert!(matches!(
        value,
        RuntimeValue::Seq(seq) if seq.as_bool_slice() == Some([true, false].as_slice())
    ));
}

#[test]
fn vm_pure_backend_evaluates_deterministic_helper_expr() {
    let request = PureFunctionRequest::new(
        "score",
        RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Add,
            rhs: Box::new(RuntimeExpr::Call {
                callee: RuntimeCallTarget::intrinsic(RuntimeIntrinsic::Add),
                args: vec![
                    RuntimeExpr::Local("bonus".to_owned()),
                    RuntimeExpr::Value(RuntimeValue::i64(2)),
                ],
            }),
        },
        [int_binding("base", 3), int_binding("bonus", 4)],
    );

    let result = VmPureFunctionBackend
        .evaluate(&request)
        .expect("pure helper evaluates");

    assert_eq!(result.backend, PureFunctionBackendKind::Vm);
    assert_eq!(result.value, RuntimeValue::i64(9));
    assert_eq!(result.stats.evaluated_calls, 1);
    assert_eq!(result.stats.evaluated_binary_ops, 1);
    assert!(result.stats.evaluated_exprs >= 5);
}

#[test]
fn vm_pure_backend_evaluates_builtin_matrix_and_tensor_calls() {
    let lhs = DenseMatrixF32::new(2, 3, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]).unwrap();
    let rhs = DenseMatrixF32::new(3, 2, vec![7.0, 8.0, 9.0, 10.0, 11.0, 12.0]).unwrap();
    let request = PureFunctionRequest::new(
        "matrix",
        RuntimeExpr::Call {
            callee: RuntimeCallTarget::intrinsic(RuntimeIntrinsic::MathMatmulF32),
            args: vec![
                RuntimeExpr::Value(RuntimeValue::matrix_f32(lhs)),
                RuntimeExpr::Value(RuntimeValue::matrix_f32(rhs)),
            ],
        },
        [],
    );

    let result = VmPureFunctionBackend
        .evaluate(&request)
        .expect("matrix call evaluates");

    assert!(matches!(
        result.value,
        RuntimeValue::MatrixF32(matrix)
            if matrix.shape() == crate::math::MatrixShape::new(2, 2)
                && matrix.values() == [58.0, 64.0, 139.0, 154.0]
    ));

    let lhs = DenseTensorF32::new(vec![2, 2], vec![1.0, 2.0, 3.0, 4.0]).unwrap();
    let rhs = DenseTensorF32::new(vec![2, 2], vec![5.0, 6.0, 7.0, 8.0]).unwrap();
    let request = PureFunctionRequest::new(
        "tensor",
        RuntimeExpr::Call {
            callee: RuntimeCallTarget::intrinsic(RuntimeIntrinsic::MathTensorAddF32),
            args: vec![
                RuntimeExpr::Value(RuntimeValue::tensor_f32(lhs)),
                RuntimeExpr::Value(RuntimeValue::tensor_f32(rhs)),
            ],
        },
        [],
    );

    let result = VmPureFunctionBackend
        .evaluate(&request)
        .expect("tensor call evaluates");

    assert!(matches!(
        result.value,
        RuntimeValue::TensorF32(tensor)
            if tensor.shape().dims() == [2, 2] && tensor.values() == [6.0, 8.0, 10.0, 12.0]
    ));
}

#[test]
fn vm_pure_backend_evaluates_builtin_f64_matrix_and_tensor_calls() {
    let lhs = DenseMatrixF64::new(2, 2, vec![1.5, 2.0, 3.25, 4.5]).unwrap();
    let rhs = DenseMatrixF64::new(2, 2, vec![5.0, 6.5, 7.0, 8.25]).unwrap();
    let request = PureFunctionRequest::new(
        "matrix_f64",
        RuntimeExpr::Call {
            callee: RuntimeCallTarget::intrinsic(RuntimeIntrinsic::MathMatmulF64),
            args: vec![
                RuntimeExpr::Value(RuntimeValue::matrix_f64(lhs)),
                RuntimeExpr::Value(RuntimeValue::matrix_f64(rhs)),
            ],
        },
        [],
    );

    let result = VmPureFunctionBackend
        .evaluate(&request)
        .expect("f64 matrix call evaluates");

    assert!(matches!(
        result.value,
        RuntimeValue::MatrixF64(matrix)
            if matrix.shape() == crate::math::MatrixShape::new(2, 2)
                && matrix.values() == [21.5, 26.25, 47.75, 58.25]
    ));

    let lhs = DenseTensorF64::new(vec![2, 2], vec![1.5, 2.25, 3.75, 4.5]).unwrap();
    let rhs = DenseTensorF64::new(vec![2, 2], vec![5.0, 6.25, 7.5, 8.75]).unwrap();
    let request = PureFunctionRequest::new(
        "tensor_f64",
        RuntimeExpr::Call {
            callee: RuntimeCallTarget::intrinsic(RuntimeIntrinsic::MathTensorAddF64),
            args: vec![
                RuntimeExpr::Value(RuntimeValue::tensor_f64(lhs)),
                RuntimeExpr::Value(RuntimeValue::tensor_f64(rhs)),
            ],
        },
        [],
    );

    let result = VmPureFunctionBackend
        .evaluate(&request)
        .expect("f64 tensor call evaluates");

    assert!(matches!(
        result.value,
        RuntimeValue::TensorF64(tensor)
            if tensor.shape().dims() == [2, 2] && tensor.values() == [6.5, 8.5, 11.25, 13.25]
    ));
}

#[test]
fn vm_pure_backend_evaluates_lexical_let_expr() {
    let request = PureFunctionRequest::new(
        "score_with_local",
        RuntimeExpr::Let {
            name: "boosted".to_owned(),
            expr: Box::new(RuntimeExpr::Call {
                callee: RuntimeCallTarget::intrinsic(RuntimeIntrinsic::Add),
                args: vec![
                    RuntimeExpr::Local("bonus".to_owned()),
                    RuntimeExpr::Value(RuntimeValue::i64(2)),
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

    assert_eq!(result.value, RuntimeValue::i64(18));
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
                RuntimeValue::i64(18),
                RuntimeValue::i64(15),
                RuntimeValue::i64(20),
            ]),
        }],
    );

    let result = VmPureFunctionBackend
        .evaluate(&request)
        .expect("pure helper sums local sequence");

    assert_eq!(result.value, RuntimeValue::i64(53));
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

    assert_eq!(result.value, RuntimeValue::i64(53));
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

        assert_eq!(result.value, RuntimeValue::i64(53));
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

    assert_eq!(result.value, RuntimeValue::usize(4));
    assert_eq!(result.stats.evaluated_method_calls, 1);
}

#[test]
fn vm_pure_backend_checks_sequence_contains_and_record_get() {
    let contains = PureFunctionRequest::new(
        "has_choice",
        RuntimeExpr::MethodCall {
            receiver: Box::new(RuntimeExpr::Local("actions".to_owned())),
            method: "contains".to_owned(),
            args: vec![RuntimeExpr::Record(vec![RuntimeFieldExpr {
                name: "target".to_owned(),
                value: RuntimeExpr::Value(RuntimeValue::String("choice.opening.listen".to_owned())),
            }])],
        },
        [RuntimeBinding {
            name: "actions".to_owned(),
            value: RuntimeValue::Seq(RuntimeSeq::values(vec![RuntimeValue::Record(vec![
                RuntimeFieldValue {
                    name: "target".to_owned(),
                    value: RuntimeValue::String("choice.opening.listen".to_owned()),
                },
            ])])),
        }],
    );
    let contains = VmPureFunctionBackend
        .evaluate(&contains)
        .expect("pure helper evaluates contains");
    assert_eq!(contains.value, RuntimeValue::Bool(true));

    let index = PureFunctionRequest::new(
        "first_action",
        RuntimeExpr::MethodCall {
            receiver: Box::new(RuntimeExpr::Local("actions".to_owned())),
            method: "__index".to_owned(),
            args: vec![RuntimeExpr::Value(RuntimeValue::i64(0))],
        },
        [RuntimeBinding {
            name: "actions".to_owned(),
            value: RuntimeValue::Seq(RuntimeSeq::values(vec![RuntimeValue::Record(vec![
                RuntimeFieldValue {
                    name: "target".to_owned(),
                    value: RuntimeValue::String("choice.opening.listen".to_owned()),
                },
            ])])),
        }],
    );
    let index = VmPureFunctionBackend
        .evaluate(&index)
        .expect("pure helper evaluates sequence index");
    assert_eq!(
        index.value,
        RuntimeValue::Record(vec![RuntimeFieldValue {
            name: "target".to_owned(),
            value: RuntimeValue::String("choice.opening.listen".to_owned()),
        }])
    );

    let get = PureFunctionRequest::new(
        "signal_value",
        RuntimeExpr::MethodCall {
            receiver: Box::new(RuntimeExpr::Local("signals".to_owned())),
            method: "get".to_owned(),
            args: vec![RuntimeExpr::EntityRef("signal.ready".to_owned())],
        },
        [RuntimeBinding {
            name: "signals".to_owned(),
            value: RuntimeValue::Record(vec![RuntimeFieldValue {
                name: "signal.ready".to_owned(),
                value: RuntimeValue::Bool(true),
            }]),
        }],
    );
    let get = VmPureFunctionBackend
        .evaluate(&get)
        .expect("pure helper evaluates record get");
    assert_eq!(get.value, RuntimeValue::Bool(true));
}

#[test]
fn vm_pure_backend_requires_observed_object_by_role() {
    let request = PureFunctionRequest::new(
        "dialogue_textbox",
        RuntimeExpr::MethodCall {
            receiver: Box::new(RuntimeExpr::Local("objects".to_owned())),
            method: "require_role".to_owned(),
            args: vec![RuntimeExpr::Value(RuntimeValue::String(
                "dialogue_textbox".to_owned(),
            ))],
        },
        [RuntimeBinding {
            name: "objects".to_owned(),
            value: RuntimeValue::Seq(RuntimeSeq::values(vec![
                RuntimeValue::Record(vec![
                    RuntimeFieldValue {
                        name: "id".to_owned(),
                        value: RuntimeValue::String("object.background".to_owned()),
                    },
                    RuntimeFieldValue {
                        name: "role".to_owned(),
                        value: RuntimeValue::String("background".to_owned()),
                    },
                ]),
                RuntimeValue::Record(vec![
                    RuntimeFieldValue {
                        name: "id".to_owned(),
                        value: RuntimeValue::String("object.dialogue.0.0".to_owned()),
                    },
                    RuntimeFieldValue {
                        name: "role".to_owned(),
                        value: RuntimeValue::String("dialogue_textbox".to_owned()),
                    },
                ]),
            ])),
        }],
    );

    let result = VmPureFunctionBackend
        .evaluate(&request)
        .expect("pure helper finds observed object role");

    assert!(matches!(
        result.value,
        RuntimeValue::Record(ref fields)
            if fields.iter().any(|field| field.name == "id"
                && field.value == RuntimeValue::String("object.dialogue.0.0".to_owned()))
    ));
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
            rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i64(2))),
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

    assert_eq!(first, RuntimeValue::i64(7));
    assert_eq!(second, RuntimeValue::i64(11));
    assert_eq!(third, RuntimeValue::i64(14));
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
                rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i64(2))),
            }),
            body: Box::new(RuntimeExpr::If {
                condition: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                    op: RuntimeBinaryOp::Ge,
                    rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i64(3))),
                }),
                then_expr: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                    op: RuntimeBinaryOp::Mul,
                    rhs: Box::new(RuntimeExpr::Local("boosted".to_owned())),
                }),
                else_expr: Box::new(RuntimeExpr::Value(RuntimeValue::i64(0))),
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
    assert_eq!(conformance.candidate.value, RuntimeValue::i64(18));
    assert_eq!(conformance.candidate.stats.evaluated_binary_ops, 3);
}

#[test]
fn aot_compiled_i64_plan_can_be_called_repeatedly() {
    let request = PureFunctionRequest::new(
        "score",
        RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Value(RuntimeValue::i64(20))),
            op: RuntimeBinaryOp::Add,
            rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i64(22))),
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
fn scalar_integer_overflow_is_wrapping_in_vm_and_aot() {
    let request = PureFunctionRequest::new(
        "wrap_i8",
        RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Value(RuntimeValue::i8(i8::MAX))),
            op: RuntimeBinaryOp::Add,
            rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i8(1))),
        },
        [],
    );

    let vm = VmPureFunctionBackend
        .evaluate(&request)
        .expect("VM evaluates wrapping i8 arithmetic");
    let mut slots = Vec::new();
    let aot = AotPureFunctionBackend::new()
        .compile_scalar_with_inputs(
            &request,
            std::iter::empty::<&str>(),
            RuntimePureInputType::I8,
            RuntimePureOutputType::I8,
        )
        .expect("scalar AOT compiles wrapping i8 arithmetic")
        .call_exact_int_with_inputs_scratch::<i8>(&[], &mut slots)
        .expect("scalar AOT evaluates wrapping i8 arithmetic");

    assert_eq!(vm.value, RuntimeValue::i8(i8::MIN));
    assert_eq!(aot.0, i8::MIN);
}

#[test]
fn aot_compiled_i64_plan_accepts_runtime_inputs() {
    let request = PureFunctionRequest::new(
        "score_inputs",
        RuntimeExpr::If {
            condition: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                op: RuntimeBinaryOp::Ge,
                rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i64(3))),
            }),
            then_expr: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                op: RuntimeBinaryOp::Mul,
                rhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
            }),
            else_expr: Box::new(RuntimeExpr::Value(RuntimeValue::i64(0))),
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
            callee: RuntimeCallTarget::from_label("play_audio"),
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
