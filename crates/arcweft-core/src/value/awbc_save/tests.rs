use std::sync::Arc;

use super::*;
use crate::{
    awbc::schema::{
        AwbcProgram, AwbcRecordField, AwbcRuntimeType, AwbcRuntimeTypeShape, AwbcStringId,
        AwbcTypeId, AwbcVariantCase, AwbcVariantIdentity,
    },
    entry::{
        RuntimeNominalRecordShape, RuntimeNominalSchemaBody, RuntimeNominalSchemaCase,
        RuntimeNominalSchemaDefinition, RuntimeNominalSchemaField, RuntimeNominalSchemaGraph,
        RuntimeNominalSchemaIdentity, RuntimeNominalTypeId, RuntimeSchemaLimits, RuntimeTypeSchema,
        TypeLayoutHash,
    },
    plan::{
        RuntimeNominalRecordDomainFieldSeed, RuntimeNominalRecordDomainSeed, RuntimePlanBuilder,
        RuntimePlanTypeProjection, RuntimePlanTypeSeed, RuntimeVariantCaseSeed,
        RuntimeVariantDomainSeed,
    },
    value::RuntimeRecordFieldId,
};

#[test]
fn need_snapshot_preserves_handle_identity_and_rejects_empty_ids() {
    let value = RuntimeValue::Need(crate::task::NeedId("need.profile".to_owned()));
    let snapshot =
        AwbcRuntimeValueSnapshot::from_runtime_value(&value).expect("typed Need handle snapshots");
    let encoded = serde_json::to_vec(&snapshot).expect("Need snapshot serializes");
    let decoded: AwbcRuntimeValueSnapshot =
        serde_json::from_slice(&encoded).expect("Need snapshot decodes");
    let owner = RuntimeProgramOwner::Awbc(Arc::new(AwbcProgram::default()));
    assert_eq!(decoded.into_runtime_value_for_program(&owner), Ok(value));
    assert!(
        AwbcRuntimeValueSnapshot::from_runtime_value(&RuntimeValue::Need(crate::task::NeedId(
            String::new()
        )))
        .is_err()
    );
    assert!(
        AwbcRuntimeValueSnapshot::Need(crate::task::NeedId(String::new()))
            .into_runtime_value_for_program(&owner)
            .is_err()
    );
}

#[test]
fn awbc_snapshot_deserialize_rejects_empty_all_and_any_predicates() {
    for value in [
        serde_json::json!({ "All": { "predicates": [] } }),
        serde_json::json!({ "Any": { "predicates": [] } }),
    ] {
        assert!(serde_json::from_value::<AwbcRuntimeAgentPredicateSnapshot>(value).is_err());
    }
}

#[test]
fn awbc_snapshot_deserialize_rejects_nested_empty_predicates() {
    let value = serde_json::json!({
        "All": {
            "predicates": [{
                "Any": {
                    "predicates": [],
                },
            }],
        },
    });
    assert!(serde_json::from_value::<AwbcRuntimeAgentPredicateSnapshot>(value).is_err());
}

fn semantic(index: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([index; 32])
}

