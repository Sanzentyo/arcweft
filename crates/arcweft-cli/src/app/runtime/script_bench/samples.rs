use super::super::bench::RuntimeBenchTrace;
use crate::output::{
    ScriptBenchDeterministicSummary, ScriptBenchElapsedSummary, ScriptBenchSectionRunSummary,
};
use arcweft_runtime_host::{
    NativeSchedulerStats, NativeTaskClassCounts, NativeTaskStats, RuntimeExecutorMathStatsSummary,
    RuntimeExecutorStats,
};
use arcweft_test::{BenchSection, ScriptCommand};

#[derive(Default)]
pub(in crate::app) struct RuntimeBenchSamples {
    elapsed: Vec<u128>,
    executed_ops: Vec<usize>,
    child_fiber_ticks: Vec<usize>,
    max_child_fibers: Vec<usize>,
    line_effects: Vec<usize>,
    task_requests: Vec<usize>,
    task_events_in: Vec<usize>,
    pure_calls: Vec<usize>,
    math_calls: Vec<usize>,
    math_accelerated_calls: Vec<usize>,
    pure_batch_calls: Vec<usize>,
    pure_batch_items: Vec<usize>,
    pure_flat_batch_calls: Vec<usize>,
    pure_flat_batch_items: Vec<usize>,
    pure_flat_batch_bytes_borrowed: Vec<usize>,
    pure_flatten_materializations: Vec<usize>,
    pure_flatten_bytes_copied: Vec<usize>,
    pure_jit_calls: Vec<usize>,
    pure_aot_calls: Vec<usize>,
    pure_vm_calls: Vec<usize>,
    pure_parallel_policy_checks: Vec<usize>,
    pure_parallel_work_units: Vec<usize>,
    pure_parallel_batches: Vec<usize>,
    pure_parallel_skipped_backend: Vec<usize>,
    pure_parallel_skipped_small: Vec<usize>,
    pure_thread_pool_jobs: Vec<usize>,
    pure_thread_pool_build_elapsed_ns: Vec<u128>,
    pure_arg_stack_packs: Vec<usize>,
    pure_arg_vec_allocations: Vec<usize>,
    pure_arg_bytes_copied: Vec<usize>,
    pure_arg_bytes_borrowed: Vec<usize>,
    pure_result_bytes_copied: Vec<usize>,
    pure_fallbacks: Vec<usize>,
    aot_fast_path_ops: Vec<usize>,
    executor_stats_samples: Vec<RuntimeExecutorStats>,
    native_io: NativeTaskStatsSamples,
    diagnostics: usize,
}

impl RuntimeBenchSamples {
    pub(in crate::app) fn with_capacity(capacity: usize) -> Self {
        Self {
            elapsed: Vec::with_capacity(capacity),
            executed_ops: Vec::with_capacity(capacity),
            child_fiber_ticks: Vec::with_capacity(capacity),
            max_child_fibers: Vec::with_capacity(capacity),
            line_effects: Vec::with_capacity(capacity),
            task_requests: Vec::with_capacity(capacity),
            task_events_in: Vec::with_capacity(capacity),
            pure_calls: Vec::with_capacity(capacity),
            math_calls: Vec::with_capacity(capacity),
            math_accelerated_calls: Vec::with_capacity(capacity),
            pure_batch_calls: Vec::with_capacity(capacity),
            pure_batch_items: Vec::with_capacity(capacity),
            pure_flat_batch_calls: Vec::with_capacity(capacity),
            pure_flat_batch_items: Vec::with_capacity(capacity),
            pure_flat_batch_bytes_borrowed: Vec::with_capacity(capacity),
            pure_flatten_materializations: Vec::with_capacity(capacity),
            pure_flatten_bytes_copied: Vec::with_capacity(capacity),
            pure_jit_calls: Vec::with_capacity(capacity),
            pure_aot_calls: Vec::with_capacity(capacity),
            pure_vm_calls: Vec::with_capacity(capacity),
            pure_parallel_policy_checks: Vec::with_capacity(capacity),
            pure_parallel_work_units: Vec::with_capacity(capacity),
            pure_parallel_batches: Vec::with_capacity(capacity),
            pure_parallel_skipped_backend: Vec::with_capacity(capacity),
            pure_parallel_skipped_small: Vec::with_capacity(capacity),
            pure_thread_pool_jobs: Vec::with_capacity(capacity),
            pure_thread_pool_build_elapsed_ns: Vec::with_capacity(capacity),
            pure_arg_stack_packs: Vec::with_capacity(capacity),
            pure_arg_vec_allocations: Vec::with_capacity(capacity),
            pure_arg_bytes_copied: Vec::with_capacity(capacity),
            pure_arg_bytes_borrowed: Vec::with_capacity(capacity),
            pure_result_bytes_copied: Vec::with_capacity(capacity),
            pure_fallbacks: Vec::with_capacity(capacity),
            aot_fast_path_ops: Vec::with_capacity(capacity),
            executor_stats_samples: Vec::with_capacity(capacity),
            native_io: NativeTaskStatsSamples::with_capacity(capacity),
            diagnostics: 0,
        }
    }

