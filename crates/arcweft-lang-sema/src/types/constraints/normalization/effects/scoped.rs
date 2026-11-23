//! Prepare one source's exact Γ, D and original R for joint completion.

use super::*;
use crate::effect_row::prepared::{
    PreparedEffectFailure, PreparedEffectRelation, PreparedEffectTermEdit,
};
use crate::effect_row::{
    EffectConstraintEligibility,
    prepared::{PreparedEffectGraph, PreparedEffectPredicate, PreparedEffectScopedConstraint},
};

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Eq, PartialEq)]
struct HeaderEffectConstraint {
    scope: Arc<ConstraintLexicalScope>,
    predicate: PreparedEffectPredicate,
}

/// The original relation owns one ambient clause and the clauses issued
/// while comparing locally bound function headers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::types::constraints) struct BranchEffectConstraints {
    pub(in crate::types::constraints) ambient: EffectConstraintEnvironment<PreparedEffects>,
    headers: Arc<Vec<HeaderEffectConstraint>>,
}

impl BranchEffectConstraints {
    pub(in crate::types::constraints) fn new(
        ambient: EffectConstraintEnvironment<PreparedEffects>,
    ) -> Self {
        Self {
            ambient,
            headers: Arc::default(),
        }
    }

    fn retain_header<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &mut self,
        scope: &Arc<ConstraintLexicalScope>,
        graph: &mut PreparedEffectGraph,
        context: &mut TypeConstraintContext<'_, A, D>,
        prepare: impl FnOnce(
            &EffectConstraintEnvironment<PreparedEffects>,
            &mut PreparedEffectTermEdit<'_>,
            &mut TypeConstraintContext<'_, A, D>,
        ) -> Result<
            PreparedEffectPredicate,
            PreparedEffectFailure<TypeConstraintError>,
        >,
    ) -> Result<(), TypeConstraintError> {
        let mut edit = graph.edit_terms();
        let predicate = prepare(&self.ambient, &mut edit, context)?;
        if predicate.constant_truth() == Some(true) {
            context.enter_node()?;
            edit.commit()?;
            return Ok(());
        }
        if Arc::get_mut(&mut self.headers).is_none() {
            for _ in self.headers.iter() {
                context.enter_node()?;
            }
        }
        context.enter_node()?;
        edit.commit()?;
        Arc::make_mut(&mut self.headers).push(HeaderEffectConstraint {
            scope: Arc::clone(scope),
            predicate,
        });
        Ok(())
    }
}

