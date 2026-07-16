use crate::id::{CharacterId, CharacterLookId, CharacterPartId, CharacterVariantId};

/// Typed manifest symbol selected by project aliases and tooling queries.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CharacterSymbolDescriptor {
    Owner {
        character: CharacterId,
    },
    Look {
        character: CharacterId,
        look: CharacterLookId,
    },
    Part {
        character: CharacterId,
        part: CharacterPartId,
    },
    Variant {
        character: CharacterId,
        part: CharacterPartId,
        variant: CharacterVariantId,
    },
}

impl CharacterSymbolDescriptor {
    /// Returns the owning character for this manifest symbol.
    pub const fn character(&self) -> &CharacterId {
        match self {
            Self::Owner { character }
            | Self::Look { character, .. }
            | Self::Part { character, .. }
            | Self::Variant { character, .. } => character,
        }
    }

    /// Returns the owning part for part and variant symbols.
    pub const fn part(&self) -> Option<&CharacterPartId> {
        match self {
            Self::Part { part, .. } | Self::Variant { part, .. } => Some(part),
            Self::Owner { .. } | Self::Look { .. } => None,
        }
    }
}
