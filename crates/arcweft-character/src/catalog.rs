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

/// Digest of the complete validated runtime character catalog.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CharacterCatalogRuntimeDigest([u8; 32]);

impl CharacterCatalogRuntimeDigest {
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// String-bearing manifest coordinate used by canonical digest validation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CharacterCatalogStringField {
    CharacterId,
    DefaultLookId,
    PartId,
    VariantId,
    AssetPath,
    LookId,
    SelectionPartId,
    SelectionVariantId,
}

/// Sequence-bearing manifest coordinate used by canonical digest validation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CharacterCatalogSequenceField {
    CatalogRows,
    Parts,
    Variants,
    Looks,
    LookSelections,
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

/// Failure to compute the canonical runtime character-catalog digest.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CharacterCatalogRuntimeDigestError {
    #[error(transparent)]
    Catalog(#[from] CharacterCatalogError),
    #[error("character catalog contains {observed} rows; maximum is {maximum}")]
    EntryLimit { observed: usize, maximum: usize },
    #[error("character catalog key {key} does not equal manifest owner {owner}")]
    KeyOwnerMismatch {
        key: CharacterId,
        owner: CharacterId,
    },
    #[error("character {character} field {field:?} has {bytes} UTF-8 bytes; maximum is {maximum}")]
    StringLength {
        character: CharacterId,
        field: CharacterCatalogStringField,
        bytes: usize,
        maximum: u32,
    },
    #[error("character {character} field {field:?} has {observed} rows; maximum is {maximum}")]
    SequenceLength {
        character: CharacterId,
        field: CharacterCatalogSequenceField,
        observed: usize,
        maximum: u32,
    },
}

impl CharacterCatalog {
    pub const MAX_RUNTIME_DIGEST_ROWS: usize = 65_536;

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

