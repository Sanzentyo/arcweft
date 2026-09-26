use super::*;
use crate::entry::{
    RuntimeNominalSchemaBody, RuntimeNominalSchemaCase, RuntimeNominalSchemaDefinition,
    RuntimeNominalSchemaGraph, RuntimeNominalSchemaIdentity, RuntimeNominalTypeId,
};
use crate::pattern::RuntimeCheckedType;
use crate::time::LogicalDuration;
use crate::value::{Progress, RuntimeColor, RuntimeEntityReference};
use arcweft_id::{DeclarationIdentityFamily, PublicId};

fn reference() -> RuntimeValue {
    RuntimeValue::EntityRef(
        RuntimeEntityReference::try_project(
            DeclarationIdentityFamily::Character,
            PublicId::try_new("character.alice").unwrap(),
        )
        .unwrap(),
    )
}

#[test]
fn runtime_scalar_schemas_preserve_families_and_value_allowances() {
    let limits = RuntimeSchemaLimits::engine_default();
    let cases = [
        (
            Schema::Duration,
            RuntimeValue::Duration(LogicalDuration::from_nanos(17)),
        ),
        (
            Schema::Progress,
            RuntimeValue::Progress(Progress::new(0.5).unwrap().with_label("load")),
        ),
        (Schema::EntityReference, reference()),
        (
            Schema::Color,
            RuntimeValue::Color(RuntimeColor::new(12, 34, 56, 78)),
        ),
    ];
    for (expected, (schema, value)) in cases.iter().enumerate() {
        let encoded = bytes(value);
        let exact = RuntimeSchemaLimits {
            max_depth: 0,
            max_nodes: 1,
            max_encoded_bytes: u64::try_from(encoded.len()).unwrap(),
            ..limits
        };
        assert_eq!(
            schema.validate_value(value, exact).unwrap().as_bytes(),
            blake3::hash(&encoded).as_bytes()
        );
        for (actual, (_, other)) in cases.iter().enumerate() {
            assert_eq!(
                schema.validate_value(other, limits).is_ok(),
                expected == actual
            );
        }
        assert!(matches!(
            schema.validate_value(
                value,
                RuntimeSchemaLimits {
                    max_nodes: 0,
                    ..exact
                }
            ),
            Err(Error::BudgetExceeded { budget: "nodes" })
        ));
        assert!(matches!(
            schema.validate_value(
                value,
                RuntimeSchemaLimits {
                    max_encoded_bytes: exact.max_encoded_bytes - 1,
                    ..exact
                }
            ),
            Err(Error::BudgetExceeded {
                budget: "encoded_bytes"
            })
        ));
    }
    assert!(matches!(
        cases[1].0.validate_value(
            &cases[1].1,
            RuntimeSchemaLimits {
                max_string_bytes: 3,
                ..limits
            }
        ),
        Err(Error::BudgetExceeded {
            budget: "string_bytes"
        })
    ));
    assert!(matches!(
        cases[2].0.validate_value(
            &cases[2].1,
            RuntimeSchemaLimits {
                max_string_bytes: 1,
                ..limits
            }
        ),
        Err(Error::BudgetExceeded {
            budget: "string_bytes"
        })
    ));
    assert!(
        Schema::Duration
            .validate_value(&RuntimeValue::u64(17), limits)
            .is_err()
    );
    assert!(
        Schema::Progress
            .validate_value(&RuntimeValue::F32(0.5), limits)
            .is_err()
    );
    assert!(
        Schema::EntityReference
            .validate_value(&RuntimeValue::String("character.alice".to_owned()), limits)
            .is_err()
    );
}

#[test]
fn never_is_uninhabited_but_option_none_remains_inhabited() {
    let limits = RuntimeSchemaLimits::engine_default();
    assert!(matches!(
        Schema::Never.validate_value(&RuntimeValue::Unit, limits),
        Err(Error::Type {
            expected: "never",
            ..
        })
    ));
    let schema = Schema::option(Schema::Never);
    assert!(
        schema
            .validate_value(&RuntimeValue::option_none(), limits)
            .is_ok()
    );
    assert!(
        schema
            .validate_value(&RuntimeValue::option_some(RuntimeValue::Unit), limits)
            .is_err()
    );
}

#[test]
fn agent_value_schema_shares_the_checked_value_algebra() {
    let limits = RuntimeSchemaLimits::engine_default();
    let record = |value| RuntimeValue::try_record(vec![("field".to_owned(), value)]).unwrap();
    let cases = [
        RuntimeValue::Unit,
        RuntimeValue::Bool(true),
        RuntimeValue::i64(-1),
        RuntimeValue::u64(2),
        RuntimeValue::F64(1.5),
        RuntimeValue::String("text".to_owned()),
        reference(),
        RuntimeValue::Seq(RuntimeSeq::dense_i64(vec![-1, 2])),
        record(RuntimeValue::Seq(RuntimeSeq::dense_u64(vec![1, 2]))),
        RuntimeValue::i8(1),
        RuntimeValue::u32(1),
        RuntimeValue::F32(1.0),
        RuntimeValue::F64(f64::NAN),
        RuntimeValue::Char('x'),
        RuntimeValue::Duration(LogicalDuration::from_nanos(1)),
        RuntimeValue::Tuple(vec![]),
        RuntimeValue::option_none(),
        RuntimeValue::Seq(RuntimeSeq::dense_u8(vec![1])),
        record(RuntimeValue::i16(1)),
        owner().try_wrap(RuntimeValue::Unit).unwrap(),
    ];
    for value in cases {
        let accepted = Schema::AgentValue.validate_value(&value, limits);
        assert_eq!(
            accepted.is_ok(),
            RuntimeCheckedType::AgentValue.accepts_value(&value),
            "{value:?}"
        );
        if let Ok(digest) = accepted {
            assert_eq!(digest.as_bytes(), blake3::hash(&bytes(&value)).as_bytes());
        }
    }
}

