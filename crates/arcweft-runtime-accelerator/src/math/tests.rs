use super::*;

#[cfg(all(feature = "math-glam", feature = "math-ndarray"))]
#[test]
fn scalar_ndarray_and_glam_matmul_match() {
    let lhs = DenseMatrixF32::new(4, 4, (0_u8..16).map(f32::from).collect()).unwrap();
    let rhs = DenseMatrixF32::new(
        4,
        4,
        (0_u8..16).map(|value| f32::from(value) * 0.5).collect(),
    )
    .unwrap();
    let expected = lhs.matmul_scalar(&rhs).unwrap();

    let mut scalar = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
        backend: RuntimeMathBackend::Scalar,
        ..RuntimeMathAcceleratorConfig::default()
    });
    let mut glam = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
        backend: RuntimeMathBackend::Glam,
        ..RuntimeMathAcceleratorConfig::default()
    });
    let mut ndarray = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
        backend: RuntimeMathBackend::Ndarray,
        ..RuntimeMathAcceleratorConfig::default()
    });

    assert_eq!(scalar.matmul_f32(&lhs, &rhs).unwrap(), expected);
    assert_eq!(glam.matmul_f32(&lhs, &rhs).unwrap(), expected);
    assert_eq!(ndarray.matmul_f32(&lhs, &rhs).unwrap(), expected);
    assert_eq!(glam.stats().glam_calls, 1);
    assert_eq!(ndarray.stats().ndarray_calls, 1);
}

#[test]
fn matmul_bias_add_fuses_scalar_work_and_records_backend_stats() {
    let lhs = DenseMatrixF32::new(2, 3, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]).unwrap();
    let rhs = DenseMatrixF32::new(3, 2, vec![7.0, 8.0, 9.0, 10.0, 11.0, 12.0]).unwrap();
    let bias = DenseTensorF32::new(vec![2], vec![0.5, -0.25]).unwrap();
    let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
        backend: RuntimeMathBackend::Scalar,
        ..RuntimeMathAcceleratorConfig::default()
    });

    let out = accelerator.matmul_bias_add_f32(&lhs, &rhs, &bias).unwrap();

    assert_eq!(out.values(), &[58.5, 63.75, 139.5, 153.75]);
    assert_eq!(accelerator.stats().fused_matmul_bias_add_calls, 1);
    assert_eq!(accelerator.stats().scalar_calls, 1);
    assert_eq!(
        accelerator.stats().last_backend,
        Some(RuntimeMathBackend::Scalar)
    );
}

#[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
#[test]
fn matmul_bias_add_uses_fused_wgpu_one_shot_kernel_when_available() {
    let lhs = DenseMatrixF32::new(2, 3, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]).unwrap();
    let rhs = DenseMatrixF32::new(3, 2, vec![7.0, 8.0, 9.0, 10.0, 11.0, 12.0]).unwrap();
    let bias = DenseTensorF32::new(vec![2], vec![0.5, -0.25]).unwrap();
    let expected = DenseMatrixF32::new(2, 2, vec![58.5, 63.75, 139.5, 153.75]).unwrap();
    let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
        backend: RuntimeMathBackend::Wgpu,
        ..RuntimeMathAcceleratorConfig::default()
    });

    let Ok(out) = accelerator.matmul_bias_add_f32(&lhs, &rhs, &bias) else {
        return;
    };

    assert_eq!(out, expected);
    assert_eq!(accelerator.stats().wgpu_calls, 1);
    assert_eq!(accelerator.stats().fused_matmul_bias_add_calls, 1);
    assert_eq!(accelerator.stats().gpu_buffer_creations, 7);
    assert_eq!(
        accelerator.stats().bytes_uploaded,
        (lhs.values().len() + rhs.values().len() + bias.values().len())
            * std::mem::size_of::<f32>()
    );
    assert_eq!(
        accelerator.stats().bytes_downloaded,
        std::mem::size_of_val(expected.values())
    );
    assert_eq!(
        accelerator.stats().last_backend,
        Some(RuntimeMathBackend::Wgpu)
    );
}

#[cfg(all(feature = "math-glam", feature = "math-ndarray"))]
#[test]
fn scalar_ndarray_and_glam_f64_matmul_match_without_widening() {
    let lhs = DenseMatrixF64::new(4, 4, (0_u8..16).map(f64::from).collect()).unwrap();
    let rhs = DenseMatrixF64::new(
        4,
        4,
        (0_u8..16).map(|value| f64::from(value) * 0.5).collect(),
    )
    .unwrap();
    let expected = lhs.matmul_scalar(&rhs).unwrap();

    let mut scalar = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
        backend: RuntimeMathBackend::Scalar,
        ..RuntimeMathAcceleratorConfig::default()
    });
    let mut glam = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
        backend: RuntimeMathBackend::Glam,
        ..RuntimeMathAcceleratorConfig::default()
    });
    let mut ndarray = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
        backend: RuntimeMathBackend::Ndarray,
        ..RuntimeMathAcceleratorConfig::default()
    });

    assert_eq!(scalar.matmul_f64(&lhs, &rhs).unwrap(), expected);
    assert_eq!(glam.matmul_f64(&lhs, &rhs).unwrap(), expected);
    assert_eq!(ndarray.matmul_f64(&lhs, &rhs).unwrap(), expected);
    assert_eq!(glam.stats().glam_calls, 1);
    assert_eq!(ndarray.stats().ndarray_calls, 1);
}

#[cfg(feature = "math-ndarray")]
#[test]
fn tensor_add_keeps_shape_and_backend_stats() {
    let lhs = DenseTensorF32::new(vec![2, 2, 2], vec![1.0; 8]).unwrap();
    let rhs = DenseTensorF32::new(vec![2, 2, 2], vec![2.0; 8]).unwrap();
    let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
        backend: RuntimeMathBackend::Ndarray,
        ..RuntimeMathAcceleratorConfig::default()
    });

    let out = accelerator.tensor_add_f32(&lhs, &rhs).unwrap();

    assert_eq!(out.shape().dims(), &[2, 2, 2]);
    assert_eq!(out.values(), &[3.0; 8]);
    assert_eq!(accelerator.stats().ndarray_calls, 1);
    assert_eq!(
        accelerator.stats().last_backend,
        Some(RuntimeMathBackend::Ndarray)
    );
}

