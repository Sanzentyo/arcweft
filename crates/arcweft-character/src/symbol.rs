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
