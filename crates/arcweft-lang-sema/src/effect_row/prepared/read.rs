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
}

impl PreparedEffectGraph {
    pub(crate) fn read(&self) -> PreparedEffectReadView<'_> {
        PreparedEffectReadView {
            graph: self,
            extracted: None,
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
        })
    }
}

impl<'a> PreparedEffectReadView<'a> {
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
            .extracted
            .and_then(|delta| delta.functions.get(&id))
            .or_else(|| self.graph.functions.get(&id))
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
        self.extracted
            .and_then(|delta| delta.rows.get(&id))
            .or_else(|| self.graph.rows.get(&id))
            .map(Arc::as_ref)
            .ok_or(PreparedEffectInvariant::StaleReference)
    }

    pub(in crate::effect_row::prepared) fn predicate(
        self,
        reference: &PreparedEffectPredicateRef,
    ) -> Result<&'a PredicateTerm, PreparedEffectInvariant> {
        let id = self.graph.validate(&reference.0)?;
        self.extracted
            .and_then(|delta| delta.predicates.get(&id))
            .or_else(|| self.graph.predicates.get(&id))
            .map(Arc::as_ref)
            .ok_or(PreparedEffectInvariant::StaleReference)
    }
}
