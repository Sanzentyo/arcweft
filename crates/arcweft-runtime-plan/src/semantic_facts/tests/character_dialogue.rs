use super::*;
use crate::semantic_facts::RuntimeCharacterDialogueCall;
use arcweft_interaction_model::dialogue::{
    CharacterDialogueFieldCoordinate as Field, CharacterDialogueOperation,
    CharacterDialoguePatchField, CharacterDialoguePatchOperation as Patch,
};

fn source_row(source: arcweft_lang_hir::identity::ExprId) -> Vec<RuntimeResolvedCallOperand> {
    [
        RuntimeResolvedCallOperandOrigin::Callee,
        RuntimeResolvedCallOperandOrigin::Argument {
            argument: 0,
            slot: 0,
        },
        RuntimeResolvedCallOperandOrigin::Argument {
            argument: 1,
            slot: 0,
        },
    ]
    .into_iter()
    .enumerate()
    .map(|(index, origin)| {
        RuntimeResolvedCallOperand::new(
            u32::try_from(index).unwrap(),
            origin,
            RuntimeResolvedCallOperandSource::Expression(source),
            unit_type(),
            RuntimeResolvedCallOperandBinding::Positional,
            RuntimeResolvedCallOperandProjection::Scalar,
            None,
        )
    })
    .collect()
}

fn call(
    operands: Vec<RuntimeResolvedCallOperand>,
    fields: Vec<CharacterDialoguePatchField<u32>>,
) -> Result<RuntimeResolvedCall, RuntimeResolvedCallError> {
    RuntimeResolvedCall::try_new(
        RuntimeResolvedCallDispatch::Static(RuntimeResolvedStaticCallTarget::CharacterDialogue(
            RuntimeCharacterDialogueCall::new(CharacterDialogueOperation::Reconfigure, 0, fields),
        )),
        arcweft_lang_sema::callable::CallableGroupIndex::try_from_usize(0).unwrap(),
        operands,
        None,
        None,
        RuntimeCallResultShape::Value,
    )
}

fn fields() -> Vec<CharacterDialoguePatchField<u32>> {
    vec![
        CharacterDialoguePatchField {
            coordinate: Field::SourceLocale,
            operation: Patch::Set(1),
        },
        CharacterDialoguePatchField {
            coordinate: Field::Voice,
            operation: Patch::Clear,
        },
    ]
}

#[test]
fn character_dialogue_retains_callee_and_clear_operands_in_source_order() {
    let project = project_fixture("dialogue-source-order", "fn root() { true }\n");
    let source = boolean_literal(&project);
    let row = source_row(source);
    let call = call(row.clone(), fields()).expect("complete source row is admitted");
    assert_eq!(call.operands(), row);
    assert!(call.requires_specialized_operand_anf());
    assert!(call.evaluates_callee(source));
    let RuntimeResolvedCallDispatch::Static(RuntimeResolvedStaticCallTarget::CharacterDialogue(
        dialogue,
    )) = call.dispatch()
    else {
        panic!("closed dialogue operation is retained");
    };
    assert_eq!(dialogue.fields(), fields());
}

#[test]
fn character_dialogue_rejects_dropped_or_reassigned_source_operands() {
    let project = project_fixture("dialogue-source-coverage", "fn root() { true }\n");
    let row = source_row(boolean_literal(&project));
    let mut missing_clear = row.clone();
    missing_clear.pop();
    assert_eq!(
        call(missing_clear, fields()),
        Err(RuntimeResolvedCallError::CharacterDialogueSourceRow)
    );
    let mut reassigned = fields();
    reassigned[0].operation = Patch::Set(2);
    assert_eq!(
        call(row.clone(), reassigned),
        Err(RuntimeResolvedCallError::CharacterDialogueSourceRow)
    );
    let mut duplicate = fields();
    duplicate[1].coordinate = Field::SourceLocale;
    assert_eq!(
        call(row, duplicate),
        Err(RuntimeResolvedCallError::CharacterDialogueSourceRow)
    );
}

#[test]
fn ordinary_runtime_calls_cannot_admit_a_producer_callee_row() {
    let project = project_fixture("dialogue-callee-domain", "fn root() { true }\n");
    let source = boolean_literal(&project);
    assert_eq!(
        RuntimeResolvedCall::try_new(
            RuntimeResolvedCallDispatch::Value { callee: source },
            arcweft_lang_sema::callable::CallableGroupIndex::try_from_usize(0).unwrap(),
            source_row(source),
            None,
            None,
            RuntimeCallResultShape::Value,
        ),
        Err(RuntimeResolvedCallError::CharacterDialogueSourceRow),
    );
}
