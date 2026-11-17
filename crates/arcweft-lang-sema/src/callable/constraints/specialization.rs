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
        graph: &mut PreparedCallGraph<P, U>,
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
        let alternatives = {
            let (effects, _) = graph
                .effect_declarations_mut()
                .map_err(FunctionSpecializationFailure::Prepared)?
                .relation();
            pending.into_results(&receipt, effects, self.context)
        }
        .map_err(|error| FunctionSpecializationFailure::Constraint(error.into()))?;
        let mut transformed: Option<PendingChildConstraint<D>> = None;
        for alternative in alternatives {
            let source = alternative.source_type();
            if !matches!(source, TypeKind::Function { binder, .. } if !binder.is_empty()) {
                let unchanged = alternative.into_pending();
                match &mut transformed {
                    Some(transformed) => transformed
                        .append(unchanged)
                        .map_err(|error| FunctionSpecializationFailure::Constraint(error.into()))?,
                    None => transformed = Some(unchanged),
                }
                continue;
            }
            let initialization = graph.issue_function_specialization(
                self.authority,
                receipt.clone(),
                application,
                source,
                alternative.source_scope(),
                enclosing,
                limits,
                self.context,
            )?;
            let (application, parameters, template) = initialization
                .into_lower_parts(self.authority, &receipt)
                .map_err(FunctionSpecializationFailure::Prepared)?;
            let next = {
                let (effects, _) = graph
                    .effect_declarations_mut()
                    .map_err(FunctionSpecializationFailure::Prepared)?
                    .relation();
                alternative.transform(
                    self.context,
                    effects,
                    application,
                    parameters,
                    &template,
                    result.clone(),
                )
            }
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
        graph: &mut PreparedCallGraph<P, U>,
        application: D::Application,
        source: &TypeKind<PreparedEffects>,
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
        let initialization = graph.issue_function_specialization(
            self.authority,
            receipt.clone(),
            application,
            source,
            &GenericScope::default(),
            enclosing,
            limits,
            self.context,
        )?;
        let (application, parameters, template) = initialization
            .into_lower_parts(self.authority, &receipt)
            .map_err(FunctionSpecializationFailure::Prepared)?;
        let nested = self
            .ticket
            .fork_for_child(self.context)
            .map_err(|error| FunctionSpecializationFailure::Constraint(error.into()))?;
        let (effects, _) = graph
            .effect_declarations_mut()
            .map_err(FunctionSpecializationFailure::Prepared)?
            .relation();
        let mut lower = TypeConstraintTransaction::initialize_from_nested_path(
            self.context,
            effects,
            nested
                .original_context()
                .map_err(|error| FunctionSpecializationFailure::Constraint(error.into()))?
                .clone(),
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
        lower.request_value_use_projection(
            self.context,
            effects,
            result.clone(),
            source,
            &template,
        );
        lower
            .suspend_result(self.context, result)
            .and_then(|frontier| frontier.into_child(self.context))
            .map_err(FunctionSpecializationFailure::Constraint)
    }
}

