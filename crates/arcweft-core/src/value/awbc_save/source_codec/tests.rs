use super::super::{
    AwbcRuntimeAgentPredicateSnapshot as Predicate, AwbcRuntimeAgentSnapshot as Agent,
    AwbcRuntimeFieldSnapshot, AwbcRuntimeIteratorSnapshot as IteratorSource,
    AwbcRuntimeRecordSeqFieldSnapshot, AwbcRuntimeSeqSnapshot as Sequence,
    AwbcRuntimeValueSnapshot as Value,
};
use super::*;

mod live;
mod metadata;
use crate::entry::schema::value_encoding::ValueAdmissionOrigin;
use crate::time::LogicalDuration;
use crate::value::{
    AgentPredicateOperands, DenseSeq, DenseSeqStorage, RuntimeAgentAction,
    RuntimeAgentActionDispatch, RuntimeAgentActionTarget, RuntimeAgentCaptureTarget,
    RuntimeAgentCompareOp, RuntimeAgentPath, RuntimeAgentProbe, RuntimeColor,
    RuntimeEntityReference, RuntimeISizeValue, RuntimeInt, RuntimeRange, RuntimeRangeIterator,
    RuntimeRecordFieldId, RuntimeSignedIntWidth, RuntimeUInt, RuntimeUSizeValue,
    RuntimeUnsignedIntWidth,
};

fn limits() -> RuntimeLiteralSourceLimits {
    RuntimeLiteralSourceLimits {
        encoded_bytes: 1 << 23,
        decoded_bytes: 1 << 23,
        elements: 100_000,
    }
}

fn resources() -> ValueResourceAdmission {
    ValueResourceAdmission::snapshot(
        RuntimeSchemaLimits::engine_default(),
        ValueAdmissionOrigin::InertSnapshot,
    )
}

fn meter() -> TaskSemanticMeter {
    TaskSemanticMeter::new(u64::MAX, u64::MAX)
}

fn encode(value: &Value) -> Vec<u8> {
    let prepared = value
        .prepare_literal_source_encoding(limits(), &resources(), &mut meter())
        .unwrap();
    let mut bytes = Vec::with_capacity(prepared.encoded_bytes());
    prepared.append_to(&mut bytes).unwrap();
    bytes
}

fn roundtrip(value: Value) {
    let mut bank = meter();
    let resources = resources();
    let prepared = value
        .prepare_literal_source_encoding(limits(), &resources, &mut bank)
        .unwrap();
    let footprint = (
        prepared.storage_bytes(),
        prepared.elements(),
        prepared.source_nodes(),
    );
    let mut bytes = Vec::with_capacity(prepared.encoded_bytes());
    prepared.append_to(&mut bytes).unwrap();
    let source = Value::preflight_literal_source(&bytes, limits(), &resources, &mut bank).unwrap();
    assert_eq!(
        (
            source.storage_bytes(),
            source.elements(),
            source.source_nodes()
        ),
        footprint
    );
    let decoded = source.materialize(&mut bank).unwrap();
    assert_eq!(decoded, value);
    // Equality of some domain values intentionally omits presentation metadata.
    // Re-encoding checks every retained field and exact floating-point bits.
    assert_eq!(encode(&decoded), bytes);
}

fn rejected(bytes: &[u8]) -> RuntimeLiteralSourceError {
    let mut bank = meter();
    let error = Value::preflight_literal_source(bytes, limits(), &resources(), &mut bank)
        .err()
        .expect("invalid frame");
    assert_eq!(bank.status(), Err(TaskSemanticEncodingError::OwnerRejected));
    assert!(matches!(
        Value::preflight_literal_source(&encode(&Value::Unit), limits(), &resources(), &mut bank),
        Err(RuntimeLiteralSourceError::Resource(
            TaskSemanticEncodingError::OwnerRejected
        ))
    ));
    error
}

