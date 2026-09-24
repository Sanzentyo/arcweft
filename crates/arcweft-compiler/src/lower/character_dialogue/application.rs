//! Closed target value projection for a dialogue content application.

use arcweft_runtime_plan::semantic_facts::RuntimeDialogueApplicationTarget;

use super::*;

pub(in crate::lower) fn runtime_dialogue_target(
    target: &CheckedCharacterDialogueTarget,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
    enclosing: Option<ProjectInstanceTypes<'_>>,
) -> Result<RuntimeDialogueApplicationTarget, RuntimeSemanticProjectionError> {
    let expression = target.expression();
    let invalid = |reason: &str| RuntimeSemanticProjectionError::Dialogue {
        owner: Some(expression),
        reason: reason.to_owned(),
    };
    let checked = analysis
        .expression(expression)
        .ok_or_else(|| invalid("dialogue target has no checked source expression"))?;
    let source_type = runtime_type_under(
        checked_expression_type(checked, expression)?,
        enclosing,
        symbols,
        world,
        analysis,
    )?;
    let dialogue_type = runtime_type_under(
        &TypeKind::CharacterDialogue(arcweft_dialogue::CharacterDialogueType::new(
            target.character().clone(),
        )),
        enclosing,
        symbols,
        world,
        analysis,
    )?;
    if matches!(source_type.shape(), RuntimeTypeShape::EntityReference) {
        Ok(RuntimeDialogueApplicationTarget::CharacterReference {
            expression,
            source_type,
            dialogue_type,
        })
    } else if source_type == dialogue_type {
        Ok(RuntimeDialogueApplicationTarget::CharacterDialogue {
            expression,
            dialogue_type,
        })
    } else {
        Err(invalid(
            "dialogue target source type disagrees with its checked Character precision",
        ))
    }
}
