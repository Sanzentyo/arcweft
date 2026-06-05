use arcweft_core::math::{DenseMatrixF32, DenseMatrixF64, DenseTensorF32, DenseTensorF64};
use arcweft_runtime_accelerator::inference::{
    AcceleratedInferenceAdapter, InferenceGraph, InferenceSession, InferenceShape,
    InferenceTensorId,
};
use arcweft_runtime_accelerator::math::{
    RuntimeMathAccelerator, RuntimeMathAcceleratorConfig, RuntimeMathAcceleratorError,
    RuntimeMathAutoSelectionReason, RuntimeMathBackend, RuntimeMathStats,
    RuntimePreparedMatrixAddF32, RuntimePreparedMatrixMatmulBiasAddF32,
    RuntimePreparedMatrixMatmulF32, RuntimePreparedTensorAddF32,
};
use serde::Serialize;
use std::time::Instant;

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let options = BenchOptions::parse(&args);
    let backends = match options.backend {
        BenchBackend::All => vec![
            RuntimeMathBackend::Scalar,
            RuntimeMathBackend::Glam,
            RuntimeMathBackend::Ndarray,
            RuntimeMathBackend::Wgpu,
            RuntimeMathBackend::Auto,
        ],
        BenchBackend::One(backend) => vec![backend],
    };
    let reports = backends
        .into_iter()
        .map(|backend| run_backend(backend, &options))
        .collect::<Vec<_>>();

    let report = MathBenchReport::new(&options, reports);
    println!(
        "{}",
        serde_json::to_string_pretty(&report).expect("math bench report serializes")
    );
}

fn run_backend(backend: RuntimeMathBackend, options: &BenchOptions) -> BackendReport {
    if matches!(options.op, BenchOp::InferenceMatmulBiasAdd) {
        return run_inference_matmul_bias_add(backend, options);
    }
    let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
        backend,
        wgpu_min_elements: options.wgpu_min_elements,
    });
    match options.op {
        BenchOp::Matmul => run_matmul(&mut accelerator, backend, options),
        BenchOp::MatmulBiasAdd => run_matmul_bias_add(&mut accelerator, backend, options),
        BenchOp::InferenceMatmulBiasAdd => unreachable!("inference op is handled before math"),
        BenchOp::MatrixAdd => run_matrix_add(&mut accelerator, backend, options),
        BenchOp::TensorAdd => run_tensor_add(&mut accelerator, backend, options),
        BenchOp::MatmulF64 => run_matmul_f64(&mut accelerator, backend, options),
        BenchOp::MatrixAddF64 => run_matrix_add_f64(&mut accelerator, backend, options),
        BenchOp::TensorAddF64 => run_tensor_add_f64(&mut accelerator, backend, options),
    }
}

fn run_matmul(
    accelerator: &mut RuntimeMathAccelerator,
    backend: RuntimeMathBackend,
    options: &BenchOptions,
) -> BackendReport {
    let lhs = matrix_fixture(options.size, options.size, 1.0);
    let rhs = matrix_fixture(options.size, options.size, 0.25);
    let reference = match lhs.matmul_scalar(&rhs) {
        Ok(value) => value,
        Err(error) => return BackendReport::failed(backend, error.to_string()),
    };

    if options.reuse_mode.is_enabled() {
        return run_prepared_matrix_matmul(accelerator, backend, options, &lhs, &rhs, &reference);
    }

    for _ in 0..options.warmup {
        if let Err(error) = accelerator.matmul_f32(&lhs, &rhs) {
            return BackendReport::skipped_or_failed(backend, error.to_string());
        }
    }

    let mut samples = Vec::with_capacity(options.iterations);
    for _ in 0..options.iterations {
        let started = Instant::now();
        let output = match accelerator.matmul_f32(&lhs, &rhs) {
            Ok(value) => value,
            Err(error) => return BackendReport::skipped_or_failed(backend, error.to_string()),
        };
        let elapsed = started.elapsed().as_nanos();
        if !approx_eq(output.values(), reference.values(), 1.0e-3) {
            return BackendReport::failed(backend, "matmul result mismatch".to_owned());
        }
        samples.push(elapsed);
    }
    BackendReport::measured(backend, samples, accelerator.stats())
}

fn run_matmul_bias_add(
    accelerator: &mut RuntimeMathAccelerator,
    backend: RuntimeMathBackend,
    options: &BenchOptions,
) -> BackendReport {
    let lhs = matrix_fixture(options.size, options.size, 1.0);
    let rhs = matrix_fixture(options.size, options.size, 0.25);
    let bias = bias_fixture(options.size);
    let mut reference = match lhs.matmul_scalar(&rhs) {
        Ok(value) => value,
        Err(error) => return BackendReport::failed(backend, error.to_string()),
    };
    apply_bias_to_reference(&mut reference, &bias);

    if options.reuse_mode.is_enabled() {
        return run_prepared_matrix_matmul_bias_add(
            accelerator,
            backend,
            options,
            &lhs,
            &rhs,
            &bias,
            &reference,
        );
    }

    for _ in 0..options.warmup {
        if let Err(error) = accelerator.matmul_bias_add_f32(&lhs, &rhs, &bias) {
            return BackendReport::skipped_or_failed(backend, error.to_string());
        }
    }

    let mut samples = Vec::with_capacity(options.iterations);
    for _ in 0..options.iterations {
        let started = Instant::now();
        let output = match accelerator.matmul_bias_add_f32(&lhs, &rhs, &bias) {
            Ok(value) => value,
            Err(error) => return BackendReport::skipped_or_failed(backend, error.to_string()),
        };
        let elapsed = started.elapsed().as_nanos();
        if !approx_eq(output.values(), reference.values(), 1.0e-3) {
            return BackendReport::failed(backend, "matmul-bias result mismatch".to_owned());
        }
        samples.push(elapsed);
    }
    BackendReport::measured(backend, samples, accelerator.stats())
}

fn run_matmul_f64(
    accelerator: &mut RuntimeMathAccelerator,
    backend: RuntimeMathBackend,
    options: &BenchOptions,
) -> BackendReport {
    if options.reuse_mode.is_enabled() {
        return BackendReport::failed(
            backend,
            "prepared reuse is only available for f32 benchmark ops".to_owned(),
        );
    }
    let lhs = matrix_fixture_f64(options.size, options.size, 1.0);
    let rhs = matrix_fixture_f64(options.size, options.size, 0.25);
    let reference = match lhs.matmul_scalar(&rhs) {
        Ok(value) => value,
        Err(error) => return BackendReport::failed(backend, error.to_string()),
    };

    for _ in 0..options.warmup {
        if let Err(error) = accelerator.matmul_f64(&lhs, &rhs) {
            return BackendReport::skipped_or_failed(backend, error.to_string());
        }
    }

    let mut samples = Vec::with_capacity(options.iterations);
    for _ in 0..options.iterations {
        let started = Instant::now();
        let output = match accelerator.matmul_f64(&lhs, &rhs) {
            Ok(value) => value,
            Err(error) => return BackendReport::skipped_or_failed(backend, error.to_string()),
        };
        let elapsed = started.elapsed().as_nanos();
        if !approx_eq_f64(output.values(), reference.values(), 1.0e-9) {
            return BackendReport::failed(backend, "matmul_f64 result mismatch".to_owned());
        }
        samples.push(elapsed);
    }
    BackendReport::measured(backend, samples, accelerator.stats())
}

fn run_matrix_add_f64(
    accelerator: &mut RuntimeMathAccelerator,
    backend: RuntimeMathBackend,
    options: &BenchOptions,
) -> BackendReport {
    if options.reuse_mode.is_enabled() {
        return BackendReport::failed(
            backend,
            "prepared reuse is only available for f32 benchmark ops".to_owned(),
        );
    }
    let lhs = matrix_fixture_f64(options.size, options.size, 1.0);
    let rhs = matrix_fixture_f64(options.size, options.size, 0.25);
    let reference = match lhs.add_scalar(&rhs) {
        Ok(value) => value,
        Err(error) => return BackendReport::failed(backend, error.to_string()),
    };

    for _ in 0..options.warmup {
        if let Err(error) = accelerator.matrix_add_f64(&lhs, &rhs) {
            return BackendReport::skipped_or_failed(backend, error.to_string());
        }
    }

    let mut samples = Vec::with_capacity(options.iterations);
    for _ in 0..options.iterations {
        let started = Instant::now();
        let output = match accelerator.matrix_add_f64(&lhs, &rhs) {
            Ok(value) => value,
            Err(error) => return BackendReport::skipped_or_failed(backend, error.to_string()),
        };
        let elapsed = started.elapsed().as_nanos();
        if !approx_eq_f64(output.values(), reference.values(), 1.0e-12) {
            return BackendReport::failed(backend, "matrix_add_f64 result mismatch".to_owned());
        }
        samples.push(elapsed);
    }
    BackendReport::measured(backend, samples, accelerator.stats())
}