#[test]
fn literal_source_scalars_preserve_all_widths_float_bits_and_exhausted_cursors() {
    let mut values = vec![
        Value::Unit,
        Value::Bool(false),
        Value::Bool(true),
        Value::String("物語\0".into()),
        Value::Color(RuntimeColor::new(0, 17, 128, 255)),
        Value::Char('🦀'),
        Value::Duration(LogicalDuration::from_nanos(u64::MAX)),
        Value::Progress {
            ratio: -0.0,
            label: Some("開始".into()),
        },
        Value::Progress {
            ratio: 1.0,
            label: None,
        },
        Value::F32(0x8000_0000),
        Value::F32(0x7fc0_1234),
        Value::F64(0x8000_0000_0000_0000),
        Value::F64(0x7ff8_1234_5678_9abc),
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
        .map(Value::Int),
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
        .map(Value::UInt),
    );
    values.extend([
        Value::Range(RuntimeRange::Int {
            start: None,
            end: Some(RuntimeInt::I16(42)),
            inclusive: false,
        }),
        Value::Range(RuntimeRange::UInt {
            start: Some(RuntimeUInt::U128(0)),
            end: Some(RuntimeUInt::U128(u128::MAX)),
            inclusive: true,
        }),
        Value::Iterator(IteratorSource::Values {
            items: vec![Value::Bool(true), Value::String("next".into())],
        }),
        Value::Iterator(IteratorSource::Range(RuntimeRangeIterator::Int {
            width: RuntimeSignedIntWidth::I8,
            current: 128,
            end: 127,
            inclusive: true,
            done: true,
        })),
        Value::Iterator(IteratorSource::Range(RuntimeRangeIterator::UInt {
            width: RuntimeUnsignedIntWidth::U128,
            current: u128::MAX,
            end: u128::MAX,
            inclusive: true,
            done: true,
        })),
    ]);
    for value in values {
        roundtrip(value);
    }
}

fn dense_values() -> Vec<DenseSeq> {
    vec![
        DenseSeq::Units(3),
        DenseSeq::I8(DenseSeqStorage::new(vec![i8::MIN, i8::MAX])),
        DenseSeq::I16(DenseSeqStorage::new(vec![i16::MIN, i16::MAX])),
        DenseSeq::I32(DenseSeqStorage::new(vec![i32::MIN, i32::MAX])),
        DenseSeq::I64(DenseSeqStorage::new(vec![i64::MIN, i64::MAX])),
        DenseSeq::I128(DenseSeqStorage::new(vec![i128::MIN, i128::MAX])),
        DenseSeq::ISize(DenseSeqStorage::new(vec![
            RuntimeISizeValue::new(i64::MIN),
            RuntimeISizeValue::new(i64::MAX),
        ])),
        DenseSeq::U8(DenseSeqStorage::new(vec![0, u8::MAX])),
        DenseSeq::U16(DenseSeqStorage::new(vec![0, u16::MAX])),
        DenseSeq::U32(DenseSeqStorage::new(vec![0, u32::MAX])),
        DenseSeq::U64(DenseSeqStorage::new(vec![0, u64::MAX])),
        DenseSeq::U128(DenseSeqStorage::new(vec![0, u128::MAX])),
        DenseSeq::USize(DenseSeqStorage::new(vec![
            RuntimeUSizeValue::new(0),
            RuntimeUSizeValue::new(u64::MAX),
        ])),
        DenseSeq::F32(DenseSeqStorage::new(vec![
            -0.0,
            f32::from_bits(0x7fc0_1234),
        ])),
        DenseSeq::F64(DenseSeqStorage::new(vec![
            -0.0,
            f64::from_bits(0x7ff8_1234_5678_9abc),
        ])),
        DenseSeq::Bool(DenseSeqStorage::new(vec![false, true])),
        DenseSeq::Bytes(DenseSeqStorage::new(vec![0, 255])),
        DenseSeq::Chars(DenseSeqStorage::new(vec!['\0', '語'])),
        DenseSeq::Durations(DenseSeqStorage::new(vec![LogicalDuration::from_nanos(
            u64::MAX,
        )])),
        DenseSeq::Strings(DenseSeqStorage::new(vec![String::new(), "終".into()])),
        DenseSeq::EntityRefs(DenseSeqStorage::new(vec![
            RuntimeEntityReference::StructuralFlow(
                crate::plan::FlowRuntimeId::from_runtime_contract("main.child", "公開名").unwrap(),
            ),
        ])),
    ]
}