    /// Computes the canonical version-one digest of all live runtime manifests.
    pub fn runtime_digest_v1(
        &self,
    ) -> Result<CharacterCatalogRuntimeDigest, CharacterCatalogRuntimeDigestError> {
        if self.len() > Self::MAX_RUNTIME_DIGEST_ROWS {
            return Err(CharacterCatalogRuntimeDigestError::EntryLimit {
                observed: self.len(),
                maximum: Self::MAX_RUNTIME_DIGEST_ROWS,
            });
        }

        for (key, manifest) in &self.manifests {
            manifest
                .validate()
                .map_err(CharacterCatalogError::from)
                .map_err(CharacterCatalogRuntimeDigestError::from)?;
            if key != manifest.character() {
                return Err(CharacterCatalogRuntimeDigestError::KeyOwnerMismatch {
                    key: key.clone(),
                    owner: manifest.character().clone(),
                });
            }
            validate_manifest_lengths(manifest)?;
        }

        let mut hasher = blake3::Hasher::new();
        hasher.update(b"arcweft.character-catalog.runtime.v1\0");
        hasher.update(&1_u32.to_le_bytes());
        hasher.update(&u32::try_from(self.len()).unwrap_or(u32::MAX).to_le_bytes());
        for (character, manifest) in &self.manifests {
            hash_string(&mut hasher, character.as_str());
            hasher.update(manifest.semantic_fingerprint_v1().as_bytes());
        }
        Ok(CharacterCatalogRuntimeDigest(*hasher.finalize().as_bytes()))
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

fn validate_manifest_lengths(
    manifest: &CharacterManifest,
) -> Result<(), CharacterCatalogRuntimeDigestError> {
    let character = manifest.character();
    validate_string(
        character,
        CharacterCatalogStringField::CharacterId,
        character.as_str(),
    )?;
    validate_string(
        character,
        CharacterCatalogStringField::DefaultLookId,
        manifest.default_look().as_str(),
    )?;
    validate_sequence(
        character,
        CharacterCatalogSequenceField::Parts,
        manifest.parts().len(),
    )?;
    let mut parts = manifest.parts().iter().collect::<Vec<_>>();
    parts.sort_by_key(|part| part.id());
    for part in parts {
        validate_string(
            character,
            CharacterCatalogStringField::PartId,
            part.id().as_str(),
        )?;
        validate_sequence(
            character,
            CharacterCatalogSequenceField::Variants,
            part.variants().len(),
        )?;
        let mut variants = part.variants().iter().collect::<Vec<_>>();
        variants.sort_by_key(|variant| variant.id());
        for variant in variants {
            validate_string(
                character,
                CharacterCatalogStringField::VariantId,
                variant.id().as_str(),
            )?;
            validate_string(
                character,
                CharacterCatalogStringField::AssetPath,
                variant.asset().as_str(),
            )?;
        }
    }
    validate_sequence(
        character,
        CharacterCatalogSequenceField::Looks,
        manifest.looks().len(),
    )?;
    let mut looks = manifest.looks().iter().collect::<Vec<_>>();
    looks.sort_by_key(|look| look.id());
    for look in looks {
        validate_string(
            character,
            CharacterCatalogStringField::LookId,
            look.id().as_str(),
        )?;
        validate_sequence(
            character,
            CharacterCatalogSequenceField::LookSelections,
            look.selections().len(),
        )?;
        let mut selections = look.selections().iter().collect::<Vec<_>>();
        selections.sort_by_key(|selection| selection.part());
        for selection in selections {
            validate_string(
                character,
                CharacterCatalogStringField::SelectionPartId,
                selection.part().as_str(),
            )?;
            validate_string(
                character,
                CharacterCatalogStringField::SelectionVariantId,
                selection.variant().as_str(),
            )?;
        }
    }
    Ok(())
}

fn validate_string(
    character: &CharacterId,
    field: CharacterCatalogStringField,
    value: &str,
) -> Result<(), CharacterCatalogRuntimeDigestError> {
    if value.len() > u32::MAX as usize {
        return Err(CharacterCatalogRuntimeDigestError::StringLength {
            character: character.clone(),
            field,
            bytes: value.len(),
            maximum: u32::MAX,
        });
    }
    Ok(())
}

fn validate_sequence(
    character: &CharacterId,
    field: CharacterCatalogSequenceField,
    observed: usize,
) -> Result<(), CharacterCatalogRuntimeDigestError> {
    if observed > u32::MAX as usize {
        return Err(CharacterCatalogRuntimeDigestError::SequenceLength {
            character: character.clone(),
            field,
            observed,
            maximum: u32::MAX,
        });
    }
    Ok(())
}

fn hash_string(hasher: &mut blake3::Hasher, value: &str) {
    hasher.update(&u32::try_from(value.len()).unwrap_or(u32::MAX).to_le_bytes());
    hasher.update(value.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        id::{CharacterLookId, CharacterPartId, CharacterVariantId},
        manifest::{
            CharacterAssetPath, CharacterBlendMode, CharacterCanvas, CharacterLook,
            CharacterManifest, CharacterPart, CharacterPartSelection, CharacterPoint,
            CharacterRect, CharacterSource, CharacterVariant,
        },
    };

    fn manifest(character: &str, opacity: u8, with_source: bool) -> CharacterManifest {
        let part = CharacterPart::new(
            CharacterPartId::try_new("body").expect("part"),
            0,
            vec![CharacterVariant::new(
                CharacterVariantId::try_new("default").expect("variant"),
                CharacterAssetPath::try_new("layers/body.png").expect("asset"),
                CharacterRect::new(0, 0, 64, 128),
                opacity,
                CharacterBlendMode::Normal,
                false,
            )],
        );
        let look = CharacterLook::new(
            CharacterLookId::try_new("normal").expect("look"),
            vec![CharacterPartSelection::new(
                CharacterPartId::try_new("body").expect("part"),
                CharacterVariantId::try_new("default").expect("variant"),
            )],
        );
        CharacterManifest::new(
            CharacterId::try_new(character).expect("character"),
            CharacterCanvas::new(64, 128),
            CharacterPoint::new(32, 128),
            CharacterLookId::try_new("normal").expect("look"),
            vec![part],
            vec![look],
            with_source
                .then(|| CharacterSource::psd("source.psd", "digest", "test-importer", Vec::new())),
        )
        .expect("manifest")
    }

    #[test]
    fn runtime_digest_is_order_independent_and_runtime_sensitive() {
        let akane = manifest("character.akane", u8::MAX, false);
        let aoi = manifest("character.aoi", u8::MAX, false);
        let first =
            CharacterCatalog::try_from_manifests([akane.clone(), aoi.clone()]).expect("catalog");
        let reordered =
            CharacterCatalog::try_from_manifests([aoi, akane]).expect("reordered catalog");
        assert_eq!(
            first.runtime_digest_v1().expect("digest"),
            reordered.runtime_digest_v1().expect("digest")
        );

        let changed = CharacterCatalog::try_from_manifests([
            manifest("character.akane", 127, false),
            manifest("character.aoi", u8::MAX, false),
        ])
        .expect("changed catalog");
        assert_ne!(
            first.runtime_digest_v1().expect("digest"),
            changed.runtime_digest_v1().expect("changed digest")
        );
    }

    #[test]
    fn runtime_digest_excludes_source_only_manifest_evidence() {
        let plain =
            CharacterCatalog::try_from_manifests([manifest("character.akane", u8::MAX, false)])
                .expect("plain catalog");
        let sourced =
            CharacterCatalog::try_from_manifests([manifest("character.akane", u8::MAX, true)])
                .expect("sourced catalog");
        assert_eq!(
            plain.runtime_digest_v1().expect("plain digest"),
            sourced.runtime_digest_v1().expect("sourced digest")
        );
    }
}
