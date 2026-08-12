use crate::{
    entry::{RuntimeNominalTypeId, TypeLayoutHash},
    pattern::{RuntimeSemanticTypeId, RuntimeVariantIdentity},
    plan::RuntimePureHelperId,
    time::LogicalDuration,
    value::{
        DenseSeqKind, MAX_RUNTIME_VALUE_NESTING_DEPTH, RuntimeBinaryOp, RuntimeBinding, RuntimeEnv,
        RuntimeExpr, RuntimeIntrinsic, RuntimeIterator, RuntimeNominalRecordValue, RuntimeRange,
        RuntimeSeq, RuntimeUnaryOp, RuntimeValue, RuntimeValueNestingError,
        evaluate_core_iter_collect_intrinsic, evaluate_std_float_intrinsic,
        runtime_sequence_dense_bool, runtime_sequence_dense_bytes, runtime_sequence_dense_chars,
        runtime_sequence_dense_durations, runtime_sequence_dense_entity_refs,
        runtime_sequence_dense_f32, runtime_sequence_dense_f64, runtime_sequence_dense_i8,
        runtime_sequence_dense_i16, runtime_sequence_dense_i32, runtime_sequence_dense_i64,
        runtime_sequence_dense_i128, runtime_sequence_dense_isize, runtime_sequence_dense_strings,
        runtime_sequence_dense_u8, runtime_sequence_dense_u16, runtime_sequence_dense_u32,
        runtime_sequence_dense_u64, runtime_sequence_dense_u128, runtime_sequence_dense_units,
        runtime_sequence_dense_usize, runtime_sequence_from_literal_values,
        runtime_sequence_repeat_value, runtime_value_label,
    },
};

#[test]
fn nominal_and_anonymous_records_have_distinct_identity_and_bytes() {
    let nominal = RuntimeValue::NominalRecord(RuntimeNominalRecordValue::new(
        RuntimeNominalTypeId::try_new("test.Named").expect("type"),
        TypeLayoutHash::from_bytes([7; 32]),
        vec![RuntimeValue::i32(1)],
    ));
    let anonymous = crate::tests::runtime_record!([RuntimeFieldValue {
        name: "value".to_owned(),
        value: RuntimeValue::i32(1),
    }]);

    assert_ne!(nominal, anonymous);
    assert_ne!(
        nominal.try_canonical_bytes(1024).expect("nominal bytes"),
        anonymous
            .try_canonical_bytes(1024)
            .expect("anonymous bytes")
    );
    assert_eq!(runtime_value_label(&nominal), "nominal-record/test.Named/1");
}

#[test]
fn canonical_float_encoding_normalizes_negative_zero_and_rejects_non_finite() {
    assert_eq!(
        RuntimeValue::F32(-0.0)
            .try_canonical_bytes(32)
            .expect("negative zero"),
        RuntimeValue::F32(0.0)
            .try_canonical_bytes(32)
            .expect("positive zero")
    );
    assert_eq!(
        RuntimeValue::F64(-0.0)
            .try_digest(32)
            .expect("negative zero"),
        RuntimeValue::F64(0.0)
            .try_digest(32)
            .expect("positive zero")
    );
    assert!(RuntimeValue::F32(f32::NAN).try_canonical_bytes(32).is_err());
    assert!(
        RuntimeValue::F64(f64::INFINITY)
            .try_canonical_bytes(32)
            .is_err()
    );
}

#[test]
fn runtime_value_nesting_accepts_64_and_rejects_65() {
    fn nested_nominal(depth: usize) -> RuntimeValue {
        let type_id = RuntimeNominalTypeId::try_new("test.Nested").expect("type");
        let layout = TypeLayoutHash::from_bytes([19; 32]);
        (0..depth).fold(RuntimeValue::Unit, |value, _| {
            RuntimeValue::NominalRecord(RuntimeNominalRecordValue::new(
                type_id.clone(),
                layout,
                vec![value],
            ))
        })
    }

    nested_nominal(MAX_RUNTIME_VALUE_NESTING_DEPTH)
        .validate_nesting_depth(MAX_RUNTIME_VALUE_NESTING_DEPTH)
        .expect("depth 64 is accepted");
    assert_eq!(
        nested_nominal(MAX_RUNTIME_VALUE_NESTING_DEPTH + 1)
            .validate_nesting_depth(MAX_RUNTIME_VALUE_NESTING_DEPTH)
            .expect_err("depth 65 is rejected"),
        RuntimeValueNestingError::Exceeded {
            maximum: MAX_RUNTIME_VALUE_NESTING_DEPTH,
        }
    );
}

#[test]
fn option_none_conversion_rejects_same_named_non_option_variants() {
    let option = RuntimeValue::Variant {
        owner: RuntimeVariantIdentity::Option,
        ordinal: 0,
        name: "Some".to_owned(),
        payload: Some(Box::new(RuntimeValue::Bool(true))),
    };
    assert_eq!(
        option.option_none_with_same_owner(),
        Some(RuntimeValue::Variant {
            owner: RuntimeVariantIdentity::Option,
            ordinal: 1,
            name: "None".to_owned(),
            payload: None,
        })
    );

    let unrelated = RuntimeValue::Variant {
        owner: RuntimeVariantIdentity::Nominal {
            nominal: RuntimeNominalTypeId::try_new("custom.Choice").expect("nominal identity"),
            semantic_identity: RuntimeSemanticTypeId::from_bytes([3; 32]),
        },
        ordinal: 0,
        name: "Some".to_owned(),
        payload: Some(Box::new(RuntimeValue::Bool(true))),
    };
    assert_eq!(unrelated.option_none_with_same_owner(), None);

    let malformed = RuntimeValue::Variant {
        owner: RuntimeVariantIdentity::Option,
        ordinal: 0,
        name: "Some".to_owned(),
        payload: None,
    };
    assert_eq!(malformed.option_none_with_same_owner(), None);
}

