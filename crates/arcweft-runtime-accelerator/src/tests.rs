use super::*;
#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
use arcweft_core::value::runtime_sequence_dense_u32;
use arcweft_core::{
    engine::{Engine, FlowExit, FlowFiberStatus},
    plan::{FlowOp, FlowRuntimeId, RuntimeFlow, RuntimePlan},
    step::{RuntimeStepInput, RuntimeStepOptions},
};
use arcweft_core::{
    plan::RuntimePureHelperId,
    value::{
        RuntimeBinaryOp, RuntimeCallTarget, RuntimeExpr, RuntimeFieldValue, RuntimeISizeValue,
        RuntimeSeq, RuntimeUSizeValue,
    },
};

#[test]
fn data_external_call_encodes_and_decodes_json_with_format_enum() {
    let mut accelerator =
        RuntimePureAccelerator::with_config(RuntimePureAcceleratorConfig::default(), &[]);
    let value = RuntimeValue::Seq(RuntimeSeq::Values(vec![RuntimeValue::String(
        "hello".to_owned(),
    )]));
    let format = RuntimeValue::Variant {
        path: None,
        name: "Json".to_owned(),
        payload: None,
    };

    let encoded = accelerator
        .call_external(
            &RuntimeCallTarget::from_label("data.encode"),
            &[value.clone(), format.clone()],
        )
        .expect("data encode is handled")
        .expect("data encode succeeds");
    let RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::Bytes(bytes))) = &encoded else {
        panic!("expected encoded bytes");
    };
    assert_eq!(bytes.as_slice(), br#"["hello"]"#);

    let decoded = accelerator
        .call_external(
            &RuntimeCallTarget::from_label("data.decode"),
            &[encoded, format],
        )
        .expect("data decode is handled")
        .expect("data decode succeeds");

    assert_eq!(decoded, value);
}

#[test]
fn data_external_call_round_trips_dynamic_avro() {
    let mut accelerator =
        RuntimePureAccelerator::with_config(RuntimePureAcceleratorConfig::default(), &[]);
    let value = RuntimeValue::Record(vec![arcweft_core::value::RuntimeFieldValue {
        name: "speaker".to_owned(),
        value: RuntimeValue::String("alice".to_owned()),
    }]);
    let format = RuntimeValue::Variant {
        path: None,
        name: "Avro".to_owned(),
        payload: None,
    };

    let encoded = accelerator
        .call_external(
            &RuntimeCallTarget::from_label("data.encode"),
            &[value.clone(), format.clone()],
        )
        .expect("data encode is handled")
        .expect("data encode succeeds");
    let decoded = accelerator
        .call_external(
            &RuntimeCallTarget::from_label("data.decode"),
            &[encoded, format],
        )
        .expect("data decode is handled")
        .expect("data decode succeeds");

    assert_eq!(decoded, value);
}

#[test]
fn data_external_call_encodes_shape_required_formats_and_rejects_dynamic_decode() {
    for variant in ["Csv", "ArrowIpc", "Parquet", "ArcweftBinary"] {
        let mut accelerator =
            RuntimePureAccelerator::with_config(RuntimePureAcceleratorConfig::default(), &[]);
        let value = RuntimeValue::Seq(RuntimeSeq::Values(vec![RuntimeValue::Record(vec![
            RuntimeFieldValue {
                name: "line".to_owned(),
                value: RuntimeValue::String("hello".to_owned()),
            },
            RuntimeFieldValue {
                name: "speaker".to_owned(),
                value: RuntimeValue::String("alice".to_owned()),
            },
        ])]));
        let format = RuntimeValue::Variant {
            path: None,
            name: variant.to_owned(),
            payload: None,
        };

        let encoded = accelerator
            .call_external(
                &RuntimeCallTarget::from_label("data.encode"),
                &[value.clone(), format.clone()],
            )
            .unwrap_or_else(|| panic!("{variant} data encode is handled"))
            .unwrap_or_else(|error| panic!("{variant} data encode succeeds: {error}"));
        let error = accelerator
            .call_external(
                &RuntimeCallTarget::from_label("data.decode"),
                &[encoded, format],
            )
            .unwrap_or_else(|| panic!("{variant} data decode is handled"))
            .expect_err("shape-required formats need an explicit decode shape");

        let RuntimeEvalError::UnsupportedPure { reason, .. } = error else {
            panic!("expected UnsupportedPure for {variant} dynamic decode");
        };
        assert!(
            reason.contains("requires an explicit TypeShape"),
            "{variant} should explain why dynamic decode is unavailable: {reason}"
        );
    }
}

#[test]
fn data_external_call_decodes_shape_required_formats_with_explicit_shape() {
    for variant in ["Csv", "ArrowIpc", "Parquet", "ArcweftBinary"] {
        let mut accelerator =
            RuntimePureAccelerator::with_config(RuntimePureAcceleratorConfig::default(), &[]);
        let value = RuntimeValue::Seq(RuntimeSeq::Values(vec![RuntimeValue::Record(vec![
            RuntimeFieldValue {
                name: "line".to_owned(),
                value: RuntimeValue::String("hello".to_owned()),
            },
            RuntimeFieldValue {
                name: "speaker".to_owned(),
                value: RuntimeValue::String("alice".to_owned()),
            },
        ])]));
        let format = RuntimeValue::Variant {
            path: None,
            name: variant.to_owned(),
            payload: None,
        };
        let shape = accelerator
            .call_external(
                &RuntimeCallTarget::from_label("data.shape"),
                std::slice::from_ref(&value),
            )
            .unwrap_or_else(|| panic!("{variant} data shape is handled"))
            .unwrap_or_else(|error| panic!("{variant} data shape succeeds: {error}"));
        let encoded = accelerator
            .call_external(
                &RuntimeCallTarget::from_label("data.encode"),
                &[value.clone(), format.clone()],
            )
            .unwrap_or_else(|| panic!("{variant} data encode is handled"))
            .unwrap_or_else(|error| panic!("{variant} data encode succeeds: {error}"));
        let decoded = accelerator
            .call_external(
                &RuntimeCallTarget::from_label("data.decode"),
                &[encoded, format, shape],
            )
            .unwrap_or_else(|| panic!("{variant} data decode is handled"))
            .unwrap_or_else(|error| panic!("{variant} data decode succeeds: {error}"));

        assert_eq!(decoded, value, "{variant} explicit shape decode roundtrip");
    }
}

#[test]
fn runtime_flow_external_inference_call_sequence_uses_adapter_boundary() {
    let image = DenseTensorF32::new(
        vec![1, 1, 4, 4],
        vec![
            8.0, 1.0, 1.0, 1.0, 1.0, 4.0, 1.0, 1.0, 1.0, 1.0, 2.0, 1.0, 1.0, 1.0, 1.0, 1.0,
        ],
    )
    .expect("image tensor shape is valid");
    let kernel = DenseTensorF32::new(vec![1, 1, 2, 2], vec![1.0, 0.0, 0.0, -1.0])
        .expect("kernel tensor shape is valid");
    let dense = DenseTensorF32::new(vec![4, 2], vec![0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 2.0])
        .expect("dense tensor shape is valid");
    let bias = DenseTensorF32::new(vec![2], vec![0.0, 0.0]).expect("bias tensor shape is valid");
    let conv_target = RuntimeCallTarget::from_label("conv2d.valid_f32");
    assert!(matches!(conv_target, RuntimeCallTarget::Named(_)));
    let plan = RuntimePlan::new(
        Some(FlowRuntimeId("flow.infer".to_owned())),
        vec![RuntimeFlow {
            id: FlowRuntimeId("flow.infer".to_owned()),
            ops: vec![FlowOp::ReturnExpr(RuntimeExpr::Let {
                name: "conv".to_owned(),
                expr: Box::new(RuntimeExpr::Call {
                    callee: conv_target,
                    args: vec![
                        RuntimeExpr::Value(RuntimeValue::tensor_f32(image)),
                        RuntimeExpr::Value(RuntimeValue::tensor_f32(kernel)),
                        RuntimeExpr::Value(RuntimeValue::usize(1)),
                        RuntimeExpr::Value(RuntimeValue::usize(1)),
                    ],
                }),
                body: Box::new(RuntimeExpr::Let {
                    name: "pooled".to_owned(),
                    expr: Box::new(RuntimeExpr::Call {
                        callee: RuntimeCallTarget::from_label("infer.max_pool2d_f32"),
                        args: vec![
                            RuntimeExpr::Call {
                                callee: RuntimeCallTarget::from_label("infer.relu_f32"),
                                args: vec![RuntimeExpr::Local("conv".to_owned())],
                            },
                            RuntimeExpr::Value(RuntimeValue::usize(2)),
                            RuntimeExpr::Value(RuntimeValue::usize(2)),
                            RuntimeExpr::Value(RuntimeValue::usize(1)),
                            RuntimeExpr::Value(RuntimeValue::usize(1)),
                        ],
                    }),
                    body: Box::new(RuntimeExpr::Let {
                        name: "logits".to_owned(),
                        expr: Box::new(RuntimeExpr::Call {
                            callee: RuntimeCallTarget::from_label("infer.matmul_bias_add_f32"),
                            args: vec![
                                RuntimeExpr::Call {
                                    callee: RuntimeCallTarget::from_label(
                                        "infer.flatten_outer_f32",
                                    ),
                                    args: vec![RuntimeExpr::Local("pooled".to_owned())],
                                },
                                RuntimeExpr::Value(RuntimeValue::tensor_f32(dense)),
                                RuntimeExpr::Value(RuntimeValue::tensor_f32(bias)),
                            ],
                        }),
                        body: Box::new(RuntimeExpr::Call {
                            callee: RuntimeCallTarget::from_label("infer.argmax_last_dim_f32"),
                            args: vec![RuntimeExpr::Local("logits".to_owned())],
                        }),
                    }),
                }),
            })],
        }],
        Vec::new(),
    )
    .expect("runtime plan is valid");
    let mut engine = Engine::new(plan);
    let mut accelerator = RuntimePureAccelerator::new(RuntimePureBackendMode::Auto, &[]);

    let result = engine.step_with_pure_backend(
        RuntimeStepInput::default(),
        RuntimeStepOptions::default(),
        &mut accelerator,
    );

    assert_eq!(result.stats.pure.math_calls, 6);
    assert!(matches!(
        result.fiber_status,
        FlowFiberStatus::Done(FlowExit::Return(summary)) if summary == "seq/usize/1"
    ));
    assert!(matches!(
        RuntimeExternalCallBackend::call_external(
            &mut accelerator,
            &RuntimeCallTarget::from_label("infer.argmax_last_dim_f32"),
            &[RuntimeValue::tensor_f32(
                DenseTensorF32::new(vec![1, 2], vec![0.0, 1.0]).unwrap()
            )]
        ),
        Some(Ok(RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::USize(values)))))
            if values.as_slice()[0].get() == 1
    ));
}

#[cfg(feature = "math-glam")]
#[test]
fn runtime_flow_math_intrinsic_uses_adapter_math_accelerator() {
    let lhs = DenseMatrixF32::new(
        4,
        4,
        vec![
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ],
    )
    .expect("matrix shape is valid");
    let rhs = DenseMatrixF32::new(
        4,
        4,
        vec![
            1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0,
        ],
    )
    .expect("matrix shape is valid");
    let plan = RuntimePlan::new(
        Some(FlowRuntimeId("flow.math".to_owned())),
        vec![RuntimeFlow {
            id: FlowRuntimeId("flow.math".to_owned()),
            ops: vec![FlowOp::ReturnExpr(RuntimeExpr::Call {
                callee: RuntimeCallTarget::intrinsic(RuntimeIntrinsic::MathMatmulF32),
                args: vec![
                    RuntimeExpr::Value(RuntimeValue::matrix_f32(lhs)),
                    RuntimeExpr::Value(RuntimeValue::matrix_f32(rhs)),
                ],
            })],
        }],
        Vec::new(),
    )
    .expect("runtime plan is valid");
    let mut engine = Engine::new(plan);
    let mut accelerator = RuntimePureAccelerator::new(RuntimePureBackendMode::Auto, &[]);

    let result = engine.step_with_pure_backend(
        RuntimeStepInput::default(),
        RuntimeStepOptions::default(),
        &mut accelerator,
    );

    assert_eq!(result.stats.pure.math_calls, 1);
    assert_eq!(result.stats.pure.math_accelerated_calls, 1);
    assert_eq!(
        accelerator.math_stats().last_backend,
        Some(math::RuntimeMathBackend::Glam)
    );
    assert!(matches!(
        result.fiber_status,
        FlowFiberStatus::Done(FlowExit::Return(_))
    ));
}

