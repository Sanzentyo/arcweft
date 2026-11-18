//! Reattach a source's conditional least tuple to its real inference identities.

use super::*;
use crate::effect_row::prepared::read::SnapshotSource;

#[cfg(test)]
mod tests;

/// These are conditional facts, not a canonical result or a completed check.
/// Keeping the attachment retains the exact original C and Future bindings.
#[derive(Debug)]
pub(crate) struct PreparedEffectScopedBinding {
    relation: PreparedEffectImportedRelation,
    constraints: Box<[PreparedEffectScopedConstraint]>,
}

/// The source definitions with its local functions tied to the conditional
/// minimum. These definitions are inputs to final closure.
#[derive(Debug)]
pub(crate) struct BoundScopedEffectCompletion {
    pub(super) graph: PreparedEffectDefinitions,
}

impl BoundScopedEffectCompletion {
    pub(crate) fn require_unknown(
        &self,
        reference: &PreparedEffectUnknownRef,
    ) -> Result<(), PreparedEffectInvariant> {
        self.graph
            .function(&FunctionRef::Unknown(reference.clone()))
            .map(|_| ())
    }

    #[cfg(test)]
    pub(crate) fn compile_residual_for_test<C: DecisionControl>(
        self,
        control: &mut C,
    ) -> Result<Arc<ScopedEffectCompletion>, PreparedEffectFailure<C::Error>> {
        self.graph.complete_scoped(control)
    }
}

impl PreparedEffectScopedBinding {
    /// Freeze all candidate facts together. Preserve the original retained
    /// query ports by identity, including ports unrelated to an obligation.
    /// The original snapshot already selected these terms; no source is read
    /// or prepared again and no later value-use becomes a new inference input.
    pub(crate) fn freeze<C: DecisionControl>(
        self,
        graph: PreparedEffectReadView<'_>,
        control: &mut C,
    ) -> Result<BoundScopedEffectCompletion, PreparedEffectFailure<C::Error>> {
        graph.validate_owner(&self.relation.owner)?;
        self.relation.relation.validate(graph, control)?;
        let original = &self.relation.relation.completion;
        let mut constraints = Vec::new();
        for constraint in &self.constraints {
            control
                .charge(DecisionWork::Node)
                .map_err(PreparedEffectFailure::Control)?;
            constraints.push(constraint);
        }
        let graph = graph.snapshot(
            &[],
            &[],
            &constraints,
            SnapshotSource::Bound(&original.graph),
            control,
        )?;
        Ok(BoundScopedEffectCompletion { graph })
    }
}

impl PreparedEffectGraph {
    /// Bind a fresh snapshot back to the very functions it froze. In contrast
    /// to a cross-graph import, no caller may choose replacement identities.
    /// Future inputs remain their original live functions; each local output
    /// equals the conditional least function on its entire declared signature.
    pub(crate) fn bind_original_scoped<C: DecisionControl>(
        &mut self,
        completion: &Arc<ScopedEffectCompletion>,
        control: &mut C,
    ) -> Result<PreparedEffectScopedBinding, PreparedEffectFailure<C::Error>> {
        let mut checkpoint = self.checkpoint(control)?;
        let result = self.bind_original_scoped_in(completion, control);
        match result {
            Ok(binding) => match self.commit(&checkpoint) {
                Ok(()) => Ok(binding),
                Err(error) => {
                    self.rollback(&mut checkpoint)?;
                    Err(error.into())
                }
            },
            Err(error) => {
                self.rollback(&mut checkpoint)?;
                Err(error)
            }
        }
    }

