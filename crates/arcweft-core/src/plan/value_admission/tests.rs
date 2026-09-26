use super::*;
use crate::entry::{
    RuntimeNominalRecordShape, RuntimeNominalSchemaBody, RuntimeNominalSchemaCase,
    RuntimeNominalSchemaDefinition, RuntimeNominalSchemaField, RuntimeNominalSchemaGraph,
    RuntimeNominalSchemaIdentity, RuntimeNominalTypeId, RuntimeTypeSchema as Schema,
    TypeLayoutHash,
};
use crate::pattern::{RuntimeSemanticTypeId, RuntimeVariantIdentity};
use crate::plan::{
    RuntimeNominalRecordDomainFieldSeed, RuntimeNominalRecordDomainSeed, RuntimePlanBuildError,
    RuntimePlanBuilder, RuntimePlanSequenceKind, RuntimePlanTypeSeed, RuntimePlanTypeTableError,
    RuntimeVariantCaseSeed, RuntimeVariantDomainSeed,
};
use crate::value::{
    RuntimeColor, RuntimeNominalRecordValue, RuntimeRecordFieldId, RuntimeSeq,
    RuntimeSignedIntWidth,
};

fn semantic(tag: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([tag; 32])
}
fn nominal() -> RuntimeNominalTypeId {
    RuntimeNominalTypeId::try_new("fixture.ValueGraph").unwrap()
}
fn schema(body: RuntimeNominalSchemaBody) -> RuntimeNominalSchemaGraph {
    RuntimeNominalSchemaGraph::try_new(
        vec![RuntimeNominalSchemaDefinition::new(
            RuntimeNominalSchemaIdentity::new(nominal(), semantic(1)),
            vec![],
            body,
        )],
        RuntimeSchemaLimits::engine_default(),
    )
    .unwrap()
}
fn seed(tag: u8, projection: Type<RuntimeSemanticTypeId>) -> RuntimePlanTypeSeed {
    RuntimePlanTypeSeed::new(semantic(tag), projection)
}
fn nominal_type(layout: TypeLayoutHash) -> Type<RuntimeSemanticTypeId> {
    Type::Nominal {
        nominal: nominal(),
        layout,
        arguments: Box::new([]),
    }
}
fn id(plan: &RuntimePlan, tag: u8) -> RuntimePlanTypeId {
    plan.type_table().id_for_semantic(semantic(tag)).unwrap()
}
fn plan(types: impl IntoIterator<Item = RuntimePlanTypeSeed>) -> RuntimePlan {
    let mut builder = RuntimePlanBuilder::new();
    builder.admit_type_batch(types, []).unwrap();
    builder.finish().unwrap()
}
fn admitted(
    plan: &RuntimePlan,
    tag: u8,
    value: &RuntimeValue,
) -> Result<RuntimeValueDigest, RuntimePlanValueAdmissionError> {
    plan.accepts_value(id(plan, tag), value, RuntimeSchemaLimits::engine_default())
}

#[test]
fn runtime_color_is_admitted_by_the_dedicated_plan_color_type() {
    let plan = plan([seed(1, Type::Color), seed(2, Type::String)]);
    let color = RuntimeValue::Color(RuntimeColor::new(12, 34, 56, 78));

    assert!(admitted(&plan, 1, &color).is_ok());
    assert!(admitted(&plan, 2, &color).is_err());
}

