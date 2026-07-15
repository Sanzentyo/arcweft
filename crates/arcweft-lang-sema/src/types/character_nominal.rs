//! Structural character nominal identity.

use arcweft_character::id::{CharacterId, CharacterPartId};

/// Manifest-backed character enum family.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CharacterNominalFamily {
    Look,
    Part,
    Variant,
}

/// Structural identity of a manifest-derived character enum.
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

    /// Display-only Arcweft surface spelling.
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
}
