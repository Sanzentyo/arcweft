use crate::{
    time::LogicalDuration,
    value::{
        DenseSeqKind, RuntimeBinding, RuntimeEnv, RuntimeSeq, RuntimeValue,
        runtime_sequence_dense_bool, runtime_sequence_dense_bytes, runtime_sequence_dense_chars,
        runtime_sequence_dense_durations, runtime_sequence_dense_entity_refs,
        runtime_sequence_dense_float_literals, runtime_sequence_dense_i8,
        runtime_sequence_dense_i16, runtime_sequence_dense_i32, runtime_sequence_dense_i64,
        runtime_sequence_dense_i128, runtime_sequence_dense_isize, runtime_sequence_dense_strings,
        runtime_sequence_dense_u8, runtime_sequence_dense_u16, runtime_sequence_dense_u32,
        runtime_sequence_dense_u64, runtime_sequence_dense_u128, runtime_sequence_dense_units,
        runtime_sequence_dense_usize, runtime_sequence_from_literal_values,
        runtime_sequence_repeat_value, runtime_value_label,
    },
};

#[test]
fn root_binding_ref_updates_existing_slots() {
    let mut env = RuntimeEnv::default();
    let first = [RuntimeBinding {
        name: "seed".to_owned(),
        value: RuntimeValue::Int(1),
    }];
    let second = [RuntimeBinding {
        name: "seed".to_owned(),
        value: RuntimeValue::Int(2),
    }];

    env.bind_all_root_ref(&first);
    env.bind_all_root_ref(&second);

    assert_eq!(env.get("seed"), Some(&RuntimeValue::Int(2)));
}

#[test]
fn root_binding_ref_reuses_matching_ordered_slots() {
    let mut env = RuntimeEnv::default();
    let first = [
        RuntimeBinding {
            name: "lhs".to_owned(),
            value: RuntimeValue::Int(1),
        },
        RuntimeBinding {
            name: "rhs".to_owned(),
            value: RuntimeValue::Int(2),
        },
    ];
    let second = [
        RuntimeBinding {
            name: "lhs".to_owned(),
            value: RuntimeValue::Int(3),
        },
        RuntimeBinding {
            name: "rhs".to_owned(),
            value: RuntimeValue::Int(4),
        },
    ];

    env.bind_all_root_ref(&first);
    env.bind_all_root_ref(&second);

    assert_eq!(env.get("lhs"), Some(&RuntimeValue::Int(3)));
    assert_eq!(env.get("rhs"), Some(&RuntimeValue::Int(4)));
}

#[test]
fn scoped_i64_binding_updates_without_value_input() {
    let mut env = RuntimeEnv::default();

    env.push_scope_with_capacity(1);
    env.set_i64("item", 3);
    env.set_i64("item", 5);

    assert_eq!(env.get("item"), Some(&RuntimeValue::Int(5)));
}

