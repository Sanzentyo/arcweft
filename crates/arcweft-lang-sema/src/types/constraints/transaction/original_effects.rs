//! Retain the prepared query roots of the original constraint calculation.

use super::super::normalization::{ConstraintClosedEffects, PreparedConstraintEffects};
use super::*;
use crate::{
    effect_row::prepared::{
        PreparedEffectFailure, PreparedEffectGraph, PreparedEffectScopedConstraint,
        ScopedEffectCompletion,
    },
    types::{GenericEffectReference, TypeProjectionError, effect_domain::PreparedTypeEffectRoots},
};

impl<D: ConstraintDomain> ConstraintPath<D> {
    /// Complete the source relation from within its original read scope.
    /// A direct terminal calculation has no caller-provided Future rows.
    pub(in crate::types::constraints) fn close_original_path_effects<
        A: TypeConstraintAccounting,
    >(
        &self,
        graph: &mut PreparedEffectGraph,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Arc<ConstraintClosedEffects<D>>, TypeConstraintError> {
        let original = self.prepare_original_path_effects(graph, context)?;
        ConstraintClosedEffects::for_original_path(self, original, &[], graph, context)
    }

    pub(super) fn collect_original_effect_types<'a, A: TypeConstraintAccounting>(
        &'a self,
        types: &mut Vec<&'a TypeKind<PreparedEffects>>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(), TypeConstraintError> {
        for request in &self.lineage.projections {
            context.enter_node()?;
            types.push(request.value.value());
            if let Some(input) = &request.input {
                context.enter_node()?;
                types.push(input.value());
            }
        }
        for source in &self.lineage.probe_trace {
            source.visit_original_effect_types(&mut |ty| {
                context.enter_node()?;
                types.push(ty);
                Ok::<_, TypeConstraintError>(())
            })?;
        }
        for equation in &self.equations {
            context.enter_node()?;
            context.enter_node()?;
            types.push(equation.pattern.value());
            types.push(equation.actual.value());
        }
        for binding in self.bindings.values() {
            context.enter_node()?;
            types.push(binding.value());
        }
        Ok(())
    }

    pub(super) fn prepare_original_path_effects<A: TypeConstraintAccounting>(
        &self,
        graph: &mut PreparedEffectGraph,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<PreparedConstraintEffects, TypeConstraintError> {
        let constraint = self.prepare_effect_constraint(graph, context)?;
        let mut types = Vec::new();
        self.collect_original_effect_types(&mut types, context)?;
        let roots =
            PreparedTypeEffectRoots::collect(types, [], context).map_err(|error| match error {
                TypeProjectionError::Control(error) => error,
                TypeProjectionError::Instantiation(error) => {
                    TypeConstraintInvariant::Instantiation(error).into()
                }
            })?;
        let original = self.freeze_original_effect_roots::<_, TypeConstraintError>(
            &roots,
            &[&constraint],
            graph,
            context,
        )?;
        PreparedConstraintEffects::new(original, constraint, context)
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
        let mut introduced = Vec::new();
        for parameter in self.effects.original_variables() {
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
        Ok(graph.read().freeze_product(
            roots.rows(),
            roots.predicates(),
            constraints,
            &introduced,
            context,
        )?)
    }
}
