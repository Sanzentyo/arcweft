use super::executor::RuntimeExecutorCore;
use super::parse::step_options;
use super::steps::RuntimeStepRunConfig;
use arcweft_core::engine::FlowFiberStatus;
use arcweft_core::step::{
    RuntimePureCallStats, RuntimeStepInput, RuntimeStepResult, RuntimeStepStats,
};
use arcweft_core::value::RuntimeBinding;
use arcweft_host_adapter::HostCallPolicy;
use arcweft_runtime_accelerator::RuntimePureAccelerator;
use arcweft_runtime_host::{
    NativeAdapterRegistrar, NativeTaskBridge, NativeTaskStats, RuntimeExecutorStats,
    runtime_executor_stats,
};
use std::path::Path;
use std::process::ExitCode;

pub(in crate::app) fn run_runtime_bench_steps_with_pure(
    mut executor: RuntimeExecutorCore,
    source_path: Option<&Path>,
    config: RuntimeStepRunConfig,
    host_policy: &HostCallPolicy,
    adapter_registrars: &[NativeAdapterRegistrar],
    values: &[RuntimeBinding],
    pure: &mut RuntimePureAccelerator,
) -> Result<RuntimeBenchTrace, ExitCode> {
    let mut host = None;
    let mut task_events = Vec::new();
    let mut totals = RuntimeBenchStepTotals::default();
    for _ in 0..config.steps {
        let result = executor.step_with_root_bindings(
            RuntimeStepInput {
                task_events: std::mem::take(&mut task_events),
                ..RuntimeStepInput::default()
            },
            values,
            step_options(config.mode, config.max_ops),
            pure,
        );
        let RuntimeStepResult {
            mut output,
            fiber_status,
            stats,
            ..
        } = result;
        let task_requests = std::mem::take(&mut output.requests.tasks);
        totals.push(&stats, task_requests.len(), output.diagnostics.len());
        let done = matches!(
            fiber_status,
            FlowFiberStatus::Done(_) | FlowFiberStatus::Failed(_)
        );
        if done {
            break;
        }
        if let Some(source_path) = source_path
            && !task_requests.is_empty()
        {
            if host.is_none() {
                host = Some(
                    NativeTaskBridge::try_new(source_path, host_policy.clone(), adapter_registrars)
                        .map_err(|error| {
                            eprintln!("error: {error}");
                            ExitCode::FAILURE
                        })?,
                );
            }
            if let Some(host) = host.as_mut() {
                task_events = host.complete_tasks(task_requests);
            }
        }
    }
    Ok(RuntimeBenchTrace {
        totals,
        executor_stats: runtime_executor_stats(executor.fast_path_ops(), pure),
        native_io: host
            .as_ref()
            .map_or_else(NativeTaskStats::default, NativeTaskBridge::stats),
    })
}

pub(in crate::app) struct RuntimeBenchTrace {
    pub(in crate::app) totals: RuntimeBenchStepTotals,
    pub(in crate::app) executor_stats: RuntimeExecutorStats,
    pub(in crate::app) native_io: NativeTaskStats,
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