#[cfg(feature = "math-ndarray")]
#[test]
fn f64_tensor_add_keeps_shape_and_backend_stats_without_widening() {
    let lhs = DenseTensorF64::new(vec![2, 2, 2], vec![1.5; 8]).unwrap();
    let rhs = DenseTensorF64::new(vec![2, 2, 2], vec![2.25; 8]).unwrap();
    let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
        backend: RuntimeMathBackend::Ndarray,
        ..RuntimeMathAcceleratorConfig::default()
    });

    let out = accelerator.tensor_add_f64(&lhs, &rhs).unwrap();

    assert_eq!(out.shape().dims(), &[2, 2, 2]);
    assert_eq!(out.values(), &[3.75; 8]);
    assert_eq!(accelerator.stats().ndarray_calls, 1);
    assert_eq!(
        accelerator.stats().last_backend,
        Some(RuntimeMathBackend::Ndarray)
    );
    assert_eq!(
        accelerator.stats().bytes_borrowed,
        16 * std::mem::size_of::<f64>()
    );
}

#[test]
fn explicit_glam_f64_tensor_add_records_scalar_fallback_without_widening() {
    let lhs = DenseTensorF64::new(vec![2, 2], vec![1.25, 2.5, 3.75, 4.5]).unwrap();
    let rhs = DenseTensorF64::new(vec![2, 2], vec![5.0, 6.25, 7.5, 8.75]).unwrap();
    let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
        backend: RuntimeMathBackend::Glam,
        ..RuntimeMathAcceleratorConfig::default()
    });

    let out = accelerator.tensor_add_f64(&lhs, &rhs).unwrap();

    assert_eq!(out.shape().dims(), &[2, 2]);
    assert_eq!(out.values(), &[6.25, 8.75, 11.25, 13.25]);
    assert_eq!(accelerator.stats().glam_calls, 0);
    assert_eq!(accelerator.stats().scalar_calls, 1);
    assert_eq!(accelerator.stats().fallback_calls, 1);
    assert_eq!(
        accelerator.stats().last_backend,
        Some(RuntimeMathBackend::Scalar)
    );
    assert_eq!(
        accelerator.stats().bytes_borrowed,
        8 * std::mem::size_of::<f64>()
    );
}

#[cfg(feature = "math-ndarray")]
#[test]
fn auto_f64_math_stays_on_cpu_even_when_wgpu_threshold_matches() {
    let lhs = DenseMatrixF64::new(8, 8, vec![1.0; 64]).unwrap();
    let rhs = DenseMatrixF64::new(8, 8, vec![2.0; 64]).unwrap();
    let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
        wgpu_min_elements: 1,
        ..RuntimeMathAcceleratorConfig::default()
    });

    let out = accelerator.matmul_f64(&lhs, &rhs).unwrap();

    assert_eq!(out, lhs.matmul_scalar(&rhs).unwrap());
    assert_eq!(accelerator.stats().wgpu_calls, 0);
    assert_eq!(
        accelerator.stats().last_backend,
        Some(RuntimeMathBackend::Scalar)
    );
    assert_eq!(
        accelerator.stats().last_auto_reason,
        Some(RuntimeMathAutoSelectionReason::MatmulScalarSmallWork)
    );
}

#[cfg(feature = "math-ndarray")]
#[test]
fn auto_large_f64_matmul_prefers_ndarray_cpu_backend() {
    let values = 65 * 65;
    let lhs = DenseMatrixF64::new(65, 65, vec![1.0; values]).unwrap();
    let rhs = DenseMatrixF64::new(65, 65, vec![2.0; values]).unwrap();
    let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig::default());

    let out = accelerator.matmul_f64(&lhs, &rhs).unwrap();

    assert_eq!(out, lhs.matmul_scalar(&rhs).unwrap());
    assert_eq!(
        accelerator.stats().last_backend,
        Some(RuntimeMathBackend::Ndarray)
    );
    assert_eq!(
        accelerator.stats().last_auto_reason,
        Some(RuntimeMathAutoSelectionReason::MatmulNdarrayCpuDefault)
    );
}

#[test]
fn explicit_wgpu_f64_math_reports_portability_error() {
    let lhs = DenseTensorF64::new(vec![2, 2], vec![1.0; 4]).unwrap();
    let rhs = DenseTensorF64::new(vec![2, 2], vec![2.0; 4]).unwrap();
    let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
        backend: RuntimeMathBackend::Wgpu,
        ..RuntimeMathAcceleratorConfig::default()
    });

    let error = accelerator
        .tensor_add_f64(&lhs, &rhs)
        .expect_err("portable f64 wgpu kernels are not available");

    assert!(error.to_string().contains("portable f64 tensor kernels"));
    assert_eq!(accelerator.stats().wgpu_calls, 0);
}

#[cfg(feature = "math-ndarray")]
#[test]
fn auto_small_general_matmul_prefers_scalar_cpu_backend() {
    let lhs = DenseMatrixF32::new(8, 8, (0_u8..64).map(f32::from).collect()).unwrap();
    let rhs = DenseMatrixF32::new(
        8,
        8,
        (0_u8..64).map(|value| f32::from(value) * 0.5).collect(),
    )
    .unwrap();
    let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig::default());

    let out = accelerator.matmul_f32(&lhs, &rhs).unwrap();

    assert_eq!(out, lhs.matmul_scalar(&rhs).unwrap());
    assert_eq!(
        accelerator.stats().last_backend,
        Some(RuntimeMathBackend::Scalar)
    );
    assert_eq!(
        accelerator.stats().last_auto_reason,
        Some(RuntimeMathAutoSelectionReason::MatmulScalarSmallWork)
    );
    assert_eq!(accelerator.stats().wgpu_calls, 0);
}

