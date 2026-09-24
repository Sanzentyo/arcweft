//! Accepted Character, resource and source-type inputs for one producer generation.

use arcweft_character::catalog::{CharacterCatalog, CharacterVisualManifestEvidence};
use arcweft_core::entry::RuntimeValueDigest;
use arcweft_dialogue::{
    CharacterDialogueCharacterDeclaration, CharacterDialogueConfig,
    CharacterDialogueGenerationDeclaration, CharacterDialoguePresentationContract,
    CharacterDialogueRolePayloadCodec, CharacterDialogueRuntimeCustomFieldCatalog,
    CharacterDialogueRuntimeCustomFieldDescriptor, CharacterDialogueRuntimeDefault,
    CharacterDialogueRuntimeRoleBody, CharacterDialogueRuntimeRoleType,
    CharacterDialogueRuntimeRoleTypes, CharacterDialogueType, CharacterDialogueVisualType,
};
use arcweft_interaction_model::dialogue::CharacterDialogueRuntimeRole;
use arcweft_view::ViewRegistry;

use super::{
    Arc, BTreeSet, CharacterId, CheckedDialogueProfile, FinalSemanticAnalysis,
    HirAnalysisProjectView, HirItemKind, ProjectSymbolTable, RegisteredSemanticWorld,
    RuntimeNormalizedType, RuntimeSemanticProjectionError, TypeKind, runtime_type,
};

/// Joins accepted source authorities with domain-owned role bodies and defaults.
pub(in crate::lower) fn project_generation(
    project: HirAnalysisProjectView<'_>,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
    profile: &CheckedDialogueProfile,
) -> Result<
    Arc<CharacterDialogueGenerationDeclaration<RuntimeNormalizedType>>,
    RuntimeSemanticProjectionError,
> {
    let roles = project_roles(symbols, world, analysis)?;
    let effective_default =
        CharacterDialogueConfig::try_from_presentation_profile(profile.presentation(), &roles)
            .map_err(invalid)?;
    let catalog = project_character_catalog(project, world)?;
    let mut characters = Vec::new();
    for character in catalog.characters() {
        let dialogue_type = runtime_type(
            &TypeKind::CharacterDialogue(CharacterDialogueType::exact(character.clone())),
            symbols,
            world,
            analysis,
        )?;
        let visual = match catalog.visual_manifest(character) {
            None => CharacterDialogueVisualType::Absent,
            Some(manifest) => CharacterDialogueVisualType::Present {
                manifest: RuntimeValueDigest::from_bytes(
                    *manifest.semantic_fingerprint_v1().as_bytes(),
                ),
                look_type: runtime_type(
                    &TypeKind::character_look(character.clone()),
                    symbols,
                    world,
                    analysis,
                )?,
            },
        };
        let defaults =
            CharacterDialogueRuntimeDefault::new(character.clone(), effective_default.clone());
        characters.push((
            character.clone(),
            CharacterDialogueCharacterDeclaration::new(dialogue_type, visual, defaults),
        ));
    }
    let any_dialogue = runtime_type(
        &TypeKind::CharacterDialogue(CharacterDialogueType::any()),
        symbols,
        world,
        analysis,
    )?;
    let voice = runtime_type(
        &TypeKind::Named("DialogueVoice".to_owned()),
        symbols,
        world,
        analysis,
    )?;
    let custom_fields = world
        .environment()
        .character_dialogue_fields()
        .descriptors()
        .map(|field| {
            Ok(CharacterDialogueRuntimeCustomFieldDescriptor::new(
                field.id().clone(),
                runtime_type(field.value_type(), symbols, world, analysis)?,
                field.clearable(),
                field.accepted_views().clone(),
            ))
        })
        .collect::<Result<Vec<_>, RuntimeSemanticProjectionError>>()?;
    let custom_fields =
        CharacterDialogueRuntimeCustomFieldCatalog::try_new(custom_fields).map_err(invalid)?;
    CharacterDialogueGenerationDeclaration::try_new(
        characters,
        any_dialogue,
        voice,
        roles,
        custom_fields,
        presentation_contract(profile)?,
    )
    .map(Arc::new)
    .map_err(invalid)
}

