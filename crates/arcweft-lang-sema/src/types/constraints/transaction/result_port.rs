//! Affine transformation of a pending result on the path that produced it.

use super::{
    Arc, ConstraintClosurePolicy, ConstraintDomain, ConstraintSourceReceipt, NestedConstraintPath,
    PendingChildAlternative, PendingChildConstraint, ProjectionRequest, SourceProbeTerm,
    TypeConstraintAccounting, TypeConstraintContext, TypeConstraintError, TypeConstraintFailure,
    TypeConstraintProjectionClosure, TypeConstraintSourceProtocolInvariant,
    TypeConstraintTransaction, TypeKind, project_type, protocol_error, resolve_probe_term,
};

/// The head premise and path are inseparable. A consumer can admit a value-use
/// application without sealing, re-solving, or replaying its source expression.
pub(crate) struct PendingResultConstraint<D: ConstraintDomain> {
    receipt: ConstraintSourceReceipt<D>,
    alternative: PendingChildAlternative<D>,
    source: TypeKind,
}

impl<D: ConstraintDomain> PendingChildConstraint<D> {
    pub(crate) fn into_results<A: TypeConstraintAccounting>(
        self,
        receipt: &ConstraintSourceReceipt<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Vec<PendingResultConstraint<D>>, TypeConstraintError> {
        if !self.receipt.matches(receipt) {
            return Err(protocol_error(
                TypeConstraintSourceProtocolInvariant::WrongSource,
            ));
        }
        self.alternatives
            .into_iter()
            .map(|alternative| {
                context.enter_node()?;
                let (opened, _) = resolve_probe_term(
                    SourceProbeTerm::Result(alternative.result.clone()),
                    &alternative.path,
                    context,
                )?;
                let source = project_type(
                    &opened,
                    alternative.path.projection_view(),
                    ConstraintClosurePolicy::Hint,
                    context,
                )?
                .value;
                Ok(PendingResultConstraint {
                    receipt: self.receipt.clone(),
                    alternative,
                    source,
                })
            })
            .collect()
    }
}

impl<D: ConstraintDomain> PendingResultConstraint<D> {
    pub(crate) const fn source_type(&self) -> &TypeKind {
        &self.source
    }

    /// Only an already registered source result can be consumed. The new
    /// projection owns that edge, and admission ensures its domain owner is
    /// distinct from every application already present on this exact path.
    pub(crate) fn transform<A: TypeConstraintAccounting>(
        self,
        context: &mut TypeConstraintContext<'_, A, D>,
        application: D::Application,
        parameters: super::super::TypeConstraintParameterScope,
        template: &TypeKind,
        key: D::Projection,
    ) -> Result<PendingChildConstraint<D>, TypeConstraintFailure<D>>
    where
        D::Projection: Clone,
    {
        context.enter_node()?;
        let mut transaction = TypeConstraintTransaction::initialize_from_nested_path(
            context,
            application,
            parameters,
            None,
            NestedConstraintPath {
                receipt: self.receipt,
                path: self.alternative.path,
            },
        )
        .map_err(|error| match error {
            super::super::TypeConstraintInitializationFailure::Abort(error) => {
                TypeConstraintFailure::Abort(error)
            }
            super::super::TypeConstraintInitializationFailure::Invariant(error) => {
                TypeConstraintFailure::Invariant(
                    super::super::TypeConstraintFailureInvariant::Constraint(error),
                )
            }
        })?;
        let request = Arc::new(ProjectionRequest {
            application: transaction.application,
            key: Arc::new(key.clone()),
            value: template.clone(),
            closure: TypeConstraintProjectionClosure::Closed,
            source: Some(self.alternative.result),
            input: Some(self.source),
        });
        for path in &mut transaction.frontier {
            path.projections.push(Arc::clone(&request));
        }
        let mut pending = transaction.defer_child_result(key)?;
        for alternative in &mut pending.alternatives {
            alternative.branch.clone_from(&self.alternative.branch);
        }
        Ok(pending)
    }
}
