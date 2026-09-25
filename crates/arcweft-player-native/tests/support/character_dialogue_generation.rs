//! Typed `CharacterDialogue` generation shared by player-native fixtures.

use arcweft_bundle::{
    ArcweftBundle,
    resource_codec::{ValidatedViewProduct, ViewProductValidationLimits},
};
use arcweft_character::id::CharacterId;
use arcweft_core::{
    entry::{
        RuntimeNominalSchemaBody, RuntimeNominalSchemaCase, RuntimeNominalSchemaDefinition,
        RuntimeNominalSchemaGraph, RuntimeNominalSchemaIdentity, RuntimeNominalTypeId,
        RuntimeSchemaLimits, RuntimeValueDigest,
    },
    pattern::{RuntimeOpaqueTypeOwner, RuntimeSemanticTypeId},
    plan::{
        RuntimeExprSeed, RuntimeExprSeedKind, RuntimePlanBuilder, RuntimePlanTypeProjection,
        RuntimePlanTypeSeed, RuntimeVariantCaseSeed, RuntimeVariantDomainSeed,
    },
    value::RuntimeEntityReference,
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
};
use arcweft_id::{DeclarationIdentityFamily, PublicId};
use arcweft_runtime_driver::view_runtime::BundleViewRuntime;
use std::sync::Arc;

fn semantic(tag: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([tag; 32])
}

fn voice_type() -> RuntimeSemanticTypeId {
    semantic(56)
}

fn voice_nominal() -> RuntimeNominalTypeId {
    RuntimeNominalTypeId::try_new("DialogueVoice").expect("fixture nominal identity is valid")
}

fn role_types() -> CharacterDialogueRuntimeRoleTypes {
    CharacterDialogueRuntimeRoleTypes::new(
        Role::AUTHORED_BASE.map(|role| {
            let body = if role == Role::RichText {
                let payload = CharacterDialogueRolePayloadCodec::RichTextProperties
                    .payload_schema()
                    .expect("RichText payload schema");
                CharacterDialogueRuntimeRoleBody::bound(
                    payload.root(),
                    CharacterDialogueRolePayloadCodec::RichTextProperties,
                )
            } else {
                CharacterDialogueRuntimeRoleBody::Unbound
            };
            CharacterDialogueRuntimeRoleType::new(semantic(20 + role.canonical_tag()), body)
        }),
        semantic(50),
    )
}

/// Produce the generation-owned Dialogue value through the typed factory
/// operation instead of embedding a fabricated opaque constant in AWBC.
pub(crate) fn factory_expression(character: &CharacterId) -> RuntimeExprSeed {
    let dialogue_type = CharacterDialogueType::exact(character.clone());
    RuntimeExprSeed::new(
        dialogue_type.runtime_semantic_identity(),
        RuntimeExprSeedKind::CharacterDialogue {
            operation: arcweft_interaction_model::dialogue::CharacterDialogueOperation::Factory,
            target: Box::new(RuntimeExprSeed::new(
                semantic(51),
                RuntimeExprSeedKind::EntityRef(RuntimeEntityReference::Project {
                    family: DeclarationIdentityFamily::Character,
                    public_id: PublicId::try_new(character.as_str())
                        .expect("fixture character public ID"),
                }),
            )),
            fields: Box::default(),
        },
    )
}

