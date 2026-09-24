//! Admission of one declaration against its immutable executable and resources.

use std::{collections::BTreeSet, sync::Arc};

use arcweft_character::{catalog::CharacterCatalog, id::CharacterId};
use arcweft_core::{
    character_nominal::{RuntimeCharacterLookSourceAuthority, RuntimeCharacterLookSourceError},
    entry::RuntimeValueDigest,
    pattern::{RuntimeCheckedType, RuntimeSemanticTypeId},
    program_types::RuntimeProgramTypeError,
    task::RuntimeProgramOwner,
};
use arcweft_view::{ViewRegistry, ViewRegistryRuntimeDigestError};
use thiserror::Error;

use super::{
    CharacterDialogueGenerationDeclaration, CharacterDialogueGenerationDeclarationError,
    CharacterDialogueRuntimeSchema, CharacterDialogueType, CharacterDialogueValueError,
    CharacterDialogueVisualType,
};

/// A generation input that disagrees with the executable or resources being bound.
#[derive(Debug, Error)]
pub enum CharacterDialogueGenerationBindingError {
    #[error("logical Character `{0}` is absent from the runtime catalog")]
    MissingCharacter(CharacterId),
    #[error("runtime Character `{0}` is absent from the generation declaration")]
    UnexpectedCharacter(CharacterId),
    #[error("visual manifest evidence differs for Character `{character}`")]
    VisualManifestMismatch {
        character: CharacterId,
        expected: Option<RuntimeValueDigest>,
        actual: Option<RuntimeValueDigest>,
    },
    #[error("executable type {identity:?} does not have its declared CharacterDialogue contract")]
    DialogueTypeMismatch { identity: RuntimeSemanticTypeId },
    #[error(transparent)]
    ViewRegistry(#[from] ViewRegistryRuntimeDigestError),
    #[error(transparent)]
    Presentation(Box<CharacterDialogueGenerationDeclarationError>),
    #[error(transparent)]
    ProgramType(#[from] RuntimeProgramTypeError),
    #[error(transparent)]
    Look(Box<RuntimeCharacterLookSourceError>),
    #[error(transparent)]
    Schema(Box<CharacterDialogueValueError>),
}

impl CharacterDialogueGenerationDeclaration<RuntimeSemanticTypeId> {
    /// Binds the declared inputs to one exact executable lease. The supplied
    /// Style fingerprint comes from the actual accepted Style resource; `None`
    /// means that the generation has no such resource.
    ///
    /// The returned schema owns the executable and catalogs. Its generation
    /// digest identifies these declared inputs, while its program owner keeps
    /// producer calls and decoded values pinned to the selected executable.
    pub fn bind_runtime(
        &self,
        views: Arc<ViewRegistry>,
        characters: Arc<CharacterCatalog>,
        actual_style_digest: Option<RuntimeValueDigest>,
        owner: RuntimeProgramOwner,
    ) -> Result<CharacterDialogueRuntimeSchema, CharacterDialogueGenerationBindingError> {
        self.validate_character_binding(&characters)?;
        let view_digest = RuntimeValueDigest::from_bytes(*views.runtime_digest_v1()?.as_bytes());
        self.presentation()
            .verify_resource_fingerprints(view_digest, actual_style_digest)
            .map_err(|error| {
                CharacterDialogueGenerationBindingError::Presentation(Box::new(error))
            })?;

        let mut references = BTreeSet::new();
        self.visit_type_refs(&mut |identity| {
            references.insert(*identity);
        });
        for identity in references {
            owner.types().require_type(identity)?;
        }
        Self::validate_dialogue_binding(&owner, &CharacterDialogueType::any())?;
        for character in self.characters().keys() {
            Self::validate_dialogue_binding(
                &owner,
                &CharacterDialogueType::exact(character.clone()),
            )?;
        }
        let looks = Arc::new(
            RuntimeCharacterLookSourceAuthority::try_new(owner.clone(), characters)
                .map_err(|error| CharacterDialogueGenerationBindingError::Look(Box::new(error)))?,
        );
        CharacterDialogueRuntimeSchema::try_bind_generation(self, views, looks, owner)
            .map_err(|error| CharacterDialogueGenerationBindingError::Schema(Box::new(error)))
    }

    fn validate_character_binding(
        &self,
        characters: &CharacterCatalog,
    ) -> Result<(), CharacterDialogueGenerationBindingError> {
        for character in characters.characters() {
            if !self.characters().contains_key(character) {
                return Err(
                    CharacterDialogueGenerationBindingError::UnexpectedCharacter(character.clone()),
                );
            }
        }
        for (character, declaration) in self.characters() {
            if !characters.contains_character(character) {
                return Err(CharacterDialogueGenerationBindingError::MissingCharacter(
                    character.clone(),
                ));
            }
            let expected = match declaration.visual() {
                CharacterDialogueVisualType::Absent => None,
                CharacterDialogueVisualType::Present { manifest, .. } => Some(*manifest),
            };
            let actual = characters.visual_manifest(character).map(|manifest| {
                RuntimeValueDigest::from_bytes(*manifest.semantic_fingerprint_v1().as_bytes())
            });
            if expected != actual {
                return Err(
                    CharacterDialogueGenerationBindingError::VisualManifestMismatch {
                        character: character.clone(),
                        expected,
                        actual,
                    },
                );
            }
        }
        Ok(())
    }

    fn validate_dialogue_binding(
        owner: &RuntimeProgramOwner,
        dialogue: &CharacterDialogueType,
    ) -> Result<(), CharacterDialogueGenerationBindingError> {
        let identity = dialogue.runtime_semantic_identity();
        if owner.types().checked_type(identity)?
            != (RuntimeCheckedType::Opaque {
                owner: dialogue.runtime_opaque_owner(),
            })
        {
            return Err(CharacterDialogueGenerationBindingError::DialogueTypeMismatch { identity });
        }
        Ok(())
    }
}