    pub(in crate::app) fn push(&mut self, elapsed_ns: u128, trace: &RuntimeBenchTrace) {
        self.elapsed.push(elapsed_ns);
        self.push_step_stats(trace);
        self.push_pure_stats(trace);
        self.aot_fast_path_ops
            .push(trace.executor_stats.aot_fast_path_ops);
        self.executor_stats_samples.push(trace.executor_stats);
        self.native_io.push(&trace.native_io);
    }

    fn push_step_stats(&mut self, trace: &RuntimeBenchTrace) {
        self.executed_ops.push(trace.totals.executed_ops);
        self.child_fiber_ticks.push(trace.totals.child_fiber_ticks);
        self.max_child_fibers.push(trace.totals.max_child_fibers);
        self.line_effects.push(trace.totals.line_effects);
        self.task_requests.push(trace.totals.task_requests);
        self.task_events_in.push(trace.totals.task_events_in);
        self.diagnostics += trace.totals.diagnostics;
    }

    fn push_pure_stats(&mut self, trace: &RuntimeBenchTrace) {
        self.pure_calls.push(trace.totals.pure.pure_calls);
        self.math_calls.push(trace.totals.pure.math_calls);
        self.math_accelerated_calls
            .push(trace.totals.pure.math_accelerated_calls);
        self.pure_batch_items.push(trace.totals.pure.batch_items);
        self.pure_batch_calls.push(trace.totals.pure.batch_calls);
        self.pure_flat_batch_calls
            .push(trace.totals.pure.flat_batch_calls);
        self.pure_flat_batch_items
            .push(trace.totals.pure.flat_batch_items);
        self.pure_flat_batch_bytes_borrowed
            .push(trace.totals.pure.flat_batch_bytes_borrowed);
        self.pure_flatten_materializations
            .push(trace.totals.pure.flatten_materializations);
        self.pure_flatten_bytes_copied
            .push(trace.totals.pure.flatten_bytes_copied);
        self.pure_jit_calls.push(trace.totals.pure.jit_calls);
        self.pure_aot_calls.push(trace.totals.pure.aot_calls);
        self.pure_vm_calls.push(trace.totals.pure.vm_calls);
        self.pure_parallel_policy_checks
            .push(trace.totals.pure.parallel_policy_checks);
        self.pure_parallel_work_units
            .push(trace.totals.pure.parallel_work_units);
        self.pure_parallel_batches
            .push(trace.totals.pure.parallel_batches);
        self.pure_parallel_skipped_backend
            .push(trace.totals.pure.parallel_skipped_backend);
        self.pure_parallel_skipped_small
            .push(trace.totals.pure.parallel_skipped_small);
        self.pure_thread_pool_jobs
            .push(trace.totals.pure.thread_pool_jobs);
        self.pure_thread_pool_build_elapsed_ns
            .push(trace.totals.pure.thread_pool_build_elapsed_ns);
        self.pure_arg_stack_packs
            .push(trace.totals.pure.arg_stack_packs);
        self.pure_arg_vec_allocations
            .push(trace.totals.pure.arg_vec_allocations);
        self.pure_arg_bytes_copied
            .push(trace.totals.pure.arg_bytes_copied);
        self.pure_arg_bytes_borrowed
            .push(trace.totals.pure.arg_bytes_borrowed);
        self.pure_result_bytes_copied
            .push(trace.totals.pure.result_bytes_copied);
        self.pure_fallbacks.push(trace.totals.pure.fallbacks);
    }

