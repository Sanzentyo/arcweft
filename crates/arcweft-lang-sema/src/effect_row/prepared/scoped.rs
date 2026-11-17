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

/// One immutable original constraint and its complete universal Γ. It grants
/// no mutation or completion authority. Clones keep the same original fact.
#[derive(Clone, Debug)]
pub(crate) struct PreparedEffectScopedConstraint {
    pub(super) owner: PreparedEffectGraphOwner,
    pub(super) obligation: Arc<ScopedObligation>,
}

impl PreparedEffectScopedConstraint {
    pub(crate) fn domain(&self) -> &PreparedEffectPredicate {
        &self.obligation.domain
    }

    pub(crate) fn predicate(&self) -> &PreparedEffectPredicate {
        &self.obligation.predicate
    }
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
        self.changed()?;
        self.relation.obligations.push(Arc::new(ScopedObligation {
            parameters: parameters.into_boxed_slice(),
            domain,
            predicate,
        }));
        Ok(())
    }
}
