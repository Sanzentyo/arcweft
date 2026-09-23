use super::*;
use crate::pattern::RuntimeCheckedType;

fn array(item: Schema, length: u64) -> Schema {
    Schema::Array {
        item: Box::new(item),
        length,
    }
}

#[test]
fn exact_arrays_share_checked_item_and_length_rules_across_value_storage() {
    let schema = array(Schema::Unit, 2);
    let checked = RuntimeCheckedType::Array {
        item: Box::new(RuntimeCheckedType::Unit),
        length: 2,
    };
    let limits = RuntimeSchemaLimits {
        max_validation_work: 3,
        max_nodes: 3,
        max_depth: 1,
        max_sequence_items: 2,
        ..RuntimeSchemaLimits::engine_default()
    };
    for value in [
        RuntimeValue::Seq(RuntimeSeq::dense_units(2)),
        RuntimeValue::Seq(RuntimeSeq::values(vec![RuntimeValue::Unit; 2])),
    ] {
        assert!(checked.accepts_value(&value));
        assert_eq!(
            schema.validate_value(&value, limits).unwrap(),
            value.try_digest(limits.platform_encoded_bytes()).unwrap()
        );
        assert!(
            schema
                .validate_value(
                    &value,
                    RuntimeSchemaLimits {
                        max_nodes: 2,
                        ..limits
                    }
                )
                .is_err()
        );
        assert!(
            schema
                .validate_value(
                    &value,
                    RuntimeSchemaLimits {
                        max_validation_work: 2,
                        ..limits
                    }
                )
                .is_err()
        );
        assert!(
            schema
                .validate_value(
                    &value,
                    RuntimeSchemaLimits {
                        max_depth: 0,
                        ..limits
                    }
                )
                .is_err()
        );
    }
    for value in [
        RuntimeValue::Seq(RuntimeSeq::dense_units(1)),
        RuntimeValue::Seq(RuntimeSeq::dense_units(3)),
        RuntimeValue::Seq(RuntimeSeq::values(vec![
            RuntimeValue::Unit,
            RuntimeValue::Bool(true),
        ])),
        RuntimeValue::Tuple(vec![RuntimeValue::Unit; 2]),
    ] {
        assert!(!checked.accepts_value(&value));
        assert!(
            schema
                .validate_value(&value, RuntimeSchemaLimits::engine_default())
                .is_err()
        );
    }
    let empty = RuntimeValue::Seq(RuntimeSeq::dense_units(0));
    assert!(
        array(Schema::Never, 0)
            .validate_value(&empty, limits)
            .is_ok()
    );
    assert!(
        RuntimeCheckedType::Array {
            item: Box::new(RuntimeCheckedType::Never),
            length: 0
        }
        .accepts_value(&empty)
    );
}

#[test]
fn array_length_mismatch_keeps_its_full_width_and_nested_choice_path() {
    let schema = Schema::Tuple(vec![array(Schema::Unit, u64::MAX)].into_boxed_slice());
    let value = RuntimeValue::Tuple(vec![RuntimeValue::Seq(RuntimeSeq::dense_units(1))]);
    assert_eq!(
        schema.validate_value(&value, RuntimeSchemaLimits::engine_default()),
        Err(Error::ArrayLength {
            path: "$[0]".to_owned(),
            expected: u64::MAX,
            actual: 1
        })
    );
    let choice =
        Schema::Choice(vec![array(Schema::Unit, 1), array(Schema::Unit, 2)].into_boxed_slice());
    assert!(
        choice
            .validate_value(
                &RuntimeValue::Seq(RuntimeSeq::dense_units(2)),
                RuntimeSchemaLimits::engine_default()
            )
            .is_ok()
    );
    assert!(matches!(
        choice.validate_value(
            &RuntimeValue::Seq(RuntimeSeq::dense_units(0)),
            RuntimeSchemaLimits::engine_default()
        ),
        Err(Error::ChoiceNoMatch { .. })
    ));
    let checked = RuntimeCheckedType::Array {
        item: Box::new(RuntimeCheckedType::Unit),
        length: u64::MAX,
    };
    assert!(!checked.accepts_value(&RuntimeValue::Seq(RuntimeSeq::dense_units(1))));
    assert_ne!(
        checked.semantic_identity_digest(),
        RuntimeCheckedType::Sequence(Box::new(RuntimeCheckedType::Unit)).semantic_identity_digest()
    );
    assert_ne!(
        checked.semantic_identity_digest(),
        RuntimeCheckedType::Array {
            item: Box::new(RuntimeCheckedType::Unit),
            length: 1
        }
        .semantic_identity_digest()
    );
}
