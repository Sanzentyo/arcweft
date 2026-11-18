//! A typed pattern retains its annotation owner until the joint type seal.

use std::collections::BTreeMap;

use arcweft_lang_hir::identity::TypeId;

use crate::final_analysis::{
    CheckedPattern, CheckedPatternResolution, CheckedTypedBinding, FinalSemanticAnalysisError,
    type_seal::PublicationSourceTypes,
};
use crate::types::{PreparedEffects, TypeKind};

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
