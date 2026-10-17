use super::*;
use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::{
    RuntimeCallableParameterSeed, RuntimeExprSeed, RuntimeExprSeedKind,
    RuntimeFunctionDefinitionIdentity, RuntimeFunctionParameterIdentity,
    RuntimeFunctionParameterPassing, RuntimeLocalDeclarationSeed, RuntimeLocalDeclarationSource,
    RuntimeLocalReadSeed, RuntimePlan, RuntimePlanBuilder, RuntimePlanTypeProjection,
    RuntimePlanTypeSeed, RuntimePureHelperOrigin, RuntimePureHelperSeed, RuntimePureInputType as I,
    RuntimePureOutputType as O,
};
use crate::pure::{RuntimePureCallBackend, VmRuntimePureCallBackend};
use crate::value::{
    RuntimeLocalReadMode, RuntimeSignedIntWidth as S, RuntimeUnsignedIntWidth as U,
};
use std::sync::Arc;

fn helper_plan(
    input: I,
    output: O,
    ty: RuntimePlanTypeProjection<RuntimeSemanticTypeId>,
    arity: usize,
    constant: Option<RuntimeValue>,
) -> Arc<RuntimePlan> {
    let semantic = RuntimeSemanticTypeId::from_bytes([0x93; 32]);
    let mut builder = RuntimePlanBuilder::new();
    let parameters = (0..arity)
        .map(|position| {
            RuntimeFunctionParameterIdentity::from_accepted_identity(
                [u8::try_from(position + 1).expect("small fixture arity"); 32],
            )
        })
        .collect::<Vec<_>>();
    let admission = builder
        .admit_type_batch(
            [RuntimePlanTypeSeed::new(semantic, ty)],
            parameters.iter().map(|parameter| {
                RuntimeLocalDeclarationSeed::new(
                    RuntimeLocalDeclarationSource::Parameter(*parameter),
                    semantic,
                )
            }),
        )
        .unwrap();
    let body = if let Some(value) = constant {
        RuntimeExprSeed::new(semantic, RuntimeExprSeedKind::Value(value))
    } else {
        let local = admission.local_ids().first().expect("identity input");
        RuntimeExprSeed::new(
            semantic,
            RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                local.clone(),
                RuntimeLocalReadMode::Copy,
            )),
        )
    };
    builder
        .push_pure_helper_seed(RuntimePureHelperSeed {
            definition: RuntimeFunctionDefinitionIdentity::from_accepted_identity([0x94; 32]),
            name: "numeric_map_identity".to_owned(),
            inputs: admission
                .local_ids()
                .iter()
                .cloned()
                .zip(parameters)
                .map(|(local, identity)| RuntimeCallableParameterSeed {
                    identity,
                    local,
                    passing: RuntimeFunctionParameterPassing::Value,
                    abi: input,
                })
                .collect(),
            output_abi: output,
            body,
            scalar_eval_supported: true,
            origin: RuntimePureHelperOrigin::Annotated,
        })
        .unwrap();
    Arc::new(builder.finish().unwrap())
}

fn projection(
    plan: &Arc<RuntimePlan>,
    inputs: Box<[RuntimeNumericInput]>,
) -> RuntimeNumericProjection<'_> {
    RuntimeNumericProjection {
        function: RuntimePureFunctionRef::resolve(plan, plan.pure_helpers()[0].id).unwrap(),
        inputs,
        forwarding_ops: 0,
    }
}

fn shape<T>(rows: &Rows<T>) -> (bool, usize) {
    match rows {
        Rows::Repeated(values) => {
            assert!(values.capacity() <= crate::pure::RuntimeI64Args::MAX);
            (true, values.len())
        }
        Rows::Dense(values) => (false, values.len()),
    }
}

