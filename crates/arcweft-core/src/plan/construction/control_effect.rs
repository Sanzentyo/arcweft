//! Aggregate construction of source-ordered static control/effect contracts.

use std::sync::Arc;

use super::{RuntimePlanBuildError, RuntimePlanBuilder, seed::RuntimePlanConstructionIssuer};
use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::{
    RuntimeControlEffectContract, RuntimeControlEffectContractDefinition,
    RuntimeControlEffectContractError, RuntimeControlEffectContractId, RuntimeControlEffectRow,
};
use crate::runtime_id::RuntimePlanTypeId;

/// Construction-only capability. Ordinal equality is insufficient to resolve
/// a child: every use must retain this builder's private issuer.
#[derive(Clone, Debug)]
pub struct RuntimeControlEffectContractSeedId {
    issuer: Arc<RuntimePlanConstructionIssuer>,
    id: RuntimeControlEffectContractId,
}

impl RuntimeControlEffectContractSeedId {
    /// Table reference for lookup after the aggregate has successfully finished.
    #[must_use]
    pub const fn id(&self) -> RuntimeControlEffectContractId {
        self.id
    }

    fn resolve(
        &self,
        builder: &RuntimePlanBuilder,
    ) -> Result<RuntimeControlEffectContractId, RuntimePlanBuildError> {
        if !Arc::ptr_eq(&self.issuer, &builder.issuer) {
            return Err(RuntimeControlEffectContractError::ForeignSeed.into());
        }
        if self.id.index() >= builder.control_effect_contracts.len() {
            return Err(RuntimeControlEffectContractError::UnknownChild {
                index: self.id.index(),
            }
            .into());
        }
        Ok(self.id)
    }
}

pub type RuntimeControlEffectContractSeed = RuntimeControlEffectContractDefinition<
    RuntimeSemanticTypeId,
    RuntimeControlEffectContractSeedId,
>;

impl RuntimePlanBuilder {
    /// Reserves declaration order before resolving possibly forward child edges.
    pub fn reserve_control_effect_contract(
        &mut self,
    ) -> Result<RuntimeControlEffectContractSeedId, RuntimePlanBuildError> {
        self.ensure_usable()?;
        let id = RuntimeControlEffectContractId::for_index(self.control_effect_contracts.len())
            .ok_or(RuntimeControlEffectContractError::IdentityExhausted)?;
        self.control_effect_contracts.push(None);
        Ok(RuntimeControlEffectContractSeedId {
            issuer: Arc::clone(&self.issuer),
            id,
        })
    }

    /// Defines a reserved row only after all its type/child references resolve.
    /// A rejected definition leaves the reservation available for a correction.
    pub fn define_control_effect_contract(
        &mut self,
        handle: &RuntimeControlEffectContractSeedId,
        seed: RuntimeControlEffectContractSeed,
    ) -> Result<(), RuntimePlanBuildError> {
        self.ensure_usable()?;
        let id = handle.resolve(self)?;
        if self.control_effect_contracts[id.index()].is_some() {
            return Err(
                RuntimeControlEffectContractError::AlreadyDefined { index: id.index() }.into(),
            );
        }
        let row = self.admit_control_effect_definition(seed)?;
        self.control_effect_contracts[id.index()] = Some(row);
        Ok(())
    }

    /// Admits one complete row atomically, without leaving a reservation when
    /// any input is rejected. No caller supplies a completed semantic digest.
    pub fn push_control_effect_contract(
        &mut self,
        seed: RuntimeControlEffectContractSeed,
    ) -> Result<RuntimeControlEffectContractSeedId, RuntimePlanBuildError> {
        self.ensure_usable()?;
        let row = self.admit_control_effect_definition(seed)?;
        let id = RuntimeControlEffectContractId::for_index(self.control_effect_contracts.len())
            .ok_or(RuntimeControlEffectContractError::IdentityExhausted)?;
        self.control_effect_contracts.push(Some(row));
        Ok(RuntimeControlEffectContractSeedId {
            issuer: Arc::clone(&self.issuer),
            id,
        })
    }

    fn admit_control_effect_definition(
        &self,
        seed: RuntimeControlEffectContractSeed,
    ) -> Result<RuntimeControlEffectContract, RuntimePlanBuildError> {
        let effects = seed
            .effects
            .into_vec()
            .into_iter()
            .map(|row| {
                let inputs = row
                    .inputs
                    .into_vec()
                    .into_iter()
                    .map(|ty| self.control_effect_type(ty))
                    .collect::<Result<Box<[_]>, _>>()?;
                let output = row
                    .output
                    .map(|ty| self.control_effect_type(ty))
                    .transpose()?;
                Ok(RuntimeControlEffectRow {
                    kind: row.kind,
                    identity: row.identity,
                    inputs,
                    output,
                    cardinality: row.cardinality,
                    ordering: row.ordering,
                    cancellation: row.cancellation,
                    terminal: row.terminal,
                })
            })
            .collect::<Result<Box<[_]>, RuntimePlanBuildError>>()?;
        let children = seed
            .children
            .iter()
            .map(|child| child.resolve(self))
            .collect::<Result<Box<[_]>, _>>()?;
        Ok(RuntimeControlEffectContract::new(
            RuntimeControlEffectContractDefinition {
                mode: seed.mode,
                effects,
                children,
            },
        ))
    }

    fn control_effect_type(
        &self,
        semantic_identity: RuntimeSemanticTypeId,
    ) -> Result<RuntimePlanTypeId, RuntimePlanBuildError> {
        let ty = self
            .types
            .id_for_semantic(semantic_identity)
            .ok_or(RuntimePlanBuildError::UnknownSemanticType { semantic_identity })?;
        Ok(ty)
    }
}
