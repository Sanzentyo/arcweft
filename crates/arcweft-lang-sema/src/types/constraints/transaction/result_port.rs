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
    pub(crate) fn into_pending(self) -> PendingChildConstraint<D> {
        PendingChildConstraint {
            receipt: self.receipt,
            alternatives: vec![self.alternative],
        }
    }

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

impl<D: ConstraintDomain> TypeConstraintTransaction<D> {
    /// Relate a registered result to a use without closing its producing
    /// application. A value-use scope, when needed, is admitted on that exact
    /// path and records the source projection it consumed.
    pub(crate) fn constrain_result_use<A: TypeConstraintAccounting>(
        &mut self,
        context: &mut TypeConstraintContext<'_, A, D>,
        key: D::Projection,
        expected: &TypeKind,
        mut prepare: impl FnMut(
            &TypeKind,
            &mut TypeConstraintContext<'_, A, D>,
        ) -> Result<
            Option<(
                D::Application,
                super::super::TypeConstraintParameterScope,
                TypeKind,
            )>,
            TypeConstraintFailure<D>,
        >,
    ) -> Result<(), TypeConstraintFailure<D>>
    where
        D::Projection: Clone,
    {
        if self.first_failure.is_some() || self.closed {
            return Ok(());
        }
        if self.probe.is_some() || self.probe_group.is_some() {
            return Err(protocol_error(TypeConstraintSourceProtocolInvariant::Outcome).into());
        }
        let ordinal = self.next_equation;
        self.next_equation = ordinal.checked_add(1).ok_or(TypeConstraintError::Abort(
            super::super::TypeConstraintAbort::ArithmeticOverflow,
        ))?;
        let origin = super::ConstraintResultProjection {
            application: self.application,
            key: Arc::new(key.clone()),
        };
        let mut advanced = Vec::new();
        for mut path in core::mem::take(&mut self.frontier) {
            context.enter_node()?;
            let (opened, _) =
                resolve_probe_term(SourceProbeTerm::Result(origin.clone()), &path, context)?;
            let source = project_type(
                &opened,
                path.projection_view(),
                ConstraintClosurePolicy::Hint,
                context,
            )?
            .value;
            let pattern = match prepare(&source, context)? {
                Some((application, parameters, template)) => {
                    let scope = super::ConstraintApplicationScope::new(application, parameters);
                    let application = scope.id();
                    path = context.admit_application(path, scope)?;
                    path = Self::prepare_application(context, application, path, None)?;
                    path.projections.push(Arc::new(ProjectionRequest {
                        application,
                        key: Arc::new(key.clone()),
                        value: template.clone(),
                        closure: TypeConstraintProjectionClosure::Closed,
                        source: Some(origin.clone()),
                        input: Some(source),
                    }));
                    context.open_template_type(&template, &path, application)?
                }
                None => opened,
            };
            path.equations.push(super::PendingEquation {
                ordinal: super::ConstraintEquationId {
                    application: self.application,
                    ordinal,
                },
                direction: crate::types::ConstraintAcceptance::ActualAcceptsPattern,
                pattern: pattern.clone(),
                actual: expected.clone(),
                source_ordinal: None,
                final_expected: None,
            });
            advanced.extend(super::relate_selected_call(
                &pattern,
                expected,
                path,
                context,
                crate::types::ConstraintAcceptance::ActualAcceptsPattern,
            )?);
        }
        if advanced.is_empty() {
            self.record_failure(
                TypeConstraintError::Rejected(super::super::TypeConstraintRejection::Mismatch)
                    .into(),
            );
        } else {
            self.frontier = advanced;
        }
        Ok(())
    }
}
