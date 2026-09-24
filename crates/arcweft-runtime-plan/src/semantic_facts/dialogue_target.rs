//! Exact source value consumed before one dialogue application's content.

use std::collections::BTreeMap;

use arcweft_lang_hir::{
    dialogue_application::HirAttachedContentApplicationFamily,
    expr::HirExprKind,
    identity::{ExprId, HirModuleId},
    module::HirModule,
};

use super::{
    RuntimeDialogueApplication, RuntimeNormalizedType, RuntimeSemanticFactsError, RuntimeTypeShape,
    resolve_expr, validate_normalized_type,
};

impl RuntimeDialogueApplication {
    pub(super) fn validate_target(
        &self,
        modules: &BTreeMap<HirModuleId, &HirModule>,
        application: ExprId,
        source_type: Option<&RuntimeNormalizedType>,
    ) -> Result<(), RuntimeSemanticFactsError> {
        self.target().validate(modules, application, source_type)?;
        if let arcweft_dialogue::character_presentation::CharacterPresentationTargetEvidence::Exact(
            character,
        ) = self.content().character().target()
            && self.target().dialogue_type().identity()
                != arcweft_dialogue::CharacterDialogueType::exact(character.clone())
                    .runtime_semantic_identity()
        {
            return Err(RuntimeSemanticFactsError::DialogueTargetMismatch {
                expression: application,
                target: self.target().expression(),
            });
        }
        Ok(())
    }
}

/// Checked target of one dialogue application, before its value is materialized.
/// A Character reference is consumed by the ordinary empty factory operation;
/// a CharacterDialogue value retains its exact or producer-wide source type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeDialogueApplicationTarget {
    CharacterReference {
        expression: ExprId,
        source_type: RuntimeNormalizedType,
        dialogue_type: RuntimeNormalizedType,
    },
    CharacterDialogue {
        expression: ExprId,
        dialogue_type: RuntimeNormalizedType,
    },
}

impl RuntimeDialogueApplicationTarget {
    #[must_use]
    pub const fn expression(&self) -> ExprId {
        match self {
            Self::CharacterReference { expression, .. }
            | Self::CharacterDialogue { expression, .. } => *expression,
        }
    }

    #[must_use]
    pub const fn source_type(&self) -> &RuntimeNormalizedType {
        match self {
            Self::CharacterReference { source_type, .. } => source_type,
            Self::CharacterDialogue { dialogue_type, .. } => dialogue_type,
        }
    }

    #[must_use]
    pub const fn dialogue_type(&self) -> &RuntimeNormalizedType {
        match self {
            Self::CharacterReference { dialogue_type, .. }
            | Self::CharacterDialogue { dialogue_type, .. } => dialogue_type,
        }
    }

    pub(super) fn has_valid_types(&self) -> bool {
        self.dialogue_type().is_character_dialogue_value()
            && match self {
                Self::CharacterReference { source_type, .. } => {
                    matches!(source_type.shape(), RuntimeTypeShape::EntityReference)
                }
                Self::CharacterDialogue { .. } => true,
            }
    }

    pub(super) fn validate(
        &self,
        modules: &BTreeMap<HirModuleId, &HirModule>,
        application: ExprId,
        source_type: Option<&RuntimeNormalizedType>,
    ) -> Result<(), RuntimeSemanticFactsError> {
        let invalid = || RuntimeSemanticFactsError::DialogueTargetMismatch {
            expression: application,
            target: self.expression(),
        };
        let HirExprKind::AttachedContentApplication(hir) = resolve_expr(modules, application)?
        else {
            return Err(invalid());
        };
        let HirAttachedContentApplicationFamily::DialogueLine { target, .. } = hir.family() else {
            return Err(invalid());
        };
        if *target != self.expression()
            || source_type != Some(self.source_type())
            || !self.has_valid_types()
        {
            return Err(invalid());
        }
        validate_normalized_type(modules, self.source_type())?;
        validate_normalized_type(modules, self.dialogue_type())
    }
}