#[test]
fn variant_canonical_bytes_retain_closed_owner_ordinal_and_semantic_identity() {
    let option = RuntimeValue::Variant {
        owner: RuntimeVariantIdentity::Option,
        ordinal: 0,
        name: "Some".to_owned(),
        payload: Some(Box::new(RuntimeValue::Unit)),
    };
    let result = RuntimeValue::result_ok(RuntimeValue::Unit);
    let nominal = |semantic_identity| RuntimeValue::Variant {
        owner: RuntimeVariantIdentity::Nominal {
            nominal: RuntimeNominalTypeId::try_new("game.State").expect("nominal identity"),
            semantic_identity: RuntimeSemanticTypeId::from_bytes(semantic_identity),
        },
        ordinal: 0,
        name: "Some".to_owned(),
        payload: Some(Box::new(RuntimeValue::Unit)),
    };

    let encode = |value: RuntimeValue| value.try_canonical_bytes(1024).expect("canonical value");
    assert_ne!(encode(option), encode(result));
    assert_ne!(encode(nominal([1; 32])), encode(nominal([2; 32])));
}

#[test]
fn runtime_collection_indices_use_one_width_preserving_conversion_rule() {
    let host_max = usize::MAX;
    let host_max_i128 = i128::try_from(host_max).expect("supported host usize fits i128");

    assert_eq!(
        RuntimeValue::i128(host_max_i128).to_collection_index(),
        Some(host_max)
    );
    assert_eq!(
        RuntimeValue::u128(u128::try_from(host_max).expect("usize fits u128"))
            .to_collection_index(),
        Some(host_max)
    );
    assert_eq!(RuntimeValue::i8(-1).to_collection_index(), None);
    assert_eq!(RuntimeValue::u128(u128::MAX).to_collection_index(), None);
    assert_eq!(RuntimeValue::f64(0.0).to_collection_index(), None);

    assert_eq!(
        RuntimeValue::from_collection_len(host_max),
        RuntimeValue::usize(u64::try_from(host_max).unwrap_or(u64::MAX))
    );
}

#[test]
fn runtime_sequence_index_returns_unit_for_invalid_or_out_of_bounds_values() {
    let sequence = RuntimeSeq::values(vec![RuntimeValue::String("first".to_owned())]);

    assert_eq!(
        sequence.value_at_runtime_index(&RuntimeValue::u8(0)),
        RuntimeValue::String("first".to_owned())
    );
    assert_eq!(
        sequence.value_at_runtime_index(&RuntimeValue::u8(1)),
        RuntimeValue::Unit
    );
    assert_eq!(
        sequence.value_at_runtime_index(&RuntimeValue::i8(-1)),
        RuntimeValue::Unit
    );
    assert_eq!(
        sequence.value_at_runtime_index(&RuntimeValue::String("0".to_owned())),
        RuntimeValue::Unit
    );
}

#[test]
fn root_binding_ref_updates_existing_slots() {
    let mut env = RuntimeEnv::default();
    let first = [RuntimeBinding {
        name: "seed".to_owned(),
        value: RuntimeValue::i64(1),
    }];
    let second = [RuntimeBinding {
        name: "seed".to_owned(),
        value: RuntimeValue::i64(2),
    }];

    env.bind_all_root_ref(&first);
    env.bind_all_root_ref(&second);

    assert_eq!(env.get("seed"), Some(&RuntimeValue::i64(2)));
}

#[test]
fn runtime_range_iterates_one_value_at_a_time() {
    let range = RuntimeValue::Range(
        RuntimeRange::new(
            Some(RuntimeValue::i32(0)),
            Some(RuntimeValue::i32(3)),
            false,
        )
        .expect("matching i32 range is valid"),
    );
    let mut iterator = RuntimeIterator::from_value(range).expect("range is iterable");

    assert_eq!(iterator.next(), Some(RuntimeValue::i32(0)));
    assert_eq!(iterator.next(), Some(RuntimeValue::i32(1)));
    assert_eq!(iterator.next(), Some(RuntimeValue::i32(2)));
    assert_eq!(iterator.next(), None);
}

#[test]
fn core_iter_collect_materializes_range_sequence() {
    let range = RuntimeValue::Range(
        RuntimeRange::new(
            Some(RuntimeValue::i32(0)),
            Some(RuntimeValue::i32(3)),
            false,
        )
        .expect("matching i32 range is valid"),
    );
    let value = evaluate_core_iter_collect_intrinsic(range).expect("range is iterable");
    let RuntimeValue::Seq(sequence) = value else {
        panic!("core.iter.collect should return a sequence");
    };

    assert_eq!(sequence.len(), 3);
    assert_eq!(sequence.value_at(0), RuntimeValue::i32(0));
    assert_eq!(sequence.value_at(1), RuntimeValue::i32(1));
    assert_eq!(sequence.value_at(2), RuntimeValue::i32(2));
}

