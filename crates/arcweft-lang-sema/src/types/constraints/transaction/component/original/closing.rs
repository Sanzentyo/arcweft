//! Failure-atomic final validation of the already prepared original forest.

use super::*;
use crate::types::constraints::transaction::materialization::MaterializationInput;

enum OriginalProductClosure<D: ConstraintDomain> {
    ExistingRejection,
    Rejected(OriginalProductRejection),
    Accepted(
        Vec<(
            usize,
            Arc<ConstraintClosedEffects<D>>,
            Box<[CompletedOriginalMember<D>]>,
        )>,
    ),
}

impl<D: ConstraintDomain, P> OriginalConstraintForest<D, P> {
    /// Finalize without preparing any source again. Operational failure returns
    /// this same affine forest for retry and rolls back only new graph metadata.
    pub(crate) fn close<A: TypeConstraintAccounting>(
        self,
        inputs: &[(
            &crate::effect_row::prepared::PreparedEffectUnknownRef,
            &crate::effect_row::EffectRow,
        )],
        graph: &mut PreparedEffectGraph,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<ClosedOriginalConstraintForest<D, P>, OriginalForestCloseFailure<D, P>> {
        let staged = context.with_effect_transaction(graph, |context, graph| {
            graph
                .read()
                .validate_owner(&self.original.effect_owner)
                .map_err(TypeConstraintError::from)?;
            let inputs = graph.read().actual_inputs(inputs, context)?;
            let mut staged = Vec::new();
            for product in &self.products {
                context.enter_node()?;
                context.enter_node()?;
                let Ok(prepared) = &product.result else {
                    staged.push(OriginalProductClosure::ExistingRejection);
                    continue;
                };
                let mut sources = Vec::new();
                for member in product.members.iter() {
                    context.enter_node()?;
                    sources.push(
                        self.original
                            .original_path(*member)?
                            .lineage
                            .original
                            .clone(),
                    );
                }
                let sources: Arc<[OriginalProducerContext]> = sources.into();
                let mut accepted = Vec::new();
                let mut rejected = None;
                for (index, prepared) in prepared.iter().enumerate() {
                    if !Arc::ptr_eq(&prepared.branch.lineage, &product.members) {
                        return Err(PreparedValueFailure::from(TypeConstraintError::from(TypeConstraintInvariant::OriginalProducer)));
                    }
                    let closed = context.with_original_scope(product.scope.clone(), |context| {
                        context.with_effect_transaction(graph, |context, graph| {
                            let effects = ConstraintClosedEffects::for_branch(
                                &prepared.branch,
                                &product.scope,
                                Arc::clone(&sources),
                                &prepared.effects,
                                &inputs,
                                graph,
                                context,
                            )
                            .map_err(|error| reject(OriginalProductCheck::Completion, error))?;
                            self.original.validate_original_bindings(
                                &prepared.branch,
                                &product.members,
                                &product.scope,
                                &effects,
                                graph,
                                context,
                            )?;
                            let mut completed = Vec::new();
                            for member in product.members.iter() {
                                context.enter_node()?;
                                let source = self.original.original_path(*member)?;
                                let input = MaterializationInput::from_closed_original(
                                    source, source.root_id(), &prepared.branch, &effects, graph.read(), context,
                                ).map_err(|error| reject(OriginalProductCheck::MemberCompletion(*member), error))?;
                                context.enter_node()?;
                                let materialization = crate::types::constraints::transaction::ConstraintMaterialization::from_component(input, &self.original.effect_owner, context)
                                    .map_err(|error| reject(OriginalProductCheck::MemberCompletion(*member), error))?;
                                completed.push(CompletedOriginalMember { member: *member, materialization });
                            }
                            Ok::<_, OriginalInferenceFailure>((effects, completed.into_boxed_slice()))
                        })
                    });
                    match closed {
                        Ok((effects, completed)) => {
                            context.enter_node()?;
                            context.enter_node()?;
                            accepted.push((index, effects, completed));
                        }
                        Err(OriginalInferenceFailure::Rejected(reason)) => {
                            if rejected.is_none() {
                                rejected = Some(reason);
                            }
                        }
                        Err(OriginalInferenceFailure::Constraint(error)) => {
                            return Err(PreparedValueFailure::from(error));
                        }
                        Err(OriginalInferenceFailure::Instantiation(error)) => {
                            return Err(PreparedValueFailure::from(error));
                        }
                        Err(OriginalInferenceFailure::Effect(error)) => {
                            return Err(PreparedValueFailure::from(error));
                        }
                    }
                }
                staged.push(if accepted.is_empty() {
                    OriginalProductClosure::Rejected(rejected.ok_or_else(|| {
                        TypeConstraintError::from(TypeConstraintInvariant::OriginalProducer)
                    })?)
                } else {
                    OriginalProductClosure::Accepted(accepted)
                });
            }
            Ok::<_, PreparedValueFailure>(staged)
        });
        let staged = match staged {
            Ok(staged) => staged,
            Err(error) => {
                return Err(OriginalForestCloseFailure {
                    error,
                    forest: self,
                });
            }
        };
        let mut products = Vec::with_capacity(staged.len());
        for (product, staged) in self.products.into_vec().into_iter().zip(staged) {
            let result = match (product.result, staged) {
                (Err(reason), OriginalProductClosure::ExistingRejection) => Err(reason),
                (Ok(_), OriginalProductClosure::Rejected(reason)) => Err(reason),
                (Ok(prepared), OriginalProductClosure::Accepted(accepted)) => {
                    let mut accepted = accepted.into_iter().peekable();
                    let mut closed = Vec::with_capacity(accepted.len());
                    for (index, prepared) in prepared.into_vec().into_iter().enumerate() {
                        if accepted
                            .peek()
                            .is_some_and(|(selected, _, _)| *selected == index)
                        {
                            let (_, effects, completed) =
                                accepted.next().expect("peeked validated original product");
                            closed.push(ClosedOriginalProduct {
                                branch: prepared.branch,
                                effects,
                                completed,
                            });
                        }
                    }
                    Ok(closed.into_boxed_slice())
                }
                _ => unreachable!("closure staging borrows the same immutable product inventory"),
            };
            products.push(OriginalProductInference {
                members: product.members,
                scope: product.scope,
                result,
            });
        }
        Ok(ClosedOriginalConstraintForest {
            original: self.original,
            products: products.into_boxed_slice(),
        })
    }
}