#[cfg(feature = "math-ndarray")]
#[test]
fn runtime_flow_f64_math_intrinsic_uses_width_preserving_adapter_backend() {
    let lhs = DenseMatrixF64::new(2, 2, vec![1.5, 2.0, 3.25, 4.5]).expect("matrix shape is valid");
    let rhs = DenseMatrixF64::new(2, 2, vec![5.0, 6.5, 7.0, 8.25]).expect("matrix shape is valid");
    let plan = RuntimePlan::new(
        Some(FlowRuntimeId("flow.math_f64".to_owned())),
        vec![RuntimeFlow {
            id: FlowRuntimeId("flow.math_f64".to_owned()),
            ops: vec![FlowOp::ReturnExpr(RuntimeExpr::Call {
                callee: RuntimeCallTarget::intrinsic(RuntimeIntrinsic::MathMatmulF64),
                args: vec![
                    RuntimeExpr::Value(RuntimeValue::matrix_f64(lhs)),
                    RuntimeExpr::Value(RuntimeValue::matrix_f64(rhs)),
                ],
            })],
        }],
        Vec::new(),
    )
    .expect("runtime plan is valid");
    let mut engine = Engine::new(plan);
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            math: math::RuntimeMathAcceleratorConfig {
                backend: math::RuntimeMathBackend::Ndarray,
                ..math::RuntimeMathAcceleratorConfig::default()
            },
            ..RuntimePureAcceleratorConfig::default()
        },
        &[],
    );

    let result = engine.step_with_pure_backend(
        RuntimeStepInput::default(),
        RuntimeStepOptions::default(),
        &mut accelerator,
    );

    assert_eq!(result.stats.pure.math_calls, 1);
    assert_eq!(result.stats.pure.math_accelerated_calls, 1);
    assert_eq!(
        result.stats.pure.arg_bytes_borrowed,
        8 * std::mem::size_of::<f64>()
    );
    assert_eq!(
        accelerator.math_stats().last_backend,
        Some(math::RuntimeMathBackend::Ndarray)
    );
    assert!(matches!(
        result.fiber_status,
        FlowFiberStatus::Done(FlowExit::Return(summary)) if summary == "matrix/f64/2x2"
    ));
}

#[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
#[test]
fn runtime_wgpu_math_cache_reuses_prepared_matmul_buffers_across_counter_reset() {
    let lhs = DenseMatrixF32::new(16, 16, vec![1.0; 256]).expect("matrix shape is valid");
    let rhs = DenseMatrixF32::new(16, 16, vec![2.0; 256]).expect("matrix shape is valid");
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            math: math::RuntimeMathAcceleratorConfig {
                backend: math::RuntimeMathBackend::Wgpu,
                ..math::RuntimeMathAcceleratorConfig::default()
            },
            ..RuntimePureAcceleratorConfig::default()
        },
        &[],
    );

    let Ok(first) = RuntimeMathCallBackend::call_math_matmul_f32(&mut accelerator, &lhs, &rhs)
    else {
        return;
    };
    assert_eq!(first.rows(), 16);
    assert_eq!(first.cols(), 16);
    assert_eq!(accelerator.math_stats().gpu_buffer_creations, 4);

    accelerator.reset_runtime_counters();
    let second = RuntimeMathCallBackend::call_math_matmul_f32(&mut accelerator, &lhs, &rhs)
        .expect("prepared runtime math matmul cache is reusable");

    assert_eq!(second.values(), first.values());
    assert_eq!(accelerator.math_stats().wgpu_calls, 1);
    assert_eq!(accelerator.math_stats().gpu_buffer_creations, 0);
    assert_eq!(accelerator.math_stats().gpu_buffer_reuse_hits, 4);
    assert_eq!(accelerator.math_stats().gpu_reused_dispatches, 1);
    assert_eq!(accelerator.math_stats().bytes_uploaded, 0);
    assert_eq!(
        accelerator.math_stats().bytes_downloaded,
        std::mem::size_of_val(first.values())
    );
    assert_eq!(
        accelerator.stats().arg_bytes_borrowed,
        (lhs.values().len() + rhs.values().len()) * std::mem::size_of::<f32>()
    );
    assert_eq!(
        accelerator.stats().result_bytes_copied,
        std::mem::size_of_val(second.values())
    );
}

#[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
#[test]
fn runtime_wgpu_math_cache_updates_prepared_matmul_inputs_for_same_shape() {
    let lhs = DenseMatrixF32::new(16, 16, vec![1.0; 256]).expect("matrix shape is valid");
    let rhs = DenseMatrixF32::new(16, 16, vec![2.0; 256]).expect("matrix shape is valid");
    let changed_lhs = DenseMatrixF32::new(16, 16, vec![3.0; 256]).expect("matrix shape is valid");
    let changed_rhs = DenseMatrixF32::new(16, 16, vec![0.5; 256]).expect("matrix shape is valid");
    let expected = changed_lhs
        .matmul_scalar(&changed_rhs)
        .expect("scalar matmul succeeds");
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            math: math::RuntimeMathAcceleratorConfig {
                backend: math::RuntimeMathBackend::Wgpu,
                ..math::RuntimeMathAcceleratorConfig::default()
            },
            ..RuntimePureAcceleratorConfig::default()
        },
        &[],
    );

    if RuntimeMathCallBackend::call_math_matmul_f32(&mut accelerator, &lhs, &rhs).is_err() {
        return;
    }

    accelerator.reset_runtime_counters();
    let second =
        RuntimeMathCallBackend::call_math_matmul_f32(&mut accelerator, &changed_lhs, &changed_rhs)
            .expect("prepared runtime math matmul cache updates same-shape inputs");

    assert_eq!(second.values(), expected.values());
    assert_eq!(accelerator.math_stats().wgpu_calls, 1);
    assert_eq!(accelerator.math_stats().gpu_buffer_creations, 0);
    assert_eq!(accelerator.math_stats().gpu_buffer_reuse_hits, 7);
    assert_eq!(
        accelerator.math_stats().bytes_uploaded,
        (changed_lhs.values().len() + changed_rhs.values().len()) * std::mem::size_of::<f32>()
    );
}

#[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
#[test]
fn runtime_external_infer_matmul_bias_add_reuses_prepared_wgpu_buffers() {
    let lhs = DenseTensorF32::new(vec![16, 16], vec![1.0; 256]).expect("tensor shape is valid");
    let rhs = DenseTensorF32::new(vec![16, 16], vec![2.0; 256]).expect("tensor shape is valid");
    let bias = DenseTensorF32::new(vec![16], vec![0.25; 16]).expect("bias shape is valid");
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            math: math::RuntimeMathAcceleratorConfig {
                backend: math::RuntimeMathBackend::Wgpu,
                ..math::RuntimeMathAcceleratorConfig::default()
            },
            ..RuntimePureAcceleratorConfig::default()
        },
        &[],
    );
    let target = RuntimeCallTarget::from_label("infer.matmul_bias_add_f32");

    let Some(Ok(first)) = RuntimeExternalCallBackend::call_external(
        &mut accelerator,
        &target,
        &[
            RuntimeValue::tensor_f32(lhs.clone()),
            RuntimeValue::tensor_f32(rhs.clone()),
            RuntimeValue::tensor_f32(bias.clone()),
        ],
    ) else {
        return;
    };
    let RuntimeValue::TensorF32(first) = first else {
        panic!("matmul-bias external call returns a tensor");
    };
    assert_eq!(first.shape().dims(), &[16, 16]);
    assert_eq!(accelerator.math_stats().gpu_buffer_creations, 7);

    accelerator.reset_runtime_counters();
    let expected_arg_bytes = (lhs.values().len() + rhs.values().len() + bias.values().len())
        * std::mem::size_of::<f32>();
    let Some(Ok(second)) = RuntimeExternalCallBackend::call_external(
        &mut accelerator,
        &target,
        &[
            RuntimeValue::tensor_f32(lhs),
            RuntimeValue::tensor_f32(rhs),
            RuntimeValue::tensor_f32(bias),
        ],
    ) else {
        panic!("prepared matmul-bias external call cache is reusable");
    };
    let RuntimeValue::TensorF32(second) = second else {
        panic!("matmul-bias external call returns a tensor");
    };

    assert_eq!(second.values(), first.values());
    assert_eq!(accelerator.math_stats().wgpu_calls, 1);
    assert_eq!(accelerator.math_stats().fused_matmul_bias_add_calls, 1);
    assert_eq!(accelerator.math_stats().gpu_buffer_creations, 0);
    assert_eq!(accelerator.math_stats().gpu_buffer_reuse_hits, 7);
    assert_eq!(accelerator.math_stats().gpu_reused_dispatches, 1);
    assert_eq!(accelerator.math_stats().bytes_uploaded, 0);
    assert_eq!(
        accelerator.math_stats().bytes_downloaded,
        std::mem::size_of_val(second.values())
    );
    assert_eq!(accelerator.stats().arg_bytes_borrowed, expected_arg_bytes);
    assert_eq!(
        accelerator.stats().result_bytes_copied,
        std::mem::size_of_val(second.values())
    );
}

#[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
#[test]
fn runtime_auto_wgpu_matmul_uses_prepared_cache_when_threshold_selects_gpu() {
    let lhs = DenseMatrixF32::new(8, 8, vec![1.0; 64]).expect("matrix shape is valid");
    let rhs = DenseMatrixF32::new(8, 8, vec![2.0; 64]).expect("matrix shape is valid");
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            math: math::RuntimeMathAcceleratorConfig {
                backend: math::RuntimeMathBackend::Auto,
                wgpu_min_elements: 1,
            },
            ..RuntimePureAcceleratorConfig::default()
        },
        &[],
    );

    let Ok(first) = RuntimeMathCallBackend::call_math_matmul_f32(&mut accelerator, &lhs, &rhs)
    else {
        return;
    };
    assert_eq!(
        accelerator.math_stats().last_auto_reason,
        Some(math::RuntimeMathAutoSelectionReason::MatmulWgpuWorkThreshold)
    );

    accelerator.reset_runtime_counters();
    let second = RuntimeMathCallBackend::call_math_matmul_f32(&mut accelerator, &lhs, &rhs)
        .expect("auto-selected wgpu matmul reuses prepared runtime cache");

    assert_eq!(second.values(), first.values());
    assert_eq!(
        accelerator.math_stats().last_auto_reason,
        Some(math::RuntimeMathAutoSelectionReason::MatmulWgpuWorkThreshold)
    );
    assert_eq!(accelerator.math_stats().wgpu_calls, 1);
    assert_eq!(accelerator.math_stats().gpu_buffer_creations, 0);
    assert_eq!(accelerator.math_stats().gpu_buffer_reuse_hits, 4);
    assert_eq!(accelerator.math_stats().bytes_uploaded, 0);
}

