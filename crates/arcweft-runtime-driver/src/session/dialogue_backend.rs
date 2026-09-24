//! Selects the retained Dialogue producer by the exact calling executable.

use arcweft_core::{
    pattern::RuntimeSemanticTypeId,
    pure::{RuntimeExternalCallBackend, RuntimeExternalCallContext},
    task::RuntimeProgramOwner,
    value::{RuntimeCallTarget, RuntimeEvalError, RuntimeValue},
};
use arcweft_dialogue::{
    CharacterDialogueRuntimeExternalCallBackend, CharacterDialogueRuntimeSchema,
};
use arcweft_interaction_model::dialogue::{
    CharacterDialogueOperation, CharacterDialoguePatchField,
};
use std::sync::Arc;

/// The session may retain an old fiber while a replacement runtime image is
/// already available for new entries. Each producer call selects its schema
/// from the executable lease supplied by Core.
pub(super) struct GenerationDialogueBackend {
    schemas: Vec<Arc<CharacterDialogueRuntimeSchema>>,
}

impl GenerationDialogueBackend {
    pub(super) fn new(schemas: Vec<Arc<CharacterDialogueRuntimeSchema>>) -> Self {
        Self { schemas }
    }
}

impl RuntimeExternalCallBackend for GenerationDialogueBackend {
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
        let schema = self
            .schemas
            .iter()
            .map(Arc::as_ref)
            .find(|schema| schema.program_owner().same_program(owner))
            .ok_or(RuntimeEvalError::CharacterDialogueProducerUnavailable)?;
        CharacterDialogueRuntimeExternalCallBackend::new(schema).produce_character_dialogue(
            owner,
            operation,
            target,
            fields,
            result_type,
        )
    }
}
