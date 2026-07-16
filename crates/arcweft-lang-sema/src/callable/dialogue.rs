//! Dialogue callable identities.

use arcweft_character::id::CharacterId;

use super::{CallablePath, CallableSchemaError, CallableSignatureSchema};
use crate::env::RegisteredTypeCheckEnv;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DialogueCallableId {
    SpeakerLine,
    ContentCall,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DialogueCalleeIdentity {
    Speaker { character: CharacterId },
    SpeakerPreset { character: CharacterId },
    Content { path: CallablePath },
}

#[derive(Clone, Debug)]
pub struct DialogueSchemaContext<'a> {
    pub callee: &'a DialogueCalleeIdentity,
    pub environment: &'a RegisteredTypeCheckEnv,
}

impl PartialEq for DialogueSchemaContext<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.callee == other.callee && std::ptr::eq(self.environment, other.environment)
    }
}

impl Eq for DialogueSchemaContext<'_> {}

impl DialogueCallableId {
    pub const fn resolve(callee: &DialogueCalleeIdentity) -> Self {
        match callee {
            DialogueCalleeIdentity::Speaker { .. }
            | DialogueCalleeIdentity::SpeakerPreset { .. } => Self::SpeakerLine,
            DialogueCalleeIdentity::Content { .. } => Self::ContentCall,
        }
    }

    #[allow(
        clippy::needless_pass_by_value,
        reason = "the exact public schema API owns its lightweight context value"
    )]
    pub fn signature_schema(
        self,
        context: DialogueSchemaContext<'_>,
    ) -> Result<CallableSignatureSchema, CallableSchemaError> {
        super::schema::dialogue_schema(self, context.callee)
    }
}
