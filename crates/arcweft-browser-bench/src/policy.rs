use crate::model::{
    BrowserBenchMode, BrowserMathBenchCapacity, BrowserMathBenchLimits,
    BrowserMathBenchPolicyReason, BrowserMathBenchShape,
};
use arcweft_runtime_accelerator::math::browser_webgpu_policy::{
    BrowserWebGpuLimits, BrowserWebGpuMathAutoPolicy, BrowserWebGpuMathAutoReason,
    BrowserWebGpuMathMode,
};

pub(crate) fn browser_math_policy_selection(
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
pub(crate) struct BrowserMathBenchPolicySelection {
    pub(crate) mode: BrowserBenchMode,
    pub(crate) capacity: Option<BrowserMathBenchCapacity>,
    pub(crate) reason: BrowserMathBenchPolicyReason,
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