#[test]
fn root_binding_ref_reuses_matching_ordered_slots() {
    let mut env = RuntimeEnv::default();
    let first = [
        RuntimeBinding {
            name: "lhs".to_owned(),
            value: RuntimeValue::i64(1),
        },
        RuntimeBinding {
            name: "rhs".to_owned(),
            value: RuntimeValue::i64(2),
        },
    ];
    let second = [
        RuntimeBinding {
            name: "lhs".to_owned(),
            value: RuntimeValue::i64(3),
        },
        RuntimeBinding {
            name: "rhs".to_owned(),
            value: RuntimeValue::i64(4),
        },
    ];

    env.bind_all_root_ref(&first);
    env.bind_all_root_ref(&second);

    assert_eq!(env.get("lhs"), Some(&RuntimeValue::i64(3)));
    assert_eq!(env.get("rhs"), Some(&RuntimeValue::i64(4)));
}

#[test]
fn scoped_i64_binding_updates_without_value_input() {
    let mut env = RuntimeEnv::default();

    env.push_scope_with_capacity(1);
    env.set("item", RuntimeValue::i64(3));
    env.set("item", RuntimeValue::i64(5));

    assert_eq!(env.get("item"), Some(&RuntimeValue::i64(5)));
}

#[test]
fn spare_scopes_do_not_affect_runtime_env_semantics() {
    let mut env = RuntimeEnv::default();
    env.push_scope_with_capacity(2);
    env.set("scoped", RuntimeValue::i64(1));
    env.pop_scope();

    let baseline = RuntimeEnv::default();
    assert_eq!(env, baseline);
    assert_eq!(env.clone(), baseline);

    env.push_scope_with_capacity(1);
    assert!(env.get("scoped").is_none());
}

#[test]
fn runtime_expr_display_is_stable_diagnostic_label() {
    assert_eq!(RuntimeExpr::Local("score".to_owned()).to_string(), "score");
    assert_eq!(
        RuntimeExpr::SpreadArg(Box::new(RuntimeExpr::Local("items".to_owned()))).to_string(),
        "items..."
    );
    assert_eq!(
        RuntimeExpr::PureCall {
            helper: RuntimePureHelperId(7),
            args: vec![RuntimeExpr::Value(RuntimeValue::i64(1))],
        }
        .to_string(),
        "pure#7()"
    );
}

#[test]
fn runtime_operator_display_uses_surface_labels() {
    assert_eq!(RuntimeUnaryOp::Neg.to_string(), "-");
    assert_eq!(RuntimeUnaryOp::Not.to_string(), "!");
    assert_eq!(RuntimeBinaryOp::Add.to_string(), "+");
    assert_eq!(RuntimeBinaryOp::And.to_string(), "&&");
}

#[test]
fn dense_sequence_kind_covers_deterministic_scalar_storage() {
    let cases = [
        (runtime_sequence_dense_units(1), DenseSeqKind::Units),
        (runtime_sequence_dense_i8(vec![1]), DenseSeqKind::I8),
        (runtime_sequence_dense_i16(vec![1]), DenseSeqKind::I16),
        (runtime_sequence_dense_i32(vec![1]), DenseSeqKind::I32),
        (runtime_sequence_dense_i64(vec![1]), DenseSeqKind::I64),
        (runtime_sequence_dense_i128(vec![1]), DenseSeqKind::I128),
        (runtime_sequence_dense_isize(vec![1]), DenseSeqKind::ISize),
        (runtime_sequence_dense_u8(vec![1]), DenseSeqKind::U8),
        (runtime_sequence_dense_u16(vec![1]), DenseSeqKind::U16),
        (runtime_sequence_dense_u32(vec![1]), DenseSeqKind::U32),
        (runtime_sequence_dense_u64(vec![1]), DenseSeqKind::U64),
        (runtime_sequence_dense_u128(vec![1]), DenseSeqKind::U128),
        (runtime_sequence_dense_usize(vec![1]), DenseSeqKind::USize),
        (runtime_sequence_dense_f32(vec![(1.0)]), DenseSeqKind::F32),
        (runtime_sequence_dense_f64(vec![(1.0)]), DenseSeqKind::F64),
        (runtime_sequence_dense_bool(vec![true]), DenseSeqKind::Bool),
        (runtime_sequence_dense_bytes(vec![1]), DenseSeqKind::Bytes),
        (runtime_sequence_dense_chars(vec!['a']), DenseSeqKind::Chars),
        (
            runtime_sequence_dense_durations(vec![LogicalDuration::from_nanos(1)]),
            DenseSeqKind::Durations,
        ),
        (
            runtime_sequence_dense_strings(vec!["a".to_owned()]),
            DenseSeqKind::Strings,
        ),
        (
            runtime_sequence_dense_entity_refs(vec!["char.alice".to_owned()]),
            DenseSeqKind::EntityRefs,
        ),
    ];

    for (value, expected) in cases {
        let RuntimeValue::Seq(seq) = value else {
            panic!("dense helper returns a sequence");
        };
        assert_eq!(seq.dense_kind(), Some(expected));
    }
}

