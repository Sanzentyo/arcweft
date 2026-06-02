//! Browser WebGPU math benchmark harness.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct BrowserMathBenchConfig {
    pub warmup_iters: usize,
    pub sample_iters: usize,
    pub seed: u32,
    pub add_lengths: Vec<usize>,
    pub matmul_shapes: Vec<MatmulShape>,
    pub modes: Vec<BrowserBenchMode>,
}

impl Default for BrowserMathBenchConfig {
    fn default() -> Self {
        Self {
            warmup_iters: 3,
            sample_iters: 10,
            seed: 0xace5_2026,
            add_lengths: vec![0, 1, 255, 256, 257, 4096, 65_536],
            matmul_shapes: vec![
                MatmulShape {
                    rows: 1,
                    shared: 1,
                    cols: 1,
                },
                MatmulShape {
                    rows: 2,
                    shared: 3,
                    cols: 4,
                },
                MatmulShape {
                    rows: 17,
                    shared: 19,
                    cols: 23,
                },
                MatmulShape {
                    rows: 64,
                    shared: 64,
                    cols: 64,
                },
                MatmulShape {
                    rows: 128,
                    shared: 128,
                    cols: 128,
                },
            ],
            modes: vec![
                BrowserBenchMode::CpuWasm,
                BrowserBenchMode::WebGpuOneShot,
                BrowserBenchMode::WebGpuPreparedUpload,
                BrowserBenchMode::WebGpuPreparedResident,
            ],
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct MatmulShape {
    pub rows: usize,
    pub shared: usize,
    pub cols: usize,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserBenchMode {
    CpuWasm,
    WebGpuOneShot,
    WebGpuPreparedUpload,
    WebGpuPreparedResident,
}

#[derive(Clone, Debug, Serialize)]
pub struct BrowserMathBenchReport {
    pub schema_version: &'static str,
    pub run: BrowserMathBenchRun,
    pub cases: Vec<BrowserMathBenchCase>,
    pub skips: Vec<BrowserMathBenchSkip>,
}

#[derive(Clone, Debug, Serialize)]
pub struct BrowserMathBenchRun {
    pub secure_context: bool,
    pub cross_origin_isolated: bool,
    pub webgpu: BrowserMathBenchWebGpu,
}

#[derive(Clone, Debug, Serialize)]
pub struct BrowserMathBenchWebGpu {
    pub available: bool,
    pub fallback_reason: Option<String>,
    pub limits: Option<BrowserMathBenchLimits>,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct BrowserMathBenchLimits {
    pub max_storage_buffer_binding_size: u64,
    pub max_buffer_size: u64,
    pub max_compute_invocations_per_workgroup: u32,
    pub max_compute_workgroups_per_dimension: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct BrowserMathBenchCase {
    pub case_id: String,
    pub op: &'static str,
    pub shape: BrowserMathBenchShape,
    pub mode: BrowserBenchMode,
    pub warmup_iters: usize,
    pub sample_iters: usize,
    pub median_ms: Option<f64>,
    pub mad_ms: Option<f64>,
    pub min_ms: Option<f64>,
    pub p95_ms: Option<f64>,
    pub bytes_uploaded: usize,
    pub bytes_readback: usize,
    pub dispatches: usize,
    pub buffer_alloc_count: usize,
    pub buffer_reuse_count: usize,
    pub correctness: BrowserMathBenchCorrectness,
    pub fallback_reason: Option<String>,
    pub checksum: f64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserMathBenchShape {
    Len {
        len: usize,
    },
    Matmul {
        rows: usize,
        shared: usize,
        cols: usize,
    },
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct BrowserMathBenchCorrectness {
    pub passed: bool,
    pub max_abs: f32,
    pub max_rel: f32,
}

#[derive(Clone, Debug, Serialize)]
pub struct BrowserMathBenchSkip {
    pub scope: &'static str,
    pub reason: String,
}

#[cfg(target_arch = "wasm32")]
mod wasm {
    use super::{
        BrowserBenchMode, BrowserMathBenchCase, BrowserMathBenchConfig,
        BrowserMathBenchCorrectness, BrowserMathBenchLimits, BrowserMathBenchReport,
        BrowserMathBenchRun, BrowserMathBenchShape, BrowserMathBenchSkip, BrowserMathBenchWebGpu,
        MatmulShape,
    };
    use arcweft_core::math::{DenseMatrixF32, DenseTensorF32};
    use arcweft_runtime_accelerator::math::browser_webgpu::{
        BrowserMatmulCapacity, BrowserWebGpuError, BrowserWebGpuMathContext, BrowserWebGpuMathStats,
    };
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    pub async fn run_arcweft_browser_math_bench(config_json: String) -> Result<JsValue, JsValue> {
        let config = parse_config(&config_json).map_err(|error| JsValue::from_str(&error))?;
        let report = run(config).await;
        let json = serde_json::to_string(&report)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        js_sys::JSON::parse(&json)
    }

    async fn run(config: BrowserMathBenchConfig) -> BrowserMathBenchReport {
        let availability = BrowserWebGpuMathContext::availability();
        let mut skips = Vec::new();
        let mut context = match BrowserWebGpuMathContext::new().await {
            Ok(context) => Some(context),
            Err(error) => {
                skips.push(BrowserMathBenchSkip {
                    scope: "webgpu",
                    reason: fallback_reason(&error),
                });
                None
            }
        };
        let limits = context.as_ref().map(|context| {
            let limits = context.limits();
            BrowserMathBenchLimits {
                max_storage_buffer_binding_size: limits.max_storage_buffer_binding_size,
                max_buffer_size: limits.max_buffer_size,
                max_compute_invocations_per_workgroup: limits.max_compute_invocations_per_workgroup,
                max_compute_workgroups_per_dimension: limits.max_compute_workgroups_per_dimension,
            }
        });
        let mut cases = Vec::new();
        for len in &config.add_lengths {
            for mode in &config.modes {
                cases.push(run_add_case(&config, context.as_mut(), *mode, *len).await);
            }
        }
        for shape in &config.matmul_shapes {
            for mode in &config.modes {
                cases.push(run_matmul_case(&config, context.as_mut(), *mode, *shape).await);
            }
        }
        BrowserMathBenchReport {
            schema_version: "arcweft.browser_webgpu_bench.v1",
            run: BrowserMathBenchRun {
                secure_context: availability.secure_context,
                cross_origin_isolated: availability.cross_origin_isolated,
                webgpu: BrowserMathBenchWebGpu {
                    available: context.is_some(),
                    fallback_reason: skips.first().map(|skip| skip.reason.clone()),
                    limits,
                },
            },
            cases,
            skips,
        }
    }

    async fn run_add_case(
        config: &BrowserMathBenchConfig,
        context: Option<&mut BrowserWebGpuMathContext>,
        mode: BrowserBenchMode,
        len: usize,
    ) -> BrowserMathBenchCase {
        let lhs = deterministic_values(len, config.seed ^ len as u32);
        let rhs = deterministic_values(len, config.seed.rotate_left(7) ^ len as u32);
        let expected = add_cpu(&lhs, &rhs);
        let shape = BrowserMathBenchShape::Len { len };
        let mut case = empty_case(
            format!("tensor_add_f32_len{len}_{mode:?}"),
            "tensor_add_f32",
            shape,
            mode,
            config,
        );
        match mode {
            BrowserBenchMode::CpuWasm => {
                let mut out = Vec::new();
                case = measure_case(case, config, || {
                    out = add_cpu(&lhs, &rhs);
                });
                case.correctness = compare(&expected, &out, 1.0e-6, 1.0e-6);
                case.checksum = checksum(&out);
            }
            BrowserBenchMode::WebGpuOneShot => {
                let Some(context) = context else {
                    return skipped_case(case, "webgpu_unavailable");
                };
                let lhs_tensor =
                    DenseTensorF32::new(vec![len], lhs.clone()).expect("valid lhs tensor");
                let rhs_tensor =
                    DenseTensorF32::new(vec![len], rhs.clone()).expect("valid rhs tensor");
                context.reset_stats();
                let mut out = Vec::new();
                let mut samples = Vec::with_capacity(config.sample_iters);
                let mut error = None;
                for _ in 0..config.warmup_iters {
                    match context.tensor_add_f32(&lhs_tensor, &rhs_tensor).await {
                        Ok(tensor) => out = tensor.values().to_vec(),
                        Err(current) => {
                            error = Some(current);
                            break;
                        }
                    }
                }
                if error.is_none() {
                    for _ in 0..config.sample_iters {
                        let start = now_ms();
                        match context.tensor_add_f32(&lhs_tensor, &rhs_tensor).await {
                            Ok(tensor) => out = tensor.values().to_vec(),
                            Err(current) => {
                                error = Some(current);
                                break;
                            }
                        }
                        samples.push(now_ms() - start);
                    }
                }
                case = finish_gpu_case(case, error, samples, context.stats(), &expected, &out);
            }
            BrowserBenchMode::WebGpuPreparedUpload => {
                let Some(context) = context else {
                    return skipped_case(case, "webgpu_unavailable");
                };
                context.reset_stats();
                let prepared = match context.prepare_elementwise_f32(len) {
                    Ok(prepared) => prepared,
                    Err(error) => return skipped_case(case, &fallback_reason(&error)),
                };
                let mut out = vec![0.0; len];
                let mut samples = Vec::with_capacity(config.sample_iters);
                let mut error = None;
                for _ in 0..config.warmup_iters {
                    if let Err(current) = context
                        .dispatch_prepared_elementwise_f32(&prepared, &lhs, &rhs, &mut out)
                        .await
                    {
                        error = Some(current);
                        break;
                    }
                }
                if error.is_none() {
                    for _ in 0..config.sample_iters {
                        let start = now_ms();
                        if let Err(current) = context
                            .dispatch_prepared_elementwise_f32(&prepared, &lhs, &rhs, &mut out)
                            .await
                        {
                            error = Some(current);
                            break;
                        }
                        samples.push(now_ms() - start);
                    }
                }
                case = finish_gpu_case(case, error, samples, context.stats(), &expected, &out);
            }
            BrowserBenchMode::WebGpuPreparedResident => {
                let Some(context) = context else {
                    return skipped_case(case, "webgpu_unavailable");
                };
                context.reset_stats();
                let prepared = match context.prepare_elementwise_f32(len) {
                    Ok(prepared) => prepared,
                    Err(error) => return skipped_case(case, &fallback_reason(&error)),
                };
                if let Err(error) = context.upload_prepared_elementwise_f32(&prepared, &lhs, &rhs) {
                    return skipped_case(case, &fallback_reason(&error));
                }
                let mut out = vec![0.0; len];
                let mut samples = Vec::with_capacity(config.sample_iters);
                let mut error = None;
                for _ in 0..config.warmup_iters {
                    if let Err(current) = context
                        .dispatch_resident_elementwise_f32(&prepared, &mut out)
                        .await
                    {
                        error = Some(current);
                        break;
                    }
                }
                if error.is_none() {
                    for _ in 0..config.sample_iters {
                        let start = now_ms();
                        if let Err(current) = context
                            .dispatch_resident_elementwise_f32(&prepared, &mut out)
                            .await
                        {
                            error = Some(current);
                            break;
                        }
                        samples.push(now_ms() - start);
                    }
                }
                case = finish_gpu_case(case, error, samples, context.stats(), &expected, &out);
            }
        }
        case
    }

    async fn run_matmul_case(
        config: &BrowserMathBenchConfig,
        context: Option<&mut BrowserWebGpuMathContext>,
        mode: BrowserBenchMode,
        shape: MatmulShape,
    ) -> BrowserMathBenchCase {
        let lhs_len = shape.rows * shape.shared;
        let rhs_len = shape.shared * shape.cols;
        let lhs = deterministic_values(lhs_len, config.seed ^ lhs_len as u32);
        let rhs = deterministic_values(rhs_len, config.seed.rotate_left(11) ^ rhs_len as u32);
        let expected = matmul_cpu(&lhs, &rhs, shape);
        let mut case = empty_case(
            format!(
                "matmul_f32_m{}_k{}_n{}_{mode:?}",
                shape.rows, shape.shared, shape.cols
            ),
            "matmul_f32",
            BrowserMathBenchShape::Matmul {
                rows: shape.rows,
                shared: shape.shared,
                cols: shape.cols,
            },
            mode,
            config,
        );
        match mode {
            BrowserBenchMode::CpuWasm => {
                let mut out = Vec::new();
                case = measure_case(case, config, || {
                    out = matmul_cpu(&lhs, &rhs, shape);
                });
                case.correctness = compare(&expected, &out, matmul_abs_tol(shape.shared), 1.0e-4);
                case.checksum = checksum(&out);
            }
            BrowserBenchMode::WebGpuOneShot => {
                let Some(context) = context else {
                    return skipped_case(case, "webgpu_unavailable");
                };
                let lhs_matrix = DenseMatrixF32::new(shape.rows, shape.shared, lhs.clone())
                    .expect("valid lhs matrix");
                let rhs_matrix = DenseMatrixF32::new(shape.shared, shape.cols, rhs.clone())
                    .expect("valid rhs matrix");
                context.reset_stats();
                let mut out = Vec::new();
                let mut samples = Vec::with_capacity(config.sample_iters);
                let mut error = None;
                for _ in 0..config.warmup_iters {
                    match context.matmul_f32(&lhs_matrix, &rhs_matrix).await {
                        Ok(matrix) => out = matrix.values().to_vec(),
                        Err(current) => {
                            error = Some(current);
                            break;
                        }
                    }
                }
                if error.is_none() {
                    for _ in 0..config.sample_iters {
                        let start = now_ms();
                        match context.matmul_f32(&lhs_matrix, &rhs_matrix).await {
                            Ok(matrix) => out = matrix.values().to_vec(),
                            Err(current) => {
                                error = Some(current);
                                break;
                            }
                        }
                        samples.push(now_ms() - start);
                    }
                }
                case = finish_gpu_case(case, error, samples, context.stats(), &expected, &out);
            }
            BrowserBenchMode::WebGpuPreparedUpload => {
                let Some(context) = context else {
                    return skipped_case(case, "webgpu_unavailable");
                };
                context.reset_stats();
                let prepared = match context.prepare_matmul_f32(BrowserMatmulCapacity {
                    rows: shape.rows,
                    shared: shape.shared,
                    cols: shape.cols,
                }) {
                    Ok(prepared) => prepared,
                    Err(error) => return skipped_case(case, &fallback_reason(&error)),
                };
                let mut out = vec![0.0; shape.rows * shape.cols];
                let mut samples = Vec::with_capacity(config.sample_iters);
                let mut error = None;
                for _ in 0..config.warmup_iters {
                    if let Err(current) = context
                        .dispatch_prepared_matmul_f32(
                            &prepared,
                            &lhs,
                            &rhs,
                            &mut out,
                            shape.rows,
                            shape.shared,
                            shape.cols,
                        )
                        .await
                    {
                        error = Some(current);
                        break;
                    }
                }
                if error.is_none() {
                    for _ in 0..config.sample_iters {
                        let start = now_ms();
                        if let Err(current) = context
                            .dispatch_prepared_matmul_f32(
                                &prepared,
                                &lhs,
                                &rhs,
                                &mut out,
                                shape.rows,
                                shape.shared,
                                shape.cols,
                            )
                            .await
                        {
                            error = Some(current);
                            break;
                        }
                        samples.push(now_ms() - start);
                    }
                }
                case = finish_gpu_case(case, error, samples, context.stats(), &expected, &out);
            }
            BrowserBenchMode::WebGpuPreparedResident => {
                let Some(context) = context else {
                    return skipped_case(case, "webgpu_unavailable");
                };
                context.reset_stats();
                let prepared = match context.prepare_matmul_f32(BrowserMatmulCapacity {
                    rows: shape.rows,
                    shared: shape.shared,
                    cols: shape.cols,
                }) {
                    Ok(prepared) => prepared,
                    Err(error) => return skipped_case(case, &fallback_reason(&error)),
                };
                if let Err(error) = context.upload_prepared_matmul_f32(
                    &prepared,
                    &lhs,
                    &rhs,
                    shape.rows,
                    shape.shared,
                    shape.cols,
                ) {
                    return skipped_case(case, &fallback_reason(&error));
                }
                let mut out = vec![0.0; shape.rows * shape.cols];
                let mut samples = Vec::with_capacity(config.sample_iters);
                let mut error = None;
                for _ in 0..config.warmup_iters {
                    if let Err(current) = context
                        .dispatch_resident_matmul_f32(&prepared, &mut out, shape.rows, shape.cols)
                        .await
                    {
                        error = Some(current);
                        break;
                    }
                }
                if error.is_none() {
                    for _ in 0..config.sample_iters {
                        let start = now_ms();
                        if let Err(current) = context
                            .dispatch_resident_matmul_f32(
                                &prepared, &mut out, shape.rows, shape.cols,
                            )
                            .await
                        {
                            error = Some(current);
                            break;
                        }
                        samples.push(now_ms() - start);
                    }
                }
                case = finish_gpu_case(case, error, samples, context.stats(), &expected, &out);
            }
        }
        case
    }

    fn parse_config(config_json: &str) -> Result<BrowserMathBenchConfig, String> {
        if config_json.trim().is_empty() {
            Ok(BrowserMathBenchConfig::default())
        } else {
            serde_json::from_str(config_json).map_err(|error| error.to_string())
        }
    }

    fn empty_case(
        case_id: String,
        op: &'static str,
        shape: BrowserMathBenchShape,
        mode: BrowserBenchMode,
        config: &BrowserMathBenchConfig,
    ) -> BrowserMathBenchCase {
        BrowserMathBenchCase {
            case_id,
            op,
            shape,
            mode,
            warmup_iters: config.warmup_iters,
            sample_iters: config.sample_iters,
            median_ms: None,
            mad_ms: None,
            min_ms: None,
            p95_ms: None,
            bytes_uploaded: 0,
            bytes_readback: 0,
            dispatches: 0,
            buffer_alloc_count: 0,
            buffer_reuse_count: 0,
            correctness: BrowserMathBenchCorrectness {
                passed: false,
                max_abs: 0.0,
                max_rel: 0.0,
            },
            fallback_reason: None,
            checksum: 0.0,
        }
    }

    fn measure_case(
        mut case: BrowserMathBenchCase,
        config: &BrowserMathBenchConfig,
        mut run: impl FnMut(),
    ) -> BrowserMathBenchCase {
        for _ in 0..config.warmup_iters {
            run();
        }
        let mut samples = Vec::with_capacity(config.sample_iters);
        for _ in 0..config.sample_iters {
            let start = now_ms();
            run();
            samples.push(now_ms() - start);
        }
        fill_timing(&mut case, samples);
        case
    }

    fn finish_gpu_case(
        mut case: BrowserMathBenchCase,
        error: Option<BrowserWebGpuError>,
        samples: Vec<f64>,
        stats: BrowserWebGpuMathStats,
        expected: &[f32],
        out: &[f32],
    ) -> BrowserMathBenchCase {
        if let Some(error) = error {
            return skipped_case(case, &fallback_reason(&error));
        }
        fill_timing(&mut case, samples);
        let tolerance = if case.op == "matmul_f32" {
            (1.0e-4, 1.0e-4)
        } else {
            (1.0e-6, 1.0e-6)
        };
        case.correctness = compare(expected, out, tolerance.0, tolerance.1);
        case.checksum = checksum(out);
        case.dispatches = stats.dispatches;
        case.bytes_uploaded = stats.bytes_uploaded;
        case.bytes_readback = stats.bytes_downloaded;
        case.buffer_alloc_count = stats.buffer_creations + stats.readback_buffer_creations;
        case.buffer_reuse_count = stats.buffer_reuse_hits + stats.readback_buffer_reuse_hits;
        case
    }

    fn skipped_case(mut case: BrowserMathBenchCase, reason: &str) -> BrowserMathBenchCase {
        case.fallback_reason = Some(reason.to_owned());
        case
    }

    fn fill_timing(case: &mut BrowserMathBenchCase, mut samples: Vec<f64>) {
        if samples.is_empty() {
            return;
        }
        samples.sort_by(f64::total_cmp);
        let median = samples[samples.len() / 2];
        let min = samples[0];
        let p95 = samples[(samples.len() * 95 / 100).min(samples.len() - 1)];
        let mut deviations = samples
            .iter()
            .map(|sample| (sample - median).abs())
            .collect::<Vec<_>>();
        deviations.sort_by(f64::total_cmp);
        case.median_ms = Some(median);
        case.mad_ms = Some(deviations[deviations.len() / 2]);
        case.min_ms = Some(min);
        case.p95_ms = Some(p95);
    }

    fn deterministic_values(len: usize, seed: u32) -> Vec<f32> {
        let mut state = seed | 1;
        (0..len)
            .map(|_| {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let value = ((state >> 8) & 0xffff) as f32 / 65_535.0;
                value * 2.0 - 1.0
            })
            .collect()
    }

    fn add_cpu(lhs: &[f32], rhs: &[f32]) -> Vec<f32> {
        lhs.iter().zip(rhs).map(|(lhs, rhs)| lhs + rhs).collect()
    }

    fn matmul_cpu(lhs: &[f32], rhs: &[f32], shape: MatmulShape) -> Vec<f32> {
        let lhs = DenseMatrixF32::new(shape.rows, shape.shared, lhs.to_vec()).expect("valid lhs");
        let rhs = DenseMatrixF32::new(shape.shared, shape.cols, rhs.to_vec()).expect("valid rhs");
        lhs.matmul_scalar(&rhs)
            .expect("valid matmul")
            .values()
            .to_vec()
    }

    fn compare(
        expected: &[f32],
        actual: &[f32],
        abs_tol: f32,
        rel_tol: f32,
    ) -> BrowserMathBenchCorrectness {
        let mut max_abs = 0.0_f32;
        let mut max_rel = 0.0_f32;
        for (expected, actual) in expected.iter().zip(actual) {
            let abs = (expected - actual).abs();
            let rel = abs / expected.abs().max(1.0);
            max_abs = max_abs.max(abs);
            max_rel = max_rel.max(rel);
        }
        BrowserMathBenchCorrectness {
            passed: expected.len() == actual.len() && max_abs <= abs_tol && max_rel <= rel_tol,
            max_abs,
            max_rel,
        }
    }

    fn checksum(values: &[f32]) -> f64 {
        values
            .iter()
            .enumerate()
            .map(|(index, value)| f64::from(*value) * ((index + 1) as f64))
            .sum()
    }

    fn matmul_abs_tol(shared: usize) -> f32 {
        (4.0 * f32::EPSILON * shared as f32).max(1.0e-4)
    }

    fn fallback_reason(error: &BrowserWebGpuError) -> String {
        error
            .reason()
            .map(|reason| format!("{reason:?}"))
            .unwrap_or_else(|| "Math".to_owned())
    }

    fn now_ms() -> f64 {
        web_sys::window()
            .and_then(|window| window.performance())
            .map(|performance| performance.now())
            .unwrap_or_else(js_sys::Date::now)
    }
}

#[cfg(target_arch = "wasm32")]
pub use wasm::run_arcweft_browser_math_bench;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_report_schema_serializes_without_paths() {
        let report = BrowserMathBenchReport {
            schema_version: "arcweft.browser_webgpu_bench.v1",
            run: BrowserMathBenchRun {
                secure_context: true,
                cross_origin_isolated: false,
                webgpu: BrowserMathBenchWebGpu {
                    available: false,
                    fallback_reason: Some("navigator_gpu_missing".to_owned()),
                    limits: None,
                },
            },
            cases: vec![BrowserMathBenchCase {
                case_id: "tensor_add_f32_len256_cpu".to_owned(),
                op: "tensor_add_f32",
                shape: BrowserMathBenchShape::Len { len: 256 },
                mode: BrowserBenchMode::CpuWasm,
                warmup_iters: 1,
                sample_iters: 1,
                median_ms: Some(0.0),
                mad_ms: Some(0.0),
                min_ms: Some(0.0),
                p95_ms: Some(0.0),
                bytes_uploaded: 0,
                bytes_readback: 0,
                dispatches: 0,
                buffer_alloc_count: 0,
                buffer_reuse_count: 0,
                correctness: BrowserMathBenchCorrectness {
                    passed: true,
                    max_abs: 0.0,
                    max_rel: 0.0,
                },
                fallback_reason: None,
                checksum: 0.0,
            }],
            skips: vec![BrowserMathBenchSkip {
                scope: "webgpu",
                reason: "navigator_gpu_missing".to_owned(),
            }],
        };

        let json = serde_json::to_string(&report).expect("report serializes");

        assert!(json.contains("arcweft.browser_webgpu_bench.v1"));
        assert!(!json.contains("\\\\"));
        assert!(!json.contains("D:"));
    }
}
