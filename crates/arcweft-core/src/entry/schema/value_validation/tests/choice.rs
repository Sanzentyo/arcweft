use super::*;

fn choice(alternatives: Vec<Schema>) -> Schema {
    Schema::Choice(alternatives.into_boxed_slice())
}

#[test]
fn unique_choice_emits_one_value_and_charges_every_alternative() {
    let schema = choice(vec![Schema::Bool, Schema::Unit]);
    let value = RuntimeValue::Unit;
    let limits = RuntimeSchemaLimits {
        max_depth: 1,
        max_nodes: 1,
        max_sequence_items: 2,
        max_validation_work: 5,
        max_encoded_bytes: u64::try_from(bytes(&value).len()).unwrap(),
        ..RuntimeSchemaLimits::engine_default()
    };
    assert_eq!(
        schema.validate_value(&value, limits).unwrap(),
        value.try_digest(limits.platform_encoded_bytes()).unwrap()
    );
    assert!(matches!(
        schema.validate_value(
            &value,
            RuntimeSchemaLimits {
                max_validation_work: 4,
                ..limits
            }
        ),
        Err(Error::ValidationWork {
            limit: 4,
            consumed: 5,
            ..
        })
    ));
    assert!(matches!(
        schema.validate_value(
            &value,
            RuntimeSchemaLimits {
                max_depth: 0,
                ..limits
            }
        ),
        Err(Error::ValidationDepth { limit: 0, .. })
    ));
}

