use super::executor::RuntimeExecutorCore;
use super::parse::step_options;
use super::steps::{
    NativeRunHost, NativeRunSource, RuntimeBundleAssets, RuntimeStepRunConfig, RuntimeStepRunError,
    build_native_task_bridge,
};
use arcweft_core::engine::{FlowFiberStatus, FlowStatusLabelStyle};
use arcweft_core::step::{
    RuntimePureCallStats, RuntimeStepInput, RuntimeStepResult, RuntimeStepStats,
};
use arcweft_core::task::{GenerationId, LogicalEpoch};
use arcweft_host_adapter::HostCallPolicy;
use arcweft_runtime_accelerator::RuntimePureAccelerator;
use arcweft_runtime_host::{
    NativeAdapterRegistrar, NativeTaskBridge, NativeTaskStats, RuntimeExecutorStats,
    runtime_executor_stats,
};

pub(in crate::app) fn run_runtime_bench_steps_with_pure(
    mut executor: RuntimeExecutorCore,
    source: Option<NativeRunSource<'_>>,
    bundle_assets: RuntimeBundleAssets<'_>,
    config: RuntimeStepRunConfig,
    host_policy: &HostCallPolicy,
    adapter_registrars: &[NativeAdapterRegistrar],
    pure: &mut RuntimePureAccelerator,
) -> Result<CompletedRuntimeBenchTrace, RuntimeBenchRunError> {
    let mut host: Option<NativeTaskBridge> = None;
    let mut task_events = Vec::new();
    let mut host_call_results = Vec::new();
    let mut totals = RuntimeBenchStepTotals::default();
    let mut final_status = FlowFiberStatus::Running;
    for step_index in 0..config.steps {
        if let Some(host) = host.as_mut() {
            host.pump_main_thread().map_err(RuntimeStepRunError::from)?;
            task_events.extend(host.poll_completions().map_err(RuntimeStepRunError::from)?);
            host_call_results.extend(host.take_host_call_results());
        }
        let result = executor.step(
            RuntimeStepInput {
                task_events: std::mem::take(&mut task_events),
                host_call_results: std::mem::take(&mut host_call_results),
                ..RuntimeStepInput::default()
            },
            step_options(config.mode, config.max_ops),
            pure,
        );
        let RuntimeStepResult {
            mut output,
            fiber_status,
            stats,
            ..
        } = result;
        let mut program_completions = std::mem::take(&mut output.requests.program_task_completions);
        let task_requests = std::mem::take(&mut output.requests.tasks);
        let host_call_requests = std::mem::take(&mut output.requests.host_calls);
        totals.push(&stats, task_requests.len(), output.diagnostics.len());
        if let Some(source) = source
            && (!task_requests.is_empty()
                || !host_call_requests.is_empty()
                || !program_completions.is_empty())
        {
            if host.is_none() {
                host = Some(build_native_task_bridge(
                    source,
                    NativeRunHost {
                        source: Some(source),
                        bundle_assets: Some(bundle_assets),
                        policy: host_policy,
                        adapter_registrars,
                        cli_args: &[],
                    },
                )?);
            }
            if let Some(host) = host.as_mut() {
                let epoch = LogicalEpoch(
                    u64::try_from(step_index)
                        .map_err(|_| RuntimeStepRunError::LogicalEpochOverflow)?,
                );
                task_events = host
                    .complete_tasks_with_generation_and_bundle_asset_context(
                        executor.program_owner(),
                        GenerationId::new(0),
                        epoch,
                        task_requests,
                        Some(bundle_assets.context),
                    )
                    .map_err(RuntimeStepRunError::from)?;
                task_events.extend(
                    host.complete_program_tasks(&mut program_completions)
                        .map_err(RuntimeStepRunError::from)?,
                );
                host_call_results =
                    host.complete_host_calls(executor.program_owner(), epoch, host_call_requests);
            }
        }
        match fiber_status {
            FlowFiberStatus::Done(_) => {
                return Ok(CompletedRuntimeBenchTrace {
                    totals,
                    executor_stats: runtime_executor_stats(executor.fast_path_ops(), pure),
                    native_io: host
                        .as_ref()
                        .map_or_else(NativeTaskStats::default, NativeTaskBridge::stats),
                });
            }
            FlowFiberStatus::Failed(reason) => {
                return Err(RuntimeBenchRunError::Failed {
                    steps_run: step_index + 1,
                    reason,
                });
            }
            status => final_status = status,
        }
    }
    Err(RuntimeBenchRunError::StepLimit {
        step_limit: config.steps,
        status: Box::new(final_status),
    })
}

