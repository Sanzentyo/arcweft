//! Move-only ownership of an open result and its complete producing frontier.

use super::{
    Arc, ConstraintDomain, ConstraintResultProjection, PendingChildAlternative,
    PendingChildConstraint, TypeConstraintAccounting, TypeConstraintContext, TypeConstraintError,
    TypeConstraintFailure, TypeConstraintProjectionInvariant, TypeConstraintRejection,
    TypeConstraintSourceProtocolInvariant, TypeConstraintTransaction, protocol_error,
};

#[cfg(test)]
mod tests;

/// Suspension preserves equations, source receipts, application scopes and all
/// transaction ordinals. It issues neither a completed solution nor a type
/// snapshot. A root result does not need a parent source in order to survive.
#[must_use = "a suspended frontier must be resumed or retained by its component"]
pub(crate) struct SuspendedConstraintFrontier<D: ConstraintDomain> {
    pub(super) transaction: TypeConstraintTransaction<D>,
    pub(super) result: ConstraintResultProjection<D>,
}

impl<D: ConstraintDomain> SuspendedConstraintFrontier<D> {
    pub(crate) fn original_context(&self) -> &super::super::OriginalProducerContext {
        &self.transaction.original
    }
}

impl<D: ConstraintDomain> TypeConstraintTransaction<D> {
    pub(crate) fn suspend_result<A: TypeConstraintAccounting>(
        self,
        context: &mut TypeConstraintContext<'_, A, D>,
        key: D::Projection,
    ) -> Result<SuspendedConstraintFrontier<D>, TypeConstraintFailure<D>> {
        context.check_cancelled()?;
        if self.probe.is_some()
            || self.probe_group.is_some()
            || self.closed
            || self.active_materialization.is_some()
            || !self.materialization.is_empty()
            || !self.materialized.is_empty()
        {
            return Err(protocol_error(TypeConstraintSourceProtocolInvariant::WrongPhase).into());
        }
        if let Some(failure) = self.first_failure {
            return Err(failure);
        }
        if self.frontier.is_empty() {
            return Err(
                TypeConstraintError::Rejected(TypeConstraintRejection::UnresolvedType).into(),
            );
        }
        let mut result = None;
        for path in &self.frontier {
            context.enter_node()?;
            path.applications
                .require_application(self.application)
                .map_err(TypeConstraintError::Invariant)?;
            let mut found = None;
            for request in &path.lineage.projections {
                context.enter_node()?;
                if request.application == self.application && request.key.as_ref() == &key {
                    if found.is_some() {
                        return Err(TypeConstraintError::Invariant(
                            super::super::TypeConstraintInvariant::Projection(
                                TypeConstraintProjectionInvariant::MissingKey,
                            ),
                        )
                        .into());
                    }
                    found = Some(request);
                }
            }
            let request = found.ok_or(TypeConstraintError::Invariant(
                super::super::TypeConstraintInvariant::Projection(
                    TypeConstraintProjectionInvariant::MissingKey,
                ),
            ))?;
            // Reuse the admitted projection's key allocation. The reference
            // identifies its owner; it carries no copied or joined type.
            result.get_or_insert_with(|| ConstraintResultProjection {
                application: self.application,
                key: Arc::clone(&request.key),
            });
        }
        let result = result.ok_or(TypeConstraintError::Rejected(
            TypeConstraintRejection::UnresolvedType,
        ))?;
        Ok(SuspendedConstraintFrontier {
            transaction: self,
            result,
        })
    }
}

impl<D: ConstraintDomain> SuspendedConstraintFrontier<D> {
    pub(crate) const fn result(&self) -> &ConstraintResultProjection<D> {
        &self.result
    }

    /// The original transaction resumes in place; no solution, path, source
    /// inventory or ordinal is reconstructed from an observation.
    pub(in crate::types::constraints) fn resume(self) -> TypeConstraintTransaction<D> {
        self.transaction
    }

    /// An already admitted child can transfer its paths to its exact parent.
    /// Independent roots remain suspended until their component consumes them.
    pub(crate) fn into_child<A: TypeConstraintAccounting>(
        self,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<PendingChildConstraint<D>, TypeConstraintFailure<D>> {
        let mut transaction = self.transaction;
        let receipt = transaction
            .parent_receipt
            .take()
            .ok_or_else(|| protocol_error(TypeConstraintSourceProtocolInvariant::WrongPhase))?;
        for _ in &transaction.frontier {
            context.enter_node()?;
        }
        let mut alternatives = Vec::with_capacity(transaction.frontier.len());
        for path in transaction.frontier {
            alternatives.push(PendingChildAlternative {
                path,
                result: self.result.clone(),
                branch: None,
            });
        }
        Ok(PendingChildConstraint {
            receipt,
            alternatives,
        })
    }
}