fn storage(batch: &RuntimeNumericBatch<'_>) -> (bool, usize) {
    match &batch.inputs {
        Inputs::I8(rows) => shape(rows),
        Inputs::I16(rows) => shape(rows),
        Inputs::I32(rows) => shape(rows),
        Inputs::I64(rows) => shape(rows),
        Inputs::I128(rows) => shape(rows),
        Inputs::ISize(rows) => shape(rows),
        Inputs::U8(rows) => shape(rows),
        Inputs::U16(rows) => shape(rows),
        Inputs::U32(rows) => shape(rows),
        Inputs::U64(rows) => shape(rows),
        Inputs::U128(rows) => shape(rows),
        Inputs::USize(rows) => shape(rows),
        Inputs::F32(rows) => shape(rows),
        Inputs::F64(rows) => shape(rows),
    }
}

#[test]
fn all_numeric_widths_keep_complete_uniform_and_dense_rows() {
    use RuntimePlanTypeProjection::{F32, F64, Signed, Unsigned};
    use RuntimePureScalar as V;
    let cases = [
        (I::I8, O::I8, Signed(S::I8), V::I8(-7), V::I8(11)),
        (I::I16, O::I16, Signed(S::I16), V::I16(-700), V::I16(1100)),
        (
            I::I32,
            O::I32,
            Signed(S::I32),
            V::I32(-70_000),
            V::I32(110_000),
        ),
        (
            I::I64,
            O::I64,
            Signed(S::I64),
            V::I64(i64::MIN),
            V::I64(i64::MAX),
        ),
        (
            I::I128,
            O::I128,
            Signed(S::I128),
            V::I128(-(1 << 100)),
            V::I128(1 << 100),
        ),
        (
            I::ISize,
            O::ISize,
            Signed(S::ISize),
            V::ISize(i64::MIN),
            V::ISize(i64::MAX),
        ),
        (I::U8, O::U8, Unsigned(U::U8), V::U8(7), V::U8(u8::MAX)),
        (
            I::U16,
            O::U16,
            Unsigned(U::U16),
            V::U16(700),
            V::U16(u16::MAX),
        ),
        (
            I::U32,
            O::U32,
            Unsigned(U::U32),
            V::U32(70000),
            V::U32(u32::MAX),
        ),
        (
            I::U64,
            O::U64,
            Unsigned(U::U64),
            V::U64(7),
            V::U64(u64::MAX),
        ),
        (
            I::U128,
            O::U128,
            Unsigned(U::U128),
            V::U128(1 << 100),
            V::U128(u128::MAX),
        ),
        (
            I::USize,
            O::USize,
            Unsigned(U::USize),
            V::USize(7),
            V::USize(u64::MAX),
        ),
        (I::F32, O::F32, F32, V::F32(1.25), V::F32(-2.5)),
        (I::F64, O::F64, F64, V::F64(1.25), V::F64(-2.5)),
    ];
    assert_eq!(cases.len(), 14);
    for (input, output, ty, a, b) in cases {
        let nullary = helper_plan(input, output, ty.clone(), 0, Some(a.into_runtime_value()));
        let nullary = self::projection(&nullary, Box::new([]));
        let source = RuntimeValue::Unit;
        let batch = nullary
            .admit(&[&source, &source, &source], false)
            .unwrap()
            .materialize();
        assert_eq!(storage(&batch), (true, 0));
        let mut vm = VmRuntimePureCallBackend::default();
        let RuntimeNumericBatchResult::Values(result) = batch.execute(&mut vm).unwrap() else {
            panic!("nullary collect mode")
        };
        assert_eq!(result, vec![a.into_runtime_value(); 3], "{input:?}");
        assert_eq!(vm.stats().pure_calls, 3);
        assert_eq!(vm.stats().arg_bytes_borrowed, 0);
        let plan = helper_plan(input, output, ty, 2, None);
        let projection = projection(
            &plan,
            Box::new([
                RuntimeNumericInput::Column(0),
                RuntimeNumericInput::Constant(b),
            ]),
        );
        for (values, expected_storage) in [(vec![a, a, a], (true, 2)), (vec![a, b, a], (false, 6))]
        {
            let values = values
                .into_iter()
                .map(RuntimePureScalar::into_runtime_value)
                .collect::<Vec<_>>();
            let refs = values.iter().collect::<Vec<_>>();
            let batch = projection.admit(&refs, false).unwrap().materialize();
            assert_eq!(storage(&batch), expected_storage, "{input:?}");
            assert_eq!(batch.rows(), 3);
            assert!(Arc::ptr_eq(batch.function().plan(), &plan));
            let mut vm = VmRuntimePureCallBackend::default();
            let RuntimeNumericBatchResult::Values(result) = batch.execute(&mut vm).unwrap() else {
                panic!("collect mode")
            };
            assert_eq!(result, values, "{input:?}");
            assert_eq!(vm.stats().pure_calls, 3);
            assert_eq!(vm.stats().batch_calls, 1);
            let RuntimeValue::Seq(dense) =
                crate::value::runtime_sequence_from_literal_values(values.clone())
            else {
                panic!("scalar source sequence")
            };
            assert!(matches!(dense, RuntimeSeq::Dense(_)), "{input:?}");
            for sequence in [RuntimeSeq::Values(values.clone()), dense] {
                let batch = projection
                    .admit_sequence(&sequence, false)
                    .unwrap()
                    .materialize();
                assert_eq!(storage(&batch), expected_storage, "{input:?}");
                assert_eq!(batch.rows(), 3);
                assert!(Arc::ptr_eq(batch.function().plan(), &plan));
                let mut vm = VmRuntimePureCallBackend::default();
                let RuntimeNumericBatchResult::Values(result) = batch.execute(&mut vm).unwrap()
                else {
                    panic!("sequence collect mode")
                };
                assert_eq!(result, values, "{input:?}");
                assert_eq!(vm.stats().pure_calls, 3);
                assert_eq!(vm.stats().batch_calls, 1);
            }
        }
    }
}

