use super::*;
mod array;
mod atoms;
mod choice;
use crate::entry::{RuntimeSchemaLimits, RuntimeSchemaValueField};
use crate::pattern::{RuntimeOpaqueTypeOwner, RuntimeOpaqueTypeProducerId, RuntimeSemanticTypeId};
use crate::value::{
    RuntimeAgentPredicate, RuntimeAgentValue, RuntimeRecordFieldId, RuntimeRecordValue, RuntimeSeq,
    RuntimeValue,
};

fn owner() -> RuntimeOpaqueTypeOwner {
    RuntimeOpaqueTypeOwner::exact(
        RuntimeOpaqueTypeProducerId::try_new("p").unwrap(),
        RuntimeSemanticTypeId::from_bytes([7; 32]),
    )
}

fn opaque_schema() -> Schema {
    Schema::ExactOpaque {
        owner: owner(),
        arguments: vec![Schema::Bool].into_boxed_slice(),
    }
}

fn bytes(value: &RuntimeValue) -> Vec<u8> {
    super::super::canonical_runtime_value_bytes(value, 1_000_000).unwrap()
}

#[test]
fn opaque_payloads_share_the_enclosing_value_and_byte_allowance() {
    let schema = Schema::Tuple(vec![opaque_schema(), opaque_schema()].into_boxed_slice());
    let value = RuntimeValue::Tuple(vec![
        owner()
            .try_wrap(RuntimeValue::Seq(RuntimeSeq::dense_units(2)))
            .unwrap(),
        owner()
            .try_wrap(RuntimeValue::Seq(RuntimeSeq::dense_units(2)))
            .unwrap(),
    ]);
    let encoded = bytes(&value);
    let limits = RuntimeSchemaLimits {
        max_validation_work: RuntimeSchemaLimits::engine_default().max_validation_work,
        max_nodes: 9,
        max_depth: 3,
        max_sequence_items: 2,
        max_string_bytes: 1,
        max_encoded_bytes: u64::try_from(encoded.len()).unwrap(),
    };
    let digest = schema.validate_value(&value, limits).unwrap();
    assert_eq!(digest.as_bytes(), blake3::hash(&encoded).as_bytes());
    assert!(matches!(
        schema.validate_value(
            &value,
            RuntimeSchemaLimits {
                max_nodes: 8,
                ..limits
            }
        ),
        Err(Error::BudgetExceeded { budget: "nodes" })
    ));
    assert!(matches!(
        schema.validate_value(
            &value,
            RuntimeSchemaLimits {
                max_depth: 2,
                ..limits
            }
        ),
        Err(Error::BudgetExceeded { budget: "depth" })
    ));
    assert!(matches!(
        schema.validate_value(
            &value,
            RuntimeSchemaLimits {
                max_sequence_items: 1,
                ..limits
            }
        ),
        Err(Error::BudgetExceeded {
            budget: "sequence_items"
        })
    ));
    assert!(matches!(
        schema.validate_value(
            &value,
            RuntimeSchemaLimits {
                max_encoded_bytes: limits.max_encoded_bytes - 1,
                ..limits
            }
        ),
        Err(Error::BudgetExceeded {
            budget: "encoded_bytes"
        })
    ));
}

#[test]
fn agent_structure_and_text_inside_opaque_payloads_are_bounded() {
    let schema = opaque_schema();
    let value = owner()
        .try_wrap(RuntimeValue::Agent(RuntimeAgentValue::Predicate(
            RuntimeAgentPredicate::Not {
                predicate: Box::new(RuntimeAgentPredicate::Not {
                    predicate: Box::new(RuntimeAgentPredicate::DiagnosticsHasError),
                }),
            },
        )))
        .unwrap();
    let limits = RuntimeSchemaLimits {
        max_depth: 3,
        max_nodes: 4,
        ..RuntimeSchemaLimits::engine_default()
    };
    schema.validate_value(&value, limits).unwrap();
    assert!(matches!(
        schema.validate_value(
            &value,
            RuntimeSchemaLimits {
                max_depth: 2,
                ..limits
            }
        ),
        Err(Error::BudgetExceeded { budget: "depth" })
    ));
    assert!(matches!(
        schema.validate_value(
            &value,
            RuntimeSchemaLimits {
                max_nodes: 3,
                ..limits
            }
        ),
        Err(Error::BudgetExceeded { budget: "nodes" })
    ));
    let text = owner()
        .try_wrap(RuntimeValue::Agent(RuntimeAgentValue::BinaryData(
            "four".to_owned(),
        )))
        .unwrap();
    assert!(matches!(
        schema.validate_value(
            &text,
            RuntimeSchemaLimits {
                max_string_bytes: 3,
                ..limits
            }
        ),
        Err(Error::BudgetExceeded {
            budget: "string_bytes"
        })
    ));
}

