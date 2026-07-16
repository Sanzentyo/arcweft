use thiserror::Error;

use super::{
    CharacterLookField, CharacterManifestRootField, CharacterManifestToken,
    CharacterManifestTokenPath, CharacterPartField, CharacterVariantField,
    SourceBackedCharacterManifest,
};
use crate::{
    id::{CharacterId, CharacterLookId, CharacterPartId, CharacterVariantId},
    manifest::CharacterManifest,
    symbol::CharacterSymbolDescriptor,
};

/// Failure to project a typed character symbol to its exact authored token.
#[derive(Clone, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum CharacterManifestDeclarationError {
    #[error("descriptor owner does not match the manifest owner")]
    WrongOwner {
        manifest: CharacterId,
        requested: CharacterId,
    },
    #[error("look is absent from the manifest")]
    UnknownLook {
        character: CharacterId,
        look: CharacterLookId,
    },
    #[error("part is absent from the manifest")]
    UnknownPart {
        character: CharacterId,
        part: CharacterPartId,
    },
    #[error("variant is absent from the requested owning part")]
    UnknownVariant {
        character: CharacterId,
        part: CharacterPartId,
        variant: CharacterVariantId,
    },
    #[error("variant exists only under a different owning part")]
    WrongOwningPart {
        character: CharacterId,
        requested: CharacterPartId,
        variant: CharacterVariantId,
        actual: Vec<CharacterPartId>,
    },
    #[error("validated manifest contains a duplicate typed look id")]
    DuplicateLook {
        character: CharacterId,
        look: CharacterLookId,
        positions: Vec<usize>,
    },
    #[error("validated manifest contains a duplicate typed part id")]
    DuplicatePart {
        character: CharacterId,
        part: CharacterPartId,
        positions: Vec<usize>,
    },
    #[error("validated manifest contains a duplicate typed variant id in one part")]
    DuplicateVariant {
        character: CharacterId,
        part: CharacterPartId,
        variant: CharacterVariantId,
        positions: Vec<usize>,
    },
    #[error("manifest source map is missing the projected token")]
    MissingToken { path: CharacterManifestTokenPath },
    #[error("projected declaration token is not a JSON string")]
    NonStringToken { path: CharacterManifestTokenPath },
}

impl SourceBackedCharacterManifest {
    /// Projects a typed character symbol to its exact structural declaration token.
    pub fn declaration_token(
        &self,
        descriptor: &CharacterSymbolDescriptor,
    ) -> Result<
        (CharacterManifestTokenPath, &CharacterManifestToken),
        CharacterManifestDeclarationError,
    > {
        let path = self.manifest().declaration_token_path(descriptor)?;
        let token = self.source_map().token(&path).ok_or_else(|| {
            CharacterManifestDeclarationError::MissingToken { path: path.clone() }
        })?;
        if token.string_content().is_none() {
            return Err(CharacterManifestDeclarationError::NonStringToken { path });
        }
        Ok((path, token))
    }
}

impl CharacterManifest {
    /// Projects a typed character symbol to its structural JSON declaration path.
    pub fn declaration_token_path(
        &self,
        descriptor: &CharacterSymbolDescriptor,
    ) -> Result<CharacterManifestTokenPath, CharacterManifestDeclarationError> {
        if descriptor.character() != self.character() {
            return Err(CharacterManifestDeclarationError::WrongOwner {
                manifest: self.character().clone(),
                requested: descriptor.character().clone(),
            });
        }

        match descriptor {
            CharacterSymbolDescriptor::Owner { .. } => Ok(CharacterManifestTokenPath::Root(
                CharacterManifestRootField::Character,
            )),
            CharacterSymbolDescriptor::Look { look, .. } => self.look_declaration_path(look),
            CharacterSymbolDescriptor::Part { part, .. } => self.part_declaration_path(part),
            CharacterSymbolDescriptor::Variant { part, variant, .. } => {
                self.variant_declaration_path(part, variant)
            }
        }
    }

