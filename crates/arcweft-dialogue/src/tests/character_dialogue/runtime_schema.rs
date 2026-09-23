use super::*;
use crate::{
    CharacterDialogueRuntimeCustomFieldDescriptor, CharacterDialogueRuntimeRole as Role,
    CharacterDialogueRuntimeRoleType, CharacterDialogueRuntimeRoleTypes,
    CharacterDialogueRuntimeSchema, CharacterDialogueType,
};
use arcweft_core::{
    awbc::schema::{
        AwbcProgram, AwbcRecordField, AwbcRuntimeType, AwbcRuntimeTypeShape as AwbcType,
        AwbcStringId, AwbcTypeId,
    },
    entry::{
        RuntimeNominalRecordShape, RuntimeNominalSchemaBody, RuntimeNominalSchemaDefinition,
        RuntimeNominalSchemaField, RuntimeNominalSchemaGraph, RuntimeNominalSchemaIdentity,
        RuntimeNominalTypeId, RuntimeSchemaLimits, RuntimeTypeSchema, TypeLayoutHash,
    },
    pattern::{RuntimeOpaqueTypeAdmission, RuntimeOpaqueTypeOwner, RuntimeSemanticTypeId},
    plan::{
        RuntimeNominalRecordDomainFieldSeed, RuntimeNominalRecordDomainSeed, RuntimePlan,
        RuntimePlanBuilder, RuntimePlanTypeProjection as Type, RuntimePlanTypeSeed,
    },
    program_types::RuntimeProgramTypes,
    value::{
        RuntimeEntityReference, RuntimeOpaquePersistence, RuntimeOpaqueValue,
        RuntimeOpaqueValueClass, RuntimeRecordFieldId,
    },
};
use arcweft_id::{DeclarationIdentityFamily, PublicId};
use std::collections::BTreeSet;

fn semantic(tag: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([tag; 32])
}
fn role_semantic(role: Role) -> RuntimeSemanticTypeId {
    semantic(20 + role.canonical_tag())
}

fn test_payload_graph() -> (
    RuntimeNominalTypeId,
    RuntimeSemanticTypeId,
    RuntimeNominalSchemaGraph,
) {
    let field = RuntimeRecordFieldId::try_from_zero_based_ordinal(0).unwrap();
    let nominal = RuntimeNominalTypeId::try_new("dialogue.TestPayload").unwrap();
    let identity = semantic(54);
    let graph = RuntimeNominalSchemaGraph::try_new(
        vec![RuntimeNominalSchemaDefinition::new(
            RuntimeNominalSchemaIdentity::new(nominal.clone(), identity),
            vec![],
            RuntimeNominalSchemaBody::Record {
                shape: RuntimeNominalRecordShape::Record,
                fields: vec![RuntimeNominalSchemaField::new(
                    field,
                    Some("flag".into()),
                    RuntimeTypeSchema::Bool,
                )]
                .into(),
            },
        )],
        RuntimeSchemaLimits::engine_default(),
    )
    .unwrap();
    (nominal, identity, graph)
}

pub(super) fn test_payload_descriptor()
-> (RuntimeNominalTypeId, RuntimeSemanticTypeId, TypeLayoutHash) {
    let (nominal, identity, graph) = test_payload_graph();
    let layout = graph.try_layout_hash(identity).unwrap();
    (nominal, identity, layout)
}

pub(super) fn role_value(role: Role, payload: RuntimeValue) -> CharacterDialogueTypedValue {
    let owner = RuntimeOpaqueTypeOwner::exact(
        CharacterDialogueRuntimeSchema::opaque_type_producer(),
        role_semantic(role),
    );
    CharacterDialogueTypedValue::try_new(owner.try_wrap(payload).unwrap()).unwrap()
}

struct Types {
    plan: RuntimePlan,
    awbc: AwbcProgram,
    roles: CharacterDialogueRuntimeRoleTypes,
    defaults: BTreeMap<CharacterId, RuntimeValueDigest>,
    nominal: RuntimeNominalTypeId,
    nominal_identity: RuntimeSemanticTypeId,
    layout: TypeLayoutHash,
}