#[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
#[test]
fn runtime_auto_wgpu_matmul_reuses_capacity_cache_for_smaller_shape() {
    let lhs = DenseMatrixF32::new(16, 16, vec![1.0; 256]).expect("matrix shape is valid");
    let rhs = DenseMatrixF32::new(16, 16, vec![2.0; 256]).expect("matrix shape is valid");
    let smaller_lhs = DenseMatrixF32::new(8, 8, vec![3.0; 64]).expect("matrix shape is valid");
    let smaller_rhs = DenseMatrixF32::new(8, 8, vec![0.5; 64]).expect("matrix shape is valid");
    let expected = smaller_lhs
        .matmul_scalar(&smaller_rhs)
        .expect("scalar matmul succeeds");
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            math: math::RuntimeMathAcceleratorConfig {
                backend: math::RuntimeMathBackend::Auto,
                wgpu_min_elements: 1,
            },
            ..RuntimePureAcceleratorConfig::default()
        },
        &[],
    );

    if RuntimeMathCallBackend::call_math_matmul_f32(&mut accelerator, &lhs, &rhs).is_err() {
        return;
    }

    accelerator.reset_runtime_counters();
    let second =
        RuntimeMathCallBackend::call_math_matmul_f32(&mut accelerator, &smaller_lhs, &smaller_rhs)
            .expect("auto-selected wgpu matmul reuses capacity-prepared runtime cache");

    assert_eq!(second.values(), expected.values());
    assert_eq!(
        accelerator.math_stats().last_auto_reason,
        Some(math::RuntimeMathAutoSelectionReason::MatmulWgpuWorkThreshold)
    );
    assert_eq!(accelerator.math_stats().wgpu_calls, 1);
    assert_eq!(accelerator.math_stats().gpu_buffer_creations, 0);
    assert_eq!(accelerator.math_stats().gpu_buffer_reuse_hits, 7);
    assert_eq!(
        accelerator.math_stats().bytes_uploaded,
        (smaller_lhs.values().len() + smaller_rhs.values().len()) * std::mem::size_of::<f32>()
    );
    assert_eq!(
        accelerator.math_stats().bytes_downloaded,
        std::mem::size_of_val(second.values())
    );
}

#[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
#[test]
fn runtime_wgpu_math_cache_reuses_prepared_tensor_add_buffers() {
    let lhs = DenseTensorF32::new(vec![32], vec![1.0; 32]).expect("tensor shape is valid");
    let rhs = DenseTensorF32::new(vec![32], vec![2.0; 32]).expect("tensor shape is valid");
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            math: math::RuntimeMathAcceleratorConfig {
                backend: math::RuntimeMathBackend::Wgpu,
                ..math::RuntimeMathAcceleratorConfig::default()
            },
            ..RuntimePureAcceleratorConfig::default()
        },
        &[],
    );

    let Ok(first) = RuntimeMathCallBackend::call_math_tensor_add_f32(&mut accelerator, &lhs, &rhs)
    else {
        return;
    };
    assert_eq!(first.values(), vec![3.0; 32].as_slice());

    accelerator.reset_runtime_counters();
    let second = RuntimeMathCallBackend::call_math_tensor_add_f32(&mut accelerator, &lhs, &rhs)
        .expect("prepared runtime tensor add cache is reusable");

    assert_eq!(second.values(), vec![3.0; 32].as_slice());
    assert_eq!(accelerator.math_stats().wgpu_calls, 1);
    assert_eq!(accelerator.math_stats().gpu_buffer_creations, 0);
    assert_eq!(accelerator.math_stats().gpu_buffer_reuse_hits, 4);
    assert_eq!(accelerator.math_stats().bytes_uploaded, 0);
    assert_eq!(
        accelerator.math_stats().bytes_downloaded,
        std::mem::size_of_val(second.values())
    );
}

#[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
#[test]
fn runtime_auto_wgpu_matrix_add_uses_prepared_cache_when_threshold_selects_gpu() {
    let lhs = DenseMatrixF32::new(8, 8, vec![1.0; 64]).expect("matrix shape is valid");
    let rhs = DenseMatrixF32::new(8, 8, vec![2.0; 64]).expect("matrix shape is valid");
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            math: math::RuntimeMathAcceleratorConfig {
                backend: math::RuntimeMathBackend::Auto,
                wgpu_min_elements: 1,
            },
            ..RuntimePureAcceleratorConfig::default()
        },
        &[],
    );

    let Ok(first) = RuntimeMathCallBackend::call_math_matrix_add_f32(&mut accelerator, &lhs, &rhs)
    else {
        return;
    };
    assert_eq!(
        accelerator.math_stats().last_auto_reason,
        Some(math::RuntimeMathAutoSelectionReason::ElementwiseWgpuWorkThreshold)
    );

    accelerator.reset_runtime_counters();
    let second = RuntimeMathCallBackend::call_math_matrix_add_f32(&mut accelerator, &lhs, &rhs)
        .expect("auto-selected wgpu matrix add reuses prepared runtime cache");

    assert_eq!(second.values(), first.values());
    assert_eq!(
        accelerator.math_stats().last_auto_reason,
        Some(math::RuntimeMathAutoSelectionReason::ElementwiseWgpuWorkThreshold)
    );
    assert_eq!(accelerator.math_stats().wgpu_calls, 1);
    assert_eq!(accelerator.math_stats().gpu_buffer_creations, 0);
    assert_eq!(accelerator.math_stats().gpu_buffer_reuse_hits, 4);
    assert_eq!(accelerator.math_stats().bytes_uploaded, 0);
}

#[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
#[test]
fn runtime_auto_wgpu_matrix_add_reuses_capacity_cache_for_smaller_shape() {
    let lhs = DenseMatrixF32::new(16, 16, vec![1.0; 256]).expect("matrix shape is valid");
    let rhs = DenseMatrixF32::new(16, 16, vec![2.0; 256]).expect("matrix shape is valid");
    let smaller_lhs = DenseMatrixF32::new(8, 8, vec![4.0; 64]).expect("matrix shape is valid");
    let smaller_rhs = DenseMatrixF32::new(8, 8, vec![5.0; 64]).expect("matrix shape is valid");
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            math: math::RuntimeMathAcceleratorConfig {
                backend: math::RuntimeMathBackend::Auto,
                wgpu_min_elements: 1,
            },
            ..RuntimePureAcceleratorConfig::default()
        },
        &[],
    );

    if RuntimeMathCallBackend::call_math_matrix_add_f32(&mut accelerator, &lhs, &rhs).is_err() {
        return;
    }

    accelerator.reset_runtime_counters();
    let second = RuntimeMathCallBackend::call_math_matrix_add_f32(
        &mut accelerator,
        &smaller_lhs,
        &smaller_rhs,
    )
    .expect("auto-selected wgpu matrix add reuses capacity-prepared runtime cache");

    assert_eq!(second.values(), vec![9.0; 64].as_slice());
    assert_eq!(
        accelerator.math_stats().last_auto_reason,
        Some(math::RuntimeMathAutoSelectionReason::ElementwiseWgpuWorkThreshold)
    );
    assert_eq!(accelerator.math_stats().wgpu_calls, 1);
    assert_eq!(accelerator.math_stats().gpu_buffer_creations, 0);
    assert_eq!(accelerator.math_stats().gpu_buffer_reuse_hits, 7);
    assert_eq!(
        accelerator.math_stats().bytes_uploaded,
        (smaller_lhs.values().len() + smaller_rhs.values().len()) * std::mem::size_of::<f32>()
    );
    assert_eq!(
        accelerator.math_stats().bytes_downloaded,
        std::mem::size_of_val(second.values())
    );
}

#[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
#[test]
fn runtime_wgpu_math_cache_updates_prepared_matrix_add_inputs_for_same_shape() {
    let lhs = DenseMatrixF32::new(8, 8, vec![1.0; 64]).expect("matrix shape is valid");
    let rhs = DenseMatrixF32::new(8, 8, vec![2.0; 64]).expect("matrix shape is valid");
    let changed_lhs = DenseMatrixF32::new(8, 8, vec![4.0; 64]).expect("matrix shape is valid");
    let changed_rhs = DenseMatrixF32::new(8, 8, vec![5.0; 64]).expect("matrix shape is valid");
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            math: math::RuntimeMathAcceleratorConfig {
                backend: math::RuntimeMathBackend::Wgpu,
                ..math::RuntimeMathAcceleratorConfig::default()
            },
            ..RuntimePureAcceleratorConfig::default()
        },
        &[],
    );

    if RuntimeMathCallBackend::call_math_matrix_add_f32(&mut accelerator, &lhs, &rhs).is_err() {
        return;
    }

    accelerator.reset_runtime_counters();
    let second = RuntimeMathCallBackend::call_math_matrix_add_f32(
        &mut accelerator,
        &changed_lhs,
        &changed_rhs,
    )
    .expect("prepared runtime matrix add cache updates same-shape inputs");

    assert_eq!(second.values(), vec![9.0; 64].as_slice());
    assert_eq!(accelerator.math_stats().wgpu_calls, 1);
    assert_eq!(accelerator.math_stats().gpu_buffer_creations, 0);
    assert_eq!(accelerator.math_stats().gpu_buffer_reuse_hits, 7);
    assert_eq!(
        accelerator.math_stats().bytes_uploaded,
        (changed_lhs.values().len() + changed_rhs.values().len()) * std::mem::size_of::<f32>()
    );
}

#[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
#[test]
fn runtime_auto_wgpu_tensor_add_uses_prepared_cache_when_threshold_selects_gpu() {
    let lhs = DenseTensorF32::new(vec![64], vec![1.0; 64]).expect("tensor shape is valid");
    let rhs = DenseTensorF32::new(vec![64], vec![2.0; 64]).expect("tensor shape is valid");
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            math: math::RuntimeMathAcceleratorConfig {
                backend: math::RuntimeMathBackend::Auto,
                wgpu_min_elements: 1,
            },
            ..RuntimePureAcceleratorConfig::default()
        },
        &[],
    );

    let Ok(first) = RuntimeMathCallBackend::call_math_tensor_add_f32(&mut accelerator, &lhs, &rhs)
    else {
        return;
    };
    assert_eq!(
        accelerator.math_stats().last_auto_reason,
        Some(math::RuntimeMathAutoSelectionReason::ElementwiseWgpuWorkThreshold)
    );

    accelerator.reset_runtime_counters();
    let second = RuntimeMathCallBackend::call_math_tensor_add_f32(&mut accelerator, &lhs, &rhs)
        .expect("auto-selected wgpu tensor add reuses prepared runtime cache");

    assert_eq!(second.values(), first.values());
    assert_eq!(
        accelerator.math_stats().last_auto_reason,
        Some(math::RuntimeMathAutoSelectionReason::ElementwiseWgpuWorkThreshold)
    );
    assert_eq!(accelerator.math_stats().wgpu_calls, 1);
    assert_eq!(accelerator.math_stats().gpu_buffer_creations, 0);
    assert_eq!(accelerator.math_stats().gpu_buffer_reuse_hits, 4);
    assert_eq!(accelerator.math_stats().bytes_uploaded, 0);
}

#[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
#[test]
fn runtime_auto_wgpu_tensor_add_reuses_capacity_cache_for_smaller_len() {
    let lhs = DenseTensorF32::new(vec![128], vec![1.0; 128]).expect("tensor shape is valid");
    let rhs = DenseTensorF32::new(vec![128], vec![2.0; 128]).expect("tensor shape is valid");
    let smaller_lhs = DenseTensorF32::new(vec![64], vec![6.0; 64]).expect("tensor shape is valid");
    let smaller_rhs = DenseTensorF32::new(vec![64], vec![7.0; 64]).expect("tensor shape is valid");
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            math: math::RuntimeMathAcceleratorConfig {
                backend: math::RuntimeMathBackend::Auto,
                wgpu_min_elements: 1,
            },
            ..RuntimePureAcceleratorConfig::default()
        },
        &[],
    );

    if RuntimeMathCallBackend::call_math_tensor_add_f32(&mut accelerator, &lhs, &rhs).is_err() {
        return;
    }

    accelerator.reset_runtime_counters();
    let second = RuntimeMathCallBackend::call_math_tensor_add_f32(
        &mut accelerator,
        &smaller_lhs,
        &smaller_rhs,
    )
    .expect("auto-selected wgpu tensor add reuses capacity-prepared runtime cache");

    assert_eq!(second.values(), vec![13.0; 64].as_slice());
    assert_eq!(
        accelerator.math_stats().last_auto_reason,
        Some(math::RuntimeMathAutoSelectionReason::ElementwiseWgpuWorkThreshold)
    );
    assert_eq!(accelerator.math_stats().wgpu_calls, 1);
    assert_eq!(accelerator.math_stats().gpu_buffer_creations, 0);
    assert_eq!(accelerator.math_stats().gpu_buffer_reuse_hits, 7);
    assert_eq!(
        accelerator.math_stats().bytes_uploaded,
        (smaller_lhs.values().len() + smaller_rhs.values().len()) * std::mem::size_of::<f32>()
    );
    assert_eq!(
        accelerator.math_stats().bytes_downloaded,
        std::mem::size_of_val(second.values())
    );
}