fn run_tensor_add_f64(
    accelerator: &mut RuntimeMathAccelerator,
    backend: RuntimeMathBackend,
    options: &BenchOptions,
) -> BackendReport {
    if options.reuse_mode.is_enabled() {
        return BackendReport::failed(
            backend,
            "prepared reuse is only available for f32 benchmark ops".to_owned(),
        );
    }
    let elements = options.size * options.size;
    let lhs = tensor_fixture_f64(elements, 1.0);
    let rhs = tensor_fixture_f64(elements, 0.25);
    let reference = match lhs.add_scalar(&rhs) {
        Ok(value) => value,
        Err(error) => return BackendReport::failed(backend, error.to_string()),
    };

    for _ in 0..options.warmup {
        if let Err(error) = accelerator.tensor_add_f64(&lhs, &rhs) {
            return BackendReport::skipped_or_failed(backend, error.to_string());
        }
    }

    let mut samples = Vec::with_capacity(options.iterations);
    for _ in 0..options.iterations {
        let started = Instant::now();
        let output = match accelerator.tensor_add_f64(&lhs, &rhs) {
            Ok(value) => value,
            Err(error) => return BackendReport::skipped_or_failed(backend, error.to_string()),
        };
        let elapsed = started.elapsed().as_nanos();
        if !approx_eq_f64(output.values(), reference.values(), 1.0e-12) {
            return BackendReport::failed(backend, "tensor_add_f64 result mismatch".to_owned());
        }
        samples.push(elapsed);
    }
    BackendReport::measured(backend, samples, accelerator.stats())
}

fn run_inference_matmul_bias_add(
    backend: RuntimeMathBackend,
    options: &BenchOptions,
) -> BackendReport {
    if options.reuse_mode.uses_capacity() {
        return BackendReport::skipped_or_failed(
            backend,
            "inference graph shape is fixed; capacity reuse belongs to prepared math ops"
                .to_owned(),
        );
    }
    let rhs = matrix_fixture(options.size, options.size, 0.25);
    let bias = bias_fixture(options.size);
    let (graph, input_id) = match inference_matmul_bias_graph(options.size, &rhs, &bias) {
        Ok(value) => value,
        Err(error) => return BackendReport::failed(backend, error),
    };
    let cases = inference_matmul_bias_cases(
        options.size,
        &rhs,
        &bias,
        options.warmup + options.iterations,
        options.reuse_mode.updates_inputs(),
    );
    if options.reuse_mode.is_enabled() {
        return run_reused_inference_session(backend, options, graph, input_id, &cases);
    }
    run_cold_inference_sessions(backend, options, &graph, input_id, &cases)
}

fn run_reused_inference_session(
    backend: RuntimeMathBackend,
    options: &BenchOptions,
    graph: InferenceGraph,
    input_id: InferenceTensorId,
    cases: &[InferenceMatmulBiasCase],
) -> BackendReport {
    let mut session = inference_session(backend, options, graph);
    for case in cases.iter().take(options.warmup) {
        if let Err(error) = run_inference_case(&mut session, input_id, case) {
            return BackendReport::skipped_or_failed(backend, error);
        }
    }
    let mut samples = Vec::with_capacity(options.iterations);
    for case in cases.iter().skip(options.warmup).take(options.iterations) {
        let started = Instant::now();
        if let Err(error) = run_inference_case(&mut session, input_id, case) {
            return BackendReport::skipped_or_failed(backend, error);
        }
        samples.push(started.elapsed().as_nanos());
    }
    BackendReport::measured(backend, samples, session.adapter().accelerator().stats())
}

fn run_cold_inference_sessions(
    backend: RuntimeMathBackend,
    options: &BenchOptions,
    graph: &InferenceGraph,
    input_id: InferenceTensorId,
    cases: &[InferenceMatmulBiasCase],
) -> BackendReport {
    let mut stats = RuntimeMathStats::default();
    for case in cases.iter().take(options.warmup) {
        let mut session = inference_session(backend, options, graph.clone());
        if let Err(error) = run_inference_case(&mut session, input_id, case) {
            return BackendReport::skipped_or_failed(backend, error);
        }
        add_math_stats(&mut stats, session.adapter().accelerator().stats());
    }
    let mut samples = Vec::with_capacity(options.iterations);
    for case in cases.iter().skip(options.warmup).take(options.iterations) {
        let mut session = inference_session(backend, options, graph.clone());
        let started = Instant::now();
        if let Err(error) = run_inference_case(&mut session, input_id, case) {
            return BackendReport::skipped_or_failed(backend, error);
        }
        samples.push(started.elapsed().as_nanos());
        add_math_stats(&mut stats, session.adapter().accelerator().stats());
    }
    BackendReport::measured(backend, samples, stats)
}

fn inference_session(
    backend: RuntimeMathBackend,
    options: &BenchOptions,
    graph: InferenceGraph,
) -> InferenceSession<AcceleratedInferenceAdapter> {
    InferenceSession::new(
        graph,
        AcceleratedInferenceAdapter::new(RuntimeMathAccelerator::new(
            RuntimeMathAcceleratorConfig {
                backend,
                wgpu_min_elements: options.wgpu_min_elements,
            },
        )),
    )
}

fn run_inference_case(
    session: &mut InferenceSession<AcceleratedInferenceAdapter>,
    input_id: InferenceTensorId,
    case: &InferenceMatmulBiasCase,
) -> Result<(), String> {
    let output = session
        .run_borrowed([(input_id, &case.input)])
        .map_err(|error| error.to_string())?;
    let tensor = output
        .first()
        .and_then(arcweft_runtime_accelerator::inference::InferenceValue::as_tensor)
        .ok_or_else(|| "inference output is not a tensor".to_owned())?;
    if approx_eq(tensor.values(), case.reference.values(), 1.0e-3) {
        Ok(())
    } else {
        Err("inference matmul-bias result mismatch".to_owned())
    }
}

fn run_prepared_matrix_matmul_bias_add(
    accelerator: &mut RuntimeMathAccelerator,
    backend: RuntimeMathBackend,
    options: &BenchOptions,
    lhs: &DenseMatrixF32,
    rhs: &DenseMatrixF32,
    bias: &DenseTensorF32,
    reference: &DenseMatrixF32,
) -> BackendReport {
    if backend != RuntimeMathBackend::Wgpu {
        return BackendReport::skipped_or_failed(
            backend,
            "prepared GPU reuse is only available for the wgpu backend".to_owned(),
        );
    }
    let prepared = match if options.reuse_mode.uses_capacity() {
        let capacity = options.matrix_capacity_size();
        accelerator.prepare_matrix_matmul_bias_add_f32_capacity(capacity, capacity, capacity)
    } else {
        accelerator.prepare_matrix_matmul_bias_add_f32(lhs, rhs, bias)
    } {
        Ok(value) => value,
        Err(error) => return BackendReport::skipped_or_failed(backend, error.to_string()),
    };
    let update_cases = options
        .reuse_mode
        .updates_inputs()
        .then(|| matmul_bias_update_cases(options.size, options.warmup + options.iterations));
    if options.reuse_mode.uses_capacity()
        && update_cases.is_none()
        && let Err(error) =
            accelerator.update_prepared_matrix_matmul_bias_add_f32(&prepared, lhs, rhs, bias)
    {
        return BackendReport::skipped_or_failed(backend, error.to_string());
    }
    let mut output = vec![0.0; lhs.rows() * rhs.cols()];
    for index in 0..options.warmup {
        if let Some(cases) = &update_cases {
            let (lhs, rhs, bias, _) = &cases[index];
            if let Err(error) =
                accelerator.update_prepared_matrix_matmul_bias_add_f32(&prepared, lhs, rhs, bias)
            {
                return BackendReport::skipped_or_failed(backend, error.to_string());
            }
        }
        let result = dispatch_prepared_matmul_bias_add_sample(
            accelerator,
            options.reuse_mode,
            &prepared,
            lhs.rows(),
            rhs.cols(),
            &mut output,
        );
        if let Err(error) = result {
            return BackendReport::skipped_or_failed(backend, error.to_string());
        }
    }
    let mut samples = Vec::with_capacity(options.iterations);
    for index in 0..options.iterations {
        let case = update_cases
            .as_ref()
            .map(|cases| &cases[options.warmup + index]);
        let started = Instant::now();
        if let Some((lhs, rhs, bias, _)) = case
            && let Err(error) =
                accelerator.update_prepared_matrix_matmul_bias_add_f32(&prepared, lhs, rhs, bias)
        {
            return BackendReport::skipped_or_failed(backend, error.to_string());
        }
        let result = dispatch_prepared_matmul_bias_add_sample(
            accelerator,
            options.reuse_mode,
            &prepared,
            lhs.rows(),
            rhs.cols(),
            &mut output,
        );
        if let Err(error) = result {
            return BackendReport::skipped_or_failed(backend, error.to_string());
        }
        let elapsed = started.elapsed().as_nanos();
        let expected = case.map_or_else(
            || reference.values(),
            |(_, _, _, reference)| reference.values(),
        );
        if !options.reuse_mode.submit_only() && !approx_eq(&output, expected, 1.0e-3) {
            return BackendReport::failed(
                backend,
                "prepared matrix matmul-bias-add result mismatch".to_owned(),
            );
        }
        samples.push(elapsed);
    }
    if let Err(error) = validate_matmul_bias_submit_only_output(
        accelerator,
        options.reuse_mode,
        &prepared,
        (lhs.rows(), rhs.cols()),
        &mut output,
        update_cases.as_deref(),
        reference,
    ) {
        return error.into_report(backend);
    }
    BackendReport::measured(backend, samples, accelerator.stats())
}

