//! Typed collection operations over the admitted runtime storage families.

use super::{RuntimeEvalError, RuntimeIntrinsic, RuntimeValue};

pub(crate) fn evaluate_collection_intrinsic(
    intrinsic: RuntimeIntrinsic,
    args: &mut Vec<RuntimeValue>,
) -> Result<Option<RuntimeValue>, RuntimeEvalError> {
    if !matches!(
        intrinsic,
        RuntimeIntrinsic::CoreSeqLen | RuntimeIntrinsic::CoreSeqSum
    ) {
        return Ok(None);
    }
    let [RuntimeValue::Seq(_)] = args.as_slice() else {
        return Err(collection_error(
            intrinsic,
            "expected exactly one collection receiver",
        ));
    };
    let Some(RuntimeValue::Seq(sequence)) = args.pop() else {
        unreachable!("one checked collection receiver remains owned until dispatch");
    };
    let result = match intrinsic {
        RuntimeIntrinsic::CoreSeqLen => RuntimeValue::usize(sequence.len() as u64),
        RuntimeIntrinsic::CoreSeqSum => RuntimeValue::i64(sequence.into_checked_sum_as_i64()?),
        _ => unreachable!("the closed collection intrinsic family was checked above"),
    };
    Ok(Some(result))
}

fn collection_error(intrinsic: RuntimeIntrinsic, reason: &str) -> RuntimeEvalError {
    RuntimeEvalError::UnsupportedPure {
        name: intrinsic.as_label().to_owned(),
        reason: reason.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::*;

    fn assert_sum(receiver: RuntimeValue, expected: i64) {
        let mut args = vec![receiver];
        assert_eq!(
            evaluate_collection_intrinsic(RuntimeIntrinsic::CoreSeqSum, &mut args).unwrap(),
            Some(RuntimeValue::i64(expected)),
        );
        assert!(
            args.is_empty(),
            "the receiver moves into the collection call"
        );
    }

    fn assert_first_rejection(receiver: RuntimeValue, expected: &str) {
        let mut args = vec![receiver];
        assert_eq!(
            evaluate_collection_intrinsic(RuntimeIntrinsic::CoreSeqSum, &mut args).unwrap_err(),
            RuntimeEvalError::UnsupportedBinary {
                op: "+",
                lhs: "int".into(),
                rhs: expected.into(),
            },
        );
        assert!(
            args.is_empty(),
            "the failed receiver remains consumed by the call"
        );
    }

    #[test]
    fn collection_length_retains_dense_logical_length_without_materializing_units() {
        let mut args = vec![runtime_sequence_dense_units(1_000_000)];
        assert_eq!(
            evaluate_collection_intrinsic(RuntimeIntrinsic::CoreSeqLen, &mut args).unwrap(),
            Some(RuntimeValue::usize(1_000_000)),
        );
        assert!(args.is_empty());
    }

    #[test]
    fn collection_sum_admits_all_integer_widths_and_bytes_without_source_width_overflow() {
        macro_rules! check {
            ($dense:ident, $value:ident, $maximum:expr, $expected:expr) => {{
                assert_sum($dense(vec![$maximum, 1]), $expected);
                assert_sum(
                    runtime_sequence_values(vec![
                        RuntimeValue::$value($maximum),
                        RuntimeValue::$value(1),
                    ]),
                    $expected,
                );
                assert_sum($dense(Vec::new()), 0);
            }};
        }
        check!(
            runtime_sequence_dense_i8,
            i8,
            i8::MAX,
            i64::from(i8::MAX) + 1
        );
        check!(
            runtime_sequence_dense_i16,
            i16,
            i16::MAX,
            i64::from(i16::MAX) + 1
        );
        check!(
            runtime_sequence_dense_i32,
            i32,
            i32::MAX,
            i64::from(i32::MAX) + 1
        );
        check!(runtime_sequence_dense_i64, i64, i64::MAX, i64::MIN);
        check!(
            runtime_sequence_dense_i128,
            i128,
            i128::from(i64::MAX),
            i64::MIN
        );
        check!(runtime_sequence_dense_isize, isize, i64::MAX, i64::MIN);
        check!(
            runtime_sequence_dense_u8,
            u8,
            u8::MAX,
            i64::from(u8::MAX) + 1
        );
        check!(
            runtime_sequence_dense_u16,
            u16,
            u16::MAX,
            i64::from(u16::MAX) + 1
        );
        check!(
            runtime_sequence_dense_u32,
            u32,
            u32::MAX,
            i64::from(u32::MAX) + 1
        );
        check!(runtime_sequence_dense_u64, u64, i64::MAX as u64, i64::MIN);
        check!(
            runtime_sequence_dense_u128,
            u128,
            i64::MAX as u128,
            i64::MIN
        );
        check!(
            runtime_sequence_dense_usize,
            usize,
            i64::MAX as u64,
            i64::MIN
        );
        assert_sum(runtime_sequence_dense_bytes(vec![u8::MAX, 1]), 256);
        assert_sum(runtime_sequence_dense_bytes(Vec::new()), 0);
    }

    #[test]
    fn collection_dense_sum_preserves_the_first_exact_narrowing_failure() {
        let first_signed = i128::from(i64::MIN) - 1;
        let first_unsigned = i64::MAX as u128 + 1;
        for receiver in [
            runtime_sequence_dense_i128(vec![7, first_signed, i128::MAX]),
            runtime_sequence_values(vec![
                RuntimeValue::i128(7),
                RuntimeValue::i128(first_signed),
                RuntimeValue::i128(i128::MAX),
            ]),
        ] {
            assert_first_rejection(receiver, &first_signed.to_string());
        }
        for receiver in [
            runtime_sequence_dense_u128(vec![7, first_unsigned, u128::MAX]),
            runtime_sequence_values(vec![
                RuntimeValue::u128(7),
                RuntimeValue::u128(first_unsigned),
                RuntimeValue::u128(u128::MAX),
            ]),
        ] {
            assert_first_rejection(receiver, &first_unsigned.to_string());
        }
        let first_u64 = i64::MAX as u64 + 1;
        for receiver in [
            runtime_sequence_dense_u64(vec![7, first_u64, u64::MAX]),
            runtime_sequence_dense_usize(vec![7, first_u64, u64::MAX]),
        ] {
            assert_first_rejection(receiver, &first_u64.to_string());
        }
    }

    #[test]
    fn collection_sum_rejects_noninteger_storage_with_the_canonical_binary_error() {
        assert_first_rejection(runtime_sequence_dense_units(2), "()");
        assert_first_rejection(runtime_sequence_dense_bool(vec![true, false]), "true");
    }

    #[test]
    fn collection_intrinsic_shape_rejection_retains_authored_arguments() {
        for mut args in [
            Vec::new(),
            vec![RuntimeValue::Bool(true)],
            vec![
                runtime_sequence_dense_i32(vec![1]),
                runtime_sequence_dense_i32(vec![2]),
            ],
        ] {
            let before = args.clone();
            assert_eq!(
                evaluate_collection_intrinsic(RuntimeIntrinsic::CoreSeqSum, &mut args).unwrap_err(),
                RuntimeEvalError::UnsupportedPure {
                    name: RuntimeIntrinsic::CoreSeqSum.as_label().into(),
                    reason: "expected exactly one collection receiver".into(),
                },
            );
            assert_eq!(
                args, before,
                "shape rejection precedes receiver custody transfer"
            );
        }
    }
}