#[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
#[test]
fn runtime_wgpu_math_cache_updates_prepared_tensor_add_inputs_for_same_shape() {
    let lhs = DenseTensorF32::new(vec![64], vec![1.0; 64]).expect("tensor shape is valid");
    let rhs = DenseTensorF32::new(vec![64], vec![2.0; 64]).expect("tensor shape is valid");
    let changed_lhs = DenseTensorF32::new(vec![64], vec![6.0; 64]).expect("tensor shape is valid");
    let changed_rhs = DenseTensorF32::new(vec![64], vec![7.0; 64]).expect("tensor shape is valid");
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            math: math::RuntimeMathAcceleratorConfig {
                backend: math::RuntimeMathBackend::Wgpu,
                ..math::RuntimeMathAcceleratorConfig::default()
            },
            ..RuntimePureAcceleratorConfig::default()
        },
        &[],
    );

    if RuntimeMathCallBackend::call_math_tensor_add_f32(&mut accelerator, &lhs, &rhs).is_err() {
        return;
    }

    accelerator.reset_runtime_counters();
    let second = RuntimeMathCallBackend::call_math_tensor_add_f32(
        &mut accelerator,
        &changed_lhs,
        &changed_rhs,
    )
    .expect("prepared runtime tensor add cache updates same-shape inputs");

    assert_eq!(second.values(), vec![13.0; 64].as_slice());
    assert_eq!(accelerator.math_stats().wgpu_calls, 1);
    assert_eq!(accelerator.math_stats().gpu_buffer_creations, 0);
    assert_eq!(accelerator.math_stats().gpu_buffer_reuse_hits, 7);
    assert_eq!(
        accelerator.math_stats().bytes_uploaded,
        (changed_lhs.values().len() + changed_rhs.values().len()) * std::mem::size_of::<f32>()
    );
}

#[test]
fn auto_accelerator_uses_aot_for_cold_scalar_calls_without_value_vec_allocation() {
    let helper = RuntimePureHelper {
        id: RuntimePureHelperId(0),
        name: "score".to_owned(),
        input_names: vec!["base".to_owned(), "bonus".to_owned()],
        input_types: vec![RuntimePureInputType::I64, RuntimePureInputType::I64],
        output_type: RuntimePureOutputType::I64,
        expr: RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
                op: RuntimeBinaryOp::Add,
                rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i64(2))),
            }),
        },
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Annotated,
    };
    let mut accelerator =
        RuntimePureAccelerator::new(RuntimePureBackendMode::Auto, std::slice::from_ref(&helper));

    let value = accelerator
        .call_i64(&helper, RuntimeI64Args::new([3, 4, 0, 0], 2))
        .expect("accelerated call succeeds");

    assert_eq!(value, Some(18));
    assert_eq!(accelerator.stats().pure_calls, 1);
    assert_eq!(accelerator.stats().arg_stack_packs, 1);
    assert_eq!(accelerator.stats().arg_vec_allocations, 0);
    assert_eq!(
        accelerator.stats().arg_bytes_copied,
        2 * std::mem::size_of::<i64>()
    );
    assert_eq!(accelerator.stats().result_bytes_copied, 0);
    assert!(accelerator.resolved_worker_count() >= 1);
    assert!(!accelerator.has_worker_pool());
    assert_eq!(accelerator.summary().aot, 1);
    assert_eq!(accelerator.summary().jit, 0);
    assert_eq!(accelerator.compile_stats().auto_aot_selected, 1);
    assert_eq!(accelerator.compile_stats().auto_jit_deferred, 1);
    assert_eq!(accelerator.compile_stats().object_attempts, 0);
    assert_eq!(accelerator.compile_stats().object_bytes, 0);
}

#[test]
fn aot_scalar_preserves_i32_and_f32_without_vm_fallback() {
    let i32_helper = RuntimePureHelper {
        id: RuntimePureHelperId(0),
        name: "i32_score".to_owned(),
        input_names: vec!["base".to_owned(), "bonus".to_owned()],
        input_types: vec![RuntimePureInputType::I32, RuntimePureInputType::I32],
        output_type: RuntimePureOutputType::I32,
        expr: RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Add,
            rhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
        },
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Annotated,
    };
    let f32_helper = RuntimePureHelper {
        id: RuntimePureHelperId(1),
        name: "f32_score".to_owned(),
        input_names: vec!["base".to_owned(), "scale".to_owned()],
        input_types: vec![RuntimePureInputType::F32, RuntimePureInputType::F32],
        output_type: RuntimePureOutputType::F32,
        expr: RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Local("scale".to_owned())),
        },
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Annotated,
    };
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            backend: RuntimePureBackendMode::Aot,
            workers: RuntimePureWorkerCount::Fixed(1),
            batch_min_len: 1024,
            emit_object_artifacts: true,
            ..RuntimePureAcceleratorConfig::default()
        },
        &[i32_helper.clone(), f32_helper.clone()],
    );

    let i32_value = accelerator
        .call_i32_slice(&i32_helper, &[7, 9])
        .expect("i32 AOT scalar succeeds");
    let f32_value = accelerator
        .call_f32_slice(&f32_helper, &[3.5, 2.0])
        .expect("f32 AOT scalar succeeds");
    let mut i32_out = [0; 3];
    accelerator
        .call_i32_flat_batch(&i32_helper, &[1, 2, 3, 4, 5, 6], 2, &mut i32_out)
        .expect("i32 AOT flat batch succeeds");
    let i32_sum = accelerator
        .call_i32_flat_batch_sum(&i32_helper, &[1, 2, 3, 4, 5, 6], 2, 3)
        .expect("i32 AOT flat batch sum succeeds");

    assert_eq!(i32_value, Some(16));
    assert_eq!(f32_value, Some(7.0));
    assert_eq!(i32_out, [3, 7, 11]);
    assert_eq!(i32_sum, 21);
    assert_eq!(accelerator.stats().aot_calls, 8);
    assert_eq!(accelerator.stats().vm_calls, 0);
    assert_eq!(accelerator.stats().fallbacks, 0);
    assert_eq!(accelerator.summary().aot, 2);
    #[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
    {
        let compile = accelerator.compile_stats();
        assert_eq!(compile.object_attempts, 2);
        assert_eq!(compile.object_successes, 2);
        assert_eq!(compile.object_failures, 0);
        assert!(compile.object_bytes > 0);
    }
    #[cfg(not(all(feature = "native-jit", not(target_arch = "wasm32"))))]
    {
        assert_eq!(accelerator.compile_stats().object_attempts, 0);
    }
}

#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
#[test]
fn explicit_jit_uses_native_i16_for_slice_and_flat_batch() {
    let helper = RuntimePureHelper {
        id: RuntimePureHelperId(0),
        name: "i16_score".to_owned(),
        input_names: vec!["base".to_owned(), "bonus".to_owned()],
        input_types: vec![RuntimePureInputType::I16, RuntimePureInputType::I16],
        output_type: RuntimePureOutputType::I16,
        expr: RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
                op: RuntimeBinaryOp::Add,
                rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i16(2))),
            }),
        },
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Annotated,
    };
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            backend: RuntimePureBackendMode::Jit,
            workers: RuntimePureWorkerCount::Fixed(1),
            batch_min_len: 1024,
            ..RuntimePureAcceleratorConfig::default()
        },
        std::slice::from_ref(&helper),
    );
    let value = accelerator
        .call_i16_slice(&helper, &[30, 4])
        .expect("native i16 JIT slice call succeeds");
    let mut out = [0; 3];
    accelerator
        .call_i16_flat_batch(&helper, &[30, 4, -20, 1, 70, 1], 2, &mut out)
        .expect("native i16 JIT flat batch succeeds");
    let sum = accelerator
        .call_i16_flat_batch_sum(&helper, &[30, 4, -20, 1, 70, 1], 2, 3)
        .expect("native i16 JIT flat batch sum succeeds");

    assert_eq!(value, Some(180));
    assert_eq!(out, [180, -60, 210]);
    assert_eq!(sum, 330);
    assert_eq!(accelerator.stats().jit_calls, 7);
    assert_eq!(accelerator.stats().aot_calls, 0);
    assert_eq!(accelerator.stats().vm_calls, 0);
    assert_eq!(accelerator.stats().fallbacks, 0);
    assert_eq!(accelerator.compile_stats().jit_attempts, 1);
    assert_eq!(accelerator.compile_stats().jit_failures, 0);
    assert_eq!(accelerator.summary().jit, 1);
}

#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
#[test]
fn explicit_jit_uses_native_i32_for_slice_and_flat_batch() {
    let helper = RuntimePureHelper {
        id: RuntimePureHelperId(0),
        name: "i32_score_jit".to_owned(),
        input_names: vec!["base".to_owned(), "bonus".to_owned()],
        input_types: vec![RuntimePureInputType::I32, RuntimePureInputType::I32],
        output_type: RuntimePureOutputType::I32,
        expr: RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
                op: RuntimeBinaryOp::Add,
                rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i32(2))),
            }),
        },
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Annotated,
    };
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            backend: RuntimePureBackendMode::Jit,
            workers: RuntimePureWorkerCount::Fixed(1),
            batch_min_len: 1024,
            ..RuntimePureAcceleratorConfig::default()
        },
        std::slice::from_ref(&helper),
    );

    let value = accelerator
        .call_i32_slice(&helper, &[3, 4])
        .expect("native i32 JIT slice call succeeds");
    let mut out = [0; 3];
    accelerator
        .call_i32_flat_batch(&helper, &[3, 4, 2, 99, 7, 1], 2, &mut out)
        .expect("native i32 JIT flat batch succeeds");
    let sum = accelerator
        .call_i32_flat_batch_sum(&helper, &[3, 4, 2, 99, 7, 1], 2, 3)
        .expect("native i32 JIT flat batch sum succeeds");

    assert_eq!(value, Some(18));
    assert_eq!(out, [18, 202, 21]);
    assert_eq!(sum, 241);
    assert_eq!(accelerator.stats().jit_calls, 7);
    assert_eq!(accelerator.stats().aot_calls, 0);
    assert_eq!(accelerator.stats().vm_calls, 0);
    assert_eq!(accelerator.summary().jit, 1);
}

#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
#[test]
fn explicit_jit_uses_native_u32_for_slice_and_flat_batch() {
    let helper = RuntimePureHelper {
        id: RuntimePureHelperId(0),
        name: "u32_score_jit".to_owned(),
        input_names: vec!["base".to_owned(), "divisor".to_owned()],
        input_types: vec![RuntimePureInputType::U32, RuntimePureInputType::U32],
        output_type: RuntimePureOutputType::U32,
        expr: RuntimeExpr::If {
            condition: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                op: RuntimeBinaryOp::Ge,
                rhs: Box::new(RuntimeExpr::Value(RuntimeValue::u32(u32::MAX - 4))),
            }),
            then_expr: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                op: RuntimeBinaryOp::Div,
                rhs: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("divisor".to_owned())),
                    op: RuntimeBinaryOp::Add,
                    rhs: Box::new(RuntimeExpr::Value(RuntimeValue::u32(1))),
                }),
            }),
            else_expr: Box::new(RuntimeExpr::Value(RuntimeValue::u32(0))),
        },
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Annotated,
    };
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            backend: RuntimePureBackendMode::Jit,
            workers: RuntimePureWorkerCount::Fixed(1),
            batch_min_len: 1024,
            ..RuntimePureAcceleratorConfig::default()
        },
        std::slice::from_ref(&helper),
    );

    let value = accelerator
        .call_u32_slice(&helper, &[u32::MAX - 1, 1])
        .expect("native u32 JIT slice call succeeds");
    let mut out = [0; 3];
    accelerator
        .call_u32_flat_batch(&helper, &[u32::MAX - 1, 1, 3, 99, u32::MAX, 4], 2, &mut out)
        .expect("native u32 JIT flat batch succeeds");
    let sum = accelerator
        .call_u32_flat_batch_sum(&helper, &[u32::MAX - 1, 1, 3, 99, u32::MAX, 4], 2, 3)
        .expect("native u32 JIT flat batch sum succeeds");

    assert_eq!(value, Some((u32::MAX - 1) / 2));
    assert_eq!(out, [(u32::MAX - 1) / 2, 0, u32::MAX / 5]);
    assert_eq!(sum, i64::from((u32::MAX - 1) / 2) + i64::from(u32::MAX / 5));
    assert_eq!(accelerator.stats().jit_calls, 7);
    assert_eq!(accelerator.stats().aot_calls, 0);
    assert_eq!(accelerator.stats().vm_calls, 0);
    assert_eq!(accelerator.summary().jit, 1);
}

