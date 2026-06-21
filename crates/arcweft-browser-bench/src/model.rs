//! Browser WebGPU math benchmark harness.

use arcweft_runtime_accelerator::math::browser_webgpu_policy::BrowserMatmulCapacity;
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
