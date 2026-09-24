//! Closed lower-layer identity for the CharacterDialogue opaque producer.

use crate::pattern::RuntimeOpaqueTypeProducerId;

/// Registry identity shared by plan admission, AWBC verification, and the
/// dialogue-owned producer. Source callable spelling never selects this ID.
pub struct RuntimeCharacterDialogueProducerId;

impl RuntimeCharacterDialogueProducerId {
    #[must_use]
    pub fn get() -> RuntimeOpaqueTypeProducerId {
        RuntimeOpaqueTypeProducerId::try_new("std.character_dialogue")
            .expect("the canonical CharacterDialogue producer ID is valid")
    }
}
