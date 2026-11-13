//! Whole-row obligations under their original universal parameter scope.

use super::*;

#[cfg(test)]
mod tests;

#[derive(Debug)]
pub(super) struct ScopedObligation {
    pub(super) parameters: Box<[GenericEffectReference]>,
    pub(super) domain: PreparedEffectPredicate,
    pub(super) predicate: PreparedEffectPredicate,
}

impl PreparedEffectGraph {
    /// Retain D and C separately: pointwise implication of their membership
    /// formulas does not express implication between whole finite-row facts.
    pub(crate) fn add_scoped_obligation<C: DecisionControl>(
        &mut self,
        parameters: Vec<GenericEffectReference>,
        domain: PreparedEffectPredicate,
        predicate: PreparedEffectPredicate,
        control: &mut C,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        let mut work = Work(control);
        Self::validate_parameters(&parameters, &mut work)?;
        self.validate_predicate(&domain)?;
        self.validate_predicate(&predicate)?;
        work.charge(DecisionWork::Node)?;
        self.obligations.push(ScopedObligation {
            parameters: parameters.into_boxed_slice(),
            domain,
            predicate,
        });
        Ok(())
    }
}
