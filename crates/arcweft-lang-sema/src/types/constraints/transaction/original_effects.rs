//! Retain the prepared query roots of the original constraint calculation.

use super::super::normalization::{ConstraintClosedEffects, PreparedConstraintEffects};
use super::*;
use crate::{
    effect_row::prepared::{
        PreparedEffectFailure, PreparedEffectGraph, PreparedEffectInvariant,
        PreparedEffectReadView, PreparedEffectScopedConstraint, PreparedEffectUnknownRef,
        ScopedEffectCompletion,
    },
    types::{GenericEffectReference, TypeProjectionError, effect_domain::PreparedTypeEffectRoots},
};

pub(super) type OriginalEffectType<'a> = (
    &'a TypeKind<PreparedEffects>,
    Option<&'a PreparedNamedEffectPort>,
);

pub(super) fn original_effect_roots<'a, A: TypeConstraintAccounting, D: ConstraintDomain>(
    types: Vec<OriginalEffectType<'a>>,
    predicates: impl IntoIterator<Item = &'a crate::effect_row::prepared::PreparedEffectPredicate>,
    graph: PreparedEffectReadView<'a>,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<PreparedTypeEffectRoots<'a>, TypeConstraintError> {
    let projection_error = |error| match error {
        TypeProjectionError::Control(error) => error,
        TypeProjectionError::Instantiation(error) => {
            TypeConstraintInvariant::Instantiation(error).into()
        }
    };
    let mut roots =
        PreparedTypeEffectRoots::collect([], predicates, context).map_err(projection_error)?;
    for (value, port) in types {
        let named = port
            .map(|port| {
                port.application(value, graph, context)?
                    .ok_or(PreparedEffectFailure::Invariant(
                        PreparedEffectInvariant::StaleReference,
                    ))
            })
            .transpose()?;
        roots
            .extend_type(value, named, context)
            .map_err(projection_error)?;
    }
    Ok(roots)
}

impl<D: ConstraintDomain> ConstraintPath<D> {
    /// Complete the source relation within its original read scope. Only an
    /// application issuing a continuation can retain its own Future rows.
    pub(in crate::types::constraints) fn close_original_path_effects<
        A: TypeConstraintAccounting,
    >(
        &self,
        graph: &mut PreparedEffectGraph,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Arc<ConstraintClosedEffects<D>>, TypeConstraintError> {
        let mut residual = BTreeSet::new();
        for request in self.lineage.projections.iter().filter(|request| {
            request.closure == TypeConstraintProjectionClosure::AllowFutureEligible
        }) {
            context.enter_node()?;
            let application = self.applications.require_application(request.application)?;
            for row in application.effects().variables() {
                context.enter_node()?;
                if row.eligibility()
                    == crate::effect_row::EffectConstraintEligibility::FutureEligible
                {
                    let reference = application
                        .parameters()
                        .effect_reference(row.variable())
                        .ok_or(TypeConstraintInvariant::OriginalProducer)?;
                    context.enter_node()?;
                    residual.insert(reference);
                }
            }
        }
        let residual = residual.into_iter().collect::<Vec<_>>();
        self.close_path_effects(&residual, graph, context)
    }

    #[cfg(test)]
    pub(in crate::types::constraints) fn close_fixture_continuation_effects<
        A: TypeConstraintAccounting,
    >(
        &self,
        residual: &[GenericEffectReference],
        graph: &mut PreparedEffectGraph,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Arc<ConstraintClosedEffects<D>>, TypeConstraintError> {
        self.close_path_effects(residual, graph, context)
    }

    pub(super) fn collect_original_effect_types<'a, A: TypeConstraintAccounting>(
        &'a self,
        types: &mut Vec<OriginalEffectType<'a>>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(), TypeConstraintError> {
        for request in &self.lineage.projections {
            context.enter_node()?;
            types.push((request.value.value(), request.named_effect.as_ref()));
            if let Some(input) = &request.input {
                context.enter_node()?;
                types.push((input.value(), None));
            }
        }
        for source in &self.lineage.probe_trace {
            source.visit_original_effect_types(&mut |ty| {
                context.enter_node()?;
                types.push((ty, None));
                Ok::<_, TypeConstraintError>(())
            })?;
        }
        for equation in &self.equations {
            if equation.is_next_invocation() {
                // A preview contributes through the bindings it inferred.
                // The next invocation owns the original effect operands.
                continue;
            }
            context.enter_node()?;
            context.enter_node()?;
            types.push((equation.pattern.value(), None));
            types.push((equation.actual.value(), None));
        }
        for binding in self.bindings.values() {
            context.enter_node()?;
            types.push((binding.value(), None));
        }
        Ok(())
    }

    fn close_path_effects<A: TypeConstraintAccounting>(
        &self,
        residual: &[GenericEffectReference],
        graph: &mut PreparedEffectGraph,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Arc<ConstraintClosedEffects<D>>, TypeConstraintError> {
        let constraints = self.prepare_effect_constraints(graph, context)?;
        let mut types = Vec::new();
        self.collect_original_effect_types(&mut types, context)?;
        let roots = original_effect_roots(types, [], graph.read(), context)?;
        let mut clauses = vec![&constraints.primary];
        for header in &constraints.headers {
            context.enter_node()?;
            clauses.push(header);
        }
        let introduced = self.original_effect_functions(context)?;
        if residual.is_empty()
            && let Some(proof) = graph.read().close_constant_product(
                roots.rows(),
                roots.predicates(),
                &clauses,
                &introduced,
                context,
            )?
        {
            return ConstraintClosedEffects::for_constant_path(
                self,
                proof,
                constraints.primary,
                context,
            );
        }
        let frozen = graph.read().freeze_product(
            roots.rows(),
            roots.predicates(),
            &clauses,
            &introduced,
            context,
        )?;
        let original = PreparedConstraintEffects::new(frozen, constraints.primary, context)?;
        ConstraintClosedEffects::for_original_path(self, original, residual, graph, context)
    }
}

impl<D: ConstraintDomain, L> ConstraintInferenceBranch<D, L> {
    pub(super) fn freeze_original_effect_roots<A, E>(
        &self,
        roots: &PreparedTypeEffectRoots<'_>,
        constraints: &[&PreparedEffectScopedConstraint],
        graph: &PreparedEffectGraph,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Arc<ScopedEffectCompletion>, E>
    where
        A: TypeConstraintAccounting,
        E: From<TypeConstraintError> + From<PreparedEffectFailure<TypeConstraintError>>,
    {
        let introduced = self.original_effect_functions(context)?;
        Ok(graph.read().freeze_product(
            roots.rows(),
            roots.predicates(),
            constraints,
            &introduced,
            context,
        )?)
    }

    fn original_effect_functions<A: TypeConstraintAccounting>(
        &self,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Vec<&PreparedEffectUnknownRef>, TypeConstraintError> {
        let mut introduced = Vec::new();
        for parameter in self.effects.ambient.original_variables() {
            context.enter_node()?;
            if matches!(parameter, GenericEffectReference::Inference(_)) {
                let function = self
                    .applications
                    .effect_function(parameter)
                    .ok_or(TypeConstraintInvariant::OriginalProducer)
                    .map_err(TypeConstraintError::from)?;
                context.enter_node()?;
                introduced.push(function);
            }
        }
        Ok(introduced)
    }
}
