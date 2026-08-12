//! Canonical semantic and opaque runtime owner for `CharacterDialogue`.

use arcweft_character::id::CharacterId;
use arcweft_core::pattern::{
    RuntimeOpaqueTypeOwner, RuntimeOpaqueTypeProducerId, RuntimeSemanticTypeId,
    RuntimeSemanticTypeIdentityEncoder,
};

pub(super) fn character_dialogue_opaque_type_producer() -> RuntimeOpaqueTypeProducerId {
    RuntimeOpaqueTypeProducerId::try_new("std.character_dialogue")
        .expect("the canonical CharacterDialogue producer is valid")
}

/// Character identity precision retained by a checked `CharacterDialogue` value.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CharacterDialogueCharacterType {
    Exact(CharacterId),
    Any,
}

/// Semantic type of one immutable first-class dialogue presentation value.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CharacterDialogueType {
    character: CharacterDialogueCharacterType,
}

impl CharacterDialogueCharacterType {
    #[must_use]
    pub fn accepts(&self, actual: &Self) -> bool {
        matches!(self, Self::Any)
            || matches!((self, actual), (Self::Exact(expected), Self::Exact(actual)) if expected == actual)
    }

    #[must_use]
    pub fn join(left: Self, right: &Self) -> Self {
        if &left == right { left } else { Self::Any }
    }

    #[must_use]
    pub const fn exact(&self) -> Option<&CharacterId> {
        match self {
            Self::Exact(character) => Some(character),
            Self::Any => None,
        }
    }
}

impl CharacterDialogueType {
    #[must_use]
    pub const fn new(character: CharacterDialogueCharacterType) -> Self {
        Self { character }
    }

    #[must_use]
    pub fn exact(character: CharacterId) -> Self {
        Self::new(CharacterDialogueCharacterType::Exact(character))
    }

    #[must_use]
    pub const fn any() -> Self {
        Self::new(CharacterDialogueCharacterType::Any)
    }

    #[must_use]
    pub const fn character(&self) -> &CharacterDialogueCharacterType {
        &self.character
    }

    #[must_use]
    pub fn accepts(&self, actual: &Self) -> bool {
        self.character.accepts(&actual.character)
    }

    #[must_use]
    pub fn join(left: Self, right: &Self) -> Self {
        Self::new(CharacterDialogueCharacterType::join(
            left.character,
            &right.character,
        ))
    }

    #[must_use]
    pub fn source_label(&self) -> String {
        match &self.character {
            CharacterDialogueCharacterType::Exact(character) => {
                format!("CharacterDialogue<{}>", character.as_str())
            }
            CharacterDialogueCharacterType::Any => "CharacterDialogue".to_owned(),
        }
    }

    /// Appends the canonical `CharacterDialogue` fragment to a semantic identity.
    pub fn encode_runtime_semantic_identity(
        &self,
        encoder: &mut RuntimeSemanticTypeIdentityEncoder,
    ) {
        encoder.write_tag(69);
        match self.character() {
            CharacterDialogueCharacterType::Exact(character) => {
                encoder.write_u8(0);
                encoder.write_str(character.as_str());
            }
            CharacterDialogueCharacterType::Any => encoder.write_u8(1),
        }
    }

    /// Complete semantic identity for this `CharacterDialogue` type.
    #[must_use]
    pub fn runtime_semantic_identity(&self) -> RuntimeSemanticTypeId {
        let mut encoder = RuntimeSemanticTypeIdentityEncoder::new();
        self.encode_runtime_semantic_identity(&mut encoder);
        encoder.finish()
    }

    /// Canonical exact or producer-wide opaque runtime owner.
    ///
    /// # Panics
    ///
    /// Panics only if the compile-time canonical producer literal violates the
    /// core producer identity grammar.
    #[must_use]
    pub fn runtime_opaque_owner(&self) -> RuntimeOpaqueTypeOwner {
        let producer = character_dialogue_opaque_type_producer();
        match self.character() {
            CharacterDialogueCharacterType::Exact(_) => {
                RuntimeOpaqueTypeOwner::exact(producer, self.runtime_semantic_identity())
            }
            CharacterDialogueCharacterType::Any => {
                RuntimeOpaqueTypeOwner::producer_wide(producer, self.runtime_semantic_identity())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn character(value: &str) -> CharacterId {
        CharacterId::try_new(value).expect("Character ID")
    }

    #[test]
    fn exact_and_any_acceptance_preserves_nominal_precision() {
        let alice = CharacterDialogueType::exact(character("character.alice"));
        let bob = CharacterDialogueType::exact(character("character.bob"));
        assert!(alice.accepts(&alice));
        assert!(!alice.accepts(&bob));
        assert!(CharacterDialogueType::any().accepts(&alice));
        assert!(!alice.accepts(&CharacterDialogueType::any()));
    }

    #[test]
    fn branch_join_widens_only_distinct_character_identities() {
        let alice = CharacterDialogueCharacterType::Exact(character("character.alice"));
        let bob = CharacterDialogueCharacterType::Exact(character("character.bob"));
        assert_eq!(
            CharacterDialogueCharacterType::join(alice.clone(), &alice),
            alice
        );
        assert_eq!(
            CharacterDialogueCharacterType::join(alice, &bob),
            CharacterDialogueCharacterType::Any
        );
    }

    #[test]
    fn exact_and_any_owners_use_one_canonical_producer() {
        let exact = CharacterDialogueType::exact(character("character.alice"));
        let any = CharacterDialogueType::any();
        assert_eq!(
            exact.runtime_opaque_owner().producer().as_str(),
            "std.character_dialogue"
        );
        assert_eq!(
            any.runtime_opaque_owner().producer().as_str(),
            "std.character_dialogue"
        );
        assert_ne!(
            exact.runtime_semantic_identity(),
            any.runtime_semantic_identity()
        );
    }
}