type MatmulUpdateCase = (DenseMatrixF32, DenseMatrixF32, DenseMatrixF32);
type TensorAddUpdateCase = (DenseTensorF32, DenseTensorF32, DenseTensorF32);
type MatmulBiasUpdateCase = (
    DenseMatrixF32,
    DenseMatrixF32,
    DenseTensorF32,
    DenseMatrixF32,
);

enum SubmitOnlyValidationError {
    Backend(String),
    Mismatch(&'static str),
}

impl SubmitOnlyValidationError {
    fn into_report(self, backend: RuntimeMathBackend) -> BackendReport {
        match self {
            Self::Backend(diagnostic) => BackendReport::skipped_or_failed(backend, diagnostic),
            Self::Mismatch(diagnostic) => BackendReport::failed(backend, diagnostic.to_owned()),
        }
    }
}

fn validate_matmul_bias_submit_only_output(
    accelerator: &mut RuntimeMathAccelerator,
    reuse_mode: PreparedReuseMode,
    prepared: &RuntimePreparedMatrixMatmulBiasAddF32,
    shape: (usize, usize),
    output: &mut [f32],
    update_cases: Option<&[MatmulBiasUpdateCase]>,
    reference: &DenseMatrixF32,
) -> Result<(), SubmitOnlyValidationError> {
    if reuse_mode.submit_only() {
        read_prepared_matmul_bias_add_output(
            accelerator,
            reuse_mode,
            prepared,
            shape.0,
            shape.1,
            output,
        )
        .map_err(|error| SubmitOnlyValidationError::Backend(error.to_string()))?;
        let expected = update_cases.and_then(|cases| cases.last()).map_or_else(
            || reference.values(),
            |(_, _, _, reference)| reference.values(),
        );
        if !approx_eq(output, expected, 1.0e-3) {
            return Err(SubmitOnlyValidationError::Mismatch(
                "prepared matrix matmul-bias-add submit-only result mismatch",
            ));
        }
    }
    Ok(())
}

fn run_matrix_add(
    accelerator: &mut RuntimeMathAccelerator,
    backend: RuntimeMathBackend,
    options: &BenchOptions,
) -> BackendReport {
    let lhs = matrix_fixture(options.size, options.size, 1.0);
    let rhs = matrix_fixture(options.size, options.size, 0.25);
    let reference = match lhs.add_scalar(&rhs) {
        Ok(value) => value,
        Err(error) => return BackendReport::failed(backend, error.to_string()),
    };

    if options.reuse_mode.is_enabled() {
        return run_prepared_matrix_add(accelerator, backend, options, &lhs, &rhs, &reference);
    }

    for _ in 0..options.warmup {
        if let Err(error) = accelerator.matrix_add_f32(&lhs, &rhs) {
            return BackendReport::skipped_or_failed(backend, error.to_string());
        }
    }

    let mut samples = Vec::with_capacity(options.iterations);
    for _ in 0..options.iterations {
        let started = Instant::now();
        let output = match accelerator.matrix_add_f32(&lhs, &rhs) {
            Ok(value) => value,
            Err(error) => return BackendReport::skipped_or_failed(backend, error.to_string()),
        };
        let elapsed = started.elapsed().as_nanos();
        if !approx_eq(output.values(), reference.values(), 1.0e-6) {
            return BackendReport::failed(backend, "matrix add result mismatch".to_owned());
        }
        samples.push(elapsed);
    }
    BackendReport::measured(backend, samples, accelerator.stats())
}

fn run_tensor_add(
    accelerator: &mut RuntimeMathAccelerator,
    backend: RuntimeMathBackend,
    options: &BenchOptions,
) -> BackendReport {
    let elements = options.size * options.size;
    let lhs = tensor_fixture(elements, 1.0);
    let rhs = tensor_fixture(elements, 0.25);
    let reference = match lhs.add_scalar(&rhs) {
        Ok(value) => value,
        Err(error) => return BackendReport::failed(backend, error.to_string()),
    };

    if options.reuse_mode.is_enabled() {
        return run_prepared_tensor_add(accelerator, backend, options, &lhs, &rhs, &reference);
    }

    for _ in 0..options.warmup {
        if let Err(error) = accelerator.tensor_add_f32(&lhs, &rhs) {
            return BackendReport::skipped_or_failed(backend, error.to_string());
        }
    }

    let mut samples = Vec::with_capacity(options.iterations);
    for _ in 0..options.iterations {
        let started = Instant::now();
        let output = match accelerator.tensor_add_f32(&lhs, &rhs) {
            Ok(value) => value,
            Err(error) => return BackendReport::skipped_or_failed(backend, error.to_string()),
        };
        let elapsed = started.elapsed().as_nanos();
        if !approx_eq(output.values(), reference.values(), 1.0e-6) {
            return BackendReport::failed(backend, "tensor add result mismatch".to_owned());
        }
        samples.push(elapsed);
    }
    BackendReport::measured(backend, samples, accelerator.stats())
}

fn run_prepared_matrix_matmul(
    accelerator: &mut RuntimeMathAccelerator,
    backend: RuntimeMathBackend,
    options: &BenchOptions,
    lhs: &DenseMatrixF32,
    rhs: &DenseMatrixF32,
    reference: &DenseMatrixF32,
) -> BackendReport {
    if backend != RuntimeMathBackend::Wgpu {
        return BackendReport::skipped_or_failed(
            backend,
            "prepared GPU reuse is only available for the wgpu backend".to_owned(),
        );
    }
    let prepared = match if options.reuse_mode.uses_capacity() {
        let capacity = options.matrix_capacity_size();
        accelerator.prepare_matrix_matmul_f32_capacity(capacity, capacity, capacity)
    } else {
        accelerator.prepare_matrix_matmul_f32(lhs, rhs)
    } {
        Ok(value) => value,
        Err(error) => return BackendReport::skipped_or_failed(backend, error.to_string()),
    };
    let update_cases = options
        .reuse_mode
        .updates_inputs()
        .then(|| matrix_update_cases(options.size, options.warmup + options.iterations, true));
    if options.reuse_mode.uses_capacity()
        && update_cases.is_none()
        && let Err(error) = accelerator.update_prepared_matrix_matmul_f32(&prepared, lhs, rhs)
    {
        return BackendReport::skipped_or_failed(backend, error.to_string());
    }
    let mut output = vec![0.0; lhs.rows() * rhs.cols()];
    for index in 0..options.warmup {
        if let Some(cases) = &update_cases {
            let (lhs, rhs, _) = &cases[index];
            if let Err(error) = accelerator.update_prepared_matrix_matmul_f32(&prepared, lhs, rhs) {
                return BackendReport::skipped_or_failed(backend, error.to_string());
            }
        }
        let result = dispatch_prepared_matmul_sample(
            accelerator,
            options.reuse_mode,
            &prepared,
            lhs.rows(),
            rhs.cols(),
            &mut output,
        );
        if let Err(error) = result {
            return BackendReport::skipped_or_failed(backend, error.to_string());
        }
    }
    let mut samples = Vec::with_capacity(options.iterations);
    for index in 0..options.iterations {
        let case = update_cases
            .as_ref()
            .map(|cases| &cases[options.warmup + index]);
        let started = Instant::now();
        if let Some((lhs, rhs, _)) = case
            && let Err(error) = accelerator.update_prepared_matrix_matmul_f32(&prepared, lhs, rhs)
        {
            return BackendReport::skipped_or_failed(backend, error.to_string());
        }
        let result = dispatch_prepared_matmul_sample(
            accelerator,
            options.reuse_mode,
            &prepared,
            lhs.rows(),
            rhs.cols(),
            &mut output,
        );
        if let Err(error) = result {
            return BackendReport::skipped_or_failed(backend, error.to_string());
        }
        let elapsed = started.elapsed().as_nanos();
        let expected = case.map_or_else(
            || reference.values(),
            |(_, _, reference)| reference.values(),
        );
        if !options.reuse_mode.submit_only() && !approx_eq(&output, expected, 1.0e-3) {
            return BackendReport::failed(
                backend,
                "prepared matrix matmul result mismatch".to_owned(),
            );
        }
        samples.push(elapsed);
    }
    if let Err(error) = validate_matmul_submit_only_output(
        accelerator,
        options.reuse_mode,
        &prepared,
        (lhs.rows(), rhs.cols()),
        &mut output,
        update_cases.as_deref(),
        reference,
    ) {
        return error.into_report(backend);
    }
    BackendReport::measured(backend, samples, accelerator.stats())
}

fn validate_matmul_submit_only_output(
    accelerator: &mut RuntimeMathAccelerator,
    reuse_mode: PreparedReuseMode,
    prepared: &RuntimePreparedMatrixMatmulF32,
    shape: (usize, usize),
    output: &mut [f32],
    update_cases: Option<&[MatmulUpdateCase]>,
    reference: &DenseMatrixF32,
) -> Result<(), SubmitOnlyValidationError> {
    if reuse_mode.submit_only() {
        read_prepared_matmul_output(accelerator, reuse_mode, prepared, shape.0, shape.1, output)
            .map_err(|error| SubmitOnlyValidationError::Backend(error.to_string()))?;
        let expected = update_cases.and_then(|cases| cases.last()).map_or_else(
            || reference.values(),
            |(_, _, reference)| reference.values(),
        );
        if !approx_eq(output, expected, 1.0e-3) {
            return Err(SubmitOnlyValidationError::Mismatch(
                "prepared matrix matmul submit-only result mismatch",
            ));
        }
    }
    Ok(())
}

fn dispatch_prepared_matmul_bias_add_sample(
    accelerator: &mut RuntimeMathAccelerator,
    reuse_mode: PreparedReuseMode,
    prepared: &RuntimePreparedMatrixMatmulBiasAddF32,
    rows: usize,
    cols: usize,
    output: &mut [f32],
) -> Result<(), RuntimeMathAcceleratorError> {
    if reuse_mode.submit_only() && reuse_mode.uses_capacity() {
        accelerator
            .submit_prepared_matrix_matmul_bias_add_f32_shape_without_readback(prepared, rows, cols)
    } else if reuse_mode.submit_only() {
        accelerator.submit_prepared_matrix_matmul_bias_add_f32_without_readback(prepared)
    } else if reuse_mode.uses_capacity() {
        accelerator.run_prepared_matrix_matmul_bias_add_f32_shape_into(prepared, rows, cols, output)
    } else {
        accelerator.run_prepared_matrix_matmul_bias_add_f32_into(prepared, output)
    }
}

fn read_prepared_matmul_bias_add_output(
    accelerator: &mut RuntimeMathAccelerator,
    reuse_mode: PreparedReuseMode,
    prepared: &RuntimePreparedMatrixMatmulBiasAddF32,
    rows: usize,
    cols: usize,
    output: &mut [f32],
) -> Result<(), RuntimeMathAcceleratorError> {
    if reuse_mode.uses_capacity() {
        accelerator.read_prepared_matrix_matmul_bias_add_f32_shape_output_into(
            prepared, rows, cols, output,
        )
    } else {
        accelerator.read_prepared_matrix_matmul_bias_add_f32_output_into(prepared, output)
    }
}

fn dispatch_prepared_matmul_sample(
    accelerator: &mut RuntimeMathAccelerator,
    reuse_mode: PreparedReuseMode,
    prepared: &RuntimePreparedMatrixMatmulF32,
    rows: usize,
    cols: usize,
    output: &mut [f32],
) -> Result<(), RuntimeMathAcceleratorError> {
    if reuse_mode.submit_only() && reuse_mode.uses_capacity() {
        accelerator.submit_prepared_matrix_matmul_f32_shape_without_readback(prepared, rows, cols)
    } else if reuse_mode.submit_only() {
        accelerator.submit_prepared_matrix_matmul_f32_without_readback(prepared)
    } else if reuse_mode.uses_capacity() {
        accelerator.run_prepared_matrix_matmul_f32_shape_into(prepared, rows, cols, output)
    } else {
        accelerator.run_prepared_matrix_matmul_f32_into(prepared, output)
    }
}

fn read_prepared_matmul_output(
    accelerator: &mut RuntimeMathAccelerator,
    reuse_mode: PreparedReuseMode,
    prepared: &RuntimePreparedMatrixMatmulF32,
    rows: usize,
    cols: usize,
    output: &mut [f32],
) -> Result<(), RuntimeMathAcceleratorError> {
    if reuse_mode.uses_capacity() {
        accelerator.read_prepared_matrix_matmul_f32_shape_output_into(prepared, rows, cols, output)
    } else {
        accelerator.read_prepared_matrix_matmul_f32_output_into(prepared, output)
    }
}

fn validate_matrix_add_submit_only_output(
    accelerator: &mut RuntimeMathAccelerator,
    reuse_mode: PreparedReuseMode,
    prepared: &RuntimePreparedMatrixAddF32,
    shape: (usize, usize),
    output: &mut [f32],
    update_cases: Option<&[MatmulUpdateCase]>,
    reference: &DenseMatrixF32,
) -> Result<(), SubmitOnlyValidationError> {
    if reuse_mode.submit_only() {
        read_prepared_matrix_add_output(accelerator, reuse_mode, prepared, shape, output)
            .map_err(|error| SubmitOnlyValidationError::Backend(error.to_string()))?;
        let expected = update_cases.and_then(|cases| cases.last()).map_or_else(
            || reference.values(),
            |(_, _, reference)| reference.values(),
        );
        if !approx_eq(output, expected, 1.0e-6) {
            return Err(SubmitOnlyValidationError::Mismatch(
                "prepared matrix add submit-only result mismatch",
            ));
        }
    }
    Ok(())
}

fn dispatch_prepared_matrix_add_sample(
    accelerator: &mut RuntimeMathAccelerator,
    reuse_mode: PreparedReuseMode,
    prepared: &RuntimePreparedMatrixAddF32,
    shape: (usize, usize),
    output: &mut [f32],
) -> Result<(), RuntimeMathAcceleratorError> {
    if reuse_mode.submit_only() && reuse_mode.uses_capacity() {
        accelerator
            .submit_prepared_matrix_add_f32_shape_without_readback(prepared, shape.0, shape.1)
    } else if reuse_mode.submit_only() {
        accelerator.submit_prepared_matrix_add_f32_without_readback(prepared)
    } else if reuse_mode.uses_capacity() {
        accelerator.run_prepared_matrix_add_f32_shape_into(prepared, shape.0, shape.1, output)
    } else {
        accelerator.run_prepared_matrix_add_f32_into(prepared, output)
    }
}

fn read_prepared_matrix_add_output(
    accelerator: &mut RuntimeMathAccelerator,
    reuse_mode: PreparedReuseMode,
    prepared: &RuntimePreparedMatrixAddF32,
    shape: (usize, usize),
    output: &mut [f32],
) -> Result<(), RuntimeMathAcceleratorError> {
    if reuse_mode.uses_capacity() {
        accelerator
            .read_prepared_matrix_add_f32_shape_output_into(prepared, shape.0, shape.1, output)
    } else {
        accelerator.read_prepared_matrix_add_f32_output_into(prepared, output)
    }
}

fn run_prepared_matrix_add(
    accelerator: &mut RuntimeMathAccelerator,
    backend: RuntimeMathBackend,
    options: &BenchOptions,
    lhs: &DenseMatrixF32,
    rhs: &DenseMatrixF32,
    reference: &DenseMatrixF32,
) -> BackendReport {
    if backend != RuntimeMathBackend::Wgpu {
        return BackendReport::skipped_or_failed(
            backend,
            "prepared GPU reuse is only available for the wgpu backend".to_owned(),
        );
    }
    let prepared = match if options.reuse_mode.uses_capacity() {
        let capacity = options.matrix_capacity_size();
        accelerator.prepare_matrix_add_f32_capacity(capacity, capacity)
    } else {
        accelerator.prepare_matrix_add_f32(lhs, rhs)
    } {
        Ok(value) => value,
        Err(error) => return BackendReport::skipped_or_failed(backend, error.to_string()),
    };
    let update_cases = options
        .reuse_mode
        .updates_inputs()
        .then(|| matrix_update_cases(options.size, options.warmup + options.iterations, false));
    if options.reuse_mode.uses_capacity()
        && update_cases.is_none()
        && let Err(error) = accelerator.update_prepared_matrix_add_f32(&prepared, lhs, rhs)
    {
        return BackendReport::skipped_or_failed(backend, error.to_string());
    }
    let mut output = vec![0.0; lhs.values().len()];
    for index in 0..options.warmup {
        if let Some(cases) = &update_cases {
            let (lhs, rhs, _) = &cases[index];
            if let Err(error) = accelerator.update_prepared_matrix_add_f32(&prepared, lhs, rhs) {
                return BackendReport::skipped_or_failed(backend, error.to_string());
            }
        }
        let result = dispatch_prepared_matrix_add_sample(
            accelerator,
            options.reuse_mode,
            &prepared,
            (lhs.rows(), lhs.cols()),
            &mut output,
        );
        if let Err(error) = result {
            return BackendReport::skipped_or_failed(backend, error.to_string());
        }
    }
    let mut samples = Vec::with_capacity(options.iterations);
    for index in 0..options.iterations {
        let case = update_cases
            .as_ref()
            .map(|cases| &cases[options.warmup + index]);
        let started = Instant::now();
        if let Some((lhs, rhs, _)) = case
            && let Err(error) = accelerator.update_prepared_matrix_add_f32(&prepared, lhs, rhs)
        {
            return BackendReport::skipped_or_failed(backend, error.to_string());
        }
        let result = dispatch_prepared_matrix_add_sample(
            accelerator,
            options.reuse_mode,
            &prepared,
            (lhs.rows(), lhs.cols()),
            &mut output,
        );
        if let Err(error) = result {
            return BackendReport::skipped_or_failed(backend, error.to_string());
        }
        let elapsed = started.elapsed().as_nanos();
        let expected = case.map_or_else(
            || reference.values(),
            |(_, _, reference)| reference.values(),
        );
        if !options.reuse_mode.submit_only() && !approx_eq(&output, expected, 1.0e-6) {
            return BackendReport::failed(
                backend,
                "prepared matrix add result mismatch".to_owned(),
            );
        }
        samples.push(elapsed);
    }
    if let Err(error) = validate_matrix_add_submit_only_output(
        accelerator,
        options.reuse_mode,
        &prepared,
        (lhs.rows(), lhs.cols()),
        &mut output,
        update_cases.as_deref(),
        reference,
    ) {
        return error.into_report(backend);
    }
    BackendReport::measured(backend, samples, accelerator.stats())
}

fn validate_tensor_add_submit_only_output(
    accelerator: &mut RuntimeMathAccelerator,
    reuse_mode: PreparedReuseMode,
    prepared: &RuntimePreparedTensorAddF32,
    len: usize,
    output: &mut [f32],
    update_cases: Option<&[TensorAddUpdateCase]>,
    reference: &DenseTensorF32,
) -> Result<(), SubmitOnlyValidationError> {
    if reuse_mode.submit_only() {
        read_prepared_tensor_add_output(accelerator, reuse_mode, prepared, len, output)
            .map_err(|error| SubmitOnlyValidationError::Backend(error.to_string()))?;
        let expected = update_cases.and_then(|cases| cases.last()).map_or_else(
            || reference.values(),
            |(_, _, reference)| reference.values(),
        );
        if !approx_eq(output, expected, 1.0e-6) {
            return Err(SubmitOnlyValidationError::Mismatch(
                "prepared tensor add submit-only result mismatch",
            ));
        }
    }
    Ok(())
}

fn dispatch_prepared_tensor_add_sample(
    accelerator: &mut RuntimeMathAccelerator,
    reuse_mode: PreparedReuseMode,
    prepared: &RuntimePreparedTensorAddF32,
    len: usize,
    output: &mut [f32],
) -> Result<(), RuntimeMathAcceleratorError> {
    if reuse_mode.submit_only() && reuse_mode.uses_capacity() {
        accelerator.submit_prepared_tensor_add_f32_len_without_readback(prepared, len)
    } else if reuse_mode.submit_only() {
        accelerator.submit_prepared_tensor_add_f32_without_readback(prepared)
    } else if reuse_mode.uses_capacity() {
        accelerator.run_prepared_tensor_add_f32_len_into(prepared, len, output)
    } else {
        accelerator.run_prepared_tensor_add_f32_into(prepared, output)
    }
}

fn read_prepared_tensor_add_output(
    accelerator: &mut RuntimeMathAccelerator,
    reuse_mode: PreparedReuseMode,
    prepared: &RuntimePreparedTensorAddF32,
    len: usize,
    output: &mut [f32],
) -> Result<(), RuntimeMathAcceleratorError> {
    if reuse_mode.uses_capacity() {
        accelerator.read_prepared_tensor_add_f32_len_output_into(prepared, len, output)
    } else {
        accelerator.read_prepared_tensor_add_f32_output_into(prepared, output)
    }
}

fn run_prepared_tensor_add(
    accelerator: &mut RuntimeMathAccelerator,
    backend: RuntimeMathBackend,
    options: &BenchOptions,
    lhs: &DenseTensorF32,
    rhs: &DenseTensorF32,
    reference: &DenseTensorF32,
) -> BackendReport {
    if backend != RuntimeMathBackend::Wgpu {
        return BackendReport::skipped_or_failed(
            backend,
            "prepared GPU reuse is only available for the wgpu backend".to_owned(),
        );
    }
    let prepared = match if options.reuse_mode.uses_capacity() {
        accelerator.prepare_tensor_add_f32_capacity(options.tensor_capacity_len())
    } else {
        accelerator.prepare_tensor_add_f32(lhs, rhs)
    } {
        Ok(value) => value,
        Err(error) => return BackendReport::skipped_or_failed(backend, error.to_string()),
    };
    let update_cases = options
        .reuse_mode
        .updates_inputs()
        .then(|| tensor_update_cases(lhs.values().len(), options.warmup + options.iterations));
    if options.reuse_mode.uses_capacity()
        && update_cases.is_none()
        && let Err(error) = accelerator.update_prepared_tensor_add_f32(&prepared, lhs, rhs)
    {
        return BackendReport::skipped_or_failed(backend, error.to_string());
    }
    let mut output = vec![0.0; lhs.values().len()];
    for index in 0..options.warmup {
        if let Some(cases) = &update_cases {
            let (lhs, rhs, _) = &cases[index];
            if let Err(error) = accelerator.update_prepared_tensor_add_f32(&prepared, lhs, rhs) {
                return BackendReport::skipped_or_failed(backend, error.to_string());
            }
        }
        let result = dispatch_prepared_tensor_add_sample(
            accelerator,
            options.reuse_mode,
            &prepared,
            lhs.values().len(),
            &mut output,
        );
        if let Err(error) = result {
            return BackendReport::skipped_or_failed(backend, error.to_string());
        }
    }
    let mut samples = Vec::with_capacity(options.iterations);
    for index in 0..options.iterations {
        let case = update_cases
            .as_ref()
            .map(|cases| &cases[options.warmup + index]);
        let started = Instant::now();
        if let Some((lhs, rhs, _)) = case
            && let Err(error) = accelerator.update_prepared_tensor_add_f32(&prepared, lhs, rhs)
        {
            return BackendReport::skipped_or_failed(backend, error.to_string());
        }
        let result = dispatch_prepared_tensor_add_sample(
            accelerator,
            options.reuse_mode,
            &prepared,
            lhs.values().len(),
            &mut output,
        );
        if let Err(error) = result {
            return BackendReport::skipped_or_failed(backend, error.to_string());
        }
        let elapsed = started.elapsed().as_nanos();
        let expected = case.map_or_else(
            || reference.values(),
            |(_, _, reference)| reference.values(),
        );
        if !options.reuse_mode.submit_only() && !approx_eq(&output, expected, 1.0e-6) {
            return BackendReport::failed(
                backend,
                "prepared tensor add result mismatch".to_owned(),
            );
        }
        samples.push(elapsed);
    }
    if let Err(error) = validate_tensor_add_submit_only_output(
        accelerator,
        options.reuse_mode,
        &prepared,
        lhs.values().len(),
        &mut output,
        update_cases.as_deref(),
        reference,
    ) {
        return error.into_report(backend);
    }
    BackendReport::measured(backend, samples, accelerator.stats())
}

fn matrix_fixture(rows: usize, cols: usize, scale: f32) -> DenseMatrixF32 {
    DenseMatrixF32::new(
        rows,
        cols,
        (0..rows * cols)
            .map(|index| {
                let value = small_f32(index % 31) - 15.0;
                value * scale
            })
            .collect(),
    )
    .expect("fixture shape is valid")
}

fn matrix_fixture_f64(rows: usize, cols: usize, scale: f64) -> DenseMatrixF64 {
    DenseMatrixF64::new(
        rows,
        cols,
        (0..rows * cols)
            .map(|index| {
                let value = small_f64(index % 31) - 15.0;
                value * scale
            })
            .collect(),
    )
    .expect("f64 fixture shape is valid")
}

fn tensor_fixture(elements: usize, scale: f32) -> DenseTensorF32 {
    DenseTensorF32::new(
        vec![elements],
        (0..elements)
            .map(|index| (small_f32(index % 127) - 63.0) * scale)
            .collect(),
    )
    .expect("fixture shape is valid")
}

fn tensor_fixture_f64(elements: usize, scale: f64) -> DenseTensorF64 {
    DenseTensorF64::new(
        vec![elements],
        (0..elements)
            .map(|index| (small_f64(index % 127) - 63.0) * scale)
            .collect(),
    )
    .expect("f64 fixture shape is valid")
}

fn bias_fixture(cols: usize) -> DenseTensorF32 {
    DenseTensorF32::new(
        vec![cols],
        (0..cols)
            .map(|index| (small_f32(index % 17) - 8.0) * 0.125)
            .collect(),
    )
    .expect("bias fixture shape is valid")
}

fn apply_bias_to_reference(matrix: &mut DenseMatrixF32, bias: &DenseTensorF32) {
    for row in matrix.values_mut().chunks_exact_mut(bias.values().len()) {
        for (value, bias) in row.iter_mut().zip(bias.values().iter().copied()) {
            *value += bias;
        }
    }
}

fn matrix_update_cases(
    size: usize,
    count: usize,
    matmul: bool,
) -> Vec<(DenseMatrixF32, DenseMatrixF32, DenseMatrixF32)> {
    (0..count)
        .map(|index| {
            let lhs_scale = 1.0 + small_f32(index % 7) * 0.03125;
            let rhs_scale = 0.25 + small_f32((index + 3) % 11) * 0.015_625;
            let lhs = matrix_fixture(size, size, lhs_scale);
            let rhs = matrix_fixture(size, size, rhs_scale);
            let reference = if matmul {
                lhs.matmul_scalar(&rhs)
            } else {
                lhs.add_scalar(&rhs)
            }
            .expect("updated matrix fixture has compatible shape");
            (lhs, rhs, reference)
        })
        .collect()
}

fn matmul_bias_update_cases(
    size: usize,
    count: usize,
) -> Vec<(
    DenseMatrixF32,
    DenseMatrixF32,
    DenseTensorF32,
    DenseMatrixF32,
)> {
    (0..count)
        .map(|index| {
            let lhs = matrix_fixture(size, size, 1.0 + small_f32(index % 7) * 0.03125);
            let rhs = matrix_fixture(size, size, 0.25 + small_f32((index + 3) % 11) * 0.015_625);
            let bias = bias_fixture(size);
            let mut reference = lhs
                .matmul_scalar(&rhs)
                .expect("updated matmul-bias fixture has compatible shape");
            apply_bias_to_reference(&mut reference, &bias);
            (lhs, rhs, bias, reference)
        })
        .collect()
}

struct InferenceMatmulBiasCase {
    input: DenseTensorF32,
    reference: DenseMatrixF32,
}

fn inference_matmul_bias_graph(
    size: usize,
    rhs: &DenseMatrixF32,
    bias: &DenseTensorF32,
) -> Result<(InferenceGraph, InferenceTensorId), String> {
    let mut builder = InferenceGraph::builder();
    let input_id = builder.add_input(
        "x",
        InferenceShape::matrix(size, size).map_err(|error| error.to_string())?,
    );
    let weights = builder
        .add_constant("w", DenseTensorF32::from_matrix(rhs.clone()))
        .map_err(|error| error.to_string())?;
    let bias = builder
        .add_constant("b", bias.clone())
        .map_err(|error| error.to_string())?;
    let logits = builder
        .add_matmul(input_id, weights)
        .map_err(|error| error.to_string())?;
    let logits = builder
        .add_bias_add(logits, bias)
        .map_err(|error| error.to_string())?;
    builder
        .set_outputs([logits])
        .map_err(|error| error.to_string())?;
    builder
        .build()
        .map(|graph| (graph, input_id))
        .map_err(|error| error.to_string())
}

fn inference_matmul_bias_cases(
    size: usize,
    rhs: &DenseMatrixF32,
    bias: &DenseTensorF32,
    count: usize,
    update_inputs: bool,
) -> Vec<InferenceMatmulBiasCase> {
    (0..count)
        .map(|index| {
            let scale = if update_inputs {
                1.0 + small_f32(index % 7) * 0.03125
            } else {
                1.0
            };
            let input = matrix_fixture(size, size, scale);
            let mut reference = input
                .matmul_scalar(rhs)
                .expect("inference matmul-bias fixture has compatible shape");
            apply_bias_to_reference(&mut reference, bias);
            InferenceMatmulBiasCase {
                input: DenseTensorF32::from_matrix(input),
                reference,
            }
        })
        .collect()
}

fn tensor_update_cases(
    elements: usize,
    count: usize,
) -> Vec<(DenseTensorF32, DenseTensorF32, DenseTensorF32)> {
    (0..count)
        .map(|index| {
            let lhs = tensor_fixture(elements, 1.0 + small_f32(index % 7) * 0.03125);
            let rhs = tensor_fixture(elements, 0.25 + small_f32((index + 3) % 11) * 0.015_625);
            let reference = lhs
                .add_scalar(&rhs)
                .expect("updated tensor fixture has compatible shape");
            (lhs, rhs, reference)
        })
        .collect()
}

fn approx_eq(lhs: &[f32], rhs: &[f32], epsilon: f32) -> bool {
    lhs.len() == rhs.len()
        && lhs
            .iter()
            .zip(rhs.iter())
            .all(|(lhs, rhs)| (lhs - rhs).abs() <= epsilon)
}

fn approx_eq_f64(lhs: &[f64], rhs: &[f64], epsilon: f64) -> bool {
    lhs.len() == rhs.len()
        && lhs
            .iter()
            .zip(rhs.iter())
            .all(|(lhs, rhs)| (lhs - rhs).abs() <= epsilon)
}

#[derive(Clone, Copy)]
enum BenchOp {
    Matmul,
    MatmulBiasAdd,
    InferenceMatmulBiasAdd,
    MatrixAdd,
    TensorAdd,
    MatmulF64,
    MatrixAddF64,
    TensorAddF64,
}

impl BenchOp {
    const fn label(self) -> &'static str {
        match self {
            Self::Matmul => "matmul_f32",
            Self::MatmulBiasAdd => "matmul_bias_add_f32",
            Self::InferenceMatmulBiasAdd => "inference_matmul_bias_add_f32",
            Self::MatrixAdd => "matrix_add_f32",
            Self::TensorAdd => "tensor_add_f32",
            Self::MatmulF64 => "matmul_f64",
            Self::MatrixAddF64 => "matrix_add_f64",
            Self::TensorAddF64 => "tensor_add_f64",
        }
    }
}

