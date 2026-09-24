//! Manifest-owned Character nominal identities and program-bound Look admission.

use arcweft_character::{
    catalog::CharacterCatalog,
    id::{CharacterId, CharacterLookId, CharacterPartId},
    manifest::CharacterManifest,
};
use std::sync::Arc;
use thiserror::Error;

use crate::{
    entry::{
        RuntimeNominalSchemaBody, RuntimeNominalSchemaCase, RuntimeNominalSchemaDefinition,
        RuntimeNominalSchemaGraph, RuntimeNominalSchemaGraphError, RuntimeNominalSchemaIdentity,
        RuntimeNominalTypeId, RuntimeSchemaLimits,
    },
    pattern::{
        RuntimeCheckedType, RuntimeSemanticTypeId, RuntimeSemanticTypeIdentityEncoder,
        RuntimeVariantIdentity,
    },
    program_types::RuntimeProgramTypeError,
    task::RuntimeProgramOwner,
    value::RuntimeValue,
};

/// Manifest-backed Character enum family.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CharacterNominalFamily {
    Look,
    Part,
    Variant,
}

/// Structural identity of one manifest-derived Character enum. This type owns
/// the version-one semantic fragment used by Sema and runtime admission.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CharacterNominalType {
    Look {
        character: CharacterId,
    },
    Part {
        character: CharacterId,
    },
    Variant {
        character: CharacterId,
        part: CharacterPartId,
    },
}

impl CharacterNominalType {
    #[must_use]
    pub const fn family(&self) -> CharacterNominalFamily {
        match self {
            Self::Look { .. } => CharacterNominalFamily::Look,
            Self::Part { .. } => CharacterNominalFamily::Part,
            Self::Variant { .. } => CharacterNominalFamily::Variant,
        }
    }

    #[must_use]
    pub const fn character(&self) -> &CharacterId {
        match self {
            Self::Look { character }
            | Self::Part { character }
            | Self::Variant { character, .. } => character,
        }
    }

    #[must_use]
    pub const fn part(&self) -> Option<&CharacterPartId> {
        match self {
            Self::Variant { part, .. } => Some(part),
            Self::Look { .. } | Self::Part { .. } => None,
        }
    }

    #[must_use]
    pub fn source_label(&self) -> String {
        match self {
            Self::Look { character } => format!("CharacterLook<{character}>"),
            Self::Part { character } => format!("CharacterPart<{character}>"),
            Self::Variant { character, part } => {
                format!("CharacterVariant<{character},{part}>")
            }
        }
    }

    /// Writes the complete Character nominal fragment into the shared checked
    /// type identity grammar. No source spelling or caller-supplied digest is
    /// trusted at runtime.
    pub fn encode_runtime_semantic_identity(
        &self,
        encoder: &mut RuntimeSemanticTypeIdentityEncoder,
    ) {
        encoder.write_tag(73);
        match self {
            Self::Look { character } => {
                encoder.write_u8(0);
                encoder.write_str(character.as_str());
            }
            Self::Part { character } => {
                encoder.write_u8(1);
                encoder.write_str(character.as_str());
            }
            Self::Variant { character, part } => {
                encoder.write_u8(2);
                encoder.write_str(character.as_str());
                encoder.write_str(part.as_str());
            }
        }
    }

    #[must_use]
    pub fn runtime_semantic_identity(&self) -> RuntimeSemanticTypeId {
        let mut encoder = RuntimeSemanticTypeIdentityEncoder::new();
        self.encode_runtime_semantic_identity(&mut encoder);
        encoder.finish()
    }
}

/// One immutable executable and logical Character catalog that jointly admit
/// Look<C> values. Every present manifest's complete enum row is checked when
/// this authority is built; decode never trusts an external Character→type map.
#[derive(Clone, Debug)]
pub struct RuntimeCharacterLookSourceAuthority {
    owner: RuntimeProgramOwner,
    characters: Arc<CharacterCatalog>,
}