#[cfg(feature = "math-glam")]
#[test]
fn auto_4x4_matmul_records_glam_policy_reason() {
    let lhs = DenseMatrixF32::new(4, 4, (0_u8..16).map(f32::from).collect()).unwrap();
    let rhs = DenseMatrixF32::new(
        4,
        4,
        (0_u8..16).map(|value| f32::from(value) * 0.5).collect(),
    )
    .unwrap();
    let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig::default());

    let out = accelerator.matmul_f32(&lhs, &rhs).unwrap();

    assert_eq!(out, lhs.matmul_scalar(&rhs).unwrap());
    assert_eq!(
        accelerator.stats().last_backend,
        Some(RuntimeMathBackend::Glam)
    );
    assert_eq!(
        accelerator.stats().last_auto_reason,
        Some(RuntimeMathAutoSelectionReason::Matmul4x4Glam)
    );
}

#[cfg(feature = "math-ndarray")]
#[test]
fn auto_elementwise_records_scalar_cpu_policy_reason() {
    let lhs = DenseMatrixF32::new(2, 2, vec![1.0; 4]).unwrap();
    let rhs = DenseMatrixF32::new(2, 2, vec![2.0; 4]).unwrap();
    let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig::default());

    let out = accelerator.matrix_add_f32(&lhs, &rhs).unwrap();

    assert_eq!(out.values(), &[3.0; 4]);
    assert_eq!(
        accelerator.stats().last_backend,
        Some(RuntimeMathBackend::Scalar)
    );
    assert_eq!(
        accelerator.stats().last_auto_reason,
        Some(RuntimeMathAutoSelectionReason::ElementwiseScalarCpuDefault)
    );
}

#[cfg(feature = "math-ndarray")]
#[test]
fn auto_tensor_elementwise_records_ndarray_cpu_policy_reason() {
    let lhs = DenseTensorF32::new(vec![2, 2, 2], vec![1.0; 8]).unwrap();
    let rhs = DenseTensorF32::new(vec![2, 2, 2], vec![2.0; 8]).unwrap();
    let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig::default());

    let out = accelerator.tensor_add_f32(&lhs, &rhs).unwrap();

    assert_eq!(out.values(), &[3.0; 8]);
    assert_eq!(
        accelerator.stats().last_backend,
        Some(RuntimeMathBackend::Ndarray)
    );
    assert_eq!(
        accelerator.stats().last_auto_reason,
        Some(RuntimeMathAutoSelectionReason::ElementwiseNdarrayCpuDefault)
    );
}

#[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
#[test]
fn auto_large_elementwise_records_wgpu_threshold_policy_reason() {
    let lhs = DenseMatrixF32::new(4, 4, vec![1.0; 16]).unwrap();
    let rhs = DenseMatrixF32::new(4, 4, vec![2.0; 16]).unwrap();
    let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
        wgpu_min_elements: 1,
        ..RuntimeMathAcceleratorConfig::default()
    });

    let Ok(out) = accelerator.matrix_add_f32(&lhs, &rhs) else {
        return;
    };

    assert_eq!(out.values(), &[3.0; 16]);
    assert_eq!(
        accelerator.stats().last_backend,
        Some(RuntimeMathBackend::Wgpu)
    );
    assert_eq!(
        accelerator.stats().last_auto_reason,
        Some(RuntimeMathAutoSelectionReason::ElementwiseWgpuWorkThreshold)
    );
}

#[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
#[test]
fn auto_large_matmul_records_wgpu_threshold_policy_reason() {
    let lhs = DenseMatrixF32::new(8, 8, vec![1.0; 64]).unwrap();
    let rhs = DenseMatrixF32::new(8, 8, vec![2.0; 64]).unwrap();
    let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
        wgpu_min_elements: 1,
        ..RuntimeMathAcceleratorConfig::default()
    });

    let Ok(out) = accelerator.matmul_f32(&lhs, &rhs) else {
        return;
    };

    assert_eq!(out, lhs.matmul_scalar(&rhs).unwrap());
    assert_eq!(
        accelerator.stats().last_backend,
        Some(RuntimeMathBackend::Wgpu)
    );
    assert_eq!(
        accelerator.stats().last_auto_reason,
        Some(RuntimeMathAutoSelectionReason::MatmulWgpuWorkThreshold)
    );
}

#[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
#[test]
fn prepared_matrix_matmul_reuses_gpu_buffers_when_adapter_is_available() {
    let lhs = DenseMatrixF32::new(
        8,
        16,
        (0_u16..128)
            .map(|value| f32::from(value % 11) - 5.0)
            .collect(),
    )
    .unwrap();
    let rhs = DenseMatrixF32::new(
        16,
        4,
        (0_u16..64)
            .map(|value| (f32::from(value % 7) - 3.0) * 0.25)
            .collect(),
    )
    .unwrap();
    let expected = lhs.matmul_scalar(&rhs).unwrap();
    let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
        backend: RuntimeMathBackend::Wgpu,
        ..RuntimeMathAcceleratorConfig::default()
    });
    let Ok(prepared) = accelerator.prepare_matrix_matmul_f32(&lhs, &rhs) else {
        return;
    };

    let mut out = vec![0.0; expected.values().len()];
    accelerator
        .run_prepared_matrix_matmul_f32_into(&prepared, &mut out)
        .expect("prepared GPU matrix matmul writes into caller buffer");
    accelerator
        .run_prepared_matrix_matmul_f32_into(&prepared, &mut out)
        .expect("prepared GPU matrix matmul reuses staging buffer");

    assert_eq!(out, expected.values());
    assert_eq!(accelerator.stats().wgpu_calls, 2);
    assert_eq!(accelerator.stats().gpu_buffer_creations, 4);
    assert_eq!(accelerator.stats().gpu_reused_dispatches, 2);
    assert_eq!(accelerator.stats().gpu_buffer_reuse_hits, 8);
    assert_eq!(accelerator.stats().gpu_staging_buffer_creations, 1);
    assert_eq!(accelerator.stats().gpu_staging_buffer_reuse_hits, 1);
    assert_eq!(
        accelerator.stats().bytes_uploaded,
        (lhs.values().len() + rhs.values().len()) * std::mem::size_of::<f32>()
    );
    assert_eq!(
        accelerator.stats().bytes_downloaded,
        std::mem::size_of_val(expected.values()) * 2
    );
}