enum BenchBackend {
    All,
    One(RuntimeMathBackend),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum PreparedReuseMode {
    #[default]
    None,
    Exact,
    UpdateInputs,
    Capacity,
    SubmitOnly,
}

impl PreparedReuseMode {
    const fn is_enabled(self) -> bool {
        !matches!(self, Self::None)
    }

    const fn updates_inputs(self) -> bool {
        matches!(self, Self::UpdateInputs)
    }

    const fn uses_capacity(self) -> bool {
        matches!(self, Self::Capacity)
    }

    const fn submit_only(self) -> bool {
        matches!(self, Self::SubmitOnly)
    }
}

struct BenchOptions {
    backend: BenchBackend,
    op: BenchOp,
    size: usize,
    iterations: usize,
    warmup: usize,
    wgpu_min_elements: usize,
    reuse_mode: PreparedReuseMode,
}

impl BenchOptions {
    fn parse(args: &[String]) -> Self {
        let mut options = Self {
            backend: BenchBackend::All,
            op: BenchOp::Matmul,
            size: 64,
            iterations: 10,
            warmup: 2,
            wgpu_min_elements: RuntimeMathAcceleratorConfig::default().wgpu_min_elements,
            reuse_mode: PreparedReuseMode::None,
        };
        let mut index = 0;
        while index < args.len() {
            match args[index].as_str() {
                "--backend" => {
                    index += 1;
                    options.backend = match args.get(index).map(String::as_str) {
                        Some("all") | None => BenchBackend::All,
                        Some(value) => BenchBackend::One(parse_backend(value)),
                    };
                }
                "--op" => {
                    index += 1;
                    options.op = match args.get(index).map(String::as_str) {
                        Some(
                            "matmul-bias"
                            | "matmul_bias"
                            | "matmul-bias-add"
                            | "matmul_bias_add"
                            | "matmul_bias_add_f32",
                        ) => BenchOp::MatmulBiasAdd,
                        Some("inference-matmul-bias-add" | "inference_matmul_bias_add_f32") => {
                            BenchOp::InferenceMatmulBiasAdd
                        }
                        Some("matrix-add" | "matrix_add" | "matrix_add_f32") => BenchOp::MatrixAdd,
                        Some("tensor-add" | "tensor_add" | "tensor_add_f32") => BenchOp::TensorAdd,
                        Some("matmul-f64" | "matmul_f64") => BenchOp::MatmulF64,
                        Some("matrix-add-f64" | "matrix_add_f64") => BenchOp::MatrixAddF64,
                        Some("tensor-add-f64" | "tensor_add_f64") => BenchOp::TensorAddF64,
                        _ => BenchOp::Matmul,
                    };
                }
                "--size" => {
                    index += 1;
                    options.size = parse_usize(args.get(index), options.size);
                }
                "--iterations" => {
                    index += 1;
                    options.iterations = parse_usize(args.get(index), options.iterations);
                }
                "--warmup" => {
                    index += 1;
                    options.warmup = parse_usize(args.get(index), options.warmup);
                }
                "--wgpu-min-elements" => {
                    index += 1;
                    options.wgpu_min_elements =
                        parse_usize(args.get(index), options.wgpu_min_elements);
                }
                "--reuse" => {
                    options.reuse_mode = PreparedReuseMode::Exact;
                }
                "--reuse-update-inputs" => {
                    options.reuse_mode = PreparedReuseMode::UpdateInputs;
                }
                "--reuse-capacity" => {
                    options.reuse_mode = PreparedReuseMode::Capacity;
                }
                "--submit-only" => {
                    options.reuse_mode = PreparedReuseMode::SubmitOnly;
                }
                _ => {}
            }
            index += 1;
        }
        options
    }