#[derive(Debug, thiserror::Error)]
pub(in crate::app) enum RuntimeBenchRunError {
    #[error(transparent)]
    Step(#[from] RuntimeStepRunError),
    #[error("runtime failed after {steps_run} step(s): {reason}")]
    Failed { steps_run: usize, reason: String },
    #[error(
        "runtime step limit {step_limit} reached before completion (status: {})",
        .status.status_label(FlowStatusLabelStyle::Compact)
    )]
    StepLimit {
        step_limit: usize,
        status: Box<FlowFiberStatus>,
    },
}

/// Only the Core's normal terminal state can admit a timing sample.
pub(in crate::app) struct CompletedRuntimeBenchTrace {
    totals: RuntimeBenchStepTotals,
    executor_stats: RuntimeExecutorStats,
    native_io: NativeTaskStats,
}

impl CompletedRuntimeBenchTrace {
    pub(in crate::app) const fn totals(&self) -> &RuntimeBenchStepTotals {
        &self.totals
    }

    pub(in crate::app) const fn executor_stats(&self) -> RuntimeExecutorStats {
        self.executor_stats
    }

    pub(in crate::app) const fn native_io(&self) -> &NativeTaskStats {
        &self.native_io
    }
}

#[derive(Default)]
pub(in crate::app) struct RuntimeBenchStepTotals {
    pub(in crate::app) executed_ops: usize,
    pub(in crate::app) child_fiber_ticks: usize,
    pub(in crate::app) max_child_fibers: usize,
    pub(in crate::app) line_effects: usize,
    pub(in crate::app) task_requests: usize,
    pub(in crate::app) task_events_in: usize,
    pub(in crate::app) diagnostics: usize,
    pub(in crate::app) pure: RuntimePureCallStats,
}

impl RuntimeBenchStepTotals {
    fn push(&mut self, stats: &RuntimeStepStats, task_requests: usize, diagnostics: usize) {
        self.executed_ops += stats.executed_ops;
        self.child_fiber_ticks += stats.child_fibers;
        self.max_child_fibers = self.max_child_fibers.max(stats.child_fibers);
        self.line_effects += stats.line_effects;
        self.task_requests += task_requests;
        self.task_events_in += stats.task_events_in;
        self.diagnostics += diagnostics;
        add_pure_stats(&mut self.pure, stats.pure);
    }
}

fn add_pure_stats(total: &mut RuntimePureCallStats, stats: RuntimePureCallStats) {
    total.pure_calls += stats.pure_calls;
    total.math_calls += stats.math_calls;
    total.math_accelerated_calls += stats.math_accelerated_calls;
    total.batch_calls += stats.batch_calls;
    total.batch_items += stats.batch_items;
    total.flat_batch_calls += stats.flat_batch_calls;
    total.flat_batch_items += stats.flat_batch_items;
    total.flat_batch_bytes_borrowed += stats.flat_batch_bytes_borrowed;
    total.flatten_materializations += stats.flatten_materializations;
    total.flatten_bytes_copied += stats.flatten_bytes_copied;
    total.jit_calls += stats.jit_calls;
    total.aot_calls += stats.aot_calls;
    total.vm_calls += stats.vm_calls;
    total.arg_stack_packs += stats.arg_stack_packs;
    total.arg_vec_allocations += stats.arg_vec_allocations;
    total.arg_bytes_copied += stats.arg_bytes_copied;
    total.arg_bytes_borrowed += stats.arg_bytes_borrowed;
    total.result_bytes_copied += stats.result_bytes_copied;
    total.parallel_policy_checks += stats.parallel_policy_checks;
    total.parallel_work_units += stats.parallel_work_units;
    total.parallel_batches += stats.parallel_batches;
    total.parallel_skipped_backend += stats.parallel_skipped_backend;
    total.parallel_skipped_small += stats.parallel_skipped_small;
    total.thread_pool_jobs += stats.thread_pool_jobs;
    total.thread_pool_build_elapsed_ns += stats.thread_pool_build_elapsed_ns;
    total.fallbacks += stats.fallbacks;
}
