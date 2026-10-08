use super::*;
use crate::task::semantic::{TaskSemanticEncodingError, TaskSemanticMeter};
use crate::value::runtime_sequence_dense_i64;

fn digest(value: &RuntimeValue) -> blake3::Hash {
    let mut meter = TaskSemanticMeter::new(1_000_000, 10_000_000);
    let mut encoder = TaskSemanticEncoder::new(b"literal-test.v1\0", &mut meter);
    value.encode_static_literal(&mut encoder).unwrap();
    encoder.finish().unwrap()
}

#[test]
fn logical_dense_and_value_sequence_storage_have_identical_semantics() {
    let dense = runtime_sequence_dense_i64(vec![3, -5]);
    let values = RuntimeValue::Seq(RuntimeSeq::Values(vec![
        RuntimeValue::i64(3),
        RuntimeValue::i64(-5),
    ]));
    assert_eq!(digest(&dense), digest(&values));
    assert_ne!(
        digest(&dense),
        digest(&runtime_sequence_dense_i64(vec![-5, 3]))
    );
}

#[test]
fn float_bits_integer_width_and_matrix_shape_are_executable_semantics() {
    assert_ne!(
        digest(&RuntimeValue::F32(0.0)),
        digest(&RuntimeValue::F32(-0.0))
    );
    assert_ne!(
        digest(&RuntimeValue::i64(7)),
        digest(&RuntimeValue::Int(RuntimeInt::i32(7)))
    );
    let row = RuntimeValue::MatrixF32(crate::math::DenseMatrix::new(1, 2, vec![1.0, 2.0]).unwrap());
    let col = RuntimeValue::MatrixF32(crate::math::DenseMatrix::new(2, 1, vec![1.0, 2.0]).unwrap());
    assert_ne!(digest(&row), digest(&col));
}

#[test]
fn literal_work_and_byte_limits_prevent_partial_digest() {
    let value = RuntimeValue::Bool(true);
    let mut exact = TaskSemanticMeter::new(2, 3);
    let mut encoder = TaskSemanticEncoder::new(b"d", &mut exact);
    value.encode_static_literal(&mut encoder).unwrap();
    assert_eq!(encoder.finish().unwrap(), blake3::hash(&[b'd', 1, 1]));
    assert_eq!(exact.totals(), (2, 3));
    for (work, bytes, error) in [
        (1, 3, TaskSemanticEncodingError::SemanticWork),
        (2, 2, TaskSemanticEncodingError::TranscriptBytes),
    ] {
        let mut meter = TaskSemanticMeter::new(work, bytes);
        let mut encoder = TaskSemanticEncoder::new(b"d", &mut meter);
        assert!(
            matches!(value.encode_static_literal(&mut encoder), Err(RuntimeBodySemanticError::Encoding(actual)) if actual == error)
        );
        assert_eq!(encoder.finish(), Err(error));
    }
}

#[test]
fn agent_predicate_order_and_nested_literal_bits_change_semantics() {
    let probe = RuntimeAgentProbe::Signal {
        target: crate::entry::RuntimeCommandTargetId::try_new("signal.test").unwrap(),
    };
    let predicate = |value| RuntimeAgentPredicate::Compare {
        probe: probe.clone(),
        op: RuntimeAgentCompareOp::Eq,
        value: Box::new(RuntimeValue::F64(value)),
    };
    let a = RuntimeValue::Agent(RuntimeAgentValue::Predicate(
        RuntimeAgentPredicate::try_all(vec![predicate(1.0), predicate(2.0)]).unwrap(),
    ));
    let b = RuntimeValue::Agent(RuntimeAgentValue::Predicate(
        RuntimeAgentPredicate::try_all(vec![predicate(2.0), predicate(1.0)]).unwrap(),
    ));
    assert_ne!(digest(&a), digest(&b));
    let mut widths = Vec::new();
    a.try_visit_static_literal_child_counts(&mut |count| {
        widths.push(count);
        Ok::<(), ()>(())
    })
    .unwrap();
    assert_eq!(widths, [1, 2, 1, 1]);
}

#[test]
fn live_need_is_rejected_and_cannot_finalize_a_static_literal_digest() {
    let value = RuntimeValue::NeedHandle(crate::tests::reusable_need("need.invalid_static"));
    let mut meter = TaskSemanticMeter::new(100, 1000);
    let mut encoder = TaskSemanticEncoder::new(b"literal-test.v1\0", &mut meter);
    assert!(matches!(
        value.encode_static_literal(&mut encoder),
        Err(RuntimeBodySemanticError::InvalidStaticLiteral)
    ));
    assert_eq!(
        encoder.finish(),
        Err(TaskSemanticEncodingError::OwnerRejected)
    );
}

