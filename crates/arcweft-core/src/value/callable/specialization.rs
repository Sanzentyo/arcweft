//! Runtime selection from the program's admitted specialization relation.

use super::{RuntimeCallableValue, RuntimeCallableValueError};
use crate::runtime_id::RuntimeCallableSpecializationId;
use crate::task::RuntimeProgramOwner;

#[cfg(test)]
mod tests;

impl RuntimeCallableValue {
    /// Specializes this exact program-owned value without evaluating a capture,
    /// argument, default, or body. The original retained order is re-admitted
    /// against the relation's target state before the new value is published.
    pub fn specialize(
        self,
        owner: &RuntimeProgramOwner,
        specialization: RuntimeCallableSpecializationId,
    ) -> Result<Self, RuntimeCallableValueError> {
        self.validate_for_owner(owner)?;
        let states = match owner {
            RuntimeProgramOwner::Plan(plan) => plan
                .callable_specializations()
                .get(specialization.index())
                .map(|definition| definition.states.as_ref()),
            RuntimeProgramOwner::Awbc(program) => program
                .callable_specializations
                .get(specialization.index())
                .map(|definition| definition.states.as_ref()),
        }
        .ok_or(RuntimeCallableValueError::MissingSpecialization { specialization })?;
        let target = states
            .iter()
            .find(|row| row.source == self.state)
            .map(|row| row.target)
            .ok_or(RuntimeCallableValueError::SpecializationSource {
                specialization,
                state: self.state,
            })?;
        Self::try_new(self.owner, target, self.retained)
    }
}