#[test]
fn program_value_admission_distinguishes_live_snapshot_and_canonical_opaque_rules() {
    use crate::{
        awbc::schema::{
            AwbcProgram, AwbcRuntimeType, AwbcRuntimeTypeShape as AwbcShape, AwbcStringId,
            AwbcTypeId,
        },
        entry::RuntimeSchemaLimits,
        pattern::{
            RuntimeOpaqueTypeAdmission, RuntimeOpaqueTypeOwner, RuntimeOpaqueTypeProducerId,
        },
        program_types::RuntimeProgramTypes,
        value::{RuntimeHandleKind, RuntimeOpaquePersistence, RuntimeOpaqueValueClass},
    };

    let limits = RuntimeSchemaLimits::engine_default();
    for value_class in [
        RuntimeOpaqueValueClass::Plain,
        RuntimeOpaqueValueClass::AffineHandle(RuntimeHandleKind::Cue),
    ] {
        let producer = RuntimeOpaqueTypeProducerId::try_new("fixture.result").unwrap();
        let owner = RuntimeOpaqueTypeOwner::exact_with(
            producer.clone(),
            semantic(1),
            value_class,
            RuntimeOpaquePersistence::SnapshotOnly,
        );
        let plan = plan([
            seed(
                1,
                Type::Opaque {
                    producer: producer.clone(),
                    admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
                    value_class,
                    persistence: RuntimeOpaquePersistence::SnapshotOnly,
                    arguments: Box::new([]),
                },
            ),
            seed(2, Type::Tuple(Box::new([semantic(1)]))),
        ]);
        let program = AwbcProgram {
            strings: vec![producer.as_str().to_owned()],
            runtime_types: vec![
                AwbcRuntimeType::new(semantic(0), AwbcShape::Unit),
                AwbcRuntimeType::new(
                    semantic(1),
                    AwbcShape::Opaque {
                        producer: AwbcStringId(0),
                        admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
                        value_class,
                        persistence: RuntimeOpaquePersistence::SnapshotOnly,
                        arguments: vec![],
                    },
                ),
                AwbcRuntimeType::new(semantic(2), AwbcShape::Tuple(vec![AwbcTypeId(1)])),
            ],
            ..AwbcProgram::default()
        };
        let value = RuntimeValue::Tuple(vec![owner.try_wrap(RuntimeValue::Bool(true)).unwrap()]);
        let foreign = RuntimeOpaqueTypeOwner::exact_with(
            producer,
            semantic(3),
            value_class,
            RuntimeOpaquePersistence::SnapshotOnly,
        );
        let invalid =
            RuntimeValue::Tuple(vec![foreign.try_wrap(RuntimeValue::Bool(true)).unwrap()]);
        for types in [
            RuntimeProgramTypes::Plan(&plan),
            RuntimeProgramTypes::Awbc(&program),
        ] {
            types
                .validate_live_value(semantic(2), &value, limits)
                .unwrap();
            assert!(
                types
                    .validate_live_value(semantic(2), &invalid, limits)
                    .is_err()
            );
            assert!(types.accepts_value(semantic(2), &value, limits).is_err());
            if value_class == RuntimeOpaqueValueClass::Plain {
                types
                    .validate_snapshot_value(semantic(2), &value, limits)
                    .unwrap();
            } else {
                assert!(
                    types
                        .validate_snapshot_value(semantic(2), &value, limits)
                        .is_err()
                );
            }
        }
    }
}

#[test]
fn recursive_records_follow_values_and_validate_every_descendant() {
    let schema = schema(RuntimeNominalSchemaBody::Record {
        shape: RuntimeNominalRecordShape::Tuple,
        fields: [
            Schema::Bool,
            Schema::option(Schema::NominalRef(RuntimeNominalSchemaIdentity::new(
                nominal(),
                semantic(1),
            ))),
        ]
        .into_iter()
        .enumerate()
        .map(|(ordinal, schema)| {
            RuntimeNominalSchemaField::new(
                RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal).unwrap(),
                None,
                schema,
            )
        })
        .collect(),
    });
    let layout = schema.try_layout_hash(semantic(1)).unwrap();
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_semantic_batch(
            [
                seed(1, nominal_type(layout)),
                seed(
                    2,
                    Type::Option {
                        item: semantic(1),
                        some_payload: semantic(3),
                    },
                ),
                seed(3, Type::Tuple(Box::new([semantic(1)]))),
                seed(4, Type::Bool),
            ],
            [],
            [RuntimeNominalRecordDomainSeed::new(
                semantic(1),
                RuntimeNominalRecordShape::Tuple,
                [4, 2].into_iter().enumerate().map(|(ordinal, tag)| {
                    RuntimeNominalRecordDomainFieldSeed::new(
                        RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal).unwrap(),
                        None,
                        semantic(tag),
                    )
                }),
            )],
            [],
            &schema,
        )
        .unwrap();
    let plan = builder.finish().unwrap();
    let node = |flag, next| {
        RuntimeValue::NominalRecord(RuntimeNominalRecordValue::new(
            nominal(),
            semantic(1),
            layout,
            vec![flag, next],
        ))
    };
    let mut value = node(RuntimeValue::Bool(true), RuntimeValue::option_none());
    for _ in 0..32 {
        value = node(RuntimeValue::Bool(false), RuntimeValue::option_some(value));
    }
    assert_eq!(
        admitted(&plan, 1, &value).unwrap(),
        value.try_digest(100_000).unwrap()
    );
    let bad = node(
        RuntimeValue::Bool(true),
        RuntimeValue::option_some(node(
            RuntimeValue::String("wrong".to_owned()),
            RuntimeValue::option_none(),
        )),
    );
    assert!(
        matches!(admitted(&plan, 1, &bad), Err(RuntimePlanValueAdmissionError::Value {
        source: RuntimeSchemaError::Type { path, .. }, ..
    }) if path == "$[1].Some[0][0]")
    );
    assert!(matches!(
        plan.accepts_value(
            id(&plan, 1),
            &value,
            RuntimeSchemaLimits {
                max_depth: 16,
                ..RuntimeSchemaLimits::engine_default()
            }
        ),
        Err(RuntimePlanValueAdmissionError::Value {
            source: RuntimeSchemaError::BudgetExceeded { budget: "depth" },
            ..
        })
    ));
}

