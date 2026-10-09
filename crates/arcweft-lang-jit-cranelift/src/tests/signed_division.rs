use super::*;
use arcweft_core::value::RuntimeExpressionFailure;

fn division_request(scalar: Scalar, executable: bool) -> PureFunctionRequest {
    let body = |ids: &[RuntimeLocalSeedId]| {
        binary(
            scalar,
            local(scalar, ids[0].clone()),
            RuntimeBinaryOp::Div,
            local(scalar, ids[1].clone()),
        )
    };
    if executable {
        ordinary_executable_request(scalar, |ids| {
            vec![arcweft_core::plan::RuntimeFlowOpSeed::ReturnExpr(body(ids))]
        })
    } else {
        ordinary_scope_request(scalar, body)
    }
}

fn checked_scalar_object_symbols(object: &ObjectPureInputs) {
    use cranelift_object::object::{Object, ObjectSymbol};
    assert_object_symbols(object);
    let parsed = cranelift_object::object::File::parse(object.object_bytes.as_slice()).unwrap();
    let symbols = parsed
        .symbols()
        .filter_map(|symbol| symbol.name().ok())
        .collect::<Vec<_>>();
    assert!(has_symbol(
        &symbols,
        object
            .batch_sum_symbol
            .as_deref()
            .expect("checked sum symbol")
    ));
}

macro_rules! signed_division_case {
    ($name:ident, $scalar:ident, $ty:ty, $compile:ident, $emit:ident, $check:ident, $wrap:expr) => {
        #[test]
        fn $name() {
            let request = division_request(Scalar::$scalar, true);
            let function = request.function_ref().unwrap();
            assert!(request.plan().pure_helpers().is_empty());
            assert!(function.body.is_executable());
            assert!(!function.supports_total_numeric_completion());
            let locals = function
                .inputs
                .iter()
                .map(arcweft_core::pure::RuntimePureFunctionInputRef::local)
                .collect::<Vec<_>>();
            assert_eq!(
                locals.len(),
                3,
                "the unused original formal remains present"
            );
            let compiled = CraneliftPureFunctionBackend
                .$compile(&request, locals.iter().copied())
                .expect("exact signed division uses the checked native ABI");
            let rows: [([$ty; 3], $ty); 5] = [
                ([21, 3, 99], 7),
                ([-21, 3, 99], -7),
                ([21, -3, 99], -7),
                ([<$ty>::MIN, -1, 99], <$ty>::MIN),
                ([<$ty>::MAX, -2, 99], <$ty>::MAX.wrapping_div(-2)),
            ];
            for (arguments, expected) in rows {
                assert!(compiled.call(&arguments[..2]).is_err());
                assert_eq!(compiled.call(&arguments).unwrap(), expected);
                let values = arguments.into_iter().map($wrap).collect::<Vec<_>>();
                assert_eq!(
                    vm_executable_result(&request, values.clone()).unwrap(),
                    $wrap(expected)
                );
                assert_eq!(
                    aot_scalar_body_result(&request, &values).unwrap(),
                    $wrap(expected)
                );
            }
            let zero = [12, 0, 99];
            let values = zero.into_iter().map($wrap).collect::<Vec<_>>();
            assert_eq!(
                aot_scalar_body_result(&request, &values),
                Err(RuntimeEvalError::RecoverableExpression(
                    RuntimeExpressionFailure::DivisionByZero
                ))
            );
            // The owning synchronous expression evaluator retains the exact
            // failure type; the standalone Engine facade reports diagnostics.
            let expression_request = division_request(Scalar::$scalar, false);
            let mut scratch = arcweft_core::pure::VmPureFunctionScratch::default();
            assert_eq!(
                scratch.evaluate_values(
                    expression_request.plan(),
                    expression_request.function_id(),
                    values
                ),
                Err(RuntimeEvalError::RecoverableExpression(
                    RuntimeExpressionFailure::DivisionByZero
                ))
            );
            assert!(matches!(
                compiled.call(&zero),
                Err(CraneliftCodegenError::Execution {
                    error: RuntimeEvalError::RecoverableExpression(
                        RuntimeExpressionFailure::DivisionByZero
                    ),
                    completed_rows: 0,
                })
            ));
            let flat = [21, 3, 99, 7, 0, 99, 20, 2, 99];
            let mut output = [77; 3];
            assert!(matches!(
                compiled.call_flat_batch(&flat, &mut output),
                Err(CraneliftCodegenError::Execution {
                    error: RuntimeEvalError::RecoverableExpression(
                        RuntimeExpressionFailure::DivisionByZero
                    ),
                    completed_rows: 1,
                })
            ));
            assert_eq!(
                output,
                [7, 77, 77],
                "failed row and suffix publish no values"
            );
            assert!(matches!(
                compiled.call_flat_batch_sum(&flat, 3),
                Err(CraneliftCodegenError::Execution {
                    error: RuntimeEvalError::RecoverableExpression(
                        RuntimeExpressionFailure::DivisionByZero
                    ),
                    completed_rows: 1,
                })
            ));
            assert_eq!(
                compiled
                    .call_flat_batch_sum(&[21, 3, 99, 20, 2, 99], 2)
                    .unwrap(),
                17
            );
            assert_eq!(
                compiled.call(&[20, 2, 99]).unwrap(),
                10,
                "a fault does not poison code"
            );
            assert_eq!(compiled.call_flat_batch_sum(&[], 0).unwrap(), 0);
            let object = CraneliftPureFunctionBackend
                .$emit(&request, locals.iter().copied())
                .expect("object emission uses the same checked signed lowerer");
            let repeated = CraneliftPureFunctionBackend
                .$emit(&request, locals.iter().copied())
                .unwrap();
            $check(&object);
            assert_eq!(object.input_locals, locals);
            assert_eq!(object.object_bytes, repeated.object_bytes);
            assert_eq!(NATIVE_PURE_CALL_ABI_VERSION, 1);
        }
    };
}