impl Types {
    fn new() -> Self {
        let field = RuntimeRecordFieldId::try_from_zero_based_ordinal(0).unwrap();
        let (nominal, nominal_identity, graph) = test_payload_graph();
        let layout = graph.try_layout_hash(nominal_identity).unwrap();
        let producer = CharacterDialogueRuntimeSchema::opaque_type_producer();
        let mut seeds = Role::AUTHORED_BASE
            .into_iter()
            .map(|role| {
                RuntimePlanTypeSeed::new(
                    role_semantic(role),
                    Type::Opaque {
                        producer: producer.clone(),
                        admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
                        value_class: RuntimeOpaqueValueClass::Plain,
                        persistence: RuntimeOpaquePersistence::ConstantAndSnapshot,
                        arguments: Box::new([]),
                    },
                )
            })
            .collect::<Vec<_>>();
        seeds.extend([
            RuntimePlanTypeSeed::new(
                semantic(50),
                Type::Choice(vec![semantic(51), role_semantic(Role::RichText)].into()),
            ),
            RuntimePlanTypeSeed::new(semantic(51), Type::EntityReference),
            RuntimePlanTypeSeed::new(semantic(52), Type::Tuple(Box::new([]))),
            RuntimePlanTypeSeed::new(semantic(53), Type::String),
            RuntimePlanTypeSeed::new(
                semantic(54),
                Type::Nominal {
                    nominal: nominal.clone(),
                    layout,
                    arguments: Box::new([]),
                },
            ),
            RuntimePlanTypeSeed::new(semantic(55), Type::Bool),
        ]);
        let mut builder = RuntimePlanBuilder::new();
        builder
            .admit_semantic_batch(
                seeds,
                [],
                [RuntimeNominalRecordDomainSeed::new(
                    semantic(54),
                    RuntimeNominalRecordShape::Record,
                    [RuntimeNominalRecordDomainFieldSeed::new(
                        field,
                        Some("flag".into()),
                        semantic(55),
                    )],
                )],
                [],
                &graph,
            )
            .unwrap();
        let plan = builder.finish().unwrap();
        let mut runtime_types = Role::AUTHORED_BASE
            .into_iter()
            .map(|role| {
                AwbcRuntimeType::new(
                    role_semantic(role),
                    AwbcType::Opaque {
                        producer: AwbcStringId(0),
                        admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
                        value_class: RuntimeOpaqueValueClass::Plain,
                        persistence: RuntimeOpaquePersistence::ConstantAndSnapshot,
                        arguments: vec![],
                    },
                )
            })
            .collect::<Vec<_>>();
        runtime_types.extend([
            AwbcRuntimeType::new(
                semantic(50),
                AwbcType::Choice(vec![AwbcTypeId(7), AwbcTypeId(5)]),
            ),
            AwbcRuntimeType::new(semantic(51), AwbcType::EntityRef),
            AwbcRuntimeType::new(semantic(52), AwbcType::Tuple(vec![])),
            AwbcRuntimeType::new(semantic(53), AwbcType::String),
            AwbcRuntimeType::new(
                semantic(54),
                AwbcType::NominalRecord {
                    public_id: AwbcStringId(1),
                    layout: *layout.as_bytes(),
                    arguments: vec![],
                    shape: RuntimeNominalRecordShape::Record,
                    fields: vec![AwbcRecordField {
                        field,
                        name: Some(AwbcStringId(2)),
                        ty: AwbcTypeId(11),
                    }],
                },
            ),
            AwbcRuntimeType::new(semantic(55), AwbcType::Bool),
        ]);
        let awbc = AwbcProgram {
            strings: vec![
                producer.as_str().to_owned(),
                nominal.as_str().to_owned(),
                "flag".into(),
            ],
            runtime_types,
            ..AwbcProgram::default()
        };
        let roles = CharacterDialogueRuntimeRoleTypes::new(
            Role::AUTHORED_BASE.map(|role| {
                CharacterDialogueRuntimeRoleType::new(
                    role_semantic(role),
                    if role == Role::Hook {
                        semantic(54)
                    } else {
                        semantic(52)
                    },
                )
            }),
            semantic(50),
        );
        Self {
            plan,
            awbc,
            roles,
            defaults: BTreeMap::from([(
                sample_manifest().character().clone(),
                RuntimeValueDigest::from_bytes([2; 32]),
            )]),
            nominal,
            nominal_identity,
            layout,
        }
    }
    fn programs(&self) -> [RuntimeProgramTypes<'_>; 2] {
        [
            RuntimeProgramTypes::Plan(&self.plan),
            RuntimeProgramTypes::Awbc(&self.awbc),
        ]
    }
    fn schema<'a>(
        &'a self,
        characters: &'a CharacterCatalog,
        views: &'a ViewRegistry,
        custom: &'a CharacterDialogueRuntimeCustomFieldCatalog,
        program: RuntimeProgramTypes<'a>,
    ) -> CharacterDialogueRuntimeSchema<'a> {
        CharacterDialogueRuntimeSchema::try_new(
            characters,
            views,
            custom,
            &self.defaults,
            &self.roles,
            program,
        )
        .unwrap()
    }
    fn nominal(&self, value: RuntimeValue) -> RuntimeValue {
        RuntimeValue::NominalRecord(RuntimeNominalRecordValue::new(
            self.nominal.clone(),
            self.nominal_identity,
            self.layout,
            vec![value],
        ))
    }
}