    pub(in crate::app) fn executor_stats(&mut self) -> RuntimeExecutorStats {
        let mut executor_stats = self
            .executor_stats_samples
            .first()
            .copied()
            .unwrap_or_else(RuntimeExecutorStats::default);
        executor_stats.aot_fast_path_ops = median_usize(&mut self.aot_fast_path_ops);
        executor_stats.math = median_executor_math_stats(&self.executor_stats_samples);
        executor_stats
    }

    pub(in crate::app) fn elapsed_summary(&mut self) -> ScriptBenchElapsedSummary {
        ScriptBenchElapsedSummary {
            min: *self.elapsed.iter().min().unwrap_or(&0),
            median: median_u128(&mut self.elapsed),
            max: *self.elapsed.iter().max().unwrap_or(&0),
        }
    }

    pub(in crate::app) fn per_executed_op_ns(&mut self) -> u128 {
        let elapsed = median_u128(&mut self.elapsed);
        let executed_ops = median_usize(&mut self.executed_ops);
        if executed_ops == 0 {
            0
        } else {
            elapsed / executed_ops as u128
        }
    }

    pub(in crate::app) fn deterministic_summary(&mut self) -> ScriptBenchDeterministicSummary {
        ScriptBenchDeterministicSummary {
            executed_ops_median: median_usize(&mut self.executed_ops),
            child_fiber_ticks_median: median_usize(&mut self.child_fiber_ticks),
            max_child_fibers_median: median_usize(&mut self.max_child_fibers),
            line_effects_median: median_usize(&mut self.line_effects),
            task_requests_median: median_usize(&mut self.task_requests),
            task_events_in_median: median_usize(&mut self.task_events_in),
            pure_calls_median: median_usize(&mut self.pure_calls),
            math_calls_median: median_usize(&mut self.math_calls),
            math_accelerated_calls_median: median_usize(&mut self.math_accelerated_calls),
            pure_batch_calls_median: median_usize(&mut self.pure_batch_calls),
            pure_batch_items_median: median_usize(&mut self.pure_batch_items),
            pure_flat_batch_calls_median: median_usize(&mut self.pure_flat_batch_calls),
            pure_flat_batch_items_median: median_usize(&mut self.pure_flat_batch_items),
            pure_flat_batch_bytes_borrowed_median: median_usize(
                &mut self.pure_flat_batch_bytes_borrowed,
            ),
            pure_flatten_materializations_median: median_usize(
                &mut self.pure_flatten_materializations,
            ),
            pure_flatten_bytes_copied_median: median_usize(&mut self.pure_flatten_bytes_copied),
            pure_jit_calls_median: median_usize(&mut self.pure_jit_calls),
            pure_aot_calls_median: median_usize(&mut self.pure_aot_calls),
            pure_vm_calls_median: median_usize(&mut self.pure_vm_calls),
            pure_parallel_policy_checks_median: median_usize(&mut self.pure_parallel_policy_checks),
            pure_parallel_work_units_median: median_usize(&mut self.pure_parallel_work_units),
            pure_parallel_batches_median: median_usize(&mut self.pure_parallel_batches),
            pure_parallel_skipped_backend_median: median_usize(
                &mut self.pure_parallel_skipped_backend,
            ),
            pure_parallel_skipped_small_median: median_usize(&mut self.pure_parallel_skipped_small),
            pure_thread_pool_jobs_median: median_usize(&mut self.pure_thread_pool_jobs),
            pure_thread_pool_build_elapsed_ns_median: median_u128(
                &mut self.pure_thread_pool_build_elapsed_ns,
            ),
            pure_arg_stack_packs_median: median_usize(&mut self.pure_arg_stack_packs),
            pure_arg_vec_allocations_median: median_usize(&mut self.pure_arg_vec_allocations),
            pure_arg_bytes_copied_median: median_usize(&mut self.pure_arg_bytes_copied),
            pure_arg_bytes_borrowed_median: median_usize(&mut self.pure_arg_bytes_borrowed),
            pure_result_bytes_copied_median: median_usize(&mut self.pure_result_bytes_copied),
            pure_fallbacks_median: median_usize(&mut self.pure_fallbacks),
            diagnostics: self.diagnostics,
        }
    }

    pub(in crate::app) fn native_io_median(&mut self) -> NativeTaskStats {
        self.native_io.median()
    }
}

