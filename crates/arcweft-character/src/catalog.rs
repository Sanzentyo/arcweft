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
    #[error("duplicate character manifest `{0}`")]
    DuplicateCharacter(String),
    #[error("character `{0}` is not present in the catalog")]
    MissingCharacter(String),
    #[error(transparent)]
    Manifest(#[from] CharacterManifestError),
}

impl CharacterCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts one validated manifest, rejecting duplicate public character ids.
    pub fn insert(&mut self, manifest: CharacterManifest) -> Result<(), CharacterCatalogError> {
        manifest.validate()?;
        let character = manifest.character().clone();
        if self.manifests.contains_key(&character) {
            return Err(CharacterCatalogError::DuplicateCharacter(
                character.to_string(),
            ));
        }
        self.manifests.insert(character, manifest);
        Ok(())
    }

    pub fn with_manifest(
        mut self,
        manifest: CharacterManifest,
    ) -> Result<Self, CharacterCatalogError> {
        self.insert(manifest)?;
        Ok(self)
    }

    pub fn get(&self, character: &CharacterId) -> Option<&CharacterManifest> {
        self.manifests.get(character)
    }

    pub fn get_by_str(&self, character: &str) -> Option<&CharacterManifest> {
        self.manifests
            .iter()
            .find_map(|(id, manifest)| (id.as_str() == character).then_some(manifest))
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
            .ok_or_else(|| CharacterCatalogError::MissingCharacter(character.to_string()))?
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        id::{CharacterLookId, CharacterPartId, CharacterVariantId},
        manifest::{
            CharacterAssetPath, CharacterBlendMode, CharacterCanvas, CharacterLook, CharacterPart,
            CharacterPartSelection, CharacterPoint, CharacterRect, CharacterVariant,
        },
    };

    fn manifest(id: &str) -> CharacterManifest {
        let look = CharacterLookId::try_new("normal").expect("look");
        CharacterManifest::new(
            CharacterId::try_new(id).expect("character"),
            CharacterCanvas::new(8, 8),
            CharacterPoint::new(4, 8),
            look.clone(),
            vec![CharacterPart::new(
                CharacterPartId::try_new("body").expect("part"),
                0,
                vec![CharacterVariant::new(
                    CharacterVariantId::try_new("default").expect("variant"),
                    CharacterAssetPath::try_new("layers/body.png").expect("asset"),
                    CharacterRect::new(0, 0, 8, 8),
                    u8::MAX,
                    CharacterBlendMode::Normal,
                    false,
                )],
            )],
            vec![CharacterLook::new(
                look,
                vec![CharacterPartSelection::new(
                    CharacterPartId::try_new("body").expect("part"),
                    CharacterVariantId::try_new("default").expect("variant"),
                )],
            )],
            None,
        )
        .expect("manifest")
    }

    #[test]
    fn duplicate_characters_are_rejected() {
        let mut catalog = CharacterCatalog::new();
        catalog
            .insert(manifest("character.akane"))
            .expect("first insert");
        assert!(matches!(
            catalog.insert(manifest("character.akane")),
            Err(CharacterCatalogError::DuplicateCharacter(_))
        ));
    }
}
