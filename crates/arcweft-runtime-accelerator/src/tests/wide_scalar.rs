use super::*;

#[test]
fn executable_wide_integer_calls_keep_native_scalar_execution_after_auto_promotion() {
    let base = (1_i128 << 100) + 4;
    assert_executable_scalar(base, i128::MAX, 1, base + 1);
    let base = (1_u128 << 100) + 4;
    assert_executable_scalar(base, u128::MAX, 1, base + 1);
}

fn assert_executable_scalar<T>(base: T, unused: T, one: T, expected: T)
where
    T: RuntimePureScalarInteger + PartialEq + std::fmt::Debug,
{
    let owner = admitted_ordinary_numeric_body_with_unused_formal(
        T::INPUT_TYPE,
        one.into_runtime_value(),
        true,
    );
    let function = owner.function_ref();
    assert!(function.body.is_executable());
    assert!(owner.plan().pure_helpers().is_empty());
    assert_eq!(function.inputs.len(), 2);
    let foreign_plan = Arc::new(owner.plan().as_ref().clone());
    let foreign = RuntimePureFunctionRef::resolve(&foreign_plan, function.id()).unwrap();

    for mode in [
        RuntimePureBackendMode::Vm,
        RuntimePureBackendMode::Aot,
        RuntimePureBackendMode::Jit,
        RuntimePureBackendMode::Auto,
    ] {
        let mut accelerator = RuntimePureAccelerator::new(mode, owner.plan());
        let attempts = accelerator.compile_stats().jit_attempts;
        assert_eq!(
            accelerator
                .call_exact_int_slice(foreign, &[base, unused])
                .unwrap(),
            None,
            "equal source rows do not grant a foreign plan lease"
        );
        assert_eq!(accelerator.compile_stats().jit_attempts, attempts);
        assert_eq!(accelerator.stats().pure_calls, 0);
        if mode == RuntimePureBackendMode::Vm {
            assert_eq!(
                accelerator
                    .call_exact_int_slice(function, &[base, unused])
                    .unwrap(),
                None,
                "VM retains the original scheduled function continuation"
            );
            assert_eq!(accelerator.stats().pure_calls, 0);
            continue;
        }
        for _ in 0..160 {
            assert_eq!(
                accelerator
                    .call_exact_int_slice(function, &[base, unused])
                    .unwrap(),
                Some(expected)
            );
        }
        let stats = accelerator.stats();
        assert_eq!(stats.pure_calls, 160);
        assert_eq!(stats.aot_calls + stats.jit_calls, 160);
        assert_eq!(stats.vm_calls, 0);
        assert_eq!(stats.fallbacks, 0);
        assert_eq!(stats.arg_bytes_borrowed, 160 * 2 * size_of::<T>());
        assert_eq!(stats.arg_vec_allocations, 0);
        assert_eq!(stats.arg_bytes_copied, 0);
        match mode {
            RuntimePureBackendMode::Jit => assert_eq!(stats.jit_calls, 160),
            RuntimePureBackendMode::Auto => {
                assert!(stats.jit_calls > 0);
                assert_eq!(accelerator.compile_stats().auto_jit_promotions, 1);
            }
            RuntimePureBackendMode::Aot => assert_eq!(stats.aot_calls, 160),
            RuntimePureBackendMode::Vm => unreachable!(),
        }
        accelerator.reset_runtime_counters();
        let attempts = accelerator.compile_stats().jit_attempts;
        assert_eq!(
            accelerator
                .call_exact_int_slice(foreign, &[base, unused])
                .unwrap(),
            None
        );
        assert_eq!(accelerator.compile_stats().jit_attempts, attempts);
        assert_eq!(accelerator.stats().pure_calls, 0);
        assert_eq!(
            accelerator
                .call_exact_int_slice(function, &[base, unused])
                .unwrap(),
            Some(expected)
        );
        assert_eq!(accelerator.stats().vm_calls, 0);
        assert_eq!(accelerator.stats().fallbacks, 0);
    }
}