#[derive(Default)]
struct NativeTaskStatsSamples {
    completed_tasks: Vec<usize>,
    failed_tasks: Vec<usize>,
    read_ops: Vec<usize>,
    write_ops: Vec<usize>,
    system_info_ops: Vec<usize>,
    bytes_read: Vec<usize>,
    bytes_written: Vec<usize>,
    parallel_batches: Vec<usize>,
    parallel_tasks: Vec<usize>,
    parallel_io_tasks: Vec<usize>,
    parallel_system_info_tasks: Vec<usize>,
    parallel_marker_tasks: Vec<usize>,
    parallel_workers: Vec<usize>,
    scheduler_submit_elapsed_ns: Vec<u128>,
    scheduler_dispatch_elapsed_ns: Vec<u128>,
    host_complete_elapsed_ns: Vec<u128>,
    event_build_elapsed_ns: Vec<u128>,
    scheduler_complete_elapsed_ns: Vec<u128>,
    scheduler: NativeSchedulerStatsSamples,
}

impl NativeTaskStatsSamples {
    pub(in crate::app) fn with_capacity(capacity: usize) -> Self {
        Self {
            completed_tasks: Vec::with_capacity(capacity),
            failed_tasks: Vec::with_capacity(capacity),
            read_ops: Vec::with_capacity(capacity),
            write_ops: Vec::with_capacity(capacity),
            system_info_ops: Vec::with_capacity(capacity),
            bytes_read: Vec::with_capacity(capacity),
            bytes_written: Vec::with_capacity(capacity),
            parallel_batches: Vec::with_capacity(capacity),
            parallel_tasks: Vec::with_capacity(capacity),
            parallel_io_tasks: Vec::with_capacity(capacity),
            parallel_system_info_tasks: Vec::with_capacity(capacity),
            parallel_marker_tasks: Vec::with_capacity(capacity),
            parallel_workers: Vec::with_capacity(capacity),
            scheduler_submit_elapsed_ns: Vec::with_capacity(capacity),
            scheduler_dispatch_elapsed_ns: Vec::with_capacity(capacity),
            host_complete_elapsed_ns: Vec::with_capacity(capacity),
            event_build_elapsed_ns: Vec::with_capacity(capacity),
            scheduler_complete_elapsed_ns: Vec::with_capacity(capacity),
            scheduler: NativeSchedulerStatsSamples::with_capacity(capacity),
        }
    }

    fn push(&mut self, stats: &NativeTaskStats) {
        self.completed_tasks.push(stats.completed_tasks);
        self.failed_tasks.push(stats.failed_tasks);
        self.read_ops.push(stats.read_ops);
        self.write_ops.push(stats.write_ops);
        self.system_info_ops.push(stats.system_info_ops);
        self.bytes_read.push(stats.bytes_read);
        self.bytes_written.push(stats.bytes_written);
        self.parallel_batches.push(stats.parallel_batches);
        self.parallel_tasks.push(stats.parallel_tasks);
        self.parallel_io_tasks.push(stats.parallel_io_tasks);
        self.parallel_system_info_tasks
            .push(stats.parallel_system_info_tasks);
        self.parallel_marker_tasks.push(stats.parallel_marker_tasks);
        self.parallel_workers.push(stats.parallel_workers);
        self.scheduler_submit_elapsed_ns
            .push(stats.scheduler_submit_elapsed_ns);
        self.scheduler_dispatch_elapsed_ns
            .push(stats.scheduler_dispatch_elapsed_ns);
        self.host_complete_elapsed_ns
            .push(stats.host_complete_elapsed_ns);
        self.event_build_elapsed_ns
            .push(stats.event_build_elapsed_ns);
        self.scheduler_complete_elapsed_ns
            .push(stats.scheduler_complete_elapsed_ns);
        self.scheduler.push(&stats.scheduler);
    }

