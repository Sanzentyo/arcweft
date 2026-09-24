use super::*;
use crate::{
    CharacterDialogueConfig, CharacterDialogueRolePayloadCodec,
    CharacterDialogueRuntimeCustomFieldDescriptor, CharacterDialogueRuntimeDefault,
    CharacterDialogueRuntimeDefaultCatalog, CharacterDialogueRuntimeRole as Role,
    CharacterDialogueRuntimeRoleBody, CharacterDialogueRuntimeRoleType,
    CharacterDialogueRuntimeRoleTypes, CharacterDialogueRuntimeSchema, CharacterDialogueType,
};
use arcweft_character::catalog::CharacterVisualManifestEvidence;
use arcweft_core::{
    awbc::schema::{
        AwbcProgram, AwbcRecordField, AwbcRuntimeType, AwbcRuntimeTypeShape as AwbcType,
        AwbcStringId, AwbcTypeId, AwbcVariantCase, AwbcVariantIdentity,
    },
    character_nominal::{CharacterNominalType, RuntimeCharacterLookSourceAuthority},
    entry::{
        RuntimeNominalRecordShape, RuntimeNominalSchemaBody, RuntimeNominalSchemaCase,
        RuntimeNominalSchemaDefinition, RuntimeNominalSchemaField, RuntimeNominalSchemaGraph,
        RuntimeNominalSchemaIdentity, RuntimeNominalTypeId, RuntimeSchemaLimits, RuntimeTypeSchema,
        TypeLayoutHash,
    },
    pattern::{
        RuntimeBuiltinVariantCaseIdentity, RuntimeBuiltinVariantIdentity, RuntimeCheckedType,
        RuntimeOpaqueTypeAdmission, RuntimeOpaqueTypeOwner, RuntimeSemanticTypeId,
        RuntimeVariantIdentity,
    },
    plan::{
        RuntimeNominalRecordDomainFieldSeed, RuntimeNominalRecordDomainSeed, RuntimePlan,
        RuntimePlanBuilder, RuntimePlanTypeProjection as Type, RuntimePlanTypeSeed,
        RuntimeVariantCaseSeed, RuntimeVariantDomainSeed,
    },
    task::RuntimeProgramOwner,
    value::{
        RuntimeEntityReference, RuntimeOpaquePersistence, RuntimeOpaqueValue,
        RuntimeOpaqueValueClass, RuntimeRecordFieldId, RuntimeSignedIntWidth,
        RuntimeUnsignedIntWidth, runtime_sequence_dense_bytes,
    },
};
use arcweft_id::{DeclarationIdentityFamily, PublicId};
use arcweft_interaction_model::dialogue::{
    CharacterDialogueFieldCoordinate as Coordinate, CharacterDialogueOperation,
    CharacterDialoguePatchField, CharacterDialoguePatchOperation as Operation,
};
use arcweft_view::{ViewId, ViewStyleSheetId};
use std::{collections::BTreeSet, sync::Arc};

fn semantic(tag: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([tag; 32])
}
fn role_semantic(role: Role) -> RuntimeSemanticTypeId {
    semantic(20 + role.canonical_tag())
}
fn role_types() -> CharacterDialogueRuntimeRoleTypes {
    CharacterDialogueRuntimeRoleTypes::new(
        Role::AUTHORED_BASE.map(|role| {
            let body = if role == Role::RichText {
                CharacterDialogueRuntimeRoleBody::bound(
                    CharacterDialogueRolePayloadCodec::RichTextProperties
                        .payload_schema()
                        .expect("RichText payload schema")
                        .root(),
                    CharacterDialogueRolePayloadCodec::RichTextProperties,
                )
            } else {
                CharacterDialogueRuntimeRoleBody::Unbound
            };
            CharacterDialogueRuntimeRoleType::new(role_semantic(role), body)
        }),
        semantic(50),
    )
}

