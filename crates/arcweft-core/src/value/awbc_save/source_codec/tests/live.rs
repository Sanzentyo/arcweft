use super::*;
use crate::value::{
    Progress, RecordSeq, RuntimeAgentPredicate, RuntimeAgentValue, RuntimeScalarView, RuntimeSeq,
    RuntimeValue as Live, RuntimeValueView, TupleSeq,
};

fn parity(value: &Live) {
    let snapshot = Value::from_admitted_storage(value, None).unwrap();
    let resources = resources();
    let mut live_bank = meter();
    let mut snapshot_bank = meter();
    let live = value
        .prepare_literal_source_encoding(limits(), &resources, &mut live_bank)
        .unwrap();
    let inert = snapshot
        .prepare_literal_source_encoding(limits(), &resources, &mut snapshot_bank)
        .unwrap();
    assert_eq!(live.encoded_bytes(), inert.encoded_bytes());
    assert_eq!(
        (live.storage_bytes(), live.elements(), live.source_nodes()),
        (
            inert.storage_bytes(),
            inert.elements(),
            inert.source_nodes()
        )
    );
    assert_eq!(live_bank.totals(), snapshot_bank.totals());
    let mut actual = Vec::with_capacity(live.encoded_bytes());
    live.append_to(&mut actual).unwrap();
    let mut expected = Vec::with_capacity(inert.encoded_bytes());
    inert.append_to(&mut expected).unwrap();
    assert_eq!(actual, expected);
    let decoded = Value::preflight_literal_source(&actual, limits(), &resources, &mut live_bank)
        .unwrap()
        .materialize(&mut live_bank)
        .unwrap();
    // Re-encoding compares exact bits even when ordinary floating equality is false.
    assert_eq!(encode(&decoded), expected);
}

#[test]
fn borrowed_live_source_preserves_scalar_bits_and_recursive_agent_data() {
    let mut values = vec![
        Live::Unit,
        Live::Bool(true),
        Live::String("物語\0".into()),
        Live::F32(f32::from_bits(0x7f80_0123)),
        Live::F64(f64::from_bits(0x7ff0_0000_0000_0123)),
        Live::F32(-0.0),
        Live::F64(-0.0),
        Live::Color(RuntimeColor::new(0, 17, 128, 255)),
        Live::Char('🦀'),
        Live::Duration(LogicalDuration::from_nanos(u64::MAX)),
        Live::Progress(Progress::new(-0.0).unwrap().with_label("開始")),
        Live::EntityRef(RuntimeEntityReference::StructuralFlow(
            crate::plan::FlowRuntimeId::from_runtime_contract("main.child", "public.selector")
                .unwrap(),
        )),
        Live::Range(RuntimeRange::Int {
            start: Some(RuntimeInt::I16(-2)),
            end: Some(RuntimeInt::I16(4)),
            inclusive: true,
        }),
        Live::Iterator(crate::value::RuntimeIterator::Values {
            items: vec![Live::Bool(false), Live::String("next".into())].into(),
        }),
    ];
    values.extend(
        [
            RuntimeInt::I8(i8::MIN),
            RuntimeInt::I16(i16::MIN),
            RuntimeInt::I32(i32::MIN),
            RuntimeInt::I64(i64::MIN),
            RuntimeInt::I128(i128::MIN),
            RuntimeInt::ISize(i64::MIN),
        ]
        .map(Live::Int),
    );
    values.extend(
        [
            RuntimeUInt::U8(u8::MAX),
            RuntimeUInt::U16(u16::MAX),
            RuntimeUInt::U32(u32::MAX),
            RuntimeUInt::U64(u64::MAX),
            RuntimeUInt::U128(u128::MAX),
            RuntimeUInt::USize(u64::MAX),
        ]
        .map(Live::UInt),
    );
    let predicate = RuntimeAgentPredicate::try_all(vec![
        RuntimeAgentPredicate::Compare {
            probe: RuntimeAgentProbe::StatePath {
                path: RuntimeAgentPath::try_new("state.score").unwrap(),
            },
            op: RuntimeAgentCompareOp::Greater,
            value: Box::new(Live::Int(RuntimeInt::I8(-3))),
        },
        RuntimeAgentPredicate::Not {
            predicate: Box::new(RuntimeAgentPredicate::DiagnosticsHasError),
        },
        RuntimeAgentPredicate::try_any(vec![RuntimeAgentPredicate::Exists {
            probe: RuntimeAgentProbe::Metric {
                target: crate::entry::RuntimeCommandTargetId::try_new("scene.metric").unwrap(),
            },
        }])
        .unwrap(),
    ])
    .unwrap();
    values.push(Live::Agent(RuntimeAgentValue::Predicate(predicate)));
    values.push(Live::Agent(RuntimeAgentValue::ViewportPoint {
        x: u32::MAX,
        y: 0,
    }));
    parity(&Live::Tuple(values));
}