/// Admit the complete runtime type graph required by the accepted
/// `CharacterDialogue` generation declaration before lowering the fixture AWBC.
pub(crate) fn admit_generation_types(builder: &mut RuntimePlanBuilder, character: &CharacterId) {
    let exact = CharacterDialogueType::exact(character.clone());
    let any = CharacterDialogueType::any();
    let exact_owner = exact.runtime_opaque_owner();
    let any_owner = any.runtime_opaque_owner();
    let producer = CharacterDialogueRuntimeSchema::opaque_type_producer();
    let voice_graph = RuntimeNominalSchemaGraph::try_new(
        vec![RuntimeNominalSchemaDefinition::new(
            RuntimeNominalSchemaIdentity::new(voice_nominal(), voice_type()),
            vec![],
            RuntimeNominalSchemaBody::Variant {
                cases: vec![RuntimeNominalSchemaCase::new(0, "auto".to_owned(), None)]
                    .into_boxed_slice(),
            },
        )],
        RuntimeSchemaLimits::engine_default(),
    )
    .expect("Voice schema graph");
    let rich_text_identity = semantic(20 + Role::RichText.canonical_tag());
    let policy_graph = CharacterDialoguePolicyTypeGraph::try_new(
        RuntimeOpaqueTypeOwner::exact(producer.clone(), rich_text_identity),
        RuntimeSchemaLimits::engine_default(),
    )
    .expect("CharacterDialogue policy graph");
    let graph = RuntimeNominalSchemaGraph::try_merge(
        [&voice_graph, policy_graph.schema_graph().as_ref()],
        RuntimeSchemaLimits::engine_default(),
    )
    .expect("Voice and policy schema graphs merge");
    let voice_layout = graph.try_layout_hash(voice_type()).expect("Voice layout");
    let mut types = Role::AUTHORED_BASE
        .into_iter()
        .map(|role| {
            let identity = semantic(20 + role.canonical_tag());
            let owner = RuntimeOpaqueTypeOwner::exact(producer.clone(), identity);
            RuntimePlanTypeSeed::new(
                identity,
                RuntimePlanTypeProjection::Opaque {
                    producer: owner.producer().clone(),
                    admission: owner.admission(),
                    value_class: owner.value_class(),
                    persistence: owner.persistence(),
                    arguments: Box::default(),
                },
            )
        })
        .collect::<Vec<_>>();
    types.extend(
        CharacterDialogueRolePayloadCodec::RichTextProperties
            .payload_schema()
            .expect("RichText payload schema")
            .types()
            .iter()
            .cloned(),
    );
    types.extend(policy_graph.type_seeds().into_vec());
    types.extend([
        RuntimePlanTypeSeed::new(
            semantic(50),
            RuntimePlanTypeProjection::Choice(
                vec![semantic(51), semantic(20 + Role::RichText.canonical_tag())].into(),
            ),
        ),
        RuntimePlanTypeSeed::new(semantic(51), RuntimePlanTypeProjection::EntityReference),
        RuntimePlanTypeSeed::new(
            exact.runtime_semantic_identity(),
            RuntimePlanTypeProjection::Opaque {
                producer: exact_owner.producer().clone(),
                admission: exact_owner.admission(),
                value_class: exact_owner.value_class(),
                persistence: exact_owner.persistence(),
                arguments: Box::default(),
            },
        ),
        RuntimePlanTypeSeed::new(
            any.runtime_semantic_identity(),
            RuntimePlanTypeProjection::Opaque {
                producer: any_owner.producer().clone(),
                admission: any_owner.admission(),
                value_class: any_owner.value_class(),
                persistence: any_owner.persistence(),
                arguments: Box::default(),
            },
        ),
        RuntimePlanTypeSeed::new(
            voice_type(),
            RuntimePlanTypeProjection::Nominal {
                nominal: voice_nominal(),
                layout: voice_layout,
                arguments: Box::default(),
            },
        ),
    ]);
    let mut variant_domains = vec![RuntimeVariantDomainSeed::new(
        voice_type(),
        voice_nominal(),
        voice_layout,
        [RuntimeVariantCaseSeed::new("auto", None)],
    )];
    variant_domains.extend(policy_graph.variant_domain_seeds());
    builder
        .admit_semantic_batch(types, [], [], variant_domains, &graph)
        .expect("CharacterDialogue runtime types admit");
}

/// Attach the typed declaration to a fixture bundle using the exact view
/// registry and style resources that the runtime will accept.
pub(crate) fn with_generation(
    bundle: ArcweftBundle,
    character: CharacterId,
    revision: DialogueProfileRevision,
) -> ArcweftBundle {
    let product = ValidatedViewProduct::try_new(
        Some(bundle.source_map.clone()),
        bundle.view_program.clone(),
        bundle.view_style.clone(),
        ViewProductValidationLimits::default(),
    )
    .expect("fixture View product validates");
    let view_runtime = BundleViewRuntime::try_new_with_awbc(
        product,
        bundle.view_text.clone(),
        Arc::new(bundle.product_awbc.program.clone()),
    )
    .expect("fixture View registry joins AWBC");
    let view_fingerprint = RuntimeValueDigest::from_bytes(
        *view_runtime
            .registry()
            .runtime_digest_v1()
            .expect("fixture View registry digest")
            .as_bytes(),
    );
    let style_fingerprint = bundle.view_style.as_ref().map(|style| {
        style
            .canonical_digest()
            .map(|digest| RuntimeValueDigest::from_bytes(digest.as_bytes()))
            .expect("fixture style digest")
    });
    let profile = DialoguePresentationProfile::engine_default();
    let presentation = CharacterDialoguePresentationContract::try_new(
        profile.clone(),
        revision,
        view_fingerprint,
        style_fingerprint,
    )
    .expect("fixture presentation contract");
    let config = CharacterDialogueConfig::try_from_presentation_profile(&profile, &role_types())
        .expect("fixture CharacterDialogue defaults");
    let exact = CharacterDialogueType::exact(character.clone());
    let declaration = CharacterDialogueGenerationDeclaration::try_new(
        [(
            character.clone(),
            CharacterDialogueCharacterDeclaration::new(
                exact.runtime_semantic_identity(),
                CharacterDialogueVisualType::Absent,
                CharacterDialogueRuntimeDefault::new(character, config),
            ),
        )],
        CharacterDialogueType::any().runtime_semantic_identity(),
        voice_type(),
        role_types(),
        CharacterDialogueRuntimeCustomFieldCatalog::try_new([])
            .expect("empty fixture custom field catalog"),
        presentation,
    )
    .expect("fixture generation declaration");
    bundle.with_character_dialogue_generation(declaration)
}