#[test]
fn dense_integer_sequences_expose_width_specific_views() {
    let i8_seq = runtime_sequence_dense_i8(vec![1, 2]);
    let i16_seq = runtime_sequence_dense_i16(vec![1, 2]);
    let u8_seq = runtime_sequence_dense_u8(vec![1, 2]);
    let u16_seq = runtime_sequence_dense_u16(vec![1, 2]);
    let u32_seq = runtime_sequence_dense_u32(vec![1, 2]);

    assert_eq!(runtime_value_label(&i8_seq), "seq/i8/2");
    assert_eq!(runtime_value_label(&i16_seq), "seq/i16/2");
    assert_eq!(runtime_value_label(&u8_seq), "seq/u8/2");
    assert_eq!(runtime_value_label(&u16_seq), "seq/u16/2");
    assert_eq!(runtime_value_label(&u32_seq), "seq/u32/2");

    let RuntimeValue::Seq(i8_seq) = &i8_seq else {
        panic!("dense i8 helper returns a sequence");
    };
    assert_eq!(i8_seq.as_i8_slice(), Some([1, 2].as_slice()));
    let RuntimeValue::Seq(i16_seq) = &i16_seq else {
        panic!("dense i16 helper returns a sequence");
    };
    assert_eq!(i16_seq.as_i16_slice(), Some([1, 2].as_slice()));
    let RuntimeValue::Seq(u8_seq) = &u8_seq else {
        panic!("dense u8 helper returns a sequence");
    };
    assert_eq!(u8_seq.as_u8_slice(), Some([1, 2].as_slice()));
    assert_eq!(u8_seq.as_bytes(), Some([1, 2].as_slice()));
    let RuntimeValue::Seq(u16_seq) = &u16_seq else {
        panic!("dense u16 helper returns a sequence");
    };
    assert_eq!(u16_seq.as_u16_slice(), Some([1, 2].as_slice()));
    let RuntimeValue::Seq(u32_seq) = &u32_seq else {
        panic!("dense u32 helper returns a sequence");
    };
    assert_eq!(u32_seq.as_u32_slice(), Some([1, 2].as_slice()));
}

#[test]
fn dense_sequences_expose_typed_views_and_materialize_values() {
    let unit_seq = runtime_sequence_dense_units(3);
    let bool_seq = runtime_sequence_dense_bool(vec![true, false]);
    let chars_seq = runtime_sequence_dense_chars(vec!['a', 'b']);
    let duration_seq = runtime_sequence_dense_durations(vec![LogicalDuration::from_nanos(3)]);

    assert_eq!(runtime_value_label(&unit_seq), "seq/units/3");
    assert_eq!(runtime_value_label(&bool_seq), "seq/bool/2");
    assert_eq!(runtime_value_label(&chars_seq), "seq/chars/2");
    assert_eq!(runtime_value_label(&duration_seq), "seq/durations/1");

    let RuntimeValue::Seq(unit_seq) = unit_seq else {
        panic!("dense units helper returns a sequence");
    };
    assert_eq!(unit_seq.unit_len(), Some(3));
    assert_eq!(unit_seq.len(), 3);
    assert_eq!(
        unit_seq.into_values(),
        vec![RuntimeValue::Unit, RuntimeValue::Unit, RuntimeValue::Unit]
    );

    let RuntimeValue::Seq(bool_seq) = bool_seq else {
        panic!("dense bool helper returns a sequence");
    };
    assert_eq!(bool_seq.as_bool_slice(), Some([true, false].as_slice()));
    assert_eq!(
        bool_seq.into_values(),
        vec![RuntimeValue::Bool(true), RuntimeValue::Bool(false)]
    );

    let RuntimeValue::Seq(chars_seq) = chars_seq else {
        panic!("dense chars helper returns a sequence");
    };
    assert_eq!(chars_seq.as_chars(), Some(['a', 'b'].as_slice()));
    assert_eq!(
        chars_seq.into_values(),
        vec![RuntimeValue::Char('a'), RuntimeValue::Char('b')]
    );

    let RuntimeValue::Seq(duration_seq) = duration_seq else {
        panic!("dense duration helper returns a sequence");
    };
    assert_eq!(
        duration_seq.as_durations(),
        Some([LogicalDuration::from_nanos(3)].as_slice())
    );
    assert_eq!(
        duration_seq.into_values(),
        vec![RuntimeValue::Duration(LogicalDuration::from_nanos(3))]
    );
}

