//! The joint relation retains each source's own universal premise separately.

use super::*;
use crate::{
    effect_row::prepared::{PreparedEffectScopedConstraint, ScopedEffectCompletion},
    types::effect_domain::PreparedTypeEffectRoots,
};

#[cfg(test)]
mod tests;

pub(super) struct PreparedOriginalEffects {
    sources: Vec<PreparedEffectScopedConstraint>,
    joint_domain: PreparedEffectPredicate,
}

impl<D: ConstraintDomain, P> OpenConstraintComponent<D, P> {
    pub(super) fn prepare_original_effects<A: TypeConstraintAccounting>(
        &self,
        members: &[OriginalProductMember],
        scope: &ConstraintLexicalScope,
        graph: &mut PreparedEffectGraph,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<PreparedOriginalEffects, OriginalInferenceFailure> {
        context.with_effect_transaction(graph, |context, graph| {
            let mut sources = Vec::new();
            let mut domains = Vec::new();
            for member in members {
                context.enter_node()?;
                let path = self.original_path(*member)?;
                let constraint =
                    path.prepare_effect_constraint(graph, context)
                        .map_err(|error| {
                            reject(OriginalProductCheck::EffectRelation(*member), error)
                        })?;
                context.enter_node()?;
                sources.push(constraint);
                let domain = path.lineage.original.domain();
                crate::types::projection_control::visit_effect_predicate(context, &domain, 1)
                    .map_err(projection_failure)?;
                context.enter_node()?;
                let domain = scope
                    .transport_predicate(path.lineage.original.gamma(), &domain, context)
                    .map_err(projection_failure)?;
                domains.push(PreparedEffectPredicate::Canonical(domain));
            }
            let joint_domain = graph.and(domains, context)?;
            Ok(PreparedOriginalEffects {
                sources,
                joint_domain,
            })
        })
    }

    pub(super) fn freeze_original_effects<A: TypeConstraintAccounting>(
        &self,
        branch: &JointBranch<D>,
        members: &[OriginalProductMember],
        scope: &ConstraintLexicalScope,
        original: &PreparedOriginalEffects,
        graph: &mut PreparedEffectGraph,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Arc<ScopedEffectCompletion>, OriginalInferenceFailure> {
        context.with_effect_transaction(graph, |context, graph| {
            let joint = branch
                .prepare_effect_constraint_in(scope, &original.joint_domain, graph, context)
                .map_err(|error| reject(OriginalProductCheck::Completion, error))?;
            let mut constraints = Vec::new();
            for source in &original.sources {
                context.enter_node()?;
                constraints.push(source);
            }
            context.enter_node()?;
            constraints.push(&joint);

            let mut selected = BTreeMap::new();
            for member in members {
                context.enter_node()?;
                selected.insert(
                    member.producer,
                    ProducerBranch::Constraint {
                        frontier: member.frontier,
                        path: member.path,
                    },
                );
            }
            let mut types = Vec::new();
            let mut predicates = Vec::new();
            for value in self.values.values() {
                context.enter_node()?;
                // Later value uses are not inputs, even when their results
                // contain new effect labels or refer to an original producer.
                if matches!(value.definition, ValueDefinition::Use(_)) {
                    continue;
                }
                for alternative in &value.alternatives {
                    context.enter_node()?;
                    let mut applicable = true;
                    for (producer, origin) in &alternative.origins {
                        context.enter_node()?;
                        applicable &= selected.get(producer) == Some(origin);
                    }
                    if !applicable {
                        continue;
                    }
                    context.enter_node()?;
                    predicates.push(&alternative.predicate);
                    if let ValueContent::Available { ty } = &alternative.value {
                        context.enter_node()?;
                        types.push(ty);
                    }
                }
            }
            for member in members {
                let path = self.original_path(*member)?;
                for request in &path.lineage.projections {
                    context.enter_node()?;
                    types.push(&request.value);
                }
                for binding in path.bindings.values() {
                    context.enter_node()?;
                    types.push(binding.value());
                }
            }
            for binding in branch.bindings.values() {
                context.enter_node()?;
                types.push(binding.value());
            }
            let roots = PreparedTypeEffectRoots::collect(types, predicates, context)
                .map_err(projection_failure)?;
            graph
                .read()
                .freeze_scoped(roots.rows(), roots.predicates(), &constraints, context)
                .map_err(Into::into)
        })
    }
}
