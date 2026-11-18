//! Failure-atomic final validation of the already prepared original forest.

use super::*;
use crate::types::constraints::transaction::ConstraintMaterialization;
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
            let mut input_scopes = Vec::new();
            if !inputs.is_empty() {
                for reads in self
                    .products
                    .iter()
                    .filter_map(|product| product.effect_reads.as_ref())
                {
                    context.enter_node()?;
                    input_scopes.push(graph.compose_read_scopes([reads], context)?);
                }
            }
            let inputs = graph.actual_inputs_with_scopes(inputs, &input_scopes, context)?;
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
                let reads = context.compose_effect_reads(graph, product.effect_reads.iter())?;
                for (index, prepared) in prepared.iter().enumerate() {
                    if !Arc::ptr_eq(&prepared.branch.lineage, &product.members) {
                        return Err(PreparedValueFailure::from(TypeConstraintError::from(
                            TypeConstraintInvariant::OriginalProducer,
                        )));
                    }
                    let closed = context.with_original_scope(product.scope.clone(), |context| {
                        let validate =
                            |context: &mut TypeConstraintContext<'_, A, D>,
                             graph: &mut PreparedEffectGraph| {
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
                                let mut projection = effects.projection();
                                self.original.validate_original_bindings(
                                    &prepared.branch,
                                    &product.members,
                                    &product.scope,
                                    &mut projection,
                                    graph,
                                    context,
                                )?;
                                let mut completed = Vec::new();
                                for member in product.members.iter() {
                                    context.enter_node()?;
                                    let source = self.original.original_path(*member)?;
                                    let input = MaterializationInput::from_closed_original(
                                        source,
                                        source.root_id(),
                                        &prepared.branch,
                                        &mut projection,
                                        graph.read(),
                                        context,
                                    )
                                    .map_err(|error| {
                                        reject(
                                            OriginalProductCheck::MemberCompletion(*member),
                                            error,
                                        )
                                    })?;
                                    context.enter_node()?;
                                    completed.push(CompletedOriginalMember {
                                        member: *member,
                                        input,
                                    });
                                }
                                Ok::<_, OriginalInferenceFailure>((
                                    effects,
                                    completed.into_boxed_slice(),
                                ))
                            };
                        match reads.as_ref() {
                            Some(reads) => {
                                graph.validate_in_read_scope(reads, context, |graph, context| {
                                    validate(context, graph)
                                })
                            }
                            None => context.with_effect_transaction(graph, validate),
                        }
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
                effect_reads: product.effect_reads,
                result,
            });
        }
        Ok(ClosedOriginalConstraintForest {
            original: self.original,
            products: products.into_boxed_slice(),
        })
    }
}

impl<D: ConstraintDomain> ClosedOriginalConstraintForest<D, ()> {
    /// A single producer can use the existing candidate callback protocol.
    /// Joint products stay grouped and cannot be mistaken for alternatives of
    /// one producer. Consuming the original inventory releases its aliases to
    /// the source preparations before a selected callback result takes them.
    pub(crate) fn into_candidate_materialization<A: TypeConstraintAccounting>(
        self,
        producer: &PreparedValueRef<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<ConstraintMaterialization<D>, PreparedValueFailure> {
        let value = self.original.value(producer)?;
        if self.original.values.len() != 1
            || !matches!(&value.definition, ValueDefinition::Constraint { .. })
        {
            return Err(PreparedValueFailure::ProducerApplication);
        }
        let mut materialization =
            ConstraintMaterialization::new(self.original.effect_owner.clone(), context)?;
        let mut accepted = false;
        let mut rejected = None;
        for product in self.products.into_vec() {
            context.enter_node()?;
            let [member] = product.members.as_ref() else {
                return Err(PreparedValueFailure::ProducerApplication);
            };
            if member.producer != producer.ordinal {
                return Err(PreparedValueFailure::ProducerApplication);
            }
            match product.result {
                Err(rejection) => {
                    rejected.get_or_insert(rejection.reason);
                }
                Ok(branches) => {
                    for branch in branches.into_vec() {
                        context.enter_node()?;
                        let [completed] = branch
                            .completed
                            .into_vec()
                            .try_into()
                            .map_err(|_| PreparedValueFailure::ProducerApplication)?;
                        let completed: CompletedOriginalMember<D> = completed;
                        if &completed.member != member {
                            return Err(PreparedValueFailure::ProducerApplication);
                        }
                        materialization.push_component(completed.input, context)?;
                        accepted = true;
                    }
                }
            }
        }
        if !accepted {
            let rejection = rejected.unwrap_or(TypeConstraintRejection::Mismatch);
            return Err(TypeConstraintError::Rejected(rejection).into());
        }
        Ok(materialization)
    }
}
