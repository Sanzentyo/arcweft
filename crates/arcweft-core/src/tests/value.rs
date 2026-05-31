use crate::{
    time::LogicalDuration,
    value::{
        RuntimeBinding, RuntimeEnv, RuntimeSeq, RuntimeValue, runtime_sequence_dense_bool,
        runtime_sequence_dense_bytes, runtime_sequence_dense_chars,
        runtime_sequence_dense_durations, runtime_sequence_dense_i8, runtime_sequence_dense_i16,
        runtime_sequence_dense_i32, runtime_sequence_dense_i64, runtime_sequence_dense_u8,
        runtime_sequence_dense_u16, runtime_sequence_dense_u32, runtime_sequence_dense_u64,
        runtime_value_label,
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
fn dense_sequences_expose_typed_views_and_materialize_values() {
    let i8_seq = runtime_sequence_dense_i8(vec![1, 2]);
    let i16_seq = runtime_sequence_dense_i16(vec![1, 2]);
    let i32_seq = runtime_sequence_dense_i32(vec![1, 2, 3]);
    let i64_seq = runtime_sequence_dense_i64(vec![1, 2, 3]);
    let u8_seq = runtime_sequence_dense_u8(vec![1, 2]);
    let u16_seq = runtime_sequence_dense_u16(vec![1, 2]);
    let u32_seq = runtime_sequence_dense_u32(vec![1, 2]);
    let u64_seq = runtime_sequence_dense_u64(vec![1, 2]);
    let bool_seq = runtime_sequence_dense_bool(vec![true, false]);
    let bytes_seq = runtime_sequence_dense_bytes(vec![65, 66]);
    let chars_seq = runtime_sequence_dense_chars(vec!['a', 'b']);
    let duration_seq = runtime_sequence_dense_durations(vec![LogicalDuration::from_nanos(3)]);

    assert_eq!(runtime_value_label(&i8_seq), "seq/i8/2");
    assert_eq!(runtime_value_label(&i16_seq), "seq/i16/2");
    assert_eq!(runtime_value_label(&i32_seq), "seq/i32/3");
    let RuntimeValue::Seq(i64_seq) = &i64_seq else {
        panic!("dense i64 helper returns a sequence");
    };
    assert_eq!(i64_seq.as_i64_slice(), Some([1, 2, 3].as_slice()));
    assert_eq!(runtime_value_label(&u8_seq), "seq/u8/2");
    assert_eq!(runtime_value_label(&u16_seq), "seq/u16/2");
    assert_eq!(runtime_value_label(&u32_seq), "seq/u32/2");
    assert_eq!(runtime_value_label(&u64_seq), "seq/u64/2");
    assert_eq!(runtime_value_label(&bool_seq), "seq/bool/2");
    assert_eq!(runtime_value_label(&bytes_seq), "seq/bytes/2");
    assert_eq!(runtime_value_label(&chars_seq), "seq/chars/2");
    assert_eq!(runtime_value_label(&duration_seq), "seq/durations/1");

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
fn dense_sequence_tail_preserves_storage_strategy() {
    let RuntimeValue::Seq(seq) = runtime_sequence_dense_chars(vec!['a', 'b', 'c']) else {
        panic!("dense chars helper returns a sequence");
    };

    let RuntimeSeq::Dense(tail) = seq.tail_from(1) else {
        panic!("dense tail remains dense");
    };

    assert_eq!(tail.as_chars(), Some(['b', 'c'].as_slice()));
}