    fn matrix_capacity_size(&self) -> usize {
        self.size.saturating_mul(2).max(self.size)
    }

    fn tensor_capacity_len(&self) -> usize {
        self.size
            .saturating_mul(self.size)
            .saturating_mul(2)
            .max(self.size.saturating_mul(self.size))
    }
}

fn parse_backend(value: &str) -> RuntimeMathBackend {
    match value {
        "scalar" => RuntimeMathBackend::Scalar,
        "glam" => RuntimeMathBackend::Glam,
        "ndarray" => RuntimeMathBackend::Ndarray,
        "wgpu" => RuntimeMathBackend::Wgpu,
        _ => RuntimeMathBackend::Auto,
    }
}

fn parse_usize(value: Option<&String>, fallback: usize) -> usize {
    value
        .and_then(|value| value.parse().ok())
        .unwrap_or(fallback)
}

struct BackendReport {
    backend: RuntimeMathBackend,
    status: &'static str,
    median_ns: Option<u128>,
    min_ns: Option<u128>,
    max_ns: Option<u128>,
    stats: Option<RuntimeMathStats>,
    diagnostic: Option<String>,
}

#[derive(Clone, Copy, Serialize)]
#[serde(transparent)]
struct JsonBool(bool);

#[derive(Serialize)]
struct MathBenchReport {
    bench: &'static str,
    build_mode: &'static str,
    host_system: HostSystemReport,
    op: &'static str,
    size: usize,
    iterations: usize,
    warmup: usize,
    reuse: JsonBool,
    reuse_update_inputs: JsonBool,
    reuse_capacity: JsonBool,
    submit_only: JsonBool,
    capacity_size: Option<usize>,
    results: Vec<BackendReportJson>,
}

impl MathBenchReport {
    fn new(options: &BenchOptions, results: Vec<BackendReport>) -> Self {
        Self {
            bench: "runtime_math",
            build_mode: build_mode_label(),
            host_system: HostSystemReport::current(),
            op: options.op.label(),
            size: options.size,
            iterations: options.iterations,
            warmup: options.warmup,
            reuse: JsonBool(options.reuse_mode.is_enabled()),
            reuse_update_inputs: JsonBool(options.reuse_mode.updates_inputs()),
            reuse_capacity: JsonBool(options.reuse_mode.uses_capacity()),
            submit_only: JsonBool(options.reuse_mode.submit_only()),
            capacity_size: if options.reuse_mode.uses_capacity()
                && matches!(
                    options.op,
                    BenchOp::Matmul
                        | BenchOp::MatmulBiasAdd
                        | BenchOp::MatrixAdd
                        | BenchOp::TensorAdd
                ) {
                Some(match options.op {
                    BenchOp::TensorAdd => options.tensor_capacity_len(),
                    BenchOp::Matmul | BenchOp::MatmulBiasAdd | BenchOp::MatrixAdd => {
                        options.matrix_capacity_size()
                    }
                    BenchOp::InferenceMatmulBiasAdd
                    | BenchOp::MatmulF64
                    | BenchOp::MatrixAddF64
                    | BenchOp::TensorAddF64 => unreachable!("filtered above"),
                })
            } else {
                None
            },
            results: results.into_iter().map(BackendReport::into_json).collect(),
        }
    }
}

#[derive(Serialize)]
struct HostSystemReport {
    physical_cores: usize,
    logical_threads: usize,
    available_parallelism: usize,
}

impl HostSystemReport {
    fn current() -> Self {
        Self {
            physical_cores: num_cpus::get_physical().max(1),
            logical_threads: num_cpus::get().max(1),
            available_parallelism: std::thread::available_parallelism()
                .map_or(1, std::num::NonZero::get),
        }
    }
}

#[derive(Serialize)]
struct BackendReportJson {
    backend: &'static str,
    status: &'static str,
    median_ns: Option<u128>,
    min_ns: Option<u128>,
    max_ns: Option<u128>,
    stats: Option<RuntimeMathStatsJson>,
    diagnostic: Option<String>,
}

#[derive(Serialize)]
struct RuntimeMathStatsJson {
    scalar_calls: usize,
    glam_calls: usize,
    ndarray_calls: usize,
    wgpu_calls: usize,
    fused_matmul_bias_add_calls: usize,
    fallback_calls: usize,
    bytes_borrowed: usize,
    bytes_copied: usize,
    bytes_uploaded: usize,
    bytes_downloaded: usize,
    gpu_buffer_creations: usize,
    gpu_buffer_reuse_hits: usize,
    gpu_staging_buffer_creations: usize,
    gpu_staging_buffer_reuse_hits: usize,
    gpu_reused_dispatches: usize,
    last_backend: Option<&'static str>,
    last_auto_reason: Option<&'static str>,
}

impl From<RuntimeMathStats> for RuntimeMathStatsJson {
    fn from(stats: RuntimeMathStats) -> Self {
        Self {
            scalar_calls: stats.scalar_calls,
            glam_calls: stats.glam_calls,
            ndarray_calls: stats.ndarray_calls,
            wgpu_calls: stats.wgpu_calls,
            fused_matmul_bias_add_calls: stats.fused_matmul_bias_add_calls,
            fallback_calls: stats.fallback_calls,
            bytes_borrowed: stats.bytes_borrowed,
            bytes_copied: stats.bytes_copied,
            bytes_uploaded: stats.bytes_uploaded,
            bytes_downloaded: stats.bytes_downloaded,
            gpu_buffer_creations: stats.gpu_buffer_creations,
            gpu_buffer_reuse_hits: stats.gpu_buffer_reuse_hits,
            gpu_staging_buffer_creations: stats.gpu_staging_buffer_creations,
            gpu_staging_buffer_reuse_hits: stats.gpu_staging_buffer_reuse_hits,
            gpu_reused_dispatches: stats.gpu_reused_dispatches,
            last_backend: stats.last_backend.map(backend_label),
            last_auto_reason: stats.last_auto_reason.map(auto_reason_label),
        }
    }
}

impl BackendReport {
    fn measured(
        backend: RuntimeMathBackend,
        mut samples: Vec<u128>,
        stats: RuntimeMathStats,
    ) -> Self {
        samples.sort_unstable();
        Self {
            backend,
            status: "measured",
            median_ns: samples.get(samples.len() / 2).copied(),
            min_ns: samples.first().copied(),
            max_ns: samples.last().copied(),
            stats: Some(stats),
            diagnostic: None,
        }
    }

