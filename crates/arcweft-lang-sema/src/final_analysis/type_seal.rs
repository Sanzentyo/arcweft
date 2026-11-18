//! Canonical publication borrows the actual producer scope and completed effect relation.

use std::sync::Arc;

use arcweft_lang_hir::project::{HirProjectEvaluationTopology, HirSemanticPathOwnerId};

use super::{
    FinalSemanticAnalysisError, SourceTypeRetentionFailure, analyzer::CallableEffectClosureWork,
};
use crate::{
    callable::CheckedCallableCatalog,
    effect_row::prepared::PrincipalEffectClosure,
    types::{
        GenericScope, PreparedEffects, SemanticTypeDigest, TypeInstantiationError, TypeKind,
        TypeProjectionControl, TypeProjectionError,
    },
};

pub(super) struct PublicationTypeEffects<'a, 'control> {
    pub(super) principal: &'a PrincipalEffectClosure,
    pub(super) work: &'a mut CallableEffectClosureWork<'control>,
}

pub(super) struct PublicationTypes<'a, 'control> {
    callables: &'a CheckedCallableCatalog,
    topology: &'a Arc<HirProjectEvaluationTopology>,
    effects: PublicationTypeEffects<'a, 'control>,
}

impl<'a, 'control> PublicationTypes<'a, 'control> {
    pub(super) fn new(
        callables: &'a CheckedCallableCatalog,
        topology: &'a Arc<HirProjectEvaluationTopology>,
        effects: PublicationTypeEffects<'a, 'control>,
    ) -> Self {
        Self {
            callables,
            topology,
            effects,
        }
    }

    pub(super) fn source(
        &mut self,
        owner: impl Into<HirSemanticPathOwnerId>,
    ) -> Result<PublicationSourceTypes<'_, 'control>, FinalSemanticAnalysisError> {
        let owner = owner.into();
        self.effects.work.visit_binding()?;
        let source = self
            .callables
            .constraint_source_owner(self.topology, owner)
            .map_err(|error| FinalSemanticAnalysisError::SourceTypeRetention {
                owner,
                error: SourceTypeRetentionFailure::Owner(error),
            })?;
        let binder = source
            .declaration_formals_with_control(self.effects.work)
            .map_err(|error| FinalSemanticAnalysisError::source_type_projection(owner, error))?;
        Ok(PublicationSourceTypes {
            owner,
            binder,
            principal: self.effects.principal,
            work: self.effects.work,
        })
    }

    pub(super) fn visit_binding(&mut self) -> Result<(), FinalSemanticAnalysisError> {
        self.effects.work.visit_binding()
    }
}

pub(super) struct PublicationSourceTypes<'a, 'control> {
    owner: HirSemanticPathOwnerId,
    binder: crate::types::GenericDeclarationBinder,
    principal: &'a PrincipalEffectClosure,
    work: &'a mut CallableEffectClosureWork<'control>,
}

impl PublicationSourceTypes<'_, '_> {
    pub(super) const fn scope(&self) -> &GenericScope {
        self.binder.template_scope()
    }

    pub(super) fn canonical(
        &mut self,
        ty: &TypeKind<PreparedEffects>,
    ) -> Result<TypeKind, FinalSemanticAnalysisError> {
        ty.project_canonical_effects(self.binder.template_scope(), self.principal, self.work)
            .map_err(|error| {
                FinalSemanticAnalysisError::source_type_effect_projection(self.owner, error)
            })
    }

    /// A completed call can issue a new type for its expression after the
    /// principal relation has closed. Retaining it adds no open effect terms.
    pub(super) fn retain_closed(
        &mut self,
        ty: &TypeKind,
    ) -> Result<TypeKind<PreparedEffects>, FinalSemanticAnalysisError> {
        ty.prepare_effects(self.binder.template_scope(), self.work)
            .map_err(|error| FinalSemanticAnalysisError::source_type_projection(self.owner, error))
    }

    pub(super) fn control(
        &mut self,
    ) -> &mut impl TypeProjectionControl<Error = FinalSemanticAnalysisError> {
        self.work
    }

    pub(super) fn digest(
        &mut self,
        ty: &TypeKind,
    ) -> Result<SemanticTypeDigest, FinalSemanticAnalysisError> {
        let scope = self.binder.template_scope();
        let root =
            ty.semantic_identity_digest_in_scope_with_control(&GenericScope::default(), self.work);
        match root {
            Err(TypeProjectionError::Instantiation(TypeInstantiationError::Scope(_)))
                if !scope.binders().is_empty() =>
            {
                ty.semantic_identity_digest_in_scope_with_control(scope, self.work)
            }
            result => result,
        }
        .map_err(|error| FinalSemanticAnalysisError::source_type_projection(self.owner, error))
    }
}