#[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
#[test]
fn prepared_matrix_matmul_capacity_reuses_buffers_for_smaller_shapes() {
    let lhs = DenseMatrixF32::new(8, 8, vec![3.0; 64]).unwrap();
    let rhs = DenseMatrixF32::new(8, 8, vec![0.5; 64]).unwrap();
    let expected = lhs.matmul_scalar(&rhs).unwrap();
    let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
        backend: RuntimeMathBackend::Wgpu,
        ..RuntimeMathAcceleratorConfig::default()
    });
    let Ok(prepared) = accelerator.prepare_matrix_matmul_f32_capacity(16, 16, 16) else {
        return;
    };

    accelerator.reset_stats();
    accelerator
        .update_prepared_matrix_matmul_f32(&prepared, &lhs, &rhs)
        .expect("capacity-prepared matmul accepts smaller compatible input");
    let mut out = vec![0.0; expected.values().len()];
    accelerator
        .run_prepared_matrix_matmul_f32_shape_into(&prepared, 8, 8, &mut out)
        .expect("capacity-prepared matmul dispatches smaller output");

    assert_eq!(out, expected.values());
    assert_eq!(accelerator.stats().gpu_buffer_creations, 0);
    assert_eq!(accelerator.stats().gpu_buffer_reuse_hits, 7);
    assert_eq!(accelerator.stats().gpu_reused_dispatches, 1);
    assert_eq!(
        accelerator.stats().bytes_uploaded,
        (lhs.values().len() + rhs.values().len()) * std::mem::size_of::<f32>()
    );
    assert_eq!(
        accelerator.stats().bytes_downloaded,
        std::mem::size_of_val(expected.values())
    );
}

#[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
#[test]
fn prepared_matrix_matmul_bias_add_reuses_gpu_buffers_when_adapter_is_available() {
    let lhs = DenseMatrixF32::new(2, 3, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]).unwrap();
    let rhs = DenseMatrixF32::new(3, 2, vec![7.0, 8.0, 9.0, 10.0, 11.0, 12.0]).unwrap();
    let bias = DenseTensorF32::new(vec![2], vec![0.5, -0.25]).unwrap();
    let expected = DenseMatrixF32::new(2, 2, vec![58.5, 63.75, 139.5, 153.75]).unwrap();
    let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
        backend: RuntimeMathBackend::Wgpu,
        ..RuntimeMathAcceleratorConfig::default()
    });
    let Ok(prepared) = accelerator.prepare_matrix_matmul_bias_add_f32(&lhs, &rhs, &bias) else {
        return;
    };

    let mut out = vec![0.0; expected.values().len()];
    accelerator
        .run_prepared_matrix_matmul_bias_add_f32_into(&prepared, &mut out)
        .expect("prepared GPU matrix matmul-bias-add writes into caller buffer");
    accelerator
        .run_prepared_matrix_matmul_bias_add_f32_into(&prepared, &mut out)
        .expect("prepared GPU matrix matmul-bias-add reuses staging buffer");

    assert_eq!(out, expected.values());
    assert_eq!(accelerator.stats().wgpu_calls, 2);
    assert_eq!(accelerator.stats().fused_matmul_bias_add_calls, 2);
    assert_eq!(accelerator.stats().gpu_buffer_creations, 7);
    assert_eq!(accelerator.stats().gpu_reused_dispatches, 2);
    assert_eq!(accelerator.stats().gpu_buffer_reuse_hits, 14);
    assert_eq!(accelerator.stats().gpu_staging_buffer_creations, 1);
    assert_eq!(accelerator.stats().gpu_staging_buffer_reuse_hits, 1);
    assert_eq!(
        accelerator.stats().bytes_uploaded,
        (lhs.values().len() + rhs.values().len() + bias.values().len())
            * std::mem::size_of::<f32>()
    );
    assert_eq!(
        accelerator.stats().bytes_downloaded,
        std::mem::size_of_val(expected.values()) * 2
    );
}

#[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
#[test]
fn prepared_matrix_matmul_submit_only_defers_readback_when_adapter_is_available() {
    let lhs = DenseMatrixF32::new(2, 3, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]).unwrap();
    let rhs = DenseMatrixF32::new(3, 2, vec![7.0, 8.0, 9.0, 10.0, 11.0, 12.0]).unwrap();
    let expected = lhs.matmul_scalar(&rhs).unwrap();
    let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
        backend: RuntimeMathBackend::Wgpu,
        ..RuntimeMathAcceleratorConfig::default()
    });
    let Ok(prepared) = accelerator.prepare_matrix_matmul_f32(&lhs, &rhs) else {
        return;
    };

    accelerator.reset_stats();
    accelerator
        .submit_prepared_matrix_matmul_f32_without_readback(&prepared)
        .expect("prepared GPU matmul submit can defer readback");

    assert_eq!(accelerator.stats().wgpu_calls, 1);
    assert_eq!(accelerator.stats().gpu_reused_dispatches, 1);
    assert_eq!(accelerator.stats().gpu_buffer_reuse_hits, 4);
    assert_eq!(accelerator.stats().bytes_downloaded, 0);
    assert_eq!(accelerator.stats().gpu_staging_buffer_creations, 0);

    let mut out = vec![0.0; expected.values().len()];
    accelerator
        .read_prepared_matrix_matmul_f32_output_into(&prepared, &mut out)
        .expect("prepared GPU matmul output can be read after submit");

    assert_eq!(out, expected.values());
    assert_eq!(
        accelerator.stats().bytes_downloaded,
        std::mem::size_of_val(expected.values())
    );
    assert_eq!(accelerator.stats().gpu_staging_buffer_creations, 1);
}