fn rewrap(dialogue: &CharacterDialogue, payload: RuntimeValue) -> RuntimeOpaqueValue {
    let RuntimeValue::Opaque(value) = CharacterDialogueType::exact(dialogue.character().clone())
        .runtime_opaque_owner()
        .try_wrap(payload)
        .unwrap()
    else {
        unreachable!()
    };
    value
}

#[test]
fn native_and_awbc_round_trip_the_tuple_and_every_policy_case() {
    let (base, characters, views, custom) = fixture();
    let types = Types::new();
    let entity_style = CharacterDialogueStyleValue::try_new(
        CharacterDialogueTypedValue::try_new(RuntimeValue::EntityRef(
            RuntimeEntityReference::Project {
                family: DeclarationIdentityFamily::Style,
                public_id: PublicId::try_new("style.dialogue").unwrap(),
            },
        ))
        .unwrap(),
    )
    .unwrap();
    let policies = [
        InlineFailurePolicy::FailLine,
        InlineFailurePolicy::Discard,
        InlineFailurePolicy::Fallback {
            fallback: InlineFallback::Text {
                text: "fallback".into(),
                style: FallbackStylePolicy::Apply {
                    styles: vec![style(5), entity_style],
                },
            },
        },
        InlineFailurePolicy::Fallback {
            fallback: InlineFallback::ExprSource {
                style: FallbackStylePolicy::Plain,
            },
        },
        InlineFailurePolicy::Fallback {
            fallback: InlineFallback::CallSource {
                style: FallbackStylePolicy::InheritSurrounding,
            },
        },
        InlineFailurePolicy::Fallback {
            fallback: InlineFallback::ValuePlain,
        },
    ];
    for policy in policies {
        for voice in [
            None,
            Some(CharacterDialogueVoice::Auto),
            Some(CharacterDialogueVoice::Id(
                CharacterDialogueVoiceId::try_new("voice.alice").unwrap(),
            )),
        ] {
            let dialogue = base
                .patched(
                    &CharacterDialoguePatch::default()
                        .with_inline_failure(PatchField::Set(policy.clone()))
                        .with_voice(voice.map_or(PatchField::Clear, PatchField::Set))
                        .with_stage(PatchField::Set(
                            crate::CharacterDialogueStageValue::try_new(role_value(
                                Role::Stage,
                                RuntimeValue::Tuple(vec![]),
                            ))
                            .unwrap(),
                        ))
                        .with_hooks(PatchField::Set(vec![
                            CharacterDialogueHookValue::try_new(role_value(
                                Role::Hook,
                                types.nominal(RuntimeValue::Bool(true)),
                            ))
                            .unwrap(),
                        ])),
                )
                .unwrap();
            let mut digests = Vec::new();
            for program in types.programs() {
                let schema = types.schema(&characters, &views, &custom, program);
                let encoded = schema.encode(&dialogue).unwrap();
                let RuntimeValue::Tuple(fields) = encoded.opaque().payload() else {
                    panic!("tuple")
                };
                assert_eq!(fields.len(), 18);
                assert!(matches!(&fields[16], RuntimeValue::Variant { .. }));
                assert_eq!(
                    schema
                        .try_decode_opaque(encoded.opaque())
                        .unwrap()
                        .dialogue(),
                    &dialogue
                );
                digests.push(schema.digest(&dialogue).unwrap());
            }
            assert_eq!(digests[0], digests[1]);
        }
    }
}

