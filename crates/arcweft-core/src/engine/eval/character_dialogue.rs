use super::*;
use arcweft_interaction_model::dialogue::{
    CharacterDialogueOperation, CharacterDialoguePatchField, CharacterDialoguePatchOperation,
};

impl Engine {
    pub(super) fn evaluate_character_dialogue_expr(
        &mut self,
        result_type: crate::runtime_id::RuntimePlanTypeId,
        operation: CharacterDialogueOperation,
        target: &RuntimeExpr,
        fields: &[CharacterDialoguePatchField<RuntimeExpr>],
        backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let target = self.evaluate_expr_with_backend(target, backend)?;
        let mut evaluated = Vec::with_capacity(fields.len());
        for field in fields {
            let operation = match &field.operation {
                CharacterDialoguePatchOperation::Set(expression) => {
                    CharacterDialoguePatchOperation::Set(
                        self.evaluate_expr_with_backend(expression, backend)?,
                    )
                }
                CharacterDialoguePatchOperation::Clear => CharacterDialoguePatchOperation::Clear,
            };
            evaluated.push(CharacterDialoguePatchField {
                coordinate: field.coordinate.clone(),
                operation,
            });
        }
        let semantic_type = self
            .plan
            .type_table()
            .get(result_type)
            .ok_or(RuntimeEvalError::UnknownPlanType(result_type))?
            .semantic_identity();
        let owner = crate::task::RuntimeProgramOwner::Plan(Arc::clone(&self.plan));
        let value = backend.produce_character_dialogue(
            &owner,
            operation,
            target,
            &evaluated,
            semantic_type,
        )?;
        if !self.plan.value_matches_type(result_type, &value)? {
            return Err(RuntimeEvalError::InvalidExpressionType(result_type));
        }
        Ok(value)
    }
}