    fn median(&mut self) -> NativeTaskStats {
        NativeTaskStats {
            completed_tasks: median_usize(&mut self.completed_tasks),
            failed_tasks: median_usize(&mut self.failed_tasks),
            read_ops: median_usize(&mut self.read_ops),
            write_ops: median_usize(&mut self.write_ops),
            system_info_ops: median_usize(&mut self.system_info_ops),
            bytes_read: median_usize(&mut self.bytes_read),
            bytes_written: median_usize(&mut self.bytes_written),
            parallel_batches: median_usize(&mut self.parallel_batches),
            parallel_tasks: median_usize(&mut self.parallel_tasks),
            parallel_io_tasks: median_usize(&mut self.parallel_io_tasks),
            parallel_system_info_tasks: median_usize(&mut self.parallel_system_info_tasks),
            parallel_marker_tasks: median_usize(&mut self.parallel_marker_tasks),
            parallel_workers: median_usize(&mut self.parallel_workers),
            scheduler_submit_elapsed_ns: median_u128(&mut self.scheduler_submit_elapsed_ns),
            scheduler_dispatch_elapsed_ns: median_u128(&mut self.scheduler_dispatch_elapsed_ns),
            host_complete_elapsed_ns: median_u128(&mut self.host_complete_elapsed_ns),
            event_build_elapsed_ns: median_u128(&mut self.event_build_elapsed_ns),
            scheduler_complete_elapsed_ns: median_u128(&mut self.scheduler_complete_elapsed_ns),
            scheduler: self.scheduler.median(),
        }
    }
}

#[derive(Default)]
struct NativeSchedulerStatsSamples {
    submitted: Vec<usize>,
    joined: Vec<usize>,
    dispatched: Vec<usize>,
    completed: Vec<usize>,
    failed: Vec<usize>,
    cancelled: Vec<usize>,
    cancel_requested: Vec<usize>,
    joined_completed: Vec<usize>,
    in_flight: Vec<usize>,
    max_in_flight: Vec<usize>,
    dispatch_sorts: Vec<usize>,
    dispatch_sort_items: Vec<usize>,
    completion_sorts: Vec<usize>,
    completion_sort_items: Vec<usize>,
    completion_normalization_passes: Vec<usize>,
    completion_normalization_checks: Vec<usize>,
    completion_events_in: Vec<usize>,
    completion_events_joined: Vec<usize>,
    completion_events_out: Vec<usize>,
    completion_sort_skipped_items: Vec<usize>,
    completion_sort_performed_items: Vec<usize>,
    joined_completion_events_emitted: Vec<usize>,
    submitted_by_class: Vec<NativeTaskClassCounts>,
    dispatched_by_class: Vec<NativeTaskClassCounts>,
    completed_by_class: Vec<NativeTaskClassCounts>,
}

impl NativeSchedulerStatsSamples {
    pub(in crate::app) fn with_capacity(capacity: usize) -> Self {
        Self {
            submitted: Vec::with_capacity(capacity),
            joined: Vec::with_capacity(capacity),
            dispatched: Vec::with_capacity(capacity),
            completed: Vec::with_capacity(capacity),
            failed: Vec::with_capacity(capacity),
            cancelled: Vec::with_capacity(capacity),
            cancel_requested: Vec::with_capacity(capacity),
            joined_completed: Vec::with_capacity(capacity),
            in_flight: Vec::with_capacity(capacity),
            max_in_flight: Vec::with_capacity(capacity),
            dispatch_sorts: Vec::with_capacity(capacity),
            dispatch_sort_items: Vec::with_capacity(capacity),
            completion_sorts: Vec::with_capacity(capacity),
            completion_sort_items: Vec::with_capacity(capacity),
            completion_normalization_passes: Vec::with_capacity(capacity),
            completion_normalization_checks: Vec::with_capacity(capacity),
            completion_events_in: Vec::with_capacity(capacity),
            completion_events_joined: Vec::with_capacity(capacity),
            completion_events_out: Vec::with_capacity(capacity),
            completion_sort_skipped_items: Vec::with_capacity(capacity),
            completion_sort_performed_items: Vec::with_capacity(capacity),
            joined_completion_events_emitted: Vec::with_capacity(capacity),
            submitted_by_class: Vec::with_capacity(capacity),
            dispatched_by_class: Vec::with_capacity(capacity),
            completed_by_class: Vec::with_capacity(capacity),
        }
    }

