//! Final closure reads actual bodies and inputs while preserving original C.

use super::binding::ScopedCompletionBinding;
use super::*;
use crate::effect_row::prepared::read::SnapshotSource;

#[cfg(test)]
mod tests;

impl ScopedEffectCompletion {
    /// Reuse requires the exact immutable fact included in this original C.
    pub(crate) fn require_retained_constraint<C: DecisionControl>(
        &self,
        constraint: &PreparedEffectScopedConstraint,
        control: &mut C,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        if self
            .graph
            .retains_constraint(constraint, &mut Work(control))?
        {
            Ok(())
        } else {
            Err(PreparedEffectInvariant::ImportedRelationOrigin.into())
        }
    }

    /// A final mathematical closure requires every actual Future row and the
    /// actual original Named bodies. It neither prepares a body nor chooses a
    /// convenient replacement for an unavailable input. No live facts change.
    #[cfg(test)]
    pub(crate) fn close_original<C: DecisionControl>(
        &self,
        receiving: PreparedEffectReadView<'_>,
        query_rows: &[&PreparedEffectRow],
        query_predicates: &[&PreparedEffectPredicate],
        bindings: &[(&PreparedEffectUnknownRef, &EffectRow)],
        control: &mut C,
    ) -> Result<PrincipalEffectClosure, PreparedEffectFailure<C::Error>> {
        self.final_definitions(receiving, query_rows, query_predicates, control)?
            .specialize_principal(bindings, control)
    }

    #[cfg(test)]
    pub(crate) fn close_original_with_inputs<C: DecisionControl>(
        &self,
        receiving: PreparedEffectReadView<'_>,
        query_rows: &[&PreparedEffectRow],
        query_predicates: &[&PreparedEffectPredicate],
        inputs: &PreparedEffectActualInputs<'_>,
        control: &mut C,
    ) -> Result<PrincipalEffectClosure, PreparedEffectFailure<C::Error>> {
        self.final_definitions(receiving, query_rows, query_predicates, control)?
            .solve_with_actual_inputs(inputs, control)
    }

    #[cfg(test)]
    fn final_definitions<C: DecisionControl>(
        &self,
        receiving: PreparedEffectReadView<'_>,
        query_rows: &[&PreparedEffectRow],
        query_predicates: &[&PreparedEffectPredicate],
        control: &mut C,
    ) -> Result<PreparedEffectDefinitions, PreparedEffectFailure<C::Error>> {
        let owner = PreparedEffectGraphOwner(Arc::clone(&self.graph.issuer));
        receiving.validate_owner(&owner)?;
        if !self.fixed.is_empty() {
            return Err(PreparedEffectInvariant::ImportedRelationOrigin.into());
        }
        receiving.snapshot(
            query_rows,
            query_predicates,
            &[],
            SnapshotSource::Final(&self.graph),
            control,
        )
    }
}

impl BoundScopedEffectCompletion {
    pub(crate) fn complete_residual_rows<C: DecisionControl>(
        &self,
        receiving: &mut PreparedEffectGraph,
        query_rows: &[&PreparedEffectRow],
        query_predicates: &[&PreparedEffectPredicate],
        ports: &[(&PreparedEffectUnknownRef, &GenericEffectReference)],
        control: &mut C,
    ) -> Result<super::closure::ResidualEffectClosure, PreparedEffectFailure<C::Error>> {
        self.attached_definitions(receiving, query_rows, query_predicates, control)?
            .complete_residual_rows(ports, control)
    }

    pub(crate) fn close_original_with_inputs<C: DecisionControl>(
        &self,
        receiving: &mut PreparedEffectGraph,
        query_rows: &[&PreparedEffectRow],
        query_predicates: &[&PreparedEffectPredicate],
        inputs: &PreparedEffectActualInputs<'_>,
        control: &mut C,
    ) -> Result<PrincipalEffectClosure, PreparedEffectFailure<C::Error>> {
        if let ScopedCompletionBinding::Original(original) = &self.binding
            && let Some(definitions) = original.unchanged_relation(
                receiving.read(),
                query_rows,
                query_predicates,
                control,
            )?
        {
            return definitions.solve_with_actual_inputs(inputs, control);
        }
        self.attached_definitions(receiving, query_rows, query_predicates, control)?
            .solve_with_actual_inputs(inputs, control)
    }

