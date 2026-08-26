use crate::step::RuntimePureCallStats;

fn stats(value: usize) -> RuntimePureCallStats {
    RuntimePureCallStats {
        awbc_pure_program_calls: value,
        pure_calls: value,
        math_calls: value,
        math_accelerated_calls: value,
        batch_calls: value,
        batch_items: value,
        flat_batch_calls: value,
        flat_batch_items: value,
        flat_batch_bytes_borrowed: value,
        flatten_materializations: value,
        flatten_bytes_copied: value,
        jit_calls: value,
        aot_calls: value,
        vm_calls: value,
        arg_stack_packs: value,
        arg_vec_allocations: value,
        arg_bytes_copied: value,
        arg_bytes_borrowed: value,
        result_bytes_copied: value,
        parallel_policy_checks: value,
        parallel_work_units: value,
        parallel_batches: value,
        parallel_skipped_backend: value,
        parallel_skipped_small: value,
        thread_pool_jobs: value,
        thread_pool_build_elapsed_ns: value as u128,
        fallbacks: value,
    }
}

#[test]
fn runtime_pure_call_stats_saturating_delta_covers_every_counter() {
    assert_eq!(stats(8).saturating_delta(stats(3)), stats(5));
}

#[test]
fn runtime_pure_call_stats_saturating_delta_never_underflows() {
    assert_eq!(
        stats(3).saturating_delta(stats(8)),
        RuntimePureCallStats::default()
    );
}