impl<D: ConstraintDomain, L> ConstraintInferenceBranch<D, L> {
    pub(in crate::types) fn constrain_header_predicates<A: TypeConstraintAccounting>(
        &mut self,
        left: &ConstraintFunctionHeaderView<'_, D>,
        right: &ConstraintFunctionHeaderView<'_, D>,
        graph: &mut PreparedEffectGraph,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(), TypeConstraintError> {
        let scope = left.lexical_scope();
        if scope != right.lexical_scope() {
            return Err(TypeConstraintInvariant::OriginalProducer.into());
        }
        if scope.as_ref() == &self.scope {
            self.effects.ambient.constrain_predicate_equality(
                graph,
                left.predicate(),
                right.predicate(),
                context,
            )?;
        } else {
            self.effects
                .retain_header(scope, graph, context, |environment, edit, context| {
                    environment.prepare_predicate_equality(
                        edit,
                        left.predicate(),
                        right.predicate(),
                        context,
                    )
                })?;
        }
        Ok(())
    }

    pub(in crate::types) fn constrain_header_rows<A: TypeConstraintAccounting>(
        &mut self,
        actual: &ConstraintFunctionHeaderView<'_, D>,
        permitted: &ConstraintFunctionHeaderView<'_, D>,
        relation: PreparedEffectRelation,
        graph: &mut PreparedEffectGraph,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(), TypeConstraintError> {
        let scope = actual.lexical_scope();
        if scope != permitted.lexical_scope() {
            return Err(TypeConstraintInvariant::OriginalProducer.into());
        }
        if scope.as_ref() == &self.scope {
            self.effects.ambient.constrain_relation(
                graph,
                relation,
                actual.row(),
                permitted.row(),
                context,
            )?;
        } else {
            self.effects
                .retain_header(scope, graph, context, |environment, edit, context| {
                    environment.prepare_relation(
                        edit,
                        relation,
                        actual.row(),
                        permitted.row(),
                        context,
                    )
                })?;
        }
        Ok(())
    }
}

impl<D: ConstraintDomain> ConstraintPath<D> {
    /// The original source owns D. The branch supplies R and its real
    /// application owners; neither observed support nor a candidate contract
    /// can replace the source's ambient Γ or become its premise.
    pub(in crate::types::constraints) fn prepare_effect_constraints<A: TypeConstraintAccounting>(
        &self,
        graph: &mut PreparedEffectGraph,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<PreparedBranchEffectConstraints, TypeConstraintError> {
        let domain = self
            .lineage
            .original
            .project_domain(&self.scope, context)
            .map_err(|error| match error {
                crate::types::TypeProjectionError::Control(error) => error,
                crate::types::TypeProjectionError::Instantiation(error) => {
                    TypeConstraintInvariant::Instantiation(error).into()
                }
            })?;
        let domain = PreparedEffectPredicate::Canonical(domain);
        self.prepare_effect_constraints_in(&domain, graph, context)
    }

    #[cfg(test)]
    pub(in crate::types::constraints) fn prepare_effect_constraint<A: TypeConstraintAccounting>(
        &self,
        graph: &mut PreparedEffectGraph,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<PreparedEffectScopedConstraint, TypeConstraintError> {
        self.prepare_effect_constraints(graph, context)?
            .into_single()
    }
}

pub(in crate::types::constraints) struct PreparedBranchEffectConstraints {
    pub(in crate::types::constraints) primary: PreparedEffectScopedConstraint,
    pub(in crate::types::constraints) headers: Vec<PreparedEffectScopedConstraint>,
}

impl PreparedBranchEffectConstraints {
    #[cfg(test)]
    fn into_single(self) -> Result<PreparedEffectScopedConstraint, TypeConstraintError> {
        if self.headers.is_empty() {
            Ok(self.primary)
        } else {
            Err(TypeConstraintInvariant::OriginalProducer.into())
        }
    }
}

impl<D: ConstraintDomain, L> ConstraintInferenceBranch<D, L> {
    /// A joint original product supplies its real merged scope and original
    /// source premises. It keeps the same Γ admission and application fold as
    /// an individual source, without manufacturing a source lineage.
    pub(in crate::types::constraints) fn prepare_effect_constraints_in<
        A: TypeConstraintAccounting,
    >(
        &self,
        domain: &PreparedEffectPredicate,
        graph: &mut PreparedEffectGraph,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<PreparedBranchEffectConstraints, TypeConstraintError> {
        context.with_effect_transaction(graph, |context, graph| {
            let ambient = &self.scope;
            let primary = self.prepare_effect_clause(
                ambient,
                domain,
                self.effects.ambient.original_predicate(),
                graph,
                context,
            )?;
            let mut headers = Vec::new();
            for header in self.effects.headers.iter() {
                context.enter_node()?;
                for _ in ambient.shape().binders() {
                    context.enter_node()?;
                }
                // A header may be visited through a retained equation's
                // actual frames. Both the ambient premise and the header
                // constraint enter the same quantified scope.
                let mut scope = ambient.clone();
                scope.merge(&header.scope, context)?;
                let mut edit = graph.edit_terms();
                let domain = transport_effect_clause(&scope, ambient, domain, &mut edit, context)?;
                let predicate = transport_effect_clause(
                    &scope,
                    &header.scope,
                    &header.predicate,
                    &mut edit,
                    context,
                )?;
                edit.commit()?;
                headers
                    .push(self.prepare_effect_clause(&scope, &domain, &predicate, graph, context)?);
            }
            Ok(PreparedBranchEffectConstraints { primary, headers })
        })
    }

    fn prepare_effect_clause<A: TypeConstraintAccounting>(
        &self,
        scope: &ConstraintLexicalScope,
        domain: &PreparedEffectPredicate,
        predicate: &PreparedEffectPredicate,
        graph: &mut PreparedEffectGraph,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<PreparedEffectScopedConstraint, TypeConstraintError> {
        let ambient = &self.effects.ambient;
        let predicate = self
            .applications
            .prepare_effect_predicate(ambient, predicate, scope, graph, context)?;
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
        for reference in ambient.original_variables() {
            context.enter_node()?;
            if reference.free_parameter().is_some()
                && ambient.eligibility(reference) == Some(EffectConstraintEligibility::Rigid)
            {
                parameters.push(reference.clone());
            }
        }
        graph
            .read()
            .scoped_constraint(&parameters, domain, &predicate, context)
            .map_err(Into::into)
    }
}

fn transport_effect_clause<A: TypeConstraintAccounting, D: ConstraintDomain>(
    target: &ConstraintLexicalScope,
    source: &ConstraintLexicalScope,
    predicate: &PreparedEffectPredicate,
    edit: &mut PreparedEffectTermEdit<'_>,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<PreparedEffectPredicate, TypeConstraintError> {
    if target == source || predicate.constant_truth().is_some() {
        return Ok(edit.read().retain_predicate(predicate, context)?);
    }
    edit.rebase_predicate_parameters(predicate, context, &mut |reference, context| {
        let mapped = target
            .effect_reference(source, source.shape(), target.shape(), reference, context)
            .map_err(|error| match error {
                crate::types::TypeProjectionError::Control(error) => error,
                crate::types::TypeProjectionError::Instantiation(error) => {
                    TypeConstraintInvariant::Instantiation(error).into()
                }
            })
            .map_err(PreparedEffectFailure::Control)?;
        Ok((mapped != *reference).then_some(mapped))
    })
    .map_err(Into::into)
}