#[test]
fn floating_rows_preserve_signed_zero_and_nan_payloads() {
    for (input, output, ty, cases) in [
        (
            I::F32,
            O::F32,
            RuntimePlanTypeProjection::F32,
            vec![
                vec![RuntimePureScalar::F32(0.0), RuntimePureScalar::F32(-0.0)],
                vec![RuntimePureScalar::F32(f32::from_bits(0x7fc0_0001)); 2],
                vec![
                    RuntimePureScalar::F32(f32::from_bits(0x7fc0_0001)),
                    RuntimePureScalar::F32(f32::from_bits(0x7fc0_0002)),
                ],
            ],
        ),
        (
            I::F64,
            O::F64,
            RuntimePlanTypeProjection::F64,
            vec![
                vec![RuntimePureScalar::F64(0.0), RuntimePureScalar::F64(-0.0)],
                vec![RuntimePureScalar::F64(f64::from_bits(0x7ff8_0000_0000_0001)); 2],
                vec![
                    RuntimePureScalar::F64(f64::from_bits(0x7ff8_0000_0000_0001)),
                    RuntimePureScalar::F64(f64::from_bits(0x7ff8_0000_0000_0002)),
                ],
            ],
        ),
    ] {
        let plan = helper_plan(input, output, ty, 1, None);
        let projection = projection(&plan, Box::new([RuntimeNumericInput::Column(0)]));
        for (index, values) in cases.into_iter().enumerate() {
            let values = values
                .into_iter()
                .map(RuntimePureScalar::into_runtime_value)
                .collect::<Vec<_>>();
            let refs = values.iter().collect::<Vec<_>>();
            let batch = projection.admit(&refs, false).unwrap().materialize();
            assert_eq!(
                storage(&batch),
                if index == 1 { (true, 1) } else { (false, 2) }
            );
            let mut vm = VmRuntimePureCallBackend::default();
            let RuntimeNumericBatchResult::Values(result) = batch.execute(&mut vm).unwrap() else {
                panic!("collect mode")
            };
            for (actual, expected) in result.iter().zip(&values) {
                match (actual, expected) {
                    (RuntimeValue::F32(actual), RuntimeValue::F32(expected)) => {
                        assert_eq!(actual.to_bits(), expected.to_bits());
                    }
                    (RuntimeValue::F64(actual), RuntimeValue::F64(expected)) => {
                        assert_eq!(actual.to_bits(), expected.to_bits());
                    }
                    _ => panic!("the original float ABI is retained"),
                }
            }
        }
    }
}

