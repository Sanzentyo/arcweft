//! Prepare one source's exact Γ, D and original R for joint completion.

use super::*;
use crate::effect_row::{
    EffectConstraintEligibility,
    prepared::{PreparedEffectGraph, PreparedEffectPredicate, PreparedEffectScopedConstraint},
};

#[cfg(test)]
mod tests;

impl<D: ConstraintDomain> ConstraintPath<D> {
    /// The original source owns D. The branch supplies R and its real
    /// application owners; neither observed support nor a candidate contract
    /// can replace the source's ambient Γ or become its premise.
    pub(in crate::types::constraints) fn prepare_effect_constraint<A: TypeConstraintAccounting>(
        &self,
        graph: &mut PreparedEffectGraph,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<PreparedEffectScopedConstraint, TypeConstraintError> {
        context.with_effect_transaction(graph, |context, graph| {
            let scope = &self.lineage.scope;
            let effects =
                self.applications
                    .prepare_effects(&self.effects, scope, graph, context)?;
            let mut parameters = Vec::new();
            for (depth, binder) in scope.shape().binders().iter().rev().enumerate() {
                context.enter_node()?;
                let depth = u32::try_from(depth).map_err(|_| {
                    TypeConstraintInvariant::Instantiation(
                        crate::types::TypeInstantiationError::DepthOverflow,
                    )
                })?;
                for slot in 0..binder.effects() {
                    context.enter_node()?;
                    parameters.push(scope.shape().bound_effect(depth, slot)?);
                }
            }
            for reference in effects.original_variables() {
                context.enter_node()?;
                if reference.free_parameter().is_some()
                    && effects.eligibility(reference) == Some(EffectConstraintEligibility::Rigid)
                {
                    parameters.push(reference.clone());
                }
            }
            let domain = self.lineage.original.domain();
            crate::types::projection_control::visit_effect_predicate(context, &domain, 1).map_err(
                |error| match error {
                    crate::types::TypeProjectionError::Control(error) => error,
                    crate::types::TypeProjectionError::Instantiation(error) => {
                        TypeConstraintInvariant::Instantiation(error).into()
                    }
                },
            )?;
            let domain = PreparedEffectPredicate::Canonical(domain.into_owned());
            graph
                .read()
                .scoped_constraint(&parameters, &domain, effects.original_predicate(), context)
                .map_err(Into::into)
        })
    }
}