    fn push(&mut self, stats: &NativeSchedulerStats) {
        self.submitted.push(stats.submitted);
        self.joined.push(stats.joined);
        self.dispatched.push(stats.dispatched);
        self.completed.push(stats.completed);
        self.failed.push(stats.failed);
        self.cancelled.push(stats.cancelled);
        self.cancel_requested.push(stats.cancel_requested);
        self.joined_completed.push(stats.joined_completed);
        self.in_flight.push(stats.in_flight);
        self.max_in_flight.push(stats.max_in_flight);
        self.dispatch_sorts.push(stats.dispatch_sorts);
        self.dispatch_sort_items.push(stats.dispatch_sort_items);
        self.completion_sorts.push(stats.completion_sorts);
        self.completion_sort_items.push(stats.completion_sort_items);
        self.completion_normalization_passes
            .push(stats.completion_normalization_passes);
        self.completion_normalization_checks
            .push(stats.completion_normalization_checks);
        self.completion_events_in.push(stats.completion_events_in);
        self.completion_events_joined
            .push(stats.completion_events_joined);
        self.completion_events_out.push(stats.completion_events_out);
        self.completion_sort_skipped_items
            .push(stats.completion_sort_skipped_items);
        self.completion_sort_performed_items
            .push(stats.completion_sort_performed_items);
        self.joined_completion_events_emitted
            .push(stats.joined_completion_events_emitted);
        self.submitted_by_class.push(stats.submitted_by_class);
        self.dispatched_by_class.push(stats.dispatched_by_class);
        self.completed_by_class.push(stats.completed_by_class);
    }

    fn median(&mut self) -> NativeSchedulerStats {
        NativeSchedulerStats {
            submitted: median_usize(&mut self.submitted),
            joined: median_usize(&mut self.joined),
            dispatched: median_usize(&mut self.dispatched),
            completed: median_usize(&mut self.completed),
            failed: median_usize(&mut self.failed),
            cancelled: median_usize(&mut self.cancelled),
            cancel_requested: median_usize(&mut self.cancel_requested),
            joined_completed: median_usize(&mut self.joined_completed),
            in_flight: median_usize(&mut self.in_flight),
            max_in_flight: median_usize(&mut self.max_in_flight),
            dispatch_sorts: median_usize(&mut self.dispatch_sorts),
            dispatch_sort_items: median_usize(&mut self.dispatch_sort_items),
            completion_sorts: median_usize(&mut self.completion_sorts),
            completion_sort_items: median_usize(&mut self.completion_sort_items),
            completion_normalization_passes: median_usize(
                &mut self.completion_normalization_passes,
            ),
            completion_normalization_checks: median_usize(
                &mut self.completion_normalization_checks,
            ),
            completion_events_in: median_usize(&mut self.completion_events_in),
            completion_events_joined: median_usize(&mut self.completion_events_joined),
            completion_events_out: median_usize(&mut self.completion_events_out),
            completion_sort_skipped_items: median_usize(&mut self.completion_sort_skipped_items),
            completion_sort_performed_items: median_usize(
                &mut self.completion_sort_performed_items,
            ),
            joined_completion_events_emitted: median_usize(
                &mut self.joined_completion_events_emitted,
            ),
            submitted_by_class: median_task_class_counts(&mut self.submitted_by_class),
            dispatched_by_class: median_task_class_counts(&mut self.dispatched_by_class),
            completed_by_class: median_task_class_counts(&mut self.completed_by_class),
        }
    }
}

fn median_task_class_counts(values: &mut [NativeTaskClassCounts]) -> NativeTaskClassCounts {
    NativeTaskClassCounts {
        local_view: median_task_class_field(values, |value| value.local_view),
        io: median_task_class_field(values, |value| value.io),
        cpu: median_task_class_field(values, |value| value.cpu),
        gpu_prepare: median_task_class_field(values, |value| value.gpu_prepare),
        shader_compile: median_task_class_field(values, |value| value.shader_compile),
        wasm_call: median_task_class_field(values, |value| value.wasm_call),
        asset_decode: median_task_class_field(values, |value| value.asset_decode),
        audio_decode: median_task_class_field(values, |value| value.audio_decode),
        audio_render: median_task_class_field(values, |value| value.audio_render),
        tts_synthesis: median_task_class_field(values, |value| value.tts_synthesis),
        bgm_precompose: median_task_class_field(values, |value| value.bgm_precompose),
        lsp: median_task_class_field(values, |value| value.lsp),
        background: median_task_class_field(values, |value| value.background),
    }
}

fn median_task_class_field(
    values: &[NativeTaskClassCounts],
    field: impl Fn(&NativeTaskClassCounts) -> usize,
) -> usize {
    let mut counts = values.iter().map(field).collect::<Vec<_>>();
    median_usize(&mut counts)
}

pub(in crate::app) fn bench_goto_flow(section: &BenchSection) -> Option<String> {
    section.body.iter().find_map(command_goto_flow)
}