#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
#[test]
fn explicit_jit_uses_native_u64_for_slice_and_flat_batch() {
    let helper = RuntimePureHelper {
        id: RuntimePureHelperId(0),
        name: "u64_score_jit".to_owned(),
        input_names: vec!["base".to_owned(), "divisor".to_owned()],
        input_types: vec![RuntimePureInputType::U64, RuntimePureInputType::U64],
        output_type: RuntimePureOutputType::U64,
        expr: RuntimeExpr::If {
            condition: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                op: RuntimeBinaryOp::Ge,
                rhs: Box::new(RuntimeExpr::Value(RuntimeValue::u64(5))),
            }),
            then_expr: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                op: RuntimeBinaryOp::Div,
                rhs: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("divisor".to_owned())),
                    op: RuntimeBinaryOp::Add,
                    rhs: Box::new(RuntimeExpr::Value(RuntimeValue::u64(1))),
                }),
            }),
            else_expr: Box::new(RuntimeExpr::Value(RuntimeValue::u64(0))),
        },
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Annotated,
    };
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            backend: RuntimePureBackendMode::Jit,
            workers: RuntimePureWorkerCount::Fixed(1),
            batch_min_len: 1024,
            ..RuntimePureAcceleratorConfig::default()
        },
        std::slice::from_ref(&helper),
    );

    let value = accelerator
        .call_u64_slice(&helper, &[8, 1])
        .expect("native u64 JIT slice call succeeds");
    let mut out = [0; 3];
    accelerator
        .call_u64_flat_batch(&helper, &[8, 1, 3, 99, 10, 4], 2, &mut out)
        .expect("native u64 JIT flat batch succeeds");
    let sum = accelerator
        .call_u64_flat_batch_sum(&helper, &[8, 1, 3, 99, 10, 4], 2, 3)
        .expect("native u64 JIT flat batch sum succeeds");

    assert_eq!(value, Some(4));
    assert_eq!(out, [4, 0, 2]);
    assert_eq!(sum, 6);
    assert_eq!(accelerator.stats().jit_calls, 7);
    assert_eq!(accelerator.stats().aot_calls, 0);
    assert_eq!(accelerator.stats().vm_calls, 0);
    assert_eq!(accelerator.summary().jit, 1);
}

#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
#[test]
fn explicit_jit_uses_native_f32_for_slice_and_flat_batch() {
    let helper = RuntimePureHelper {
        id: RuntimePureHelperId(0),
        name: "f32_score_jit".to_owned(),
        input_names: vec!["base".to_owned(), "scale".to_owned()],
        input_types: vec![RuntimePureInputType::F32, RuntimePureInputType::F32],
        output_type: RuntimePureOutputType::F32,
        expr: RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("scale".to_owned())),
                op: RuntimeBinaryOp::Add,
                rhs: Box::new(RuntimeExpr::Value(RuntimeValue::F32(2.0))),
            }),
        },
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Annotated,
    };
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            backend: RuntimePureBackendMode::Jit,
            workers: RuntimePureWorkerCount::Fixed(1),
            batch_min_len: 1024,
            ..RuntimePureAcceleratorConfig::default()
        },
        std::slice::from_ref(&helper),
    );

    let value = accelerator
        .call_f32_slice(&helper, &[3.0, 4.0])
        .expect("native f32 JIT slice call succeeds");
    let mut out = [0.0; 3];
    accelerator
        .call_f32_flat_batch(&helper, &[3.0, 4.0, 2.0, 99.0, 7.0, 1.0], 2, &mut out)
        .expect("native f32 JIT flat batch succeeds");

    assert_eq!(value.map(f32::to_bits), Some(18.0f32.to_bits()));
    assert_eq!(
        out.map(f32::to_bits),
        [18.0f32, 202.0, 21.0].map(f32::to_bits)
    );
    assert_eq!(accelerator.stats().jit_calls, 4);
    assert_eq!(accelerator.stats().aot_calls, 0);
    assert_eq!(accelerator.stats().vm_calls, 0);
    assert_eq!(
        accelerator.stats().flat_batch_bytes_borrowed,
        6 * std::mem::size_of::<f32>()
    );
    assert_eq!(
        accelerator.stats().result_bytes_copied,
        3 * std::mem::size_of::<f32>()
    );
    assert_eq!(accelerator.summary().jit, 1);
}

#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
#[test]
fn explicit_jit_uses_native_f64_for_slice_and_flat_batch() {
    let helper = RuntimePureHelper {
        id: RuntimePureHelperId(0),
        name: "f64_score_jit".to_owned(),
        input_names: vec!["base".to_owned(), "scale".to_owned()],
        input_types: vec![RuntimePureInputType::F64, RuntimePureInputType::F64],
        output_type: RuntimePureOutputType::F64,
        expr: RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("scale".to_owned())),
                op: RuntimeBinaryOp::Add,
                rhs: Box::new(RuntimeExpr::Value(RuntimeValue::F64(2.0))),
            }),
        },
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Annotated,
    };
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            backend: RuntimePureBackendMode::Jit,
            workers: RuntimePureWorkerCount::Fixed(1),
            batch_min_len: 1024,
            ..RuntimePureAcceleratorConfig::default()
        },
        std::slice::from_ref(&helper),
    );

    let value = accelerator
        .call_f64_slice(&helper, &[3.0, 4.0])
        .expect("native f64 JIT slice call succeeds");
    let mut out = [0.0; 3];
    accelerator
        .call_f64_flat_batch(&helper, &[3.0, 4.0, 2.0, 99.0, 7.0, 1.0], 2, &mut out)
        .expect("native f64 JIT flat batch succeeds");

    assert_eq!(value.map(f64::to_bits), Some(18.0f64.to_bits()));
    assert_eq!(
        out.map(f64::to_bits),
        [18.0f64, 202.0, 21.0].map(f64::to_bits)
    );
    assert_eq!(accelerator.stats().jit_calls, 4);
    assert_eq!(accelerator.stats().aot_calls, 0);
    assert_eq!(accelerator.stats().vm_calls, 0);
    assert_eq!(
        accelerator.stats().flat_batch_bytes_borrowed,
        6 * std::mem::size_of::<f64>()
    );
    assert_eq!(
        accelerator.stats().result_bytes_copied,
        3 * std::mem::size_of::<f64>()
    );
    assert_eq!(accelerator.summary().jit, 1);
}

#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
#[test]
fn auto_promotes_large_i32_flat_batch_to_native_jit() {
    let helper = RuntimePureHelper {
        id: RuntimePureHelperId(0),
        name: "i32_score_auto_jit".to_owned(),
        input_names: vec!["base".to_owned(), "bonus".to_owned()],
        input_types: vec![RuntimePureInputType::I32, RuntimePureInputType::I32],
        output_type: RuntimePureOutputType::I32,
        expr: RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Add,
            rhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
        },
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Annotated,
    };
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            backend: RuntimePureBackendMode::Auto,
            workers: RuntimePureWorkerCount::Fixed(1),
            batch_min_len: 1024,
            ..RuntimePureAcceleratorConfig::default()
        },
        std::slice::from_ref(&helper),
    );
    let flat_inputs = (0..128).flat_map(|value| [value, 1]).collect::<Vec<i32>>();
    let mut out = [0; 128];

    accelerator
        .call_i32_flat_batch(&helper, &flat_inputs, 2, &mut out)
        .expect("auto promotes large i32 flat batch");

    assert_eq!(out[0], 1);
    assert_eq!(out[127], 128);
    assert_eq!(accelerator.stats().jit_calls, 128);
    assert_eq!(accelerator.stats().aot_calls, 0);
    assert_eq!(accelerator.compile_stats().auto_jit_promotions, 1);
    assert_eq!(accelerator.summary().jit, 1);
}

#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
#[test]
fn auto_promotes_large_u32_flat_batch_to_native_jit() {
    let helper = RuntimePureHelper {
        id: RuntimePureHelperId(0),
        name: "u32_score_auto_jit".to_owned(),
        input_names: vec!["base".to_owned(), "bonus".to_owned()],
        input_types: vec![RuntimePureInputType::U32, RuntimePureInputType::U32],
        output_type: RuntimePureOutputType::U32,
        expr: RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Add,
            rhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
        },
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Annotated,
    };
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            backend: RuntimePureBackendMode::Auto,
            workers: RuntimePureWorkerCount::Fixed(1),
            batch_min_len: 1024,
            ..RuntimePureAcceleratorConfig::default()
        },
        std::slice::from_ref(&helper),
    );
    let flat_inputs = (0..128).flat_map(|value| [value, 1]).collect::<Vec<u32>>();
    let mut out = [0; 128];

    accelerator
        .call_u32_flat_batch(&helper, &flat_inputs, 2, &mut out)
        .expect("auto promotes large u32 flat batch");

    assert_eq!(out[0], 1);
    assert_eq!(out[127], 128);
    assert_eq!(accelerator.stats().jit_calls, 128);
    assert_eq!(accelerator.stats().aot_calls, 0);
    assert_eq!(accelerator.compile_stats().auto_jit_promotions, 1);
    assert_eq!(accelerator.summary().jit, 1);
}

#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
fn exact_int_add_helper(
    name: &str,
    input_type: RuntimePureInputType,
    output_type: RuntimePureOutputType,
    one: RuntimeValue,
) -> RuntimePureHelper {
    scalar_add_helper(name, input_type, output_type, one)
}

#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
fn scalar_add_helper(
    name: &str,
    input_type: RuntimePureInputType,
    output_type: RuntimePureOutputType,
    one: RuntimeValue,
) -> RuntimePureHelper {
    RuntimePureHelper {
        id: RuntimePureHelperId(0),
        name: name.to_owned(),
        input_names: vec!["base".to_owned(), "bonus".to_owned()],
        input_types: vec![input_type, input_type],
        output_type,
        expr: RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Add,
            rhs: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
                op: RuntimeBinaryOp::Add,
                rhs: Box::new(RuntimeExpr::Value(one)),
            }),
        },
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Annotated,
    }
}

#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
fn assert_exact_int_scalar_jit<T>(name: &str, args: &[T], one: T, expected: T)
where
    T: RuntimePureScalarInteger + PartialEq + std::fmt::Debug,
{
    let helper = exact_int_add_helper(
        name,
        T::INPUT_TYPE,
        T::OUTPUT_TYPE,
        one.into_runtime_value(),
    );
    let mut accelerator =
        RuntimePureAccelerator::new(RuntimePureBackendMode::Jit, std::slice::from_ref(&helper));

    let value = accelerator
        .call_exact_int_slice::<T>(&helper, args)
        .expect("generic exact-int call succeeds");

    assert_eq!(value, Some(expected));
    assert_eq!(accelerator.stats().jit_calls, 1);
    assert_eq!(accelerator.stats().vm_calls, 0);
    assert_eq!(accelerator.stats().fallbacks, 0);
    assert_eq!(accelerator.stats().arg_vec_allocations, 0);
}