#[test]
fn dense_integer_sequences_keep_exact_i64_projection_separate_from_other_widths() {
    let i32_seq = runtime_sequence_dense_i32(vec![1, 2, 3]);
    let i64_seq = runtime_sequence_dense_i64(vec![1, 2, 3]);
    let u64_seq = runtime_sequence_dense_u64(vec![1, 2]);
    let bytes_seq = runtime_sequence_dense_bytes(vec![65, 66]);

    assert_eq!(runtime_value_label(&i32_seq), "seq/i32/3");
    assert_eq!(runtime_value_label(&u64_seq), "seq/u64/2");
    assert_eq!(runtime_value_label(&bytes_seq), "seq/bytes/2");

    let RuntimeValue::Seq(i64_seq) = &i64_seq else {
        panic!("dense i64 helper returns a sequence");
    };
    assert_eq!(i64_seq.as_i64_slice(), Some([1, 2, 3].as_slice()));
    let mut flat = Vec::new();
    assert!(i64_seq.copy_i64_values_to(&mut flat));
    assert_eq!(flat, vec![1, 2, 3]);
    assert_eq!(i64_seq.first_i64(), Some(Some(1)));

    let RuntimeValue::Seq(i32_seq) = i32_seq else {
        panic!("dense i32 helper returns a sequence");
    };
    assert_eq!(i32_seq.as_i32_slice(), Some([1, 2, 3].as_slice()));
    assert_eq!(i32_seq.sum_as_i64(), Some(6));
    let mut flat = Vec::new();
    assert!(!i32_seq.copy_i64_values_to(&mut flat));
    assert!(flat.is_empty());
    assert_eq!(i32_seq.first_i64(), None);
    assert_eq!(
        i32_seq.into_values(),
        vec![
            RuntimeValue::i32(1),
            RuntimeValue::i32(2),
            RuntimeValue::i32(3)
        ]
    );

    let RuntimeValue::Seq(u64_seq) = u64_seq else {
        panic!("dense u64 helper returns a sequence");
    };
    assert_eq!(u64_seq.as_u64_slice(), Some([1, 2].as_slice()));
    assert_eq!(u64_seq.sum_as_i64(), Some(3));
    let mut flat = Vec::new();
    assert!(!u64_seq.copy_i64_values_to(&mut flat));
    assert!(flat.is_empty());
    assert_eq!(u64_seq.first_i64(), None);
    assert_eq!(
        u64_seq.into_values(),
        vec![RuntimeValue::u64(1), RuntimeValue::u64(2)]
    );

    let RuntimeValue::Seq(bytes_seq) = bytes_seq else {
        panic!("dense bytes helper returns a sequence");
    };
    assert_eq!(bytes_seq.as_bytes(), Some([65, 66].as_slice()));
    let mut flat = Vec::new();
    assert!(!bytes_seq.copy_i64_values_to(&mut flat));
    assert!(flat.is_empty());
    assert_eq!(bytes_seq.first_i64(), None);
    assert_eq!(
        bytes_seq.into_values(),
        vec![RuntimeValue::u8(65), RuntimeValue::u8(66)]
    );
}

#[test]
fn dense_wide_integer_sequences_expose_typed_views_and_materialize_values() {
    let i128_seq = runtime_sequence_dense_i128(vec![1, 2, 3]);
    let isize_seq = runtime_sequence_dense_isize(vec![1, 2, 3]);
    let u128_seq = runtime_sequence_dense_u128(vec![1, 2]);
    let usize_seq = runtime_sequence_dense_usize(vec![1, 2]);

    assert_eq!(runtime_value_label(&i128_seq), "seq/i128/3");
    assert_eq!(runtime_value_label(&isize_seq), "seq/isize/3");
    assert_eq!(runtime_value_label(&u128_seq), "seq/u128/2");
    assert_eq!(runtime_value_label(&usize_seq), "seq/usize/2");

    let RuntimeValue::Seq(i128_seq) = i128_seq else {
        panic!("dense i128 helper returns a sequence");
    };
    assert_eq!(i128_seq.as_i128_slice(), Some([1, 2, 3].as_slice()));
    assert_eq!(i128_seq.sum_as_i64(), Some(6));
    let mut flat = Vec::new();
    assert!(!i128_seq.copy_i64_values_to(&mut flat));
    assert!(flat.is_empty());
    assert_eq!(i128_seq.first_i64(), None);
    assert_eq!(
        i128_seq.into_values(),
        vec![
            RuntimeValue::i128(1),
            RuntimeValue::i128(2),
            RuntimeValue::i128(3)
        ]
    );

    let RuntimeValue::Seq(isize_seq) = isize_seq else {
        panic!("dense isize helper returns a sequence");
    };
    assert_eq!(isize_seq.as_isize_values(), Some(vec![1, 2, 3]));
    assert_eq!(isize_seq.sum_as_i64(), Some(6));
    let mut flat = Vec::new();
    assert!(!isize_seq.copy_i64_values_to(&mut flat));
    assert!(flat.is_empty());
    assert_eq!(isize_seq.first_i64(), None);
    assert_eq!(
        isize_seq.into_values(),
        vec![
            RuntimeValue::isize(1),
            RuntimeValue::isize(2),
            RuntimeValue::isize(3)
        ]
    );

    let RuntimeValue::Seq(u128_seq) = u128_seq else {
        panic!("dense u128 helper returns a sequence");
    };
    assert_eq!(u128_seq.as_u128_slice(), Some([1, 2].as_slice()));
    assert_eq!(u128_seq.sum_as_i64(), Some(3));
    let mut flat = Vec::new();
    assert!(!u128_seq.copy_i64_values_to(&mut flat));
    assert!(flat.is_empty());
    assert_eq!(u128_seq.first_i64(), None);
    assert_eq!(
        u128_seq.into_values(),
        vec![RuntimeValue::u128(1), RuntimeValue::u128(2)]
    );

    let RuntimeValue::Seq(usize_seq) = usize_seq else {
        panic!("dense usize helper returns a sequence");
    };
    assert_eq!(usize_seq.as_usize_values(), Some(vec![1, 2]));
    assert_eq!(usize_seq.sum_as_i64(), Some(3));
    let mut flat = Vec::new();
    assert!(!usize_seq.copy_i64_values_to(&mut flat));
    assert!(flat.is_empty());
    assert_eq!(usize_seq.first_i64(), None);
    assert_eq!(
        usize_seq.into_values(),
        vec![RuntimeValue::usize(1), RuntimeValue::usize(2)]
    );
}