    fn skipped_or_failed(backend: RuntimeMathBackend, diagnostic: String) -> Self {
        let status = if backend == RuntimeMathBackend::Wgpu {
            "skipped"
        } else {
            "failed"
        };
        Self {
            backend,
            status,
            median_ns: None,
            min_ns: None,
            max_ns: None,
            stats: None,
            diagnostic: Some(diagnostic),
        }
    }

    fn failed(backend: RuntimeMathBackend, diagnostic: String) -> Self {
        Self {
            backend,
            status: "failed",
            median_ns: None,
            min_ns: None,
            max_ns: None,
            stats: None,
            diagnostic: Some(diagnostic),
        }
    }

    fn into_json(self) -> BackendReportJson {
        BackendReportJson {
            backend: backend_label(self.backend),
            status: self.status,
            median_ns: self.median_ns,
            min_ns: self.min_ns,
            max_ns: self.max_ns,
            stats: self.stats.map(Into::into),
            diagnostic: self.diagnostic,
        }
    }
}

fn add_math_stats(total: &mut RuntimeMathStats, sample: RuntimeMathStats) {
    total.scalar_calls += sample.scalar_calls;
    total.glam_calls += sample.glam_calls;
    total.ndarray_calls += sample.ndarray_calls;
    total.wgpu_calls += sample.wgpu_calls;
    total.fused_matmul_bias_add_calls += sample.fused_matmul_bias_add_calls;
    total.fallback_calls += sample.fallback_calls;
    total.bytes_borrowed += sample.bytes_borrowed;
    total.bytes_copied += sample.bytes_copied;
    total.bytes_uploaded += sample.bytes_uploaded;
    total.bytes_downloaded += sample.bytes_downloaded;
    total.gpu_buffer_creations += sample.gpu_buffer_creations;
    total.gpu_buffer_reuse_hits += sample.gpu_buffer_reuse_hits;
    total.gpu_staging_buffer_creations += sample.gpu_staging_buffer_creations;
    total.gpu_staging_buffer_reuse_hits += sample.gpu_staging_buffer_reuse_hits;
    total.gpu_reused_dispatches += sample.gpu_reused_dispatches;
    total.last_backend = sample.last_backend.or(total.last_backend);
    total.last_auto_reason = sample.last_auto_reason.or(total.last_auto_reason);
}

const fn backend_label(value: RuntimeMathBackend) -> &'static str {
    match value {
        RuntimeMathBackend::Scalar => "scalar",
        RuntimeMathBackend::Glam => "glam",
        RuntimeMathBackend::Ndarray => "ndarray",
        RuntimeMathBackend::Wgpu => "wgpu",
        RuntimeMathBackend::Auto => "auto",
    }
}

