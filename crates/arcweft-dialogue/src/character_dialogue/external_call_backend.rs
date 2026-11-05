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

impl RuntimeSchemas<'_> {
    fn for_program(
        &self,
        owner: &RuntimeProgramOwner,
    ) -> Result<&CharacterDialogueRuntimeSchema, RuntimeEvalError> {
        match self {
            Self::Borrowed(schema) => Ok(schema),
            Self::Retained(schemas) => schemas
                .iter()
                .map(Arc::as_ref)
                .find(|schema| schema.program_owner().same_program(owner))
                .ok_or(RuntimeEvalError::CharacterDialogueProducerUnavailable),
        }
    }
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
        context: &RuntimeExternalCallContext,
        callee: &RuntimeCallTarget,
        args: &[RuntimeValue],
    ) -> Option<Result<RuntimeValue, RuntimeEvalError>> {
        let RuntimeCallTarget::External(callable) = callee else {
            return None;
        };
        let factory = super::CharacterDialogueStyleFactory::for_callable(callable)?;
        Some((|| {
            let owner = context
                .program_owner()
                .ok_or(RuntimeEvalError::CharacterDialogueProducerUnavailable)?;
            let schema = self.schemas.for_program(&owner)?;
            factory
                .construct(schema, context, callable, args)
                .map_err(|error| RuntimeEvalError::CharacterDialogueConstruction(error.to_string()))
        })())
    }

    fn produce_character_dialogue(
        &mut self,
        owner: &RuntimeProgramOwner,
        operation: CharacterDialogueOperation,
        target: RuntimeValue,
        fields: &[CharacterDialoguePatchField<RuntimeValue>],
        result_type: RuntimeSemanticTypeId,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let schema = self.schemas.for_program(owner)?;
        schema
            .apply(owner, operation, target, fields, result_type)
            .map_err(|error| RuntimeEvalError::CharacterDialogueConstruction(error.to_string()))
    }
}
