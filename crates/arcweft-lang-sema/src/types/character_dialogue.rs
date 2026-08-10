use arcweft_character::id::CharacterId;

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
}

#[cfg(test)]
mod tests {
    use arcweft_character::id::CharacterId;

    use super::{CharacterDialogueCharacterType, CharacterDialogueType};

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
}
