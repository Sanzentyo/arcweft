//! Issuer-bound admission of checked callable type specializations.

use std::{fmt, sync::Arc};

use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::RuntimeCallableSpecializationDefinition;
use crate::runtime_id::{
    RuntimeCallableSpecializationId, RuntimeCallableStateId, RuntimePlanTypeId,
};

use super::seed::RuntimePlanConstructionIssuer;
use super::{RuntimeCallableStateSeedId, RuntimePlanBuildError, RuntimePlanBuilder};

/// Construction-only specialization of existing callable states. It cannot
/// introduce code or capture expressions.
pub type RuntimeCallableSpecializationSeed =
    RuntimeCallableSpecializationDefinition<RuntimeSemanticTypeId, RuntimeCallableStateSeedId>;

/// An admitted specialization owned by exactly one plan builder.
#[derive(Clone)]
pub struct RuntimeCallableSpecializationSeedId {
    issuer: Arc<RuntimePlanConstructionIssuer>,
    specialization: RuntimeCallableSpecializationId,
}

impl fmt::Debug for RuntimeCallableSpecializationSeedId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("RuntimeCallableSpecializationSeedId")
            .field(&self.specialization)
            .finish()
    }
}

impl PartialEq for RuntimeCallableSpecializationSeedId {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.issuer, &other.issuer) && self.specialization == other.specialization
    }
}

impl Eq for RuntimeCallableSpecializationSeedId {}

impl RuntimePlanBuilder {
    /// Publishes one inert relation after all referenced states have been
    /// reserved. Complete substitution and state-pair validation is atomic
    /// with final program admission.
    pub fn push_callable_specialization_seed(
        &mut self,
        definition: RuntimeCallableSpecializationSeed,
    ) -> Result<RuntimeCallableSpecializationSeedId, RuntimePlanBuildError> {
        self.ensure_usable()?;
        let specialization =
            RuntimeCallableSpecializationId::from_zero_based(self.callable_specializations.len())
                .ok_or(RuntimePlanBuildError::CallableSpecializationIdentityExhausted)?;
        let definition = definition.try_map(
            |semantic_identity| {
                self.types
                    .id_for_semantic(semantic_identity)
                    .ok_or(RuntimePlanBuildError::UnknownSemanticType { semantic_identity })
            },
            |state| self.resolve_callable_state_seed(&state),
        )?;
        self.callable_specializations.push(definition);
        Ok(RuntimeCallableSpecializationSeedId {
            issuer: Arc::clone(&self.issuer),
            specialization,
        })
    }

    pub(super) fn resolve_callable_specialization_seed(
        &self,
        handle: &RuntimeCallableSpecializationSeedId,
    ) -> Result<
        (
            RuntimeCallableSpecializationId,
            &RuntimeCallableSpecializationDefinition<RuntimePlanTypeId, RuntimeCallableStateId>,
        ),
        RuntimePlanBuildError,
    > {
        if !Arc::ptr_eq(&self.issuer, &handle.issuer) {
            return Err(RuntimePlanBuildError::ForeignCallableSpecializationSeed);
        }
        self.callable_specializations
            .get(handle.specialization.index())
            .map(|definition| (handle.specialization, definition))
            .ok_or(RuntimePlanBuildError::ForeignCallableSpecializationSeed)
    }
}
