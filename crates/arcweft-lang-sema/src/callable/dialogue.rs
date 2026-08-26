//! First-class `CharacterDialogue` callable identities and schema contexts.

use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;

use super::{CallablePath, CallableSchemaError, CallableSignatureSchema};
use crate::{
    character_dialogue::CharacterDialogueCustomFieldRegistry,
    types::{CharacterDialogueCharacterType, TypeKind},
};

/// Closed language-owned `CharacterDialogue` operation family.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DialogueCallableId {
    CharacterFactory,
    CharacterReconfigure,
    ContentApplication,
    ContentCall,
}

/// Parenthesized schema context selected by the structural source owner.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CharacterDialoguePatchContext {
    ReusableValue,
    ImmediateContentApplication,
}

/// Typed callee identity consumed by the shared callable resolver.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DialogueCalleeIdentity {
    Character {
        character: CharacterDialogueCharacterType,
    },
    CharacterDialogue {
        character: CharacterDialogueCharacterType,
    },
    Content {
        path: CallablePath,
    },
}

/// Accepted-world inputs required to materialize one context-dependent schema.
#[derive(Clone, Copy, Debug)]
pub struct DialogueSchemaContext<'a> {
    pub callee: &'a DialogueCalleeIdentity,
    pub module: &'a CanonicalModulePath,
    pub custom_fields: &'a CharacterDialogueCustomFieldRegistry,
    pub patch_context: CharacterDialoguePatchContext,
    pub result: DialogueCallableResultContext<'a>,
}

/// Exact result authority supplied before a Dialogue callable schema is
/// constructed. Content applications specialize their schema from the checked
/// line-plan result; every other Dialogue callable uses its closed declaration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DialogueCallableResultContext<'a> {
    Declared,
    ContentApplication { line_result: &'a TypeKind },
}

impl PartialEq for DialogueSchemaContext<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.callee == other.callee
            && self.module == other.module
            && self.patch_context == other.patch_context
            && self.result == other.result
            && std::ptr::eq(self.custom_fields, other.custom_fields)
    }
}

impl Eq for DialogueSchemaContext<'_> {}

impl DialogueCallableId {
    #[must_use]
    pub const fn supports_callee(self, callee: &DialogueCalleeIdentity) -> bool {
        matches!(
            (self, callee),
            (
                Self::CharacterFactory,
                DialogueCalleeIdentity::Character { .. }
            ) | (
                Self::CharacterReconfigure,
                DialogueCalleeIdentity::CharacterDialogue { .. }
            ) | (
                Self::ContentApplication,
                DialogueCalleeIdentity::Character { .. }
                    | DialogueCalleeIdentity::CharacterDialogue { .. }
            ) | (Self::ContentCall, DialogueCalleeIdentity::Content { .. })
        )
    }

    #[must_use]
    pub const fn resolve(callee: &DialogueCalleeIdentity) -> Self {
        match callee {
            DialogueCalleeIdentity::Character { .. } => Self::CharacterFactory,
            DialogueCalleeIdentity::CharacterDialogue { .. } => Self::CharacterReconfigure,
            DialogueCalleeIdentity::Content { .. } => Self::ContentCall,
        }
    }

    pub fn signature_schema(
        self,
        context: DialogueSchemaContext<'_>,
    ) -> Result<CallableSignatureSchema, CallableSchemaError> {
        super::schema::dialogue_schema(self, context)
    }
}