#[test]
fn columnar_records_preserve_exact_fields_and_logical_node_counts() {
    let field = |ordinal, name: &str, schema| {
        RuntimeSchemaValueField::new(
            RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal).unwrap(),
            name.to_owned(),
            schema,
        )
    };
    let schema = Schema::Seq(Box::new(Schema::RecordValue {
        fields: vec![field(0, "z", Schema::Bool), field(1, "a", Schema::I64)].into_boxed_slice(),
    }));
    let columnar = RuntimeValue::Seq(
        RuntimeSeq::record_columns(
            2,
            vec![
                ("z".to_owned(), RuntimeSeq::dense_bool(vec![true, false])),
                ("a".to_owned(), RuntimeSeq::dense_i64(vec![1, 2])),
            ],
        )
        .unwrap(),
    );
    let plain = RuntimeValue::Seq(RuntimeSeq::values(vec![
        RuntimeValue::Record(
            RuntimeRecordValue::try_new(vec![
                ("z".to_owned(), RuntimeValue::Bool(true)),
                ("a".to_owned(), RuntimeValue::i64(1)),
            ])
            .unwrap(),
        ),
        RuntimeValue::Record(
            RuntimeRecordValue::try_new(vec![
                ("z".to_owned(), RuntimeValue::Bool(false)),
                ("a".to_owned(), RuntimeValue::i64(2)),
            ])
            .unwrap(),
        ),
    ]));
    let limits = RuntimeSchemaLimits {
        max_nodes: 7,
        max_depth: 2,
        max_sequence_items: 2,
        ..RuntimeSchemaLimits::engine_default()
    };
    assert_eq!(
        schema.validate_value(&columnar, limits).unwrap(),
        schema.validate_value(&plain, limits).unwrap()
    );
    assert_eq!(bytes(&columnar), bytes(&plain));
    assert!(matches!(
        schema.validate_value(
            &columnar,
            RuntimeSchemaLimits {
                max_nodes: 6,
                ..limits
            }
        ),
        Err(Error::BudgetExceeded { budget: "nodes" })
    ));
    let reordered = Schema::Seq(Box::new(Schema::RecordValue {
        fields: vec![field(0, "a", Schema::I64), field(1, "z", Schema::Bool)].into_boxed_slice(),
    }));
    assert!(
        matches!(reordered.validate_value(&columnar, limits), Err(Error::RecordField { path, ordinal: 0 }) if path == "$[0]")
    );
}

#[test]
fn nested_columnar_tuple_rows_validate_without_materializing_rows() {
    let schema = Schema::Seq(Box::new(Schema::Tuple(
        vec![
            Schema::I8,
            Schema::Tuple(vec![Schema::String].into_boxed_slice()),
        ]
        .into_boxed_slice(),
    )));
    let value = RuntimeValue::Seq(
        RuntimeSeq::tuple_columns(
            2,
            vec![
                RuntimeSeq::dense_i8(vec![1, 2]),
                RuntimeSeq::tuple_columns(
                    2,
                    vec![RuntimeSeq::dense_strings(vec![
                        "a".to_owned(),
                        "b".to_owned(),
                    ])],
                )
                .unwrap(),
            ],
        )
        .unwrap(),
    );
    let limits = RuntimeSchemaLimits {
        max_nodes: 9,
        max_depth: 3,
        max_sequence_items: 2,
        max_string_bytes: 1,
        ..RuntimeSchemaLimits::engine_default()
    };
    schema.validate_value(&value, limits).unwrap();
    assert!(matches!(
        schema.validate_value(
            &value,
            RuntimeSchemaLimits {
                max_nodes: 8,
                ..limits
            }
        ),
        Err(Error::BudgetExceeded { budget: "nodes" })
    ));
    let RuntimeValue::Seq(sequence) = &value else {
        unreachable!()
    };
    let plain = RuntimeValue::Seq(RuntimeSeq::values(sequence.clone().into_values()));
    assert_eq!(bytes(&value), bytes(&plain));
}

#[test]
fn dense_scalar_storage_preserves_each_canonical_width_and_borrows_strings() {
    let sequences = vec![
        RuntimeSeq::dense_units(1),
        RuntimeSeq::dense_bool(vec![true]),
        RuntimeSeq::dense_i8(vec![-1]),
        RuntimeSeq::dense_i16(vec![-1]),
        RuntimeSeq::dense_i32(vec![-1]),
        RuntimeSeq::dense_i64(vec![-1]),
        RuntimeSeq::dense_i128(vec![-1]),
        RuntimeSeq::dense_isize(vec![-1]),
        RuntimeSeq::dense_u8(vec![1]),
        RuntimeSeq::dense_u16(vec![1]),
        RuntimeSeq::dense_u32(vec![1]),
        RuntimeSeq::dense_u64(vec![1]),
        RuntimeSeq::dense_u128(vec![1]),
        RuntimeSeq::dense_usize(vec![1]),
        RuntimeSeq::dense_f32(vec![-0.0]),
        RuntimeSeq::dense_f64(vec![-0.0]),
        RuntimeSeq::dense_bytes(vec![1]),
        RuntimeSeq::dense_chars(vec!['界']),
        RuntimeSeq::dense_strings(vec!["borrowed".to_owned()]),
    ];
    for sequence in sequences {
        let plain = RuntimeValue::Seq(RuntimeSeq::values(sequence.clone().into_values()));
        assert_eq!(bytes(&RuntimeValue::Seq(sequence)), bytes(&plain));
    }
    let strings = RuntimeSeq::dense_strings(vec!["borrowed".to_owned()]);
    let View::Scalar(Scalar::String(view)) = strings.value_view(0).unwrap() else {
        unreachable!()
    };
    assert_eq!(view.as_ptr(), strings.as_strings().unwrap()[0].as_ptr());
}