    fn look_declaration_path(
        &self,
        look: &CharacterLookId,
    ) -> Result<CharacterManifestTokenPath, CharacterManifestDeclarationError> {
        let positions = self
            .looks()
            .iter()
            .enumerate()
            .filter_map(|(position, candidate)| (candidate.id() == look).then_some(position))
            .collect::<Vec<_>>();
        match positions.as_slice() {
            [look] => Ok(CharacterManifestTokenPath::Look {
                look: *look,
                field: CharacterLookField::Id,
            }),
            [] => Err(CharacterManifestDeclarationError::UnknownLook {
                character: self.character().clone(),
                look: look.clone(),
            }),
            _ => Err(CharacterManifestDeclarationError::DuplicateLook {
                character: self.character().clone(),
                look: look.clone(),
                positions,
            }),
        }
    }

    fn part_declaration_path(
        &self,
        part: &CharacterPartId,
    ) -> Result<CharacterManifestTokenPath, CharacterManifestDeclarationError> {
        let positions = self
            .parts()
            .iter()
            .enumerate()
            .filter_map(|(position, candidate)| (candidate.id() == part).then_some(position))
            .collect::<Vec<_>>();
        match positions.as_slice() {
            [part] => Ok(CharacterManifestTokenPath::Part {
                part: *part,
                field: CharacterPartField::Id,
            }),
            [] => Err(CharacterManifestDeclarationError::UnknownPart {
                character: self.character().clone(),
                part: part.clone(),
            }),
            _ => Err(CharacterManifestDeclarationError::DuplicatePart {
                character: self.character().clone(),
                part: part.clone(),
                positions,
            }),
        }
    }

    fn variant_declaration_path(
        &self,
        part: &CharacterPartId,
        variant: &CharacterVariantId,
    ) -> Result<CharacterManifestTokenPath, CharacterManifestDeclarationError> {
        let part_positions = self
            .parts()
            .iter()
            .enumerate()
            .filter_map(|(position, candidate)| (candidate.id() == part).then_some(position))
            .collect::<Vec<_>>();
        let part_position = match part_positions.as_slice() {
            [position] => *position,
            [] => {
                return Err(CharacterManifestDeclarationError::UnknownPart {
                    character: self.character().clone(),
                    part: part.clone(),
                });
            }
            _ => {
                return Err(CharacterManifestDeclarationError::DuplicatePart {
                    character: self.character().clone(),
                    part: part.clone(),
                    positions: part_positions,
                });
            }
        };
        let variant_positions = self.parts()[part_position]
            .variants()
            .iter()
            .enumerate()
            .filter_map(|(position, candidate)| (candidate.id() == variant).then_some(position))
            .collect::<Vec<_>>();
        match variant_positions.as_slice() {
            [variant] => Ok(CharacterManifestTokenPath::Variant {
                part: part_position,
                variant: *variant,
                field: CharacterVariantField::Id,
            }),
            [] => Err(self.missing_variant_error(part, variant)),
            _ => Err(CharacterManifestDeclarationError::DuplicateVariant {
                character: self.character().clone(),
                part: part.clone(),
                variant: variant.clone(),
                positions: variant_positions,
            }),
        }
    }

    fn missing_variant_error(
        &self,
        part: &CharacterPartId,
        variant: &CharacterVariantId,
    ) -> CharacterManifestDeclarationError {
        let mut actual = self
            .parts()
            .iter()
            .filter(|candidate| candidate.id() != part)
            .filter(|candidate| {
                candidate
                    .variants()
                    .iter()
                    .any(|candidate| candidate.id() == variant)
            })
            .map(|candidate| candidate.id().clone())
            .collect::<Vec<_>>();
        actual.sort();
        actual.dedup();
        if actual.is_empty() {
            CharacterManifestDeclarationError::UnknownVariant {
                character: self.character().clone(),
                part: part.clone(),
                variant: variant.clone(),
            }
        } else {
            CharacterManifestDeclarationError::WrongOwningPart {
                character: self.character().clone(),
                requested: part.clone(),
                variant: variant.clone(),
                actual,
            }
        }
    }
}
