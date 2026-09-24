//! Scheme uses enter the current source's component as deferred value results.

use super::*;
use crate::callable::{CallableLimits, EnclosingGenericParameterScope, PreparedCallGraph};
use crate::types::{GenericScope, TypeProjectionError};

pub(crate) enum FunctionSpecializationFailure<D: ConstraintDomain> {
    Prepared(CallConstraintInvariant),
    Constraint(TypeConstraintFailure<D>),
}

impl<'control, D: ConstraintDomain> CandidateConstraintSourceContext<'_, 'control, D> {
    /// Opens a known scheme returned by an in-flight child. Each alternative
    /// uses its own head premise and complete child path; the parent still
    /// closes both the producing call and the specialization together.
    pub(crate) fn specialize_pending_function_value<P, U>(
        &mut self,
        graph: &PreparedCallGraph<P, U>,
        application: D::Application,
        pending: PendingChildConstraint<D>,
        enclosing: &EnclosingGenericParameterScope,
        limits: &CallableLimits,
        result: D::Projection,
    ) -> Result<PendingChildConstraint<D>, FunctionSpecializationFailure<D>>
    where
        D::Projection: Clone,
    {
        graph
            .validate_constraint_authority(self.authority)
            .map_err(FunctionSpecializationFailure::Prepared)?;
        let receipt = self.ticket.receipt();
        let alternatives = pending
            .into_results(&receipt, self.context)
            .map_err(|error| FunctionSpecializationFailure::Constraint(error.into()))?;
        let mut transformed: Option<PendingChildConstraint<D>> = None;
        for alternative in alternatives {
            let source = alternative.source_type();
            source
                .semantic_identity_digest_in_scope_with_control(
                    &GenericScope::default(),
                    self.context,
                )
                .map_err(|error| match error {
                    TypeProjectionError::Control(error) => {
                        FunctionSpecializationFailure::Constraint(error.into())
                    }
                    TypeProjectionError::Instantiation(error) => {
                        FunctionSpecializationFailure::Prepared(error.into())
                    }
                })?;
            let initialization = graph
                .issue_function_specialization(
                    self.authority,
                    receipt.clone(),
                    application,
                    source,
                    enclosing,
                    limits,
                )
                .map_err(FunctionSpecializationFailure::Prepared)?;
            let (application, parameters, template) = initialization
                .into_lower_parts(self.authority, &receipt)
                .map_err(FunctionSpecializationFailure::Prepared)?;
            let next = alternative
                .transform(
                    self.context,
                    application,
                    parameters,
                    &template,
                    result.clone(),
                )
                .map_err(FunctionSpecializationFailure::Constraint)?;
            match &mut transformed {
                Some(transformed) => transformed
                    .append(next)
                    .map_err(|error| FunctionSpecializationFailure::Constraint(error.into()))?,
                None => transformed = Some(next),
            }
        }
        transformed.ok_or(FunctionSpecializationFailure::Prepared(
            CallConstraintInvariant::PreparedCallSiteMismatch,
        ))
    }

    /// Opens only the source function's outer binder. The returned port is
    /// related to the parent's expectation by the normal source observation
    /// protocol; no child solution seals before later parent operands arrive.
    pub(crate) fn specialize_function_value<P, U>(
        &mut self,
        graph: &PreparedCallGraph<P, U>,
        application: D::Application,
        source: &TypeKind,
        enclosing: &EnclosingGenericParameterScope,
        limits: &CallableLimits,
        result: D::Projection,
    ) -> Result<PendingChildConstraint<D>, FunctionSpecializationFailure<D>>
    where
        D::Projection: Clone,
    {
        graph
            .validate_constraint_authority(self.authority)
            .map_err(FunctionSpecializationFailure::Prepared)?;
        // Admit the full source, including nested binders and effect DAGs, to
        // this borrowed component's work before constructing its schema.
        source
            .semantic_identity_digest_in_scope_with_control(&GenericScope::default(), self.context)
            .map_err(|error| match error {
                TypeProjectionError::Control(error) => {
                    FunctionSpecializationFailure::Constraint(error.into())
                }
                TypeProjectionError::Instantiation(error) => {
                    FunctionSpecializationFailure::Prepared(error.into())
                }
            })?;
        let receipt = self.ticket.receipt();
        let initialization = graph
            .issue_function_specialization(
                self.authority,
                receipt.clone(),
                application,
                source,
                enclosing,
                limits,
            )
            .map_err(FunctionSpecializationFailure::Prepared)?;
        let (application, parameters, template) = initialization
            .into_lower_parts(self.authority, &receipt)
            .map_err(FunctionSpecializationFailure::Prepared)?;
        let nested = self
            .ticket
            .fork_for_child(self.context)
            .map_err(|error| FunctionSpecializationFailure::Constraint(error.into()))?;
        let mut lower = TypeConstraintTransaction::initialize_from_nested_path(
            self.context,
            application,
            parameters,
            None,
            nested,
        )
        .map_err(|error| {
            FunctionSpecializationFailure::Constraint(match error {
                TypeConstraintInitializationFailure::Abort(error) => {
                    TypeConstraintFailure::Abort(error)
                }
                TypeConstraintInitializationFailure::Invariant(error) => {
                    TypeConstraintFailure::Invariant(TypeConstraintFailureInvariant::Constraint(
                        error,
                    ))
                }
            })
        })?;
        lower.request_value_use_projection(self.context, result.clone(), source, &template);
        lower
            .defer_child_result(result)
            .map_err(FunctionSpecializationFailure::Constraint)
    }
}

#[cfg(test)]
mod tests;
