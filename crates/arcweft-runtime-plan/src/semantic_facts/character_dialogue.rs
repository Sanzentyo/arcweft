//! Closed CharacterDialogue operations over the admitted call's source row.

use std::collections::BTreeSet;

use arcweft_interaction_model::dialogue::{
    CharacterDialogueOperation, CharacterDialoguePatchField, CharacterDialoguePatchOperation,
};

use super::{
    RuntimeCallResultShape, RuntimeNormalizedType, RuntimeResolvedCallOperand,
    RuntimeResolvedCallOperandOrigin, RuntimeResolvedCallOperandProjection, RuntimeTypeShape,
};

impl RuntimeNormalizedType {
    pub(super) fn is_character_dialogue_value(&self) -> bool {
        matches!(
            self.shape(),
            RuntimeTypeShape::Opaque {
                producer,
                value_class: arcweft_core::value::RuntimeOpaqueValueClass::Plain,
                persistence: arcweft_core::value::RuntimeOpaquePersistence::ConstantAndSnapshot,
                arguments,
                ..
            } if producer == &arcweft_core::value::RuntimeCharacterDialogueProducerId::get()
                && arguments.is_empty()
        )
    }
}

/// Compiler-selected immutable dialogue construction or reconfiguration.
///
/// `target` and every `Set` refer to the sole source-ordered physical operand
/// row. Clear operands remain in that row and are evaluated before publication.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeCharacterDialogueCall {
    operation: CharacterDialogueOperation,
    target: u32,
    fields: Box<[CharacterDialoguePatchField<u32>]>,
}

impl RuntimeCharacterDialogueCall {
    #[must_use]
    pub fn new(
        operation: CharacterDialogueOperation,
        target: u32,
        fields: impl Into<Box<[CharacterDialoguePatchField<u32>]>>,
    ) -> Self {
        Self {
            operation,
            target,
            fields: fields.into(),
        }
    }

    #[must_use]
    pub const fn operation(&self) -> CharacterDialogueOperation {
        self.operation
    }

    #[must_use]
    pub const fn target(&self) -> u32 {
        self.target
    }

    #[must_use]
    pub const fn fields(&self) -> &[CharacterDialoguePatchField<u32>] {
        &self.fields
    }

    pub(super) fn accepts_source_row(
        &self,
        operands: &[RuntimeResolvedCallOperand],
        result: RuntimeCallResultShape,
    ) -> bool {
        if result != RuntimeCallResultShape::Value
            || self.target != 0
            || operands.len().checked_sub(1) != Some(self.fields.len())
            || !matches!(
                operands.first().map(RuntimeResolvedCallOperand::origin),
                Some(RuntimeResolvedCallOperandOrigin::Callee)
            )
            || operands.iter().any(|operand| {
                !matches!(
                    operand.projection(),
                    RuntimeResolvedCallOperandProjection::Scalar
                )
            })
        {
            return false;
        }
        let mut coordinates = BTreeSet::new();
        self.fields
            .iter()
            .zip(operands.iter().enumerate().skip(1))
            .all(|(field, (source_index, operand))| {
                coordinates.insert(&field.coordinate)
                    && matches!(
                        operand.origin(),
                        RuntimeResolvedCallOperandOrigin::Argument { slot: 0, .. }
                    )
                    && match field.operation {
                        CharacterDialoguePatchOperation::Set(source) => {
                            usize::try_from(source) == Ok(source_index)
                        }
                        CharacterDialoguePatchOperation::Clear => true,
                    }
            })
    }

    pub(super) fn accepts_types(
        &self,
        operands: &[RuntimeResolvedCallOperand],
        result: Option<&RuntimeNormalizedType>,
    ) -> bool {
        let Some(result) = result else {
            return false;
        };
        if !result.is_character_dialogue_value() {
            return false;
        }
        let Some(target) = operands.first() else {
            return false;
        };
        match self.operation {
            CharacterDialogueOperation::Factory => {
                matches!(target.ty().shape(), RuntimeTypeShape::EntityReference)
            }
            CharacterDialogueOperation::Reconfigure => target.ty() == result,
        }
    }
}