#[test]
fn dense_float_sequences_expose_bit_exact_views_and_materialize_values() {
    let f32_values = [1.5_f32, f32::from_bits(f32::NAN.to_bits())];
    let f64_values = [2.25_f64, f64::from_bits((-0.0f64).to_bits())];
    let f32_seq = runtime_sequence_dense_f32(f32_values.to_vec());
    let f64_seq = runtime_sequence_dense_f64(f64_values.to_vec());

    assert_eq!(runtime_value_label(&f32_seq), "seq/f32/2");
    assert_eq!(runtime_value_label(&f64_seq), "seq/f64/2");

    let RuntimeValue::Seq(f32_seq) = f32_seq else {
        panic!("dense f32 helper returns a sequence");
    };
    let f32_slice = f32_seq.as_f32_slice().expect("dense f32 slice exists");
    assert_eq!(f32_slice.len(), f32_values.len());
    assert_eq!(f32_slice[0].to_bits(), f32_values[0].to_bits());
    assert_eq!(f32_slice[1].to_bits(), f32_values[1].to_bits());
    let f32_values_out = f32_seq.into_values();
    assert_eq!(f32_values_out[0], RuntimeValue::F32(f32_values[0]));
    assert!(
        matches!(f32_values_out[1], RuntimeValue::F32(value) if value.to_bits() == f32_values[1].to_bits())
    );

    let RuntimeValue::Seq(f64_seq) = f64_seq else {
        panic!("dense f64 helper returns a sequence");
    };
    assert_eq!(f64_seq.as_f64_slice(), Some(f64_values.as_slice()));
    assert_eq!(
        f64_seq.into_values(),
        vec![
            RuntimeValue::F64(f64_values[0]),
            RuntimeValue::F64(f64_values[1])
        ]
    );
}

#[test]
fn dense_non_i64_integer_storage_does_not_widen_into_i64_projection() {
    let cases = [
        runtime_sequence_dense_i8(vec![1, 2]),
        runtime_sequence_dense_i16(vec![1, 2]),
        runtime_sequence_dense_i32(vec![1, 2]),
        runtime_sequence_dense_i128(vec![1, 2]),
        runtime_sequence_dense_isize(vec![1, 2]),
        runtime_sequence_dense_u8(vec![1, 2]),
        runtime_sequence_dense_u16(vec![1, 2]),
        runtime_sequence_dense_u32(vec![1, 2]),
        runtime_sequence_dense_u64(vec![1, 2]),
        runtime_sequence_dense_u128(vec![1, 2]),
        runtime_sequence_dense_usize(vec![1, 2]),
        runtime_sequence_dense_bytes(vec![1, 2]),
    ];

    for value in cases {
        let RuntimeValue::Seq(seq) = value else {
            panic!("dense helper returns a sequence");
        };
        let mut flat = vec![99];
        assert!(!seq.copy_i64_values_to(&mut flat));
        assert_eq!(flat, vec![99]);
        assert_eq!(seq.first_i64(), None);

        let mut visited = Vec::new();
        let compatible = seq
            .try_for_each_i64::<()>(|value| {
                visited.push(value);
                Ok(())
            })
            .expect("visitor does not fail");
        assert!(!compatible);
        assert!(visited.is_empty());
    }
}

#[test]
fn dense_textual_sequences_expose_typed_views_and_materialize_values() {
    let strings_seq = runtime_sequence_dense_strings(vec!["a".to_owned(), "b".to_owned()]);
    let entities_seq =
        runtime_sequence_dense_entity_refs(vec!["char.alice".to_owned(), "char.bob".to_owned()]);

    assert_eq!(runtime_value_label(&strings_seq), "seq/strings/2");
    assert_eq!(runtime_value_label(&entities_seq), "seq/entity_refs/2");

    let RuntimeValue::Seq(strings_seq) = strings_seq else {
        panic!("dense strings helper returns a sequence");
    };
    assert_eq!(
        strings_seq.as_strings(),
        Some(["a".to_owned(), "b".to_owned()].as_slice())
    );
    assert_eq!(
        strings_seq.into_values(),
        vec![
            RuntimeValue::String("a".to_owned()),
            RuntimeValue::String("b".to_owned())
        ]
    );

    let RuntimeValue::Seq(entities_seq) = entities_seq else {
        panic!("dense entity refs helper returns a sequence");
    };
    assert_eq!(
        entities_seq.as_entity_refs(),
        Some(["char.alice".to_owned(), "char.bob".to_owned()].as_slice())
    );
    assert_eq!(
        entities_seq.into_values(),
        vec![
            RuntimeValue::EntityRef("char.alice".to_owned()),
            RuntimeValue::EntityRef("char.bob".to_owned())
        ]
    );
}