signed_division_case!(
    signed_i8_scalar_rows_sum_and_object_preserve_checked_outcomes,
    I8,
    i8,
    compile_i8_with_inputs,
    emit_object_i8_with_inputs,
    checked_scalar_object_symbols,
    RuntimeValue::i8
);
signed_division_case!(
    signed_i16_scalar_rows_sum_and_object_preserve_checked_outcomes,
    I16,
    i16,
    compile_i16_with_inputs,
    emit_object_i16_with_inputs,
    checked_scalar_object_symbols,
    RuntimeValue::i16
);
signed_division_case!(
    signed_i32_scalar_rows_sum_and_object_preserve_checked_outcomes,
    I32,
    i32,
    compile_i32_with_inputs,
    emit_object_i32_with_inputs,
    checked_scalar_object_symbols,
    RuntimeValue::i32
);
signed_division_case!(
    signed_i64_scalar_rows_sum_and_object_preserve_checked_outcomes,
    I64,
    i64,
    compile_i64_with_inputs,
    emit_object_i64_with_inputs,
    checked_scalar_object_symbols,
    RuntimeValue::i64
);
signed_division_case!(
    signed_isize_scalar_rows_sum_and_object_preserve_checked_outcomes,
    ISize,
    i64,
    compile_i64_with_inputs,
    emit_object_i64_with_inputs,
    checked_scalar_object_symbols,
    RuntimeValue::isize
);
signed_division_case!(
    signed_i128_scalar_rows_sum_and_object_preserve_checked_outcomes,
    I128,
    i128,
    compile_i128_batch_with_inputs,
    emit_object_i128_batch_with_inputs,
    assert_batch_symbols,
    RuntimeValue::i128
);

