use super::*;
use arcweft_core::{engine::ChoiceState, plan::ChoiceRuntimeOption};

#[test]
fn choices_preserve_a_missing_id_without_synthesizing_from_the_label() {
    let status = FlowFiberStatus::Choice(ChoiceState {
        id: None,
        options: vec![ChoiceRuntimeOption {
            id: None,
            label: "Listen".to_owned(),
            target: None,
            out: None,
            effects: Vec::new(),
        }],
    });

    assert_eq!(
        choices_from_status(&status),
        vec![BundleChoice {
            id: None,
            label: "Listen".to_owned(),
        }]
    );
}

mod dynamic_character_dialogue_context_tests {
    use super::*;
    use arcweft_bundle::resource_codec::view::ViewStyleResource;
    use arcweft_character::{
        catalog::{CharacterCatalog, CharacterVisualManifestEvidence},
        id::CharacterId,
        presentation_name::{
            AcceptedCharacterPresentationCatalog, CharacterDisplayNameInput,
            CharacterDisplayNameRecordInput, CharacterDisplayNameValue,
            CharacterPresentationCatalogData, CharacterPresentationCatalogInput,
            CharacterPresentationRole,
        },
    };
    use arcweft_core::{
        effect::RuntimeArtifactFingerprint, plan::RuntimeLineId,
        runtime_id::RuntimeDialogueContentTemplateId, value::RuntimeDialogueContentValue,
    };
    use arcweft_core::{
        entry::{
            RuntimeNominalSchemaBody, RuntimeNominalSchemaCase, RuntimeNominalSchemaDefinition,
            RuntimeNominalSchemaGraph, RuntimeNominalSchemaIdentity, RuntimeNominalTypeId,
            RuntimeSchemaLimits, RuntimeValueDigest,
        },
        pattern::{RuntimeOpaqueTypeAdmission, RuntimeOpaqueTypeOwner, RuntimeSemanticTypeId},
        plan::{
            RuntimePlan, RuntimePlanBuilder, RuntimePlanTypeProjection as Type,
            RuntimePlanTypeSeed, RuntimeVariantCaseSeed, RuntimeVariantDomainSeed,
        },
        task::RuntimeProgramOwner,
        value::{
            RuntimeEntityReference, RuntimeOpaquePersistence, RuntimeOpaqueValue,
            RuntimeOpaqueValueClass, RuntimeUInt, RuntimeValue,
        },
    };
    use arcweft_dialogue::{
        CharacterDialogueCharacterDeclaration, CharacterDialogueConfig,
        CharacterDialogueGenerationDeclaration, CharacterDialoguePolicyTypeGraph,
        CharacterDialoguePresentationContract, CharacterDialogueRolePayloadCodec,
        CharacterDialogueRuntimeCustomFieldCatalog, CharacterDialogueRuntimeDefault,
        CharacterDialogueRuntimeRole as Role, CharacterDialogueRuntimeRoleBody,
        CharacterDialogueRuntimeRoleType, CharacterDialogueRuntimeRoleTypes,
        CharacterDialogueRuntimeSchema, CharacterDialogueType, CharacterDialogueVisualType,
        DialoguePresentationProfile, DialogueProfileRevision,
        character_presentation::{
            CharacterPresentationTargetEvidence, CheckedCharacterPresentationPlan,
        },
    };
    use arcweft_id::{DeclarationIdentityFamily, LocaleTag, PublicId, TextKey};
    use arcweft_interaction_model::dialogue::{
        CharacterDialogueFieldCoordinate as Coordinate, CharacterDialoguePatchField,
        CharacterDialoguePatchOperation as Operation,
    };
    use arcweft_presentation::rich_text::{
        PRESENTATION_CONTENT_CALLABLE_CATALOG,
        PresentationContentCallableDefinitionId as Definition,
        PresentationContentCallableParameterId as Parameter,
    };
    use arcweft_render_text::resolve_frame_with_template;
    use arcweft_resource_model::registry::ResourceTypeRegistry;
    use arcweft_source::{
        ProductSourceRef, SourceDocument, SourceDocumentId, SourceName, SourceSetRevision,
    };
    use arcweft_text_model::{
        DialogueContentFragmentTemplate, DialogueContentSpec, DialoguePresentationSnapshot,
        LineDisplayFrame, RichTextColor, RichTextDocument, RichTextNode, RichTextStyle,
    };
    use arcweft_view::{
        AcceptedViewProgramRevision, RustViewId, ViewDescriptor, ViewProgramId, ViewRegistry,
        ViewSchemaId, ViewStyleProgram, ViewStyleSheet, ViewStyleSheetId,
    };
    use std::sync::Arc;