#[test]
fn repeated_i64_reduction_retains_one_row_and_executes_the_full_count() {
    let plan = helper_plan(
        I::I64,
        O::I64,
        RuntimePlanTypeProjection::Signed(S::I64),
        2,
        None,
    );
    let projection = projection(
        &plan,
        Box::new([
            RuntimeNumericInput::Column(0),
            RuntimeNumericInput::Constant(RuntimePureScalar::I64(9)),
        ]),
    );
    let values = vec![RuntimeValue::i64(i64::MAX); 4096];
    let refs = values.iter().collect::<Vec<_>>();
    let batch = projection.admit(&refs, true).unwrap().materialize();
    assert_eq!(storage(&batch), (true, 2));
    let mut vm = VmRuntimePureCallBackend::default();
    let RuntimeNumericBatchResult::Sum(result) = batch.execute(&mut vm).unwrap() else {
        panic!("sum mode")
    };
    assert_eq!(result, -4096);
    assert_eq!(vm.stats().pure_calls, 4096);
    assert_eq!(vm.stats().arg_bytes_borrowed, 16);
    assert_eq!(
        vm.stats().arg_vec_allocations
            + vm.stats().arg_bytes_copied
            + vm.stats().result_bytes_copied,
        0
    );
}

#[test]
fn admission_rejects_missing_and_out_of_range_columns_before_materialization() {
    let plan = helper_plan(
        I::I64,
        O::I64,
        RuntimePlanTypeProjection::Signed(S::I64),
        1,
        None,
    );
    let value = RuntimeValue::i64(1);
    for column in [1, crate::pure::RuntimeI64Args::MAX, usize::MAX] {
        let projection = projection(&plan, Box::new([RuntimeNumericInput::Column(column)]));
        assert!(projection.admit(&[&value], false).is_none());
        assert!(projection.admit(&[], false).is_none());
    }
}

#[test]
fn admission_rejects_late_wrong_items_and_unused_constants_before_backend_work() {
    let plan = helper_plan(
        I::I64,
        O::I64,
        RuntimePlanTypeProjection::Signed(S::I64),
        2,
        None,
    );
    let mut projection = projection(
        &plan,
        Box::new([
            RuntimeNumericInput::Column(0),
            RuntimeNumericInput::Constant(RuntimePureScalar::I64(9)),
        ]),
    );
    let good = RuntimeValue::i64(1);
    let wrong = RuntimeValue::F64(1.0);
    assert!(projection.admit(&[&good, &good, &wrong], true).is_none());
    projection.inputs[1] = RuntimeNumericInput::Constant(RuntimePureScalar::U64(9));
    assert!(projection.admit(&[&good], false).is_none());
    assert!(projection.admit(&[], false).is_none());
    projection.inputs = Box::new([RuntimeNumericInput::Column(0)]);
    assert!(projection.admit(&[&good], false).is_none());
    let floats = helper_plan(I::F64, O::F64, RuntimePlanTypeProjection::F64, 1, None);
    assert!(
        self::projection(&floats, Box::new([RuntimeNumericInput::Column(0)]))
            .admit(&[&wrong], true)
            .is_none()
    );
}

