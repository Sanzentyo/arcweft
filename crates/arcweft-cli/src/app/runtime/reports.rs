use crate::output::{RuntimeExecutorTier, RuntimeProfilePhase};
use arcweft_bundle::BundleRuntimeSummary;
use arcweft_runtime_host::{
    BundleRunnerPhase, BundleRunnerStepSummary, NativeTaskStats, RuntimeExecutorStats,
};

#[derive(serde::Serialize)]
pub(in crate::app) struct BundleCommandReport {
    pub(in crate::app) bundle: String,
    pub(in crate::app) source: String,
    pub(in crate::app) required_host_calls: Vec<String>,
    pub(in crate::app) adapter_manifests: usize,
    pub(in crate::app) bytecode_instructions: usize,
    pub(in crate::app) virtual_files: usize,
    pub(in crate::app) image_assets: usize,
    pub(in crate::app) phases: Vec<RuntimeProfilePhase>,
    pub(in crate::app) runtime: BundleRuntimeSummary,
}

#[derive(serde::Serialize)]
pub(in crate::app) struct BundleRunReport {
    pub(in crate::app) bundle: String,
    pub(in crate::app) source: String,
    pub(in crate::app) bytecode_instructions: usize,
    pub(in crate::app) adapter_manifests: usize,
    pub(in crate::app) phases: Vec<BundleRunnerPhase>,
    pub(in crate::app) executor: RuntimeExecutorTier,
    pub(in crate::app) executor_stats: RuntimeExecutorStats,
    pub(in crate::app) native_io: NativeTaskStats,
    pub(in crate::app) steps: Vec<BundleRunnerStepSummary>,
    pub(in crate::app) final_status: String,
}
