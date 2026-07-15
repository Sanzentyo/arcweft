use crate::{
    id::CharacterId,
    manifest::registration::{
        CharacterManifestRootField, CharacterManifestTokenPath, SourceBackedCharacterManifest,
    },
};
use arcweft_source::{SourceDocumentIdentity, SourceSpan};
use std::collections::BTreeMap;
use thiserror::Error;

/// Source-provenance-preserving catalog accepted by semantic registration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceBackedCharacterCatalog {
    source: SourceDocumentIdentity,
    manifests: Vec<SourceBackedCharacterManifest>,
}

impl SourceBackedCharacterCatalog {
    pub fn try_new(
        source: SourceDocumentIdentity,
        manifests: Vec<SourceBackedCharacterManifest>,
    ) -> Result<Self, SourceBackedCharacterCatalogError> {
        let mut owners = BTreeMap::<CharacterId, SourceSpan>::new();
        for manifest in &manifests {
            let owner = manifest.manifest().character().clone();
            let declaration = manifest
                .source_map()
                .token(&CharacterManifestTokenPath::Root(
                    CharacterManifestRootField::Character,
                ))
                .ok_or_else(|| SourceBackedCharacterCatalogError::MissingDeclaration {
                    owner: owner.clone(),
                    document: manifest.source_map().document().clone(),
                })?
                .value()
                .clone();
            if let Some(first) = owners.insert(owner.clone(), declaration.clone()) {
                return Err(SourceBackedCharacterCatalogError::DuplicateOwner {
                    owner,
                    first,
                    duplicate: declaration,
                });
            }
        }
        Ok(Self { source, manifests })
    }

    pub const fn source(&self) -> &SourceDocumentIdentity {
        &self.source
    }

    pub fn manifests(&self) -> impl ExactSizeIterator<Item = &SourceBackedCharacterManifest> {
        self.manifests.iter()
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum SourceBackedCharacterCatalogError {
    #[error("character `{owner}` has no declaration token in `{document:?}`")]
    MissingDeclaration {
        owner: CharacterId,
        document: SourceDocumentIdentity,
    },
    #[error("character `{owner}` occurs more than once in one catalog")]
    DuplicateOwner {
        owner: CharacterId,
        first: SourceSpan,
        duplicate: SourceSpan,
    },
}