#[test]
fn borrowed_live_source_preserves_all_dense_kinds_and_physical_columns() {
    let dense = dense_values();
    assert_eq!(dense.len(), 21);
    for value in dense {
        parity(&Live::Seq(RuntimeSeq::Dense(value)));
    }
    let records = RecordSeq::try_from_accepted_fields(
        2,
        vec![(
            "payload".into(),
            RuntimeSeq::Values(vec![Live::Bool(true), Live::Bool(false)]),
        )],
    )
    .unwrap();
    let columns = TupleSeq::new(
        2,
        vec![
            RuntimeSeq::Dense(DenseSeq::I16(DenseSeqStorage::new(vec![-7, 8]))),
            RuntimeSeq::RecordColumns(records),
        ],
    )
    .unwrap();
    parity(&Live::Seq(RuntimeSeq::TupleColumns(columns)));
    parity(&Live::Record(
        crate::value::RuntimeRecordValue::try_new(vec![(
            "field".into(),
            Live::Tuple(vec![Live::Unit]),
        )])
        .unwrap(),
    ));
}

#[test]
fn borrowed_live_source_preserves_all_numeric_containers_and_float_payloads() {
    use crate::math::{DenseMatrixF32, DenseMatrixF64, DenseTensorF32, DenseTensorF64};
    let f32s = || vec![-0.0, f32::from_bits(0x7f80_0123)];
    let f64s = || vec![-0.0, f64::from_bits(0x7ff0_0000_0000_0123)];
    for value in [
        Live::MatrixF32(DenseMatrixF32::new(1, 2, f32s()).unwrap()),
        Live::MatrixF64(DenseMatrixF64::new(1, 2, f64s()).unwrap()),
        Live::TensorF32(DenseTensorF32::new(vec![1, 2, 1], f32s()).unwrap()),
        Live::TensorF64(DenseTensorF64::new(vec![1, 2, 1], f64s()).unwrap()),
    ] {
        parity(&value);
    }
}