    const ALICE: &str = "character.dynamic_alice";
    const BOB: &str = "character.dynamic_bob";

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
                            .expect("accepted RichText schema")
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

    fn runtime_plan() -> (RuntimePlan, RuntimeSemanticTypeId) {
        let roles = role_types();
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
        seeds.extend(
            CharacterDialogueRolePayloadCodec::RichTextProperties
                .payload_schema()
                .expect("accepted RichText schema")
                .types()
                .iter()
                .cloned(),
        );
        let policy_graph = CharacterDialoguePolicyTypeGraph::try_new(
            RuntimeOpaqueTypeOwner::exact_with(
                producer.clone(),
                role_semantic(Role::RichText),
                RuntimeOpaqueValueClass::Plain,
                RuntimeOpaquePersistence::ConstantAndSnapshot,
            ),
            RuntimeSchemaLimits::engine_default(),
        )
        .expect("accepted CharacterDialogue policy graph");
        seeds.extend(policy_graph.type_seeds());

        let style_entity_type = semantic(51);
        let style_type = *roles.style_ref();
        seeds.extend([
            RuntimePlanTypeSeed::new(style_entity_type, Type::EntityReference),
            RuntimePlanTypeSeed::new(
                style_type,
                Type::Choice(vec![style_entity_type, role_semantic(Role::RichText)].into()),
            ),
        ]);

        let alice_owner = CharacterDialogueType::exact(
            CharacterId::try_new(ALICE).expect("valid Alice identity"),
        )
        .runtime_opaque_owner();
        let bob_owner =
            CharacterDialogueType::exact(CharacterId::try_new(BOB).expect("valid Bob identity"))
                .runtime_opaque_owner();
        let any_owner = CharacterDialogueType::any().runtime_opaque_owner();
        let any_type = any_owner.semantic_identity();
        for owner in [alice_owner, bob_owner, any_owner] {
            let dialogue_type = owner.semantic_identity();
            seeds.push(RuntimePlanTypeSeed::new(
                dialogue_type,
                Type::Opaque {
                    producer: owner.producer().clone(),
                    admission: owner.admission(),
                    value_class: owner.value_class(),
                    persistence: owner.persistence(),
                    arguments: Box::new([]),
                },
            ));
        }

        let voice_type = semantic(56);
        let voice_nominal = RuntimeNominalTypeId::try_new("DialogueVoice")
            .expect("canonical DialogueVoice nominal id");
        let voice_graph = RuntimeNominalSchemaGraph::try_new(
            [RuntimeNominalSchemaDefinition::new(
                arcweft_core::entry::RuntimeNominalDeclarationId::from_bytes(
                    *(RuntimeNominalSchemaIdentity::new(voice_nominal.clone(), voice_type))
                        .semantic_identity()
                        .as_bytes(),
                ),
                RuntimeNominalSchemaIdentity::new(voice_nominal.clone(), voice_type),
                Vec::<arcweft_core::entry::RuntimeTypeSchema>::new(),
                RuntimeNominalSchemaBody::Variant {
                    cases: vec![RuntimeNominalSchemaCase::new(0, "auto".to_owned(), None)].into(),
                },
            )],
            RuntimeSchemaLimits::engine_default(),
        )
        .expect("DialogueVoice schema");
        let voice_layout = voice_graph
            .try_layout_hash(voice_type)
            .expect("DialogueVoice layout");
        seeds.push(RuntimePlanTypeSeed::new(
            voice_type,
            Type::Nominal {
                nominal: voice_nominal.clone(),
                layout: voice_layout,
                arguments: Box::new([]),
            },
        ));
        let graph = RuntimeNominalSchemaGraph::try_merge(
            [&voice_graph, policy_graph.schema_graph().as_ref()],
            RuntimeSchemaLimits::engine_default(),
        )
        .expect("merged CharacterDialogue and voice schemas");
        let mut variant_domains = vec![RuntimeVariantDomainSeed::new(
            voice_type,
            voice_nominal,
            voice_layout,
            [RuntimeVariantCaseSeed::new("auto", None)],
        )];
        variant_domains.extend(policy_graph.variant_domain_seeds());

        let mut builder = RuntimePlanBuilder::new();
        builder
            .admit_semantic_batch(seeds, [], [], variant_domains, &graph)
            .expect("runtime schema types are admitted");
        (builder.finish().expect("runtime plan"), any_type)
    }

