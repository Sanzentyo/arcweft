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
    let _ = digest(&value);
    while let RuntimeValue::Tuple(mut items) = value {
        value = items.pop().unwrap();
    }
}