#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
#[test]
fn generic_exact_int_scalar_call_recognizes_width_specific_jit_entry() {
    assert_exact_int_scalar_jit("i8_generic_jit", &[3_i8, 5_i8], 1_i8, 9_i8);
    assert_exact_int_scalar_jit("i16_generic_jit", &[7_i16, 11_i16], 1_i16, 19_i16);
    assert_exact_int_scalar_jit("i32_generic_jit", &[19_i32, 23_i32], 1_i32, 43_i32);
    assert_exact_int_scalar_jit("u16_generic_jit", &[13_u16, 17_u16], 1_u16, 31_u16);
    assert_exact_int_scalar_jit("u8_generic_jit", &[5_u8, 7_u8], 1_u8, 13_u8);
    assert_exact_int_scalar_jit("u32_generic_jit", &[29_u32, 31_u32], 1_u32, 61_u32);
    assert_exact_int_scalar_jit("u64_generic_jit", &[41_u64, 43_u64], 1_u64, 85_u64);
    assert_exact_int_scalar_jit("i128_generic_jit", &[53_i128, 59_i128], 1_i128, 113_i128);
    assert_exact_int_scalar_jit(
        "isize_generic_jit",
        &[RuntimeISizeValue::new(37), RuntimeISizeValue::new(41)],
        RuntimeISizeValue::new(1),
        RuntimeISizeValue::new(79),
    );
    assert_exact_int_scalar_jit(
        "usize_generic_jit",
        &[RuntimeUSizeValue::new(43), RuntimeUSizeValue::new(47)],
        RuntimeUSizeValue::new(1),
        RuntimeUSizeValue::new(91),
    );
    assert_exact_int_scalar_jit("u128_generic_jit", &[61_u128, 67_u128], 1_u128, 129_u128);
}

#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
#[test]
fn auto_promotes_hot_scalar_exact_int_calls_to_native_jit() {
    let helper = scalar_add_helper(
        "i128_hot_scalar_auto_jit",
        RuntimePureInputType::I128,
        RuntimePureOutputType::I128,
        RuntimeValue::i128(1),
    );
    let mut accelerator =
        RuntimePureAccelerator::new(RuntimePureBackendMode::Auto, std::slice::from_ref(&helper));

    for value in 0..160 {
        let actual = accelerator
            .call_exact_int_slice::<i128>(&helper, &[value, 11])
            .expect("hot scalar i128 call succeeds");
        assert_eq!(actual, Some(value + 12));
    }

    assert_eq!(accelerator.compile_stats().auto_jit_deferred, 1);
    assert_eq!(accelerator.compile_stats().auto_jit_promotions, 1);
    assert!(accelerator.stats().aot_calls > 0);
    assert!(accelerator.stats().jit_calls > 0);
    assert_eq!(accelerator.stats().vm_calls, 0);
    assert_eq!(accelerator.stats().fallbacks, 0);
    assert_eq!(accelerator.stats().arg_vec_allocations, 0);
}

#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
#[test]
fn auto_promotes_hot_scalar_float_calls_to_native_jit() {
    let helper = scalar_add_helper(
        "f32_hot_scalar_auto_jit",
        RuntimePureInputType::F32,
        RuntimePureOutputType::F32,
        RuntimeValue::f32(1.0),
    );
    let mut accelerator =
        RuntimePureAccelerator::new(RuntimePureBackendMode::Auto, std::slice::from_ref(&helper));

    for value in 0_u16..160 {
        let base = f32::from(value);
        let actual = accelerator
            .call_f32_slice(&helper, &[base, 11.0])
            .expect("hot scalar f32 call succeeds");
        assert_eq!(actual, Some(base + 12.0));
    }

    assert_eq!(accelerator.compile_stats().auto_jit_deferred, 1);
    assert_eq!(accelerator.compile_stats().auto_jit_promotions, 1);
    assert!(accelerator.stats().aot_calls > 0);
    assert!(accelerator.stats().jit_calls > 0);
    assert_eq!(accelerator.stats().vm_calls, 0);
    assert_eq!(accelerator.stats().fallbacks, 0);
    assert_eq!(accelerator.stats().arg_vec_allocations, 0);
}

#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
#[test]
fn auto_promotes_small_integer_flat_batches_to_native_jit() {
    let helper = exact_int_add_helper(
        "i8_auto_jit",
        RuntimePureInputType::I8,
        RuntimePureOutputType::I8,
        RuntimeValue::i8(1),
    );
    let mut accelerator =
        RuntimePureAccelerator::new(RuntimePureBackendMode::Auto, std::slice::from_ref(&helper));
    let flat_inputs = (0..64).flat_map(|value| [value, 1]).collect::<Vec<i8>>();
    let mut out = [0; 64];
    accelerator
        .call_i8_flat_batch(&helper, &flat_inputs, 2, &mut out)
        .expect("auto promotes large i8 flat batch");
    assert_eq!(out[0], 2);
    assert_eq!(out[63], 65);
    assert_eq!(accelerator.stats().jit_calls, 64);
    assert_eq!(accelerator.stats().aot_calls, 0);
    assert_eq!(accelerator.compile_stats().auto_jit_deferred, 1);
    assert_eq!(accelerator.compile_stats().auto_jit_promotions, 1);

    let helper = exact_int_add_helper(
        "i16_auto_jit",
        RuntimePureInputType::I16,
        RuntimePureOutputType::I16,
        RuntimeValue::i16(1),
    );
    let mut accelerator =
        RuntimePureAccelerator::new(RuntimePureBackendMode::Auto, std::slice::from_ref(&helper));
    let flat_inputs = (0..128).flat_map(|value| [value, 1]).collect::<Vec<i16>>();
    let mut out = [0; 128];
    accelerator
        .call_i16_flat_batch(&helper, &flat_inputs, 2, &mut out)
        .expect("auto promotes large i16 flat batch");
    assert_eq!(out[0], 2);
    assert_eq!(out[127], 129);
    assert_eq!(accelerator.stats().jit_calls, 128);
    assert_eq!(accelerator.stats().aot_calls, 0);
    assert_eq!(accelerator.compile_stats().auto_jit_deferred, 1);
    assert_eq!(accelerator.compile_stats().auto_jit_promotions, 1);

    let helper = exact_int_add_helper(
        "u8_auto_jit",
        RuntimePureInputType::U8,
        RuntimePureOutputType::U8,
        RuntimeValue::u8(1),
    );
    let mut accelerator =
        RuntimePureAccelerator::new(RuntimePureBackendMode::Auto, std::slice::from_ref(&helper));
    let flat_inputs = (0..128).flat_map(|value| [value, 1]).collect::<Vec<u8>>();
    let mut out = [0; 128];
    accelerator
        .call_u8_flat_batch(&helper, &flat_inputs, 2, &mut out)
        .expect("auto promotes large u8 flat batch");
    assert_eq!(out[0], 2);
    assert_eq!(out[127], 129);
    assert_eq!(accelerator.stats().jit_calls, 128);
    assert_eq!(accelerator.stats().aot_calls, 0);
    assert_eq!(accelerator.compile_stats().auto_jit_deferred, 1);
    assert_eq!(accelerator.compile_stats().auto_jit_promotions, 1);

    let helper = exact_int_add_helper(
        "u16_auto_jit",
        RuntimePureInputType::U16,
        RuntimePureOutputType::U16,
        RuntimeValue::u16(1),
    );
    let mut accelerator =
        RuntimePureAccelerator::new(RuntimePureBackendMode::Auto, std::slice::from_ref(&helper));
    let flat_inputs = (0..128).flat_map(|value| [value, 1]).collect::<Vec<u16>>();
    let mut out = [0; 128];
    accelerator
        .call_u16_flat_batch(&helper, &flat_inputs, 2, &mut out)
        .expect("auto promotes large u16 flat batch");
    assert_eq!(out[0], 2);
    assert_eq!(out[127], 129);
    assert_eq!(accelerator.stats().jit_calls, 128);
    assert_eq!(accelerator.stats().aot_calls, 0);
    assert_eq!(accelerator.compile_stats().auto_jit_deferred, 1);
    assert_eq!(accelerator.compile_stats().auto_jit_promotions, 1);
}

#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
#[test]
fn auto_promotes_target_size_integer_flat_batches_to_native_jit() {
    let helper = exact_int_add_helper(
        "isize_auto_jit",
        RuntimePureInputType::ISize,
        RuntimePureOutputType::ISize,
        RuntimeValue::isize(1),
    );
    let mut accelerator =
        RuntimePureAccelerator::new(RuntimePureBackendMode::Auto, std::slice::from_ref(&helper));
    let flat_inputs = (0..128_i64)
        .flat_map(|value| [RuntimeISizeValue::new(value), RuntimeISizeValue::new(1)])
        .collect::<Vec<_>>();
    let mut out = [RuntimeISizeValue::new(0); 128];
    accelerator
        .call_exact_int_flat_batch(&helper, &flat_inputs, 2, &mut out)
        .expect("auto promotes large isize flat batch");
    let sum = accelerator
        .call_exact_int_flat_batch_sum(&helper, &flat_inputs, 2, 128)
        .expect("native isize flat batch sum succeeds");
    assert_eq!(out[0], RuntimeISizeValue::new(2));
    assert_eq!(out[127], RuntimeISizeValue::new(129));
    assert_eq!(sum, 8_384);
    assert_eq!(accelerator.stats().jit_calls, 256);
    assert_eq!(accelerator.stats().aot_calls, 0);
    assert_eq!(accelerator.stats().vm_calls, 0);
    assert_eq!(accelerator.stats().fallbacks, 0);
    assert_eq!(accelerator.compile_stats().auto_jit_deferred, 1);
    assert_eq!(accelerator.compile_stats().auto_jit_promotions, 1);

    let helper = exact_int_add_helper(
        "usize_auto_jit",
        RuntimePureInputType::USize,
        RuntimePureOutputType::USize,
        RuntimeValue::usize(1),
    );
    let mut accelerator =
        RuntimePureAccelerator::new(RuntimePureBackendMode::Auto, std::slice::from_ref(&helper));
    let flat_inputs = (0..128_u64)
        .flat_map(|value| [RuntimeUSizeValue::new(value), RuntimeUSizeValue::new(1)])
        .collect::<Vec<_>>();
    let mut out = [RuntimeUSizeValue::new(0); 128];
    accelerator
        .call_exact_int_flat_batch(&helper, &flat_inputs, 2, &mut out)
        .expect("auto promotes large usize flat batch");
    let sum = accelerator
        .call_exact_int_flat_batch_sum(&helper, &flat_inputs, 2, 128)
        .expect("native usize flat batch sum succeeds");
    assert_eq!(out[0], RuntimeUSizeValue::new(2));
    assert_eq!(out[127], RuntimeUSizeValue::new(129));
    assert_eq!(sum, 8_384);
    assert_eq!(accelerator.stats().jit_calls, 256);
    assert_eq!(accelerator.stats().aot_calls, 0);
    assert_eq!(accelerator.stats().vm_calls, 0);
    assert_eq!(accelerator.stats().fallbacks, 0);
    assert_eq!(accelerator.compile_stats().auto_jit_deferred, 1);
    assert_eq!(accelerator.compile_stats().auto_jit_promotions, 1);
}

#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
#[test]
fn auto_promotes_wide_integer_flat_batches_to_native_jit() {
    let helper = exact_int_add_helper(
        "i128_auto_jit",
        RuntimePureInputType::I128,
        RuntimePureOutputType::I128,
        RuntimeValue::i128(1),
    );
    let mut accelerator =
        RuntimePureAccelerator::new(RuntimePureBackendMode::Auto, std::slice::from_ref(&helper));
    let flat_inputs = (0..128)
        .flat_map(|value| [i128::from(value), 1])
        .collect::<Vec<i128>>();
    let mut out = [0; 128];
    accelerator
        .call_i128_flat_batch(&helper, &flat_inputs, 2, &mut out)
        .expect("auto promotes large i128 flat batch");
    assert_eq!(out[0], 2);
    assert_eq!(out[127], 129);
    assert_eq!(accelerator.stats().jit_calls, 128);
    assert_eq!(accelerator.stats().aot_calls, 0);
    assert_eq!(accelerator.compile_stats().auto_jit_deferred, 1);
    assert_eq!(accelerator.compile_stats().auto_jit_promotions, 1);

    let helper = exact_int_add_helper(
        "u128_auto_jit",
        RuntimePureInputType::U128,
        RuntimePureOutputType::U128,
        RuntimeValue::u128(1),
    );
    let mut accelerator =
        RuntimePureAccelerator::new(RuntimePureBackendMode::Auto, std::slice::from_ref(&helper));
    let flat_inputs = (0..128)
        .flat_map(|value: u16| [u128::from(value), 1])
        .collect::<Vec<u128>>();
    let mut out = [0; 128];
    accelerator
        .call_u128_flat_batch(&helper, &flat_inputs, 2, &mut out)
        .expect("auto promotes large u128 flat batch");
    assert_eq!(out[0], 2);
    assert_eq!(out[127], 129);
    assert_eq!(accelerator.stats().jit_calls, 128);
    assert_eq!(accelerator.stats().aot_calls, 0);
    assert_eq!(accelerator.compile_stats().auto_jit_deferred, 1);
    assert_eq!(accelerator.compile_stats().auto_jit_promotions, 1);
}

