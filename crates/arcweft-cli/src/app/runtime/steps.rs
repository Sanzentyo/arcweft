use super::executor::RuntimeExecutorInstance;
use super::options::{CliRuntimeExecutorTier, CliRuntimeStepMode, RuntimeRunOptions};
use super::parse::step_options;
use crate::output::RuntimeStepRunSummary;
use arcweft_bundle::{ArcweftBundle, BundleArtifactIdentity};
use arcweft_compiler::runtime_diagnostics::ExecutionDiagnosticContext;
use arcweft_core::engine::FlowFiberStatus;
use arcweft_core::plan::{EntryRuntimeId, RuntimeFlowInvocation, RuntimePlan};
use arcweft_core::step::RuntimeStepInput;
use arcweft_core::task::{GenerationId, LogicalEpoch};
use arcweft_host_adapter::HostCallPolicy;
use arcweft_runtime_accelerator::RuntimePureAcceleratorConfig;
use arcweft_runtime_host::native_task::standard_cli_registry_builder;
use arcweft_runtime_host::{
    BundleAssetAdapter, NativeAdapterRegistrar, NativeFileRoots, NativeTaskBridge, NativeTaskStats,
    RuntimeExecutorStats,
};
use std::path::Path;
use std::process::ExitCode;
use thiserror::Error;

pub(in crate::app) fn run_runtime_steps(
    plan: RuntimePlan,
    entry: &EntryRuntimeId,
    host_config: NativeRunHost<'_>,
    config: RuntimeStepRunConfig,
    execution_diagnostics: &ExecutionDiagnosticContext,
) -> Result<RuntimeRunTrace, ExitCode> {
    let mut executor =
        RuntimeExecutorInstance::new(plan, entry, config.executor, config.pure_config).map_err(
            |error| {
                eprintln!(
                    "error: failed to start entry `{}`: {error}",
                    entry.public_label()
                );
                ExitCode::FAILURE
            },
        )?;
    run_runtime_steps_with_executor(
        &mut executor,
        host_config,
        config.steps,
        config.mode,
        config.max_ops,
        execution_diagnostics,
    )
}

pub(in crate::app) fn run_runtime_flow_steps(
    invocation: RuntimeFlowInvocation,
    host_config: NativeRunHost<'_>,
    config: RuntimeStepRunConfig,
    execution_diagnostics: &ExecutionDiagnosticContext,
) -> Result<RuntimeRunTrace, ExitCode> {
    let mut executor = RuntimeExecutorInstance::from_flow_invocation(
        invocation,
        config.executor,
        config.pure_config,
    )
    .map_err(|error| {
        eprintln!("error: failed to start explicit Flow invocation: {error}");
        ExitCode::FAILURE
    })?;
    run_runtime_steps_with_executor(
        &mut executor,
        host_config,
        config.steps,
        config.mode,
        config.max_ops,
        execution_diagnostics,
    )
}

pub(in crate::app) fn run_runtime_steps_with_executor(
    executor: &mut RuntimeExecutorInstance,
    host_config: NativeRunHost<'_>,
    steps: usize,
    mode: CliRuntimeStepMode,
    max_ops: usize,
    execution_diagnostics: &ExecutionDiagnosticContext,
) -> Result<RuntimeRunTrace, ExitCode> {
    try_run_runtime_steps_with_executor(
        executor,
        host_config,
        steps,
        mode,
        max_ops,
        execution_diagnostics,
    )
    .map_err(|error| {
        eprintln!("error: {error}");
        ExitCode::FAILURE
    })
}

fn try_run_runtime_steps_with_executor(
    executor: &mut RuntimeExecutorInstance,
    host_config: NativeRunHost<'_>,
    steps: usize,
    mode: CliRuntimeStepMode,
    max_ops: usize,
    execution_diagnostics: &ExecutionDiagnosticContext,
) -> Result<RuntimeRunTrace, RuntimeStepRunError> {
    let mut host = host_config
        .source
        .map(|source| build_native_task_bridge(source, host_config))
        .transpose()?;
    let mut task_events = Vec::new();
    let mut host_call_results = Vec::new();
    let mut summaries = Vec::new();
    let mut final_status = FlowFiberStatus::Running;
    for step_index in 0..steps {
        if let Some(host) = host.as_mut() {
            host.pump_main_thread()?;
            task_events.extend(host.poll_completions()?);
            host_call_results.extend(host.take_host_call_results());
        }
        let result = executor.step(
            RuntimeStepInput {
                task_events: std::mem::take(&mut task_events),
                host_call_results: std::mem::take(&mut host_call_results),
                ..RuntimeStepInput::default()
            },
            step_options(mode, max_ops),
        );
        let (summary, task_requests, host_call_requests, step_status) =
            RuntimeStepRunSummary::from_result_and_task_requests(
                step_index,
                result,
                executor.fiber(),
                execution_diagnostics,
            )?;
        let done = matches!(
            &step_status,
            FlowFiberStatus::Done(_) | FlowFiberStatus::Failed(_)
        );
        final_status = step_status;
        summaries.push(summary);
        if done {
            break;
        }
        if let Some(host) = host.as_mut() {
            task_events = host.complete_tasks_with_generation_and_bundle_asset_context(
                executor.program_owner(),
                GenerationId::new(0),
                LogicalEpoch(
                    u64::try_from(step_index)
                        .map_err(|_| RuntimeStepRunError::LogicalEpochOverflow)?,
                ),
                task_requests,
                host_config.bundle_assets.map(|assets| assets.context),
            )?;
            host_call_results = host.complete_host_calls(
                executor.program_owner(),
                LogicalEpoch(
                    u64::try_from(step_index)
                        .map_err(|_| RuntimeStepRunError::LogicalEpochOverflow)?,
                ),
                host_call_requests,
            );
        }
    }
    Ok(RuntimeRunTrace {
        steps: summaries,
        final_status,
        executor_stats: executor.executor_stats(),
        native_io: host
            .as_ref()
            .map_or_else(NativeTaskStats::default, NativeTaskBridge::stats),
    })
}