    struct Fixture {
        schema: CharacterDialogueRuntimeSchema,
        owner: RuntimeProgramOwner,
        alice: CharacterId,
        bob: CharacterId,
        catalog: AcceptedCharacterPresentationCatalog,
        locale: LocaleTag,
        project_locale: ProjectLocaleSpec,
        style_program: ViewStyleProgram,
        sheet_id: ViewStyleSheetId,
        template: DialogueContentFragmentTemplate,
        content: RuntimeDialogueContentValue,
    }

    impl Fixture {
        fn new() -> Self {
            let alice = CharacterId::try_new(ALICE).expect("valid Alice identity");
            let bob = CharacterId::try_new(BOB).expect("valid Bob identity");
            let asset_characters = Arc::new(
                CharacterCatalog::try_from_declarations([
                    (alice.clone(), CharacterVisualManifestEvidence::Absent),
                    (bob.clone(), CharacterVisualManifestEvidence::Absent),
                ])
                .expect("accepted logical Character catalog"),
            );

            let profile = DialoguePresentationProfile::engine_default();
            let mut views = ViewRegistry::default();
            views
                .register(ViewDescriptor::public_rust(
                    profile.view().clone(),
                    ViewSchemaId(1),
                    RustViewId(1),
                ))
                .expect("accepted dialogue View");
            let sheet_id = ViewStyleSheetId::try_new("style.dynamic_dialogue_fixture")
                .expect("accepted Style identity");
            let style_program = ViewStyleProgram::try_new(
                vec![
                    ViewStyleSheet::new(sheet_id.clone(), Vec::new(), Vec::new())
                        .expect("accepted Style sheet"),
                ],
                Vec::new(),
            )
            .expect("accepted View Style program");
            let style_resource = ViewStyleResource {
                style_program_id: "view_style.dynamic_dialogue_fixture".to_owned(),
                program: style_program.clone(),
                ..ViewStyleResource::default()
            };
            let style_digest = RuntimeValueDigest::from_bytes(
                style_resource
                    .canonical_digest()
                    .expect("canonical Style resource digest")
                    .as_bytes(),
            );

            let roles = role_types();
            let default_config =
                CharacterDialogueConfig::try_from_presentation_profile(&profile, &roles)
                    .expect("accepted profile defaults");
            let any_type = CharacterDialogueType::any().runtime_semantic_identity();
            let (plan, _) = runtime_plan();
            let declaration = CharacterDialogueGenerationDeclaration::try_new(
                [alice.clone(), bob.clone()].map(|character| {
                    let dialogue_type =
                        CharacterDialogueType::exact(character.clone()).runtime_semantic_identity();
                    (
                        character.clone(),
                        CharacterDialogueCharacterDeclaration::new(
                            dialogue_type,
                            CharacterDialogueVisualType::Absent,
                            CharacterDialogueRuntimeDefault::new(character, default_config.clone()),
                        ),
                    )
                }),
                any_type,
                semantic(56),
                roles,
                CharacterDialogueRuntimeCustomFieldCatalog::try_new([])
                    .expect("empty accepted custom catalog"),
                CharacterDialoguePresentationContract::try_new(
                    profile.clone(),
                    profile_revision(),
                    RuntimeValueDigest::from_bytes(
                        *views
                            .runtime_digest_v1()
                            .expect("View registry digest")
                            .as_bytes(),
                    ),
                    Some(style_digest),
                )
                .expect("accepted presentation contract"),
            )
            .expect("valid generation declaration");
            let owner = RuntimeProgramOwner::Plan(Arc::new(plan));
            let schema = declaration
                .bind_runtime(
                    Arc::new(views),
                    asset_characters,
                    Some(style_digest),
                    owner.clone(),
                )
                .expect("generation binds to its real runtime plan and resources");
            assert_eq!(schema.generation_digest(), declaration.digest());
            let locale = LocaleTag::try_new("en").expect("valid locale");
            let base_name = CharacterDisplayNameInput::Visible(
                CharacterDisplayNameValue::try_new("Alice Display Name")
                    .expect("accepted display name"),
            );
            let display_record = CharacterDisplayNameRecordInput::try_new(
                alice.clone(),
                CharacterPresentationRole::Character,
                None,
                Some(base_name),
                Vec::new(),
                None,
            )
            .expect("accepted display-name row");
            let display_data = CharacterPresentationCatalogData::try_from_inputs(
                CharacterPresentationCatalogInput::try_new(vec![display_record])
                    .expect("accepted display catalog input"),
            )
            .expect("accepted display catalog");
            let catalog = AcceptedCharacterPresentationCatalog::publish_initial(display_data)
                .expect("accepted display generation");
            let project_locale = ProjectLocaleSpec::default();

            let template = DialogueContentFragmentTemplate::try_new_canonical(
                RuntimeDialogueContentTemplateId::from_zero_based(0).expect("template identity"),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                RichTextDocument::new(vec![RichTextNode::Text {
                    text: "Dynamic dialogue".to_owned(),
                }]),
            )
            .expect("canonical dialogue template");
            let content = RuntimeDialogueContentValue::try_new(
                RuntimeArtifactFingerprint::try_from_bytes([0x34; 32])
                    .expect("artifact fingerprint"),
                template.id(),
                template.digest(),
                [],
            )
            .expect("checked content envelope");

            Self {
                schema,
                owner,
                alice,
                bob,
                catalog,
                locale,
                project_locale,
                style_program,
                sheet_id,
                template,
                content,
            }
        }

