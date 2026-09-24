//! Complete generation inputs and their executable type roots.

use std::{collections::BTreeMap, sync::Arc};

use arcweft_character::{id::CharacterId, presentation_name::CharacterPresentationCatalogData};
use arcweft_core::entry::RuntimeValueDigest;
use arcweft_dialogue::character_presentation::CharacterPresentationTargetEvidence;
use arcweft_dialogue::{
    CharacterDialogueGenerationDeclaration, CharacterDialogueTypeReference,
    CharacterDialogueVisualType,
};
use arcweft_lang_hir::{
    identity::{ExprId, HirModuleId},
    item::HirItemKind,
    module::HirModule,
    project::HirAnalysisProjectView,
};
use arcweft_lang_sema::registration::CharacterInventoryDescriptorV1;

use super::{
    RuntimeDialogueApplication, RuntimeNormalizedType, RuntimePlanSemanticFactInput,
    RuntimePlanSemanticFacts, RuntimeSemanticFactFamily, RuntimeSemanticFactsError,
    RuntimeSemanticTypeId, validate_normalized_type,
};

mod payload;

impl CharacterDialogueTypeReference for RuntimeNormalizedType {
    fn semantic_identity(&self) -> RuntimeSemanticTypeId {
        self.identity()
    }
}

/// Keeps the source-issued inventory with the declaration until the retained
/// and external logical membership has been checked against the accepted HIR.
#[derive(Clone, Debug)]
pub(super) struct RuntimeCharacterDialogueGenerationFact {
    pub(super) declaration: Arc<CharacterDialogueGenerationDeclaration<RuntimeNormalizedType>>,
    inventory: CharacterInventoryDescriptorV1,
}

impl RuntimePlanSemanticFactInput {
    /// Stages the producer's complete input contract independently of whether
    /// any dialogue line or factory expression is reachable.
    pub fn attach_character_dialogue_generation(
        &mut self,
        declaration: Arc<CharacterDialogueGenerationDeclaration<RuntimeNormalizedType>>,
        inventory: CharacterInventoryDescriptorV1,
    ) -> Result<(), RuntimeSemanticFactsError> {
        if self.character_dialogue_generation.is_some() {
            return Err(RuntimeSemanticFactsError::DuplicateFact {
                family: RuntimeSemanticFactFamily::CharacterDialogueGeneration,
            });
        }
        self.character_dialogue_generation = Some(RuntimeCharacterDialogueGenerationFact {
            declaration,
            inventory,
        });
        Ok(())
    }
}

impl RuntimePlanSemanticFacts {
    /// Generation-wide producer inputs from the exact accepted compiler world.
    /// Their types also participate in the plan's common recursive type batch.
    pub fn character_dialogue_generation(
        &self,
    ) -> Option<&Arc<CharacterDialogueGenerationDeclaration<RuntimeNormalizedType>>> {
        self.character_dialogue_generation
            .as_ref()
            .map(|fact| &fact.declaration)
    }
}

pub(super) fn validate_declaration(
    project: HirAnalysisProjectView<'_>,
    modules: &BTreeMap<HirModuleId, &HirModule>,
    fact: &RuntimeCharacterDialogueGenerationFact,
) -> Result<(), RuntimeSemanticFactsError> {
    let mut characters = std::collections::BTreeSet::new();
    for item in project.items() {
        let HirItemKind::Character(character) = item.item().kind() else {
            continue;
        };
        let public_id = character.header().public_id().resolved().ok_or(
            RuntimeSemanticFactsError::InvalidCharacterDialogueGeneration {
                reason: "a Character declaration has no accepted identity",
            },
        )?;
        let character = CharacterId::try_new(public_id.as_str()).map_err(|_| {
            RuntimeSemanticFactsError::InvalidCharacterDialogueGeneration {
                reason: "a Character declaration has an invalid public identity",
            }
        })?;
        if !characters.insert(character) {
            return Err(
                RuntimeSemanticFactsError::InvalidCharacterDialogueGeneration {
                    reason: "the accepted HIR repeats a logical Character identity",
                },
            );
        }
    }
    characters.extend(
        fact.inventory
            .external_characters()
            .map(|(_, character)| character.clone()),
    );
    let declaration = &fact.declaration;
    if !characters.iter().eq(declaration.characters().keys()) {
        return Err(
            RuntimeSemanticFactsError::InvalidCharacterDialogueGeneration {
                reason: "the generation contract does not contain every logical Character exactly once",
            },
        );
    }
    let manifests = fact
        .inventory
        .characters()
        .iter()
        .map(|(character, fingerprint)| {
            (
                character,
                RuntimeValueDigest::from_bytes(*fingerprint.as_bytes()),
            )
        })
        .collect::<BTreeMap<_, _>>();
    for (character, row) in declaration.characters() {
        let actual = match row.visual() {
            CharacterDialogueVisualType::Absent => None,
            CharacterDialogueVisualType::Present { manifest, .. } => Some(manifest),
        };
        if actual != manifests.get(character) {
            return Err(
                RuntimeSemanticFactsError::InvalidCharacterDialogueGeneration {
                    reason: "a Character visual contract differs from the accepted source inventory",
                },
            );
        }
    }
    let mut roots = Vec::new();
    declaration.visit_type_refs(&mut |ty| roots.push(ty));
    for ty in roots {
        validate_normalized_type(modules, ty)?;
    }
    Ok(())
}

pub(super) fn validate_application(
    owner: ExprId,
    application: &RuntimeDialogueApplication,
    catalog: &CharacterPresentationCatalogData,
    declaration: Option<&CharacterDialogueGenerationDeclaration<RuntimeNormalizedType>>,
) -> Result<(), RuntimeSemanticFactsError> {
    let invalid = || RuntimeSemanticFactsError::DialogueCharacterPlanMismatch { expression: owner };
    match application.content().character().target() {
        CharacterPresentationTargetEvidence::Exact(character) => {
            catalog.record(character).map_err(|_| invalid())?;
        }
        CharacterPresentationTargetEvidence::RuntimeCharacterDialogue { generation } => {
            let declaration = declaration.ok_or_else(invalid)?;
            if *generation != declaration.digest() {
                return Err(invalid());
            }
            let target_type = application.target().dialogue_type().identity();
            if target_type == declaration.any_dialogue().identity() {
                for character in declaration.characters().keys() {
                    catalog.record(character).map_err(|_| invalid())?;
                }
            } else {
                let character = declaration
                    .characters()
                    .iter()
                    .find_map(|(character, row)| {
                        (row.dialogue_type().identity() == target_type).then_some(character)
                    })
                    .ok_or_else(invalid)?;
                catalog.record(character).map_err(|_| invalid())?;
            }
        }
    }
    Ok(())
}