const fn build_mode_label() -> &'static str {
    if cfg!(debug_assertions) {
        "debug_assertions"
    } else {
        "optimized"
    }
}

const fn auto_reason_label(value: RuntimeMathAutoSelectionReason) -> &'static str {
    match value {
        RuntimeMathAutoSelectionReason::Matmul4x4Glam => "matmul_4x4_glam",
        RuntimeMathAutoSelectionReason::MatmulWgpuWorkThreshold => "matmul_wgpu_work_threshold",
        RuntimeMathAutoSelectionReason::MatmulScalarSmallWork => "matmul_scalar_small_work",
        RuntimeMathAutoSelectionReason::MatmulNdarrayCpuDefault => "matmul_ndarray_cpu_default",
        RuntimeMathAutoSelectionReason::ElementwiseWgpuWorkThreshold => {
            "elementwise_wgpu_work_threshold"
        }
        RuntimeMathAutoSelectionReason::ElementwiseNdarrayCpuDefault => {
            "elementwise_ndarray_cpu_default"
        }
    }
}

fn small_f32(value: usize) -> f32 {
    f32::from(u16::try_from(value).expect("fixture residue fits u16"))
}

fn small_f64(value: usize) -> f64 {
    f64::from(u32::try_from(value).expect("fixture residue fits u32"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_serializes_auto_reason_without_host_paths() {
        let options = BenchOptions {
            backend: BenchBackend::One(RuntimeMathBackend::Auto),
            op: BenchOp::Matmul,
            size: 4,
            iterations: 1,
            warmup: 0,
            wgpu_min_elements: RuntimeMathAcceleratorConfig::default().wgpu_min_elements,
            reuse_mode: PreparedReuseMode::None,
        };
        let report = MathBenchReport::new(
            &options,
            vec![BackendReport::measured(
                RuntimeMathBackend::Auto,
                vec![100],
                RuntimeMathStats {
                    glam_calls: 1,
                    last_backend: Some(RuntimeMathBackend::Glam),
                    last_auto_reason: Some(RuntimeMathAutoSelectionReason::Matmul4x4Glam),
                    ..RuntimeMathStats::default()
                },
            )],
        );

        let json = serde_json::to_string(&report).expect("report serializes");

        assert!(json.contains("\"build_mode\":"));
        assert!(json.contains("\"host_system\":"));
        assert!(json.contains("\"available_parallelism\":"));
        assert!(json.contains("\"last_auto_reason\":\"matmul_4x4_glam\""));
        assert!(json.contains("\"fused_matmul_bias_add_calls\":0"));
        let windows_drive_prefixes = ["C:", "D:"].map(|drive| format!("{drive}\\"));
        for prefix in windows_drive_prefixes {
            assert!(!json.contains(&prefix));
        }
        assert!(!json.contains(&["/", "home", "/"].concat()));
        assert!(!json.contains("/tmp/"));
    }

    #[test]
    fn parse_reuse_update_inputs_marks_reuse_and_report_field() {
        let args = [
            "--backend".to_owned(),
            "wgpu".to_owned(),
            "--op".to_owned(),
            "matmul".to_owned(),
            "--reuse-update-inputs".to_owned(),
        ];
        let options = BenchOptions::parse(&args);
        let report = MathBenchReport::new(
            &options,
            vec![BackendReport::skipped_or_failed(
                RuntimeMathBackend::Wgpu,
                "adapter unavailable".to_owned(),
            )],
        );

        let json = serde_json::to_string(&report).expect("report serializes");

        assert!(options.reuse_mode.is_enabled());
        assert!(options.reuse_mode.updates_inputs());
        assert!(json.contains("\"reuse\":true"));
        assert!(json.contains("\"reuse_update_inputs\":true"));
    }

    #[test]
    fn parse_submit_only_marks_reuse_and_report_field() {
        let args = [
            "--backend".to_owned(),
            "wgpu".to_owned(),
            "--op".to_owned(),
            "matmul-bias-add".to_owned(),
            "--submit-only".to_owned(),
        ];
        let options = BenchOptions::parse(&args);
        let report = MathBenchReport::new(
            &options,
            vec![BackendReport::measured(
                RuntimeMathBackend::Wgpu,
                vec![100],
                RuntimeMathStats {
                    wgpu_calls: 1,
                    gpu_reused_dispatches: 1,
                    last_backend: Some(RuntimeMathBackend::Wgpu),
                    ..RuntimeMathStats::default()
                },
            )],
        );

        let json = serde_json::to_string(&report).expect("report serializes");

        assert!(options.reuse_mode.is_enabled());
        assert!(options.reuse_mode.submit_only());
        assert!(json.contains("\"reuse\":true"));
        assert!(json.contains("\"submit_only\":true"));
        assert!(json.contains("\"gpu_reused_dispatches\":1"));
    }

    #[test]
    fn parse_reuse_capacity_marks_reuse_and_report_field() {
        let args = [
            "--backend".to_owned(),
            "wgpu".to_owned(),
            "--op".to_owned(),
            "tensor-add".to_owned(),
            "--size".to_owned(),
            "8".to_owned(),
            "--reuse-capacity".to_owned(),
        ];
        let options = BenchOptions::parse(&args);
        let report = MathBenchReport::new(
            &options,
            vec![BackendReport::skipped_or_failed(
                RuntimeMathBackend::Wgpu,
                "adapter unavailable".to_owned(),
            )],
        );

        let json = serde_json::to_string(&report).expect("report serializes");

        assert!(options.reuse_mode.is_enabled());
        assert!(options.reuse_mode.uses_capacity());
        assert_eq!(options.tensor_capacity_len(), 128);
        assert!(json.contains("\"reuse\":true"));
        assert!(json.contains("\"reuse_capacity\":true"));
        assert!(json.contains("\"capacity_size\":128"));
    }

    #[test]
    fn parse_matmul_bias_add_op_and_report_stats() {
        let args = [
            "--backend".to_owned(),
            "scalar".to_owned(),
            "--op".to_owned(),
            "matmul-bias-add".to_owned(),
        ];
        let options = BenchOptions::parse(&args);
        let report = MathBenchReport::new(
            &options,
            vec![BackendReport::measured(
                RuntimeMathBackend::Scalar,
                vec![100],
                RuntimeMathStats {
                    fused_matmul_bias_add_calls: 1,
                    scalar_calls: 1,
                    last_backend: Some(RuntimeMathBackend::Scalar),
                    ..RuntimeMathStats::default()
                },
            )],
        );

        let json = serde_json::to_string(&report).expect("report serializes");

        assert!(matches!(options.op, BenchOp::MatmulBiasAdd));
        assert!(json.contains("\"op\":\"matmul_bias_add_f32\""));
        assert!(json.contains("\"fused_matmul_bias_add_calls\":1"));
    }

    #[test]
    fn parse_f64_math_ops_and_report_auto_cpu_reason() {
        let args = [
            "--backend".to_owned(),
            "auto".to_owned(),
            "--op".to_owned(),
            "matrix-add-f64".to_owned(),
        ];
        let options = BenchOptions::parse(&args);
        let report = MathBenchReport::new(
            &options,
            vec![BackendReport::measured(
                RuntimeMathBackend::Auto,
                vec![100],
                RuntimeMathStats {
                    ndarray_calls: 1,
                    last_backend: Some(RuntimeMathBackend::Ndarray),
                    last_auto_reason: Some(
                        RuntimeMathAutoSelectionReason::ElementwiseNdarrayCpuDefault,
                    ),
                    ..RuntimeMathStats::default()
                },
            )],
        );

        let json = serde_json::to_string(&report).expect("report serializes");

        assert!(matches!(options.op, BenchOp::MatrixAddF64));
        assert!(json.contains("\"op\":\"matrix_add_f64\""));
        assert!(json.contains("\"last_auto_reason\":\"elementwise_ndarray_cpu_default\""));
        assert!(json.contains("\"capacity_size\":null"));
    }

    #[test]
    fn parse_inference_matmul_bias_add_op_reports_reused_session() {
        let args = [
            "--backend".to_owned(),
            "wgpu".to_owned(),
            "--op".to_owned(),
            "inference-matmul-bias-add".to_owned(),
            "--reuse".to_owned(),
        ];
        let options = BenchOptions::parse(&args);
        let report = MathBenchReport::new(
            &options,
            vec![BackendReport::measured(
                RuntimeMathBackend::Wgpu,
                vec![100],
                RuntimeMathStats {
                    wgpu_calls: 1,
                    fused_matmul_bias_add_calls: 1,
                    gpu_buffer_reuse_hits: 7,
                    gpu_reused_dispatches: 1,
                    last_backend: Some(RuntimeMathBackend::Wgpu),
                    ..RuntimeMathStats::default()
                },
            )],
        );

        let json = serde_json::to_string(&report).expect("report serializes");

        assert!(matches!(options.op, BenchOp::InferenceMatmulBiasAdd));
        assert!(options.reuse_mode.is_enabled());
        assert!(json.contains("\"op\":\"inference_matmul_bias_add_f32\""));
        assert!(json.contains("\"reuse\":true"));
        assert!(json.contains("\"gpu_reused_dispatches\":1"));
        assert!(!json.contains("\"capacity_size\":0"));
    }
}