        fn presentation(frame: LineDisplayFrame) -> BundlePresentationSnapshot {
            let activation = arcweft_core::runtime_id::DialogueActivationId::new(
                frame.content.artifact(),
                arcweft_core::runtime_id::RuntimePersistentFiberId::from_allocated(1),
                serde_json::from_value(serde_json::json!(1)).unwrap(),
                1,
            );
            let mut presentation = BundlePresentationSnapshot::default();
            presentation
                .dialogue
                .apply_operations(&[DialoguePresentationOperation::append(
                    activation,
                    crate::dialogue::DialogueViewDefinition::new(frame.effective.view().clone()),
                    frame,
                )])
                .unwrap();
            let retained = presentation.dialogue.iter().next().unwrap().entries()[0]
                .frame()
                .clone();
            presentation
                .view
                .mounts
                .push(crate::view_runtime::BundleViewMountOutput {
                    handle: arcweft_id::presentation::PresentationHandleId::try_new("capture.view")
                        .unwrap(),
                    mount: arcweft_view::ViewMountId::from_raw(1),
                    view: retained.effective.view().clone(),
                    path: crate::view_runtime::BundleViewInstancePath::default(),
                    host_axis_seed: None,
                    dialogue: None,
                    active_targets: Vec::new(),
                    active_images: Vec::new(),
                    action_buttons: Vec::new(),
                    paint: Vec::new(),
                    text: vec![
                        crate::view_runtime::BundleViewTextOutput {
                            source_id: "name".to_owned(),
                            targets: Vec::new(),
                            value: crate::view_runtime::BundleViewTextValue::CharacterDisplayName {
                                frame: Box::new(retained.clone()),
                            },
                            classification: arcweft_bundle::resource_codec::view::ViewObserveClassification::default(),
                            replacement: None,
                        },
                        crate::view_runtime::BundleViewTextOutput {
                            source_id: "content".to_owned(),
                            targets: Vec::new(),
                            value: crate::view_runtime::BundleViewTextValue::DisplayFrame {
                                frame: Box::new(retained),
                                stage_index: 0,
                            },
                            classification: arcweft_bundle::resource_codec::view::ViewObserveClassification::default(),
                            replacement: None,
                        },
                    ],
                    fx: Vec::new(),
                    events: Vec::new(),
                    style_nodes: Vec::new(),
                });
            presentation
        }