#[test]
fn unused_source_items_zero_formals_and_empty_batches_keep_their_complete_headers() {
    let constant = helper_plan(
        I::I64,
        O::I64,
        RuntimePlanTypeProjection::Signed(S::I64),
        1,
        None,
    );
    let constant = self::projection(
        &constant,
        Box::new([RuntimeNumericInput::Constant(RuntimePureScalar::I64(9))]),
    );
    let source = RuntimeValue::String("unused source item".to_owned());
    let batch = constant
        .admit(&[&source, &source], false)
        .unwrap()
        .materialize();
    let mut vm = VmRuntimePureCallBackend::default();
    let RuntimeNumericBatchResult::Values(values) = batch.execute(&mut vm).unwrap() else {
        panic!("collect mode")
    };
    assert_eq!(values, [RuntimeValue::i64(9), RuntimeValue::i64(9)]);
    assert_eq!(vm.stats().pure_calls, 2);
    let plan = helper_plan(
        I::I64,
        O::I64,
        RuntimePlanTypeProjection::Signed(S::I64),
        0,
        Some(RuntimeValue::i64(7)),
    );
    let projection = projection(&plan, Box::new([]));
    let source = RuntimeValue::String("unused source item".to_owned());
    let batch = projection
        .admit(&[&source, &source, &source], true)
        .unwrap()
        .materialize();
    assert_eq!(storage(&batch), (true, 0));
    let mut vm = VmRuntimePureCallBackend::default();
    let RuntimeNumericBatchResult::Sum(result) = batch.execute(&mut vm).unwrap() else {
        panic!("sum mode")
    };
    assert_eq!(result, 21);
    assert_eq!(vm.stats().pure_calls, 3);
    assert_eq!(vm.stats().arg_bytes_borrowed, 0);
    for fused in [false, true] {
        let batch = projection.admit(&[], fused).unwrap().materialize();
        let mut vm = VmRuntimePureCallBackend::default();
        match batch.execute(&mut vm).unwrap() {
            RuntimeNumericBatchResult::Sum(value) => assert_eq!(value, 0),
            RuntimeNumericBatchResult::Values(values) => assert!(values.is_empty()),
        }
        assert_eq!(vm.stats(), crate::step::RuntimePureCallStats::default());
    }
}

#[test]
fn wide_sum_admission_keeps_all_rows_and_narrowing_failures() {
    for (input, output, ty, fits, outside, expected_sum) in [
        (
            I::U64,
            O::U64,
            RuntimePlanTypeProjection::Unsigned(U::U64),
            RuntimePureScalar::U64(i64::MAX as u64),
            RuntimePureScalar::U64(i64::MAX as u64 + 1),
            i64::MAX,
        ),
        (
            I::I128,
            O::I128,
            RuntimePlanTypeProjection::Signed(S::I128),
            RuntimePureScalar::I128(i128::from(i64::MAX)),
            RuntimePureScalar::I128(i128::from(i64::MAX) + 1),
            i64::MAX,
        ),
        (
            I::I128,
            O::I128,
            RuntimePlanTypeProjection::Signed(S::I128),
            RuntimePureScalar::I128(i128::from(i64::MIN)),
            RuntimePureScalar::I128(i128::from(i64::MIN) - 1),
            i64::MIN,
        ),
        (
            I::U128,
            O::U128,
            RuntimePlanTypeProjection::Unsigned(U::U128),
            RuntimePureScalar::U128(i64::MAX as u128),
            RuntimePureScalar::U128(u128::MAX),
            i64::MAX,
        ),
        (
            I::USize,
            O::USize,
            RuntimePlanTypeProjection::Unsigned(U::USize),
            RuntimePureScalar::USize(i64::MAX as u64),
            RuntimePureScalar::USize(i64::MAX as u64 + 1),
            i64::MAX,
        ),
    ] {
        let plan = helper_plan(input, output, ty, 1, None);
        let projection = projection(&plan, Box::new([RuntimeNumericInput::Column(0)]));
        for (value, expect_sum) in [(fits, true), (outside, false)] {
            let value = value.into_runtime_value();
            let references = [&value, &value, &value];
            let admission = projection.admit(&references, true).unwrap();
            assert_eq!(admission.is_sum(), expect_sum, "{input:?}");
            let mut vm = VmRuntimePureCallBackend::default();
            let result = admission.materialize().execute(&mut vm).unwrap();
            assert_eq!(vm.stats().pure_calls, 3, "{input:?}");
            if expect_sum {
                let RuntimeNumericBatchResult::Sum(result) = result else {
                    panic!("proved Sum")
                };
                assert_eq!(result, expected_sum.wrapping_mul(3));
                assert_eq!(vm.stats().result_bytes_copied, 0);
            } else {
                let RuntimeNumericBatchResult::Values(values) = result else {
                    panic!("unproven collection")
                };
                assert_eq!(values, vec![value; 3]);
                assert!(matches!(
                    crate::value::sum_i64_sequence_ref(&values),
                    Err(RuntimeEvalError::UnsupportedBinary { op: "+", .. })
                ));
            }
        }
    }
}
