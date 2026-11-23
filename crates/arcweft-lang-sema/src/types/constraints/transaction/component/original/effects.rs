//! The joint relation retains each source's own universal premise separately.

use super::super::super::original_effects::original_effect_roots;
use super::*;
use crate::effect_row::prepared::{PreparedEffectScopedConstraint, ScopedEffectCompletion};

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
                    path.prepare_effect_constraints(graph, context)
                        .map_err(|error| {
                            reject(OriginalProductCheck::EffectRelation(*member), error)
                        })?;
                context.enter_node()?;
                sources.push(constraint.primary);
                for header in constraint.headers {
                    context.enter_node()?;
                    sources.push(header);
                }
                context.enter_node()?;
                let domain = path
                    .lineage
                    .original
                    .project_domain(scope, context)
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
        original: &PreparedOriginalEffects,
        graph: &mut PreparedEffectGraph,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<
        (Arc<ScopedEffectCompletion>, PreparedEffectScopedConstraint),
        OriginalInferenceFailure,
    > {
        context.with_effect_transaction(graph, |context, graph| {
            let joint = branch
                .prepare_effect_constraints_in(&original.joint_domain, graph, context)
                .map_err(|error| reject(OriginalProductCheck::Completion, error))?;
            let mut constraints = Vec::new();
            for source in &original.sources {
                context.enter_node()?;
                constraints.push(source);
            }
            context.enter_node()?;
            constraints.push(&joint.primary);
            for header in &joint.headers {
                context.enter_node()?;
                constraints.push(header);
            }

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
            for (&producer, value) in &self.values {
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
                        let port = match (&value.definition, selected.get(&producer)) {
                            (ValueDefinition::Constraint { frontiers }, Some(ProducerBranch::Constraint { frontier, path })) => {
                                let frontier = &frontiers[*frontier];
                                let request = crate::types::constraints::transaction::ResolvedConstraintResult::request(
                                    &frontier.result, &frontier.transaction.frontier[*path],
                                )?;
                                request.named_effect.as_ref()
                            }
                            _ => None,
                        };
                        types.push((ty, port));
                    }
                }
            }
            for member in members {
                let path = self.original_path(*member)?;
                path.collect_original_effect_types(&mut types, context)?;
            }
            for binding in branch.bindings.values() {
                context.enter_node()?;
                types.push((binding.value(), None));
            }
            let roots = original_effect_roots(types, predicates, graph.read(), context)?;
            let relation = branch.freeze_original_effect_roots::<_, OriginalInferenceFailure>(
                &roots,
                &constraints,
                graph,
                context,
            )?;
            Ok((relation, joint.primary))
        })
    }
}