fn owners() -> [RuntimeProgramOwner; 2] {
    let limits = RuntimeSchemaLimits::engine_default();
    let record = RuntimeNominalTypeId::try_new("snapshot.Record").unwrap();
    let event = RuntimeNominalTypeId::try_new("snapshot.Event").unwrap();
    let record_identity = RuntimeNominalSchemaIdentity::new(record.clone(), semantic(1));
    let field = RuntimeRecordFieldId::try_from_zero_based_ordinal(0).unwrap();
    let graph = RuntimeNominalSchemaGraph::try_new(
        vec![
            RuntimeNominalSchemaDefinition::new(
                record_identity.clone(),
                vec![],
                RuntimeNominalSchemaBody::Record {
                    shape: RuntimeNominalRecordShape::Record,
                    fields: vec![RuntimeNominalSchemaField::new(
                        field,
                        Some("flag".to_owned()),
                        RuntimeTypeSchema::Bool,
                    )]
                    .into(),
                },
            ),
            RuntimeNominalSchemaDefinition::new(
                RuntimeNominalSchemaIdentity::new(event.clone(), semantic(3)),
                vec![],
                RuntimeNominalSchemaBody::Variant {
                    cases: vec![
                        RuntimeNominalSchemaCase::new(0, "Empty".to_owned(), None),
                        RuntimeNominalSchemaCase::new(
                            1,
                            "Set".to_owned(),
                            Some(RuntimeTypeSchema::Tuple(
                                vec![RuntimeTypeSchema::NominalRef(record_identity)].into(),
                            )),
                        ),
                    ]
                    .into(),
                },
            ),
        ],
        limits,
    )
    .unwrap();
    let record_layout = graph.try_layout_hash(semantic(1)).unwrap();
    let event_layout = graph.try_layout_hash(semantic(3)).unwrap();
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_semantic_batch(
            [
                RuntimePlanTypeSeed::new(
                    semantic(1),
                    RuntimePlanTypeProjection::Nominal {
                        nominal: record.clone(),
                        layout: record_layout,
                        arguments: Box::new([]),
                    },
                ),
                RuntimePlanTypeSeed::new(semantic(2), RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(
                    semantic(3),
                    RuntimePlanTypeProjection::Nominal {
                        nominal: event.clone(),
                        layout: event_layout,
                        arguments: Box::new([]),
                    },
                ),
                RuntimePlanTypeSeed::new(
                    semantic(4),
                    RuntimePlanTypeProjection::Tuple(Box::new([semantic(1)])),
                ),
            ],
            [],
            [RuntimeNominalRecordDomainSeed::new(
                semantic(1),
                RuntimeNominalRecordShape::Record,
                [RuntimeNominalRecordDomainFieldSeed::new(
                    field,
                    Some("flag".to_owned()),
                    semantic(2),
                )],
            )],
            [RuntimeVariantDomainSeed::new(
                semantic(3),
                event,
                event_layout,
                [
                    RuntimeVariantCaseSeed::new("Empty", None),
                    RuntimeVariantCaseSeed::new("Set", Some(semantic(4))),
                ],
            )],
            &graph,
        )
        .unwrap();
    let awbc = AwbcProgram {
        strings: ["Empty", "Set", "flag", "snapshot.Event", "snapshot.Record"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        runtime_types: vec![
            AwbcRuntimeType::new(
                semantic(1),
                AwbcRuntimeTypeShape::NominalRecord {
                    public_id: AwbcStringId(4),
                    layout: *record_layout.as_bytes(),
                    arguments: vec![],
                    shape: RuntimeNominalRecordShape::Record,
                    fields: vec![AwbcRecordField {
                        field,
                        name: Some(AwbcStringId(2)),
                        ty: AwbcTypeId(1),
                    }],
                },
            ),
            AwbcRuntimeType::new(semantic(2), AwbcRuntimeTypeShape::Bool),
            AwbcRuntimeType::new(
                semantic(3),
                AwbcRuntimeTypeShape::Variant {
                    owner: AwbcVariantIdentity::Nominal {
                        public_id: AwbcStringId(3),
                        layout: *event_layout.as_bytes(),
                    },
                    arguments: vec![],
                    cases: vec![
                        AwbcVariantCase {
                            name: AwbcStringId(0),
                            payload: None,
                        },
                        AwbcVariantCase {
                            name: AwbcStringId(1),
                            payload: Some(AwbcTypeId(3)),
                        },
                    ],
                },
            ),
            AwbcRuntimeType::new(
                semantic(4),
                AwbcRuntimeTypeShape::Tuple(vec![AwbcTypeId(0)]),
            ),
        ],
        ..AwbcProgram::default()
    };
    [
        RuntimeProgramOwner::Plan(Arc::new(builder.finish().unwrap())),
        RuntimeProgramOwner::Awbc(Arc::new(awbc)),
    ]
}

#[test]
fn nominal_snapshot_restore_requires_exact_program_admission() {
    for owner in owners() {
        let record = owner
            .types()
            .try_record_value(
                semantic(1),
                vec![RuntimeValue::Bool(true)],
                RuntimeSchemaLimits::engine_default(),
            )
            .unwrap();
        let event = owner
            .types()
            .try_variant_value(
                semantic(3),
                1,
                Some(RuntimeValue::Tuple(vec![record.clone()])),
                RuntimeSchemaLimits::engine_default(),
            )
            .unwrap();
        for value in [
            record.clone(),
            event.clone(),
            RuntimeValue::Tuple(vec![event]),
        ] {
            let saved = AwbcRuntimeValueSnapshot::from_runtime_value(&value).unwrap();
            let encoded = serde_json::to_value(saved.clone()).unwrap();
            let saved: AwbcRuntimeValueSnapshot = serde_json::from_value(encoded).unwrap();
            assert_eq!(
                saved
                    .clone()
                    .into_runtime_value_for_program(&owner)
                    .unwrap(),
                value
            );
            let absent_program = RuntimeProgramOwner::Awbc(Arc::new(AwbcProgram::default()));
            assert!(
                saved
                    .into_runtime_value_for_program(&absent_program)
                    .is_err()
            );
        }
        let AwbcRuntimeValueSnapshot::NominalRecord(saved) =
            AwbcRuntimeValueSnapshot::from_runtime_value(&record).unwrap()
        else {
            unreachable!()
        };
        for defect in 0..5 {
            let mut tampered = saved.clone();
            match defect {
                0 => tampered.semantic_identity = semantic(2),
                1 => tampered.type_id = RuntimeNominalTypeId::try_new("snapshot.Foreign").unwrap(),
                2 => tampered.layout = TypeLayoutHash::from_bytes([199; 32]),
                3 => tampered.fields.push(AwbcRuntimeValueSnapshot::Bool(false)),
                4 => {
                    tampered.fields[0] =
                        AwbcRuntimeValueSnapshot::String("wrong field type".to_owned())
                }
                _ => unreachable!(),
            }
            assert!(
                AwbcRuntimeValueSnapshot::NominalRecord(tampered)
                    .into_runtime_value_for_program(&owner)
                    .is_err(),
                "defect {defect}"
            );
        }
        let foreign = RuntimeValue::NominalRecord(RuntimeNominalRecordValue::new(
            saved.type_id.clone(),
            semantic(3),
            saved.layout,
            vec![RuntimeValue::Bool(true)],
        ));
        assert_ne!(record, foreign);
        assert_ne!(
            record.try_canonical_bytes(1024).unwrap(),
            foreign.try_canonical_bytes(1024).unwrap()
        );
        assert!(
            owner
                .types()
                .validate_live_value(semantic(1), &foreign, RuntimeSchemaLimits::engine_default())
                .is_err()
        );
        let wrong_case = AwbcRuntimeValueSnapshot::Variant {
            owner: super::super::RuntimeVariantIdentity::Nominal {
                nominal: RuntimeNominalTypeId::try_new("snapshot.Event").unwrap(),
                semantic_identity: semantic(3),
                layout: match owner
                    .types()
                    .try_variant_value(semantic(3), 0, None, RuntimeSchemaLimits::engine_default())
                    .unwrap()
                {
                    RuntimeValue::Variant {
                        owner: super::super::RuntimeVariantIdentity::Nominal { layout, .. },
                        ..
                    } => layout,
                    _ => unreachable!(),
                },
            },
            ordinal: 0,
            name: "Set".to_owned(),
            payload: None,
        };
        assert!(wrong_case.into_runtime_value_for_program(&owner).is_err());
    }
}