#[test]
fn agent_value_descendants_share_nodes_depth_and_scalar_limits() {
    let value = RuntimeValue::try_record(vec![(
        "x".to_owned(),
        RuntimeValue::Seq(RuntimeSeq::dense_strings(vec![
            "a".to_owned(),
            "b".to_owned(),
        ])),
    )])
    .unwrap();
    let limits = RuntimeSchemaLimits {
        max_validation_work: RuntimeSchemaLimits::engine_default().max_validation_work,
        max_nodes: 4,
        max_depth: 2,
        max_sequence_items: 2,
        max_string_bytes: 1,
        max_encoded_bytes: u64::try_from(bytes(&value).len()).unwrap(),
    };
    assert!(Schema::AgentValue.validate_value(&value, limits).is_ok());
    assert!(matches!(
        Schema::AgentValue.validate_value(
            &value,
            RuntimeSchemaLimits {
                max_nodes: 3,
                ..limits
            }
        ),
        Err(Error::BudgetExceeded { budget: "nodes" })
    ));
    assert!(matches!(
        Schema::AgentValue.validate_value(
            &value,
            RuntimeSchemaLimits {
                max_depth: 1,
                ..limits
            }
        ),
        Err(Error::BudgetExceeded { budget: "depth" })
    ));
    assert!(matches!(
        Schema::AgentValue.validate_value(
            &value,
            RuntimeSchemaLimits {
                max_string_bytes: 0,
                ..limits
            }
        ),
        Err(Error::BudgetExceeded {
            budget: "string_bytes"
        })
    ));
    let wrong = RuntimeValue::try_record(vec![(
        "x".to_owned(),
        RuntimeValue::Seq(RuntimeSeq::dense_u8(vec![1])),
    )])
    .unwrap();
    assert!(
        matches!(Schema::AgentValue.validate_value(&wrong, limits), Err(Error::Type { path, expected: "AgentValue", .. }) if path == "$.x[0]")
    );
}

#[test]
fn nominal_variant_payloads_use_runtime_scalar_schema_authority() {
    let limits = RuntimeSchemaLimits::engine_default();
    let identity = RuntimeNominalSchemaIdentity::new(
        RuntimeNominalTypeId::try_new("motion.Policy").unwrap(),
        RuntimeSemanticTypeId::from_bytes([53; 32]),
    );
    let graph = RuntimeNominalSchemaGraph::try_new(
        vec![RuntimeNominalSchemaDefinition::new(
            identity.clone(),
            vec![],
            RuntimeNominalSchemaBody::Variant {
                cases: vec![
                    RuntimeNominalSchemaCase::new(
                        0,
                        "Stop".to_owned(),
                        Some(Schema::RecordValue {
                            fields: vec![RuntimeSchemaValueField::new(
                                RuntimeRecordFieldId::try_from_zero_based_ordinal(0).unwrap(),
                                "fade".to_owned(),
                                Schema::Duration,
                            )]
                            .into_boxed_slice(),
                        }),
                    ),
                    RuntimeNominalSchemaCase::new(1, "Cut".to_owned(), None),
                ]
                .into_boxed_slice(),
            },
        )],
        limits,
    )
    .unwrap();
    let owner = RuntimeVariantIdentity::Nominal {
        nominal: identity.nominal().clone(),
        semantic_identity: identity.semantic_identity(),
        layout: graph.try_layout_hash(identity.semantic_identity()).unwrap(),
    };
    let value = |fade| RuntimeValue::Variant {
        owner: owner.clone(),
        ordinal: 0,
        name: "Stop".to_owned(),
        payload: Some(Box::new(
            RuntimeValue::try_record(vec![("fade".to_owned(), fade)]).unwrap(),
        )),
    };
    let valid = value(RuntimeValue::Duration(LogicalDuration::from_nanos(10)));
    assert_eq!(
        graph
            .accepts_value(identity.semantic_identity(), &valid, limits)
            .unwrap(),
        valid.try_digest(limits.platform_encoded_bytes()).unwrap()
    );
    assert!(
        matches!(graph.accepts_value(identity.semantic_identity(), &value(RuntimeValue::u64(10)), limits), Err(Error::Type { path, expected: "duration", .. }) if path == "$.Stop.fade")
    );
}
