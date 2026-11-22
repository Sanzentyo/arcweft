//! A typed pattern retains its annotation owner until the joint type seal.

use std::collections::BTreeMap;

use arcweft_lang_hir::{expr::HirExprKind, identity::TypeId};

use crate::final_analysis::{
    CheckedPattern, CheckedPatternResolution, CheckedTypedBinding, FinalSemanticAnalysisError,
    type_seal::PublicationSourceTypes,
};
use crate::types::{PreparedEffects, TypeKind};

/// The value a `let` binds from its initializer. Executing a Dialogue line
/// publishes an output that can originate from any of its result selectors.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum InitializerBindingProjection {
    Value,
    DialogueResult,
}

impl InitializerBindingProjection {
    pub(crate) const fn from_initializer(kind: &HirExprKind) -> Self {
        if matches!(kind, HirExprKind::AttachedContentApplication(application) if application.is_dialogue_line())
        {
            Self::DialogueResult
        } else {
            Self::Value
        }
    }

    pub(crate) fn initializer_expectation(
        self,
        binding: TypeKind<PreparedEffects>,
    ) -> TypeKind<PreparedEffects> {
        match self {
            Self::Value => binding,
            Self::DialogueResult => TypeKind::DialogueLine(Box::new(binding)),
        }
    }

    pub(crate) fn binding_type(
        self,
        initializer: &TypeKind<PreparedEffects>,
    ) -> Option<&TypeKind<PreparedEffects>> {
        match (self, initializer) {
            (Self::Value, value) => Some(value),
            (Self::DialogueResult, TypeKind::DialogueLine(result)) => Some(result),
            (Self::DialogueResult, _) => None,
        }
    }
}

/// Source evidence only. The annotation is an edge to the original accepted
/// type owner, not a digest or an independently completed copy of that type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedTypedBindingPattern {
    annotation: TypeId,
    scrutinee: TypeKind<PreparedEffects>,
}

impl PreparedTypedBindingPattern {
    pub(crate) const fn new(annotation: TypeId, scrutinee: TypeKind<PreparedEffects>) -> Self {
        Self {
            annotation,
            scrutinee,
        }
    }

    pub(crate) const fn ty(&self) -> &TypeKind<PreparedEffects> {
        &self.scrutinee
    }

    pub(crate) const fn annotation(&self) -> TypeId {
        self.annotation
    }

    pub(in crate::final_analysis) fn seal(
        &self,
        resolved_types: &BTreeMap<TypeId, TypeKind>,
        types: &mut PublicationSourceTypes<'_, '_>,
    ) -> Result<CheckedPattern, FinalSemanticAnalysisError> {
        let annotation = resolved_types.get(&self.annotation).ok_or(
            FinalSemanticAnalysisError::TypeResolutionFailed {
                owner: self.annotation,
            },
        )?;
        let scrutinee = types.canonical(&self.scrutinee)?;
        let binding = CheckedTypedBinding::try_new(annotation.clone(), &scrutinee)
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        Ok(CheckedPattern::new(
            scrutinee,
            CheckedPatternResolution::TypedBinding(binding),
        ))
    }
}
