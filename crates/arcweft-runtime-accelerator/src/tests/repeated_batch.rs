use super::*;
use arcweft_core::pure::VmRuntimePureCallBackend;
use arcweft_core::value::RuntimeExpressionFailure;

fn modes() -> impl Iterator<Item = RuntimePureBackendMode> {
    [
        RuntimePureBackendMode::Vm,
        RuntimePureBackendMode::Aot,
        RuntimePureBackendMode::Auto,
    ]
    .into_iter()
    .chain(
        cfg!(all(feature = "native-jit", not(target_arch = "wasm32")))
            .then_some(RuntimePureBackendMode::Jit),
    )
}

fn division_with_unused_formal() -> AdmittedHelper {
    let ty = helper_type_identity(RuntimePureInputType::I64);
    admit_helper(
        "repeated_division",
        vec![RuntimePureInputType::I64; 3],
        RuntimePureOutputType::I64,
        true,
        RuntimePureHelperOrigin::Annotated,
        move |inputs, output_ty| {
            binary_expr(
                output_ty,
                local_expr(ty, inputs[0].clone(), RuntimeLocalReadMode::Copy),
                RuntimeBinaryOp::Div,
                local_expr(ty, inputs[1].clone(), RuntimeLocalReadMode::Copy),
            )
        },
    )
}

#[test]
fn repeated_batch_fault_counts_only_the_attempted_row() {
    let owner = division_with_unused_formal();
    let function = owner.function_ref();
    let row = [21, 0, i64::MAX];
    let expected =
        RuntimeEvalError::RecoverableExpression(RuntimeExpressionFailure::DivisionByZero);
    let batch = function.repeated_i64_batch(&row, 4096).unwrap();
    let mut core = VmRuntimePureCallBackend::default();
    assert_eq!(
        core.call_i64_repeated_flat_batch_sum(batch),
        Err(expected.clone())
    );
    assert_eq!(core.stats().pure_calls, 1);
    assert_eq!(core.stats().vm_calls, 1);
    let resumed = [21, 3, i64::MAX];
    let resumed_batch = function.repeated_i64_batch(&resumed, 3).unwrap();
    assert_eq!(core.call_i64_repeated_flat_batch_sum(resumed_batch), Ok(21));
    assert_eq!(core.stats().pure_calls, 4);
    for mode in modes() {
        let mut backend = RuntimePureAccelerator::new(mode, owner.plan());
        assert_eq!(
            backend.call_i64_repeated_flat_batch_sum(batch),
            Err(expected.clone())
        );
        assert_eq!(backend.stats().pure_calls, 1, "{mode:?}");
        assert_eq!(
            backend.stats().vm_calls + backend.stats().aot_calls + backend.stats().jit_calls,
            1,
            "{mode:?}"
        );
        assert_eq!(backend.stats().arg_bytes_borrowed, 24);
        assert_eq!(backend.stats().arg_bytes_copied, 0);
        assert_eq!(backend.stats().arg_vec_allocations, 0);
        assert!(!backend.has_worker_pool());
        if mode == RuntimePureBackendMode::Jit
            || (mode == RuntimePureBackendMode::Auto && native_jit_enabled())
        {
            assert_eq!(backend.stats().jit_calls, 1);
            assert_eq!(backend.stats().aot_calls + backend.stats().fallbacks, 0);
        }
        assert_eq!(
            backend.call_i64_repeated_flat_batch_sum(resumed_batch),
            Ok(21)
        );
        assert_eq!(backend.stats().pure_calls, 4);
        assert_eq!(backend.compile_stats().cache_hits, 4);
        assert_eq!(backend.stats().arg_bytes_borrowed, 48);
        assert_eq!(backend.stats().result_bytes_copied, 0);
    }
}

#[test]
fn repeated_batch_header_checks_unused_formals_and_output_abi() {
    let owner = admitted_ordinary_add_with_unused_formal(0);
    let function = owner.function_ref();
    for (row, found) in [(&[1][..], 1), (&[1, 2, 3][..], 3)] {
        assert!(matches!(
            function.repeated_i64_batch(row, 0),
            Err(RuntimeEvalError::FunctionArgumentCount { expected: 2, found: actual })
                if actual == found
        ));
    }
    assert!(matches!(
        function.repeated_i64_batch(&[0; 5], 0),
        Err(RuntimeEvalError::TooManyPureArgs {
            max: 4,
            found: 5,
            ..
        })
    ));
    if let Ok(too_many) = usize::try_from(i64::MAX as u64 + 1) {
        assert!(matches!(
            function.repeated_i64_batch(&[1, 2], too_many),
            Err(RuntimeEvalError::UnsupportedPure { reason, .. })
                if reason.contains("row count must fit i64")
        ));
    }
    // Reading only the first formal must not hide the unused second formal's ABI.
    let mixed = admit_helper(
        "mixed_formals",
        vec![RuntimePureInputType::I64, RuntimePureInputType::U64],
        RuntimePureOutputType::I64,
        true,
        RuntimePureHelperOrigin::Annotated,
        |inputs, output_ty| local_expr(output_ty, inputs[0].clone(), RuntimeLocalReadMode::Copy),
    );
    let wrong_output = admit_helper(
        "unsigned_output",
        vec![RuntimePureInputType::I64; 2],
        RuntimePureOutputType::U64,
        true,
        RuntimePureHelperOrigin::Annotated,
        |_, output_ty| value_expr(output_ty, RuntimeValue::u64(7)),
    );
    for function in [mixed.function_ref(), wrong_output.function_ref()] {
        assert!(matches!(
            function.repeated_i64_batch(&[1, 2], 0),
            Err(RuntimeEvalError::UnsupportedPure { reason, .. })
                if reason.contains("complete function signature")
        ));
    }
}