#[test]
fn spare_scopes_do_not_affect_runtime_env_semantics() {
    let mut env = RuntimeEnv::default();
    env.push_scope_with_capacity(2);
    env.set("scoped", RuntimeValue::Int(1));
    env.pop_scope();

    let baseline = RuntimeEnv::default();
    assert_eq!(env, baseline);
    assert_eq!(env.clone(), baseline);

    env.push_scope_with_capacity(1);
    assert!(env.get("scoped").is_none());
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
            runtime_sequence_dense_float_literals(vec!["1.0f64".to_owned()]),
            DenseSeqKind::FloatLiterals,
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
    let i32_seq = runtime_sequence_dense_i32(vec![1, 2, 3]);
    let i64_seq = runtime_sequence_dense_i64(vec![1, 2, 3]);
    let u64_seq = runtime_sequence_dense_u64(vec![1, 2]);
    let bool_seq = runtime_sequence_dense_bool(vec![true, false]);
    let bytes_seq = runtime_sequence_dense_bytes(vec![65, 66]);
    let chars_seq = runtime_sequence_dense_chars(vec!['a', 'b']);
    let duration_seq = runtime_sequence_dense_durations(vec![LogicalDuration::from_nanos(3)]);

    assert_eq!(runtime_value_label(&unit_seq), "seq/units/3");
    assert_eq!(runtime_value_label(&i32_seq), "seq/i32/3");
    assert_eq!(runtime_value_label(&u64_seq), "seq/u64/2");
    assert_eq!(runtime_value_label(&bool_seq), "seq/bool/2");
    assert_eq!(runtime_value_label(&bytes_seq), "seq/bytes/2");
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

    let RuntimeValue::Seq(i64_seq) = &i64_seq else {
        panic!("dense i64 helper returns a sequence");
    };
    assert_eq!(i64_seq.as_i64_slice(), Some([1, 2, 3].as_slice()));

    let RuntimeValue::Seq(bool_seq) = bool_seq else {
        panic!("dense bool helper returns a sequence");
    };
    assert_eq!(bool_seq.as_bool_slice(), Some([true, false].as_slice()));
    assert_eq!(
        bool_seq.into_values(),
        vec![RuntimeValue::Bool(true), RuntimeValue::Bool(false)]
    );

    let RuntimeValue::Seq(i32_seq) = i32_seq else {
        panic!("dense i32 helper returns a sequence");
    };
    assert_eq!(i32_seq.as_i32_slice(), Some([1, 2, 3].as_slice()));
    assert_eq!(i32_seq.sum_as_i64(), Some(6));
    let mut flat = Vec::new();
    assert!(i32_seq.copy_int_compatible_i64_values_to(&mut flat));
    assert_eq!(flat, vec![1, 2, 3]);
    assert_eq!(i32_seq.first_int_compatible_i64(), Some(Some(1)));
    assert_eq!(
        i32_seq.into_values(),
        vec![
            RuntimeValue::Int(1),
            RuntimeValue::Int(2),
            RuntimeValue::Int(3)
        ]
    );

    let RuntimeValue::Seq(u64_seq) = u64_seq else {
        panic!("dense u64 helper returns a sequence");
    };
    assert_eq!(u64_seq.as_u64_slice(), Some([1, 2].as_slice()));
    assert_eq!(u64_seq.sum_as_i64(), Some(3));
    assert_eq!(
        u64_seq.into_values(),
        vec![RuntimeValue::UInt(1), RuntimeValue::UInt(2)]
    );

    let RuntimeValue::Seq(bytes_seq) = bytes_seq else {
        panic!("dense bytes helper returns a sequence");
    };
    assert_eq!(bytes_seq.as_bytes(), Some([65, 66].as_slice()));
    let mut flat = Vec::new();
    assert!(bytes_seq.copy_int_compatible_i64_values_to(&mut flat));
    assert_eq!(flat, vec![65, 66]);
    assert_eq!(bytes_seq.first_int_compatible_i64(), Some(Some(65)));
    assert_eq!(
        bytes_seq.into_values(),
        vec![RuntimeValue::Int(65), RuntimeValue::Int(66)]
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
    assert!(!i128_seq.copy_int_compatible_i64_values_to(&mut flat));
    assert!(flat.is_empty());
    assert_eq!(i128_seq.first_int_compatible_i64(), None);
    assert_eq!(
        i128_seq.into_values(),
        vec![
            RuntimeValue::I128(1),
            RuntimeValue::I128(2),
            RuntimeValue::I128(3)
        ]
    );

    let RuntimeValue::Seq(isize_seq) = isize_seq else {
        panic!("dense isize helper returns a sequence");
    };
    assert_eq!(isize_seq.as_isize_values(), Some([1, 2, 3].as_slice()));
    assert_eq!(isize_seq.sum_as_i64(), Some(6));
    assert_eq!(
        isize_seq.into_values(),
        vec![
            RuntimeValue::ISize(1),
            RuntimeValue::ISize(2),
            RuntimeValue::ISize(3)
        ]
    );

    let RuntimeValue::Seq(u128_seq) = u128_seq else {
        panic!("dense u128 helper returns a sequence");
    };
    assert_eq!(u128_seq.as_u128_slice(), Some([1, 2].as_slice()));
    assert_eq!(u128_seq.sum_as_i64(), Some(3));
    assert_eq!(
        u128_seq.into_values(),
        vec![RuntimeValue::U128(1), RuntimeValue::U128(2)]
    );

    let RuntimeValue::Seq(usize_seq) = usize_seq else {
        panic!("dense usize helper returns a sequence");
    };
    assert_eq!(usize_seq.as_usize_values(), Some([1, 2].as_slice()));
    assert_eq!(usize_seq.sum_as_i64(), Some(3));
    assert_eq!(
        usize_seq.into_values(),
        vec![RuntimeValue::USize(1), RuntimeValue::USize(2)]
    );
}

#[test]
fn dense_textual_sequences_expose_typed_views_and_materialize_values() {
    let strings_seq = runtime_sequence_dense_strings(vec!["a".to_owned(), "b".to_owned()]);
    let floats_seq =
        runtime_sequence_dense_float_literals(vec!["1.0f64".to_owned(), "2.0f64".to_owned()]);
    let entities_seq =
        runtime_sequence_dense_entity_refs(vec!["char.alice".to_owned(), "char.bob".to_owned()]);

    assert_eq!(runtime_value_label(&strings_seq), "seq/strings/2");
    assert_eq!(runtime_value_label(&floats_seq), "seq/float_literals/2");
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

    let RuntimeValue::Seq(floats_seq) = floats_seq else {
        panic!("dense float literal helper returns a sequence");
    };
    assert_eq!(
        floats_seq.as_float_literals(),
        Some(["1.0f64".to_owned(), "2.0f64".to_owned()].as_slice())
    );
    assert_eq!(
        floats_seq.into_values(),
        vec![
            RuntimeValue::Float("1.0f64".to_owned()),
            RuntimeValue::Float("2.0f64".to_owned())
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
        runtime_sequence_from_literal_values(vec![RuntimeValue::I128(1), RuntimeValue::I128(2)])
    else {
        panic!("i128 literals lower to a sequence");
    };
    assert_eq!(i128_seq.as_i128_slice(), Some([1, 2].as_slice()));

    let RuntimeValue::Seq(usize_seq) = runtime_sequence_repeat_value(&RuntimeValue::USize(4), 2)
    else {
        panic!("usize repeat lowers to a sequence");
    };
    assert_eq!(usize_seq.as_usize_values(), Some([4, 4].as_slice()));

    let RuntimeValue::Seq(unit_repeat_seq) = runtime_sequence_repeat_value(&RuntimeValue::Unit, 2)
    else {
        panic!("unit repeat lowers to a sequence");
    };
    assert_eq!(unit_repeat_seq.unit_len(), Some(2));

    let RuntimeValue::Seq(float_seq) =
        runtime_sequence_repeat_value(&RuntimeValue::Float("1.5f64".to_owned()), 2)
    else {
        panic!("float literal repeat lowers to a sequence");
    };
    assert_eq!(
        float_seq.as_float_literals(),
        Some(["1.5f64".to_owned(), "1.5f64".to_owned()].as_slice())
    );

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