        fn spec(
            &self,
            target: CharacterPresentationTargetEvidence,
            line: &str,
        ) -> DialogueContentSpec {
            let source = SourceDocument::try_new(
                SourceDocumentId::try_new(format!("runtime-driver-{line}"))
                    .expect("source document id"),
                SourceName::Memory,
                "dialogue display fixture",
            )
            .expect("source document");
            let source = ProductSourceRef::try_for_identity(source.identity())
                .expect("product source reference");
            let checked_target =
                CheckedCharacterPresentationPlan::try_new(target, self.catalog.generation())
                    .expect("checked Character target");
            let runtime_line = format!("say.{line}");
            DialogueContentSpec::try_new(
                RuntimeLineId::from_runtime_line_value(&runtime_line)
                    .expect("runtime line identity"),
                TextKey::try_new(format!("text.{line}")).expect("text key"),
                &self.template,
                checked_target,
                DialoguePresentationSnapshot::new(
                    DialoguePresentationProfile::engine_default(),
                    profile_revision(),
                ),
                Vec::new(),
                source,
            )
            .expect("canonical dialogue content spec")
        }

        fn value(&self, character: &CharacterId, with_style: bool) -> RuntimeOpaqueValue {
            let target = RuntimeValue::EntityRef(RuntimeEntityReference::Project {
                family: DeclarationIdentityFamily::Character,
                public_id: PublicId::try_new(character.as_str()).expect("Character public id"),
            });
            let fields = if with_style {
                vec![
                    CharacterDialoguePatchField {
                        coordinate: Coordinate::Style,
                        operation: Operation::Set(RuntimeValue::EntityRef(
                            RuntimeEntityReference::Project {
                                family: DeclarationIdentityFamily::Style,
                                public_id: PublicId::try_new(self.sheet_id.as_str())
                                    .expect("Style public id"),
                            },
                        )),
                    },
                    CharacterDialoguePatchField {
                        coordinate: Coordinate::RichText,
                        operation: Operation::Set(self.rich_text_color_value()),
                    },
                ]
            } else {
                Vec::new()
            };
            self.schema
                .construct(
                    &self.owner,
                    &target,
                    &fields,
                    CharacterDialogueType::any().runtime_semantic_identity(),
                )
                .expect("accepted CharacterDialogue value")
                .opaque()
                .clone()
        }

        fn rich_text_color_value(&self) -> RuntimeValue {
            let codec = CharacterDialogueRolePayloadCodec::RichTextProperties;
            let mut payload = codec.no_overrides_payload().expect("typed empty RichText");
            let color_slot = PRESENTATION_CONTENT_CALLABLE_CATALOG
                .reusable_style_parameters()
                .position(|(definition, parameter)| {
                    definition == Definition::Color && parameter.id == Parameter::Value
                })
                .expect("catalog-owned Color/Value slot");
            let RuntimeValue::Tuple(slots) = &mut payload else {
                panic!("sparse RichText policy has a typed tuple root");
            };
            slots[color_slot] = RuntimeValue::option_some(RuntimeValue::Tuple(vec![
                RuntimeValue::UInt(RuntimeUInt::U8(17)),
                RuntimeValue::UInt(RuntimeUInt::U8(34)),
                RuntimeValue::UInt(RuntimeUInt::U8(51)),
                RuntimeValue::UInt(RuntimeUInt::U8(255)),
            ]));
            RuntimeOpaqueTypeOwner::exact(
                CharacterDialogueRuntimeSchema::opaque_type_producer(),
                role_semantic(Role::RichText),
            )
            .try_wrap(payload)
            .expect("accepted RichText role opaque value")
        }

        fn frame(
            &self,
            spec: &DialogueContentSpec,
            target: &RuntimeOpaqueValue,
        ) -> LineDisplayFrame {
            let provider = CatalogDialogueRuntimeContextProvider::new(
                &self.catalog,
                &self.locale,
                &self.project_locale,
                Some(&self.schema),
                Some(&self.style_program),
            );
            let context = provider
                .context_for(spec, target, &[])
                .expect("accepted dynamic presentation context");
            resolve_frame_with_template(spec, &self.template, &self.content, &context)
                .expect("frame resolves with admitted content and context")
        }

        fn rejected(
            &self,
            spec: &DialogueContentSpec,
            target: &RuntimeOpaqueValue,
        ) -> DialogueRuntimeContextError {
            let provider = CatalogDialogueRuntimeContextProvider::new(
                &self.catalog,
                &self.locale,
                &self.project_locale,
                Some(&self.schema),
                Some(&self.style_program),
            );
            provider
                .context_for(spec, target, &[])
                .expect_err("invalid dynamic CharacterDialogue evidence rejects")
        }
    }