#[test]
fn literal_source_preserves_all_dense_storage_kinds_and_recursive_columns() {
    let dense = dense_values();
    assert_eq!(dense.len(), 21);
    for value in dense {
        roundtrip(Value::Seq(Sequence::Dense(value)));
    }
    assert_ne!(
        encode(&Value::Seq(Sequence::Dense(DenseSeq::U8(
            DenseSeqStorage::new(vec![7])
        )))),
        encode(&Value::Seq(Sequence::Dense(DenseSeq::Bytes(
            DenseSeqStorage::new(vec![7])
        ))))
    );
    roundtrip(Value::Seq(Sequence::TupleColumns {
        len: 2,
        columns: vec![
            Sequence::Dense(DenseSeq::I16(DenseSeqStorage::new(vec![-7, 8]))),
            Sequence::RecordColumns {
                len: 2,
                fields: vec![AwbcRuntimeRecordSeqFieldSnapshot {
                    field: RuntimeRecordFieldId::try_from_zero_based_ordinal(0).unwrap(),
                    name: "payload".into(),
                    values: Sequence::Values(vec![Value::Bool(true), Value::Bool(false)]),
                }],
            },
        ],
    }));
    roundtrip(Value::Record(vec![AwbcRuntimeFieldSnapshot {
        field: RuntimeRecordFieldId::try_from_zero_based_ordinal(0).unwrap(),
        name: "field".into(),
        value: Value::Tuple(vec![Value::Unit]),
    }]));
}

#[test]
fn literal_source_dense_numeric_keeps_container_shape_and_bits() {
    use crate::math::{
        RuntimeDenseNumericElements as Elements, RuntimeDenseNumericPayload as Payload,
        RuntimeDenseNumericType, RuntimeFloatWidth,
    };
    for ty in RuntimeDenseNumericType::ALL {
        let elements = match ty.scalar_width() {
            RuntimeFloatWidth::F32 => Elements::F32(vec![0x8000_0000, 0x7fc0_1234]),
            RuntimeFloatWidth::F64 => {
                Elements::F64(vec![0x8000_0000_0000_0000, 0x7ff8_1234_5678_9abc])
            }
        };
        roundtrip(Value::DenseNumeric(
            Payload::try_new(ty, vec![1, 2], elements).unwrap(),
        ));
        if ty.accepts_rank(3) {
            let elements = match ty.scalar_width() {
                RuntimeFloatWidth::F32 => Elements::F32(vec![]),
                RuntimeFloatWidth::F64 => Elements::F64(vec![]),
            };
            roundtrip(Value::DenseNumeric(
                Payload::try_new(ty, vec![u64::MAX, u64::MAX, 0], elements).unwrap(),
            ));
        }
    }
}