#[test]
fn rejects_outer_owner_removed_carrier_and_tuple_arity_before_publication() {
    let (dialogue, characters, views, custom) = fixture();
    let types = Types::new();
    for program in types.programs() {
        let schema = types.schema(&characters, &views, &custom, program);
        let encoded = schema.encode(&dialogue).unwrap();
        let RuntimeValue::Tuple(fields) = encoded.opaque().payload() else {
            unreachable!()
        };
        for payload in [
            RuntimeValue::Tuple(fields[..17].to_vec()),
            types.nominal(RuntimeValue::Bool(true)),
            RuntimeValue::Tuple(vec![]),
        ] {
            assert!(matches!(
                schema.try_decode_opaque(&rewrap(&dialogue, payload)),
                Err(CharacterDialogueValueError::OpaquePayload)
            ));
        }
        let wrong = CharacterDialogueType::exact(CharacterId::try_new("character.other").unwrap())
            .runtime_opaque_owner()
            .try_wrap(encoded.opaque().payload().clone())
            .unwrap();
        let RuntimeValue::Opaque(wrong) = wrong else {
            unreachable!()
        };
        assert!(matches!(
            schema.try_decode_opaque(&wrong),
            Err(CharacterDialogueValueError::OpaqueSemanticIdentity { .. })
        ));
        let owner = RuntimeOpaqueTypeOwner::exact_with(
            CharacterDialogueRuntimeSchema::opaque_type_producer(),
            encoded.opaque().semantic_identity(),
            RuntimeOpaqueValueClass::Plain,
            RuntimeOpaquePersistence::SnapshotOnly,
        );
        let RuntimeValue::Opaque(wrong) =
            owner.try_wrap(encoded.opaque().payload().clone()).unwrap()
        else {
            unreachable!()
        };
        assert!(matches!(
            schema.try_decode_opaque(&wrong),
            Err(CharacterDialogueValueError::OpaqueContract)
        ));
    }
}

#[test]
fn rejects_stale_contracts_and_tampered_policy_headers() {
    let (dialogue, characters, views, custom) = fixture();
    let types = Types::new();
    for program in types.programs() {
        let schema = types.schema(&characters, &views, &custom, program);
        let encoded = schema.encode(&dialogue).unwrap();
        let RuntimeValue::Tuple(fields) = encoded.opaque().payload() else {
            unreachable!()
        };
        for index in 1..=4 {
            let mut changed = fields.clone();
            changed[index] = arcweft_core::value::runtime_sequence_dense_bytes(vec![99; 32]);
            assert!(
                schema
                    .try_decode_opaque(&rewrap(&dialogue, RuntimeValue::Tuple(changed)))
                    .is_err()
            );
        }
        for edit in 0..4 {
            let mut changed = fields.clone();
            let RuntimeValue::Variant {
                owner,
                ordinal,
                name,
                payload,
            } = &mut changed[16]
            else {
                unreachable!()
            };
            match edit {
                0 => {
                    let RuntimeVariantIdentity::Nominal { layout, .. } = owner else {
                        unreachable!()
                    };
                    *layout = TypeLayoutHash::from_bytes([0; 32]);
                }
                1 => *ordinal = 99,
                2 => *name = "Other".into(),
                _ => *payload = Some(Box::new(RuntimeValue::Unit)),
            }
            assert!(
                schema
                    .try_decode_opaque(&rewrap(&dialogue, RuntimeValue::Tuple(changed)))
                    .is_err()
            );
        }
    }
}

