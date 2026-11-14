//! Borrow the original live or extracted relation without publishing it.

use super::transaction::PreparedEffectDeltaData;
use super::*;

#[cfg(test)]
mod tests;

/// A borrowed read capability, never a checkpoint or publication permit. The
/// extracted variant retains the complete original delta beside its exact live
/// base; immutable borrows prevent that base changing during an operation.
#[derive(Clone, Copy)]
pub(crate) struct PreparedEffectReadView<'a> {
    graph: &'a PreparedEffectGraph,
    extracted: Option<&'a PreparedEffectDeltaData>,
    dependencies: Option<&'a PreparedEffectReadScope>,
}

impl PreparedEffectGraph {
    pub(crate) fn read(&self) -> PreparedEffectReadView<'_> {
        PreparedEffectReadView {
            graph: self,
            extracted: None,
            dependencies: None,
        }
    }

    pub(crate) fn read_extracted<'a, C: DecisionControl>(
        &'a self,
        delta: &'a PreparedEffectDelta,
        control: &mut C,
    ) -> Result<PreparedEffectReadView<'a>, PreparedEffectFailure<C::Error>> {
        self.validate_delta(delta, control)?;
        Ok(PreparedEffectReadView {
            graph: self,
            extracted: Some(&delta.data),
            dependencies: delta.dependencies.as_ref(),
        })
    }
}

impl<'a> PreparedEffectReadView<'a> {
    pub(in crate::effect_row::prepared) fn function_definition(
        self,
        id: u64,
    ) -> Option<&'a Arc<Function>> {
        self.extracted
            .and_then(|delta| delta.functions.get(&id))
            .or_else(|| {
                self.dependencies
                    .and_then(|scope| scope.data.functions.get(&id))
            })
            .or_else(|| {
                self.graph
                    .active_scope
                    .as_ref()
                    .and_then(|scope| scope.functions.get(&id))
            })
            .or_else(|| {
                self.graph
                    .active_scope
                    .as_ref()
                    .and_then(|scope| scope.input.data.functions.get(&id))
            })
            .or_else(|| self.graph.functions.get(&id))
    }

    pub(in crate::effect_row::prepared) fn row_definition(
        self,
        id: u64,
    ) -> Option<&'a Arc<RowTerm>> {
        self.extracted
            .and_then(|delta| delta.rows.get(&id))
            .or_else(|| self.dependencies.and_then(|scope| scope.data.rows.get(&id)))
            .or_else(|| self.graph.rows.get(&id))
            .or_else(|| {
                self.graph
                    .active_scope
                    .as_ref()
                    .and_then(|scope| scope.input.data.rows.get(&id))
            })
    }

    pub(in crate::effect_row::prepared) fn predicate_definition(
        self,
        id: u64,
    ) -> Option<&'a Arc<PredicateTerm>> {
        self.extracted
            .and_then(|delta| delta.predicates.get(&id))
            .or_else(|| {
                self.dependencies
                    .and_then(|scope| scope.data.predicates.get(&id))
            })
            .or_else(|| self.graph.predicates.get(&id))
            .or_else(|| {
                self.graph
                    .active_scope
                    .as_ref()
                    .and_then(|scope| scope.input.data.predicates.get(&id))
            })
    }

    pub(in crate::effect_row::prepared) fn contains_choice(self, id: u64) -> bool {
        self.extracted
            .is_some_and(|delta| delta.choices.contains(&id))
            || self
                .dependencies
                .is_some_and(|scope| scope.data.choices.contains(&id))
            || self.graph.choices.contains(&id)
            || self
                .graph
                .active_scope
                .as_ref()
                .is_some_and(|scope| scope.input.data.choices.contains(&id))
    }
    pub(crate) fn validate_owner(
        self,
        owner: &PreparedEffectGraphOwner,
    ) -> Result<(), PreparedEffectInvariant> {
        owner.validate(self.graph)
    }

    pub(crate) fn validate_retained_row(
        self,
        row: &PreparedEffectRow,
    ) -> Result<(), PreparedEffectInvariant> {
        if let PreparedEffectRow::Term(reference) = row {
            self.row(reference)?;
        }
        Ok(())
    }

    pub(crate) fn validate_retained_predicate(
        self,
        predicate: &PreparedEffectPredicate,
    ) -> Result<(), PreparedEffectInvariant> {
        if let PreparedEffectPredicate::Term(reference) = predicate {
            self.predicate(reference)?;
        }
        Ok(())
    }

    pub(in crate::effect_row::prepared) fn function(
        self,
        reference: &FunctionRef,
    ) -> Result<&'a Function, PreparedEffectInvariant> {
        let id = self.graph.validate(reference.reference())?;
        let value = self
            .function_definition(id)
            .map(Arc::as_ref)
            .ok_or(PreparedEffectInvariant::StaleReference)?;
        match (reference, value) {
            (FunctionRef::Scheme(_), Function::Scheme(_))
            | (FunctionRef::Unknown(_), Function::Unknown { .. })
            | (FunctionRef::Imported(_), Function::Imported(_)) => Ok(value),
            _ => Err(PreparedEffectInvariant::StaleReference),
        }
    }

    pub(in crate::effect_row::prepared) fn scheme(
        self,
        reference: &PreparedEffectSchemeRef,
    ) -> Result<&'a Scheme, PreparedEffectInvariant> {
        match self.function(&FunctionRef::Scheme(reference.clone()))? {
            Function::Scheme(value) => Ok(value),
            _ => Err(PreparedEffectInvariant::StaleReference),
        }
    }

    pub(in crate::effect_row::prepared) fn row(
        self,
        reference: &PreparedEffectRowRef,
    ) -> Result<&'a RowTerm, PreparedEffectInvariant> {
        let id = self.graph.validate(&reference.0)?;
        self.row_definition(id)
            .map(Arc::as_ref)
            .ok_or(PreparedEffectInvariant::StaleReference)
    }

    pub(in crate::effect_row::prepared) fn predicate(
        self,
        reference: &PreparedEffectPredicateRef,
    ) -> Result<&'a PredicateTerm, PreparedEffectInvariant> {
        let id = self.graph.validate(&reference.0)?;
        self.predicate_definition(id)
            .map(Arc::as_ref)
            .ok_or(PreparedEffectInvariant::StaleReference)
    }
}
