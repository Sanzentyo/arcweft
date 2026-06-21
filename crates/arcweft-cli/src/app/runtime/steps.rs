use super::executor::RuntimeExecutorInstance;
use super::options::{CliRuntimeExecutorTier, CliRuntimeStepMode, RuntimeRunOptions};
use super::parse::step_options;
use crate::output::RuntimeStepRunSummary;
use arcweft_core::engine::FlowFiberStatus;
use arcweft_core::plan::RuntimePlan;
use arcweft_core::step::RuntimeStepInput;
use arcweft_core::value::RuntimeBinding;
use arcweft_host_adapter::HostCallPolicy;
use arcweft_runtime_accelerator::RuntimePureAcceleratorConfig;
use arcweft_runtime_host::{
    NativeAdapterRegistrar, NativeTaskBridge, NativeTaskStats, RuntimeExecutorStats,
};
use std::path::Path;
use std::process::ExitCode;

pub(in crate::app) fn run_runtime_steps(
    plan: RuntimePlan,
    source_path: Option<&Path>,
    config: RuntimeStepRunConfig,
    host_policy: &HostCallPolicy,
    adapter_registrars: &[NativeAdapterRegistrar],
    values: &[RuntimeBinding],
) -> Result<RuntimeRunTrace, ExitCode> {
    let mut executor = RuntimeExecutorInstance::new(plan, config.executor, config.pure_config);
    run_runtime_steps_with_executor(
        &mut executor,
        NativeRunHost {
            source_path,
            policy: host_policy,
            adapter_registrars,
        },
        config.steps,
        config.mode,
        config.max_ops,
        values,
    )
}

pub(in crate::app) fn run_runtime_steps_with_executor(
    executor: &mut RuntimeExecutorInstance,
    host_config: NativeRunHost<'_>,
    steps: usize,
    mode: CliRuntimeStepMode,
    max_ops: usize,
    values: &[RuntimeBinding],
) -> Result<RuntimeRunTrace, ExitCode> {
    try_run_runtime_steps_with_executor(executor, host_config, steps, mode, max_ops, values)
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
    values: &[RuntimeBinding],
) -> Result<RuntimeRunTrace, arcweft_host_adapter::HostAdapterError> {
    let mut host = host_config
        .source_path
        .map(|path| {
            NativeTaskBridge::try_new(
                path,
                host_config.policy.clone(),
                host_config.adapter_registrars,
            )
        })
        .transpose()?;
    let mut task_events = Vec::new();
    let mut summaries = Vec::new();
    for step_index in 0..steps {
        let result = executor.step_with_root_bindings(
            RuntimeStepInput {
                task_events: std::mem::take(&mut task_events),
                ..RuntimeStepInput::default()
            },
            values,
            step_options(mode, max_ops),
        );
        let (summary, task_requests) = RuntimeStepRunSummary::from_result_and_task_requests(
            step_index,
            result,
            executor.fiber(),
        );
        let done = matches!(
            executor.fiber().status,
            FlowFiberStatus::Done(_) | FlowFiberStatus::Failed(_)
        );
        summaries.push(summary);
        if done {
            break;
        }
        if let Some(host) = host.as_mut() {
            task_events = host.complete_tasks(task_requests);
        }
    }
    Ok(RuntimeRunTrace {
        steps: summaries,
        final_status: executor.fiber().status.clone(),
        executor_stats: executor.executor_stats(),
        native_io: host
            .as_ref()
            .map_or_else(NativeTaskStats::default, NativeTaskBridge::stats),
    })
}

pub(in crate::app) struct RuntimeRunTrace {
    pub(in crate::app) steps: Vec<RuntimeStepRunSummary>,
    pub(in crate::app) final_status: FlowFiberStatus,
    pub(in crate::app) executor_stats: RuntimeExecutorStats,
    pub(in crate::app) native_io: NativeTaskStats,
}

#[derive(Clone, Copy)]
pub(in crate::app) struct NativeRunHost<'a> {
    pub(in crate::app) source_path: Option<&'a Path>,
    pub(in crate::app) policy: &'a HostCallPolicy,
    pub(in crate::app) adapter_registrars: &'a [NativeAdapterRegistrar],
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