#[test]
fn signed_i128_division_preserves_full_magnitudes_and_exact_sum_refusal() {
    let request = division_request(Scalar::I128, true);
    let function = request.function_ref().unwrap();
    let compiled = CraneliftPureFunctionBackend
        .compile_i128_batch_with_inputs(
            &request,
            function
                .inputs
                .iter()
                .map(arcweft_core::pure::RuntimePureFunctionInputRef::local),
        )
        .unwrap();
    let mut state = 0x9e37_79b9_7f4a_7c15_6a09_e667_f3bc_c909_u128;
    let mut rows = vec![
        (i128::MIN, 1),
        (i128::MIN, 2),
        (i128::MIN, -2),
        (i128::MIN, i128::MIN),
        (i128::MAX, i128::MIN),
        (i128::MAX, 3),
        ((1_i128 << 100) + 123, (1_i128 << 65) - 7),
        (-((1_i128 << 100) + 123), (1_i128 << 65) - 7),
    ];
    for _ in 0..64 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let lhs = i128::from_ne_bytes(state.to_ne_bytes());
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let rhs = i128::from_ne_bytes(state.to_ne_bytes());
        rows.push((lhs, if rhs == 0 { 1 } else { rhs }));
    }
    for (lhs, rhs) in rows {
        let values = vec![
            RuntimeValue::i128(lhs),
            RuntimeValue::i128(rhs),
            RuntimeValue::i128(99),
        ];
        let expected = vm_executable_result(&request, values.clone()).unwrap();
        assert_eq!(aot_scalar_body_result(&request, &values).unwrap(), expected);
        assert_eq!(
            RuntimeValue::i128(compiled.call(&[lhs, rhs, 99]).unwrap()),
            expected
        );
    }
    for wide in [i128::MIN, i128::from(i64::MAX) + 1, i128::MAX] {
        assert_eq!(compiled.call(&[wide, 1, 99]).unwrap(), wide);
        assert!(matches!(
            compiled.call_flat_batch_sum(&[21, 3, 99, wide, 1, 99, 20, 2, 99], 3),
            Err(CraneliftCodegenError::Execution {
                error: RuntimeEvalError::UnsupportedPure { reason, .. },
                completed_rows: 1,
            }) if reason.contains(&wide.to_string()) && reason.contains("i64 sum")
        ));
    }
}

#[test]
fn signed_i64_sum_preserves_owning_wrapping_accumulation() {
    let request = division_request(Scalar::I64, true);
    let compiled = CraneliftPureFunctionBackend
        .compile_i64_with_inputs(
            &request,
            request
                .function_ref()
                .unwrap()
                .inputs
                .iter()
                .map(arcweft_core::pure::RuntimePureFunctionInputRef::local),
        )
        .unwrap();
    assert_eq!(
        compiled
            .call_flat_batch_sum(&[i64::MIN, -1, 99, 1, 1, 99], 2)
            .unwrap(),
        i64::MIN.wrapping_add(1)
    );
    assert_eq!(
        compiled
            .call_flat_batch_sum(&[i64::MAX, 1, 99, 1, 1, 99], 2)
            .unwrap(),
        i64::MIN
    );
}

#[test]
fn signed_division_faults_only_on_the_selected_ordinary_body_branch() {
    for scalar in [Scalar::I64, Scalar::I128] {
        let request = ordinary_executable_request(scalar, |ids| {
            vec![arcweft_core::plan::RuntimeFlowOpSeed::If {
                condition: compare(
                    local(scalar, ids[0].clone()),
                    RuntimeBinaryOp::Ge,
                    value(scalar, scalar_number(scalar, 0)),
                ),
                then_ops: vec![arcweft_core::plan::RuntimeFlowOpSeed::ReturnExpr(binary(
                    scalar,
                    local(scalar, ids[0].clone()),
                    RuntimeBinaryOp::Div,
                    local(scalar, ids[1].clone()),
                ))],
                else_ops: vec![arcweft_core::plan::RuntimeFlowOpSeed::ReturnExpr(value(
                    scalar,
                    scalar_number(scalar, 7),
                ))],
            }]
        });
        let locals = request
            .function_ref()
            .unwrap()
            .inputs
            .iter()
            .map(arcweft_core::pure::RuntimePureFunctionInputRef::local)
            .collect::<Vec<_>>();
        macro_rules! check {
            ($compile:ident, $wrap:expr) => {{
                let compiled = CraneliftPureFunctionBackend
                    .$compile(&request, locals.iter().copied())
                    .unwrap();
                let inputs = [-1, 0, 99];
                assert_eq!(compiled.call(&inputs).unwrap(), 7);
                let values = inputs.into_iter().map($wrap).collect::<Vec<_>>();
                assert_eq!(
                    vm_executable_result(&request, values.clone()).unwrap(),
                    $wrap(7)
                );
                assert_eq!(aot_scalar_body_result(&request, &values).unwrap(), $wrap(7));
                assert!(matches!(
                    compiled.call(&[12, 0, 99]),
                    Err(CraneliftCodegenError::Execution {
                        error: RuntimeEvalError::RecoverableExpression(
                            RuntimeExpressionFailure::DivisionByZero
                        ),
                        completed_rows: 0,
                    })
                ));
            }};
        }
        match scalar {
            Scalar::I64 => check!(compile_i64_with_inputs, RuntimeValue::i64),
            Scalar::I128 => check!(compile_i128_batch_with_inputs, RuntimeValue::i128),
            _ => unreachable!("the selected-branch fixture declares only exact i64/i128"),
        }
    }
}