    fn profile_revision() -> DialogueProfileRevision {
        let manifest = SourceDocument::try_new(
            SourceDocumentId::try_new("runtime-driver-dialogue-display-profile")
                .expect("manifest id"),
            SourceName::Memory,
            "schema = 1\n",
        )
        .expect("profile manifest");
        let sources = SourceSetRevision::try_for_identities([manifest.identity()])
            .expect("profile source revision");
        DialogueProfileRevision::from_admitted_parts(
            manifest.identity().clone(),
            sources,
            sources,
            ViewProgramId::try_new("view_program.dialogue_display_fixture")
                .expect("View program identity"),
            AcceptedViewProgramRevision::try_from_bytes([0x46; 32]).expect("View program revision"),
            ResourceTypeRegistry::empty().digest(),
        )
    }

    #[test]
    fn fallback_style_capture_uses_original_role_schema_and_one_config_wire() {
        use arcweft_dialogue::{
            CharacterDialogueStyleValue, CharacterDialogueTypedValue, FallbackStylePolicy,
            InlineFailurePolicy, InlineFallback, InlineTextFailure,
        };
        let fixture = Fixture::new();
        let spec = fixture.spec(
            CharacterPresentationTargetEvidence::RuntimeCharacterDialogue {
                generation: fixture.schema.generation_digest(),
            },
            "fallback_capture",
        );
        let target = fixture.value(&fixture.alice, true);
        let mut frame = fixture.frame(&spec, &target);
        let style = frame
            .effective
            .admitted()
            .unwrap()
            .dialogue()
            .config()
            .style()
            .clone();
        let policy = |style| InlineFailurePolicy::Fallback {
            fallback: InlineFallback::Text {
                text: "fallback".to_owned(),
                style: FallbackStylePolicy::Apply {
                    styles: vec![style],
                },
            },
        };
        frame.inline_failures.push(InlineTextFailure {
            expr: "capture".to_owned(),
            reason: "fixture".to_owned(),
            policy: policy(style.clone()),
        });
        let valid = arcweft_text_model::LineDisplayFrameSnapshot::from_live(
            &frame,
            Some(&fixture.owner),
            Some(&fixture.schema),
        )
        .unwrap();
        assert_eq!(
            valid
                .clone()
                .into_live(Some(&fixture.owner), Some(&fixture.schema))
                .unwrap(),
            frame
        );
        let bytes = serde_json::to_vec(&valid).unwrap();
        let config = arcweft_text_model::CharacterDialoguePresentationConfig::standalone(
            frame.effective.view().clone(),
            policy(style),
        );
        let config_wire =
            arcweft_text_model::CharacterDialoguePresentationConfigSnapshot::from_live(
                &config,
                Some(&fixture.owner),
                Some(&fixture.schema),
            )
            .unwrap();
        assert_eq!(
            serde_json::to_value(&config).unwrap(),
            serde_json::to_value(&config_wire).unwrap()
        );
        assert_eq!(
            config_wire
                .into_live(Some(&fixture.owner), Some(&fixture.schema))
                .unwrap(),
            config
        );
        let invalid_style = CharacterDialogueStyleValue::try_new(
            CharacterDialogueTypedValue::try_new(RuntimeValue::Bool(true)).unwrap(),
        )
        .unwrap();
        frame.inline_failures[0].policy = policy(invalid_style.clone());
        assert!(
            arcweft_text_model::LineDisplayFrameSnapshot::from_live(
                &frame,
                Some(&fixture.owner),
                Some(&fixture.schema),
            )
            .is_err()
        );
        let invalid_config = arcweft_text_model::CharacterDialoguePresentationConfig::standalone(
            frame.effective.view().clone(),
            policy(invalid_style),
        );
        assert!(
            arcweft_text_model::CharacterDialoguePresentationConfigSnapshot::from_live(
                &invalid_config,
                Some(&fixture.owner),
                Some(&fixture.schema),
            )
            .is_err()
        );
        assert_eq!(serde_json::to_vec(&valid).unwrap(), bytes);
    }