fn command_goto_flow(command: &ScriptCommand) -> Option<String> {
    match command {
        ScriptCommand::Goto { target } => Some(target.clone()),
        ScriptCommand::Scope { body, .. } => body.iter().find_map(command_goto_flow),
        ScriptCommand::Expectation { .. }
        | ScriptCommand::Pure { .. }
        | ScriptCommand::Other { .. } => None,
    }
}

fn median_u128(values: &mut [u128]) -> u128 {
    if values.is_empty() {
        return 0;
    }
    let mid = values.len() / 2;
    *values.select_nth_unstable(mid).1
}

fn median_usize(values: &mut [usize]) -> usize {
    if values.is_empty() {
        return 0;
    }
    let mid = values.len() / 2;
    *values.select_nth_unstable(mid).1
}

fn median_executor_math_stats(samples: &[RuntimeExecutorStats]) -> RuntimeExecutorMathStatsSummary {
    RuntimeExecutorMathStatsSummary {
        scalar_calls: median_executor_math_field(samples, |math| math.scalar_calls),
        glam_calls: median_executor_math_field(samples, |math| math.glam_calls),
        ndarray_calls: median_executor_math_field(samples, |math| math.ndarray_calls),
        wgpu_calls: median_executor_math_field(samples, |math| math.wgpu_calls),
        fused_matmul_bias_add_calls: median_executor_math_field(samples, |math| {
            math.fused_matmul_bias_add_calls
        }),
        fallback_calls: median_executor_math_field(samples, |math| math.fallback_calls),
        bytes_borrowed: median_executor_math_field(samples, |math| math.bytes_borrowed),
        bytes_copied: median_executor_math_field(samples, |math| math.bytes_copied),
        bytes_uploaded: median_executor_math_field(samples, |math| math.bytes_uploaded),
        bytes_downloaded: median_executor_math_field(samples, |math| math.bytes_downloaded),
        gpu_buffer_creations: median_executor_math_field(samples, |math| math.gpu_buffer_creations),
        gpu_buffer_reuse_hits: median_executor_math_field(samples, |math| {
            math.gpu_buffer_reuse_hits
        }),
        gpu_staging_buffer_creations: median_executor_math_field(samples, |math| {
            math.gpu_staging_buffer_creations
        }),
        gpu_staging_buffer_reuse_hits: median_executor_math_field(samples, |math| {
            math.gpu_staging_buffer_reuse_hits
        }),
        gpu_reused_dispatches: median_executor_math_field(samples, |math| {
            math.gpu_reused_dispatches
        }),
        last_backend: modal_executor_math_label(samples, |math| math.last_backend),
        last_auto_reason: modal_executor_math_label(samples, |math| math.last_auto_reason),
    }
}

fn median_executor_math_field(
    samples: &[RuntimeExecutorStats],
    field: impl Fn(RuntimeExecutorMathStatsSummary) -> usize,
) -> usize {
    let mut values = samples
        .iter()
        .map(|sample| field(sample.math))
        .collect::<Vec<_>>();
    median_usize(&mut values)
}

fn modal_executor_math_label(
    samples: &[RuntimeExecutorStats],
    field: impl Fn(RuntimeExecutorMathStatsSummary) -> Option<&'static str>,
) -> Option<&'static str> {
    let mut counts: Vec<(Option<&'static str>, usize, usize)> = Vec::new();
    for (index, sample) in samples.iter().enumerate() {
        let label = field(sample.math);
        if let Some((_, count, _)) = counts
            .iter_mut()
            .find(|(candidate, _, _)| *candidate == label)
        {
            *count += 1;
        } else {
            counts.push((label, 1, index));
        }
    }
    counts
        .into_iter()
        .max_by(|(_, lhs_count, lhs_first), (_, rhs_count, rhs_first)| {
            lhs_count
                .cmp(rhs_count)
                .then_with(|| rhs_first.cmp(lhs_first))
        })
        .and_then(|(label, _, _)| label)
}

pub(in crate::app) fn validate_bench_section(
    section: &BenchSection,
) -> ScriptBenchSectionRunSummary {
    let mut diagnostics = Vec::new();
    if !is_known_bench_section(&section.name) {
        diagnostics.push(format!("unknown bench section `{}`", section.name));
        return ScriptBenchSectionRunSummary::new(&section.name, "unknown", diagnostics);
    }
    ScriptBenchSectionRunSummary::new(&section.name, "validated", diagnostics)
}

fn is_known_bench_section(name: &str) -> bool {
    matches!(name, "setup" | "measure" | "assert" | "report")
}
