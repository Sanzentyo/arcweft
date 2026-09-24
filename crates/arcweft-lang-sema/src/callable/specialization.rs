//! Closed evidence for one function-scheme value use, independent of invocation.

use std::sync::Arc;

use arcweft_lang_hir::identity::ExprId;
use thiserror::Error;

use super::{CallConstraintInvariant, CallableConstraintApplication};
use crate::types::{
    GenericBinder, GenericScope, TypeKind, TypeProjectionControl, TypeProjectionError,
    constraints::{CompletedResultProjectionView, ConstraintDomain, TypeConstraintSolution},
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedFunctionSpecializationDigest([u8; 32]);

impl CheckedFunctionSpecializationDigest {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
    fn issue<C: TypeProjectionControl>(
        source_digest: crate::types::SemanticTypeDigest,
        target_digest: crate::types::SemanticTypeDigest,
        solution: &TypeConstraintSolution,
        control: &mut C,
    ) -> Result<Self, TypeProjectionError<C::Error>> {
        let mut digest = blake3::Hasher::new();
        digest.update(b"arcweft.lang.function-specialization.v1\0");
        digest.update(source_digest.as_bytes());
        digest.update(target_digest.as_bytes());
        digest.update(&(solution.bindings().len() as u64).to_le_bytes());
        for (key, value) in solution.bindings() {
            control
                .visit_binding()
                .map_err(TypeProjectionError::Control)?;
            digest.update(
                key.semantic_identity_digest_with_control(control)?
                    .as_bytes(),
            );
            digest.update(
                value
                    .semantic_identity_digest_with_control(control)?
                    .as_bytes(),
            );
        }
        digest.update(&(solution.const_bindings().len() as u64).to_le_bytes());
        for (key, value) in solution.const_bindings() {
            control
                .visit_binding()
                .map_err(TypeProjectionError::Control)?;
            for bytes in [
                key.canonical_checked_bytes_with_control(control)?,
                value.canonical_checked_bytes_with_control(control)?,
            ] {
                digest.update(&(bytes.len() as u64).to_le_bytes());
                digest.update(&bytes);
            }
        }
        digest.update(&(solution.effect_bindings().len() as u64).to_le_bytes());
        for (key, value) in solution.effect_bindings() {
            control
                .visit_binding()
                .map_err(TypeProjectionError::Control)?;
            digest.update(
                key.semantic_identity_digest_with_control(control)?
                    .as_bytes(),
            );
            digest.update(
                value
                    .semantic_identity_digest_with_control(control)?
                    .as_bytes(),
            );
        }
        Ok(Self(*digest.finalize().as_bytes()))
    }
}

/// The original scheme, complete specialized arrow, and substitution all come
/// from one completed component. Invocation/callee identity is not fabricated
/// for a value conversion, and the source expression remains independently typed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedFunctionSpecialization {
    owner: ExprId,
    source: TypeKind,
    specialized: TypeKind,
    solution: Arc<TypeConstraintSolution>,
    digest: CheckedFunctionSpecializationDigest,
}

#[derive(Debug, Error)]
pub(crate) enum FunctionSpecializationSealFailure<E: std::error::Error + 'static> {
    #[error(transparent)]
    Invariant(#[from] CallConstraintInvariant),
    #[error(transparent)]
    Projection(#[from] TypeProjectionError<E>),
}

impl CheckedFunctionSpecialization {
    pub(crate) fn seal<D, C>(
        result: &CompletedResultProjectionView<'_, D>,
        control: &mut C,
    ) -> Result<Arc<Self>, FunctionSpecializationSealFailure<C::Error>>
    where
        D: ConstraintDomain<Application = CallableConstraintApplication>,
        C: TypeProjectionControl,
    {
        let CallableConstraintApplication::Specialize(owner) = result.application_id() else {
            return Err(CallConstraintInvariant::PreparedCallSiteMismatch.into());
        };
        let source = result
            .projection()
            .input_type()
            .ok_or(CallConstraintInvariant::PreparedFunctionTypeMismatch)?;
        let TypeKind::Function {
            binder,
            predicate,
            params,
            return_type,
            effects,
        } = source
        else {
            return Err(CallConstraintInvariant::PreparedFunctionTypeMismatch.into());
        };
        let solution = result.application().solution();
        if binder.is_empty()
            || solution.template_scope().binders() != [*binder]
            || !result.projection().value().scope().binders().is_empty()
        {
            return Err(CallConstraintInvariant::PreparedSchemaMismatch.into());
        }
        let source_digest = source
            .semantic_identity_digest_in_scope_with_control(&GenericScope::default(), control)?;
        if let Some(origin) = result.source()
            && (origin.application_id().expression() != owner
                || origin
                    .projection()
                    .value()
                    .semantic_identity_digest_with_control(control)?
                    != source_digest)
        {
            return Err(CallConstraintInvariant::PreparedFunctionTypeMismatch.into());
        }
        // Keep the source predicate in the verification template. Substituting
        // the same closed solution must discharge it; nested binders retain
        // their own predicates through the shared capture-avoiding fold.
        let template = TypeKind::function_with_contract(
            GenericBinder::EMPTY,
            predicate.clone(),
            params.clone(),
            return_type.as_ref().clone(),
            effects.clone(),
        );
        let projected = solution.apply_template_with_control(&template, control)?;
        let projected = projected.view();
        if !projected.scope().binders().is_empty()
            || !matches!(projected.value(), TypeKind::Function { binder, predicate, .. }
                if binder.is_empty() && predicate.is_unconstrained())
        {
            return Err(CallConstraintInvariant::PreparedFunctionTypeMismatch.into());
        }
        let target_digest = result
            .projection()
            .value()
            .semantic_identity_digest_with_control(control)?;
        if projected.semantic_identity_digest_with_control(control)? != target_digest {
            return Err(CallConstraintInvariant::PreparedFunctionTypeMismatch.into());
        }
        let digest = CheckedFunctionSpecializationDigest::issue(
            source_digest,
            target_digest,
            solution,
            control,
        )?;
        Ok(Arc::new(Self {
            owner,
            source: source.clone(),
            specialized: projected.value().clone(),
            solution: Arc::clone(solution),
            digest,
        }))
    }

    pub const fn owner(&self) -> ExprId {
        self.owner
    }
    pub const fn source_type(&self) -> &TypeKind {
        &self.source
    }
    pub const fn specialized_type(&self) -> &TypeKind {
        &self.specialized
    }
    pub const fn digest(&self) -> CheckedFunctionSpecializationDigest {
        self.digest
    }

    pub(crate) fn close_arguments_with_control<C: TypeProjectionControl>(
        &self,
        enclosing: Option<&crate::types::constraints::ClosedTypeInstantiation>,
        control: &mut C,
    ) -> Result<crate::types::constraints::ClosedTypeInstantiation, TypeProjectionError<C::Error>>
    {
        self.solution
            .close_instantiation_with_control(enclosing, control)
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        visitor(&self.source)?;
        visitor(&self.specialized)?;
        for (_, value) in self.solution.bindings() {
            visitor(value.value())?;
        }
        Ok(())
    }
}