#[test]
fn role_payload_admission_rejects_nested_nominal_type_and_header_mismatches() {
    let (base, characters, views, custom) = fixture();
    let types = Types::new();
    for program in types.programs() {
        let schema = types.schema(&characters, &views, &custom, program);
        for payload in [
            types.nominal(RuntimeValue::String("wrong child".into())),
            RuntimeValue::NominalRecord(RuntimeNominalRecordValue::new(
                types.nominal.clone(),
                types.nominal_identity,
                TypeLayoutHash::from_bytes([0; 32]),
                vec![RuntimeValue::Bool(true)],
            )),
            RuntimeValue::NominalRecord(RuntimeNominalRecordValue::new(
                types.nominal.clone(),
                semantic(55),
                types.layout,
                vec![RuntimeValue::Bool(true)],
            )),
        ] {
            let bad = base
                .patched(
                    &CharacterDialoguePatch::default().with_hooks(PatchField::Set(vec![
                        CharacterDialogueHookValue::try_new(role_value(Role::Hook, payload))
                            .unwrap(),
                    ])),
                )
                .unwrap();
            assert!(matches!(
                schema.encode(&bad),
                Err(CharacterDialogueValueError::ProgramType(_))
            ));
        }
        let wrong_role = base
            .patched(
                &CharacterDialoguePatch::default().with_stage(PatchField::Set(
                    crate::CharacterDialogueStageValue::try_new(role_value(
                        Role::Portrait,
                        RuntimeValue::Tuple(vec![]),
                    ))
                    .unwrap(),
                )),
            )
            .unwrap();
        assert!(schema.encode(&wrong_role).is_err());
        let wrong_style = CharacterDialogueStyleValue::try_new(
            CharacterDialogueTypedValue::try_new(RuntimeValue::EntityRef(
                RuntimeEntityReference::Project {
                    family: DeclarationIdentityFamily::View,
                    public_id: PublicId::try_new("view.other").unwrap(),
                },
            ))
            .unwrap(),
        )
        .unwrap();
        let bad = CharacterDialogue::try_new(
            base.character().clone(),
            base.contract(),
            CharacterDialogueConfig::try_new(
                base.config().view().clone(),
                wrong_style,
                rich_text(6),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(matches!(
            schema.encode(&bad),
            Err(CharacterDialogueValueError::RoleType {
                role: Role::Style,
                ..
            })
        ));
    }
}

#[test]
fn custom_entries_use_active_types_and_require_canonical_id_order() {
    let (base, characters, views, _) = fixture();
    let types = Types::new();
    let ids = ["alpha", "beta"].map(|name| {
        CharacterDialogueCustomFieldId::try_new(format!("character_dialogue_field.{name}")).unwrap()
    });
    let custom = CharacterDialogueRuntimeCustomFieldCatalog::try_new(
        base.contract().custom_schema(),
        ids.clone().map(|id| {
            CharacterDialogueRuntimeCustomFieldDescriptor::new(
                id,
                semantic(54),
                true,
                BTreeSet::from([base.config().view().clone()]),
            )
        }),
    )
    .unwrap();
    let custom_value = |value| {
        CharacterDialogueCustomValue::try_new(
            CharacterDialogueTypedValue::try_new(types.nominal(value)).unwrap(),
        )
        .unwrap()
    };
    let dialogue = base
        .patched(
            &CharacterDialoguePatch::default()
                .with_custom(
                    ids[0].clone(),
                    PatchField::Set(custom_value(RuntimeValue::Bool(true))),
                )
                .with_custom(
                    ids[1].clone(),
                    PatchField::Set(custom_value(RuntimeValue::Bool(false))),
                ),
        )
        .unwrap();
    for program in types.programs() {
        let schema = types.schema(&characters, &views, &custom, program);
        let encoded = schema.encode(&dialogue).unwrap();
        assert_eq!(
            schema
                .try_decode_opaque(encoded.opaque())
                .unwrap()
                .dialogue(),
            &dialogue
        );
        let RuntimeValue::Tuple(fields) = encoded.opaque().payload() else {
            unreachable!()
        };
        let mut fields = fields.clone();
        let RuntimeValue::Seq(entries) = &fields[17] else {
            unreachable!()
        };
        let mut entries = entries.clone().into_values();
        assert!(
            entries
                .iter()
                .all(|value| matches!(value, RuntimeValue::Tuple(fields) if fields.len() == 2))
        );
        entries.reverse();
        fields[17] = RuntimeValue::Seq(RuntimeSeq::values(entries));
        assert_eq!(
            schema
                .try_decode_opaque(&rewrap(&dialogue, RuntimeValue::Tuple(fields)))
                .unwrap_err(),
            CharacterDialogueValueError::NonCanonicalCustomOrder
        );
        let bad = dialogue
            .patched(&CharacterDialoguePatch::default().with_custom(
                ids[0].clone(),
                PatchField::Set(custom_value(RuntimeValue::Unit)),
            ))
            .unwrap();
        assert!(matches!(
            schema.encode(&bad),
            Err(CharacterDialogueValueError::ProgramType(_))
        ));
    }
}

#[test]
fn schema_preflights_unused_role_payloads_and_custom_types() {
    let (_, characters, views, custom) = fixture();
    let mut types = Types::new();
    types.roles = CharacterDialogueRuntimeRoleTypes::new(
        Role::AUTHORED_BASE
            .map(|role| CharacterDialogueRuntimeRoleType::new(role_semantic(role), semantic(99))),
        semantic(50),
    );
    for program in types.programs() {
        assert!(matches!(
            CharacterDialogueRuntimeSchema::try_new(
                &characters,
                &views,
                &custom,
                &types.defaults,
                &types.roles,
                program
            ),
            Err(CharacterDialogueValueError::ProgramType(_))
        ));
    }
    let types = Types::new();
    let custom = CharacterDialogueRuntimeCustomFieldCatalog::try_new(
        custom.digest(),
        [CharacterDialogueRuntimeCustomFieldDescriptor::new(
            CharacterDialogueCustomFieldId::try_new("character_dialogue_field.unused").unwrap(),
            semantic(99),
            true,
            BTreeSet::new(),
        )],
    )
    .unwrap();
    for program in types.programs() {
        assert!(matches!(
            CharacterDialogueRuntimeSchema::try_new(
                &characters,
                &views,
                &custom,
                &types.defaults,
                &types.roles,
                program
            ),
            Err(CharacterDialogueValueError::ProgramType(_))
        ));
    }
}

#[test]
fn structured_role_patches_preserve_opaque_ownership_and_normalize_the_body() {
    let style = CharacterDialogueStyleValue::try_new(role_value(
        Role::RichText,
        RuntimeValue::Tuple(vec![
            RuntimeValue::option_some(RuntimeValue::Bool(true)),
            RuntimeValue::F64(-0.0),
        ]),
    ))
    .unwrap();
    let (base, _, _, _) = fixture_with_style(style);
    let patch = StructuredPatch::try_new(
        false,
        BTreeMap::from([(
            RuntimeFieldPath::try_new(vec![0]).unwrap(),
            PatchField::Clear,
        )]),
    )
    .unwrap();
    let changed = base
        .patched(&CharacterDialoguePatch::default().with_style(patch))
        .unwrap();
    let RuntimeValue::Opaque(value) = changed.config().style().typed().value() else {
        panic!("exact opaque role")
    };
    assert_eq!(value.semantic_identity(), role_semantic(Role::RichText));
    let RuntimeValue::Tuple(fields) = value.payload() else {
        unreachable!()
    };
    assert_eq!(fields[0], RuntimeValue::option_none());
    assert!(matches!(&fields[1], RuntimeValue::F64(value) if value.to_bits() == 0));
    let RuntimeValue::Opaque(original) = base.config().style().typed().value() else {
        unreachable!()
    };
    assert_ne!(original.payload(), value.payload());
}