#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
#[test]
fn runtime_flow_dense_u32_map_sum_uses_native_jit_batch() {
    let helper = RuntimePureHelper {
        id: RuntimePureHelperId(0),
        name: "u32_flow_score".to_owned(),
        input_names: vec!["base".to_owned(), "bonus".to_owned()],
        input_types: vec![RuntimePureInputType::U32, RuntimePureInputType::U32],
        output_type: RuntimePureOutputType::U32,
        expr: RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Add,
            rhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
        },
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Annotated,
    };
    let plan = RuntimePlan::new(
        Some(FlowRuntimeId("flow.u32".to_owned())),
        vec![RuntimeFlow {
            id: FlowRuntimeId("flow.u32".to_owned()),
            ops: vec![FlowOp::ReturnExpr(RuntimeExpr::Sum {
                source: Box::new(RuntimeExpr::Map {
                    source: Box::new(RuntimeExpr::Value(runtime_sequence_dense_u32(
                        (0..128).collect(),
                    ))),
                    param: "base".to_owned(),
                    body: Box::new(RuntimeExpr::PureCall {
                        helper: RuntimePureHelperId(0),
                        args: vec![
                            RuntimeExpr::Local("base".to_owned()),
                            RuntimeExpr::Value(RuntimeValue::u32(1)),
                        ],
                    }),
                }),
            })],
        }],
        Vec::new(),
    )
    .expect("runtime plan is valid")
    .with_pure_helpers(vec![helper.clone()]);
    let mut engine = Engine::new(plan);
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            backend: RuntimePureBackendMode::Auto,
            workers: RuntimePureWorkerCount::Fixed(1),
            batch_min_len: 1024,
            ..RuntimePureAcceleratorConfig::default()
        },
        std::slice::from_ref(&helper),
    );

    let result = engine.step_with_pure_backend(
        RuntimeStepInput::default(),
        RuntimeStepOptions::default(),
        &mut accelerator,
    );

    assert!(matches!(
        result.fiber_status,
        FlowFiberStatus::Done(FlowExit::Return(ref value)) if value == "8256"
    ));
    assert_eq!(accelerator.stats().jit_calls, 128);
    assert_eq!(accelerator.stats().aot_calls, 0);
    assert_eq!(accelerator.stats().vm_calls, 0);
    assert_eq!(accelerator.compile_stats().auto_jit_promotions, 1);
    assert_eq!(result.stats.pure.batch_calls, 1);
    assert_eq!(result.stats.pure.flat_batch_calls, 1);
    assert_eq!(result.stats.pure.arg_vec_allocations, 0);
}

#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
#[test]
fn auto_promotes_large_f32_flat_batch_to_native_jit() {
    let helper = RuntimePureHelper {
        id: RuntimePureHelperId(0),
        name: "f32_score_auto_jit".to_owned(),
        input_names: vec!["base".to_owned(), "scale".to_owned()],
        input_types: vec![RuntimePureInputType::F32, RuntimePureInputType::F32],
        output_type: RuntimePureOutputType::F32,
        expr: RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("scale".to_owned())),
                op: RuntimeBinaryOp::Add,
                rhs: Box::new(RuntimeExpr::Value(RuntimeValue::F32(2.0))),
            }),
        },
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Annotated,
    };
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            backend: RuntimePureBackendMode::Auto,
            workers: RuntimePureWorkerCount::Fixed(1),
            batch_min_len: 1024,
            ..RuntimePureAcceleratorConfig::default()
        },
        std::slice::from_ref(&helper),
    );
    let flat_inputs = (1..=128)
        .flat_map(|value: u16| [f32::from(value), 2.0])
        .collect::<Vec<f32>>();
    let mut out = [0.0; 128];

    accelerator
        .call_f32_flat_batch(&helper, &flat_inputs, 2, &mut out)
        .expect("auto promotes large f32 flat batch");

    assert_eq!(out[0].to_bits(), 4.0f32.to_bits());
    assert_eq!(out[127].to_bits(), 512.0f32.to_bits());
    assert_eq!(accelerator.stats().jit_calls, 128);
    assert_eq!(accelerator.stats().aot_calls, 0);
    assert_eq!(accelerator.compile_stats().auto_jit_promotions, 1);
    assert_eq!(accelerator.summary().jit, 1);
}

#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
#[test]
fn auto_promotes_large_f64_flat_batch_to_native_jit() {
    let helper = RuntimePureHelper {
        id: RuntimePureHelperId(0),
        name: "f64_score_auto_jit".to_owned(),
        input_names: vec!["base".to_owned(), "scale".to_owned()],
        input_types: vec![RuntimePureInputType::F64, RuntimePureInputType::F64],
        output_type: RuntimePureOutputType::F64,
        expr: RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("scale".to_owned())),
                op: RuntimeBinaryOp::Add,
                rhs: Box::new(RuntimeExpr::Value(RuntimeValue::F64(2.0))),
            }),
        },
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Annotated,
    };
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            backend: RuntimePureBackendMode::Auto,
            workers: RuntimePureWorkerCount::Fixed(1),
            batch_min_len: 1024,
            ..RuntimePureAcceleratorConfig::default()
        },
        std::slice::from_ref(&helper),
    );
    let flat_inputs = (1..=128)
        .flat_map(|value: u16| [f64::from(value), 2.0])
        .collect::<Vec<f64>>();
    let mut out = [0.0; 128];

    accelerator
        .call_f64_flat_batch(&helper, &flat_inputs, 2, &mut out)
        .expect("auto promotes large f64 flat batch");

    assert_eq!(out[0].to_bits(), 4.0f64.to_bits());
    assert_eq!(out[127].to_bits(), 512.0f64.to_bits());
    assert_eq!(accelerator.stats().jit_calls, 128);
    assert_eq!(accelerator.stats().aot_calls, 0);
    assert_eq!(accelerator.compile_stats().auto_jit_promotions, 1);
    assert_eq!(accelerator.summary().jit, 1);
}

#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
#[test]
fn auto_accelerator_promotes_large_flat_batches_to_jit() {
    let helper = RuntimePureHelper {
        id: RuntimePureHelperId(0),
        name: "score".to_owned(),
        input_names: vec!["base".to_owned(), "bonus".to_owned()],
        input_types: vec![RuntimePureInputType::I64, RuntimePureInputType::I64],
        output_type: RuntimePureOutputType::I64,
        expr: RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
                op: RuntimeBinaryOp::Add,
                rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i64(2))),
            }),
        },
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Annotated,
    };
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            backend: RuntimePureBackendMode::Auto,
            workers: RuntimePureWorkerCount::Fixed(1),
            batch_min_len: 1024,
            ..RuntimePureAcceleratorConfig::default()
        },
        std::slice::from_ref(&helper),
    );
    let mut flat_inputs = Vec::new();
    for value in 1..=128 {
        flat_inputs.extend([value, 2]);
    }
    let mut out = [0; 128];

    accelerator
        .call_i64_flat_batch(&helper, &flat_inputs, 2, &mut out)
        .expect("large auto flat batch succeeds");

    assert_eq!(out[0], 4);
    assert_eq!(out[127], 512);
    assert_eq!(accelerator.stats().jit_calls, 128);
    assert_eq!(accelerator.stats().aot_calls, 0);
    assert_eq!(accelerator.stats().arg_vec_allocations, 0);
    assert_eq!(accelerator.stats().flatten_materializations, 0);
    assert_eq!(accelerator.stats().flatten_bytes_copied, 0);
    assert_eq!(
        accelerator.stats().flat_batch_bytes_borrowed,
        flat_inputs.len() * std::mem::size_of::<i64>()
    );
    assert_eq!(accelerator.summary().jit, 1);
    assert_eq!(accelerator.compile_stats().auto_aot_selected, 1);
    assert_eq!(accelerator.compile_stats().auto_jit_promotions, 1);
}

#[test]
fn aot_accelerates_exact_width_scalar_calls_without_i64_widening() {
    fn add_helper(
        id: usize,
        name: &str,
        input_type: RuntimePureInputType,
        output_type: RuntimePureOutputType,
    ) -> RuntimePureHelper {
        RuntimePureHelper {
            id: RuntimePureHelperId(id),
            name: name.to_owned(),
            input_names: vec!["lhs".to_owned(), "rhs".to_owned()],
            input_types: vec![input_type, input_type],
            output_type,
            expr: RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("lhs".to_owned())),
                op: RuntimeBinaryOp::Add,
                rhs: Box::new(RuntimeExpr::Local("rhs".to_owned())),
            },
            scalar_eval_supported: true,
            origin: RuntimePureHelperOrigin::Annotated,
        }
    }

    let helpers = [
        add_helper(
            0,
            "i32_add",
            RuntimePureInputType::I32,
            RuntimePureOutputType::I32,
        ),
        add_helper(
            1,
            "u32_add",
            RuntimePureInputType::U32,
            RuntimePureOutputType::U32,
        ),
        add_helper(
            2,
            "f32_add",
            RuntimePureInputType::F32,
            RuntimePureOutputType::F32,
        ),
        add_helper(
            3,
            "f64_add",
            RuntimePureInputType::F64,
            RuntimePureOutputType::F64,
        ),
        add_helper(
            4,
            "isize_add",
            RuntimePureInputType::ISize,
            RuntimePureOutputType::ISize,
        ),
        add_helper(
            5,
            "usize_add",
            RuntimePureInputType::USize,
            RuntimePureOutputType::USize,
        ),
    ];
    let mut accelerator = RuntimePureAccelerator::new(RuntimePureBackendMode::Aot, &helpers);

    let i32_value = accelerator
        .call_i32_slice(&helpers[0], &[7, 11])
        .expect("i32 AOT call succeeds");
    let u32_value = accelerator
        .call_exact_int_slice::<u32>(&helpers[1], &[13, 17])
        .expect("u32 AOT call succeeds");
    let f32_value = accelerator
        .call_f32_slice(&helpers[2], &[1.25, 2.5])
        .expect("f32 AOT call succeeds");
    let f64_value = accelerator
        .call_f64_slice(&helpers[3], &[3.0, 4.5])
        .expect("f64 AOT call succeeds");
    let isize_value = accelerator
        .call_exact_int_slice::<RuntimeISizeValue>(
            &helpers[4],
            &[RuntimeISizeValue::new(19), RuntimeISizeValue::new(23)],
        )
        .expect("isize AOT call succeeds");
    let usize_value = accelerator
        .call_exact_int_slice::<RuntimeUSizeValue>(
            &helpers[5],
            &[RuntimeUSizeValue::new(29), RuntimeUSizeValue::new(31)],
        )
        .expect("usize AOT call succeeds");

    assert_eq!(i32_value, Some(18));
    assert_eq!(u32_value, Some(30));
    assert_eq!(f32_value, Some(3.75));
    assert_eq!(f64_value, Some(7.5));
    assert_eq!(isize_value, Some(RuntimeISizeValue::new(42)));
    assert_eq!(usize_value, Some(RuntimeUSizeValue::new(60)));
    assert_eq!(accelerator.stats().aot_calls, 6);
    assert_eq!(accelerator.stats().vm_calls, 0);
    assert_eq!(accelerator.stats().fallbacks, 0);
    assert_eq!(accelerator.summary().aot, 6);
}