#[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
#[test]
fn prepared_matrix_matmul_bias_add_submit_only_defers_readback_when_adapter_is_available() {
    let lhs = DenseMatrixF32::new(2, 3, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]).unwrap();
    let rhs = DenseMatrixF32::new(3, 2, vec![7.0, 8.0, 9.0, 10.0, 11.0, 12.0]).unwrap();
    let bias = DenseTensorF32::new(vec![2], vec![0.5, -0.25]).unwrap();
    let expected = DenseMatrixF32::new(2, 2, vec![58.5, 63.75, 139.5, 153.75]).unwrap();
    let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
        backend: RuntimeMathBackend::Wgpu,
        ..RuntimeMathAcceleratorConfig::default()
    });
    let Ok(prepared) = accelerator.prepare_matrix_matmul_bias_add_f32(&lhs, &rhs, &bias) else {
        return;
    };

    accelerator.reset_stats();
    accelerator
        .submit_prepared_matrix_matmul_bias_add_f32_without_readback(&prepared)
        .expect("prepared GPU matmul-bias-add submit can defer readback");

    assert_eq!(accelerator.stats().wgpu_calls, 1);
    assert_eq!(accelerator.stats().fused_matmul_bias_add_calls, 1);
    assert_eq!(accelerator.stats().gpu_reused_dispatches, 1);
    assert_eq!(accelerator.stats().gpu_buffer_reuse_hits, 7);
    assert_eq!(accelerator.stats().bytes_downloaded, 0);
    assert_eq!(accelerator.stats().gpu_staging_buffer_creations, 0);

    let mut out = vec![0.0; expected.values().len()];
    accelerator
        .read_prepared_matrix_matmul_bias_add_f32_output_into(&prepared, &mut out)
        .expect("prepared GPU matmul-bias-add output can be read after submit");

    assert_eq!(out, expected.values());
    assert_eq!(
        accelerator.stats().bytes_downloaded,
        std::mem::size_of_val(expected.values())
    );
    assert_eq!(accelerator.stats().gpu_staging_buffer_creations, 1);
}

#[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
#[test]
fn prepared_matrix_add_submit_only_defers_readback_when_adapter_is_available() {
    let lhs = DenseMatrixF32::new(2, 2, vec![1.0, 2.0, 3.0, 4.0]).unwrap();
    let rhs = DenseMatrixF32::new(2, 2, vec![0.5, 0.25, -1.0, 2.0]).unwrap();
    let expected = lhs.add_scalar(&rhs).unwrap();
    let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
        backend: RuntimeMathBackend::Wgpu,
        ..RuntimeMathAcceleratorConfig::default()
    });
    let Ok(prepared) = accelerator.prepare_matrix_add_f32(&lhs, &rhs) else {
        return;
    };

    accelerator.reset_stats();
    accelerator
        .submit_prepared_matrix_add_f32_without_readback(&prepared)
        .expect("prepared GPU matrix add submit can defer readback");

    assert_eq!(accelerator.stats().wgpu_calls, 1);
    assert_eq!(accelerator.stats().gpu_reused_dispatches, 1);
    assert_eq!(accelerator.stats().gpu_buffer_reuse_hits, 4);
    assert_eq!(accelerator.stats().bytes_downloaded, 0);
    assert_eq!(accelerator.stats().gpu_staging_buffer_creations, 0);

    let mut out = vec![0.0; expected.values().len()];
    accelerator
        .read_prepared_matrix_add_f32_output_into(&prepared, &mut out)
        .expect("prepared GPU matrix add output can be read after submit");

    assert_eq!(out, expected.values());
    assert_eq!(
        accelerator.stats().bytes_downloaded,
        std::mem::size_of_val(expected.values())
    );
    assert_eq!(accelerator.stats().gpu_staging_buffer_creations, 1);
}

#[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
#[test]
fn prepared_tensor_add_submit_only_defers_readback_when_adapter_is_available() {
    let lhs = DenseTensorF32::new(vec![4], vec![1.0, 2.0, 3.0, 4.0]).unwrap();
    let rhs = DenseTensorF32::new(vec![4], vec![0.5, 0.25, -1.0, 2.0]).unwrap();
    let expected = lhs.add_scalar(&rhs).unwrap();
    let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
        backend: RuntimeMathBackend::Wgpu,
        ..RuntimeMathAcceleratorConfig::default()
    });
    let Ok(prepared) = accelerator.prepare_tensor_add_f32(&lhs, &rhs) else {
        return;
    };

    accelerator.reset_stats();
    accelerator
        .submit_prepared_tensor_add_f32_without_readback(&prepared)
        .expect("prepared GPU tensor add submit can defer readback");

    assert_eq!(accelerator.stats().wgpu_calls, 1);
    assert_eq!(accelerator.stats().gpu_reused_dispatches, 1);
    assert_eq!(accelerator.stats().gpu_buffer_reuse_hits, 4);
    assert_eq!(accelerator.stats().bytes_downloaded, 0);
    assert_eq!(accelerator.stats().gpu_staging_buffer_creations, 0);

    let mut out = vec![0.0; expected.values().len()];
    accelerator
        .read_prepared_tensor_add_f32_output_into(&prepared, &mut out)
        .expect("prepared GPU tensor add output can be read after submit");

    assert_eq!(out, expected.values());
    assert_eq!(
        accelerator.stats().bytes_downloaded,
        std::mem::size_of_val(expected.values())
    );
    assert_eq!(accelerator.stats().gpu_staging_buffer_creations, 1);
}