    fn bind_original_scoped_in<C: DecisionControl>(
        &mut self,
        completion: &Arc<ScopedEffectCompletion>,
        control: &mut C,
    ) -> Result<PreparedEffectScopedBinding, PreparedEffectFailure<C::Error>> {
        self.read()
            .validate_owner(&PreparedEffectGraphOwner(Arc::clone(
                &completion.graph.issuer,
            )))?;
        // A partially specialized input is a different attachment. Equating it
        // with the original live Future would silently discard that binding.
        if !completion.fixed.is_empty() {
            return Err(PreparedEffectInvariant::ImportedRelationOrigin.into());
        }
        let mut futures = Vec::new();
        let mut locals = Vec::new();
        for (&id, definition) in &completion.graph.functions {
            control
                .charge(DecisionWork::Visit)
                .map_err(PreparedEffectFailure::Control)?;
            let Function::Unknown { eligibility, .. } = definition.as_ref() else {
                continue;
            };
            let reference = completion.graph.unknown_reference(id)?;
            let current = self
                .read()
                .function_definition(id)
                .ok_or(PreparedEffectInvariant::StaleReference)?;
            if !Arc::ptr_eq(definition, current) {
                return Err(PreparedEffectInvariant::ImportedRelationOrigin.into());
            }
            control
                .charge(DecisionWork::Node)
                .map_err(PreparedEffectFailure::Control)?;
            match eligibility {
                PreparedEffectUnknownEligibility::Future => futures.push(reference),
                PreparedEffectUnknownEligibility::Bindable => locals.push(reference),
            }
        }
        let mut inputs = Vec::new();
        for future in &futures {
            control
                .charge(DecisionWork::Node)
                .map_err(PreparedEffectFailure::Control)?;
            inputs.push((future, future));
        }
        let relation = self.import_scoped_relation(completion, &inputs, control)?;
        let principal = self.imported_principal(&relation, control)?;
        let mut constraints = Vec::new();
        control
            .charge(DecisionWork::Node)
            .map_err(PreparedEffectFailure::Control)?;
        constraints.push(self.read().scoped_constraint(
            &[],
            &PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
            &principal,
            control,
        )?);
        for local in locals {
            let mut parameters = Vec::new();
            let mut arguments = Vec::new();
            for parameter in completion
                .graph
                .function(&FunctionRef::Unknown(local.clone()))?
                .parameters()
            {
                control
                    .charge(DecisionWork::Node)
                    .map_err(PreparedEffectFailure::Control)?;
                parameters.push(parameter.clone());
                control
                    .charge(DecisionWork::Node)
                    .map_err(PreparedEffectFailure::Control)?;
                arguments.push(PreparedEffectRow::Canonical(EffectRow::open(
                    crate::effects::EffectSet::new(),
                    parameter.clone(),
                )));
            }
            let imported =
                self.import_relation_function(&relation, &completion.unknown(&local)?, control)?;
            let mut actual_arguments = Vec::new();
            for argument in &arguments {
                control
                    .charge(DecisionWork::Node)
                    .map_err(PreparedEffectFailure::Control)?;
                actual_arguments.push(argument.clone());
            }
            let actual = self.unknown_application(&local, actual_arguments, control)?;
            let selected = self.imported_application(&imported, arguments, control)?;
            let agreement =
                self.relate(PreparedEffectRelation::Equal, actual, selected, control)?;
            control
                .charge(DecisionWork::Node)
                .map_err(PreparedEffectFailure::Control)?;
            control
                .charge(DecisionWork::Node)
                .map_err(PreparedEffectFailure::Control)?;
            // This is a function's declared extensionality scope, not a source
            // Γ inferred from occurrences. Its formal keys may name captured
            // inference parameters; they are quantified arguments here, never
            // ambient raw inference row inputs admitted by scoped_constraint.
            constraints.push(PreparedEffectScopedConstraint {
                owner: relation.owner.clone(),
                obligation: Arc::new(ScopedObligation {
                    parameters: parameters.into_boxed_slice(),
                    domain: PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
                    predicate: agreement,
                }),
            });
        }
        Ok(PreparedEffectScopedBinding {
            relation,
            constraints: constraints.into_boxed_slice(),
        })
    }
}