#[test]
fn empty_repeated_batch_does_no_work_or_promotion() {
    let owner = division_with_unused_formal();
    let row = [21, 0, i64::MAX];
    let batch = owner.function_ref().repeated_i64_batch(&row, 0).unwrap();
    let mut core = VmRuntimePureCallBackend::default();
    assert_eq!(core.call_i64_repeated_flat_batch_sum(batch), Ok(0));
    assert_eq!(core.stats(), RuntimePureCallStats::default());
    for mode in modes() {
        let mut backend = RuntimePureAccelerator::new(mode, owner.plan());
        let before = backend.compile_stats();
        assert_eq!(backend.call_i64_repeated_flat_batch_sum(batch), Ok(0));
        assert_eq!(backend.stats(), RuntimePureCallStats::default());
        assert_eq!(backend.compile_stats(), before);
        assert!(!backend.has_worker_pool());
    }
}

#[test]
fn repeated_batch_executes_each_row_and_borrows_one_complete_row() {
    let owner = admitted_ordinary_add_with_unused_formal(1);
    let row = [i64::MAX, 999];
    let batch = owner.function_ref().repeated_i64_batch(&row, 4097).unwrap();
    assert!(std::ptr::eq(batch.row().as_ptr(), row.as_ptr()));
    assert_eq!(batch.function().id(), owner.function_ref().id());
    assert!(Arc::ptr_eq(batch.function().plan(), owner.plan()));
    let expected = (0..4097).fold(0_i64, |sum, _| sum.wrapping_add(i64::MIN));
    let mut core = VmRuntimePureCallBackend::default();
    assert_eq!(core.call_i64_repeated_flat_batch_sum(batch), Ok(expected));
    assert_eq!(core.stats().pure_calls, 4097);
    assert_eq!(core.stats().vm_calls, 4097);
    assert_eq!(core.stats().arg_bytes_borrowed, 16);
    for mode in modes() {
        let mut backend = RuntimePureAccelerator::with_config(
            RuntimePureAcceleratorConfig {
                backend: mode,
                workers: RuntimePureWorkerCount::Fixed(4),
                batch_min_len: 1,
                ..RuntimePureAcceleratorConfig::default()
            },
            owner.plan(),
        );
        assert_eq!(
            backend.call_i64_repeated_flat_batch_sum(batch),
            Ok(expected)
        );
        let stats = backend.stats();
        assert_eq!(stats.pure_calls, 4097);
        assert_eq!(stats.vm_calls + stats.aot_calls + stats.jit_calls, 4097);
        assert_eq!(backend.compile_stats().cache_hits, 4097);
        assert_eq!(stats.batch_calls, 1);
        assert_eq!(stats.batch_items, 4097);
        assert_eq!(stats.flat_batch_calls, 1);
        assert_eq!(stats.flat_batch_items, 4097);
        assert_eq!(stats.arg_bytes_borrowed, 16);
        assert_eq!(stats.flat_batch_bytes_borrowed, 16);
        assert_eq!(stats.arg_bytes_copied + stats.result_bytes_copied, 0);
        assert_eq!(stats.arg_vec_allocations + stats.thread_pool_jobs, 0);
        assert!(!backend.has_worker_pool());
        if mode == RuntimePureBackendMode::Auto && native_jit_enabled() {
            assert_eq!(stats.jit_calls, 4097);
            assert_eq!(backend.compile_stats().auto_jit_promotions, 1);
        }
        backend.reset_runtime_counters();
        assert_eq!(backend.stats(), RuntimePureCallStats::default());
    }
}

#[test]
fn repeated_batch_with_no_inputs_still_executes_every_row() {
    let owner = admit_helper(
        "constant_row",
        Vec::new(),
        RuntimePureOutputType::I64,
        true,
        RuntimePureHelperOrigin::Annotated,
        |_, output_ty| value_expr(output_ty, RuntimeValue::i64(7)),
    );
    let batch = owner.function_ref().repeated_i64_batch(&[], 3).unwrap();
    let mut core = VmRuntimePureCallBackend::default();
    assert_eq!(core.call_i64_repeated_flat_batch_sum(batch), Ok(21));
    assert_eq!(core.stats().vm_calls, 3);
    for mode in modes() {
        let mut backend = RuntimePureAccelerator::new(mode, owner.plan());
        assert_eq!(backend.call_i64_repeated_flat_batch_sum(batch), Ok(21));
        assert_eq!(backend.stats().pure_calls, 3);
        assert_eq!(backend.compile_stats().cache_hits, 3);
        assert_eq!(backend.stats().arg_bytes_borrowed, 0);
    }
}

#[test]
fn repeated_batch_foreign_plan_uses_its_original_body_on_vm() {
    let cached = admitted_ordinary_add_with_unused_formal(1);
    let foreign = admitted_ordinary_add_with_unused_formal(7);
    let row = [10, i64::MAX];
    let batch = foreign.function_ref().repeated_i64_batch(&row, 3).unwrap();
    for mode in modes() {
        let mut backend = RuntimePureAccelerator::new(mode, cached.plan());
        assert_eq!(backend.call_i64_repeated_flat_batch_sum(batch), Ok(51));
        assert_eq!(backend.stats().pure_calls, 3);
        assert_eq!(backend.stats().vm_calls, 3);
        assert_eq!(backend.stats().fallbacks, 3);
        assert_eq!(backend.stats().jit_calls + backend.stats().aot_calls, 0);
        assert_eq!(backend.compile_stats().cache_hits, 0);
        assert_eq!(backend.compile_stats().cache_misses, 3);
        assert_eq!(backend.compile_stats().auto_jit_promotions, 0);
        assert_eq!(backend.stats().arg_bytes_borrowed, 16);
    }
}
