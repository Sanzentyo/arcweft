//! Core runtime-call adapter for the accepted `CharacterDialogue` generation.

use super::CharacterDialogueRuntimeSchema;
use arcweft_core::{
    pattern::RuntimeSemanticTypeId,
    pure::{RuntimeExternalCallBackend, RuntimeExternalCallContext},
    task::RuntimeProgramOwner,
    value::{RuntimeCallTarget, RuntimeEvalError, RuntimeValue},
};
use arcweft_interaction_model::dialogue::{
    CharacterDialogueOperation, CharacterDialoguePatchField,
};

/// Core runtime backend adapter backed by one accepted `CharacterDialogue` schema.
///
/// The schema retains the exact executable lease and generation authority. This
/// adapter delegates production to it directly, so it never resolves dialogue
/// types or defaults from source spelling.
#[derive(Clone, Copy)]
pub struct CharacterDialogueRuntimeExternalCallBackend<'a> {
    schema: &'a CharacterDialogueRuntimeSchema,
}

impl<'a> CharacterDialogueRuntimeExternalCallBackend<'a> {
    /// Binds the adapter to the accepted `CharacterDialogue` generation.
    #[must_use]
    pub const fn new(schema: &'a CharacterDialogueRuntimeSchema) -> Self {
        Self { schema }
    }
}

impl RuntimeExternalCallBackend for CharacterDialogueRuntimeExternalCallBackend<'_> {
    fn call_external(
        &mut self,
        _context: &RuntimeExternalCallContext,
        _callee: &RuntimeCallTarget,
        _args: &[RuntimeValue],
    ) -> Option<Result<RuntimeValue, RuntimeEvalError>> {
        None
    }

    fn produce_character_dialogue(
        &mut self,
        owner: &RuntimeProgramOwner,
        operation: CharacterDialogueOperation,
        target: RuntimeValue,
        fields: &[CharacterDialoguePatchField<RuntimeValue>],
        result_type: RuntimeSemanticTypeId,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        self.schema
            .apply(owner, operation, target, fields, result_type)
            .map_err(|error| RuntimeEvalError::CharacterDialogueConstruction(error.to_string()))
    }
}
