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
    pub(super) fn from_admitted(parameters: BTreeSet<ConstraintGenericParameterId>) -> Self {
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

fn is_free(parameter: &ConstraintGenericParameterId) -> bool {
    matches!(
        parameter,
        ConstraintGenericParameterId::Type(GenericTypeReference::Free(_))
            | ConstraintGenericParameterId::Const(GenericConstReference::Free(_))
            | ConstraintGenericParameterId::Effect(GenericEffectReference::Free(_))
    )
}

impl<D: ConstraintDomain> ConstraintApplicationInventory<D> {
    pub(in crate::types::constraints) fn retain_rigid_captures<A: TypeConstraintAccounting>(
        &self,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<RetainedRigidGenericScope, TypeConstraintError> {
        let mut parameters = BTreeSet::new();
        for application in self.applications() {
            let scope = application.parameters().completed_contract();
            for reference in scope.rigid_type_references() {
                if matches!(reference, GenericTypeReference::Free(_)) {
                    context.enter_node()?;
                    parameters.insert(ConstraintGenericParameterId::Type(reference.clone()));
                }
            }
            for reference in scope.rigid_const_references() {
                if matches!(reference, GenericConstReference::Free(_)) {
                    context.enter_node()?;
                    parameters.insert(ConstraintGenericParameterId::Const(reference.clone()));
                }
            }
            for row in application.effects().variables() {
                if row.eligibility() == EffectConstraintEligibility::Rigid
                    && matches!(row.variable(), GenericEffectReference::Free(_))
                {
                    context.enter_node()?;
                    parameters.insert(ConstraintGenericParameterId::Effect(row.variable().clone()));
                }
            }
        }
        for lease in self.imported.iter() {
            for parameter in &lease.parameters {
                if is_free(parameter) && lease.contains(parameter) {
                    context.enter_node()?;
                    parameters.insert(parameter.clone());
                }
            }
        }
        for parameter in self.captures.parameters() {
            context.enter_node()?;
            parameters.insert(parameter.clone());
        }
        Ok(RetainedRigidGenericScope::from_admitted(parameters))
    }

    pub(in crate::types::constraints) fn restore_rigid_captures<A: TypeConstraintAccounting>(
        &mut self,
        captures: &RetainedRigidGenericScope,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(), TypeConstraintError> {
        self.captures = self.captures.union(captures, context)?;
        Ok(())
    }
}