    #[cfg(test)]
    pub(crate) fn close_original<C: DecisionControl>(
        &self,
        receiving: PreparedEffectReadView<'_>,
        query_rows: &[&PreparedEffectRow],
        query_predicates: &[&PreparedEffectPredicate],
        bindings: &[(&PreparedEffectUnknownRef, &EffectRow)],
        control: &mut C,
    ) -> Result<PrincipalEffectClosure, PreparedEffectFailure<C::Error>> {
        self.final_definitions(receiving, query_rows, query_predicates, control)?
            .specialize_principal(bindings, control)
    }

    fn final_definitions<C: DecisionControl>(
        &self,
        receiving: PreparedEffectReadView<'_>,
        query_rows: &[&PreparedEffectRow],
        query_predicates: &[&PreparedEffectPredicate],
        control: &mut C,
    ) -> Result<PreparedEffectDefinitions, PreparedEffectFailure<C::Error>> {
        if !matches!(&self.binding, ScopedCompletionBinding::Attached(_)) {
            return Err(PreparedEffectInvariant::ImportedRelationOrigin.into());
        }
        receiving.snapshot(
            query_rows,
            query_predicates,
            &[],
            SnapshotSource::FinalBound(self),
            control,
        )
    }

    fn attached_definitions<C: DecisionControl>(
        &self,
        receiving: &mut PreparedEffectGraph,
        query_rows: &[&PreparedEffectRow],
        query_predicates: &[&PreparedEffectPredicate],
        control: &mut C,
    ) -> Result<PreparedEffectDefinitions, PreparedEffectFailure<C::Error>> {
        match &self.binding {
            ScopedCompletionBinding::Attached(_) => {
                self.final_definitions(receiving.read(), query_rows, query_predicates, control)
            }
            ScopedCompletionBinding::Original(original) => {
                let mut checkpoint = receiving.checkpoint(control)?;
                let definitions = (|| {
                    receiving
                        .bind_original_scoped(original, control)?
                        .freeze(receiving.read(), control)?
                        .final_definitions(receiving.read(), query_rows, query_predicates, control)
                })();
                receiving.rollback(&mut checkpoint)?;
                definitions
            }
        }
    }
}

impl ScopedEffectCompletion {
    fn unchanged_relation<C: DecisionControl>(
        &self,
        receiving: PreparedEffectReadView<'_>,
        query_rows: &[&PreparedEffectRow],
        query_predicates: &[&PreparedEffectPredicate],
        control: &mut C,
    ) -> Result<Option<PreparedEffectDefinitions>, PreparedEffectFailure<C::Error>> {
        let mut work = Work(control);
        // Late input equations, new obligations, and changes to finite label
        // support can affect the original conditional minimum.
        for function in self.graph.functions.values() {
            work.charge(DecisionWork::Visit)?;
            if function.is_input() {
                return Ok(None);
            }
        }
        let mut definitions = receiving.snapshot(
            query_rows,
            query_predicates,
            &[],
            SnapshotSource::FinalOriginal(self),
            work.0,
        )?;
        if definitions.functions.len() != self.graph.functions.len()
            || definitions.choices.len() != self.graph.choices.len()
        {
            return Ok(None);
        }
        for obligation in &definitions.obligations[self.graph.obligations.len()..] {
            let mut retained = false;
            for original in &self.graph.obligations {
                work.charge(DecisionWork::Visit)?;
                if Arc::ptr_eq(original, obligation) {
                    retained = true;
                    break;
                }
            }
            if !retained {
                return Ok(None);
            }
        }
        definitions
            .obligations
            .truncate(self.graph.obligations.len());
        if definitions.label_support(&mut work)? != self.support {
            return Ok(None);
        }
        Ok(Some(definitions))
    }
}
