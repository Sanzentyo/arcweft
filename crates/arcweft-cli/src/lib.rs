//! Library entry points for embedding the Arcweft CLI runner.

mod app;
mod output;
mod server_adapter;
mod toolchain_profile;

pub use app::{run, run_with_native_adapters};
pub use arcweft_runtime_host::{
    BundleRunnerError, BundleRunnerExecutor, BundleRunnerOptions, BundleRunnerPhase,
    BundleRunnerReport, BundleRunnerStepMode, BundleRunnerStepSummary, HostSystemInfo,
    NativeAdapterRegistrar, NativeSchedulerStats, NativeTaskClassCounts, NativeTaskStats,
    RuntimeBinding, RuntimeExecutorMathStatsSummary, RuntimeExecutorPureAccelerationSummary,
    RuntimeExecutorPureCompileStatsSummary, RuntimeExecutorPureConfigSummary,
    RuntimeExecutorPureWorkerSummary, RuntimeExecutorStats, RuntimePureAcceleratorConfig,
    run_bundle_file_with_native_adapters, run_bundle_with_native_adapters,
};

pub(crate) use app::print_json;