#[test]
fn literal_and_repeat_sequences_choose_dense_scalar_storage() {
    let RuntimeValue::Seq(unit_seq) = runtime_sequence_from_literal_values(vec![
        RuntimeValue::Unit,
        RuntimeValue::Unit,
        RuntimeValue::Unit,
    ]) else {
        panic!("unit literals lower to a sequence");
    };
    assert_eq!(unit_seq.unit_len(), Some(3));

    let RuntimeValue::Seq(bool_seq) = runtime_sequence_from_literal_values(vec![
        RuntimeValue::Bool(true),
        RuntimeValue::Bool(false),
    ]) else {
        panic!("bool literals lower to a sequence");
    };
    assert_eq!(bool_seq.as_bool_slice(), Some([true, false].as_slice()));

    let RuntimeValue::Seq(char_seq) = runtime_sequence_repeat_value(&RuntimeValue::Char('x'), 3)
    else {
        panic!("char repeat lowers to a sequence");
    };
    assert_eq!(char_seq.as_chars(), Some(['x', 'x', 'x'].as_slice()));

    let RuntimeValue::Seq(duration_seq) =
        runtime_sequence_repeat_value(&RuntimeValue::Duration(LogicalDuration::from_nanos(7)), 2)
    else {
        panic!("duration repeat lowers to a sequence");
    };
    assert_eq!(
        duration_seq.as_durations(),
        Some(
            [
                LogicalDuration::from_nanos(7),
                LogicalDuration::from_nanos(7),
            ]
            .as_slice()
        )
    );

    let RuntimeValue::Seq(string_seq) =
        runtime_sequence_from_literal_values(vec![RuntimeValue::String("ok".to_owned())])
    else {
        panic!("string literals lower to a sequence");
    };
    assert_eq!(string_seq.as_strings(), Some(["ok".to_owned()].as_slice()));

    let RuntimeValue::Seq(i128_seq) =
        runtime_sequence_from_literal_values(vec![RuntimeValue::i128(1), RuntimeValue::i128(2)])
    else {
        panic!("i128 literals lower to a sequence");
    };
    assert_eq!(i128_seq.as_i128_slice(), Some([1, 2].as_slice()));

    let RuntimeValue::Seq(i32_seq) =
        runtime_sequence_from_literal_values(vec![RuntimeValue::i32(1), RuntimeValue::i32(2)])
    else {
        panic!("i32 literals lower to a sequence");
    };
    assert_eq!(i32_seq.dense_kind(), Some(DenseSeqKind::I32));
    assert_eq!(i32_seq.value_at(0), RuntimeValue::i32(1));
    assert_eq!(
        i32_seq.clone().into_values(),
        vec![RuntimeValue::i32(1), RuntimeValue::i32(2)]
    );

    let RuntimeValue::Seq(u8_seq) =
        runtime_sequence_from_literal_values(vec![RuntimeValue::u8(3), RuntimeValue::u8(4)])
    else {
        panic!("u8 literals lower to a sequence");
    };
    assert_eq!(u8_seq.dense_kind(), Some(DenseSeqKind::U8));
    assert_eq!(u8_seq.value_at(1), RuntimeValue::u8(4));
    assert_eq!(
        u8_seq.clone().into_values(),
        vec![RuntimeValue::u8(3), RuntimeValue::u8(4)]
    );

    let RuntimeValue::Seq(usize_seq) = runtime_sequence_repeat_value(&RuntimeValue::usize(4), 2)
    else {
        panic!("usize repeat lowers to a sequence");
    };
    assert_eq!(usize_seq.as_usize_values(), Some(vec![4, 4]));

    let RuntimeValue::Seq(unit_repeat_seq) = runtime_sequence_repeat_value(&RuntimeValue::Unit, 2)
    else {
        panic!("unit repeat lowers to a sequence");
    };
    assert_eq!(unit_repeat_seq.unit_len(), Some(2));

    let RuntimeValue::Seq(float_seq) = runtime_sequence_repeat_value(&RuntimeValue::F64(1.5), 2)
    else {
        panic!("typed float repeat lowers to a sequence");
    };
    assert_eq!(float_seq.as_f64_slice(), Some([(1.5), (1.5)].as_slice()));

    let RuntimeValue::Seq(entity_seq) =
        runtime_sequence_repeat_value(&RuntimeValue::EntityRef("char.alice".to_owned()), 2)
    else {
        panic!("entity ref repeat lowers to a sequence");
    };
    assert_eq!(
        entity_seq.as_entity_refs(),
        Some(["char.alice".to_owned(), "char.alice".to_owned()].as_slice())
    );
}

