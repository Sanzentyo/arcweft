//! Browser WebGPU math benchmark harness.

use arcweft_runtime_accelerator::math::browser_webgpu_policy::{
    BrowserMatmulCapacity, BrowserWebGpuLimits, BrowserWebGpuMathAutoPolicy,
    BrowserWebGpuMathAutoReason, BrowserWebGpuMathMode,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct BrowserMathBenchConfig {
    pub warmup_iters: usize,
    pub sample_iters: usize,
    pub repeat_rounds: usize,
    pub seed: u32,
    pub add_lengths: Vec<usize>,
    pub matmul_shapes: Vec<MatmulShape>,
    pub modes: Vec<BrowserBenchMode>,
    pub mode_order: BrowserBenchModeOrder,
    pub async_batch_depth: usize,
}

impl Default for BrowserMathBenchConfig {
    fn default() -> Self {
        Self {
            warmup_iters: 3,
            sample_iters: 10,
            repeat_rounds: 1,
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
                BrowserBenchMode::Auto,
                BrowserBenchMode::AutoPipelined,
                BrowserBenchMode::AutoResidentPipelined,
                BrowserBenchMode::AutoResidentDirectPipelined,
                BrowserBenchMode::CpuWasm,
                BrowserBenchMode::WebGpuOneShot,
                BrowserBenchMode::WebGpuPreparedUpload,
                BrowserBenchMode::WebGpuPreparedResident,
                BrowserBenchMode::WebGpuPreparedCapacityResident,
                BrowserBenchMode::WebGpuPreparedResidentAsync,
                BrowserBenchMode::WebGpuPreparedResidentPipelined,
                BrowserBenchMode::WebGpuPreparedCapacityResidentPipelined,
                BrowserBenchMode::WebGpuPreparedResidentSubmitOnlyPipelined,
                BrowserBenchMode::WebGpuPreparedCapacityResidentSubmitOnlyPipelined,
                BrowserBenchMode::WebGpuPreparedResidentDispatchOnlyPipelined,
                BrowserBenchMode::WebGpuPreparedCapacityResidentDispatchOnlyPipelined,
                BrowserBenchMode::WebGpuPreparedResidentChainedDispatchOnlyPipelined,
                BrowserBenchMode::WebGpuPreparedResidentMatmulBiasDispatchOnlyPipelined,
            ],
            mode_order: BrowserBenchModeOrder::AsListed,
            async_batch_depth: 4,
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
    Auto,
    AutoPipelined,
    AutoResidentPipelined,
    AutoResidentDirectPipelined,
    CpuWasm,
    WebGpuOneShot,
    WebGpuPreparedUpload,
    WebGpuPreparedResident,
    WebGpuPreparedCapacityResident,
    WebGpuPreparedResidentAsync,
    WebGpuPreparedResidentPipelined,
    WebGpuPreparedCapacityResidentPipelined,
    WebGpuPreparedResidentSubmitOnlyPipelined,
    WebGpuPreparedCapacityResidentSubmitOnlyPipelined,
    WebGpuPreparedResidentDispatchOnlyPipelined,
    WebGpuPreparedCapacityResidentDispatchOnlyPipelined,
    WebGpuPreparedResidentChainedDispatchOnlyPipelined,
    WebGpuPreparedResidentMatmulBiasDispatchOnlyPipelined,
}

#[derive(Clone, Copy, Debug, Deserialize, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserBenchModeOrder {
    #[default]
    AsListed,
    RotateByRound,
}

#[derive(Clone, Debug, Serialize)]
pub struct BrowserMathBenchReport {
    pub schema_version: &'static str,
    pub run: BrowserMathBenchRun,
    pub cases: Vec<BrowserMathBenchCase>,
    pub stability: Vec<BrowserMathBenchStability>,
    pub recommendations: Vec<BrowserMathBenchRecommendation>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capacity: Option<BrowserMathBenchCapacity>,
    pub mode: BrowserBenchMode,
    pub round_index: usize,
    pub mode_order_index: usize,
    pub warmup_iters: usize,
    pub sample_iters: usize,
    pub median_ms: Option<f64>,
    pub mad_ms: Option<f64>,
    pub min_ms: Option<f64>,
    pub p95_ms: Option<f64>,
    pub effective_gflops: Option<f64>,
    pub submit_median_ms: Option<f64>,
    pub readback_median_ms: Option<f64>,
    pub submit_median_share: Option<f64>,
    pub readback_median_share: Option<f64>,
    pub bytes_uploaded: usize,
    pub bytes_readback: usize,
    pub dispatches: usize,
    pub async_submissions: usize,
    pub async_readbacks: usize,
    pub max_in_flight: usize,
    pub buffer_alloc_count: usize,
    pub buffer_reuse_count: usize,
    pub workgroups: usize,
    pub work_items: usize,
    pub estimated_flops: u64,
    pub correctness: BrowserMathBenchCorrectness,
    pub fallback_reason: Option<String>,
    pub checksum: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct BrowserMathBenchStability {
    pub op: &'static str,
    pub shape: BrowserMathBenchShape,
    pub mode: BrowserBenchMode,
    pub measured_rounds: usize,
    pub median_of_medians_ms: Option<f64>,
    pub min_median_ms: Option<f64>,
    pub max_median_ms: Option<f64>,
    pub median_mad_ms: Option<f64>,
    pub spread_ratio: Option<f64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserMathBenchCapacity {
    Len {
        len: usize,
    },
    Matmul {
        rows: usize,
        shared: usize,
        cols: usize,
    },
}

impl From<BrowserMatmulCapacity> for BrowserMathBenchCapacity {
    fn from(capacity: BrowserMatmulCapacity) -> Self {
        Self::Matmul {
            rows: capacity.rows,
            shared: capacity.shared,
            cols: capacity.cols,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct BrowserMathBenchRecommendation {
    pub op: &'static str,
    pub shape: BrowserMathBenchShape,
    pub selected_mode: Option<BrowserBenchMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_capacity: Option<BrowserMathBenchCapacity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_mode: Option<BrowserBenchMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_capacity: Option<BrowserMathBenchCapacity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_reason: Option<BrowserMathBenchPolicyReason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_matches_selected: Option<bool>,
    pub selected_median_ms: Option<f64>,
    pub selected_mad_ms: Option<f64>,
    pub selected_p95_ms: Option<f64>,
    pub cpu_median_ms: Option<f64>,
    pub cpu_mad_ms: Option<f64>,
    pub cpu_p95_ms: Option<f64>,
    pub speedup: Option<f64>,
    pub reason: BrowserMathBenchRecommendationReason,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserMathBenchRecommendationReason {
    WebGpuFaster,
    CpuFasterOrEqual,
    MissingCpuBaseline,
    NoMeasuredWebGpuCase,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserMathBenchPolicyReason {
    MatmulPreparedResidentPipelined,
    MatmulPreparedCapacityResidentPipelined,
    MatmulCpuDefault,
    ElementwisePreparedResidentPipelined,
    ElementwiseCpuReadbackDominated,
    StorageLimit,
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

pub fn browser_math_bench_recommendations(
    cases: &[BrowserMathBenchCase],
    limits: Option<BrowserMathBenchLimits>,
) -> Vec<BrowserMathBenchRecommendation> {
    let mut groups = Vec::<(&'static str, BrowserMathBenchShape)>::new();
    for case in cases {
        if !groups
            .iter()
            .any(|(op, shape)| *op == case.op && *shape == case.shape)
        {
            groups.push((case.op, case.shape));
        }
    }
    groups
        .into_iter()
        .map(|(op, shape)| recommend_browser_math_case(op, shape, cases, limits))
        .collect()
}

pub fn browser_math_bench_stability(
    cases: &[BrowserMathBenchCase],
) -> Vec<BrowserMathBenchStability> {
    let mut groups = Vec::<(&'static str, BrowserMathBenchShape, BrowserBenchMode)>::new();
    for case in cases {
        if case.median_ms.is_none() || !case.correctness.passed {
            continue;
        }
        let key = (case.op, case.shape, case.mode);
        if !groups.contains(&key) {
            groups.push(key);
        }
    }
    groups
        .into_iter()
        .map(|(op, shape, mode)| {
            let samples = cases
                .iter()
                .filter(|case| {
                    case.op == op
                        && case.shape == shape
                        && case.mode == mode
                        && case.correctness.passed
                })
                .filter_map(|case| case.median_ms)
                .collect::<Vec<_>>();
            let median = median_sample(samples.clone());
            let min = samples.iter().copied().reduce(f64::min);
            let max = samples.iter().copied().reduce(f64::max);
            BrowserMathBenchStability {
                op,
                shape,
                mode,
                measured_rounds: samples.len(),
                median_of_medians_ms: median,
                min_median_ms: min,
                max_median_ms: max,
                median_mad_ms: median.and_then(|center| {
                    median_sample(
                        samples
                            .iter()
                            .map(|sample| (sample - center).abs())
                            .collect(),
                    )
                }),
                spread_ratio: match (min, max) {
                    (Some(min), Some(max)) if min > 0.0 => Some(max / min),
                    _ => None,
                },
            }
        })
        .collect()
}

fn median_sample(mut samples: Vec<f64>) -> Option<f64> {
    if samples.is_empty() {
        return None;
    }
    samples.sort_by(f64::total_cmp);
    Some(samples[samples.len() / 2])
}

fn recommend_browser_math_case(
    op: &'static str,
    shape: BrowserMathBenchShape,
    cases: &[BrowserMathBenchCase],
    limits: Option<BrowserMathBenchLimits>,
) -> BrowserMathBenchRecommendation {
    let measurements = browser_math_bench_mode_measurements(op, shape, cases);
    let cpu = measurements
        .iter()
        .copied()
        .find(|measurement| measurement.mode == BrowserBenchMode::CpuWasm);
    let fastest_gpu = measurements
        .iter()
        .copied()
        .filter(|measurement| {
            !matches!(
                measurement.mode,
                BrowserBenchMode::Auto
                    | BrowserBenchMode::AutoPipelined
                    | BrowserBenchMode::AutoResidentPipelined
                    | BrowserBenchMode::AutoResidentDirectPipelined
                    | BrowserBenchMode::CpuWasm
                    | BrowserBenchMode::WebGpuPreparedResidentSubmitOnlyPipelined
                    | BrowserBenchMode::WebGpuPreparedCapacityResidentSubmitOnlyPipelined
                    | BrowserBenchMode::WebGpuPreparedResidentDispatchOnlyPipelined
                    | BrowserBenchMode::WebGpuPreparedCapacityResidentDispatchOnlyPipelined
                    | BrowserBenchMode::WebGpuPreparedResidentChainedDispatchOnlyPipelined
                    | BrowserBenchMode::WebGpuPreparedResidentMatmulBiasDispatchOnlyPipelined
            )
        })
        .min_by(|lhs, rhs| lhs.median_ms.total_cmp(&rhs.median_ms));

    let mut recommendation = build_browser_math_recommendation(op, shape, cpu, fastest_gpu);
    attach_policy_recommendation(&mut recommendation, limits);
    recommendation
}

#[derive(Clone, Copy, Debug)]
struct BrowserMathBenchModeMeasurement {
    mode: BrowserBenchMode,
    capacity: Option<BrowserMathBenchCapacity>,
    median_ms: f64,
    mad_ms: Option<f64>,
    p95_ms: Option<f64>,
}

fn browser_math_bench_mode_measurements(
    op: &'static str,
    shape: BrowserMathBenchShape,
    cases: &[BrowserMathBenchCase],
) -> Vec<BrowserMathBenchModeMeasurement> {
    let mut groups = Vec::<(BrowserBenchMode, Option<BrowserMathBenchCapacity>)>::new();
    for case in cases
        .iter()
        .filter(|case| case.op == op && case.shape == shape && measured_case(case))
    {
        let key = (case.mode, case.capacity);
        if !groups.contains(&key) {
            groups.push(key);
        }
    }
    groups
        .into_iter()
        .filter_map(|(mode, capacity)| {
            let matching_cases = cases
                .iter()
                .filter(|case| {
                    case.op == op
                        && case.shape == shape
                        && case.mode == mode
                        && case.capacity == capacity
                        && measured_case(case)
                })
                .collect::<Vec<_>>();
            let medians = matching_cases
                .iter()
                .filter_map(|case| case.median_ms)
                .collect::<Vec<_>>();
            let mad_ms = median_sample(
                matching_cases
                    .iter()
                    .filter_map(|case| case.mad_ms)
                    .collect(),
            );
            let p95_ms = median_sample(
                matching_cases
                    .iter()
                    .filter_map(|case| case.p95_ms)
                    .collect(),
            );
            median_sample(medians).map(|median_ms| BrowserMathBenchModeMeasurement {
                mode,
                capacity,
                median_ms,
                mad_ms,
                p95_ms,
            })
        })
        .collect()
}

fn build_browser_math_recommendation(
    op: &'static str,
    shape: BrowserMathBenchShape,
    cpu: Option<BrowserMathBenchModeMeasurement>,
    fastest_gpu: Option<BrowserMathBenchModeMeasurement>,
) -> BrowserMathBenchRecommendation {
    match (cpu, fastest_gpu) {
        (Some(cpu), Some(gpu)) => {
            let cpu_ms = cpu.median_ms;
            let gpu_ms = gpu.median_ms;
            if gpu_ms > 0.0 && gpu_ms < cpu_ms {
                BrowserMathBenchRecommendation {
                    op,
                    shape,
                    selected_mode: Some(gpu.mode),
                    selected_capacity: gpu.capacity,
                    policy_mode: None,
                    policy_capacity: None,
                    policy_reason: None,
                    policy_matches_selected: None,
                    selected_median_ms: Some(gpu_ms),
                    selected_mad_ms: gpu.mad_ms,
                    selected_p95_ms: gpu.p95_ms,
                    cpu_median_ms: Some(cpu_ms),
                    cpu_mad_ms: cpu.mad_ms,
                    cpu_p95_ms: cpu.p95_ms,
                    speedup: Some(cpu_ms / gpu_ms),
                    reason: BrowserMathBenchRecommendationReason::WebGpuFaster,
                }
            } else {
                BrowserMathBenchRecommendation {
                    op,
                    shape,
                    selected_mode: Some(BrowserBenchMode::CpuWasm),
                    selected_capacity: None,
                    policy_mode: None,
                    policy_capacity: None,
                    policy_reason: None,
                    policy_matches_selected: None,
                    selected_median_ms: Some(cpu_ms),
                    selected_mad_ms: cpu.mad_ms,
                    selected_p95_ms: cpu.p95_ms,
                    cpu_median_ms: Some(cpu_ms),
                    cpu_mad_ms: cpu.mad_ms,
                    cpu_p95_ms: cpu.p95_ms,
                    speedup: Some(1.0),
                    reason: BrowserMathBenchRecommendationReason::CpuFasterOrEqual,
                }
            }
        }
        (Some(cpu), None) => BrowserMathBenchRecommendation {
            op,
            shape,
            selected_mode: Some(BrowserBenchMode::CpuWasm),
            selected_capacity: None,
            policy_mode: None,
            policy_capacity: None,
            policy_reason: None,
            policy_matches_selected: None,
            selected_median_ms: Some(cpu.median_ms),
            selected_mad_ms: cpu.mad_ms,
            selected_p95_ms: cpu.p95_ms,
            cpu_median_ms: Some(cpu.median_ms),
            cpu_mad_ms: cpu.mad_ms,
            cpu_p95_ms: cpu.p95_ms,
            speedup: Some(1.0),
            reason: BrowserMathBenchRecommendationReason::NoMeasuredWebGpuCase,
        },
        (None, Some(gpu)) => BrowserMathBenchRecommendation {
            op,
            shape,
            selected_mode: Some(gpu.mode),
            selected_capacity: gpu.capacity,
            policy_mode: None,
            policy_capacity: None,
            policy_reason: None,
            policy_matches_selected: None,
            selected_median_ms: Some(gpu.median_ms),
            selected_mad_ms: gpu.mad_ms,
            selected_p95_ms: gpu.p95_ms,
            cpu_median_ms: None,
            cpu_mad_ms: None,
            cpu_p95_ms: None,
            speedup: None,
            reason: BrowserMathBenchRecommendationReason::MissingCpuBaseline,
        },
        (None, None) => BrowserMathBenchRecommendation {
            op,
            shape,
            selected_mode: None,
            selected_capacity: None,
            policy_mode: None,
            policy_capacity: None,
            policy_reason: None,
            policy_matches_selected: None,
            selected_median_ms: None,
            selected_mad_ms: None,
            selected_p95_ms: None,
            cpu_median_ms: None,
            cpu_mad_ms: None,
            cpu_p95_ms: None,
            speedup: None,
            reason: BrowserMathBenchRecommendationReason::NoMeasuredWebGpuCase,
        },
    }
}

fn measured_case(case: &BrowserMathBenchCase) -> bool {
    case.fallback_reason.is_none()
        && case.correctness.passed
        && case.median_ms.is_some_and(f64::is_finite)
}

fn attach_policy_recommendation(
    recommendation: &mut BrowserMathBenchRecommendation,
    limits: Option<BrowserMathBenchLimits>,
) {
    let Some(limits) = limits else {
        return;
    };
    let Some(policy) =
        browser_math_policy_selection(recommendation.op, recommendation.shape, limits)
    else {
        return;
    };
    recommendation.policy_mode = Some(policy.mode);
    recommendation.policy_capacity = policy.capacity;
    recommendation.policy_reason = Some(policy.reason);
    recommendation.policy_matches_selected = recommendation
        .selected_mode
        .map(|selected_mode| selected_mode == policy.mode);
}

fn browser_math_policy_selection(
    op: &'static str,
    shape: BrowserMathBenchShape,
    limits: BrowserMathBenchLimits,
) -> Option<BrowserMathBenchPolicySelection> {
    let limits = BrowserWebGpuLimits {
        max_storage_buffer_binding_size: limits.max_storage_buffer_binding_size,
        max_buffer_size: limits.max_buffer_size,
        max_compute_invocations_per_workgroup: limits.max_compute_invocations_per_workgroup,
        max_compute_workgroups_per_dimension: limits.max_compute_workgroups_per_dimension,
    };
    let policy = BrowserWebGpuMathAutoPolicy::default();
    let selection = match (op, shape) {
        ("matmul_f32", BrowserMathBenchShape::Matmul { rows, shared, cols }) => {
            policy.select_matmul_f32(rows, shared, cols, limits)
        }
        ("matrix_add_f32" | "tensor_add_f32", BrowserMathBenchShape::Len { len }) => {
            policy.select_elementwise_f32(len, limits)
        }
        (
            "matrix_add_f32",
            BrowserMathBenchShape::Matmul {
                rows,
                shared: _,
                cols,
            },
        ) => policy.select_elementwise_f32(rows.saturating_mul(cols), limits),
        _ => return None,
    };
    Some(BrowserMathBenchPolicySelection {
        mode: browser_bench_mode_for_policy(selection.mode()),
        capacity: selection.capacity().map(Into::into),
        reason: browser_math_policy_reason(selection.reason()),
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BrowserMathBenchPolicySelection {
    mode: BrowserBenchMode,
    capacity: Option<BrowserMathBenchCapacity>,
    reason: BrowserMathBenchPolicyReason,
}

const fn browser_bench_mode_for_policy(mode: BrowserWebGpuMathMode) -> BrowserBenchMode {
    match mode {
        BrowserWebGpuMathMode::CpuWasm => BrowserBenchMode::CpuWasm,
        BrowserWebGpuMathMode::WebGpuPreparedResidentPipelined => {
            BrowserBenchMode::WebGpuPreparedResidentPipelined
        }
        BrowserWebGpuMathMode::WebGpuPreparedCapacityResidentPipelined => {
            BrowserBenchMode::WebGpuPreparedCapacityResidentPipelined
        }
    }
}

const fn browser_math_policy_reason(
    reason: BrowserWebGpuMathAutoReason,
) -> BrowserMathBenchPolicyReason {
    match reason {
        BrowserWebGpuMathAutoReason::MatmulPreparedResidentPipelined => {
            BrowserMathBenchPolicyReason::MatmulPreparedResidentPipelined
        }
        BrowserWebGpuMathAutoReason::MatmulPreparedCapacityResidentPipelined => {
            BrowserMathBenchPolicyReason::MatmulPreparedCapacityResidentPipelined
        }
        BrowserWebGpuMathAutoReason::MatmulCpuDefault => {
            BrowserMathBenchPolicyReason::MatmulCpuDefault
        }
        BrowserWebGpuMathAutoReason::ElementwisePreparedResidentPipelined => {
            BrowserMathBenchPolicyReason::ElementwisePreparedResidentPipelined
        }
        BrowserWebGpuMathAutoReason::ElementwiseCpuReadbackDominated => {
            BrowserMathBenchPolicyReason::ElementwiseCpuReadbackDominated
        }
        BrowserWebGpuMathAutoReason::StorageLimit => BrowserMathBenchPolicyReason::StorageLimit,
    }
}

#[cfg(target_arch = "wasm32")]
mod wasm {
    use super::{
        BrowserBenchMode, BrowserBenchModeOrder, BrowserMathBenchCapacity, BrowserMathBenchCase,
        BrowserMathBenchConfig, BrowserMathBenchCorrectness, BrowserMathBenchLimits,
        BrowserMathBenchReport, BrowserMathBenchRun, BrowserMathBenchShape, BrowserMathBenchSkip,
        BrowserMathBenchWebGpu, MatmulShape, browser_math_bench_recommendations,
        browser_math_bench_stability, median_sample,
    };
    use arcweft_core::math::{DenseMatrixF32, DenseTensorF32};
    use arcweft_runtime_accelerator::math::browser_webgpu::{
        BrowserMatmulAddF32Shape, BrowserMatmulCapacity, BrowserResidentF32GraphInputs,
        BrowserResidentF32GraphSpec, BrowserResidentMatmulAddF32Inputs,
        BrowserResidentMatmulBiasAddF32Inputs, BrowserSubmittedF32, BrowserWebGpuAutoMathAdapter,
        BrowserWebGpuCapacityGrowth, BrowserWebGpuError, BrowserWebGpuMathAutoPolicy,
        BrowserWebGpuMathContext, BrowserWebGpuMathDispatch, BrowserWebGpuMathRequest,
        BrowserWebGpuMathResponse, BrowserWebGpuMathStats, BrowserWebGpuPreparedMath,
        BrowserWebGpuPreparedMathDispatch,
    };
    use wasm_bindgen::prelude::*;
    use wasm_bindgen_futures::JsFuture;

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
        let mut adapter = match BrowserWebGpuMathContext::new().await {
            Ok(context) => Some(BrowserWebGpuAutoMathAdapter::from_context(
                context,
                BrowserWebGpuMathAutoPolicy::default(),
            )),
            Err(error) => {
                skips.push(BrowserMathBenchSkip {
                    scope: "webgpu",
                    reason: fallback_reason(&error),
                });
                None
            }
        };
        let limits = adapter.as_ref().map(|adapter| {
            let limits = adapter.limits();
            BrowserMathBenchLimits {
                max_storage_buffer_binding_size: limits.max_storage_buffer_binding_size,
                max_buffer_size: limits.max_buffer_size,
                max_compute_invocations_per_workgroup: limits.max_compute_invocations_per_workgroup,
                max_compute_workgroups_per_dimension: limits.max_compute_workgroups_per_dimension,
            }
        });
        let mut cases = Vec::new();
        for len in &config.add_lengths {
            for round_index in 0..config.repeat_rounds.max(1) {
                let ordered_modes = ordered_modes(&config, round_index);
                for (mode_order_index, mode) in ordered_modes.into_iter().enumerate() {
                    cases.push(
                        run_add_case(
                            &config,
                            adapter.as_mut(),
                            mode,
                            *len,
                            round_index,
                            mode_order_index,
                        )
                        .await,
                    );
                }
            }
        }
        for shape in &config.matmul_shapes {
            for round_index in 0..config.repeat_rounds.max(1) {
                let ordered_modes = ordered_modes(&config, round_index);
                for (mode_order_index, mode) in ordered_modes.into_iter().enumerate() {
                    cases.push(
                        run_matmul_case(
                            &config,
                            adapter.as_mut(),
                            mode,
                            *shape,
                            round_index,
                            mode_order_index,
                        )
                        .await,
                    );
                }
            }
        }
        let stability = browser_math_bench_stability(&cases);
        let recommendations = browser_math_bench_recommendations(&cases, limits);
        BrowserMathBenchReport {
            schema_version: "arcweft.browser_webgpu_bench.v1",
            run: BrowserMathBenchRun {
                secure_context: availability.secure_context,
                cross_origin_isolated: availability.cross_origin_isolated,
                webgpu: BrowserMathBenchWebGpu {
                    available: adapter.is_some(),
                    fallback_reason: skips.first().map(|skip| skip.reason.clone()),
                    limits,
                },
            },
            cases,
            stability,
            recommendations,
            skips,
        }
    }

    fn ordered_modes(config: &BrowserMathBenchConfig, round_index: usize) -> Vec<BrowserBenchMode> {
        let mut modes = config.modes.clone();
        if modes.is_empty() {
            return modes;
        }
        match config.mode_order {
            BrowserBenchModeOrder::AsListed => modes,
            BrowserBenchModeOrder::RotateByRound => {
                let offset = round_index % modes.len();
                modes.rotate_left(offset);
                modes
            }
        }
    }

    async fn run_add_case(
        config: &BrowserMathBenchConfig,
        adapter: Option<&mut BrowserWebGpuAutoMathAdapter>,
        mode: BrowserBenchMode,
        len: usize,
        round_index: usize,
        mode_order_index: usize,
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
            round_index,
            mode_order_index,
            config,
        );
        match mode {
            BrowserBenchMode::Auto => {
                let Some(adapter) = adapter else {
                    return skipped_case(case, "webgpu_unavailable");
                };
                let lhs_tensor =
                    DenseTensorF32::new(vec![len], lhs.clone()).expect("valid lhs tensor");
                let rhs_tensor =
                    DenseTensorF32::new(vec![len], rhs.clone()).expect("valid rhs tensor");
                adapter.reset_stats();
                let mut out = Vec::new();
                let mut samples = Vec::with_capacity(config.sample_iters);
                let mut error = None;
                for _ in 0..config.warmup_iters {
                    match adapter
                        .dispatch(BrowserWebGpuMathRequest::TensorAddF32 {
                            lhs: &lhs_tensor,
                            rhs: &rhs_tensor,
                        })
                        .await
                    {
                        Ok(response) => {
                            if let Some(tensor) = response.tensor_f32() {
                                out = tensor.values().to_vec();
                            }
                        }
                        Err(current) => {
                            error = Some(current);
                            break;
                        }
                    }
                }
                if error.is_none() {
                    for _ in 0..config.sample_iters {
                        let start = now_ms();
                        match adapter
                            .dispatch(BrowserWebGpuMathRequest::TensorAddF32 {
                                lhs: &lhs_tensor,
                                rhs: &rhs_tensor,
                            })
                            .await
                        {
                            Ok(response) => {
                                if let Some(tensor) = response.tensor_f32() {
                                    out = tensor.values().to_vec();
                                    case.capacity = response.selection().capacity().map(Into::into);
                                }
                            }
                            Err(current) => {
                                error = Some(current);
                                break;
                            }
                        }
                        samples.push(now_ms() - start);
                    }
                }
                case = finish_gpu_case(case, error, samples, adapter.stats(), &expected, &out);
            }
            BrowserBenchMode::AutoPipelined => {
                let Some(adapter) = adapter else {
                    return skipped_case(case, "webgpu_unavailable");
                };
                let lhs_tensor =
                    DenseTensorF32::new(vec![len], lhs.clone()).expect("valid lhs tensor");
                let rhs_tensor =
                    DenseTensorF32::new(vec![len], rhs.clone()).expect("valid rhs tensor");
                case = run_auto_pipelined_case(
                    config,
                    mode,
                    case,
                    adapter,
                    &expected,
                    |adapter| {
                        adapter.submit(BrowserWebGpuMathRequest::TensorAddF32 {
                            lhs: &lhs_tensor,
                            rhs: &rhs_tensor,
                        })
                    },
                    capture_tensor_response,
                )
                .await;
            }
            BrowserBenchMode::AutoResidentPipelined => {
                let Some(adapter) = adapter else {
                    return skipped_case(case, "webgpu_unavailable");
                };
                let lhs_tensor =
                    DenseTensorF32::new(vec![len], lhs.clone()).expect("valid lhs tensor");
                let rhs_tensor =
                    DenseTensorF32::new(vec![len], rhs.clone()).expect("valid rhs tensor");
                case = run_auto_resident_pipelined_case(
                    config,
                    mode,
                    case,
                    adapter,
                    &expected,
                    BrowserWebGpuMathRequest::TensorAddF32 {
                        lhs: &lhs_tensor,
                        rhs: &rhs_tensor,
                    },
                    capture_tensor_response,
                )
                .await;
            }
            BrowserBenchMode::AutoResidentDirectPipelined => {
                let Some(adapter) = adapter else {
                    return skipped_case(case, "webgpu_unavailable");
                };
                let lhs_tensor =
                    DenseTensorF32::new(vec![len], lhs.clone()).expect("valid lhs tensor");
                let rhs_tensor =
                    DenseTensorF32::new(vec![len], rhs.clone()).expect("valid rhs tensor");
                case = run_auto_resident_direct_pipelined_case(
                    config,
                    mode,
                    case,
                    adapter,
                    &expected,
                    BrowserWebGpuMathRequest::TensorAddF32 {
                        lhs: &lhs_tensor,
                        rhs: &rhs_tensor,
                    },
                )
                .await;
            }
            BrowserBenchMode::CpuWasm => {
                let mut out = Vec::new();
                case = measure_case(case, config, || {
                    out = add_cpu(&lhs, &rhs);
                });
                case.correctness = compare(&expected, &out, 1.0e-6, 1.0e-6);
                case.checksum = checksum(&out);
            }
            BrowserBenchMode::WebGpuOneShot => {
                let Some(adapter) = adapter else {
                    return skipped_case(case, "webgpu_unavailable");
                };
                let context = adapter.context_mut();
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
                let Some(adapter) = adapter else {
                    return skipped_case(case, "webgpu_unavailable");
                };
                let context = adapter.context_mut();
                context.reset_stats();
                case.capacity = Some(BrowserMathBenchCapacity::Len { len });
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
            BrowserBenchMode::WebGpuPreparedResident
            | BrowserBenchMode::WebGpuPreparedCapacityResident => {
                let Some(adapter) = adapter else {
                    return skipped_case(case, "webgpu_unavailable");
                };
                let context = adapter.context_mut();
                context.reset_stats();
                let capacity_len = elementwise_capacity_len(mode, len);
                case.capacity = Some(BrowserMathBenchCapacity::Len { len: capacity_len });
                let prepared = match context.prepare_elementwise_f32(capacity_len) {
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
            BrowserBenchMode::WebGpuPreparedResidentAsync
            | BrowserBenchMode::WebGpuPreparedResidentPipelined
            | BrowserBenchMode::WebGpuPreparedCapacityResidentPipelined => {
                let Some(adapter) = adapter else {
                    return skipped_case(case, "webgpu_unavailable");
                };
                let context = adapter.context_mut();
                let batch_depth = async_batch_depth(mode, config);
                context.reset_stats();
                let capacity_len = elementwise_capacity_len(mode, len);
                case.capacity = Some(BrowserMathBenchCapacity::Len { len: capacity_len });
                let prepared = match context.prepare_elementwise_f32(capacity_len) {
                    Ok(prepared) => prepared,
                    Err(error) => return skipped_case(case, &fallback_reason(&error)),
                };
                if let Err(error) = context.upload_prepared_elementwise_f32(&prepared, &lhs, &rhs) {
                    return skipped_case(case, &fallback_reason(&error));
                }
                let mut out = vec![0.0; len];
                let mut samples = Vec::with_capacity(config.sample_iters);
                let mut submit_samples = Vec::with_capacity(config.sample_iters);
                let mut readback_samples = Vec::with_capacity(config.sample_iters);
                let mut error = None;
                for _ in 0..config.warmup_iters {
                    let mut submitted = Vec::with_capacity(batch_depth);
                    for _ in 0..batch_depth {
                        match context.submit_resident_elementwise_f32(&prepared, len) {
                            Ok(current) => submitted.push(current),
                            Err(current) => {
                                error = Some(current);
                                break;
                            }
                        }
                    }
                    yield_to_browser().await;
                    for current in submitted {
                        if let Err(current) = context.read_submitted_f32(current, &mut out).await {
                            error = Some(current);
                            break;
                        }
                    }
                    if error.is_some() {
                        break;
                    }
                }
                if error.is_none() {
                    for _ in 0..config.sample_iters {
                        let total_start = now_ms();
                        let mut submitted = Vec::with_capacity(batch_depth);
                        for _ in 0..batch_depth {
                            let submit_start = now_ms();
                            match context.submit_resident_elementwise_f32(&prepared, len) {
                                Ok(current) => submitted.push(current),
                                Err(current) => {
                                    error = Some(current);
                                    break;
                                }
                            }
                            submit_samples.push(now_ms() - submit_start);
                        }
                        if error.is_some() {
                            break;
                        }
                        yield_to_browser().await;
                        for current in submitted {
                            let readback_start = now_ms();
                            if let Err(current) =
                                context.read_submitted_f32(current, &mut out).await
                            {
                                error = Some(current);
                                break;
                            }
                            readback_samples.push(now_ms() - readback_start);
                        }
                        if error.is_some() {
                            break;
                        }
                        samples.push((now_ms() - total_start) / batch_depth as f64);
                    }
                }
                case = finish_gpu_case(case, error, samples, context.stats(), &expected, &out);
                fill_breakdown(&mut case, submit_samples, readback_samples);
            }
            BrowserBenchMode::WebGpuPreparedResidentSubmitOnlyPipelined
            | BrowserBenchMode::WebGpuPreparedCapacityResidentSubmitOnlyPipelined => {
                let Some(adapter) = adapter else {
                    return skipped_case(case, "webgpu_unavailable");
                };
                let context = adapter.context_mut();
                let batch_depth = async_batch_depth(mode, config);
                context.reset_stats();
                let capacity_len = elementwise_capacity_len(mode, len);
                case.capacity = Some(BrowserMathBenchCapacity::Len { len: capacity_len });
                let prepared = match context.prepare_elementwise_f32(capacity_len) {
                    Ok(prepared) => prepared,
                    Err(error) => return skipped_case(case, &fallback_reason(&error)),
                };
                if let Err(error) = context.upload_prepared_elementwise_f32(&prepared, &lhs, &rhs) {
                    return skipped_case(case, &fallback_reason(&error));
                }
                let mut out = vec![0.0; len];
                let mut samples = Vec::with_capacity(config.sample_iters);
                let mut submit_samples = Vec::with_capacity(config.sample_iters * batch_depth);
                let mut drain = Vec::with_capacity(config.sample_iters * batch_depth);
                let mut error = None;
                for _ in 0..config.warmup_iters {
                    let mut submitted = Vec::with_capacity(batch_depth);
                    for _ in 0..batch_depth {
                        match context.submit_resident_elementwise_f32(&prepared, len) {
                            Ok(current) => submitted.push(current),
                            Err(current) => {
                                error = Some(current);
                                break;
                            }
                        }
                    }
                    yield_to_browser().await;
                    for current in submitted {
                        if let Err(current) = context.read_submitted_f32(current, &mut out).await {
                            error = Some(current);
                            break;
                        }
                    }
                    if error.is_some() {
                        break;
                    }
                }
                if error.is_none() {
                    for _ in 0..config.sample_iters {
                        let total_start = now_ms();
                        for _ in 0..batch_depth {
                            let submit_start = now_ms();
                            match context.submit_resident_elementwise_f32(&prepared, len) {
                                Ok(current) => drain.push(current),
                                Err(current) => {
                                    error = Some(current);
                                    break;
                                }
                            }
                            submit_samples.push(now_ms() - submit_start);
                        }
                        if error.is_some() {
                            break;
                        }
                        yield_to_browser().await;
                        samples.push((now_ms() - total_start) / batch_depth as f64);
                    }
                }
                if error.is_none() {
                    for current in drain {
                        if let Err(current) = context.read_submitted_f32(current, &mut out).await {
                            error = Some(current);
                            break;
                        }
                    }
                }
                case = finish_gpu_case(case, error, samples, context.stats(), &expected, &out);
                fill_breakdown(&mut case, submit_samples, Vec::new());
            }
            BrowserBenchMode::WebGpuPreparedResidentDispatchOnlyPipelined
            | BrowserBenchMode::WebGpuPreparedCapacityResidentDispatchOnlyPipelined => {
                let Some(adapter) = adapter else {
                    return skipped_case(case, "webgpu_unavailable");
                };
                let context = adapter.context_mut();
                let batch_depth = async_batch_depth(mode, config);
                context.reset_stats();
                let capacity_len = elementwise_capacity_len(mode, len);
                case.capacity = Some(BrowserMathBenchCapacity::Len { len: capacity_len });
                let prepared = match context.prepare_elementwise_f32(capacity_len) {
                    Ok(prepared) => prepared,
                    Err(error) => return skipped_case(case, &fallback_reason(&error)),
                };
                if let Err(error) = context.upload_prepared_elementwise_f32(&prepared, &lhs, &rhs) {
                    return skipped_case(case, &fallback_reason(&error));
                }
                let mut out = vec![0.0; len];
                let mut samples = Vec::with_capacity(config.sample_iters);
                let mut submit_samples = Vec::with_capacity(config.sample_iters * batch_depth);
                let mut latest = None;
                let mut error = None;
                for _ in 0..config.warmup_iters {
                    let mut submitted = None;
                    for _ in 0..batch_depth {
                        match context
                            .submit_resident_elementwise_f32_without_readback(&prepared, len)
                        {
                            Ok(current) => submitted = Some(current),
                            Err(current) => {
                                error = Some(current);
                                break;
                            }
                        }
                    }
                    yield_to_browser().await;
                    if let Some(current) = submitted
                        && let Err(current) = context
                            .read_resident_elementwise_f32(&prepared, current, &mut out)
                            .await
                    {
                        error = Some(current);
                    }
                    if error.is_some() {
                        break;
                    }
                }
                if error.is_none() {
                    for _ in 0..config.sample_iters {
                        let total_start = now_ms();
                        for _ in 0..batch_depth {
                            let submit_start = now_ms();
                            match context
                                .submit_resident_elementwise_f32_without_readback(&prepared, len)
                            {
                                Ok(current) => latest = Some(current),
                                Err(current) => {
                                    error = Some(current);
                                    break;
                                }
                            }
                            submit_samples.push(now_ms() - submit_start);
                        }
                        if error.is_some() {
                            break;
                        }
                        yield_to_browser().await;
                        samples.push((now_ms() - total_start) / batch_depth as f64);
                    }
                }
                if error.is_none()
                    && let Some(current) = latest
                    && let Err(current) = context
                        .read_resident_elementwise_f32(&prepared, current, &mut out)
                        .await
                {
                    error = Some(current);
                }
                case = finish_gpu_case(case, error, samples, context.stats(), &expected, &out);
                fill_breakdown(&mut case, submit_samples, Vec::new());
            }
            BrowserBenchMode::WebGpuPreparedResidentChainedDispatchOnlyPipelined
            | BrowserBenchMode::WebGpuPreparedResidentMatmulBiasDispatchOnlyPipelined => {
                return skipped_case(case, "mode_only_supports_matmul");
            }
        }
        case
    }

    async fn run_matmul_case(
        config: &BrowserMathBenchConfig,
        adapter: Option<&mut BrowserWebGpuAutoMathAdapter>,
        mode: BrowserBenchMode,
        shape: MatmulShape,
        round_index: usize,
        mode_order_index: usize,
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
            round_index,
            mode_order_index,
            config,
        );
        match mode {
            BrowserBenchMode::Auto => {
                let Some(adapter) = adapter else {
                    return skipped_case(case, "webgpu_unavailable");
                };
                let lhs_matrix = DenseMatrixF32::new(shape.rows, shape.shared, lhs.clone())
                    .expect("valid lhs matrix");
                let rhs_matrix = DenseMatrixF32::new(shape.shared, shape.cols, rhs.clone())
                    .expect("valid rhs matrix");
                adapter.reset_stats();
                let mut out = Vec::new();
                let mut samples = Vec::with_capacity(config.sample_iters);
                let mut error = None;
                for _ in 0..config.warmup_iters {
                    match adapter
                        .dispatch(BrowserWebGpuMathRequest::MatmulF32 {
                            lhs: &lhs_matrix,
                            rhs: &rhs_matrix,
                        })
                        .await
                    {
                        Ok(response) => {
                            if let Some(matrix) = response.matrix_f32() {
                                out = matrix.values().to_vec();
                            }
                        }
                        Err(current) => {
                            error = Some(current);
                            break;
                        }
                    }
                }
                if error.is_none() {
                    for _ in 0..config.sample_iters {
                        let start = now_ms();
                        match adapter
                            .dispatch(BrowserWebGpuMathRequest::MatmulF32 {
                                lhs: &lhs_matrix,
                                rhs: &rhs_matrix,
                            })
                            .await
                        {
                            Ok(response) => {
                                if let Some(matrix) = response.matrix_f32() {
                                    out = matrix.values().to_vec();
                                    case.capacity = response.selection().capacity().map(Into::into);
                                }
                            }
                            Err(current) => {
                                error = Some(current);
                                break;
                            }
                        }
                        samples.push(now_ms() - start);
                    }
                }
                case = finish_gpu_case(case, error, samples, adapter.stats(), &expected, &out);
            }
            BrowserBenchMode::AutoPipelined => {
                let Some(adapter) = adapter else {
                    return skipped_case(case, "webgpu_unavailable");
                };
                let lhs_matrix = DenseMatrixF32::new(shape.rows, shape.shared, lhs.clone())
                    .expect("valid lhs matrix");
                let rhs_matrix = DenseMatrixF32::new(shape.shared, shape.cols, rhs.clone())
                    .expect("valid rhs matrix");
                case = run_auto_pipelined_case(
                    config,
                    mode,
                    case,
                    adapter,
                    &expected,
                    |adapter| {
                        adapter.submit(BrowserWebGpuMathRequest::MatmulF32 {
                            lhs: &lhs_matrix,
                            rhs: &rhs_matrix,
                        })
                    },
                    capture_matrix_response,
                )
                .await;
            }
            BrowserBenchMode::AutoResidentPipelined => {
                let Some(adapter) = adapter else {
                    return skipped_case(case, "webgpu_unavailable");
                };
                let lhs_matrix = DenseMatrixF32::new(shape.rows, shape.shared, lhs.clone())
                    .expect("valid lhs matrix");
                let rhs_matrix = DenseMatrixF32::new(shape.shared, shape.cols, rhs.clone())
                    .expect("valid rhs matrix");
                case = run_auto_resident_pipelined_case(
                    config,
                    mode,
                    case,
                    adapter,
                    &expected,
                    BrowserWebGpuMathRequest::MatmulF32 {
                        lhs: &lhs_matrix,
                        rhs: &rhs_matrix,
                    },
                    capture_matrix_response,
                )
                .await;
            }
            BrowserBenchMode::AutoResidentDirectPipelined => {
                let Some(adapter) = adapter else {
                    return skipped_case(case, "webgpu_unavailable");
                };
                let lhs_matrix = DenseMatrixF32::new(shape.rows, shape.shared, lhs.clone())
                    .expect("valid lhs matrix");
                let rhs_matrix = DenseMatrixF32::new(shape.shared, shape.cols, rhs.clone())
                    .expect("valid rhs matrix");
                case = run_auto_resident_direct_pipelined_case(
                    config,
                    mode,
                    case,
                    adapter,
                    &expected,
                    BrowserWebGpuMathRequest::MatmulF32 {
                        lhs: &lhs_matrix,
                        rhs: &rhs_matrix,
                    },
                )
                .await;
            }
            BrowserBenchMode::CpuWasm => {
                let mut out = Vec::new();
                case = measure_case(case, config, || {
                    out = matmul_cpu(&lhs, &rhs, shape);
                });
                case.correctness = compare(&expected, &out, matmul_abs_tol(shape.shared), 1.0e-4);
                case.checksum = checksum(&out);
            }
            BrowserBenchMode::WebGpuOneShot => {
                let Some(adapter) = adapter else {
                    return skipped_case(case, "webgpu_unavailable");
                };
                let context = adapter.context_mut();
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
                let Some(adapter) = adapter else {
                    return skipped_case(case, "webgpu_unavailable");
                };
                let context = adapter.context_mut();
                context.reset_stats();
                let capacity = BrowserMatmulCapacity {
                    rows: shape.rows,
                    shared: shape.shared,
                    cols: shape.cols,
                };
                case.capacity = Some(capacity.into());
                let prepared = match context.prepare_matmul_f32(capacity) {
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
            BrowserBenchMode::WebGpuPreparedResident
            | BrowserBenchMode::WebGpuPreparedCapacityResident => {
                let Some(adapter) = adapter else {
                    return skipped_case(case, "webgpu_unavailable");
                };
                let context = adapter.context_mut();
                context.reset_stats();
                let capacity = matmul_capacity(mode, shape);
                case.capacity = Some(capacity.into());
                let prepared = match context.prepare_matmul_f32(capacity) {
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
            BrowserBenchMode::WebGpuPreparedResidentAsync
            | BrowserBenchMode::WebGpuPreparedResidentPipelined
            | BrowserBenchMode::WebGpuPreparedCapacityResidentPipelined => {
                let Some(adapter) = adapter else {
                    return skipped_case(case, "webgpu_unavailable");
                };
                let context = adapter.context_mut();
                let batch_depth = async_batch_depth(mode, config);
                context.reset_stats();
                let capacity = matmul_capacity(mode, shape);
                case.capacity = Some(capacity.into());
                let prepared = match context.prepare_matmul_f32(capacity) {
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
                let mut submit_samples = Vec::with_capacity(config.sample_iters);
                let mut readback_samples = Vec::with_capacity(config.sample_iters);
                let mut error = None;
                for _ in 0..config.warmup_iters {
                    let mut submitted = Vec::with_capacity(batch_depth);
                    for _ in 0..batch_depth {
                        match context.submit_resident_matmul_f32(&prepared, shape.rows, shape.cols)
                        {
                            Ok(current) => submitted.push(current),
                            Err(current) => {
                                error = Some(current);
                                break;
                            }
                        }
                    }
                    yield_to_browser().await;
                    for current in submitted {
                        if let Err(current) = context.read_submitted_f32(current, &mut out).await {
                            error = Some(current);
                            break;
                        }
                    }
                    if error.is_some() {
                        break;
                    }
                }
                if error.is_none() {
                    for _ in 0..config.sample_iters {
                        let total_start = now_ms();
                        let mut submitted = Vec::with_capacity(batch_depth);
                        for _ in 0..batch_depth {
                            let submit_start = now_ms();
                            match context
                                .submit_resident_matmul_f32(&prepared, shape.rows, shape.cols)
                            {
                                Ok(current) => submitted.push(current),
                                Err(current) => {
                                    error = Some(current);
                                    break;
                                }
                            }
                            submit_samples.push(now_ms() - submit_start);
                        }
                        if error.is_some() {
                            break;
                        }
                        yield_to_browser().await;
                        for current in submitted {
                            let readback_start = now_ms();
                            if let Err(current) =
                                context.read_submitted_f32(current, &mut out).await
                            {
                                error = Some(current);
                                break;
                            }
                            readback_samples.push(now_ms() - readback_start);
                        }
                        if error.is_some() {
                            break;
                        }
                        samples.push((now_ms() - total_start) / batch_depth as f64);
                    }
                }
                case = finish_gpu_case(case, error, samples, context.stats(), &expected, &out);
                fill_breakdown(&mut case, submit_samples, readback_samples);
            }
            BrowserBenchMode::WebGpuPreparedResidentSubmitOnlyPipelined
            | BrowserBenchMode::WebGpuPreparedCapacityResidentSubmitOnlyPipelined => {
                let Some(adapter) = adapter else {
                    return skipped_case(case, "webgpu_unavailable");
                };
                let context = adapter.context_mut();
                let batch_depth = async_batch_depth(mode, config);
                context.reset_stats();
                let capacity = matmul_capacity(mode, shape);
                case.capacity = Some(capacity.into());
                let prepared = match context.prepare_matmul_f32(capacity) {
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
                let mut submit_samples = Vec::with_capacity(config.sample_iters * batch_depth);
                let mut drain = Vec::with_capacity(config.sample_iters * batch_depth);
                let mut error = None;
                for _ in 0..config.warmup_iters {
                    let mut submitted = Vec::with_capacity(batch_depth);
                    for _ in 0..batch_depth {
                        match context.submit_resident_matmul_f32(&prepared, shape.rows, shape.cols)
                        {
                            Ok(current) => submitted.push(current),
                            Err(current) => {
                                error = Some(current);
                                break;
                            }
                        }
                    }
                    yield_to_browser().await;
                    for current in submitted {
                        if let Err(current) = context.read_submitted_f32(current, &mut out).await {
                            error = Some(current);
                            break;
                        }
                    }
                    if error.is_some() {
                        break;
                    }
                }
                if error.is_none() {
                    for _ in 0..config.sample_iters {
                        let total_start = now_ms();
                        for _ in 0..batch_depth {
                            let submit_start = now_ms();
                            match context
                                .submit_resident_matmul_f32(&prepared, shape.rows, shape.cols)
                            {
                                Ok(current) => drain.push(current),
                                Err(current) => {
                                    error = Some(current);
                                    break;
                                }
                            }
                            submit_samples.push(now_ms() - submit_start);
                        }
                        if error.is_some() {
                            break;
                        }
                        yield_to_browser().await;
                        samples.push((now_ms() - total_start) / batch_depth as f64);
                    }
                }
                if error.is_none() {
                    for current in drain {
                        if let Err(current) = context.read_submitted_f32(current, &mut out).await {
                            error = Some(current);
                            break;
                        }
                    }
                }
                case = finish_gpu_case(case, error, samples, context.stats(), &expected, &out);
                fill_breakdown(&mut case, submit_samples, Vec::new());
            }
            BrowserBenchMode::WebGpuPreparedResidentDispatchOnlyPipelined
            | BrowserBenchMode::WebGpuPreparedCapacityResidentDispatchOnlyPipelined => {
                let Some(adapter) = adapter else {
                    return skipped_case(case, "webgpu_unavailable");
                };
                let context = adapter.context_mut();
                let batch_depth = async_batch_depth(mode, config);
                context.reset_stats();
                let capacity = matmul_capacity(mode, shape);
                case.capacity = Some(capacity.into());
                let prepared = match context.prepare_matmul_f32(capacity) {
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
                let mut submit_samples = Vec::with_capacity(config.sample_iters * batch_depth);
                let mut latest = None;
                let mut error = None;
                for _ in 0..config.warmup_iters {
                    let mut submitted = None;
                    for _ in 0..batch_depth {
                        match context.submit_resident_matmul_f32_without_readback(
                            &prepared, shape.rows, shape.cols,
                        ) {
                            Ok(current) => submitted = Some(current),
                            Err(current) => {
                                error = Some(current);
                                break;
                            }
                        }
                    }
                    yield_to_browser().await;
                    if let Some(current) = submitted
                        && let Err(current) = context
                            .read_resident_matmul_f32(
                                &prepared, current, shape.rows, shape.cols, &mut out,
                            )
                            .await
                    {
                        error = Some(current);
                    }
                    if error.is_some() {
                        break;
                    }
                }
                if error.is_none() {
                    for _ in 0..config.sample_iters {
                        let total_start = now_ms();
                        for _ in 0..batch_depth {
                            let submit_start = now_ms();
                            match context.submit_resident_matmul_f32_without_readback(
                                &prepared, shape.rows, shape.cols,
                            ) {
                                Ok(current) => latest = Some(current),
                                Err(current) => {
                                    error = Some(current);
                                    break;
                                }
                            }
                            submit_samples.push(now_ms() - submit_start);
                        }
                        if error.is_some() {
                            break;
                        }
                        yield_to_browser().await;
                        samples.push((now_ms() - total_start) / batch_depth as f64);
                    }
                }
                if error.is_none()
                    && let Some(current) = latest
                    && let Err(current) = context
                        .read_resident_matmul_f32(
                            &prepared, current, shape.rows, shape.cols, &mut out,
                        )
                        .await
                {
                    error = Some(current);
                }
                case = finish_gpu_case(case, error, samples, context.stats(), &expected, &out);
                fill_breakdown(&mut case, submit_samples, Vec::new());
            }
            BrowserBenchMode::WebGpuPreparedResidentChainedDispatchOnlyPipelined => {
                let Some(adapter) = adapter else {
                    return skipped_case(case, "webgpu_unavailable");
                };
                let context = adapter.context_mut();
                let batch_depth = async_batch_depth(mode, config);
                context.reset_stats();
                let capacity = BrowserMatmulCapacity {
                    rows: shape.rows,
                    shared: shape.shared,
                    cols: shape.cols,
                };
                let graph_shape =
                    BrowserMatmulAddF32Shape::new(shape.rows, shape.shared, shape.cols);
                case.capacity = Some(capacity.into());
                let prepared = match context
                    .prepare_resident_f32_graph(BrowserResidentF32GraphSpec::matmul_add(capacity))
                {
                    Ok(prepared) => prepared,
                    Err(error) => return skipped_case(case, &fallback_reason(&error)),
                };
                let add_rhs = vec![0.0; shape.rows * shape.cols];
                let inputs = BrowserResidentF32GraphInputs::MatmulAdd(
                    BrowserResidentMatmulAddF32Inputs::new(&lhs, &rhs, &add_rhs, graph_shape),
                );
                if let Err(error) = context.upload_prepared_resident_f32_graph(&prepared, inputs) {
                    return skipped_case(case, &fallback_reason(&error));
                }
                let mut out = vec![0.0; shape.rows * shape.cols];
                let mut samples = Vec::with_capacity(config.sample_iters);
                let mut submit_samples = Vec::with_capacity(config.sample_iters * batch_depth);
                let mut latest = None;
                let mut error = None;
                for _ in 0..config.warmup_iters {
                    let mut submitted = None;
                    for _ in 0..batch_depth {
                        match context.submit_prepared_resident_f32_graph_without_readback(
                            &prepared,
                            graph_shape,
                        ) {
                            Ok(current) => submitted = Some(current),
                            Err(current) => {
                                error = Some(current);
                                break;
                            }
                        }
                    }
                    yield_to_browser().await;
                    if let Some(current) = submitted
                        && let Err(current) = context
                            .read_prepared_resident_f32_graph(
                                &prepared,
                                current,
                                graph_shape,
                                &mut out,
                            )
                            .await
                    {
                        error = Some(current);
                    }
                    if error.is_some() {
                        break;
                    }
                }
                if error.is_none() {
                    for _ in 0..config.sample_iters {
                        let total_start = now_ms();
                        for _ in 0..batch_depth {
                            let submit_start = now_ms();
                            match context.submit_prepared_resident_f32_graph_without_readback(
                                &prepared,
                                graph_shape,
                            ) {
                                Ok(current) => latest = Some(current),
                                Err(current) => {
                                    error = Some(current);
                                    break;
                                }
                            }
                            submit_samples.push(now_ms() - submit_start);
                        }
                        if error.is_some() {
                            break;
                        }
                        yield_to_browser().await;
                        samples.push((now_ms() - total_start) / batch_depth as f64);
                    }
                }
                if error.is_none()
                    && let Some(current) = latest
                    && let Err(current) = context
                        .read_prepared_resident_f32_graph(&prepared, current, graph_shape, &mut out)
                        .await
                {
                    error = Some(current);
                }
                case = finish_gpu_case(case, error, samples, context.stats(), &expected, &out);
                fill_breakdown(&mut case, submit_samples, Vec::new());
            }
            BrowserBenchMode::WebGpuPreparedResidentMatmulBiasDispatchOnlyPipelined => {
                let Some(adapter) = adapter else {
                    return skipped_case(case, "webgpu_unavailable");
                };
                let context = adapter.context_mut();
                let batch_depth = async_batch_depth(mode, config);
                context.reset_stats();
                let capacity = BrowserMatmulCapacity {
                    rows: shape.rows,
                    shared: shape.shared,
                    cols: shape.cols,
                };
                let graph_shape =
                    BrowserMatmulAddF32Shape::new(shape.rows, shape.shared, shape.cols);
                case.capacity = Some(capacity.into());
                let prepared = match context.prepare_resident_f32_graph(
                    BrowserResidentF32GraphSpec::matmul_bias_add(capacity),
                ) {
                    Ok(prepared) => prepared,
                    Err(error) => return skipped_case(case, &fallback_reason(&error)),
                };
                let bias = vec![0.0; shape.cols];
                let inputs = BrowserResidentF32GraphInputs::MatmulBiasAdd(
                    BrowserResidentMatmulBiasAddF32Inputs::new(&lhs, &rhs, &bias, graph_shape),
                );
                if let Err(error) = context.upload_prepared_resident_f32_graph(&prepared, inputs) {
                    return skipped_case(case, &fallback_reason(&error));
                }
                let mut out = vec![0.0; shape.rows * shape.cols];
                let mut samples = Vec::with_capacity(config.sample_iters);
                let mut submit_samples = Vec::with_capacity(config.sample_iters * batch_depth);
                let mut latest = None;
                let mut error = None;
                for _ in 0..config.warmup_iters {
                    let mut submitted = None;
                    for _ in 0..batch_depth {
                        match context.submit_prepared_resident_f32_graph_without_readback(
                            &prepared,
                            graph_shape,
                        ) {
                            Ok(current) => submitted = Some(current),
                            Err(current) => {
                                error = Some(current);
                                break;
                            }
                        }
                    }
                    yield_to_browser().await;
                    if let Some(current) = submitted
                        && let Err(current) = context
                            .read_prepared_resident_f32_graph(
                                &prepared,
                                current,
                                graph_shape,
                                &mut out,
                            )
                            .await
                    {
                        error = Some(current);
                    }
                    if error.is_some() {
                        break;
                    }
                }
                if error.is_none() {
                    for _ in 0..config.sample_iters {
                        let total_start = now_ms();
                        for _ in 0..batch_depth {
                            let submit_start = now_ms();
                            match context.submit_prepared_resident_f32_graph_without_readback(
                                &prepared,
                                graph_shape,
                            ) {
                                Ok(current) => latest = Some(current),
                                Err(current) => {
                                    error = Some(current);
                                    break;
                                }
                            }
                            submit_samples.push(now_ms() - submit_start);
                        }
                        if error.is_some() {
                            break;
                        }
                        yield_to_browser().await;
                        samples.push((now_ms() - total_start) / batch_depth as f64);
                    }
                }
                if error.is_none()
                    && let Some(current) = latest
                    && let Err(current) = context
                        .read_prepared_resident_f32_graph(&prepared, current, graph_shape, &mut out)
                        .await
                {
                    error = Some(current);
                }
                case = finish_gpu_case(case, error, samples, context.stats(), &expected, &out);
                fill_breakdown(&mut case, submit_samples, Vec::new());
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
        round_index: usize,
        mode_order_index: usize,
        config: &BrowserMathBenchConfig,
    ) -> BrowserMathBenchCase {
        let workgroups = estimated_workgroups(op, &shape);
        let work_items = estimated_work_items(op, &shape);
        let estimated_flops = estimated_flops(op, &shape);
        let case_id = if config.repeat_rounds.max(1) > 1 {
            format!("{case_id}_round{round_index}_order{mode_order_index}")
        } else {
            case_id
        };
        BrowserMathBenchCase {
            case_id,
            op,
            shape,
            capacity: None,
            mode,
            round_index,
            mode_order_index,
            warmup_iters: config.warmup_iters,
            sample_iters: config.sample_iters,
            median_ms: None,
            mad_ms: None,
            min_ms: None,
            p95_ms: None,
            effective_gflops: None,
            submit_median_ms: None,
            readback_median_ms: None,
            submit_median_share: None,
            readback_median_share: None,
            bytes_uploaded: 0,
            bytes_readback: 0,
            dispatches: 0,
            async_submissions: 0,
            async_readbacks: 0,
            max_in_flight: 0,
            buffer_alloc_count: 0,
            buffer_reuse_count: 0,
            workgroups,
            work_items,
            estimated_flops,
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
        case.async_submissions = stats.async_submissions;
        case.async_readbacks = stats.async_readbacks;
        case.max_in_flight = stats.max_in_flight;
        case.bytes_uploaded = stats.bytes_uploaded;
        case.bytes_readback = stats.bytes_downloaded;
        case.buffer_alloc_count = stats.buffer_creations + stats.readback_buffer_creations;
        case.buffer_reuse_count = stats.buffer_reuse_hits + stats.readback_buffer_reuse_hits;
        case
    }

    fn fill_breakdown(
        case: &mut BrowserMathBenchCase,
        submit_samples: Vec<f64>,
        readback_samples: Vec<f64>,
    ) {
        case.submit_median_ms = median_sample(submit_samples);
        case.readback_median_ms = median_sample(readback_samples);
        case.submit_median_share = median_share(case.submit_median_ms, case.median_ms);
        case.readback_median_share = median_share(case.readback_median_ms, case.median_ms);
    }

    async fn run_auto_pipelined_case(
        config: &BrowserMathBenchConfig,
        mode: BrowserBenchMode,
        mut case: BrowserMathBenchCase,
        adapter: &mut BrowserWebGpuAutoMathAdapter,
        expected: &[f32],
        mut submit: impl FnMut(
            &mut BrowserWebGpuAutoMathAdapter,
        ) -> Result<BrowserWebGpuMathDispatch, BrowserWebGpuError>,
        capture_immediate: fn(&BrowserWebGpuMathResponse, &mut Vec<f32>),
    ) -> BrowserMathBenchCase {
        let batch_depth = async_batch_depth(mode, config);
        adapter.reset_stats();
        let mut out = Vec::new();
        let mut samples = Vec::with_capacity(config.sample_iters);
        let mut submit_samples = Vec::with_capacity(config.sample_iters);
        let mut readback_samples = Vec::with_capacity(config.sample_iters);
        let mut error = None;
        for _ in 0..config.warmup_iters {
            let mut submitted = Vec::with_capacity(batch_depth);
            for _ in 0..batch_depth {
                match submit(adapter) {
                    Ok(BrowserWebGpuMathDispatch::Immediate(response)) => {
                        capture_immediate(&response, &mut out);
                        case.capacity = response.selection().capacity().map(Into::into);
                    }
                    Ok(BrowserWebGpuMathDispatch::Submitted(current)) => {
                        case.capacity = current.selection().capacity().map(Into::into);
                        submitted.push(current);
                    }
                    Err(current) => {
                        error = Some(current);
                        break;
                    }
                }
            }
            yield_to_browser().await;
            for current in submitted {
                resize_out_for_submitted(&mut out, current.len());
                match adapter.read_submitted_values_into(current, &mut out).await {
                    Ok(_) => {}
                    Err(current) => {
                        error = Some(current);
                        break;
                    }
                }
            }
            if error.is_some() {
                break;
            }
        }
        if error.is_none() {
            for _ in 0..config.sample_iters {
                let total_start = now_ms();
                let mut submitted = Vec::with_capacity(batch_depth);
                for _ in 0..batch_depth {
                    let submit_start = now_ms();
                    match submit(adapter) {
                        Ok(BrowserWebGpuMathDispatch::Immediate(response)) => {
                            capture_immediate(&response, &mut out);
                            case.capacity = response.selection().capacity().map(Into::into);
                        }
                        Ok(BrowserWebGpuMathDispatch::Submitted(current)) => {
                            case.capacity = current.selection().capacity().map(Into::into);
                            submitted.push(current);
                        }
                        Err(current) => {
                            error = Some(current);
                            break;
                        }
                    }
                    submit_samples.push(now_ms() - submit_start);
                }
                if error.is_some() {
                    break;
                }
                yield_to_browser().await;
                for current in submitted {
                    let readback_start = now_ms();
                    resize_out_for_submitted(&mut out, current.len());
                    match adapter.read_submitted_values_into(current, &mut out).await {
                        Ok(_) => {}
                        Err(current) => {
                            error = Some(current);
                            break;
                        }
                    }
                    readback_samples.push(now_ms() - readback_start);
                }
                if error.is_some() {
                    break;
                }
                samples.push((now_ms() - total_start) / batch_depth as f64);
            }
        }
        case = finish_gpu_case(case, error, samples, adapter.stats(), expected, &out);
        fill_breakdown(&mut case, submit_samples, readback_samples);
        case
    }

    async fn run_auto_resident_pipelined_case(
        config: &BrowserMathBenchConfig,
        mode: BrowserBenchMode,
        mut case: BrowserMathBenchCase,
        adapter: &mut BrowserWebGpuAutoMathAdapter,
        expected: &[f32],
        request: BrowserWebGpuMathRequest<'_>,
        capture_immediate: fn(&BrowserWebGpuMathResponse, &mut Vec<f32>),
    ) -> BrowserMathBenchCase {
        adapter.reset_stats();
        let dispatch = match adapter.prepare_resident(request) {
            Ok(dispatch) => dispatch,
            Err(error) => {
                return finish_gpu_case(
                    case,
                    Some(error),
                    Vec::new(),
                    adapter.stats(),
                    expected,
                    &[],
                );
            }
        };
        case.capacity = dispatch.selection().capacity().map(Into::into);
        match dispatch {
            BrowserWebGpuPreparedMathDispatch::Cpu(_) => {
                let mut out = Vec::new();
                let mut samples = Vec::with_capacity(config.sample_iters);
                let mut error = None;
                for _ in 0..config.warmup_iters {
                    match adapter.dispatch(request).await {
                        Ok(response) => capture_immediate(&response, &mut out),
                        Err(current) => {
                            error = Some(current);
                            break;
                        }
                    }
                }
                if error.is_none() {
                    for _ in 0..config.sample_iters {
                        let start = now_ms();
                        match adapter.dispatch(request).await {
                            Ok(response) => capture_immediate(&response, &mut out),
                            Err(current) => {
                                error = Some(current);
                                break;
                            }
                        }
                        samples.push(now_ms() - start);
                    }
                }
                finish_gpu_case(case, error, samples, adapter.stats(), expected, &out)
            }
            BrowserWebGpuPreparedMathDispatch::Prepared(prepared) => {
                run_prepared_auto_resident_pipelined_case(
                    config, mode, case, adapter, expected, &prepared,
                )
                .await
            }
        }
    }

    async fn run_auto_resident_direct_pipelined_case(
        config: &BrowserMathBenchConfig,
        mode: BrowserBenchMode,
        mut case: BrowserMathBenchCase,
        adapter: &mut BrowserWebGpuAutoMathAdapter,
        expected: &[f32],
        request: BrowserWebGpuMathRequest<'_>,
    ) -> BrowserMathBenchCase {
        adapter.reset_stats();
        let dispatch = match adapter.prepare_resident(request) {
            Ok(dispatch) => dispatch,
            Err(error) => {
                return finish_gpu_case(
                    case,
                    Some(error),
                    Vec::new(),
                    adapter.stats(),
                    expected,
                    &[],
                );
            }
        };
        case.capacity = dispatch.selection().capacity().map(Into::into);
        match dispatch {
            BrowserWebGpuPreparedMathDispatch::Cpu(_) => {
                let mut out = Vec::new();
                let mut samples = Vec::with_capacity(config.sample_iters);
                let mut error = None;
                for _ in 0..config.warmup_iters {
                    match adapter.dispatch(request).await {
                        Ok(response) => capture_response(&response, &mut out),
                        Err(current) => {
                            error = Some(current);
                            break;
                        }
                    }
                }
                if error.is_none() {
                    for _ in 0..config.sample_iters {
                        let start = now_ms();
                        match adapter.dispatch(request).await {
                            Ok(response) => capture_response(&response, &mut out),
                            Err(current) => {
                                error = Some(current);
                                break;
                            }
                        }
                        samples.push(now_ms() - start);
                    }
                }
                finish_gpu_case(case, error, samples, adapter.stats(), expected, &out)
            }
            BrowserWebGpuPreparedMathDispatch::Prepared(prepared) => {
                run_prepared_auto_resident_direct_pipelined_case(
                    config, mode, case, adapter, expected, &prepared,
                )
                .await
            }
        }
    }

    async fn run_prepared_auto_resident_direct_pipelined_case(
        config: &BrowserMathBenchConfig,
        mode: BrowserBenchMode,
        mut case: BrowserMathBenchCase,
        adapter: &mut BrowserWebGpuAutoMathAdapter,
        expected: &[f32],
        prepared: &BrowserWebGpuPreparedMath,
    ) -> BrowserMathBenchCase {
        let batch_depth = async_batch_depth(mode, config);
        let mut out = vec![0.0; prepared.len()];
        let mut samples = Vec::with_capacity(config.sample_iters);
        let mut submit_samples = Vec::with_capacity(config.sample_iters);
        let mut readback_samples = Vec::with_capacity(config.sample_iters);
        let mut error = None;
        for _ in 0..config.warmup_iters {
            let mut submitted = Vec::with_capacity(batch_depth);
            for _ in 0..batch_depth {
                match submit_prepared_direct(adapter.context_mut(), prepared) {
                    Ok(current) => submitted.push(current),
                    Err(current) => {
                        error = Some(current);
                        break;
                    }
                }
            }
            yield_to_browser().await;
            for current in submitted {
                if let Err(current) = adapter
                    .context_mut()
                    .read_submitted_f32(current, &mut out)
                    .await
                {
                    error = Some(current);
                    break;
                }
            }
            if error.is_some() {
                break;
            }
        }
        if error.is_none() {
            for _ in 0..config.sample_iters {
                let total_start = now_ms();
                let mut submitted = Vec::with_capacity(batch_depth);
                for _ in 0..batch_depth {
                    let submit_start = now_ms();
                    match submit_prepared_direct(adapter.context_mut(), prepared) {
                        Ok(current) => submitted.push(current),
                        Err(current) => {
                            error = Some(current);
                            break;
                        }
                    }
                    submit_samples.push(now_ms() - submit_start);
                }
                if error.is_some() {
                    break;
                }
                yield_to_browser().await;
                for current in submitted {
                    let readback_start = now_ms();
                    if let Err(current) = adapter
                        .context_mut()
                        .read_submitted_f32(current, &mut out)
                        .await
                    {
                        error = Some(current);
                        break;
                    }
                    readback_samples.push(now_ms() - readback_start);
                }
                if error.is_some() {
                    break;
                }
                samples.push((now_ms() - total_start) / batch_depth as f64);
            }
        }
        case = finish_gpu_case(case, error, samples, adapter.stats(), expected, &out);
        fill_breakdown(&mut case, submit_samples, readback_samples);
        case
    }

    fn submit_prepared_direct(
        context: &mut BrowserWebGpuMathContext,
        prepared: &BrowserWebGpuPreparedMath,
    ) -> Result<BrowserSubmittedF32, BrowserWebGpuError> {
        match prepared {
            BrowserWebGpuPreparedMath::MatmulF32 {
                prepared,
                rows,
                cols,
                selection: _,
            } => context.submit_resident_matmul_f32(prepared, *rows, *cols),
            BrowserWebGpuPreparedMath::MatrixAddF32 {
                prepared,
                rows: _,
                cols: _,
                len,
                selection: _,
            }
            | BrowserWebGpuPreparedMath::TensorAddF32 {
                prepared,
                dims: _,
                len,
                selection: _,
            } => context.submit_resident_elementwise_f32(prepared, *len),
        }
    }

    async fn run_prepared_auto_resident_pipelined_case(
        config: &BrowserMathBenchConfig,
        mode: BrowserBenchMode,
        mut case: BrowserMathBenchCase,
        adapter: &mut BrowserWebGpuAutoMathAdapter,
        expected: &[f32],
        prepared: &BrowserWebGpuPreparedMath,
    ) -> BrowserMathBenchCase {
        let batch_depth = async_batch_depth(mode, config);
        let mut out = vec![0.0; prepared.len()];
        let mut samples = Vec::with_capacity(config.sample_iters);
        let mut submit_samples = Vec::with_capacity(config.sample_iters);
        let mut readback_samples = Vec::with_capacity(config.sample_iters);
        let mut error = None;
        for _ in 0..config.warmup_iters {
            let mut submitted = Vec::with_capacity(batch_depth);
            for _ in 0..batch_depth {
                match adapter.submit_prepared(prepared) {
                    Ok(current) => submitted.push(current),
                    Err(current) => {
                        error = Some(current);
                        break;
                    }
                }
            }
            yield_to_browser().await;
            for current in submitted {
                resize_out_for_submitted(&mut out, current.len());
                if let Err(current) = adapter.read_submitted_values_into(current, &mut out).await {
                    error = Some(current);
                    break;
                }
            }
            if error.is_some() {
                break;
            }
        }
        if error.is_none() {
            for _ in 0..config.sample_iters {
                let total_start = now_ms();
                let mut submitted = Vec::with_capacity(batch_depth);
                for _ in 0..batch_depth {
                    let submit_start = now_ms();
                    match adapter.submit_prepared(prepared) {
                        Ok(current) => submitted.push(current),
                        Err(current) => {
                            error = Some(current);
                            break;
                        }
                    }
                    submit_samples.push(now_ms() - submit_start);
                }
                if error.is_some() {
                    break;
                }
                yield_to_browser().await;
                for current in submitted {
                    let readback_start = now_ms();
                    resize_out_for_submitted(&mut out, current.len());
                    if let Err(current) =
                        adapter.read_submitted_values_into(current, &mut out).await
                    {
                        error = Some(current);
                        break;
                    }
                    readback_samples.push(now_ms() - readback_start);
                }
                if error.is_some() {
                    break;
                }
                samples.push((now_ms() - total_start) / batch_depth as f64);
            }
        }
        case = finish_gpu_case(case, error, samples, adapter.stats(), expected, &out);
        fill_breakdown(&mut case, submit_samples, readback_samples);
        case
    }

    fn resize_out_for_submitted(out: &mut Vec<f32>, len: usize) {
        if out.len() != len {
            out.resize(len, 0.0);
        }
    }

    fn capture_tensor_response(response: &BrowserWebGpuMathResponse, out: &mut Vec<f32>) {
        if let Some(tensor) = response.tensor_f32() {
            out.clear();
            out.extend_from_slice(tensor.values());
        }
    }

    fn capture_matrix_response(response: &BrowserWebGpuMathResponse, out: &mut Vec<f32>) {
        if let Some(matrix) = response.matrix_f32() {
            out.clear();
            out.extend_from_slice(matrix.values());
        }
    }

    fn capture_response(response: &BrowserWebGpuMathResponse, out: &mut Vec<f32>) {
        capture_tensor_response(response, out);
        capture_matrix_response(response, out);
    }

    fn async_batch_depth(mode: BrowserBenchMode, config: &BrowserMathBenchConfig) -> usize {
        if matches!(
            mode,
            BrowserBenchMode::AutoPipelined
                | BrowserBenchMode::AutoResidentPipelined
                | BrowserBenchMode::AutoResidentDirectPipelined
                | BrowserBenchMode::WebGpuPreparedResidentPipelined
                | BrowserBenchMode::WebGpuPreparedCapacityResidentPipelined
                | BrowserBenchMode::WebGpuPreparedResidentSubmitOnlyPipelined
                | BrowserBenchMode::WebGpuPreparedCapacityResidentSubmitOnlyPipelined
                | BrowserBenchMode::WebGpuPreparedResidentDispatchOnlyPipelined
                | BrowserBenchMode::WebGpuPreparedCapacityResidentDispatchOnlyPipelined
                | BrowserBenchMode::WebGpuPreparedResidentChainedDispatchOnlyPipelined
                | BrowserBenchMode::WebGpuPreparedResidentMatmulBiasDispatchOnlyPipelined
        ) {
            config.async_batch_depth.max(1)
        } else {
            1
        }
    }

    fn elementwise_capacity_len(mode: BrowserBenchMode, len: usize) -> usize {
        if uses_overcapacity(mode) {
            overcapacity_len(len)
        } else {
            len
        }
    }

    fn matmul_capacity(mode: BrowserBenchMode, shape: MatmulShape) -> BrowserMatmulCapacity {
        if uses_overcapacity(mode) {
            BrowserMatmulCapacity {
                rows: overcapacity_len(shape.rows),
                shared: overcapacity_len(shape.shared),
                cols: overcapacity_len(shape.cols),
            }
        } else {
            BrowserMatmulCapacity {
                rows: shape.rows,
                shared: shape.shared,
                cols: shape.cols,
            }
        }
    }

    const fn uses_overcapacity(mode: BrowserBenchMode) -> bool {
        matches!(
            mode,
            BrowserBenchMode::WebGpuPreparedCapacityResident
                | BrowserBenchMode::WebGpuPreparedCapacityResidentPipelined
                | BrowserBenchMode::WebGpuPreparedCapacityResidentSubmitOnlyPipelined
                | BrowserBenchMode::WebGpuPreparedCapacityResidentDispatchOnlyPipelined
        )
    }

    fn overcapacity_len(len: usize) -> usize {
        BrowserWebGpuCapacityGrowth::Double.grow(len)
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
        case.effective_gflops = effective_gflops(case.estimated_flops, median);
        case.submit_median_share = median_share(case.submit_median_ms, case.median_ms);
        case.readback_median_share = median_share(case.readback_median_ms, case.median_ms);
    }

    fn effective_gflops(estimated_flops: u64, median_ms: f64) -> Option<f64> {
        if estimated_flops == 0 || median_ms <= 0.0 {
            return None;
        }
        Some(estimated_flops as f64 / (median_ms * 1_000_000.0))
    }

    fn median_share(part: Option<f64>, whole: Option<f64>) -> Option<f64> {
        let part = part?;
        let whole = whole?;
        if whole <= 0.0 {
            return None;
        }
        Some(part / whole)
    }

    fn estimated_workgroups(op: &str, shape: &BrowserMathBenchShape) -> usize {
        match (op, shape) {
            ("tensor_add_f32", BrowserMathBenchShape::Len { len }) => len.div_ceil(256),
            ("matmul_f32", BrowserMathBenchShape::Matmul { rows, cols, .. }) => {
                rows.div_ceil(16) * cols.div_ceil(16)
            }
            _ => 0,
        }
    }

    fn estimated_work_items(op: &str, shape: &BrowserMathBenchShape) -> usize {
        match (op, shape) {
            ("tensor_add_f32", BrowserMathBenchShape::Len { len }) => {
                estimated_workgroups(op, shape) * 256.min((*len).max(1))
            }
            ("matmul_f32", BrowserMathBenchShape::Matmul { .. }) => {
                estimated_workgroups(op, shape) * 16 * 16
            }
            _ => 0,
        }
    }

    fn estimated_flops(op: &str, shape: &BrowserMathBenchShape) -> u64 {
        match (op, shape) {
            ("tensor_add_f32", BrowserMathBenchShape::Len { len }) => *len as u64,
            ("matmul_f32", BrowserMathBenchShape::Matmul { rows, shared, cols }) => {
                2 * *rows as u64 * *shared as u64 * *cols as u64
            }
            _ => 0,
        }
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

    async fn yield_to_browser() {
        let _ = JsFuture::from(js_sys::Promise::resolve(&JsValue::NULL)).await;
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
        let cases = vec![bench_case(
            "tensor_add_f32_len256_capacity",
            "tensor_add_f32",
            BrowserMathBenchShape::Len { len: 256 },
            Some(BrowserMathBenchCapacity::Len { len: 512 }),
            BrowserBenchMode::WebGpuPreparedCapacityResident,
            Some(0.25),
            true,
        )];
        let recommendations = browser_math_bench_recommendations(&cases, None);
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
            stability: browser_math_bench_stability(&cases),
            cases,
            recommendations,
            skips: vec![BrowserMathBenchSkip {
                scope: "webgpu",
                reason: "navigator_gpu_missing".to_owned(),
            }],
        };

        let json = serde_json::to_string(&report).expect("report serializes");

        assert!(json.contains("arcweft.browser_webgpu_bench.v1"));
        assert!(json.contains("\"capacity\""));
        assert!(json.contains("\"len\":512"));
        assert!(json.contains("\"effective_gflops\""));
        assert!(json.contains("\"submit_median_share\""));
        assert!(json.contains("\"readback_median_share\""));
        assert!(json.contains("\"recommendations\""));
        assert!(json.contains("\"stability\""));
        assert!(json.contains("\"round_index\""));
        assert!(json.contains("\"mode_order_index\""));
        assert!(!json.contains("\\\\"));
        assert!(!json.contains("D:"));
    }

    #[test]
    fn stability_groups_repeated_round_medians() {
        let shape = BrowserMathBenchShape::Matmul {
            rows: 256,
            shared: 256,
            cols: 256,
        };
        let cases = vec![
            bench_case(
                "matmul_round0",
                "matmul_f32",
                shape,
                None,
                BrowserBenchMode::WebGpuPreparedResidentPipelined,
                Some(1.0),
                true,
            ),
            bench_case(
                "matmul_round1",
                "matmul_f32",
                shape,
                None,
                BrowserBenchMode::WebGpuPreparedResidentPipelined,
                Some(1.5),
                true,
            ),
            bench_case(
                "matmul_wrong",
                "matmul_f32",
                shape,
                None,
                BrowserBenchMode::WebGpuPreparedResidentPipelined,
                Some(0.5),
                false,
            ),
        ];

        let stability = browser_math_bench_stability(&cases);

        assert_eq!(stability.len(), 1);
        assert_eq!(stability[0].measured_rounds, 2);
        assert_eq!(stability[0].median_of_medians_ms, Some(1.5));
        assert_eq!(stability[0].min_median_ms, Some(1.0));
        assert_eq!(stability[0].max_median_ms, Some(1.5));
        assert_eq!(stability[0].median_mad_ms, Some(0.5));
        assert_eq!(stability[0].spread_ratio, Some(1.5));
    }

    #[test]
    fn recommendations_select_fastest_correct_gpu_case_with_capacity() {
        let shape = BrowserMathBenchShape::Matmul {
            rows: 256,
            shared: 256,
            cols: 256,
        };
        let cases = vec![
            bench_case(
                "matmul_cpu",
                "matmul_f32",
                shape,
                None,
                BrowserBenchMode::CpuWasm,
                Some(8.0),
                true,
            ),
            bench_case(
                "matmul_exact",
                "matmul_f32",
                shape,
                Some(BrowserMathBenchCapacity::Matmul {
                    rows: 256,
                    shared: 256,
                    cols: 256,
                }),
                BrowserBenchMode::WebGpuPreparedResidentPipelined,
                Some(1.5),
                true,
            ),
            bench_case(
                "matmul_capacity",
                "matmul_f32",
                shape,
                Some(BrowserMathBenchCapacity::Matmul {
                    rows: 512,
                    shared: 512,
                    cols: 512,
                }),
                BrowserBenchMode::WebGpuPreparedCapacityResidentPipelined,
                Some(1.0),
                true,
            ),
            bench_case(
                "matmul_wrong",
                "matmul_f32",
                shape,
                None,
                BrowserBenchMode::WebGpuOneShot,
                Some(0.5),
                false,
            ),
        ];

        let recommendations = browser_math_bench_recommendations(&cases, Some(large_limits()));

        assert_eq!(recommendations.len(), 1);
        let recommendation = &recommendations[0];
        assert_eq!(
            recommendation.reason,
            BrowserMathBenchRecommendationReason::WebGpuFaster
        );
        assert_eq!(
            recommendation.selected_mode,
            Some(BrowserBenchMode::WebGpuPreparedCapacityResidentPipelined)
        );
        assert_eq!(
            recommendation.selected_capacity,
            Some(BrowserMathBenchCapacity::Matmul {
                rows: 512,
                shared: 512,
                cols: 512,
            })
        );
        assert_eq!(
            recommendation.policy_mode,
            Some(BrowserBenchMode::WebGpuPreparedResidentPipelined)
        );
        assert_eq!(
            recommendation.policy_capacity,
            Some(BrowserMathBenchCapacity::Matmul {
                rows: 256,
                shared: 256,
                cols: 256,
            })
        );
        assert_eq!(
            recommendation.policy_reason,
            Some(BrowserMathBenchPolicyReason::MatmulPreparedResidentPipelined)
        );
        assert_eq!(recommendation.policy_matches_selected, Some(false));
        assert_eq!(recommendation.speedup, Some(8.0));
    }

    #[test]
    fn recommendations_use_median_of_repeated_mode_medians() {
        let shape = BrowserMathBenchShape::Matmul {
            rows: 256,
            shared: 256,
            cols: 256,
        };
        let exact = Some(BrowserMathBenchCapacity::Matmul {
            rows: 256,
            shared: 256,
            cols: 256,
        });
        let capacity = Some(BrowserMathBenchCapacity::Matmul {
            rows: 512,
            shared: 512,
            cols: 512,
        });
        let cases = vec![
            bench_case(
                "matmul_cpu_round0",
                "matmul_f32",
                shape,
                None,
                BrowserBenchMode::CpuWasm,
                Some(8.0),
                true,
            ),
            bench_case(
                "matmul_cpu_round1",
                "matmul_f32",
                shape,
                None,
                BrowserBenchMode::CpuWasm,
                Some(8.2),
                true,
            ),
            bench_case(
                "matmul_exact_fast_outlier",
                "matmul_f32",
                shape,
                exact,
                BrowserBenchMode::WebGpuPreparedResidentPipelined,
                Some(0.5),
                true,
            ),
            bench_case(
                "matmul_exact_slow_round",
                "matmul_f32",
                shape,
                exact,
                BrowserBenchMode::WebGpuPreparedResidentPipelined,
                Some(4.0),
                true,
            ),
            bench_case(
                "matmul_capacity_round0",
                "matmul_f32",
                shape,
                capacity,
                BrowserBenchMode::WebGpuPreparedCapacityResidentPipelined,
                Some(1.5),
                true,
            ),
            bench_case(
                "matmul_capacity_round1",
                "matmul_f32",
                shape,
                capacity,
                BrowserBenchMode::WebGpuPreparedCapacityResidentPipelined,
                Some(1.6),
                true,
            ),
        ];

        let recommendations = browser_math_bench_recommendations(&cases, Some(large_limits()));

        assert_eq!(recommendations.len(), 1);
        let recommendation = &recommendations[0];
        assert_eq!(
            recommendation.selected_mode,
            Some(BrowserBenchMode::WebGpuPreparedCapacityResidentPipelined)
        );
        assert_eq!(recommendation.selected_capacity, capacity);
        assert_eq!(recommendation.selected_median_ms, Some(1.6));
        assert_eq!(recommendation.selected_mad_ms, Some(1.6));
        assert_eq!(recommendation.selected_p95_ms, Some(1.6));
        assert_eq!(recommendation.cpu_median_ms, Some(8.2));
        assert_eq!(recommendation.cpu_mad_ms, Some(8.2));
        assert_eq!(recommendation.cpu_p95_ms, Some(8.2));
        assert_eq!(recommendation.speedup, Some(8.2 / 1.6));
    }

    #[test]
    fn recommendations_treat_auto_as_policy_observation_not_candidate() {
        let shape = BrowserMathBenchShape::Matmul {
            rows: 128,
            shared: 128,
            cols: 128,
        };
        let cases = vec![
            bench_case(
                "matmul_cpu",
                "matmul_f32",
                shape,
                None,
                BrowserBenchMode::CpuWasm,
                Some(4.0),
                true,
            ),
            bench_case(
                "matmul_auto",
                "matmul_f32",
                shape,
                Some(BrowserMathBenchCapacity::Matmul {
                    rows: 128,
                    shared: 128,
                    cols: 128,
                }),
                BrowserBenchMode::Auto,
                Some(0.5),
                true,
            ),
            bench_case(
                "matmul_auto_pipelined",
                "matmul_f32",
                shape,
                Some(BrowserMathBenchCapacity::Matmul {
                    rows: 128,
                    shared: 128,
                    cols: 128,
                }),
                BrowserBenchMode::AutoPipelined,
                Some(0.4),
                true,
            ),
            bench_case(
                "matmul_auto_resident",
                "matmul_f32",
                shape,
                Some(BrowserMathBenchCapacity::Matmul {
                    rows: 128,
                    shared: 128,
                    cols: 128,
                }),
                BrowserBenchMode::AutoResidentPipelined,
                Some(0.3),
                true,
            ),
            bench_case(
                "matmul_auto_resident_direct",
                "matmul_f32",
                shape,
                Some(BrowserMathBenchCapacity::Matmul {
                    rows: 128,
                    shared: 128,
                    cols: 128,
                }),
                BrowserBenchMode::AutoResidentDirectPipelined,
                Some(0.2),
                true,
            ),
        ];

        let recommendations = browser_math_bench_recommendations(&cases, Some(large_limits()));

        assert_eq!(recommendations.len(), 1);
        assert_eq!(
            recommendations[0].reason,
            BrowserMathBenchRecommendationReason::NoMeasuredWebGpuCase
        );
        assert_eq!(
            recommendations[0].selected_mode,
            Some(BrowserBenchMode::CpuWasm)
        );
        assert_eq!(
            recommendations[0].policy_mode,
            Some(BrowserBenchMode::WebGpuPreparedResidentPipelined)
        );
        assert_eq!(recommendations[0].policy_matches_selected, Some(false));
    }

    #[test]
    fn recommendations_keep_cpu_when_gpu_is_not_faster() {
        let shape = BrowserMathBenchShape::Len { len: 65_536 };
        let cases = vec![
            bench_case(
                "add_cpu",
                "tensor_add_f32",
                shape,
                None,
                BrowserBenchMode::CpuWasm,
                Some(0.2),
                true,
            ),
            bench_case(
                "add_gpu",
                "tensor_add_f32",
                shape,
                Some(BrowserMathBenchCapacity::Len { len: 131_072 }),
                BrowserBenchMode::WebGpuPreparedCapacityResidentPipelined,
                Some(0.8),
                true,
            ),
        ];

        let recommendations = browser_math_bench_recommendations(&cases, Some(large_limits()));

        assert_eq!(recommendations.len(), 1);
        assert_eq!(
            recommendations[0].reason,
            BrowserMathBenchRecommendationReason::CpuFasterOrEqual
        );
        assert_eq!(
            recommendations[0].selected_mode,
            Some(BrowserBenchMode::CpuWasm)
        );
        assert_eq!(
            recommendations[0].policy_mode,
            Some(BrowserBenchMode::CpuWasm)
        );
        assert_eq!(
            recommendations[0].policy_reason,
            Some(BrowserMathBenchPolicyReason::ElementwiseCpuReadbackDominated)
        );
        assert_eq!(recommendations[0].policy_matches_selected, Some(true));
        assert_eq!(recommendations[0].speedup, Some(1.0));
    }

    const fn large_limits() -> BrowserMathBenchLimits {
        BrowserMathBenchLimits {
            max_storage_buffer_binding_size: 1_u64 << 34,
            max_buffer_size: 1_u64 << 34,
            max_compute_invocations_per_workgroup: 256,
            max_compute_workgroups_per_dimension: 65_535,
        }
    }

    fn bench_case(
        case_id: &str,
        op: &'static str,
        shape: BrowserMathBenchShape,
        capacity: Option<BrowserMathBenchCapacity>,
        mode: BrowserBenchMode,
        median_ms: Option<f64>,
        passed: bool,
    ) -> BrowserMathBenchCase {
        BrowserMathBenchCase {
            case_id: case_id.to_owned(),
            op,
            shape,
            capacity,
            mode,
            round_index: 0,
            mode_order_index: 0,
            warmup_iters: 1,
            sample_iters: 1,
            median_ms,
            mad_ms: median_ms,
            min_ms: median_ms,
            p95_ms: median_ms,
            effective_gflops: median_ms
                .filter(|median| *median > 0.0)
                .map(|median| 256.0 / (median * 1_000_000.0)),
            submit_median_ms: None,
            readback_median_ms: None,
            submit_median_share: None,
            readback_median_share: None,
            bytes_uploaded: 0,
            bytes_readback: 0,
            dispatches: usize::from(median_ms.is_some()),
            async_submissions: 0,
            async_readbacks: 0,
            max_in_flight: 0,
            buffer_alloc_count: 0,
            buffer_reuse_count: 0,
            workgroups: 1,
            work_items: 256,
            estimated_flops: 256,
            correctness: BrowserMathBenchCorrectness {
                passed,
                max_abs: 0.0,
                max_rel: 0.0,
            },
            fallback_reason: None,
            checksum: 0.0,
        }
    }
}
