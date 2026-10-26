//! Typed collection operations over the admitted runtime storage families.

use super::{RuntimeEvalError, RuntimeIntrinsic, RuntimeValue};

pub(crate) fn evaluate_collection_intrinsic(
    intrinsic: RuntimeIntrinsic,
    args: &mut Vec<RuntimeValue>,
) -> Result<Option<RuntimeValue>, RuntimeEvalError> {
    if intrinsic == RuntimeIntrinsic::CoreFanoutInputs {
        let [
            RuntimeValue::Seq(source),
            RuntimeValue::UInt(super::RuntimeUInt::U32(limit)),
        ] = args.as_slice()
        else {
            return Err(collection_error(
                intrinsic,
                "expected an owned collection and u32 limit",
            ));
        };
        if *limit == 0 {
            return Err(collection_error(intrinsic, "fanout limit must be positive"));
        }
        let limit = u64::from(*limit);
        let count = source.len() as u64;
        args.pop();
        let source = args.pop().expect("validated owned collection");
        return Ok(Some(RuntimeValue::Tuple(vec![
            source,
            RuntimeValue::usize(count),
            RuntimeValue::usize(limit),
        ])));
    }
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

    #[test]
    fn fanout_inputs_reject_invalid_limits_before_consuming_affine_source() {
        let issued = crate::tests::program_custody::issued_program_handle();
        let mut args = vec![
            RuntimeValue::Seq(RuntimeSeq::values(vec![issued.value])),
            RuntimeValue::u32(0),
        ];
        assert!(
            evaluate_collection_intrinsic(RuntimeIntrinsic::CoreFanoutInputs, &mut args).is_err()
        );
        assert_eq!(args.len(), 2);
        let handles = args[0].affine_line_handles().unwrap();
        assert_eq!(handles.len(), 1);
        assert_eq!(handles[0].token(), &issued.token);
        args[1] = RuntimeValue::u64(2);
        assert!(
            evaluate_collection_intrinsic(RuntimeIntrinsic::CoreFanoutInputs, &mut args).is_err()
        );
        assert_eq!(args.len(), 2);
    }

    #[test]
    fn fanout_inputs_move_affine_collection_and_preserve_empty_and_dynamic_limits() {
        let mut empty = vec![
            RuntimeValue::Seq(RuntimeSeq::values(vec![])),
            RuntimeValue::u32(7),
        ];
        let Some(RuntimeValue::Tuple(fields)) =
            evaluate_collection_intrinsic(RuntimeIntrinsic::CoreFanoutInputs, &mut empty).unwrap()
        else {
            panic!("owned fanout tuple");
        };
        assert_eq!(fields[1], RuntimeValue::usize(0));
        assert_eq!(fields[2], RuntimeValue::usize(7));
        assert!(empty.is_empty());
        let issued = crate::tests::program_custody::issued_program_handle();
        let mut args = vec![
            RuntimeValue::Seq(RuntimeSeq::values(vec![issued.value])),
            RuntimeValue::u32(2),
        ];
        let Some(RuntimeValue::Tuple(fields)) =
            evaluate_collection_intrinsic(RuntimeIntrinsic::CoreFanoutInputs, &mut args).unwrap()
        else {
            panic!("owned fanout tuple");
        };
        assert!(args.is_empty());
        assert_eq!(fields[1], RuntimeValue::usize(1));
        assert_eq!(fields[2], RuntimeValue::usize(2));
        let handles = fields[0].affine_line_handles().unwrap();
        assert_eq!(handles.len(), 1);
        assert_eq!(handles[0].token(), &issued.token);
    }

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