#[test]
fn recursive_variants_require_exact_owner_case_and_payload() {
    let schema = schema(RuntimeNominalSchemaBody::Variant {
        cases: vec![
            RuntimeNominalSchemaCase::new(0, "Nil".to_owned(), None),
            RuntimeNominalSchemaCase::new(
                1,
                "Cons".to_owned(),
                Some(Schema::Tuple(Box::new([
                    Schema::Bool,
                    Schema::NominalRef(RuntimeNominalSchemaIdentity::new(nominal(), semantic(1))),
                ]))),
            ),
        ]
        .into_boxed_slice(),
    });
    let layout = schema.try_layout_hash(semantic(1)).unwrap();
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_semantic_batch(
            [
                seed(1, nominal_type(layout)),
                seed(2, Type::Tuple(Box::new([semantic(3), semantic(1)]))),
                seed(3, Type::Bool),
            ],
            [],
            [],
            [RuntimeVariantDomainSeed::new(
                semantic(1),
                nominal(),
                layout,
                [
                    RuntimeVariantCaseSeed::new("Nil", None),
                    RuntimeVariantCaseSeed::new("Cons", Some(semantic(2))),
                ],
            )],
            &schema,
        )
        .unwrap();
    let plan = builder.finish().unwrap();
    assert!(plan.checked_type(id(&plan, 1)).is_err());
    let variant = |ordinal, name: &str, payload: Option<RuntimeValue>| RuntimeValue::Variant {
        owner: RuntimeVariantIdentity::Nominal {
            nominal: nominal(),
            semantic_identity: semantic(1),
            layout,
        },
        ordinal,
        name: name.to_owned(),
        payload: payload.map(Box::new),
    };
    let nil = variant(0, "Nil", None);
    let value = variant(
        1,
        "Cons",
        Some(RuntimeValue::Tuple(vec![
            RuntimeValue::Bool(true),
            nil.clone(),
        ])),
    );
    assert_eq!(
        admitted(&plan, 1, &value).unwrap(),
        value.try_digest(100_000).unwrap()
    );
    for wrong in [
        variant(1, "Cons", None),
        variant(0, "Cons", None),
        variant(9, "Nil", None),
        variant(0, "Nil", Some(RuntimeValue::Tuple(vec![]))),
        variant(
            1,
            "Cons",
            Some(RuntimeValue::Tuple(vec![RuntimeValue::i8(1), nil])),
        ),
    ] {
        assert!(admitted(&plan, 1, &wrong).is_err());
    }
    for owner in [
        RuntimeVariantIdentity::Nominal {
            nominal: nominal(),
            semantic_identity: semantic(8),
            layout,
        },
        RuntimeVariantIdentity::Nominal {
            nominal: nominal(),
            semantic_identity: semantic(1),
            layout: TypeLayoutHash::from_bytes([20; 32]),
        },
    ] {
        assert!(
            admitted(
                &plan,
                1,
                &RuntimeValue::Variant {
                    owner,
                    ordinal: 0,
                    name: "Nil".to_owned(),
                    payload: None
                }
            )
            .is_err()
        );
    }
}