#[test]
fn compound_literal_sequences_use_columnar_storage_when_shape_is_stable() {
    let RuntimeValue::Seq(tuple_seq) = runtime_sequence_from_literal_values(vec![
        RuntimeValue::Tuple(vec![RuntimeValue::i64(1), RuntimeValue::Bool(true)]),
        RuntimeValue::Tuple(vec![RuntimeValue::i64(2), RuntimeValue::Bool(false)]),
    ]) else {
        panic!("tuple literals lower to a sequence");
    };
    let RuntimeSeq::TupleColumns(tuple_seq) = tuple_seq else {
        panic!("stable tuple rows lower to tuple columns");
    };
    assert_eq!(
        runtime_value_label(&RuntimeValue::Seq(RuntimeSeq::TupleColumns(
            tuple_seq.clone()
        ))),
        "seq/tuple_columns/2"
    );
    assert_eq!(tuple_seq.len(), 2);
    assert_eq!(tuple_seq.columns().len(), 2);
    assert_eq!(
        tuple_seq.column(0).and_then(RuntimeSeq::as_i64_slice),
        Some([1, 2].as_slice())
    );
    assert_eq!(
        tuple_seq.column(1).and_then(RuntimeSeq::as_bool_slice),
        Some([true, false].as_slice())
    );

    let RuntimeValue::Seq(record_seq) = runtime_sequence_from_literal_values(vec![
        crate::tests::runtime_record!([RuntimeFieldValue {
            name: "score".to_owned(),
            value: RuntimeValue::i64(1),
        }]),
        crate::tests::runtime_record!([RuntimeFieldValue {
            name: "score".to_owned(),
            value: RuntimeValue::i64(2),
        }]),
    ]) else {
        panic!("record literals lower to a sequence");
    };
    let RuntimeSeq::RecordColumns(record_seq) = record_seq else {
        panic!("stable record rows lower to record columns");
    };
    assert_eq!(
        runtime_value_label(&RuntimeValue::Seq(RuntimeSeq::RecordColumns(
            record_seq.clone()
        ))),
        "seq/record_columns/2"
    );
    assert_eq!(record_seq.len(), 2);
    assert_eq!(record_seq.fields().len(), 1);
    assert_eq!(
        record_seq
            .field_by_name("score")
            .and_then(RuntimeSeq::as_i64_slice),
        Some([1, 2].as_slice())
    );
    assert!(record_seq.field_by_name("missing").is_none());

    let RuntimeValue::Seq(variant_seq) = runtime_sequence_from_literal_values(vec![
        RuntimeValue::Variant {
            owner: RuntimeVariantIdentity::Option,
            ordinal: 0,
            name: "Some".to_owned(),
            payload: Some(Box::new(RuntimeValue::i64(1))),
        },
        RuntimeValue::Variant {
            owner: RuntimeVariantIdentity::Option,
            ordinal: 0,
            name: "Some".to_owned(),
            payload: Some(Box::new(RuntimeValue::i64(2))),
        },
    ]) else {
        panic!("variant literals lower to a sequence");
    };
    assert_eq!(variant_seq.dense_kind(), None);
}

#[test]
fn compound_literal_sequences_fall_back_when_shape_changes() {
    let RuntimeValue::Seq(tuple_seq) = runtime_sequence_from_literal_values(vec![
        RuntimeValue::Tuple(vec![RuntimeValue::i64(1)]),
        RuntimeValue::Tuple(vec![RuntimeValue::i64(2), RuntimeValue::Bool(false)]),
    ]) else {
        panic!("tuple literals lower to a sequence");
    };
    assert!(matches!(tuple_seq, RuntimeSeq::Values(_)));

    let RuntimeValue::Seq(record_seq) = runtime_sequence_from_literal_values(vec![
        crate::tests::runtime_record!([RuntimeFieldValue {
            name: "score".to_owned(),
            value: RuntimeValue::i64(1),
        }]),
        crate::tests::runtime_record!([RuntimeFieldValue {
            name: "label".to_owned(),
            value: RuntimeValue::String("two".to_owned()),
        }]),
    ]) else {
        panic!("record literals lower to a sequence");
    };
    assert!(matches!(record_seq, RuntimeSeq::Values(_)));
}

#[test]
fn dense_sequence_tail_preserves_storage_strategy() {
    let RuntimeValue::Seq(units) = runtime_sequence_dense_units(3) else {
        panic!("dense units helper returns a sequence");
    };

    let RuntimeSeq::Dense(unit_tail) = units.tail_from(1) else {
        panic!("dense unit tail remains dense");
    };

    assert_eq!(unit_tail.unit_len(), Some(2));

    let RuntimeValue::Seq(seq) = runtime_sequence_dense_chars(vec!['a', 'b', 'c']) else {
        panic!("dense chars helper returns a sequence");
    };

    let RuntimeSeq::Dense(tail) = seq.tail_from(1) else {
        panic!("dense tail remains dense");
    };

    assert_eq!(tail.as_chars(), Some(['b', 'c'].as_slice()));
}

#[test]
fn std_float_intrinsics_use_native_semantics_and_explicit_bit_conversion() {
    assert_eq!(
        evaluate_std_float_intrinsic(RuntimeIntrinsic::StdF32Sqrt, &[RuntimeValue::F32(4.0)])
            .expect("sqrt evaluates"),
        Some(RuntimeValue::F32(2.0))
    );
    assert_eq!(
        evaluate_std_float_intrinsic(
            RuntimeIntrinsic::StdF32MulAdd,
            &[
                RuntimeValue::F32(2.0),
                RuntimeValue::F32(3.0),
                RuntimeValue::F32(4.0)
            ],
        )
        .expect("mul_add evaluates"),
        Some(RuntimeValue::F32(10.0))
    );
    assert_eq!(
        evaluate_std_float_intrinsic(RuntimeIntrinsic::StdF32ToBits, &[RuntimeValue::F32(-0.0)])
            .expect("to_bits evaluates"),
        Some(RuntimeValue::u32((-0.0f32).to_bits()))
    );
    assert_eq!(
        evaluate_std_float_intrinsic(
            RuntimeIntrinsic::StdF32FromBits,
            &[RuntimeValue::u32(f32::NAN.to_bits())]
        )
        .expect("from_bits evaluates")
        .and_then(|value| match value {
            RuntimeValue::F32(value) => Some(value.to_bits()),
            _ => None,
        }),
        Some(f32::NAN.to_bits())
    );
    assert_eq!(
        evaluate_std_float_intrinsic(RuntimeIntrinsic::StdF64ToF32, &[RuntimeValue::F64(1.5)])
            .expect("to_f32 evaluates"),
        Some(RuntimeValue::F32(1.5))
    );
}