#[test]
fn value_fallback_reuses_vm_scratch_without_value_vec_allocation() {
    let helper = RuntimePureHelper {
        id: RuntimePureHelperId(0),
        name: "echo".to_owned(),
        input_names: vec!["label".to_owned()],
        input_types: vec![RuntimePureInputType::Value],
        output_type: RuntimePureOutputType::Value,
        expr: RuntimeExpr::Local("label".to_owned()),
        scalar_eval_supported: false,
        origin: RuntimePureHelperOrigin::Annotated,
    };
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            backend: RuntimePureBackendMode::Vm,
            workers: RuntimePureWorkerCount::Fixed(1),
            batch_min_len: 2,
            ..RuntimePureAcceleratorConfig::default()
        },
        std::slice::from_ref(&helper),
    );

    let value = accelerator
        .call_values(&helper, &[RuntimeValue::String("ready".to_owned())])
        .expect("VM value fallback succeeds");

    assert_eq!(value, RuntimeValue::String("ready".to_owned()));
    assert_eq!(accelerator.stats().pure_calls, 1);
    assert_eq!(accelerator.stats().vm_calls, 1);
    assert_eq!(accelerator.stats().fallbacks, 1);
    assert_eq!(accelerator.stats().arg_vec_allocations, 0);
    assert_eq!(
        accelerator.stats().arg_bytes_borrowed,
        std::mem::size_of_val(&[RuntimeValue::String("ready".to_owned())])
    );
}

#[test]
fn aot_batch_matches_scalar_results_and_records_parallel_stats() {
    let helper = RuntimePureHelper {
        id: RuntimePureHelperId(0),
        name: "score".to_owned(),
        input_names: vec!["base".to_owned(), "bonus".to_owned()],
        input_types: vec![RuntimePureInputType::I64, RuntimePureInputType::I64],
        output_type: RuntimePureOutputType::I64,
        expr: RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
                op: RuntimeBinaryOp::Add,
                rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i64(2))),
            }),
        },
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Inferred,
    };
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            backend: RuntimePureBackendMode::Aot,
            workers: RuntimePureWorkerCount::Fixed(2),
            batch_min_len: 1,
            ..RuntimePureAcceleratorConfig::default()
        },
        std::slice::from_ref(&helper),
    );
    let rows = [
        RuntimeI64Args::new([3, 4, 0, 0], 2),
        RuntimeI64Args::new([5, 1, 0, 0], 2),
        RuntimeI64Args::new([2, 8, 0, 0], 2),
        RuntimeI64Args::new([7, 0, 0, 0], 2),
    ];
    let mut out = [0; 4];

    accelerator
        .call_i64_batch(&helper, &rows, &mut out)
        .expect("batch succeeds");

    assert_eq!(out, [18, 15, 20, 14]);
    assert_eq!(accelerator.stats().batch_calls, 1);
    assert_eq!(accelerator.stats().batch_items, 4);
    assert_eq!(accelerator.stats().aot_calls, 4);
    assert_eq!(accelerator.stats().arg_vec_allocations, 0);
    assert_eq!(accelerator.resolved_worker_count(), 2);
    assert!(accelerator.has_worker_pool());
    assert_eq!(accelerator.stats().parallel_policy_checks, 1);
    assert_eq!(accelerator.stats().parallel_batches, 1);
    assert_eq!(accelerator.stats().parallel_skipped_small, 0);
    assert_eq!(accelerator.stats().parallel_skipped_backend, 0);
    assert!(accelerator.stats().parallel_work_units > rows.len());
    assert!(accelerator.stats().thread_pool_jobs > 0);
}

#[test]
fn aot_worker_pool_is_created_only_for_parallel_batches() {
    let helper = RuntimePureHelper {
        id: RuntimePureHelperId(0),
        name: "score".to_owned(),
        input_names: vec!["base".to_owned(), "bonus".to_owned()],
        input_types: vec![RuntimePureInputType::I64, RuntimePureInputType::I64],
        output_type: RuntimePureOutputType::I64,
        expr: RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
                op: RuntimeBinaryOp::Add,
                rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i64(2))),
            }),
        },
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Annotated,
    };
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            backend: RuntimePureBackendMode::Aot,
            workers: RuntimePureWorkerCount::Fixed(2),
            batch_min_len: 2,
            ..RuntimePureAcceleratorConfig::default()
        },
        std::slice::from_ref(&helper),
    );
    let small_rows = [
        RuntimeI64Args::new([3, 4, 0, 0], 2),
        RuntimeI64Args::new([5, 1, 0, 0], 2),
    ];
    let mut small_out = [0; 2];

    accelerator
        .call_i64_batch(&helper, &small_rows, &mut small_out)
        .expect("small AOT batch succeeds without pool");

    assert_eq!(small_out, [18, 15]);
    assert!(!accelerator.has_worker_pool());
    assert_eq!(accelerator.stats().parallel_policy_checks, 1);
    assert_eq!(accelerator.stats().parallel_skipped_small, 1);
    assert_eq!(accelerator.stats().thread_pool_jobs, 0);

    let mut small_flat_out = [0; 2];
    accelerator
        .call_i64_flat_batch(&helper, &[3, 4, 5, 1], 2, &mut small_flat_out)
        .expect("small flat AOT batch reuses sequential scratch without pool");

    assert_eq!(small_flat_out, [18, 15]);
    assert!(!accelerator.has_worker_pool());
    assert_eq!(accelerator.stats().parallel_policy_checks, 2);
    assert_eq!(accelerator.stats().parallel_skipped_small, 2);
    assert_eq!(accelerator.stats().thread_pool_jobs, 0);

    let large_rows = [
        RuntimeI64Args::new([3, 4, 0, 0], 2),
        RuntimeI64Args::new([5, 1, 0, 0], 2),
        RuntimeI64Args::new([2, 8, 0, 0], 2),
        RuntimeI64Args::new([7, 0, 0, 0], 2),
        RuntimeI64Args::new([9, 1, 0, 0], 2),
    ];
    let mut large_out = [0; 5];

    accelerator
        .call_i64_batch(&helper, &large_rows, &mut large_out)
        .expect("large AOT batch creates pool");

    assert_eq!(large_out, [18, 15, 20, 14, 27]);
    assert!(accelerator.has_worker_pool());
    assert_eq!(accelerator.stats().parallel_policy_checks, 3);
    assert_eq!(accelerator.stats().parallel_batches, 1);
    assert_eq!(accelerator.stats().parallel_skipped_small, 2);
    assert_eq!(accelerator.stats().thread_pool_jobs, 2);
}

#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
#[test]
fn jit_batch_matches_scalar_results_without_value_vec_allocation() {
    let helper = RuntimePureHelper {
        id: RuntimePureHelperId(0),
        name: "score".to_owned(),
        input_names: vec!["base".to_owned(), "bonus".to_owned()],
        input_types: vec![RuntimePureInputType::I64, RuntimePureInputType::I64],
        output_type: RuntimePureOutputType::I64,
        expr: RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
                op: RuntimeBinaryOp::Add,
                rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i64(2))),
            }),
        },
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Annotated,
    };
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            backend: RuntimePureBackendMode::Jit,
            workers: RuntimePureWorkerCount::Fixed(1),
            batch_min_len: 2,
            ..RuntimePureAcceleratorConfig::default()
        },
        std::slice::from_ref(&helper),
    );
    let rows = [
        RuntimeI64Args::new([3, 4, 0, 0], 2),
        RuntimeI64Args::new([5, 1, 0, 0], 2),
        RuntimeI64Args::new([2, 8, 0, 0], 2),
    ];
    let mut out = [0; 3];

    RuntimePureCallBackend::call_i64_batch(&mut accelerator, &helper, &rows, &mut out)
        .expect("JIT batch succeeds");

    assert_eq!(out, [18, 15, 20]);
    assert_eq!(accelerator.stats().batch_calls, 1);
    assert_eq!(accelerator.stats().batch_items, 3);
    assert_eq!(accelerator.stats().jit_calls, 3);
    assert_eq!(accelerator.stats().arg_stack_packs, 3);
    assert_eq!(accelerator.stats().arg_vec_allocations, 0);
    assert_eq!(accelerator.stats().flat_batch_calls, 0);
    assert_eq!(accelerator.stats().flat_batch_items, 0);
    assert_eq!(accelerator.stats().flatten_materializations, 1);
    assert_eq!(accelerator.stats().parallel_policy_checks, 1);
    assert_eq!(accelerator.stats().parallel_skipped_backend, 1);
    assert_eq!(accelerator.stats().parallel_batches, 0);
    assert_eq!(
        accelerator.stats().flatten_bytes_copied,
        6 * std::mem::size_of::<i64>()
    );
    assert_eq!(accelerator.summary().jit, 1);
}

#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
#[test]
fn jit_flat_batch_sum_avoids_output_copy() {
    let helper = RuntimePureHelper {
        id: RuntimePureHelperId(0),
        name: "score".to_owned(),
        input_names: vec!["base".to_owned(), "bonus".to_owned()],
        input_types: vec![RuntimePureInputType::I64, RuntimePureInputType::I64],
        output_type: RuntimePureOutputType::I64,
        expr: RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
                op: RuntimeBinaryOp::Add,
                rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i64(2))),
            }),
        },
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Annotated,
    };
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            backend: RuntimePureBackendMode::Jit,
            workers: RuntimePureWorkerCount::Fixed(1),
            batch_min_len: 2,
            ..RuntimePureAcceleratorConfig::default()
        },
        std::slice::from_ref(&helper),
    );

    let sum = accelerator
        .call_i64_flat_batch_sum(&helper, &[3, 4, 5, 1, 2, 8], 2, 3)
        .expect("JIT flat batch sum succeeds");

    assert_eq!(sum, 53);
    assert_eq!(accelerator.stats().batch_calls, 1);
    assert_eq!(accelerator.stats().batch_items, 3);
    assert_eq!(accelerator.stats().flat_batch_calls, 1);
    assert_eq!(accelerator.stats().flat_batch_items, 3);
    assert_eq!(
        accelerator.stats().flat_batch_bytes_borrowed,
        6 * std::mem::size_of::<i64>()
    );
    assert_eq!(accelerator.stats().jit_calls, 3);
    assert_eq!(accelerator.stats().arg_stack_packs, 0);
    assert_eq!(accelerator.stats().arg_vec_allocations, 0);
    assert_eq!(accelerator.stats().flatten_materializations, 0);
    assert_eq!(accelerator.stats().flatten_bytes_copied, 0);
    assert_eq!(accelerator.stats().result_bytes_copied, 0);
    assert_eq!(accelerator.stats().parallel_policy_checks, 1);
    assert_eq!(accelerator.stats().parallel_skipped_backend, 1);
    assert_eq!(accelerator.stats().parallel_batches, 0);
}

#[test]
fn vm_batch_uses_i64_args_without_value_vec_allocation() {
    let helper = RuntimePureHelper {
        id: RuntimePureHelperId(0),
        name: "score".to_owned(),
        input_names: vec!["base".to_owned(), "bonus".to_owned()],
        input_types: vec![RuntimePureInputType::I64, RuntimePureInputType::I64],
        output_type: RuntimePureOutputType::I64,
        expr: RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
                op: RuntimeBinaryOp::Add,
                rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i64(2))),
            }),
        },
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Annotated,
    };
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            backend: RuntimePureBackendMode::Vm,
            workers: RuntimePureWorkerCount::Fixed(2),
            batch_min_len: 1,
            ..RuntimePureAcceleratorConfig::default()
        },
        std::slice::from_ref(&helper),
    );
    let rows = [
        RuntimeI64Args::new([3, 4, 0, 0], 2),
        RuntimeI64Args::new([5, 1, 0, 0], 2),
        RuntimeI64Args::new([2, 8, 0, 0], 2),
    ];
    let mut out = [0; 3];

    accelerator
        .call_i64_batch(&helper, &rows, &mut out)
        .expect("VM batch succeeds");

    assert_eq!(out, [18, 15, 20]);
    assert_eq!(accelerator.stats().batch_calls, 1);
    assert_eq!(accelerator.stats().batch_items, 3);
    assert_eq!(accelerator.stats().vm_calls, 3);
    assert_eq!(accelerator.stats().fallbacks, 3);
    assert_eq!(accelerator.stats().arg_stack_packs, 3);
    assert_eq!(accelerator.stats().arg_vec_allocations, 0);
    assert_eq!(accelerator.stats().parallel_policy_checks, 1);
    assert_eq!(accelerator.stats().parallel_batches, 1);
    assert_eq!(accelerator.stats().thread_pool_jobs, 2);
}
