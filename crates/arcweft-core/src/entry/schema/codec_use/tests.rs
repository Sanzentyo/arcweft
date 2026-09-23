use super::*;

#[test]
fn codec_use_preserves_different_nested_bytes_policies_without_type_identities() {
    let schema = RuntimeTypeSchema::Tuple(Box::new([
        RuntimeTypeSchema::Seq(Box::new(RuntimeTypeSchema::Bytes {
            format: RuntimeBytesFormat::Base64,
        })),
        RuntimeTypeSchema::Seq(Box::new(RuntimeTypeSchema::Bytes {
            format: RuntimeBytesFormat::Hex,
        })),
        RuntimeTypeSchema::option(RuntimeTypeSchema::Bytes {
            format: RuntimeBytesFormat::Array,
        }),
    ]));
    let limits = RuntimeSchemaLimits::engine_default();
    let policy = RuntimeCodecUse::from_schema(&schema, limits).unwrap();
    policy.validate_schema(&schema, limits).unwrap();
    let RuntimeCodecUse::Tuple { items } = &policy else {
        panic!("tuple policy");
    };
    assert_ne!(items[0], items[1]);
    let mut wrong = policy.clone();
    let RuntimeCodecUse::Tuple { items } = &mut wrong else {
        unreachable!()
    };
    items[1] = items[0].clone();
    assert!(wrong.validate_schema(&schema, limits).is_err());
    let encoded = serde_json::to_value(&policy).unwrap();
    assert_eq!(
        serde_json::from_value::<RuntimeCodecUse>(encoded).unwrap(),
        policy
    );
}

#[test]
fn codec_use_rejects_wrong_topology_and_bounded_extraction() {
    let schema = RuntimeTypeSchema::Map {
        kind: super::super::RuntimeMapKind::Sorted,
        key: Box::new(RuntimeTypeSchema::String),
        value: Box::new(RuntimeTypeSchema::Bytes {
            format: RuntimeBytesFormat::Hex,
        }),
    };
    let limits = RuntimeSchemaLimits::engine_default();
    let policy = RuntimeCodecUse::from_schema(&schema, limits).unwrap();
    assert!(
        policy
            .validate_schema(&RuntimeTypeSchema::Tuple(Box::new([])), limits)
            .is_err()
    );
    assert!(
        RuntimeCodecUse::from_schema(
            &schema,
            RuntimeSchemaLimits {
                max_validation_work: 1,
                ..limits
            }
        )
        .is_err()
    );
    assert!(
        RuntimeCodecUse::from_schema(&RuntimeTypeSchema::Named("unresolved".to_owned()), limits)
            .is_err()
    );
}

#[test]
fn nominal_layout_commits_field_wire_policy_and_rejects_changed_nested_bytes() {
    use crate::entry::{
        RuntimeNominalRecordShape, RuntimeNominalSchemaBody, RuntimeNominalSchemaDefinition,
        RuntimeNominalSchemaField, RuntimeNominalSchemaGraph, RuntimeNominalSchemaIdentity,
        RuntimeNominalTypeId,
    };
    use crate::pattern::RuntimeSemanticTypeId;
    use crate::value::RuntimeRecordFieldId;
    let identity = RuntimeSemanticTypeId::from_bytes([231; 32]);
    let schema = RuntimeTypeSchema::Seq(Box::new(RuntimeTypeSchema::Bytes {
        format: RuntimeBytesFormat::Base64,
    }));
    let make = |wire_name: &str, format| {
        let definition = RuntimeNominalSchemaDefinition::new(
            RuntimeNominalSchemaIdentity::new(
                RuntimeNominalTypeId::try_new("fixture.WirePolicy").unwrap(),
                identity,
            ),
            vec![],
            RuntimeNominalSchemaBody::Record {
                shape: RuntimeNominalRecordShape::Record,
                fields: vec![RuntimeNominalSchemaField::new(
                    RuntimeRecordFieldId::try_from_zero_based_ordinal(0).unwrap(),
                    Some("field".to_owned()),
                    schema.clone(),
                )]
                .into_boxed_slice(),
            },
        )
        .with_data_codec(RuntimeCodecUse::Record {
            name: "WirePolicy".to_owned(),
            deny_unknown_fields: true,
            fields: vec![RuntimeFieldCodecUse {
                wire_name: wire_name.to_owned(),
                has_default: false,
                default_program: None,
                skip: false,
                bytes_format: None,
                value: RuntimeCodecUse::Unary {
                    item: Box::new(RuntimeCodecUse::Bytes { format }),
                },
            }]
            .into_boxed_slice(),
        });
        RuntimeNominalSchemaGraph::try_new(vec![definition], RuntimeSchemaLimits::engine_default())
    };
    let first = make("first", RuntimeBytesFormat::Base64).unwrap();
    let second = make("second", RuntimeBytesFormat::Base64).unwrap();
    assert_ne!(
        first.try_layout_hash(identity).unwrap(),
        second.try_layout_hash(identity).unwrap()
    );
    assert!(make("first", RuntimeBytesFormat::Hex).is_err());
}

#[test]
fn nominal_layout_commits_the_exact_default_program_identity() {
    use crate::entry::{
        RuntimeNominalRecordShape, RuntimeNominalSchemaBody, RuntimeNominalSchemaDefinition,
        RuntimeNominalSchemaField, RuntimeNominalSchemaGraph, RuntimeNominalSchemaIdentity,
        RuntimeNominalTypeId,
    };
    use crate::pattern::RuntimeSemanticTypeId;
    use crate::value::RuntimeRecordFieldId;
    let semantic = RuntimeSemanticTypeId::from_bytes([233; 32]);
    let graph = |producer| {
        RuntimeNominalSchemaGraph::try_new(
            vec![
                RuntimeNominalSchemaDefinition::new(
                    RuntimeNominalSchemaIdentity::new(
                        RuntimeNominalTypeId::try_new("fixture.WithDefault").unwrap(),
                        semantic,
                    ),
                    vec![],
                    RuntimeNominalSchemaBody::Record {
                        shape: RuntimeNominalRecordShape::Record,
                        fields: vec![RuntimeNominalSchemaField::new(
                            RuntimeRecordFieldId::try_from_zero_based_ordinal(0).unwrap(),
                            Some("flag".to_owned()),
                            RuntimeTypeSchema::Bool,
                        )]
                        .into(),
                    },
                )
                .with_data_codec(RuntimeCodecUse::Record {
                    name: "WithDefault".to_owned(),
                    deny_unknown_fields: false,
                    fields: vec![RuntimeFieldCodecUse {
                        wire_name: "flag".to_owned(),
                        has_default: true,
                        default_program: Some(
                            arcweft_id::runtime_program::RuntimePureProgramId::from_checked_digest(
                                [producer; 32],
                            ),
                        ),
                        skip: false,
                        bytes_format: None,
                        value: RuntimeCodecUse::Plain,
                    }]
                    .into(),
                }),
            ],
            RuntimeSchemaLimits::engine_default(),
        )
        .unwrap()
    };
    assert_ne!(
        graph(1).try_layout_hash(semantic).unwrap(),
        graph(2).try_layout_hash(semantic).unwrap()
    );
}