#[test]
fn choice_reports_ordered_nested_mismatches() {
    let schema = choice(vec![
        Schema::Tuple(vec![Schema::Bool].into_boxed_slice()),
        Schema::Tuple(vec![Schema::String].into_boxed_slice()),
    ]);
    let value = RuntimeValue::Tuple(vec![RuntimeValue::u32(1)]);
    let Err(Error::ChoiceNoMatch { path, branches }) =
        schema.validate_value(&value, RuntimeSchemaLimits::engine_default())
    else {
        panic!("every branch must reject the nested value");
    };
    assert_eq!(path, "$");
    assert_eq!(
        branches
            .iter()
            .map(crate::entry::RuntimeSchemaChoiceMismatch::alternative)
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
    for (branch, expected) in branches.iter().zip(["bool", "string"]) {
        assert!(
            matches!(branch.source(), Error::Type { path, expected: actual, .. } if path == "$[0]" && *actual == expected)
        );
    }
    assert!(
        matches!(choice(vec![]).validate_value(&RuntimeValue::Unit, RuntimeSchemaLimits::engine_default()), Err(Error::ChoiceNoMatch { branches, .. }) if branches.is_empty())
    );
}

#[test]
fn ambiguity_waits_for_later_work_and_depth_failures() {
    let schema = choice(vec![Schema::Unit, Schema::Unit, choice(vec![Schema::Unit])]);
    let limits = RuntimeSchemaLimits {
        max_validation_work: 9,
        ..RuntimeSchemaLimits::engine_default()
    };
    assert!(matches!(
        schema.validate_value(&RuntimeValue::Unit, limits),
        Err(Error::ChoiceAmbiguous {
            first: 0,
            second: 1,
            ..
        })
    ));
    assert!(matches!(
        schema.validate_value(
            &RuntimeValue::Unit,
            RuntimeSchemaLimits {
                max_validation_work: 8,
                ..limits
            }
        ),
        Err(Error::ValidationWork { consumed: 9, .. })
    ));
    assert!(matches!(
        schema.validate_value(
            &RuntimeValue::Unit,
            RuntimeSchemaLimits {
                max_depth: 1,
                ..limits
            }
        ),
        Err(Error::ValidationDepth { limit: 1, .. })
    ));
    let invalid_schema = choice(vec![Schema::Unit, Schema::Named("missing".to_owned())]);
    assert!(
        matches!(invalid_schema.validate_value(&RuntimeValue::Unit, limits), Err(Error::UnresolvedNamed { name, .. }) if name == "missing")
    );
}

#[test]
fn sibling_choices_share_work_and_keep_parent_value_paths() {
    let schema = Schema::Tuple(
        vec![
            choice(vec![Schema::Bool, Schema::Unit]),
            choice(vec![Schema::Unit, Schema::Bool]),
        ]
        .into_boxed_slice(),
    );
    let value = RuntimeValue::Tuple(vec![RuntimeValue::Unit, RuntimeValue::Unit]);
    let limits = RuntimeSchemaLimits {
        max_nodes: 3,
        max_validation_work: 11,
        ..RuntimeSchemaLimits::engine_default()
    };
    assert_eq!(
        schema.validate_value(&value, limits).unwrap(),
        value.try_digest(limits.platform_encoded_bytes()).unwrap()
    );
    assert!(
        matches!(schema.validate_value(&value, RuntimeSchemaLimits { max_validation_work: 10, ..limits }), Err(Error::ValidationWork { path, consumed: 11, .. }) if path == "$[1]")
    );
    let wrong = RuntimeValue::Tuple(vec![
        RuntimeValue::Unit,
        RuntimeValue::String("wrong".to_owned()),
    ]);
    assert!(
        matches!(schema.validate_value(&wrong, limits), Err(Error::ChoiceNoMatch { path, branches }) if path == "$[1]" && branches.iter().all(|branch| matches!(branch.source(), Error::Type { path, .. } if path == "$[1]")))
    );
}

#[test]
fn a_failed_structural_candidate_can_be_followed_by_a_complete_candidate() {
    let record = |schema| Schema::RecordValue {
        fields: vec![RuntimeSchemaValueField::new(
            RuntimeRecordFieldId::try_from_zero_based_ordinal(0).unwrap(),
            "field".to_owned(),
            schema,
        )]
        .into_boxed_slice(),
    };
    let schema = choice(vec![
        record(Schema::I64),
        record(choice(vec![Schema::Bool, Schema::String])),
    ]);
    let value = RuntimeValue::try_record(vec![(
        "field".to_owned(),
        RuntimeValue::String("text".to_owned()),
    )])
    .unwrap();
    let limits = RuntimeSchemaLimits::engine_default();
    assert_eq!(
        schema.validate_value(&value, limits).unwrap(),
        value.try_digest(limits.platform_encoded_bytes()).unwrap()
    );
}

#[test]
fn opaque_choice_checks_owner_once_per_candidate_and_accounts_for_payload_once() {
    let schema = choice(vec![Schema::Bool, opaque_schema()]);
    let value = owner()
        .try_wrap(RuntimeValue::Seq(RuntimeSeq::dense_units(2)))
        .unwrap();
    let limits = RuntimeSchemaLimits {
        max_depth: 2,
        max_nodes: 4,
        max_sequence_items: 2,
        max_string_bytes: 1,
        max_validation_work: 5,
        max_encoded_bytes: u64::try_from(bytes(&value).len()).unwrap(),
    };
    assert_eq!(
        schema.validate_value(&value, limits).unwrap(),
        value.try_digest(limits.platform_encoded_bytes()).unwrap()
    );
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
fn deep_choices_use_the_continuation_stack_and_one_work_allowance() {
    let depth = 20_000;
    let mut schema = Schema::Unit;
    for _ in 0..depth {
        schema = choice(vec![schema]);
    }
    let limits = RuntimeSchemaLimits {
        max_depth: depth,
        max_nodes: 1,
        max_validation_work: u64::from(depth) * 2 + 1,
        ..RuntimeSchemaLimits::engine_default()
    };
    let accepted = schema.validate_value(&RuntimeValue::Unit, limits);
    let mismatch = schema.validate_value(&RuntimeValue::Bool(true), limits);
    let exhausted = schema.validate_value(
        &RuntimeValue::Unit,
        RuntimeSchemaLimits {
            max_validation_work: limits.max_validation_work - 1,
            ..limits
        },
    );
    schema.drop_iteratively();
    assert!(accepted.is_ok(), "{accepted:?}");
    assert!(matches!(&mismatch, Err(Error::ChoiceNoMatch { .. })));
    drop(mismatch);
    assert!(matches!(
        exhausted,
        Err(Error::ValidationWork {
            consumed: 40_001,
            ..
        })
    ));
}