#[test]
fn deep_static_tuple_literals_use_the_iterative_owner() {
    let mut value = RuntimeValue::Bool(true);
    for _ in 0..20_000 {
        value = RuntimeValue::Tuple(vec![value]);
    }
    let mut lists = 0;
    value
        .try_visit_static_literal_child_counts(&mut |count| {
            assert_eq!(count, 1);
            lists += 1;
            Ok::<(), ()>(())
        })
        .unwrap();
    assert_eq!(lists, 20_000);
    let _ = digest(&value);
    while let RuntimeValue::Tuple(mut items) = value {
        value = items.pop().unwrap();
    }
}

#[test]
fn literal_columnar_rows_share_logical_children_and_golden_transcript() {
    use crate::value::TupleSeq;
    let columnar = RuntimeValue::Seq(RuntimeSeq::TupleColumns(
        TupleSeq::new(
            2,
            vec![
                RuntimeSeq::dense_bool(vec![true, false]),
                RuntimeSeq::dense_f32(vec![-0.0, 2.0]),
            ],
        )
        .unwrap(),
    ));
    let values = RuntimeValue::Seq(RuntimeSeq::Values(vec![
        RuntimeValue::Tuple(vec![RuntimeValue::Bool(true), RuntimeValue::F32(-0.0)]),
        RuntimeValue::Tuple(vec![RuntimeValue::Bool(false), RuntimeValue::F32(2.0)]),
    ]));
    let mut expected = b"literal-test.v1\0".to_vec();
    expected.push(13);
    expected.extend(2_u32.to_le_bytes());
    for (boolean, float) in [(1, -0.0_f32), (0, 2.0_f32)] {
        expected.push(12);
        expected.extend(2_u32.to_le_bytes());
        expected.extend([1, boolean, 4]);
        expected.extend(float.to_bits().to_le_bytes());
    }
    for value in [&columnar, &values] {
        let mut widths = Vec::new();
        value
            .try_visit_static_literal_child_counts(&mut |count| {
                widths.push(count);
                Ok::<(), ()>(())
            })
            .unwrap();
        assert_eq!(widths, [2, 2, 2]);
        assert_eq!(digest(value), blake3::hash(&expected));
    }
}

#[test]
fn literal_count_stops_at_first_nested_width_without_semantic_admission() {
    let value = RuntimeValue::Tuple(vec![
        RuntimeValue::Tuple(vec![RuntimeValue::Unit; 3]),
        RuntimeValue::Tuple(vec![
            RuntimeValue::NeedHandle(crate::tests::reusable_need(
                "need.unvisited"
            ));
            4
        ]),
    ]);
    let mut widths = Vec::new();
    let error = value.try_visit_static_literal_child_counts(&mut |count| {
        widths.push(count);
        if count > 2 { Err(count) } else { Ok(()) }
    });
    assert_eq!(error, Err(3));
    assert_eq!(widths, [2, 3]);
}

#[test]
fn reduction_literal_preserves_state_then_command_payload_order() {
    use crate::pattern::{
        RuntimeOpaqueTypeOwner, RuntimeOpaqueTypeProducerId, RuntimeSemanticTypeId,
    };
    use crate::value::{RuntimePayload, RuntimeReductionValue};
    let value = RuntimeValue::Reduction(
        RuntimeReductionValue::try_from_admitted_parts(
            RuntimeOpaqueTypeOwner::exact(
                RuntimeOpaqueTypeProducerId::try_new("std.reduction").unwrap(),
                RuntimeSemanticTypeId::from_bytes([7; 32]),
            ),
            RuntimeValue::Tuple(vec![RuntimeValue::Bool(true), RuntimeValue::Bool(false)]),
            [RuntimeCommand::new_accepted(
                crate::entry::RuntimeCommandConstructorId::try_new("command.test").unwrap(),
                crate::entry::RuntimeCommandTargetId::try_new("target.test").unwrap(),
                RuntimePayload::new(RuntimeValue::Tuple(vec![RuntimeValue::Unit])),
            )],
        )
        .unwrap(),
    );
    let mut widths = Vec::new();
    value
        .try_visit_static_literal_child_counts(&mut |count| {
            widths.push(count);
            Ok::<(), ()>(())
        })
        .unwrap();
    assert_eq!(widths, [1, 1, 2, 1]);
    let mut expected = b"literal-test.v1\0".to_vec();
    expected.push(19);
    expected.extend(13_u32.to_le_bytes());
    expected.extend(b"std.reduction");
    expected.extend([7; 32]);
    expected.extend(1_u32.to_le_bytes());
    expected.push(12);
    expected.extend(2_u32.to_le_bytes());
    expected.extend([1, 1, 1, 0]);
    expected.extend(12_u32.to_le_bytes());
    expected.extend(b"command.test");
    expected.extend(11_u32.to_le_bytes());
    expected.extend(b"target.test");
    expected.push(12);
    expected.extend(1_u32.to_le_bytes());
    expected.push(0);
    assert_eq!(digest(&value), blake3::hash(&expected));
}