    #[test]
    fn presentation_capture_admission_rejects_foreign_lease_and_standalone_downgrade() {
        let fixture = Fixture::new();
        let foreign = Fixture::new();
        let spec = fixture.spec(
            CharacterPresentationTargetEvidence::RuntimeCharacterDialogue {
                generation: fixture.schema.generation_digest(),
            },
            "capture_admission",
        );
        let target = fixture.value(&fixture.alice, true);
        let frame = fixture.frame(&spec, &target);
        let presentation = Fixture::presentation(frame);
        let mut visited = 0;
        presentation
            .try_visit_frames(|_| {
                visited += 1;
                Ok::<_, String>(())
            })
            .unwrap();
        assert_eq!(visited, 3);
        presentation
            .validate_runtime_context(&fixture.owner, Some(&fixture.schema))
            .unwrap();
        let before = serde_json::to_vec(&presentation).unwrap();
        assert!(
            presentation
                .validate_runtime_context(&foreign.owner, Some(&foreign.schema))
                .is_err()
        );
        assert_eq!(serde_json::to_vec(&presentation).unwrap(), before);
        let mut view_only = presentation.clone();
        view_only.dialogue = DialoguePresentationStore::default();
        assert!(
            view_only
                .validate_runtime_context(&foreign.owner, Some(&foreign.schema))
                .is_err()
        );
        view_only
            .validate_runtime_context(&fixture.owner, Some(&fixture.schema))
            .unwrap();
        let mut downgraded = presentation
            .clone()
            .try_map_frames(|mut frame| {
                frame.effective =
                    arcweft_text_model::CharacterDialoguePresentationConfig::standalone(
                        frame.effective.view().clone(),
                        arcweft_dialogue::InlineFailurePolicy::FailLine,
                    );
                Ok::<_, String>(frame)
            })
            .unwrap();
        assert!(
            downgraded
                .validate_runtime_context(&fixture.owner, Some(&fixture.schema))
                .is_err()
        );
        // A renderer-only standalone frame remains legitimate without a
        // CharacterDialogue generation schema.
        downgraded
            .validate_runtime_context(&fixture.owner, None)
            .unwrap();
        downgraded.revision = 1;
        assert_eq!(serde_json::to_vec(&presentation).unwrap(), before);
    }

    #[test]
    fn presentation_frames_use_one_value_allowance_across_dialogue_and_view() {
        use arcweft_core::{entry::RuntimeSchemaLimits, value::RuntimeRetainedValueAdmission};
        let fixture = Fixture::new();
        let spec = fixture.spec(
            CharacterPresentationTargetEvidence::RuntimeCharacterDialogue {
                generation: fixture.schema.generation_digest(),
            },
            "group_admission",
        );
        let target = fixture.value(&fixture.alice, true);
        let mut frame = fixture.frame(&spec, &target);
        frame.effective = arcweft_text_model::CharacterDialoguePresentationConfig::standalone(
            frame.effective.view().clone(),
            arcweft_dialogue::InlineFailurePolicy::FailLine,
        );
        let limits = RuntimeSchemaLimits {
            max_nodes: 200,
            ..RuntimeSchemaLimits::engine_default()
        };
        let mut single = RuntimeRetainedValueAdmission::for_program(fixture.owner.clone(), limits);
        frame.admit_retained_metadata(&mut single).unwrap();
        frame.admit_retained_values(&mut single, None).unwrap();
        let presentation = Fixture::presentation(frame);
        let mut group = RuntimeRetainedValueAdmission::for_program(fixture.owner.clone(), limits);
        assert!(presentation.admit_retained(&mut group, None).is_err());
        assert!(
            group
                .validate(&arcweft_core::value::RuntimeValue::Unit)
                .is_err()
        );
        let mut inert = RuntimeRetainedValueAdmission::for_program(fixture.owner.clone(), limits);
        let snapshot = presentation
            .clone()
            .try_map_frames(|frame| {
                arcweft_text_model::LineDisplayFrameSnapshot::from_live(
                    &frame,
                    Some(&fixture.owner),
                    None,
                )
            })
            .unwrap();
        snapshot
            .try_visit_frames(|frame| frame.admit_retained_metadata(&mut inert))
            .unwrap();
        assert!(
            snapshot
                .try_visit_frames(|frame| frame.admit_retained_values(&mut inert, None))
                .is_err()
        );
    }