#[derive(Debug, Error)]
pub enum RuntimeCharacterLookSourceError {
    #[error("Character Look value belongs to a different executable")]
    ForeignProgram,
    #[error("Character `{0}` has no accepted visual manifest")]
    MissingVisualManifest(CharacterId),
    #[error("Character `{0}` has a different Look<C> executable row")]
    InvalidLookType(CharacterId),
    #[error("Character `{0}` has an invalid Look<C> value")]
    InvalidLookValue(CharacterId),
    #[error(transparent)]
    NominalGraph(#[from] RuntimeNominalSchemaGraphError),
    #[error(transparent)]
    ProgramType(#[from] RuntimeProgramTypeError),
}

impl RuntimeCharacterLookSourceAuthority {
    pub fn try_new(
        owner: RuntimeProgramOwner,
        characters: Arc<CharacterCatalog>,
    ) -> Result<Self, RuntimeCharacterLookSourceError> {
        let authority = Self { owner, characters };
        for character in authority.characters.characters() {
            if let Some(manifest) = authority.characters.visual_manifest(character) {
                authority.validate_look_type(character, manifest)?;
            }
        }
        Ok(authority)
    }

    #[must_use]
    pub const fn program_owner(&self) -> &RuntimeProgramOwner {
        &self.owner
    }

    #[must_use]
    pub fn character_catalog(&self) -> &Arc<CharacterCatalog> {
        &self.characters
    }

    fn look_identity(character: &CharacterId) -> RuntimeSemanticTypeId {
        CharacterNominalType::Look {
            character: character.clone(),
        }
        .runtime_semantic_identity()
    }

    fn validate_look_type(
        &self,
        character: &CharacterId,
        manifest: &CharacterManifest,
    ) -> Result<(), RuntimeCharacterLookSourceError> {
        let semantic = Self::look_identity(character);
        let nominal = RuntimeNominalTypeId::from_checked_digest(*semantic.as_bytes());
        let graph = RuntimeNominalSchemaGraph::try_new(
            [RuntimeNominalSchemaDefinition::new(
                RuntimeNominalSchemaIdentity::new(nominal.clone(), semantic),
                Vec::<crate::entry::RuntimeTypeSchema>::new(),
                RuntimeNominalSchemaBody::Variant {
                    cases: manifest
                        .looks()
                        .iter()
                        .enumerate()
                        .map(|(ordinal, look)| {
                            RuntimeNominalSchemaCase::new(
                                u32::try_from(ordinal).expect("validated Look inventory fits u32"),
                                look.id().as_str().to_owned(),
                                None,
                            )
                        })
                        .collect(),
                },
            )],
            RuntimeSchemaLimits::engine_default(),
        )?;
        let layout = graph.try_layout_hash(semantic)?;
        let RuntimeCheckedType::Variant {
            owner:
                RuntimeVariantIdentity::Nominal {
                    nominal: actual_nominal,
                    semantic_identity: actual_semantic,
                    layout: actual_layout,
                },
            arguments,
            cases,
        } = self.owner.types().checked_type(semantic)?
        else {
            return Err(RuntimeCharacterLookSourceError::InvalidLookType(
                character.clone(),
            ));
        };
        if actual_nominal != nominal
            || actual_semantic != semantic
            || actual_layout != layout
            || !arguments.is_empty()
            || cases.len() != manifest.looks().len()
            || cases.iter().zip(manifest.looks()).any(|(case, look)| {
                case.name.as_str() != look.id().as_str() || case.payload.is_some()
            })
        {
            return Err(RuntimeCharacterLookSourceError::InvalidLookType(
                character.clone(),
            ));
        }
        Ok(())
    }

    pub fn decode(
        &self,
        owner: &RuntimeProgramOwner,
        character: &CharacterId,
        value: &RuntimeValue,
    ) -> Result<CharacterLookId, RuntimeCharacterLookSourceError> {
        if !self.owner.same_program(owner) {
            return Err(RuntimeCharacterLookSourceError::ForeignProgram);
        }
        let manifest = self.characters.visual_manifest(character).ok_or_else(|| {
            RuntimeCharacterLookSourceError::MissingVisualManifest(character.clone())
        })?;
        let semantic = Self::look_identity(character);
        self.owner
            .types()
            .accepts_value(semantic, value, RuntimeSchemaLimits::engine_default())?;
        let RuntimeValue::Variant {
            owner:
                RuntimeVariantIdentity::Nominal {
                    semantic_identity, ..
                },
            ordinal,
            name,
            payload: None,
        } = value
        else {
            return Err(RuntimeCharacterLookSourceError::InvalidLookValue(
                character.clone(),
            ));
        };
        let look = usize::try_from(*ordinal)
            .ok()
            .and_then(|ordinal| manifest.looks().get(ordinal))
            .filter(|look| *semantic_identity == semantic && name.as_str() == look.id().as_str())
            .ok_or_else(|| RuntimeCharacterLookSourceError::InvalidLookValue(character.clone()))?;
        Ok(look.id().clone())
    }
}

#[cfg(test)]
mod tests;