#[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
#[test]
fn prepared_matrix_matmul_bias_add_capacity_reuses_buffers_for_smaller_shapes() {
    let lhs = DenseMatrixF32::new(2, 3, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]).unwrap();
    let rhs = DenseMatrixF32::new(3, 2, vec![7.0, 8.0, 9.0, 10.0, 11.0, 12.0]).unwrap();
    let bias = DenseTensorF32::new(vec![2], vec![0.5, -0.25]).unwrap();
    let expected = DenseMatrixF32::new(2, 2, vec![58.5, 63.75, 139.5, 153.75]).unwrap();
    let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
        backend: RuntimeMathBackend::Wgpu,
        ..RuntimeMathAcceleratorConfig::default()
    });
    let Ok(prepared) = accelerator.prepare_matrix_matmul_bias_add_f32_capacity(8, 8, 8) else {
        return;
    };

    accelerator.reset_stats();
    accelerator
        .update_prepared_matrix_matmul_bias_add_f32(&prepared, &lhs, &rhs, &bias)
        .expect("capacity-prepared matmul-bias-add accepts smaller compatible input");
    let mut out = vec![0.0; expected.values().len()];
    accelerator
        .run_prepared_matrix_matmul_bias_add_f32_shape_into(&prepared, 2, 2, &mut out)
        .expect("capacity-prepared matmul-bias-add dispatches smaller output");

    assert_eq!(out, expected.values());
    assert_eq!(accelerator.stats().gpu_buffer_creations, 0);
    assert_eq!(accelerator.stats().fused_matmul_bias_add_calls, 1);
    assert_eq!(accelerator.stats().gpu_buffer_reuse_hits, 12);
    assert_eq!(accelerator.stats().gpu_reused_dispatches, 1);
    assert_eq!(
        accelerator.stats().bytes_uploaded,
        (lhs.values().len() + rhs.values().len() + bias.values().len())
            * std::mem::size_of::<f32>()
    );
    assert_eq!(
        accelerator.stats().bytes_downloaded,
        std::mem::size_of_val(expected.values())
    );
}

#[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
#[test]
fn one_shot_wgpu_readback_reuses_staging_buffer_when_adapter_is_available() {
    let lhs = DenseMatrixF32::new(16, 16, vec![1.0; 256]).unwrap();
    let rhs = DenseMatrixF32::new(16, 16, vec![2.0; 256]).unwrap();
    let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
        backend: RuntimeMathBackend::Wgpu,
        ..RuntimeMathAcceleratorConfig::default()
    });

    let Ok(first) = accelerator.matrix_add_f32(&lhs, &rhs) else {
        return;
    };
    let second = accelerator
        .matrix_add_f32(&lhs, &rhs)
        .expect("second wgpu matrix add uses initialized adapter");

    assert_eq!(first.values(), &[3.0; 256]);
    assert_eq!(second.values(), &[3.0; 256]);
    assert_eq!(accelerator.stats().wgpu_calls, 2);
    assert_eq!(accelerator.stats().gpu_staging_buffer_creations, 1);
    assert_eq!(accelerator.stats().gpu_staging_buffer_reuse_hits, 1);
}

#[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
#[test]
fn prepared_tensor_add_reuses_gpu_buffers_when_adapter_is_available() {
    let lhs = DenseTensorF32::new(vec![1024], vec![1.0; 1024]).unwrap();
    let rhs = DenseTensorF32::new(vec![1024], vec![2.0; 1024]).unwrap();
    let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
        backend: RuntimeMathBackend::Wgpu,
        ..RuntimeMathAcceleratorConfig::default()
    });
    let Ok(prepared) = accelerator.prepare_tensor_add_f32(&lhs, &rhs) else {
        return;
    };

    let out = accelerator
        .run_prepared_tensor_add_f32(&prepared)
        .expect("prepared GPU tensor add dispatches");

    assert_eq!(out.values(), vec![3.0; 1024].as_slice());
    assert_eq!(accelerator.stats().wgpu_calls, 1);
    assert_eq!(accelerator.stats().gpu_buffer_creations, 4);
    assert_eq!(accelerator.stats().gpu_reused_dispatches, 1);
    assert_eq!(accelerator.stats().gpu_buffer_reuse_hits, 4);
    assert_eq!(
        accelerator.stats().bytes_uploaded,
        2048 * std::mem::size_of::<f32>()
    );
    assert_eq!(
        accelerator.stats().bytes_downloaded,
        1024 * std::mem::size_of::<f32>()
    );
}

#[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
#[test]
fn prepared_tensor_add_capacity_reuses_buffers_for_smaller_lengths() {
    let lhs = DenseTensorF32::new(vec![64], vec![6.0; 64]).unwrap();
    let rhs = DenseTensorF32::new(vec![64], vec![7.0; 64]).unwrap();
    let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
        backend: RuntimeMathBackend::Wgpu,
        ..RuntimeMathAcceleratorConfig::default()
    });
    let Ok(prepared) = accelerator.prepare_tensor_add_f32_capacity(128) else {
        return;
    };

    accelerator.reset_stats();
    accelerator
        .update_prepared_tensor_add_f32(&prepared, &lhs, &rhs)
        .expect("capacity-prepared tensor add accepts smaller compatible input");
    let mut out = vec![0.0; lhs.values().len()];
    accelerator
        .run_prepared_tensor_add_f32_len_into(&prepared, lhs.values().len(), &mut out)
        .expect("capacity-prepared tensor add dispatches smaller output");

    assert_eq!(out, vec![13.0; 64]);
    assert_eq!(accelerator.stats().gpu_buffer_creations, 0);
    assert_eq!(accelerator.stats().gpu_buffer_reuse_hits, 7);
    assert_eq!(accelerator.stats().gpu_reused_dispatches, 1);
    assert_eq!(
        accelerator.stats().bytes_uploaded,
        (lhs.values().len() + rhs.values().len()) * std::mem::size_of::<f32>()
    );
    assert_eq!(
        accelerator.stats().bytes_downloaded,
        std::mem::size_of_val(lhs.values())
    );
}