pub(in crate::app) fn build_native_task_bridge(
    source: NativeRunSource<'_>,
    host_config: NativeRunHost<'_>,
) -> Result<NativeTaskBridge, RuntimeStepRunError> {
    let builder = standard_cli_registry_builder(source.file_roots().clone(), host_config.cli_args)?;
    let builder = host_config
        .adapter_registrars
        .iter()
        .try_fold(builder, |builder, register| {
            register(source.path(), builder)
        })?;
    let builder = if let Some(assets) = host_config.bundle_assets {
        builder.register(BundleAssetAdapter::try_new(
            assets.context,
            assets.artifact_identity,
            std::sync::Arc::clone(assets.bundle),
        )?)?
    } else {
        builder
    };
    NativeTaskBridge::try_with_registry(host_config.policy.clone(), builder.build())
        .map_err(Into::into)
}

#[derive(Debug, Error)]
pub(in crate::app) enum RuntimeStepRunError {
    #[error(transparent)]
    Host(#[from] arcweft_host_adapter::HostAdapterError),
    #[error(transparent)]
    NativeTask(#[from] arcweft_runtime_host::native_task::NativeTaskBridgeError),
    #[error("fresh runtime assertion identity projection failed: {0}")]
    Assertion(#[from] arcweft_runtime_plan::assertion_identity::RuntimeAssertionProjectionError),
    #[error(transparent)]
    BundleAsset(#[from] arcweft_runtime_host::BundleAssetAdapterError),
    #[error("runtime step index does not fit a task logical epoch")]
    LogicalEpochOverflow,
}

pub(in crate::app) struct RuntimeRunTrace {
    pub(in crate::app) steps: Vec<RuntimeStepRunSummary>,
    pub(in crate::app) final_status: FlowFiberStatus,
    pub(in crate::app) executor_stats: RuntimeExecutorStats,
    pub(in crate::app) native_io: NativeTaskStats,
}

#[derive(Clone, Copy)]
pub(in crate::app) struct NativeRunHost<'a> {
    pub(in crate::app) source: Option<NativeRunSource<'a>>,
    pub(in crate::app) bundle_assets: Option<RuntimeBundleAssets<'a>>,
    pub(in crate::app) policy: &'a HostCallPolicy,
    pub(in crate::app) adapter_registrars: &'a [NativeAdapterRegistrar],
    pub(in crate::app) cli_args: &'a [String],
}

#[derive(Clone, Copy)]
pub(in crate::app) struct RuntimeBundleAssets<'a> {
    pub(in crate::app) bundle: &'a std::sync::Arc<ArcweftBundle>,
    pub(in crate::app) artifact_identity: BundleArtifactIdentity,
    pub(in crate::app) context: arcweft_core::value::RuntimeBundleAssetContext,
}

#[derive(Clone, Copy)]
pub(in crate::app) struct NativeRunSource<'a> {
    path: &'a Path,
    file_roots: &'a NativeFileRoots,
}

impl<'a> NativeRunSource<'a> {
    pub(in crate::app) const fn new(path: &'a Path, file_roots: &'a NativeFileRoots) -> Self {
        Self { path, file_roots }
    }

    pub(in crate::app) const fn path(self) -> &'a Path {
        self.path
    }

    pub(in crate::app) const fn file_roots(self) -> &'a NativeFileRoots {
        self.file_roots
    }
}

#[derive(Clone, Copy, Debug)]
pub(in crate::app) struct RuntimeStepRunConfig {
    pub(in crate::app) steps: usize,
    pub(in crate::app) mode: CliRuntimeStepMode,
    pub(in crate::app) max_ops: usize,
    pub(in crate::app) executor: CliRuntimeExecutorTier,
    pub(in crate::app) pure_config: RuntimePureAcceleratorConfig,
}

pub(in crate::app) fn runtime_step_run_config_from_run_options(
    options: &RuntimeRunOptions,
    pure_config: RuntimePureAcceleratorConfig,
) -> RuntimeStepRunConfig {
    RuntimeStepRunConfig {
        steps: options.steps,
        mode: options.mode,
        max_ops: options.max_ops,
        executor: options.executor,
        pure_config,
    }
}