fn append_rich_text_awbc_types(runtime_types: &mut Vec<AwbcRuntimeType>) {
    let seeds = CharacterDialogueRolePayloadCodec::RichTextProperties
        .payload_schema()
        .expect("RichText payload schema")
        .types();
    let first = runtime_types.len();
    let mut ids = std::collections::BTreeMap::new();
    for (offset, seed) in seeds.iter().enumerate() {
        let index = first + offset;
        ids.insert(
            seed.semantic_identity(),
            AwbcTypeId(u32::try_from(index).expect("bounded runtime type count")),
        );
    }
    let id = |identity: RuntimeSemanticTypeId| {
        *ids.get(&identity)
            .expect("codec seed graph contains every child identity")
    };
    for seed in seeds {
        let shape = match seed.projection() {
            Type::Bool => AwbcType::Bool,
            Type::Signed(width) => AwbcType::Int(match width {
                RuntimeSignedIntWidth::I8 => arcweft_core::awbc::schema::AwbcSignedIntKind::I8,
                RuntimeSignedIntWidth::I16 => arcweft_core::awbc::schema::AwbcSignedIntKind::I16,
                RuntimeSignedIntWidth::I32 => arcweft_core::awbc::schema::AwbcSignedIntKind::I32,
                RuntimeSignedIntWidth::I64 => arcweft_core::awbc::schema::AwbcSignedIntKind::I64,
                RuntimeSignedIntWidth::I128 => arcweft_core::awbc::schema::AwbcSignedIntKind::I128,
                RuntimeSignedIntWidth::ISize => {
                    arcweft_core::awbc::schema::AwbcSignedIntKind::ISize
                }
            }),
            Type::Unsigned(width) => AwbcType::UInt(match width {
                RuntimeUnsignedIntWidth::U8 => arcweft_core::awbc::schema::AwbcUnsignedIntKind::U8,
                RuntimeUnsignedIntWidth::U16 => {
                    arcweft_core::awbc::schema::AwbcUnsignedIntKind::U16
                }
                RuntimeUnsignedIntWidth::U32 => {
                    arcweft_core::awbc::schema::AwbcUnsignedIntKind::U32
                }
                RuntimeUnsignedIntWidth::U64 => {
                    arcweft_core::awbc::schema::AwbcUnsignedIntKind::U64
                }
                RuntimeUnsignedIntWidth::U128 => {
                    arcweft_core::awbc::schema::AwbcUnsignedIntKind::U128
                }
                RuntimeUnsignedIntWidth::USize => {
                    arcweft_core::awbc::schema::AwbcUnsignedIntKind::USize
                }
            }),
            Type::String => AwbcType::String,
            Type::Tuple(items) => AwbcType::Tuple(items.iter().map(|item| id(*item)).collect()),
            Type::Choice(items) => AwbcType::Choice(items.iter().map(|item| id(*item)).collect()),
            Type::Option { some_payload, .. } => AwbcType::Variant {
                owner: AwbcVariantIdentity::Builtin(RuntimeBuiltinVariantIdentity::Option),
                arguments: vec![],
                cases: vec![
                    AwbcVariantCase {
                        name: AwbcStringId(7),
                        payload: Some(id(*some_payload)),
                    },
                    AwbcVariantCase {
                        name: AwbcStringId(8),
                        payload: None,
                    },
                ],
            },
            _ => panic!("RichText codec emitted a non-structural plan seed"),
        };
        runtime_types.push(AwbcRuntimeType::new(seed.semantic_identity(), shape));
    }
}
fn voice_source_type() -> RuntimeSemanticTypeId {
    semantic(56)
}
fn look_source_type() -> RuntimeSemanticTypeId {
    CharacterNominalType::Look {
        character: sample_manifest().character().clone(),
    }
    .runtime_semantic_identity()
}
fn voice_source_nominal() -> RuntimeNominalTypeId {
    RuntimeNominalTypeId::try_new("DialogueVoice").unwrap()
}
fn look_source_nominal() -> RuntimeNominalTypeId {
    RuntimeNominalTypeId::from_checked_digest(*look_source_type().as_bytes())
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
        vec![
            RuntimeNominalSchemaDefinition::new(
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
            ),
            RuntimeNominalSchemaDefinition::new(
                RuntimeNominalSchemaIdentity::new(voice_source_nominal(), voice_source_type()),
                vec![],
                RuntimeNominalSchemaBody::Variant {
                    cases: vec![RuntimeNominalSchemaCase::new(0, "auto".into(), None)].into(),
                },
            ),
            RuntimeNominalSchemaDefinition::new(
                RuntimeNominalSchemaIdentity::new(look_source_nominal(), look_source_type()),
                vec![],
                RuntimeNominalSchemaBody::Variant {
                    cases: vec![RuntimeNominalSchemaCase::new(0, "normal".into(), None)].into(),
                },
            ),
        ],
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

fn source_variant_value(
    owner: &RuntimeProgramOwner,
    semantic_type: RuntimeSemanticTypeId,
    ordinal: u32,
    name: &str,
    payload: Option<RuntimeValue>,
) -> RuntimeValue {
    let RuntimeCheckedType::Variant {
        owner: variant_owner,
        ..
    } = owner.types().checked_type(semantic_type).unwrap()
    else {
        panic!("source binding must name an accepted variant type")
    };
    RuntimeValue::Variant {
        owner: variant_owner,
        ordinal,
        name: name.to_owned(),
        payload: payload.map(Box::new),
    }
}

struct Types {
    plan: RuntimePlan,
    awbc: AwbcProgram,
    roles: CharacterDialogueRuntimeRoleTypes,
    nominal: RuntimeNominalTypeId,
    nominal_identity: RuntimeSemanticTypeId,
    layout: TypeLayoutHash,
    character_type: RuntimeSemanticTypeId,
    any_type: RuntimeSemanticTypeId,
}

impl Types {
    fn new() -> Self {
        let field = RuntimeRecordFieldId::try_from_zero_based_ordinal(0).unwrap();
        let (nominal, nominal_identity, graph) = test_payload_graph();
        let layout = graph.try_layout_hash(nominal_identity).unwrap();
        let voice_layout = graph.try_layout_hash(voice_source_type()).unwrap();
        let look_layout = graph.try_layout_hash(look_source_type()).unwrap();
        let producer = CharacterDialogueRuntimeSchema::opaque_type_producer();
        let character = sample_manifest().character().clone();
        let character_dialogue = CharacterDialogueType::exact(character);
        let any_dialogue = CharacterDialogueType::any();
        let character_type = character_dialogue.runtime_semantic_identity();
        let any_type = any_dialogue.runtime_semantic_identity();
        let exact_owner = character_dialogue.runtime_opaque_owner();
        let any_owner = any_dialogue.runtime_opaque_owner();
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
        let rich_text_schema = CharacterDialogueRolePayloadCodec::RichTextProperties
            .payload_schema()
            .expect("RichText payload schema");
        seeds.extend(rich_text_schema.types().iter().cloned());
        seeds.extend([
            RuntimePlanTypeSeed::new(
                semantic(50),
                Type::Choice(vec![semantic(51), role_semantic(Role::RichText)].into()),
            ),
            RuntimePlanTypeSeed::new(semantic(51), Type::EntityReference),
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
            RuntimePlanTypeSeed::new(
                voice_source_type(),
                Type::Nominal {
                    nominal: voice_source_nominal(),
                    layout: voice_layout,
                    arguments: Box::new([]),
                },
            ),
            RuntimePlanTypeSeed::new(
                look_source_type(),
                Type::Nominal {
                    nominal: look_source_nominal(),
                    layout: look_layout,
                    arguments: Box::new([]),
                },
            ),
            RuntimePlanTypeSeed::new(
                character_type,
                Type::Opaque {
                    producer: exact_owner.producer().clone(),
                    admission: exact_owner.admission(),
                    value_class: exact_owner.value_class(),
                    persistence: exact_owner.persistence(),
                    arguments: Box::new([]),
                },
            ),
            RuntimePlanTypeSeed::new(
                any_type,
                Type::Opaque {
                    producer: any_owner.producer().clone(),
                    admission: any_owner.admission(),
                    value_class: any_owner.value_class(),
                    persistence: any_owner.persistence(),
                    arguments: Box::new([]),
                },
            ),
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
                [
                    RuntimeVariantDomainSeed::new(
                        voice_source_type(),
                        voice_source_nominal(),
                        voice_layout,
                        [RuntimeVariantCaseSeed::new("auto", None)],
                    ),
                    RuntimeVariantDomainSeed::new(
                        look_source_type(),
                        look_source_nominal(),
                        look_layout,
                        [RuntimeVariantCaseSeed::new("normal", None)],
                    ),
                ],
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
                        ty: AwbcTypeId(10),
                    }],
                },
            ),
            AwbcRuntimeType::new(semantic(55), AwbcType::Bool),
        ]);
        for owner in [exact_owner, any_owner] {
            runtime_types.push(AwbcRuntimeType::new(
                owner.semantic_identity(),
                AwbcType::Opaque {
                    producer: AwbcStringId(0),
                    admission: owner.admission(),
                    value_class: owner.value_class(),
                    persistence: owner.persistence(),
                    arguments: vec![],
                },
            ));
        }
        runtime_types.extend([
            AwbcRuntimeType::new(
                voice_source_type(),
                AwbcType::Variant {
                    owner: AwbcVariantIdentity::Nominal {
                        public_id: AwbcStringId(3),
                        layout: *voice_layout.as_bytes(),
                    },
                    arguments: vec![],
                    cases: vec![AwbcVariantCase {
                        name: AwbcStringId(4),
                        payload: None,
                    }],
                },
            ),
            AwbcRuntimeType::new(
                look_source_type(),
                AwbcType::Variant {
                    owner: AwbcVariantIdentity::Nominal {
                        public_id: AwbcStringId(5),
                        layout: *look_layout.as_bytes(),
                    },
                    arguments: vec![],
                    cases: vec![AwbcVariantCase {
                        name: AwbcStringId(6),
                        payload: None,
                    }],
                },
            ),
        ]);
        append_rich_text_awbc_types(&mut runtime_types);
        let awbc = AwbcProgram {
            strings: vec![
                producer.as_str().to_owned(),
                nominal.as_str().to_owned(),
                "flag".into(),
                voice_source_nominal().as_str().to_owned(),
                "auto".into(),
                look_source_nominal().as_str().to_owned(),
                "normal".into(),
                "Some".into(),
                "None".into(),
            ],
            runtime_types,
            ..AwbcProgram::default()
        };
        let roles = role_types();
        Self {
            plan,
            awbc,
            roles,
            nominal,
            nominal_identity,
            layout,
            character_type,
            any_type,
        }
    }
    fn program_owners(&self) -> [RuntimeProgramOwner; 2] {
        [
            RuntimeProgramOwner::Plan(Arc::new(self.plan.clone())),
            RuntimeProgramOwner::Awbc(Arc::new(self.awbc.clone())),
        ]
    }
    fn default_catalog() -> CharacterDialogueRuntimeDefaultCatalog {
        CharacterDialogueRuntimeDefaultCatalog::try_new([CharacterDialogueRuntimeDefault::new(
            sample_manifest().character().clone(),
            Self::default_config(),
        )])
        .unwrap()
    }
    fn try_schema(
        &self,
        characters: &CharacterCatalog,
        views: &ViewRegistry,
        custom: &CharacterDialogueRuntimeCustomFieldCatalog,
        owner: RuntimeProgramOwner,
    ) -> Result<CharacterDialogueRuntimeSchema, CharacterDialogueValueError> {
        let authority = self.look_authority(characters, owner.clone());
        self.try_schema_with_authority(views, custom, voice_source_type(), authority, owner)
    }

    fn look_authority(
        &self,
        characters: &CharacterCatalog,
        owner: RuntimeProgramOwner,
    ) -> Arc<RuntimeCharacterLookSourceAuthority> {
        Arc::new(
            RuntimeCharacterLookSourceAuthority::try_new(owner, Arc::new(characters.clone()))
                .unwrap(),
        )
    }

    fn try_schema_with_authority(
        &self,
        views: &ViewRegistry,
        custom: &CharacterDialogueRuntimeCustomFieldCatalog,
        voice_type: RuntimeSemanticTypeId,
        look_authority: Arc<RuntimeCharacterLookSourceAuthority>,
        owner: RuntimeProgramOwner,
    ) -> Result<CharacterDialogueRuntimeSchema, CharacterDialogueValueError> {
        CharacterDialogueRuntimeSchema::try_new(
            Arc::new(views.clone()),
            Arc::new(custom.clone()),
            Arc::new(Self::default_catalog()),
            self.roles.clone(),
            voice_type,
            look_authority,
            owner,
        )
    }
    fn schema(
        &self,
        characters: &CharacterCatalog,
        views: &ViewRegistry,
        custom: &CharacterDialogueRuntimeCustomFieldCatalog,
        owner: RuntimeProgramOwner,
    ) -> CharacterDialogueRuntimeSchema {
        self.try_schema(characters, views, custom, owner).unwrap()
    }

    fn default_config() -> CharacterDialogueConfig {
        CharacterDialogueConfig::try_from_presentation_profile(
            &crate::DialoguePresentationProfile::engine_default(),
            &role_types(),
        )
        .unwrap()
    }

    fn base(
        schema: &CharacterDialogueRuntimeSchema,
        owner: &RuntimeProgramOwner,
        character: &CharacterId,
        result_type: RuntimeSemanticTypeId,
    ) -> CharacterDialogue {
        let target = RuntimeValue::EntityRef(RuntimeEntityReference::Project {
            family: DeclarationIdentityFamily::Character,
            public_id: PublicId::try_new(character.as_str()).unwrap(),
        });
        schema
            .construct(owner, &target, &[], result_type)
            .unwrap()
            .dialogue()
            .clone()
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
    let (_, characters, views, custom) = fixture();
    let types = Types::new();
    let owners = types.program_owners();
    let base_owner = owners[0].clone();
    let base_schema = types.schema(&characters, &views, &custom, base_owner.clone());
    let base = Types::base(
        &base_schema,
        &base_owner,
        sample_manifest().character(),
        types.character_type,
    );
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
                        .with_voice(voice.map_or(PatchField::Clear, PatchField::Set)),
                )
                .unwrap();
            let mut digests = Vec::new();
            for owner in owners.iter().cloned() {
                let schema = types.schema(&characters, &views, &custom, owner);
                let encoded = schema.encode(&dialogue).unwrap();
                let RuntimeValue::Tuple(fields) = encoded.opaque().payload() else {
                    panic!("tuple")
                };
                assert_eq!(fields.len(), 18);
                assert!(matches!(
                    fields[1],
                    RuntimeValue::Variant { ordinal: 0, .. }
                ));
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
    let (_, characters, views, custom) = fixture();
    let types = Types::new();
    for owner in types.program_owners() {
        let schema = types.schema(&characters, &views, &custom, owner.clone());
        let dialogue = Types::base(
            &schema,
            &owner,
            sample_manifest().character(),
            types.character_type,
        );
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
    let (_, characters, views, custom) = fixture();
    let types = Types::new();
    for owner in types.program_owners() {
        let schema = types.schema(&characters, &views, &custom, owner.clone());
        let dialogue = Types::base(
            &schema,
            &owner,
            sample_manifest().character(),
            types.character_type,
        );
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
    let (_, characters, views, custom) = fixture();
    let types = Types::new();
    for owner in types.program_owners() {
        let schema = types.schema(&characters, &views, &custom, owner.clone());
        let base = Types::base(
            &schema,
            &owner,
            sample_manifest().character(),
            types.character_type,
        );
        let target = RuntimeValue::EntityRef(RuntimeEntityReference::Project {
            family: DeclarationIdentityFamily::Character,
            public_id: PublicId::try_new(sample_manifest().character().as_str()).unwrap(),
        });
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
            let bad = schema.apply(
                &owner,
                CharacterDialogueOperation::Factory,
                target.clone(),
                &[CharacterDialoguePatchField {
                    coordinate: Coordinate::RichText,
                    operation: Operation::Set(role_value(Role::RichText, payload).into_value()),
                }],
                types.any_type,
            );
            assert!(matches!(
                bad,
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
    let (_, characters, views, _) = fixture();
    let types = Types::new();
    let base_owner = types.program_owners()[0].clone();
    let ids = ["alpha", "beta"].map(|name| {
        CharacterDialogueCustomFieldId::try_new(format!("character_dialogue_field.{name}")).unwrap()
    });
    let custom = CharacterDialogueRuntimeCustomFieldCatalog::try_new(ids.clone().map(|id| {
        CharacterDialogueRuntimeCustomFieldDescriptor::new(
            id,
            semantic(54),
            true,
            BTreeSet::from([ViewId::standard_dialogue()]),
        )
    }))
    .unwrap();
    let base_schema = types.schema(&characters, &views, &custom, base_owner.clone());
    let base = Types::base(
        &base_schema,
        &base_owner,
        sample_manifest().character(),
        types.character_type,
    );
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
    for owner in types.program_owners() {
        let schema = types.schema(&characters, &views, &custom, owner);
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
    let invalid_roles = Role::AUTHORED_BASE.map(|role| {
        CharacterDialogueRuntimeRoleType::new(
            role_semantic(role),
            if role == Role::Stage {
                CharacterDialogueRuntimeRoleBody::bound(
                    semantic(99),
                    CharacterDialogueRolePayloadCodec::RichTextProperties,
                )
            } else if role == Role::RichText {
                CharacterDialogueRuntimeRoleBody::bound(
                    CharacterDialogueRolePayloadCodec::RichTextProperties
                        .payload_schema()
                        .unwrap()
                        .root(),
                    CharacterDialogueRolePayloadCodec::RichTextProperties,
                )
            } else {
                CharacterDialogueRuntimeRoleBody::Unbound
            },
        )
    });
    types.roles = CharacterDialogueRuntimeRoleTypes::new(invalid_roles, semantic(50));
    for owner in types.program_owners() {
        assert!(matches!(
            types.try_schema(&characters, &views, &custom, owner),
            Err(CharacterDialogueValueError::RoleType {
                role: Role::Stage,
                ..
            })
        ));
    }
    let types = Types::new();
    let custom = CharacterDialogueRuntimeCustomFieldCatalog::try_new([
        CharacterDialogueRuntimeCustomFieldDescriptor::new(
            CharacterDialogueCustomFieldId::try_new("character_dialogue_field.unused").unwrap(),
            semantic(99),
            true,
            BTreeSet::new(),
        ),
    ])
    .unwrap();
    for owner in types.program_owners() {
        assert!(matches!(
            types.try_schema(&characters, &views, &custom, owner),
            Err(CharacterDialogueValueError::ProgramType(_))
        ));
    }
}

#[test]
fn unbound_optional_roles_reject_set_and_profile_style_clear_is_no_overrides() {
    let (_, characters, views, custom) = fixture();
    let types = Types::new();
    let owner = types.program_owners()[0].clone();
    let character = sample_manifest().character().clone();
    let roles = role_types();
    let style_sheet = ViewStyleSheetId::try_new("style.dialogue").unwrap();
    let profile = crate::DialoguePresentationProfile::new(
        ViewId::standard_dialogue(),
        Some(style_sheet.clone()),
        crate::InlineFailurePolicy::Discard,
    );
    let config = CharacterDialogueConfig::try_from_presentation_profile(&profile, &roles).unwrap();
    assert_eq!(config.view(), profile.view());
    assert_eq!(config.inline_failure(), profile.inline_failure());
    assert_eq!(
        config.rich_text().typed().value(),
        &role_value(
            Role::RichText,
            CharacterDialogueRolePayloadCodec::RichTextProperties
                .no_overrides_payload()
                .unwrap(),
        )
        .value()
        .clone()
    );
    assert!(matches!(
        config.style().typed().value(),
        RuntimeValue::EntityRef(RuntimeEntityReference::Project {
            family: DeclarationIdentityFamily::Style,
            public_id,
        }) if public_id == style_sheet.public_id()
    ));

    let defaults =
        CharacterDialogueRuntimeDefaultCatalog::try_new([CharacterDialogueRuntimeDefault::new(
            character.clone(),
            config,
        )])
        .unwrap();
    let authority = types.look_authority(&characters, owner.clone());
    let schema = CharacterDialogueRuntimeSchema::try_new(
        Arc::new(views.clone()),
        Arc::new(custom.clone()),
        Arc::new(defaults),
        roles,
        voice_source_type(),
        authority,
        owner.clone(),
    )
    .unwrap();
    let target = RuntimeValue::EntityRef(RuntimeEntityReference::Project {
        family: DeclarationIdentityFamily::Character,
        public_id: PublicId::try_new(character.as_str()).unwrap(),
    });

    let unbound = schema.construct(
        &owner,
        &target,
        &[CharacterDialoguePatchField {
            coordinate: Coordinate::Stage,
            operation: Operation::Set(role_value(Role::Stage, RuntimeValue::Unit).into_value()),
        }],
        types.any_type,
    );
    assert!(matches!(
        unbound,
        Err(CharacterDialogueValueError::RoleType {
            role: Role::Stage,
            ..
        })
    ));

    let cleared = schema
        .construct(
            &owner,
            &target,
            &[CharacterDialoguePatchField {
                coordinate: Coordinate::Style,
                operation: Operation::Clear,
            }],
            types.any_type,
        )
        .unwrap();
    let cleared = schema
        .admit(&owner, &cleared.into_runtime_value(), types.any_type)
        .unwrap();
    assert_eq!(
        cleared.dialogue().config().style().typed().value(),
        &role_value(
            Role::RichText,
            CharacterDialogueRolePayloadCodec::RichTextProperties
                .no_overrides_payload()
                .unwrap(),
        )
        .value()
        .clone()
    );
    assert_eq!(
        cleared.dialogue().config().inline_failure(),
        &crate::InlineFailurePolicy::Discard
    );
}

#[test]
fn generation_producer_constructs_and_reconfigures_absent_visual_members() {
    let (_, _, views, custom) = fixture();
    let types = Types::new();
    let character = sample_manifest().character().clone();
    let characters = CharacterCatalog::try_from_declarations([(
        character.clone(),
        CharacterVisualManifestEvidence::Absent,
    )])
    .unwrap();
    assert!(characters.contains_character(&character));
    assert!(characters.visual_manifest(&character).is_none());
    let owner = types.program_owners()[1].clone();
    let schema = types.schema(&characters, &views, &custom, owner.clone());
    assert!(schema.program_owner().same_program(&owner));
    let target = RuntimeValue::EntityRef(RuntimeEntityReference::Project {
        family: DeclarationIdentityFamily::Character,
        public_id: PublicId::try_new(character.as_str()).unwrap(),
    });
    let fields = [
        CharacterDialoguePatchField {
            coordinate: Coordinate::SourceLocale,
            operation: Operation::Set(RuntimeValue::String("en-US".into())),
        },
        CharacterDialoguePatchField {
            coordinate: Coordinate::SourceLocale,
            operation: Operation::Clear,
        },
        CharacterDialoguePatchField {
            coordinate: Coordinate::SourceLocale,
            operation: Operation::Set(RuntimeValue::String("ja-JP".into())),
        },
    ];
    let produced = schema
        .apply(
            &owner,
            CharacterDialogueOperation::Factory,
            target.clone(),
            &fields,
            types.any_type,
        )
        .unwrap();
    let admitted = schema.admit(&owner, &produced, types.any_type).unwrap();
    assert_eq!(
        admitted
            .dialogue()
            .config()
            .source_locale()
            .unwrap()
            .as_str(),
        "ja-JP"
    );
    assert_eq!(
        admitted.opaque().semantic_identity(),
        CharacterDialogueType::exact(character.clone()).runtime_semantic_identity()
    );
    let RuntimeValue::Tuple(payload) = admitted.opaque().payload() else {
        panic!("CharacterDialogue payload must remain the exact 18-slot tuple")
    };
    assert_eq!(payload.len(), 18);
    assert_eq!(payload[1], RuntimeValue::option_none());
    assert_eq!(
        payload[2],
        runtime_sequence_dense_bytes(
            admitted
                .dialogue()
                .contract()
                .defaults()
                .as_bytes()
                .to_vec()
        )
    );
    assert_eq!(
        payload[12],
        RuntimeValue::option_some(RuntimeValue::String("ja-JP".into()))
    );

    let reconfigured = schema
        .apply(
            &owner,
            CharacterDialogueOperation::Reconfigure,
            produced.clone(),
            &[CharacterDialoguePatchField {
                coordinate: Coordinate::SourceLocale,
                operation: Operation::Clear,
            }],
            types.any_type,
        )
        .unwrap();
    let reconfigured = schema.admit(&owner, &reconfigured, types.any_type).unwrap();
    assert_eq!(reconfigured.dialogue().config().source_locale(), None);
    assert_eq!(
        reconfigured.opaque().semantic_identity(),
        admitted.opaque().semantic_identity()
    );

    let foreign_owner = RuntimeProgramOwner::Awbc(Arc::new(types.awbc.clone()));
    assert!(matches!(
        schema.admit(&foreign_owner, &produced, types.any_type),
        Err(CharacterDialogueValueError::ForeignProgramOwner)
    ));
    assert!(matches!(
        schema.construct(
            &owner,
            &target,
            &[CharacterDialoguePatchField {
                coordinate: Coordinate::Look,
                operation: Operation::Set(RuntimeValue::String("normal".into())),
            }],
            types.any_type,
        ),
        Err(CharacterDialogueValueError::MissingVisualManifest(actual)) if actual == character
    ));
}

#[test]
fn source_voice_and_look_values_are_joined_through_exact_program_rows() {
    let (_, characters, views, custom) = fixture();
    let types = Types::new();
    let character = sample_manifest().character().clone();
    let target = RuntimeValue::EntityRef(RuntimeEntityReference::Project {
        family: DeclarationIdentityFamily::Character,
        public_id: PublicId::try_new(character.as_str()).unwrap(),
    });
    let rich_text = || {
        role_value(
            Role::RichText,
            CharacterDialogueRolePayloadCodec::RichTextProperties
                .no_overrides_payload()
                .unwrap(),
        )
        .into_value()
    };
    for owner in types.program_owners() {
        let schema = types.schema(&characters, &views, &custom, owner.clone());
        let base = Types::base(&schema, &owner, &character, types.character_type);
        let base_digest = schema.effective_config_digest(&base).unwrap();
        let voice = source_variant_value(&owner, voice_source_type(), 0, "auto", None);
        let look = source_variant_value(&owner, look_source_type(), 0, "normal", None);
        let produced = schema
            .construct(
                &owner,
                &target,
                &[
                    CharacterDialoguePatchField {
                        coordinate: Coordinate::Voice,
                        operation: Operation::Set(voice),
                    },
                    CharacterDialoguePatchField {
                        coordinate: Coordinate::Look,
                        operation: Operation::Set(look),
                    },
                    CharacterDialoguePatchField {
                        coordinate: Coordinate::RichText,
                        operation: Operation::Set(rich_text()),
                    },
                    CharacterDialoguePatchField {
                        coordinate: Coordinate::RichText,
                        operation: Operation::Set(rich_text()),
                    },
                    CharacterDialoguePatchField {
                        coordinate: Coordinate::Style,
                        operation: Operation::Set(rich_text()),
                    },
                    CharacterDialoguePatchField {
                        coordinate: Coordinate::Style,
                        operation: Operation::Set(rich_text()),
                    },
                ],
                types.any_type,
            )
            .unwrap();
        let value = schema
            .admit(&owner, &produced.into_runtime_value(), types.any_type)
            .unwrap();
        assert_eq!(
            value.dialogue().config().voice(),
            Some(&CharacterDialogueVoice::Auto)
        );
        assert_eq!(
            value
                .dialogue()
                .config()
                .look()
                .map(CharacterLookId::as_str),
            Some("normal")
        );
        let RuntimeValue::Opaque(rich_text) = value.dialogue().config().rich_text().typed().value()
        else {
            panic!("exact RichText owner")
        };
        assert_eq!(
            rich_text.payload(),
            &CharacterDialogueRolePayloadCodec::RichTextProperties
                .no_overrides_payload()
                .unwrap()
        );
        let RuntimeValue::Opaque(style) = value.dialogue().config().style().typed().value() else {
            panic!("structured Style branch")
        };
        assert_eq!(
            style.payload(),
            &CharacterDialogueRolePayloadCodec::RichTextProperties
                .no_overrides_payload()
                .unwrap()
        );
        assert_ne!(
            schema.effective_config_digest(value.dialogue()).unwrap(),
            base_digest
        );

        let RuntimeValue::Tuple(payload) = value.opaque().payload() else {
            panic!("exact 18-slot wire tuple")
        };
        assert!(matches!(
            payload[5].builtin_variant_case(),
            Some((RuntimeBuiltinVariantCaseIdentity::OptionSome, Some(
                RuntimeValue::Variant { ordinal: 0, name, payload: None, .. }
            ))) if name == "Auto"
        ));
        assert_eq!(
            payload[6],
            RuntimeValue::option_some(RuntimeValue::String("normal".into()))
        );
    }
}

#[test]
fn source_patch_rows_reject_values_outside_the_exact_voice_and_character_look_cases() {
    let (_, characters, views, custom) = fixture();
    let types = Types::new();
    let owner = types.program_owners()[0].clone();
    let schema = types.schema(&characters, &views, &custom, owner.clone());
    let character = sample_manifest().character().clone();
    let target = RuntimeValue::EntityRef(RuntimeEntityReference::Project {
        family: DeclarationIdentityFamily::Character,
        public_id: PublicId::try_new(character.as_str()).unwrap(),
    });
    let valid_voice = source_variant_value(&owner, voice_source_type(), 0, "auto", None);
    let valid_look = source_variant_value(&owner, look_source_type(), 0, "normal", None);
    let RuntimeValue::Variant {
        owner: look_owner, ..
    } = source_variant_value(&owner, look_source_type(), 0, "normal", None)
    else {
        unreachable!()
    };
    let mut foreign_voice_owner = valid_voice.clone();
    let RuntimeValue::Variant {
        owner: value_owner, ..
    } = &mut foreign_voice_owner
    else {
        unreachable!()
    };
    *value_owner = look_owner;
    let invalid_voice_values = [
        foreign_voice_owner,
        source_variant_value(&owner, voice_source_type(), 0, "Auto", None),
        source_variant_value(
            &owner,
            voice_source_type(),
            0,
            "auto",
            Some(RuntimeValue::Unit),
        ),
    ];
    for value in invalid_voice_values {
        assert!(
            schema
                .construct(
                    &owner,
                    &target,
                    &[CharacterDialoguePatchField {
                        coordinate: Coordinate::Voice,
                        operation: Operation::Set(value),
                    }],
                    types.any_type,
                )
                .is_err()
        );
    }
    for value in [
        RuntimeValue::String("normal".into()),
        source_variant_value(&owner, look_source_type(), 0, "Normal", None),
        source_variant_value(&owner, look_source_type(), 1, "normal", None),
    ] {
        assert!(
            schema
                .construct(
                    &owner,
                    &target,
                    &[CharacterDialoguePatchField {
                        coordinate: Coordinate::Look,
                        operation: Operation::Set(value),
                    }],
                    types.any_type,
                )
                .is_err()
        );
    }
    assert_eq!(
        source_variant_value(&owner, look_source_type(), 0, "normal", None),
        valid_look
    );
}

#[test]
fn source_type_inventory_requires_exact_voice_rows_and_present_character_look_coverage() {
    let (_, characters, views, custom) = fixture();
    let types = Types::new();
    for owner in types.program_owners() {
        let authority = types.look_authority(&characters, owner.clone());
        let foreign_owner = match &owner {
            RuntimeProgramOwner::Plan(_) => RuntimeProgramOwner::Awbc(Arc::new(types.awbc.clone())),
            RuntimeProgramOwner::Awbc(_) => RuntimeProgramOwner::Plan(Arc::new(types.plan.clone())),
        };
        assert!(authority.program_owner().same_program(&owner));
        assert!(
            types
                .try_schema_with_authority(
                    &views,
                    &custom,
                    semantic(53),
                    Arc::clone(&authority),
                    owner.clone(),
                )
                .is_err_and(|error| matches!(
                    error,
                    CharacterDialogueValueError::VoiceSourceType { .. }
                ))
        );
        assert!(matches!(
            types.try_schema_with_authority(
                &views,
                &custom,
                voice_source_type(),
                Arc::clone(&authority),
                foreign_owner,
            ),
            Err(CharacterDialogueValueError::ForeignProgramOwner)
        ));
    }

    let mut incomplete_awbc = types.awbc.clone();
    incomplete_awbc
        .runtime_types
        .retain(|row| row.semantic_identity() != look_source_type());
    assert!(matches!(
        RuntimeCharacterLookSourceAuthority::try_new(
            RuntimeProgramOwner::Awbc(Arc::new(incomplete_awbc)),
            Arc::new(characters),
        ),
        Err(arcweft_core::character_nominal::RuntimeCharacterLookSourceError::ProgramType(_))
    ));
}

#[test]
fn generation_recomputes_default_digest_from_effective_config() {
    let (_, characters, views, custom) = fixture();
    let types = Types::new();
    let owner = types.program_owners()[0].clone();
    let actual = Types::base(
        &types.schema(&characters, &views, &custom, owner.clone()),
        &owner,
        sample_manifest().character(),
        types.character_type,
    )
    .contract()
    .defaults();
    let mut wrong = *actual.as_bytes();
    wrong[0] ^= 1;
    let defaults = CharacterDialogueRuntimeDefaultCatalog::try_new([
        CharacterDialogueRuntimeDefault::with_expected_digest(
            sample_manifest().character().clone(),
            Types::default_config(),
            RuntimeValueDigest::from_bytes(wrong),
        ),
    ])
    .unwrap();
    let look_authority = types.look_authority(&characters, owner.clone());
    assert!(matches!(
        CharacterDialogueRuntimeSchema::try_new(
            Arc::new(views),
            Arc::new(custom),
            Arc::new(defaults),
            types.roles,
            voice_source_type(),
            look_authority,
            owner,
        ),
        Err(CharacterDialogueValueError::DefaultDigestMismatch(character))
            if character == sample_manifest().character().clone()
    ));
}

#[test]
fn generation_requires_defaults_for_every_logical_character_member() {
    let (_, _, views, custom) = fixture();
    let types = Types::new();
    let catalog = CharacterCatalog::try_from_declarations([
        (
            sample_manifest().character().clone(),
            CharacterVisualManifestEvidence::Present(sample_manifest()),
        ),
        (
            CharacterId::try_new("character.bob").unwrap(),
            CharacterVisualManifestEvidence::Absent,
        ),
    ])
    .unwrap();
    let owner = types.program_owners()[0].clone();
    assert!(matches!(
        types.try_schema(&catalog, &views, &custom, owner),
        Err(CharacterDialogueValueError::MissingDefaults(character))
            if character == CharacterId::try_new("character.bob").unwrap()
    ));
}

#[test]
fn structured_role_patches_preserve_opaque_ownership_and_normalize_the_body() {
    let style = CharacterDialogueStyleValue::try_new(role_value(
        Role::RichText,
        RuntimeValue::Tuple(vec![
            RuntimeValue::option_some(RuntimeValue::Bool(true)),
            RuntimeValue::option_some(RuntimeValue::String("old".into())),
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
    assert_eq!(
        fields[1],
        RuntimeValue::option_some(RuntimeValue::String("old".into()))
    );
    let RuntimeValue::Opaque(original) = base.config().style().typed().value() else {
        unreachable!()
    };
    assert_ne!(original.payload(), value.payload());
}