#[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
#[test]
fn prepared_matrix_add_can_write_into_reused_output_buffer() {
    let lhs = DenseMatrixF32::new(16, 16, vec![1.0; 256]).unwrap();
    let rhs = DenseMatrixF32::new(16, 16, vec![2.0; 256]).unwrap();
    let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
        backend: RuntimeMathBackend::Wgpu,
        ..RuntimeMathAcceleratorConfig::default()
    });
    let Ok(prepared) = accelerator.prepare_matrix_add_f32(&lhs, &rhs) else {
        return;
    };

    let mut out = vec![0.0; lhs.values().len()];
    accelerator
        .run_prepared_matrix_add_f32_into(&prepared, &mut out)
        .expect("prepared GPU matrix add writes into caller buffer");

    assert_eq!(out, vec![3.0; 256]);
    assert_eq!(accelerator.stats().gpu_reused_dispatches, 1);
    match accelerator
        .run_prepared_matrix_add_f32_into(&prepared, &mut out[..255])
        .unwrap_err()
    {
        RuntimeMathAcceleratorError::Math(RuntimeMathError::InvalidElementCount {
            expected,
            found,
        }) => {
            assert_eq!(expected, 256);
            assert_eq!(found, 255);
        }
        error => panic!("unexpected prepared output error: {error}"),
    }
}

#[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
#[test]
fn prepared_matrix_add_capacity_reuses_buffers_for_smaller_shapes() {
    let lhs = DenseMatrixF32::new(8, 8, vec![4.0; 64]).unwrap();
    let rhs = DenseMatrixF32::new(8, 8, vec![5.0; 64]).unwrap();
    let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
        backend: RuntimeMathBackend::Wgpu,
        ..RuntimeMathAcceleratorConfig::default()
    });
    let Ok(prepared) = accelerator.prepare_matrix_add_f32_capacity(16, 16) else {
        return;
    };

    accelerator.reset_stats();
    accelerator
        .update_prepared_matrix_add_f32(&prepared, &lhs, &rhs)
        .expect("capacity-prepared matrix add accepts smaller compatible input");
    let mut out = vec![0.0; lhs.values().len()];
    accelerator
        .run_prepared_matrix_add_f32_shape_into(&prepared, 8, 8, &mut out)
        .expect("capacity-prepared matrix add dispatches smaller output");

    assert_eq!(out, vec![9.0; 64]);
    assert_eq!(accelerator.stats().gpu_buffer_creations, 0);
    assert_eq!(accelerator.stats().gpu_buffer_reuse_hits, 7);
    assert_eq!(accelerator.stats().gpu_reused_dispatches, 1);
    assert_eq!(
        accelerator.stats().bytes_uploaded,
        (lhs.values().len() + rhs.values().len()) * std::mem::size_of::<f32>()
    );
    assert_eq!(
        accelerator.stats().bytes_downloaded,
        std::mem::size_of_val(lhs.values())
    );
}

#[test]
fn browser_webgpu_auto_policy_selects_matmul_modes_by_work_size() {
    use browser_webgpu_policy::{
        BrowserWebGpuMathAutoPolicy, BrowserWebGpuMathAutoReason, BrowserWebGpuMathMode,
    };

    let policy = BrowserWebGpuMathAutoPolicy::default();
    let limits = large_browser_webgpu_limits();

    let small = policy.select_matmul_f32(64, 64, 64, limits);
    assert_eq!(small.mode(), BrowserWebGpuMathMode::CpuWasm);
    assert_eq!(
        small.reason(),
        BrowserWebGpuMathAutoReason::MatmulCpuDefault
    );
    assert_eq!(small.capacity(), None);

    let exact = policy.select_matmul_f32(128, 128, 128, limits);
    assert_eq!(
        exact.mode(),
        BrowserWebGpuMathMode::WebGpuPreparedResidentPipelined
    );
    assert_eq!(
        exact.reason(),
        BrowserWebGpuMathAutoReason::MatmulPreparedResidentPipelined
    );
    let exact_capacity = exact
        .capacity()
        .expect("exact prepared matmul records exact capacity");
    assert_eq!(exact_capacity.rows, 128);
    assert_eq!(exact_capacity.shared, 128);
    assert_eq!(exact_capacity.cols, 128);

    let exact_256 = policy.select_matmul_f32(256, 256, 256, limits);
    assert_eq!(
        exact_256.mode(),
        BrowserWebGpuMathMode::WebGpuPreparedResidentPipelined
    );
    assert_eq!(
        exact_256.reason(),
        BrowserWebGpuMathAutoReason::MatmulPreparedResidentPipelined
    );
    let exact_256_capacity = exact_256
        .capacity()
        .expect("256 prepared matmul records exact capacity");
    assert_eq!(exact_256_capacity.rows, 256);
    assert_eq!(exact_256_capacity.shared, 256);
    assert_eq!(exact_256_capacity.cols, 256);

    let exact_512 = policy.select_matmul_f32(512, 512, 512, limits);
    assert_eq!(
        exact_512.mode(),
        BrowserWebGpuMathMode::WebGpuPreparedResidentPipelined
    );
    assert_eq!(
        exact_512.reason(),
        BrowserWebGpuMathAutoReason::MatmulPreparedResidentPipelined
    );
    let exact_512_capacity = exact_512
        .capacity()
        .expect("512 prepared matmul records exact capacity");
    assert_eq!(exact_512_capacity.rows, 512);
    assert_eq!(exact_512_capacity.shared, 512);
    assert_eq!(exact_512_capacity.cols, 512);

    let capacity_policy = BrowserWebGpuMathAutoPolicy::harness_capacity_matmul(
        512 * 512 * 512,
        browser_webgpu_policy::BrowserWebGpuCapacityGrowth::Double,
    );
    let grown = capacity_policy.select_matmul_f32(512, 512, 512, limits);
    assert_eq!(
        grown.mode(),
        BrowserWebGpuMathMode::WebGpuPreparedCapacityResidentPipelined
    );
    assert_eq!(
        grown.reason(),
        BrowserWebGpuMathAutoReason::MatmulPreparedCapacityResidentPipelined
    );
    let grown_capacity = grown
        .capacity()
        .expect("capacity-prepared matmul records grown capacity");
    assert_eq!(grown_capacity.rows, 1024);
    assert_eq!(grown_capacity.shared, 1024);
    assert_eq!(grown_capacity.cols, 1024);
}

