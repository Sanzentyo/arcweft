use crate::{
    id::{CharacterId, CharacterLookId},
    manifest::{CharacterManifest, CharacterManifestError, ResolvedCharacterLayer},
};
use std::collections::BTreeMap;
use thiserror::Error;

/// Deterministic project-level collection of character manifests.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CharacterCatalog {
    manifests: BTreeMap<CharacterId, CharacterManifest>,
}

/// Character catalog insertion or resolution failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CharacterCatalogError {
    #[error("duplicate character manifest `{owner}`")]
    DuplicateOwner { owner: CharacterId },
    #[error("character `{character}` is not present in the catalog")]
    MissingCharacter { character: CharacterId },
    #[error(transparent)]
    Manifest(#[from] CharacterManifestError),
}

impl CharacterCatalog {
    /// Constructs one immutable runtime catalog after validating every manifest.
    pub fn try_from_manifests(
        manifests: impl IntoIterator<Item = CharacterManifest>,
    ) -> Result<Self, CharacterCatalogError> {
        let mut values = BTreeMap::new();
        for manifest in manifests {
            manifest.validate()?;
            let owner = manifest.character().clone();
            if values.insert(owner.clone(), manifest).is_some() {
                return Err(CharacterCatalogError::DuplicateOwner { owner });
            }
        }
        Ok(Self { manifests: values })
    }

    pub fn get(&self, character: &CharacterId) -> Option<&CharacterManifest> {
        self.manifests.get(character)
    }

    pub fn manifests(&self) -> impl ExactSizeIterator<Item = &CharacterManifest> {
        self.manifests.values()
    }

    pub fn is_empty(&self) -> bool {
        self.manifests.is_empty()
    }

    pub fn len(&self) -> usize {
        self.manifests.len()
    }

    /// Resolves one character/look pair into bottom-to-top image layers.
    pub fn resolve<'a>(
        &'a self,
        character: &CharacterId,
        look: &CharacterLookId,
    ) -> Result<Vec<ResolvedCharacterLayer<'a>>, CharacterCatalogError> {
        self.get(character)
            .ok_or_else(|| CharacterCatalogError::MissingCharacter {
                character: character.clone(),
            })?
            .resolve_look(look)
            .map_err(CharacterCatalogError::from)
    }

    /// Returns every character that defines a look with the supplied id.
    pub fn characters_with_look<'a>(
        &'a self,
        look: &'a CharacterLookId,
    ) -> impl Iterator<Item = &'a CharacterManifest> + 'a {
        self.manifests()
            .filter(move |manifest| manifest.look(look).is_some())
    }
}
