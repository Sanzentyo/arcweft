//! Read-only declaration captures retained after application inference closes.

use super::*;
use crate::types::constraints::{
    ConstraintGenericParameterId, TypeConstraintError,
    context::{TypeConstraintAccounting, TypeConstraintContext},
};

/// Only already admitted Free declaration references enter this carrier. It
/// has no application issuer, active inference variable or inference grant.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(in crate::types::constraints) struct RetainedRigidGenericScope {
    parameters: Arc<[ConstraintGenericParameterId]>,
}

impl RetainedRigidGenericScope {
    pub(in crate::types::constraints) fn from_admitted(
        parameters: BTreeSet<ConstraintGenericParameterId>,
    ) -> Self {
        Self {
            parameters: parameters.into_iter().collect(),
        }
    }

    pub(in crate::types::constraints) fn contains(
        &self,
        parameter: &ConstraintGenericParameterId,
    ) -> bool {
        self.parameters.binary_search(parameter).is_ok()
    }

    pub(in crate::types::constraints) fn len(&self) -> usize {
        self.parameters.len()
    }

    pub(in crate::types::constraints) fn parameters(&self) -> &[ConstraintGenericParameterId] {
        &self.parameters
    }

    pub(in crate::types::constraints) fn union<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        other: &Self,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self, TypeConstraintError> {
        if other.parameters.is_empty() {
            return Ok(self.clone());
        }
        if self.parameters.is_empty() {
            return Ok(other.clone());
        }
        let mut parameters = BTreeSet::new();
        for parameter in self.parameters.iter().chain(other.parameters.iter()) {
            context.enter_node()?;
            parameters.insert(parameter.clone());
        }
        Ok(Self::from_admitted(parameters))
    }
}

impl<D: ConstraintDomain> ConstraintApplicationInventory<D> {
    pub(in crate::types::constraints) fn restore_rigid_captures<A: TypeConstraintAccounting>(
        &mut self,
        captures: &RetainedRigidGenericScope,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(), TypeConstraintError> {
        self.captures = self.captures.union(captures, context)?;
        Ok(())
    }
}