#[test]
fn browser_webgpu_auto_policy_keeps_elementwise_cpu_by_default() {
    use browser_webgpu_policy::{
        BrowserWebGpuMathAutoPolicy, BrowserWebGpuMathAutoReason, BrowserWebGpuMathMode,
    };

    let policy = BrowserWebGpuMathAutoPolicy::default();
    let limits = large_browser_webgpu_limits();

    let selection = policy.select_elementwise_f32(4 * 1024 * 1024, limits);
    assert_eq!(selection.mode(), BrowserWebGpuMathMode::CpuWasm);
    assert_eq!(
        selection.reason(),
        BrowserWebGpuMathAutoReason::ElementwiseCpuReadbackDominated
    );

    let gpu_policy =
        BrowserWebGpuMathAutoPolicy::conservative().with_elementwise_gpu_min_elements(1024);
    let gpu_selection = gpu_policy.select_elementwise_f32(1024, limits);
    assert_eq!(
        gpu_selection.mode(),
        BrowserWebGpuMathMode::WebGpuPreparedResidentPipelined
    );
    assert_eq!(
        gpu_selection.reason(),
        BrowserWebGpuMathAutoReason::ElementwisePreparedResidentPipelined
    );
}

#[test]
fn browser_webgpu_policy_constructors_make_explicit_selection_visible() {
    use browser_webgpu_policy::{
        BrowserWebGpuCapacityGrowth, BrowserWebGpuMathAutoPolicy, BrowserWebGpuMathAutoReason,
        BrowserWebGpuMathMode,
    };

    let limits = large_browser_webgpu_limits();

    let cpu = BrowserWebGpuMathAutoPolicy::cpu_only();
    let cpu_matmul = cpu.select_matmul_f32(512, 512, 512, limits);
    assert_eq!(cpu_matmul.mode(), BrowserWebGpuMathMode::CpuWasm);
    assert_eq!(
        cpu_matmul.reason(),
        BrowserWebGpuMathAutoReason::MatmulCpuDefault
    );
    let cpu_elementwise = cpu.select_elementwise_f32(4 * 1024 * 1024, limits);
    assert_eq!(cpu_elementwise.mode(), BrowserWebGpuMathMode::CpuWasm);
    assert_eq!(
        cpu_elementwise.reason(),
        BrowserWebGpuMathAutoReason::ElementwiseCpuReadbackDominated
    );

    let explicit = BrowserWebGpuMathAutoPolicy::explicit_webgpu_resident();
    let explicit_matmul = explicit.select_matmul_f32(1, 2, 3, limits);
    assert_eq!(
        explicit_matmul.mode(),
        BrowserWebGpuMathMode::WebGpuPreparedResidentPipelined
    );
    assert_eq!(
        explicit_matmul.reason(),
        BrowserWebGpuMathAutoReason::MatmulPreparedResidentPipelined
    );
    let explicit_elementwise = explicit.select_elementwise_f32(1, limits);
    assert_eq!(
        explicit_elementwise.mode(),
        BrowserWebGpuMathMode::WebGpuPreparedResidentPipelined
    );
    assert_eq!(
        explicit_elementwise.reason(),
        BrowserWebGpuMathAutoReason::ElementwisePreparedResidentPipelined
    );

    let capacity = BrowserWebGpuMathAutoPolicy::harness_capacity_matmul(
        64 * 64 * 64,
        BrowserWebGpuCapacityGrowth::PowerOfTwo,
    );
    let capacity_matmul = capacity.select_matmul_f32(65, 66, 67, limits);
    assert_eq!(
        capacity_matmul.mode(),
        BrowserWebGpuMathMode::WebGpuPreparedCapacityResidentPipelined
    );
    assert_eq!(
        capacity_matmul.reason(),
        BrowserWebGpuMathAutoReason::MatmulPreparedCapacityResidentPipelined
    );
    let grown = capacity_matmul
        .capacity()
        .expect("capacity policy records grown matmul capacity");
    assert_eq!(grown.rows, 128);
    assert_eq!(grown.shared, 128);
    assert_eq!(grown.cols, 128);
}

#[test]
fn browser_webgpu_auto_policy_falls_back_on_storage_limits() {
    use browser_webgpu_policy::{
        BrowserWebGpuMathAutoPolicy, BrowserWebGpuMathAutoReason, BrowserWebGpuMathMode,
    };

    let limits = browser_webgpu_policy::BrowserWebGpuLimits {
        max_storage_buffer_binding_size: 1024,
        max_buffer_size: 1024,
        max_compute_invocations_per_workgroup: 256,
        max_compute_workgroups_per_dimension: 65_535,
    };
    let policy = BrowserWebGpuMathAutoPolicy {
        elementwise_gpu_min_elements: 1024,
        ..BrowserWebGpuMathAutoPolicy::default()
    };

    let matmul = policy.select_matmul_f32(256, 256, 256, limits);
    assert_eq!(matmul.mode(), BrowserWebGpuMathMode::CpuWasm);
    assert_eq!(matmul.reason(), BrowserWebGpuMathAutoReason::StorageLimit);

    let elementwise = policy.select_elementwise_f32(1024, limits);
    assert_eq!(elementwise.mode(), BrowserWebGpuMathMode::CpuWasm);
    assert_eq!(
        elementwise.reason(),
        BrowserWebGpuMathAutoReason::StorageLimit
    );
}

const fn large_browser_webgpu_limits() -> browser_webgpu_policy::BrowserWebGpuLimits {
    browser_webgpu_policy::BrowserWebGpuLimits {
        max_storage_buffer_binding_size: 1_u64 << 34,
        max_buffer_size: 1_u64 << 34,
        max_compute_invocations_per_workgroup: 256,
        max_compute_workgroups_per_dimension: 65_535,
    }
}
