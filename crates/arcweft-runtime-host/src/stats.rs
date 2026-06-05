use arcweft_runtime_accelerator::{
    RuntimePureAccelerator, RuntimePureBackendMode, RuntimePureCompileStats,
    RuntimePureWorkerCount,
    math::{RuntimeMathAutoSelectionReason, RuntimeMathBackend, RuntimeMathStats},
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize)]
pub struct RuntimeExecutorStats {
    pub aot_fast_path_ops: usize,
    pub pure_config: RuntimeExecutorPureConfigSummary,
    pub pure_acceleration: RuntimeExecutorPureAccelerationSummary,
    pub pure_compile: RuntimeExecutorPureCompileStatsSummary,
    pub math: RuntimeExecutorMathStatsSummary,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize)]
pub struct RuntimeExecutorPureConfigSummary {
    pub backend: &'static str,
    pub workers: RuntimeExecutorPureWorkerSummary,
    pub resolved_workers: usize,
    pub worker_pool_active: bool,
    pub batch_min_len: usize,
    pub emit_object_artifacts: bool,
    pub math_backend: &'static str,
    pub math_wgpu_min_elements: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeExecutorPureWorkerSummary {
    #[default]
    Auto,
    Fixed(usize),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize)]
pub struct RuntimeExecutorPureAccelerationSummary {
    pub annotated: usize,
    pub inferred: usize,
    pub jit: usize,
    pub aot: usize,
    pub vm: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize)]
pub struct RuntimeExecutorPureCompileStatsSummary {
    pub jit_attempts: usize,
    pub jit_successes: usize,
    pub jit_failures: usize,
    pub aot_attempts: usize,
    pub aot_successes: usize,
    pub aot_failures: usize,
    pub auto_jit_selected: usize,
    pub auto_aot_selected: usize,
    pub auto_vm_selected: usize,
    pub auto_jit_deferred: usize,
    pub auto_jit_promotions: usize,
    pub auto_jit_skipped_small: usize,
    pub cache_hits: usize,
    pub cache_misses: usize,
    pub object_attempts: usize,
    pub object_successes: usize,
    pub object_failures: usize,
    pub object_bytes: usize,
    pub compile_elapsed_ns: u128,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize)]
pub struct RuntimeExecutorMathStatsSummary {
    pub scalar_calls: usize,
    pub glam_calls: usize,
    pub ndarray_calls: usize,
    pub wgpu_calls: usize,
    pub fallback_calls: usize,
    pub bytes_borrowed: usize,
    pub bytes_copied: usize,
    pub bytes_uploaded: usize,
    pub bytes_downloaded: usize,
    pub gpu_buffer_creations: usize,
    pub gpu_buffer_reuse_hits: usize,
    pub gpu_staging_buffer_creations: usize,
    pub gpu_staging_buffer_reuse_hits: usize,
    pub gpu_reused_dispatches: usize,
    pub last_backend: Option<&'static str>,
    pub last_auto_reason: Option<&'static str>,
}

pub fn runtime_executor_stats(
    aot_fast_path_ops: usize,
    pure: &RuntimePureAccelerator,
) -> RuntimeExecutorStats {
    let config = pure.config();
    let summary = pure.summary();
    RuntimeExecutorStats {
        aot_fast_path_ops,
        pure_config: RuntimeExecutorPureConfigSummary {
            backend: runtime_pure_backend_label(config.backend),
            workers: match config.workers {
                RuntimePureWorkerCount::Auto => RuntimeExecutorPureWorkerSummary::Auto,
                RuntimePureWorkerCount::Fixed(value) => {
                    RuntimeExecutorPureWorkerSummary::Fixed(value)
                }
            },
            resolved_workers: pure.resolved_worker_count(),
            worker_pool_active: pure.has_worker_pool(),
            batch_min_len: config.batch_min_len,
            emit_object_artifacts: config.emit_object_artifacts,
            math_backend: runtime_math_backend_label(config.math.backend),
            math_wgpu_min_elements: config.math.wgpu_min_elements,
        },
        pure_acceleration: RuntimeExecutorPureAccelerationSummary {
            annotated: summary.annotated,
            inferred: summary.inferred,
            jit: summary.jit,
            aot: summary.aot,
            vm: summary.vm,
        },
        pure_compile: RuntimeExecutorPureCompileStatsSummary::from(pure.compile_stats()),
        math: RuntimeExecutorMathStatsSummary::from(pure.math_stats()),
    }
}

fn runtime_pure_backend_label(value: RuntimePureBackendMode) -> &'static str {
    match value {
        RuntimePureBackendMode::Auto => "auto",
        RuntimePureBackendMode::Vm => "vm",
        RuntimePureBackendMode::Aot => "aot",
        RuntimePureBackendMode::Jit => "jit",
    }
}

fn runtime_math_backend_label(value: RuntimeMathBackend) -> &'static str {
    match value {
        RuntimeMathBackend::Auto => "auto",
        RuntimeMathBackend::Scalar => "scalar",
        RuntimeMathBackend::Glam => "glam",
        RuntimeMathBackend::Ndarray => "ndarray",
        RuntimeMathBackend::Wgpu => "wgpu",
    }
}

fn runtime_math_auto_reason_label(value: RuntimeMathAutoSelectionReason) -> &'static str {
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

impl From<RuntimePureCompileStats> for RuntimeExecutorPureCompileStatsSummary {
    fn from(stats: RuntimePureCompileStats) -> Self {
        Self {
            jit_attempts: stats.jit_attempts,
            jit_successes: stats.jit_successes,
            jit_failures: stats.jit_failures,
            aot_attempts: stats.aot_attempts,
            aot_successes: stats.aot_successes,
            aot_failures: stats.aot_failures,
            auto_jit_selected: stats.auto_jit_selected,
            auto_aot_selected: stats.auto_aot_selected,
            auto_vm_selected: stats.auto_vm_selected,
            auto_jit_deferred: stats.auto_jit_deferred,
            auto_jit_promotions: stats.auto_jit_promotions,
            auto_jit_skipped_small: stats.auto_jit_skipped_small,
            cache_hits: stats.cache_hits,
            cache_misses: stats.cache_misses,
            object_attempts: stats.object_attempts,
            object_successes: stats.object_successes,
            object_failures: stats.object_failures,
            object_bytes: stats.object_bytes,
            compile_elapsed_ns: stats.compile_elapsed_ns,
        }
    }
}

impl From<RuntimeMathStats> for RuntimeExecutorMathStatsSummary {
    fn from(stats: RuntimeMathStats) -> Self {
        Self {
            scalar_calls: stats.scalar_calls,
            glam_calls: stats.glam_calls,
            ndarray_calls: stats.ndarray_calls,
            wgpu_calls: stats.wgpu_calls,
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
            last_backend: stats.last_backend.map(runtime_math_backend_label),
            last_auto_reason: stats.last_auto_reason.map(runtime_math_auto_reason_label),
        }
    }
}