#[test]
fn literal_source_agent_static_algebra_preserves_every_case() {
    let target = || crate::entry::RuntimeCommandTargetId::try_new("scene.action").unwrap();
    let path = || RuntimeAgentPath::try_new("state.score").unwrap();
    let probes = [
        RuntimeAgentProbe::Signal { target: target() },
        RuntimeAgentProbe::Metric { target: target() },
        RuntimeAgentProbe::StatePath { path: path() },
        RuntimeAgentProbe::ObservationField { path: path() },
    ];
    for action in [
        RuntimeAgentAction::AdvanceText,
        RuntimeAgentAction::SelectChoice,
        RuntimeAgentAction::Invoke,
        RuntimeAgentAction::Scroll,
        RuntimeAgentAction::PointerClick,
    ] {
        for dispatch in [
            RuntimeAgentActionDispatch::Semantic,
            RuntimeAgentActionDispatch::Physical,
        ] {
            for enabled in [false, true] {
                roundtrip(Value::Agent(Agent::ActionTarget(
                    RuntimeAgentActionTarget::new(target(), target(), action, dispatch, enabled),
                )));
            }
        }
    }
    for capture in [
        RuntimeAgentCaptureTarget::Viewport,
        RuntimeAgentCaptureTarget::Layer { target: target() },
        RuntimeAgentCaptureTarget::Object { target: target() },
    ] {
        roundtrip(Value::Agent(Agent::CaptureTarget(capture)));
    }
    for probe in probes {
        roundtrip(Value::Agent(Agent::Probe(probe.clone())));
        roundtrip(Value::Agent(Agent::Predicate(Predicate::Exists {
            probe: probe.clone(),
        })));
        for op in [
            RuntimeAgentCompareOp::Eq,
            RuntimeAgentCompareOp::NotEq,
            RuntimeAgentCompareOp::Greater,
            RuntimeAgentCompareOp::GreaterOrEqual,
            RuntimeAgentCompareOp::Less,
            RuntimeAgentCompareOp::LessOrEqual,
        ] {
            roundtrip(Value::Agent(Agent::Predicate(Predicate::Compare {
                probe: probe.clone(),
                op,
                value: Box::new(Value::Int(RuntimeInt::I8(-3))),
            })));
        }
    }
    for agent in [
        Agent::Diagnostics,
        Agent::DebugStatePath(path()),
        Agent::ObservationFieldPath(path()),
        Agent::ViewportPoint {
            x: 0x8000_0000,
            y: 0x7fc0_1234,
        },
        Agent::BinaryData("AAEC".into()),
    ] {
        roundtrip(Value::Agent(agent));
    }
    let operands = || {
        AgentPredicateOperands::try_from(vec![
            Predicate::DiagnosticsHasError,
            Predicate::ActionEnabled { target: target() },
        ])
        .unwrap()
    };
    roundtrip(Value::Agent(Agent::Predicate(Predicate::All {
        predicates: operands(),
    })));
    roundtrip(Value::Agent(Agent::Predicate(Predicate::Any {
        predicates: operands(),
    })));
    roundtrip(Value::Agent(Agent::Predicate(Predicate::Not {
        predicate: Box::new(Predicate::All {
            predicates: operands(),
        }),
    })));
}

#[test]
fn literal_source_frame_rejects_truncation_trailing_bytes_unknown_tags_and_bad_primitives() {
    let valid = encode(&Value::Tuple(vec![
        Value::Bool(true),
        Value::String("complete".into()),
    ]));
    for end in 0..valid.len() {
        rejected(&valid[..end]);
    }
    let mut trailing = valid;
    trailing.push(0);
    assert!(matches!(
        rejected(&trailing),
        RuntimeLiteralSourceError::Trailing { .. }
    ));
    let mut domain = encode(&Value::Unit);
    domain[0] ^= 1;
    assert_eq!(rejected(&domain), RuntimeLiteralSourceError::Domain);
    let mut version = encode(&Value::Unit);
    version[DOMAIN.len()] = 2;
    assert_eq!(
        rejected(&version),
        RuntimeLiteralSourceError::Version { actual: 2 }
    );
    let root = DOMAIN.len() + 1;
    let mut tag = encode(&Value::Unit);
    tag[root] = 255;
    assert!(matches!(
        rejected(&tag),
        RuntimeLiteralSourceError::Tag { kind: "value", .. }
    ));
    let mut boolean = encode(&Value::Bool(false));
    boolean[root + 1] = 2;
    assert!(matches!(
        rejected(&boolean),
        RuntimeLiteralSourceError::Tag { .. }
    ));
    let mut string = encode(&Value::String("a".into()));
    *string.last_mut().unwrap() = 255;
    assert!(matches!(
        rejected(&string),
        RuntimeLiteralSourceError::Utf8 { .. }
    ));
    let mut integer = encode(&Value::Int(RuntimeInt::I8(0)));
    integer[root + 2..root + 18].copy_from_slice(&128_i128.to_le_bytes());
    assert!(matches!(
        rejected(&integer),
        RuntimeLiteralSourceError::Invalid {
            kind: "signed integer width",
            ..
        }
    ));
    let mut character = encode(&Value::Char('a'));
    character[root + 1..].copy_from_slice(&0xd800_u32.to_le_bytes());
    assert!(matches!(
        rejected(&character),
        RuntimeLiteralSourceError::Invalid { .. }
    ));
    let mut progress = encode(&Value::Progress {
        ratio: 0.0,
        label: None,
    });
    progress[root + 1..root + 5].copy_from_slice(&f32::NAN.to_bits().to_le_bytes());
    assert!(matches!(
        rejected(&progress),
        RuntimeLiteralSourceError::Progress { .. }
    ));
}