#[test]
fn errors_retain_the_nested_value_location() {
    let schema = Schema::Tuple(vec![Schema::result(Schema::I8, Schema::Bool)].into_boxed_slice());
    let wrong = RuntimeValue::Tuple(vec![RuntimeValue::result_ok(RuntimeValue::i16(1))]);
    assert!(
        matches!(schema.validate_value(&wrong, RuntimeSchemaLimits::engine_default()), Err(Error::Type { path, expected: "i8", .. }) if path == "$[0].Ok[0]")
    );
    let nonfinite = Schema::Tuple(vec![Schema::F32].into_boxed_slice());
    assert!(
        matches!(nonfinite.validate_value(&RuntimeValue::Tuple(vec![RuntimeValue::F32(f32::NAN)]), RuntimeSchemaLimits::engine_default()), Err(Error::NonFinite { path, .. }) if path == "$[0]")
    );
}

#[test]
fn deep_typed_values_use_the_selected_limit_without_recursive_calls() {
    let depth = 20_000;
    let mut schema = Schema::Unit;
    let mut value = RuntimeValue::Unit;
    for _ in 0..depth {
        schema = Schema::Tuple(vec![schema].into_boxed_slice());
        value = RuntimeValue::Tuple(vec![value]);
    }
    let limits = RuntimeSchemaLimits {
        max_depth: depth,
        max_nodes: depth + 1,
        max_sequence_items: 1,
        ..RuntimeSchemaLimits::engine_default()
    };
    let accepted = schema.validate_value(&value, limits);
    let denied = schema.validate_value(
        &value,
        RuntimeSchemaLimits {
            max_depth: depth - 1,
            ..limits
        },
    );
    schema.drop_iteratively();
    while let RuntimeValue::Tuple(mut items) = value {
        value = items.pop().unwrap();
    }
    assert!(accepted.is_ok(), "{accepted:?}");
    assert!(matches!(
        denied,
        Err(Error::BudgetExceeded { budget: "depth" })
    ));
}

#[test]
fn option_and_result_validate_the_real_builtin_payload_tuple() {
    let limits = RuntimeSchemaLimits::engine_default();
    for (schema, value) in [
        (
            Schema::option(Schema::Bool),
            RuntimeValue::option_some(RuntimeValue::Bool(true)),
        ),
        (Schema::option(Schema::Bool), RuntimeValue::option_none()),
        (
            Schema::result(Schema::Bool, Schema::String),
            RuntimeValue::result_ok(RuntimeValue::Bool(true)),
        ),
        (
            Schema::result(Schema::Bool, Schema::String),
            RuntimeValue::result_err(RuntimeValue::String("error".to_owned())),
        ),
    ] {
        assert_eq!(
            schema.validate_value(&value, limits).unwrap(),
            value.try_digest(1_000_000).unwrap()
        );
    }
    let schema = Schema::option(Schema::Bool);
    let actual = RuntimeValue::option_some(RuntimeValue::Bool(true));
    assert!(
        schema
            .validate_value(
                &actual,
                RuntimeSchemaLimits {
                    max_nodes: 3,
                    max_depth: 2,
                    ..limits
                }
            )
            .is_ok()
    );
    assert!(matches!(
        schema.validate_value(
            &actual,
            RuntimeSchemaLimits {
                max_nodes: 2,
                ..limits
            }
        ),
        Err(Error::BudgetExceeded { budget: "nodes" })
    ));
    assert!(matches!(
        schema.validate_value(
            &actual,
            RuntimeSchemaLimits {
                max_depth: 1,
                ..limits
            }
        ),
        Err(Error::BudgetExceeded { budget: "depth" })
    ));
    for payload in [
        RuntimeValue::Bool(true),
        RuntimeValue::Tuple(vec![]),
        RuntimeValue::Tuple(vec![RuntimeValue::Bool(true), RuntimeValue::Bool(false)]),
    ] {
        let mut malformed = actual.clone();
        let RuntimeValue::Variant { payload: slot, .. } = &mut malformed else {
            unreachable!()
        };
        *slot = Some(Box::new(payload));
        assert!(schema.validate_value(&malformed, limits).is_err());
    }
}
