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
use std::sync::Arc;

/// Core runtime backend adapter backed by one accepted `CharacterDialogue` schema.
///
/// The schema retains the exact executable lease and generation authority. This
/// adapter delegates production to it directly, so it never resolves dialogue
/// types or defaults from source spelling.
#[derive(Clone)]
pub struct CharacterDialogueRuntimeExternalCallBackend<'a> {
    schemas: RuntimeSchemas<'a>,
}

#[derive(Clone)]
enum RuntimeSchemas<'a> {
    Borrowed(&'a CharacterDialogueRuntimeSchema),
    Retained(Vec<Arc<CharacterDialogueRuntimeSchema>>),
}

impl<'a> CharacterDialogueRuntimeExternalCallBackend<'a> {
    /// Binds the adapter to the accepted `CharacterDialogue` generation.
    #[must_use]
    pub const fn new(schema: &'a CharacterDialogueRuntimeSchema) -> Self {
        Self {
            schemas: RuntimeSchemas::Borrowed(schema),
        }
    }

    /// Retains accepted generations and selects each producer by the exact
    /// calling executable lease. An old fiber may keep using its generation
    /// after a replacement becomes available for new entries.
    #[must_use]
    pub fn with_generations(schemas: Vec<Arc<CharacterDialogueRuntimeSchema>>) -> Self {
        Self {
            schemas: RuntimeSchemas::Retained(schemas),
        }
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
        let schema = match &self.schemas {
            RuntimeSchemas::Borrowed(schema) => *schema,
            RuntimeSchemas::Retained(schemas) => schemas
                .iter()
                .map(Arc::as_ref)
                .find(|schema| schema.program_owner().same_program(owner))
                .ok_or(RuntimeEvalError::CharacterDialogueProducerUnavailable)?,
        };
        schema
            .apply(owner, operation, target, fields, result_type)
            .map_err(|error| RuntimeEvalError::CharacterDialogueConstruction(error.to_string()))
    }
}