impl CandidateConstraintWorkSession<'_> {
    /// A known value outside a borrowed constraint component starts its own
    /// specialization application. No callable candidate or invocation is
    /// synthesized, and the closed source is never resolved a second time.
    pub(crate) fn specialize_root_function_value<D, P, U>(
        self,
        graph: &mut PreparedCallGraph<P, U>,
        application: crate::callable::CallableConstraintApplication,
        source: &TypeKind<PreparedEffects>,
        expected: &TypeKind<PreparedEffects>,
        enclosing: &EnclosingGenericParameterScope,
        limits: &CallableLimits,
        result: D::Projection,
    ) -> Result<Arc<crate::callable::CheckedFunctionSpecialization>, FunctionSpecializationFailure<D>>
    where
        D: ConstraintDomain<Application = crate::callable::CallableConstraintApplication>,
        D::Projection: Clone,
    {
        let mut context = TypeConstraintContext::with_accounting(self);
        let (initialization, template) = graph
            .prepare_root_function_specialization(
                application.expression(),
                source,
                enclosing,
                limits,
                &mut context,
            )?
            .into_parts();
        let (authority, parameters, inherited, imported) = initialization
            .into_lower_parts()
            .map_err(FunctionSpecializationFailure::Prepared)?;
        let original = authority
            .original_context(&mut context)
            .map_err(|error| FunctionSpecializationFailure::Constraint(error.into()))?;
        let (effects, _) = graph
            .effect_declarations_mut()
            .map_err(FunctionSpecializationFailure::Prepared)?
            .relation();
        let mut lower = TypeConstraintTransaction::<D>::initialize_with_imported(
            &mut context,
            effects,
            original,
            application,
            parameters,
            inherited,
            imported,
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
        lower.request_value_use_projection(
            &mut context,
            effects,
            result.clone(),
            source,
            &template,
        );
        lower.constrain(
            &mut context,
            effects,
            &template,
            expected,
            ConstraintAcceptance::ActualAcceptsPattern,
        );
        let completed = lower
            // This source-free application never opens callback materialization.
            // Every branch value is the domain's actual empty sealed branch.
            .finish(&mut context, effects, |_, _, _| Ok(true))
            .map_err(FunctionSpecializationFailure::Constraint)?;
        let result = completed.component.projection(application, &result).ok_or(
            FunctionSpecializationFailure::Prepared(
                CallConstraintInvariant::PreparedFunctionTypeMismatch,
            ),
        )?;
        crate::callable::CheckedFunctionSpecialization::seal(&result, &mut context).map_err(
            |error| match error {
                crate::callable::specialization::FunctionSpecializationSealFailure::Invariant(
                    error,
                ) => FunctionSpecializationFailure::Prepared(error),
                crate::callable::specialization::FunctionSpecializationSealFailure::Projection(
                    TypeProjectionError::Control(error),
                ) => FunctionSpecializationFailure::Constraint(error.into()),
                crate::callable::specialization::FunctionSpecializationSealFailure::Projection(
                    TypeProjectionError::Instantiation(error),
                ) => FunctionSpecializationFailure::Prepared(error.into()),
            },
        )
    }
}

impl<D, C> CandidateConstraintDriver<'_, '_, D, C>
where
    D: ConstraintDomain,
    C: TypeConstraintClient<D>,
    D::Projection: Clone,
{
    pub(crate) fn constrain_function_result_use(
        &mut self,
        application: D::Application,
        result: D::Projection,
        expected: &TypeKind<PreparedEffects>,
        enclosing: &EnclosingGenericParameterScope,
        limits: &CallableLimits,
        invariant: impl Fn(CallConstraintInvariant) -> D::ClientInvariant,
    ) -> Result<(), TypeConstraintFailure<D>> {
        let authority = &self.callbacks.authority;
        let access = CandidateConstraintEffectAccess { authority };
        self.callbacks.client.with_effect_relation(access, |effects| {
            self.lower.constrain_result_use(self.callbacks.context, effects, result, expected, |source, graph, context| {
                if !matches!(source.source_type(), TypeKind::Function { binder, .. } if !binder.is_empty()) {
                    return Ok(None);
                }
                let (initialization, template) = authority
                    .prepare_function_specialization(source.source_type(), source.source_scope(), enclosing, limits, graph, context)
                    .map_err(|error| match error {
                        FunctionSpecializationFailure::Prepared(error) => TypeConstraintFailure::client_invariant(invariant(error)),
                        FunctionSpecializationFailure::Constraint(error) => error,
                    })?
                    .into_parts();
                let (_, parameters, inherited, imported) = initialization
                    .into_lower_parts()
                    .map_err(|error| TypeConstraintFailure::client_invariant(invariant(error)))?;
                if inherited.is_some() || imported.is_some() {
                    return Err(TypeConstraintFailure::client_invariant(invariant(
                        CallConstraintInvariant::MalformedSchemaInventory,
                    )));
                }
                Ok(Some(source.prepare_function_scheme(application, parameters, template, context)?))
            })
        }).map_err(CandidateConstraintCallbacks::<D, C>::checkpoint_failure)?
    }
}

#[cfg(test)]
mod tests;