#[test]
fn choice_requires_unique_success_and_never_refunds_work() {
    let plan = plan([
        seed(1, Type::Choice(Box::new([semantic(2), semantic(3)]))),
        seed(2, Type::Bool),
        seed(3, Type::AgentValue),
        seed(4, Type::Choice(Box::new([semantic(2), semantic(5)]))),
        seed(5, Type::String),
    ]);
    assert!(matches!(
        admitted(&plan, 1, &RuntimeValue::Bool(true)),
        Err(RuntimePlanValueAdmissionError::Value {
            source: RuntimeSchemaError::ChoiceAmbiguous {
                first: 0,
                second: 1,
                ..
            },
            ..
        })
    ));
    assert!(
        matches!(admitted(&plan, 4, &RuntimeValue::i8(1)), Err(RuntimePlanValueAdmissionError::Value {
        source: RuntimeSchemaError::ChoiceNoMatch { branches, .. }, ..
    }) if branches.len() == 2)
    );
    assert!(admitted(&plan, 4, &RuntimeValue::Bool(true)).is_ok());
    assert!(matches!(
        plan.accepts_value(
            id(&plan, 4),
            &RuntimeValue::Bool(true),
            RuntimeSchemaLimits {
                max_validation_work: 3,
                ..RuntimeSchemaLimits::engine_default()
            }
        ),
        Err(RuntimePlanValueAdmissionError::Value {
            source: RuntimeSchemaError::ValidationWork { consumed: 4, .. },
            ..
        })
    ));
}

#[test]
fn record_names_are_part_of_admission_and_type_identity_conflicts() {
    let types = |name| {
        [
            seed(
                1,
                Type::Record(Box::new([RuntimePlanRecordField::new(name, semantic(2))])),
            ),
            seed(2, Type::Bool),
        ]
    };
    let mut builder = RuntimePlanBuilder::new();
    builder.admit_type_batch(types("declared"), []).unwrap();
    assert!(matches!(
        builder.admit_type_batch(types("renamed"), [],),
        Err(RuntimePlanBuildError::TypeGraph(
            RuntimePlanTypeTableError::ConflictingProjection { .. }
        ))
    ));
    let plan = builder.finish().unwrap();
    assert!(
        admitted(
            &plan,
            1,
            &RuntimeValue::try_record(vec![("declared".to_owned(), RuntimeValue::Bool(true))])
                .unwrap()
        )
        .is_ok()
    );
    assert!(matches!(
        admitted(
            &plan,
            1,
            &RuntimeValue::try_record(vec![("renamed".to_owned(), RuntimeValue::Bool(true))])
                .unwrap()
        ),
        Err(RuntimePlanValueAdmissionError::Value {
            source: RuntimeSchemaError::RecordField { ordinal: 0, .. },
            ..
        })
    ));
}

#[test]
fn dense_sequences_and_arrays_share_width_and_value_budgets() {
    let plan = plan([
        seed(
            1,
            Type::Array {
                item: semantic(2),
                length: 3.into(),
            },
        ),
        seed(2, Type::Signed(RuntimeSignedIntWidth::I16)),
        seed(
            3,
            Type::Sequence {
                kind: RuntimePlanSequenceKind::Vec,
                item: semantic(2),
            },
        ),
    ]);
    let dense = RuntimeValue::Seq(RuntimeSeq::dense_i16(vec![1, 2, 3]));
    let ordinary = RuntimeValue::Seq(RuntimeSeq::values(vec![
        RuntimeValue::i16(1),
        RuntimeValue::i16(2),
        RuntimeValue::i16(3),
    ]));
    assert_eq!(
        admitted(&plan, 1, &dense).unwrap(),
        admitted(&plan, 3, &ordinary).unwrap()
    );
    assert!(
        admitted(
            &plan,
            1,
            &RuntimeValue::Seq(RuntimeSeq::dense_i8(vec![1, 2, 3]))
        )
        .is_err()
    );
    assert!(
        admitted(
            &plan,
            1,
            &RuntimeValue::Seq(RuntimeSeq::dense_i16(vec![1, 2]))
        )
        .is_err()
    );
    for limits in [
        RuntimeSchemaLimits {
            max_nodes: 3,
            ..RuntimeSchemaLimits::engine_default()
        },
        RuntimeSchemaLimits {
            max_sequence_items: 2,
            ..RuntimeSchemaLimits::engine_default()
        },
        RuntimeSchemaLimits {
            max_encoded_bytes: 2,
            ..RuntimeSchemaLimits::engine_default()
        },
    ] {
        assert!(plan.accepts_value(id(&plan, 1), &dense, limits).is_err());
    }
}