    #[test]
    fn inert_display_frame_retains_original_roles_and_rejects_forged_configuration() {
        let fixture = Fixture::new();
        let spec = fixture.spec(
            CharacterPresentationTargetEvidence::RuntimeCharacterDialogue {
                generation: fixture.schema.generation_digest(),
            },
            "inert_configuration",
        );
        let target = fixture.value(&fixture.alice, true);
        let frame = fixture.frame(&spec, &target);
        let original = frame.effective.admitted().unwrap();
        assert_eq!(original.opaque(), &target);
        let snapshot = arcweft_text_model::LineDisplayFrameSnapshot::from_live(
            &frame,
            Some(&fixture.owner),
            Some(&fixture.schema),
        )
        .unwrap();
        let bytes = serde_json::to_vec(&snapshot).unwrap();
        let decoded: arcweft_text_model::LineDisplayFrameSnapshot =
            serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            decoded
                .into_live(Some(&fixture.owner), Some(&fixture.schema))
                .unwrap(),
            frame
        );

        for defect in 0..5 {
            let mut candidate = serde_json::to_value(&snapshot).unwrap();
            match defect {
                0 => {
                    candidate["effective"]["generation"] =
                        serde_json::to_value(RuntimeValueDigest::ZERO).unwrap();
                }
                1 => {
                    candidate["effective"]["config_digest"] =
                        serde_json::to_value(RuntimeValueDigest::ZERO).unwrap()
                }
                2 => candidate["effective"]["style_sheet"] = serde_json::Value::Null,
                3 => candidate["effective"]["value"] = serde_json::json!({"Bool": true}),
                4 => candidate["character"]["id"] = serde_json::to_value(&fixture.bob).unwrap(),
                _ => unreachable!(),
            }
            let candidate: arcweft_text_model::LineDisplayFrameSnapshot =
                serde_json::from_value(candidate).unwrap();
            assert!(
                candidate
                    .into_live(Some(&fixture.owner), Some(&fixture.schema))
                    .is_err(),
                "forged configuration {defect}"
            );
            assert_eq!(serde_json::to_vec(&snapshot).unwrap(), bytes);
            assert_eq!(frame.effective.admitted().unwrap().opaque(), &target);
        }
        assert!(
            snapshot
                .clone()
                .into_live(Some(&fixture.owner), None)
                .is_err()
        );
        assert!(snapshot.into_live(None, Some(&fixture.schema)).is_err());
    }

    #[test]
    fn dynamic_context_uses_admitted_config_and_rejects_generation_or_character_mismatch() {
        let fixture = Fixture::new();
        let dynamic = |generation| CharacterPresentationTargetEvidence::RuntimeCharacterDialogue {
            generation,
        };
        let spec = fixture.spec(
            dynamic(fixture.schema.generation_digest()),
            "dynamic_context_success",
        );
        let accepted = fixture.value(&fixture.alice, true);
        let frame = fixture.frame(&spec, &accepted);
        assert_eq!(frame.character.id, fixture.alice);
        assert_eq!(frame.character.display_name, "Alice Display Name");
        assert_eq!(
            frame.effective.view().clone(),
            DialoguePresentationProfile::engine_default().view().clone()
        );
        assert_eq!(frame.effective.style_sheet(), Some(&fixture.sheet_id));
        assert_ne!(frame.effective.config_digest(), RuntimeValueDigest::ZERO);
        assert!(matches!(
            frame.base_styles.as_slice(),
            [RichTextStyle::Color {
                value: RichTextColor::Rgba8 {
                    value: [17, 34, 51, 255]
                }
            }]
        ));

        let stale_generation = fixture.spec(
            dynamic(RuntimeValueDigest::from_bytes([0x99; 32])),
            "dynamic_context_stale_generation",
        );
        assert!(matches!(
            fixture.rejected(&stale_generation, &accepted),
            DialogueRuntimeContextError::Rejected { reason, .. }
                if reason.contains("generation does not match")
        ));

        let absent_character = fixture.spec(
            dynamic(fixture.schema.generation_digest()),
            "dynamic_context_unlisted_character",
        );
        let bob = fixture.value(&fixture.bob, false);
        assert!(matches!(
            fixture.rejected(&absent_character, &bob),
            DialogueRuntimeContextError::Rejected { .. }
        ));

        let exact_alice = fixture.spec(
            CharacterPresentationTargetEvidence::Exact(fixture.alice.clone()),
            "dynamic_context_checked_character_mismatch",
        );
        assert!(matches!(
            fixture.rejected(&exact_alice, &bob),
            DialogueRuntimeContextError::Rejected { reason, .. }
                if reason == "dialogue target does not match the checked Character"
        ));
    }
}
