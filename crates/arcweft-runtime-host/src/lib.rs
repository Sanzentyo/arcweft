//! Host-side runtime execution boundary for Arcweft bundles and native tasks.

pub mod bundle_runner;
pub mod capabilities;
pub mod native_system;
pub mod native_task;
pub mod stats;

pub use arcweft_core::value::RuntimeBinding;
pub use arcweft_runtime_accelerator::RuntimePureAcceleratorConfig;
pub use bundle_runner::{
    BundleRunnerError, BundleRunnerExecutor, BundleRunnerOptions, BundleRunnerPhase,
    BundleRunnerReport, BundleRunnerStepMode, BundleRunnerStepSummary,
    run_bundle_file_with_native_adapters, run_bundle_with_native_adapters,
};
pub use capabilities::{
    RuntimeHostCapabilities, RuntimeHostConformanceDiagnostic,
    RuntimeHostConformanceDiagnosticKind, RuntimeHostConformanceReport, RuntimeHostRunnerKind,
};
pub use native_system::{HostSystemInfo, host_system_info, system_info_value};
pub use native_task::{
    INTERNAL_SCHEDULER_ADAPTER_ID, NativeAdapterRegistrar, NativeSchedulerStats, NativeTaskBridge,
    NativeTaskClassCounts, NativeTaskStats, internal_scheduler_manifest,
};
pub use stats::{
    RuntimeExecutorMathStatsSummary, RuntimeExecutorPureAccelerationSummary,
    RuntimeExecutorPureCompileStatsSummary, RuntimeExecutorPureConfigSummary,
    RuntimeExecutorPureWorkerSummary, RuntimeExecutorStats, runtime_executor_stats,
};