#[test]
fn exact_opaque_owner_and_internal_value_budget_are_enforced() {
    use crate::pattern::RuntimeOpaqueTypeProducerId;
    use crate::value::{RuntimeOpaquePersistence, RuntimeOpaqueValueClass};
    let producer = RuntimeOpaqueTypeProducerId::try_new("fixture.opaque").unwrap();
    let owner = RuntimeOpaqueTypeOwner::with_admission(
        producer.clone(),
        semantic(1),
        RuntimeOpaqueTypeAdmission::ExactIdentity,
        RuntimeOpaqueValueClass::Plain,
        RuntimeOpaquePersistence::ConstantAndSnapshot,
    );
    let plan = plan([seed(
        1,
        Type::Opaque {
            producer,
            admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
            value_class: RuntimeOpaqueValueClass::Plain,
            persistence: RuntimeOpaquePersistence::ConstantAndSnapshot,
            arguments: Box::new([]),
        },
    )]);
    let value = owner
        .try_wrap(RuntimeValue::Tuple(vec![RuntimeValue::Bool(true)]))
        .unwrap();
    assert!(admitted(&plan, 1, &value).is_ok());
    let other = RuntimeOpaqueTypeOwner::with_admission(
        owner.producer().clone(),
        semantic(2),
        owner.admission(),
        owner.value_class(),
        owner.persistence(),
    );
    assert!(admitted(&plan, 1, &other.try_wrap(RuntimeValue::Unit).unwrap()).is_err());
    assert!(matches!(
        plan.accepts_value(
            id(&plan, 1),
            &value,
            RuntimeSchemaLimits {
                max_nodes: 2,
                ..RuntimeSchemaLimits::engine_default()
            }
        ),
        Err(RuntimePlanValueAdmissionError::Value {
            source: RuntimeSchemaError::BudgetExceeded { budget: "nodes" },
            ..
        })
    ));
}

#[test]
fn builtin_payloads_are_correlated_before_type_rows_are_published() {
    use crate::pattern::RuntimeBuiltinVariantIdentity;
    for projection in [
        Type::Option {
            item: semantic(2),
            some_payload: semantic(2),
        },
        Type::Option {
            item: semantic(2),
            some_payload: semantic(3),
        },
        Type::Result {
            value: semantic(2),
            error: semantic(4),
            value_payload: semantic(3),
            error_payload: semantic(3),
        },
        Type::BuiltinVariant {
            owner: RuntimeBuiltinVariantIdentity::Option,
            cases: Box::new([Some(semantic(2)), None]),
        },
    ] {
        let mut builder = RuntimePlanBuilder::new();
        assert!(matches!(
            builder.admit_type_batch(
                [
                    seed(1, projection),
                    seed(2, Type::Bool),
                    seed(3, Type::Tuple(Box::new([semantic(4)]))),
                    seed(4, Type::String),
                ],
                [],
            ),
            Err(RuntimePlanBuildError::TypeGraph(
                RuntimePlanTypeTableError::InvalidBuiltinVariantSchema { .. }
            ))
        ));
        let plan = builder.finish().unwrap();
        assert!(plan.type_table().declarations().next().is_none());
    }
    let plan = plan([
        seed(
            1,
            Type::Option {
                item: semantic(2),
                some_payload: semantic(3),
            },
        ),
        seed(2, Type::Bool),
        seed(3, Type::Tuple(Box::new([semantic(2)]))),
    ]);
    assert!(
        admitted(
            &plan,
            1,
            &RuntimeValue::option_some(RuntimeValue::Bool(true))
        )
        .is_ok()
    );
    assert!(
        admitted(
            &plan,
            1,
            &RuntimeValue::Variant {
                owner: RuntimeVariantIdentity::Builtin(RuntimeBuiltinVariantIdentity::Option),
                ordinal: 0,
                name: "Some".to_owned(),
                payload: Some(Box::new(RuntimeValue::Bool(true))),
            }
        )
        .is_err()
    );
}