#[test]
fn borrowed_live_source_preserves_static_opaque_reductions_and_builtin_variants() {
    use crate::pattern::{
        RuntimeOpaqueTypeOwner, RuntimeOpaqueTypeProducerId, RuntimeSemanticTypeId,
    };
    use crate::value::awbc_save::{
        AwbcRuntimeCommandSnapshot, AwbcRuntimeOpaqueSnapshot, AwbcRuntimeReductionSnapshot,
    };
    use crate::value::{RuntimeOpaquePersistence, RuntimeOpaqueValueClass};
    // These context-free data fixtures enter through the actual standalone
    // snapshot admission, without creating a program owner or executable proof.
    for snapshot in [
        Value::Opaque(AwbcRuntimeOpaqueSnapshot {
            producer: RuntimeOpaqueTypeProducerId::try_new("fixture.Box").unwrap(),
            semantic_identity: RuntimeSemanticTypeId::from_bytes([5; 32]),
            value_class: RuntimeOpaqueValueClass::Plain,
            persistence: RuntimeOpaquePersistence::ConstantAndSnapshot,
            payload: Box::new(Value::Bool(true)),
        }),
        Value::Reduction(AwbcRuntimeReductionSnapshot {
            owner: RuntimeOpaqueTypeOwner::exact(
                RuntimeOpaqueTypeProducerId::try_new("std.reduction").unwrap(),
                RuntimeSemanticTypeId::from_bytes([6; 32]),
            ),
            state: Box::new(Value::String("state".into())),
            commands: vec![AwbcRuntimeCommandSnapshot {
                constructor: crate::entry::RuntimeCommandConstructorId::try_new(
                    "presentation.show",
                )
                .unwrap(),
                target: crate::entry::RuntimeCommandTargetId::try_new("scene.object").unwrap(),
                payload: Box::new(Value::Tuple(vec![Value::Bool(false)])),
            }],
        }),
    ] {
        parity(&snapshot.into_unrestricted_standalone_value().unwrap());
    }
    for value in [
        Live::option_none(),
        Live::option_some(Live::Bool(true)),
        Live::result_ok(Live::String("ok".into())),
        Live::result_err(Live::String("err".into())),
    ] {
        parity(&value);
    }
}

#[test]
fn borrowed_live_source_keeps_original_storage_and_the_precharged_sticky_bank() {
    let text = "borrowed original".to_owned();
    let original = text.as_ptr();
    let value = Live::Tuple(vec![Live::String(text), Live::Bool(true)]);
    let mut measured = meter();
    let prepared = value
        .prepare_literal_source_encoding(limits(), &resources(), &mut measured)
        .unwrap();
    let RuntimeValueView::Tuple(fields) = prepared.value else {
        unreachable!()
    };
    let Some(RuntimeValueView::Scalar(RuntimeScalarView::String(borrowed))) = fields.get(0) else {
        unreachable!()
    };
    assert_eq!(borrowed.as_ptr(), original);
    let encoded = prepared.encoded_bytes();
    let storage = prepared.storage_bytes();
    let elements = prepared.elements();
    for short in [
        RuntimeLiteralSourceLimits {
            encoded_bytes: encoded - 1,
            ..limits()
        },
        RuntimeLiteralSourceLimits {
            decoded_bytes: storage - 1,
            ..limits()
        },
        RuntimeLiteralSourceLimits {
            elements: u32::try_from(elements - 1).unwrap(),
            ..limits()
        },
    ] {
        let mut bank = meter();
        assert!(
            value
                .prepare_literal_source_encoding(short, &resources(), &mut bank)
                .is_err()
        );
        assert_eq!(bank.status(), Err(TaskSemanticEncodingError::OwnerRejected));
    }
    let work = measured.totals().0;
    let mut shared = TaskSemanticMeter::new(work * 2 + 1, u64::MAX);
    shared.charge_work(1).unwrap();
    for _ in 0..2 {
        value
            .prepare_literal_source_encoding(limits(), &resources(), &mut shared)
            .unwrap();
    }
    assert_eq!(shared.totals().0, work * 2 + 1);
    assert!(matches!(
        value.prepare_literal_source_encoding(limits(), &resources(), &mut shared),
        Err(RuntimeLiteralSourceError::Resource(
            TaskSemanticEncodingError::SemanticWork
        ))
    ));
    assert_eq!(
        shared.status(),
        Err(TaskSemanticEncodingError::SemanticWork)
    );
    let snapshot = Value::from_admitted_storage(&value, None).unwrap();
    assert!(matches!(
        snapshot.prepare_literal_source_encoding(limits(), &resources(), &mut shared),
        Err(RuntimeLiteralSourceError::Resource(
            TaskSemanticEncodingError::SemanticWork
        ))
    ));
    let mut output = Vec::with_capacity(2);
    output.extend_from_slice(&[9, 8]);
    assert!(matches!(
        prepared.append_to(&mut output),
        Err(RuntimeLiteralSourceError::OutputCapacity { .. })
    ));
    assert_eq!(output, [9, 8]);
}
