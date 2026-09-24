//! Exact CharacterDialogue call projection into one source-ordered runtime row.

use arcweft_interaction_model::dialogue::{
    CharacterDialogueOperation, CharacterDialoguePatchField, CharacterDialoguePatchOperation,
};
use arcweft_lang_sema::{
    callable::{CallableArgumentSemanticAction, DialogueCallableId},
    final_analysis::{CheckedCharacterDialoguePatch, CheckedPatchOperation},
};
use arcweft_runtime_plan::semantic_facts::RuntimeCharacterDialogueCall;

use super::*;

#[cfg(test)]
mod tests;

pub(super) fn runtime_character_dialogue_call(
    owner: ExprId,
    application: &CheckedCallApplication,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
    enclosing: Option<ProjectInstanceTypes<'_>>,
) -> Result<Option<RuntimeResolvedCall>, RuntimeSemanticProjectionError> {
    let selected = application.core().candidates().selected();
    let CallableValidator::Dialogue(operation) = selected.schema().validator() else {
        return Ok(None);
    };
    if !matches!(
        operation,
        DialogueCallableId::CharacterFactory | DialogueCallableId::CharacterReconfigure
    ) {
        return Ok(None);
    }
    let invalid = |reason: &str| RuntimeSemanticProjectionError::Call {
        owner,
        reason: reason.to_owned(),
    };
    let checked = analysis
        .expression(owner)
        .ok_or_else(|| invalid("CharacterDialogue call has no final checked expression"))?;
    let (operation, target, patch) = match (operation, checked.resolution()) {
        (
            DialogueCallableId::CharacterFactory,
            CheckedExpressionResolution::CharacterDialogueFactory(factory),
        ) => (
            CharacterDialogueOperation::Factory,
            factory.target(),
            factory.patch(),
        ),
        (
            DialogueCallableId::CharacterReconfigure,
            CheckedExpressionResolution::CharacterDialogueReconfigure(reconfigure),
        ) => (
            CharacterDialogueOperation::Reconfigure,
            reconfigure.target(),
            reconfigure.patch(),
        ),
        _ => {
            return Err(invalid(
                "CharacterDialogue selection and checked operation disagree",
            ));
        }
    };
    if !matches!(
        application.result(),
        arcweft_lang_sema::callable::CheckedCallResult::Value(_)
    ) {
        return Err(invalid(
            "CharacterDialogue operation cannot publish a callable continuation",
        ));
    }
    let target_ty = analysis
        .expression(target.expression())
        .ok_or_else(|| invalid("CharacterDialogue target has no checked type"))?;
    let target_ty = runtime_type_under(
        checked_expression_type(target_ty, target.expression())?,
        enclosing,
        symbols,
        world,
        analysis,
    )?;
    let mut operands = vec![RuntimeResolvedCallOperand::new(
        0,
        RuntimeResolvedCallOperandOrigin::Callee,
        RuntimeResolvedCallOperandSource::Expression(target.expression()),
        target_ty,
        RuntimeResolvedCallOperandBinding::Positional,
        RuntimeResolvedCallOperandProjection::Scalar,
        None,
    )];
    let fields = project_patch_operands(
        owner,
        application,
        patch,
        symbols,
        world,
        analysis,
        enclosing,
        &mut operands,
    )?;
    let call = RuntimeResolvedCall::try_new(
        RuntimeResolvedCallDispatch::Static(RuntimeResolvedStaticCallTarget::CharacterDialogue(
            RuntimeCharacterDialogueCall::new(operation, 0, fields),
        )),
        application.core().current_group(),
        operands,
        None,
        None,
        RuntimeCallResultShape::Value,
    )
    .map_err(|error| invalid(&error.to_string()))?;
    Ok(Some(call))
}

#[allow(
    clippy::too_many_arguments,
    reason = "one compiler context closes the selected patch and its source operands atomically"
)]
fn project_patch_operands(
    owner: ExprId,
    application: &CheckedCallApplication,
    patch: &CheckedCharacterDialoguePatch,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
    enclosing: Option<ProjectInstanceTypes<'_>>,
    operands: &mut Vec<RuntimeResolvedCallOperand>,
) -> Result<Box<[CharacterDialoguePatchField<u32>]>, RuntimeSemanticProjectionError> {
    let invalid = |reason: &str| RuntimeSemanticProjectionError::Call {
        owner,
        reason: reason.to_owned(),
    };
    let selected = application.core().candidates().selected();
    let source_row = application.core().runtime_operands();
    if source_row.len() != patch.fields().len() {
        return Err(invalid(
            "CharacterDialogue patch does not cover its complete runtime source row",
        ));
    }
    patch
        .fields()
        .iter()
        .zip(source_row.iter())
        .map(|(field, operand)| {
            let CheckedCallRuntimeOperand::Argument {
                argument,
                passing,
                slot,
            } = operand
            else {
                return Err(invalid(
                    "CharacterDialogue patch has a non-argument runtime operand",
                ));
            };
            let source_index = u32::try_from(operands.len())
                .map_err(|_| invalid("CharacterDialogue source operand index exceeds u32"))?;
            let operation = match (field.operation(), slot.semantic_action(selected)) {
                (
                    CheckedPatchOperation::Set { value, .. },
                    Some(CallableArgumentSemanticAction::Supply),
                ) if slot.source().raw()
                    == arcweft_lang_sema::callable::CheckedCallArgumentSlotSource::Expression(
                        *value,
                    ) =>
                {
                    CharacterDialoguePatchOperation::Set(source_index)
                }
                (CheckedPatchOperation::Clear, Some(CallableArgumentSemanticAction::Clear)) => {
                    CharacterDialoguePatchOperation::Clear
                }
                _ => {
                    return Err(invalid(
                        "CharacterDialogue patch operation disagrees with its selected operand",
                    ));
                }
            };
            let abi_position = slot
                .abi_position()
                .checked_add(1)
                .ok_or_else(|| invalid("CharacterDialogue operand ABI position exceeds u32"))?;
            let projection =
                runtime_call_operand_projection(owner, slot, symbols, world, analysis, enclosing)?;
            if !matches!(projection, RuntimeResolvedCallOperandProjection::Scalar) {
                return Err(invalid(
                    "CharacterDialogue patch operand is not a scalar contribution",
                ));
            }
            let parameter = match slot.destination() {
                CheckedCallOperandDestination::Parameter(coordinate) => {
                    Some(RuntimeCallParameterCoordinate::new(
                        u32::try_from(coordinate.group().get()).map_err(|_| {
                            invalid("CharacterDialogue parameter group exceeds u32")
                        })?,
                        u32::try_from(coordinate.parameter().get()).map_err(|_| {
                            invalid("CharacterDialogue parameter index exceeds u32")
                        })?,
                    ))
                }
                CheckedCallOperandDestination::Open(_) => None,
            };
            operands.push(RuntimeResolvedCallOperand::new(
                abi_position,
                RuntimeResolvedCallOperandOrigin::Argument {
                    argument: u32::from(argument.get()),
                    slot: u32::try_from(slot.slot().get())
                        .map_err(|_| invalid("CharacterDialogue argument slot exceeds u32"))?,
                },
                runtime_call_operand_source(slot.source().raw()),
                runtime_type_under(slot.inferred(), enclosing, symbols, world, analysis)?,
                runtime_call_operand_binding(owner, selected, *passing, slot)?,
                projection,
                parameter,
            ));
            Ok(CharacterDialoguePatchField {
                coordinate: field.coordinate().clone(),
                operation,
            })
        })
        .collect()
}
