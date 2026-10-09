use super::*;
use arcweft_core::value::RuntimeExpressionFailure;

fn signed_division_helper(
    name: &str,
    input_type: RuntimePureInputType,
    output_type: RuntimePureOutputType,
) -> AdmittedHelper {
    let ty = helper_type_identity(input_type);
    admit_helper(
        name,
        vec![input_type; 3],
        output_type,
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

macro_rules! signed_fault_case {
    ($name:ident, $input:ident, $output:ident, $ty:ty, $min:expr, $wrap:expr, $value:expr, $slice:ident, $batch:ident, $sum:ident) => {
        #[test]
        fn $name() {
            let helper = signed_division_helper(
                stringify!($name),
                RuntimePureInputType::$input,
                RuntimePureOutputType::$output,
            );
            assert_eq!(helper.function_ref().inputs.len(), 3);
            let expected =
                RuntimeEvalError::RecoverableExpression(RuntimeExpressionFailure::DivisionByZero);
            for mode in [
                RuntimePureBackendMode::Jit,
                RuntimePureBackendMode::Aot,
                RuntimePureBackendMode::Vm,
            ] {
                let mut backend = RuntimePureAccelerator::new(mode, helper.plan());
                let normal: [$ty; 3] = [$wrap(21), $wrap(3), $wrap(99)];
                assert_eq!(
                    backend.$slice(helper.function_ref(), &normal),
                    Ok(Some($wrap(7)))
                );
                let wraps: [$ty; 3] = [$wrap($min), $wrap(-1), $wrap(99)];
                assert_eq!(
                    backend.$slice(helper.function_ref(), &wraps),
                    Ok(Some($wrap($min))),
                    "signed overflow is Core's successful wrapping result"
                );
                let zero: [$ty; 3] = [$wrap(12), $wrap(0), $wrap(99)];
                assert_eq!(
                    backend.$slice(helper.function_ref(), &zero),
                    Err(expected.clone())
                );
                let resumed: [$ty; 3] = [$wrap(20), $wrap(2), $wrap(99)];
                assert_eq!(
                    backend.$slice(helper.function_ref(), &resumed),
                    Ok(Some($wrap(10))),
                    "a terminal per-call failure leaves the compiled code reusable"
                );
                let flat: [$ty; 9] = [
                    $wrap(21),
                    $wrap(3),
                    $wrap(99),
                    $wrap(7),
                    $wrap(0),
                    $wrap(99),
                    $wrap(20),
                    $wrap(2),
                    $wrap(99),
                ];
                let mut output = [$wrap(77); 3];
                assert_eq!(
                    backend.$batch(helper.function_ref(), &flat, 3, &mut output),
                    Err(expected.clone())
                );
                assert_eq!(output, [$wrap(7), $wrap(77), $wrap(77)]);
                assert_eq!(
                    backend.$sum(helper.function_ref(), &flat, 3, 3),
                    Err(expected.clone())
                );
                let sum: [$ty; 6] = [
                    $wrap(21),
                    $wrap(3),
                    $wrap(99),
                    $wrap(20),
                    $wrap(2),
                    $wrap(99),
                ];
                assert_eq!(backend.$sum(helper.function_ref(), &sum, 3, 2), Ok(17));
                if mode == RuntimePureBackendMode::Jit {
                    assert_eq!(backend.summary().jit, 1);
                    assert_eq!(backend.compile_stats().jit_successes, 1);
                    assert_eq!(backend.stats().pure_calls, 10);
                    assert_eq!(backend.stats().jit_calls, 10);
                    assert_eq!(
                        backend.stats().aot_calls
                            + backend.stats().vm_calls
                            + backend.stats().fallbacks,
                        0,
                        "a checked native failure is never retried"
                    );
                    assert_eq!(backend.stats().arg_vec_allocations, 0);
                    assert_eq!(backend.stats().arg_bytes_copied, 0);
                    assert_eq!(
                        backend.stats().arg_bytes_borrowed,
                        36 * std::mem::size_of::<$ty>()
                    );
                    assert_eq!(
                        backend.stats().result_bytes_copied,
                        std::mem::size_of::<$ty>()
                    );
                }
            }
            // A wrapped denominator faults only if its branch is selected.
            let conditional = conditional_div_helper(
                concat!(stringify!($name), "_conditional"),
                RuntimePureInputType::$input,
                RuntimePureOutputType::$output,
                $value(1),
                $value(1),
                $value(0),
            );
            for mode in [
                RuntimePureBackendMode::Jit,
                RuntimePureBackendMode::Aot,
                RuntimePureBackendMode::Vm,
            ] {
                let mut backend = RuntimePureAccelerator::new(mode, conditional.plan());
                let skipped: [$ty; 2] = [$wrap(0), $wrap(-1)];
                assert_eq!(
                    backend.$slice(conditional.function_ref(), &skipped),
                    Ok(Some($wrap(0)))
                );
                let selected: [$ty; 2] = [$wrap(12), $wrap(-1)];
                assert_eq!(
                    backend.$slice(conditional.function_ref(), &selected),
                    Err(expected.clone())
                );
                if mode == RuntimePureBackendMode::Jit {
                    assert_eq!(backend.stats().jit_calls, 2);
                    assert_eq!(
                        backend.stats().aot_calls
                            + backend.stats().vm_calls
                            + backend.stats().fallbacks,
                        0
                    );
                }
            }
        }
    };
}

signed_fault_case!(
    signed_i8_fault_is_terminal_with_wrapping_and_full_formals,
    I8,
    I8,
    i8,
    i8::MIN,
    std::convert::identity,
    RuntimeValue::i8,
    call_i8_slice,
    call_i8_flat_batch,
    call_i8_flat_batch_sum
);
signed_fault_case!(
    signed_i16_fault_is_terminal_with_wrapping_and_full_formals,
    I16,
    I16,
    i16,
    i16::MIN,
    std::convert::identity,
    RuntimeValue::i16,
    call_i16_slice,
    call_i16_flat_batch,
    call_i16_flat_batch_sum
);
signed_fault_case!(
    signed_i32_fault_is_terminal_with_wrapping_and_full_formals,
    I32,
    I32,
    i32,
    i32::MIN,
    std::convert::identity,
    RuntimeValue::i32,
    call_i32_slice,
    call_i32_flat_batch,
    call_i32_flat_batch_sum
);
signed_fault_case!(
    signed_i64_fault_is_terminal_with_wrapping_and_full_formals,
    I64,
    I64,
    i64,
    i64::MIN,
    std::convert::identity,
    RuntimeValue::i64,
    call_i64_slice,
    call_i64_flat_batch,
    call_i64_flat_batch_sum
);
signed_fault_case!(
    signed_isize_fault_is_terminal_with_wrapping_and_full_formals,
    ISize,
    ISize,
    RuntimeISizeValue,
    i64::MIN,
    RuntimeISizeValue::new,
    RuntimeValue::isize,
    call_exact_int_slice,
    call_exact_int_flat_batch,
    call_exact_int_flat_batch_sum
);
signed_fault_case!(
    signed_i128_fault_is_terminal_with_wrapping_and_full_formals,
    I128,
    I128,
    i128,
    i128::MIN,
    std::convert::identity,
    RuntimeValue::i128,
    call_exact_int_slice,
    call_i128_flat_batch,
    call_i128_flat_batch_sum
);

#[test]
fn signed_i128_division_sum_refusal_keeps_the_exact_value_and_no_retry() {
    let helper = signed_division_helper(
        "signed_i128_exact_sum_refusal",
        RuntimePureInputType::I128,
        RuntimePureOutputType::I128,
    );
    for mode in [
        RuntimePureBackendMode::Jit,
        RuntimePureBackendMode::Aot,
        RuntimePureBackendMode::Vm,
    ] {
        let mut backend = RuntimePureAccelerator::new(mode, helper.plan());
        let flat = [21, 3, 99, i128::MIN, -1, 99, 20, 2, 99];
        assert!(matches!(
            backend.call_i128_flat_batch_sum(helper.function_ref(), &flat, 3, 3),
            Err(RuntimeEvalError::UnsupportedPure { reason, .. })
                if reason.contains(&i128::MIN.to_string()) && reason.contains("i64 sum")
        ));
        assert_eq!(
            backend.call_exact_int_slice(helper.function_ref(), &[i128::MIN, -1, 99]),
            Ok(Some(i128::MIN))
        );
        if mode == RuntimePureBackendMode::Jit {
            assert_eq!(backend.stats().pure_calls, 3);
            assert_eq!(backend.stats().jit_calls, 3);
            assert_eq!(
                backend.stats().aot_calls + backend.stats().vm_calls + backend.stats().fallbacks,
                0
            );
            assert_eq!(backend.stats().result_bytes_copied, 0);
        }
    }
}