#[test]
fn literal_source_preflights_exact_storage_elements_bytes_and_shared_work() {
    let value = Value::Tuple(vec![Value::String("payload".into()), Value::Bool(true)]);
    let bytes = encode(&value);
    let resources = resources();
    let mut measured = meter();
    let prepared =
        Value::preflight_literal_source(&bytes, limits(), &resources, &mut measured).unwrap();
    let exact = RuntimeLiteralSourceLimits {
        encoded_bytes: bytes.len(),
        decoded_bytes: prepared.storage_bytes(),
        elements: prepared.elements().try_into().unwrap(),
    };
    let work = measured.totals().0;
    Value::preflight_literal_source(
        &bytes,
        exact,
        &resources,
        &mut TaskSemanticMeter::new(work, u64::MAX),
    )
    .unwrap();
    for short in [
        RuntimeLiteralSourceLimits {
            encoded_bytes: exact.encoded_bytes - 1,
            ..exact
        },
        RuntimeLiteralSourceLimits {
            decoded_bytes: exact.decoded_bytes - 1,
            ..exact
        },
        RuntimeLiteralSourceLimits {
            elements: exact.elements - 1,
            ..exact
        },
    ] {
        assert!(matches!(
            Value::preflight_literal_source(&bytes, short, &resources, &mut meter()),
            Err(RuntimeLiteralSourceError::Limit { .. })
        ));
        assert!(matches!(
            value.prepare_literal_source_encoding(short, &resources, &mut meter()),
            Err(RuntimeLiteralSourceError::Limit { .. })
        ));
    }
    let mut precharged = TaskSemanticMeter::new(work, u64::MAX);
    precharged.charge_work(1).unwrap();
    assert!(matches!(
        Value::preflight_literal_source(&bytes, exact, &resources, &mut precharged),
        Err(RuntimeLiteralSourceError::Resource(
            TaskSemanticEncodingError::SemanticWork
        ))
    ));
    assert!(matches!(
        Value::preflight_literal_source(
            &encode(&Value::Unit),
            limits(),
            &resources,
            &mut precharged
        ),
        Err(RuntimeLiteralSourceError::Resource(
            TaskSemanticEncodingError::SemanticWork
        ))
    ));
    let mut materialization = TaskSemanticMeter::new(work, u64::MAX);
    let prepared =
        Value::preflight_literal_source(&bytes, exact, &resources, &mut materialization).unwrap();
    assert!(matches!(
        prepared.materialize(&mut materialization),
        Err(RuntimeLiteralSourceError::Resource(
            TaskSemanticEncodingError::SemanticWork
        ))
    ));
}

#[test]
fn literal_source_writer_requires_reserved_capacity_before_appending_any_bytes() {
    let value = Value::String("value".into());
    let prepared = value
        .prepare_literal_source_encoding(limits(), &resources(), &mut meter())
        .unwrap();
    let mut output = Vec::new();
    assert!(matches!(
        prepared.append_to(&mut output),
        Err(RuntimeLiteralSourceError::OutputCapacity { .. })
    ));
    assert!(output.is_empty());
    let prepared = value
        .prepare_literal_source_encoding(limits(), &resources(), &mut meter())
        .unwrap();
    let mut output = Vec::with_capacity(3 + prepared.encoded_bytes());
    output.extend_from_slice(&[8, 7, 6]);
    let pointer = output.as_ptr();
    prepared.append_to(&mut output).unwrap();
    assert_eq!(output.as_ptr(), pointer);
    assert_eq!(&output[..3], &[8, 7, 6]);
    assert_eq!(&output[3..], encode(&value));
}