fn project_roles(
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
) -> Result<CharacterDialogueRuntimeRoleTypes<RuntimeNormalizedType>, RuntimeSemanticProjectionError>
{
    let source = world.environment().character_dialogue_roles();
    let authored = CharacterDialogueRuntimeRole::AUTHORED_BASE
        .into_iter()
        .map(|role| {
            let value = runtime_type(source.semantic_type(role), symbols, world, analysis)?;
            let body = if role == CharacterDialogueRuntimeRole::RichText {
                let codec = CharacterDialogueRolePayloadCodec::RichTextProperties;
                let schema = codec.payload_schema().map_err(invalid)?;
                CharacterDialogueRuntimeRoleBody::bound(
                    RuntimeNormalizedType::try_from_character_dialogue_payload(schema)?,
                    codec,
                )
            } else {
                CharacterDialogueRuntimeRoleBody::unbound()
            };
            Ok(CharacterDialogueRuntimeRoleType::new(value, body))
        })
        .collect::<Result<Vec<_>, RuntimeSemanticProjectionError>>()?;
    let authored = authored
        .try_into()
        .expect("AUTHORED_BASE owns exactly six role declarations");
    let style = runtime_type(
        source.semantic_type(CharacterDialogueRuntimeRole::Style),
        symbols,
        world,
        analysis,
    )?;
    Ok(CharacterDialogueRuntimeRoleTypes::new(authored, style))
}

/// Joins every logical declaration to its explicit optional visual resource.
/// The same construction supplies generation projection and direct-source
/// runtime binding, so neither consumer invents its own membership policy.
pub(crate) fn project_character_catalog(
    project: HirAnalysisProjectView<'_>,
    world: &RegisteredSemanticWorld,
) -> Result<Arc<CharacterCatalog>, RuntimeSemanticProjectionError> {
    let environment = world.environment();
    environment
        .verify_character_inventory(world.symbols())
        .map_err(invalid)?;
    let mut characters = BTreeSet::new();
    for item in project.items() {
        let HirItemKind::Character(character) = item.item().kind() else {
            continue;
        };
        let public_id = character
            .header()
            .public_id()
            .resolved()
            .ok_or_else(|| invalid("a logical Character has no accepted public identity"))?;
        let character = CharacterId::try_new(public_id.as_str()).map_err(invalid)?;
        if !characters.insert(character) {
            return Err(invalid(
                "the accepted HIR repeats a logical Character identity",
            ));
        }
    }
    characters.extend(
        environment
            .character_inventory()
            .external_characters()
            .map(|(_, character)| character.clone()),
    );
    let declarations = characters
        .into_iter()
        .map(|character| {
            let visual = world
                .environment()
                .character_manifest(&character)
                .map_or(CharacterVisualManifestEvidence::Absent, |manifest| {
                    CharacterVisualManifestEvidence::Present(manifest.clone())
                });
            (character, visual)
        })
        .collect::<Vec<_>>();
    CharacterCatalog::try_from_declarations(declarations)
        .map(Arc::new)
        .map_err(invalid)
}

fn presentation_contract(
    profile: &CheckedDialogueProfile,
) -> Result<CharacterDialoguePresentationContract, RuntimeSemanticProjectionError> {
    let product = profile.product();
    let program = product
        .program()
        .ok_or_else(|| invalid("the admitted profile has no accepted View program"))?;
    let mut registry = ViewRegistry::default();
    program
        .register_runtime_views(&mut registry)
        .map_err(invalid)?;
    let views = registry.runtime_digest_v1().map_err(invalid)?;
    let style = product
        .style()
        .map(|style| {
            style
                .resource()
                .canonical_digest()
                .map(|digest| RuntimeValueDigest::from_bytes(digest.as_bytes()))
                .map_err(invalid)
        })
        .transpose()?;
    CharacterDialoguePresentationContract::try_new(
        profile.presentation().clone(),
        profile.revision().clone(),
        RuntimeValueDigest::from_bytes(*views.as_bytes()),
        style,
    )
    .map_err(invalid)
}

fn invalid(reason: impl std::fmt::Display) -> RuntimeSemanticProjectionError {
    RuntimeSemanticProjectionError::Dialogue {
        owner: None,
        reason: reason.to_string(),
    }
}
