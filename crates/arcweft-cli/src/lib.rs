//! Library entry points for embedding the Arcweft CLI runner.

mod app;
mod native_system;
mod native_task;
mod output;
mod server_adapter;
mod toolchain_profile;

pub use app::{
    BundleRunnerError, BundleRunnerExecutor, BundleRunnerOptions, BundleRunnerPhase,
    BundleRunnerReport, BundleRunnerStepMode, BundleRunnerStepSummary, run,
    run_bundle_file_with_native_adapters, run_bundle_with_native_adapters,
    run_with_native_adapters,
};
pub use native_task::{
    NativeAdapterRegistrar, NativeSchedulerStats, NativeTaskClassCounts, NativeTaskStats,
};
pub use output::{
    RuntimeExecutorMathStatsSummary, RuntimeExecutorPureAccelerationSummary,
    RuntimeExecutorPureCompileStatsSummary, RuntimeExecutorPureConfigSummary,
    RuntimeExecutorPureWorkerSummary, RuntimeExecutorStats,
};

pub(crate) use app::print_json;
pub use arcweft_core::value::RuntimeBinding;
pub use arcweft_runtime_accelerator::RuntimePureAcceleratorConfig;
