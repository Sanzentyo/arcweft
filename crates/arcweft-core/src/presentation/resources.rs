//! General presentation custody. Token decoding never creates an entry here.
//!
//! The ledger owns the surface body, including its captures. Replaced or cleared
//! bodies move to their caller; an outstanding old handle keeps only its release
//! obligation and cannot clear the next occupant. There is no second slot index.

use super::RuntimePresentationError;
use crate::effect::RuntimeArtifactFingerprint;
use crate::runtime_id::{
    ExecutionInstanceId, RuntimeIdCursor, RuntimeIdExhausted, RuntimeIdNamespace,
    RuntimePersistentFiberId, RuntimePresentationBankId, RuntimePresentationHandleToken,
    RuntimeScopeInstanceId,
};
use crate::task::RuntimeProgramOwner;
#[cfg(test)]
use arcweft_id::PublicId;
use arcweft_interaction_model::presentation::{PresentationResourceKind, PresentationSlotAddress};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::num::NonZeroU64;
mod bank_rows;
mod banks;
mod execution;
mod scope;
mod storage;
pub(crate) use banks::PresentationBanks;
pub(crate) use banks::PresentationBanksSaveSnapshot;
pub(crate) use execution::{PresentationExecutionAdmission, PresentationExecutionRole};
use scope::PresentationScopeGrant;
pub(crate) use scope::{
    PresentationScopeAnchor, PresentationScopeExit, PresentationScopeOwner, PresentationScopeRef,
    PresentationScopeStorage, PresentationScopeTarget,
};
pub use scope::{RuntimePresentationScopeBinding, RuntimePresentationScopeSnapshot};
pub(crate) use storage::{
    PresentationAdmission, PresentationBody, PresentationSlotRemovalAdmission,
};

/// Slot replacement is meaningful only for a slot-bound registration. An
/// independent object or controller has its own issuance and never displaces
/// another resource merely because their resource families are equal.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) enum PresentationRegistrationSite {
    Slot(PresentationSlotAddress),
    Independent(PresentationResourceKind),
}

impl PresentationRegistrationSite {
    fn resource(&self) -> PresentationResourceKind {
        match self {
            Self::Slot(address) => address.resource(),
            Self::Independent(resource) => *resource,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PresentationLease<V> {
    token: RuntimePresentationHandleToken,
    owner: Option<PresentationScopeOwner>,
    site: PresentationRegistrationSite,
    /// Authored visibility remains in the original body. Only an explicit
    /// handle command overrides it; lifecycle state has this one owner.
    visibility_override: Option<bool>,
    body: Option<V>,
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PresentationRetirement<V> {
    pub token: RuntimePresentationHandleToken,
    pub body: V,
}

impl<V> PresentationRetirement<V> {
    pub(crate) fn into_body(self) -> V {
        self.body
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PresentationLedgerLimits {
    pub handles: usize,
    pub scopes: usize,
}

impl PresentationLedgerLimits {
    pub(crate) fn for_schema(limits: crate::entry::RuntimeSchemaLimits) -> Self {
        Self {
            handles: limits.max_sequence_items as usize,
            scopes: limits.max_sequence_items as usize,
        }
    }
}

/// Live resources have no Clone or wire reader. Rollback must admit the owning
/// executable and every retained body before projecting inert storage.
#[derive(Debug, PartialEq)]
pub(crate) struct PresentationLedger<V> {
    program: RuntimeProgramOwner,
    bank: RuntimePresentationBankId,
    /// Trusted execution policy; decoded state cannot choose its own limits.
    limits: PresentationLedgerLimits,
    state: PresentationLedgerState<V>,
}

/// The single resource-state grammar, shared by the execution lease and inert
/// save storage. Decoding state cannot attach a program or issue a live grant.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(
    deny_unknown_fields,
    bound(serialize = "V: Serialize", deserialize = "V: Deserialize<'de>")
)]
pub(crate) struct PresentationLedgerState<V> {
    artifact: RuntimeArtifactFingerprint,
    next: RuntimeIdCursor,
    next_scope: RuntimeIdCursor,
    #[serde(deserialize_with = "storage::deserialize_unique_map")]
    scopes: BTreeMap<
        RuntimeScopeInstanceId,
        (
            ExecutionInstanceId,
            RuntimePersistentFiberId,
            PresentationScopeAnchor,
        ),
    >,
    #[serde(deserialize_with = "storage::deserialize_unique_map")]
    leases: BTreeMap<NonZeroU64, PresentationLease<V>>,
}

impl<V> AsRef<PresentationLedgerState<V>> for PresentationLedger<V> {
    fn as_ref(&self) -> &PresentationLedgerState<V> {
        &self.state
    }
}

impl<V> AsRef<PresentationLedgerState<V>> for PresentationLedgerState<V> {
    fn as_ref(&self) -> &PresentationLedgerState<V> {
        self
    }
}

/// A rejected registration returns the untouched owned body to its caller.
#[derive(Debug, PartialEq)]
pub(crate) struct PresentationRegisterError<V, E = RuntimePresentationError> {
    pub cause: E,
    pub body: V,
}

impl<V> PresentationLedger<V> {
    pub(crate) const fn bank(&self) -> RuntimePresentationBankId {
        self.bank
    }

    pub(crate) fn validate_retirement(
        &self,
        token: &RuntimePresentationHandleToken,
    ) -> Result<(), RuntimePresentationError> {
        if token.artifact() != &self.state.artifact
            || token.bank() != self.bank
            || self
                .state
                .leases
                .get(&token.issuance())
                .is_some_and(|lease| lease.token != *token || lease.body.is_some())
            || self
                .state
                .next
                .last_issued()
                .is_none_or(|last| token.issuance() > last)
        {
            return Err(RuntimePresentationError::InvalidState);
        }
        Ok(())
    }
    pub(crate) fn preflight_register(
        &self,
        program: &RuntimeProgramOwner,
    ) -> Result<(), RuntimePresentationError> {
        self.validate_program(program)?;
        if self.state.leases.len() >= self.limits.handles {
            return Err(RuntimePresentationError::Capacity);
        }
        let mut next = self.state.next;
        next.take_next(RuntimeIdNamespace::AffineOwner)?;
        Ok(())
    }
    pub(crate) fn new(
        program: RuntimeProgramOwner,
        artifact: RuntimeArtifactFingerprint,
        bank: RuntimePresentationBankId,
        limits: PresentationLedgerLimits,
    ) -> Self {
        Self {
            program,
            bank,
            limits,
            state: PresentationLedgerState {
                artifact,
                next: RuntimeIdCursor::initial(),
                next_scope: RuntimeIdCursor::initial(),
                scopes: BTreeMap::new(),
                leases: BTreeMap::new(),
            },
        }
    }

    /// Issues a resource grant only when a real lexical frame first needs it.
    /// Entering a resource-free pure scope requires no resource identity.
    pub(crate) fn bind_scope(
        &mut self,
        program: &RuntimeProgramOwner,
        execution: ExecutionInstanceId,
        fiber: RuntimePersistentFiberId,
        mut target: PresentationScopeTarget<'_>,
    ) -> Result<PresentationScopeOwner, RuntimePresentationError> {
        let owner = self.preflight_bind_scope(program, execution, fiber, target.as_ref(), 0)?;
        if target
            .binding()
            .is_some_and(|binding| binding.grants.contains_key(&self.bank))
        {
            return Ok(owner);
        }
        let mut next = self.state.next_scope;
        let scope = RuntimeScopeInstanceId::from_allocated(
            next.take_next(RuntimeIdNamespace::ScopeInstance)?,
        );
        debug_assert_eq!(scope, owner.scope);
        target.insert(
            &self.program,
            PresentationScopeGrant {
                owner,
                anchor: target.anchor,
            },
        );
        self.state
            .scopes
            .insert(scope, (execution, fiber, target.anchor));
        self.state.next_scope = next;
        Ok(owner)
    }

    /// Inspect a receiving scope before old grants leave custody. The caller's
    /// actual exit contour supplies the number of grants released in this bank.
    fn preflight_bind_scope(
        &self,
        program: &RuntimeProgramOwner,
        execution: ExecutionInstanceId,
        fiber: RuntimePersistentFiberId,
        target: PresentationScopeRef<'_>,
        released_scopes: usize,
    ) -> Result<PresentationScopeOwner, RuntimePresentationError> {
        self.validate_bound_scope(
            program,
            execution,
            fiber,
            PresentationScopeRef {
                binding: target.binding,
                anchor: target.anchor,
            },
        )?;
        if let Some(grant) = target
            .binding
            .and_then(|binding| binding.grants.get(&self.bank))
        {
            return Ok(grant.owner);
        }
        let remaining = self
            .state
            .scopes
            .len()
            .checked_sub(released_scopes)
            .ok_or(RuntimePresentationError::InvalidState)?;
        if remaining >= self.limits.scopes {
            return Err(RuntimePresentationError::ScopeCapacity);
        }
        let mut next = self.state.next_scope;
        let scope = RuntimeScopeInstanceId::from_allocated(
            next.take_next(RuntimeIdNamespace::ScopeInstance)?,
        );
        Ok(PresentationScopeOwner {
            bank: self.bank,
            execution,
            fiber,
            scope,
        })
    }

    fn validate_scope(
        &self,
        owner: PresentationScopeOwner,
    ) -> Result<(), RuntimePresentationError> {
        if owner.bank != self.bank
            || self
                .state
                .scopes
                .get(&owner.scope)
                .is_none_or(|record| record.0 != owner.execution || record.1 != owner.fiber)
        {
            return Err(RuntimePresentationError::Owner);
        }
        Ok(())
    }

    pub(crate) fn exit_bound_scope(
        &mut self,
        program: &RuntimeProgramOwner,
        execution: ExecutionInstanceId,
        fiber: RuntimePersistentFiberId,
        mut target: PresentationScopeTarget<'_>,
    ) -> Result<Vec<PresentationRetirement<V>>, RuntimePresentationError> {
        self.validate_bound_scope(program, execution, fiber, target.as_ref())?;
        let Some(grant) = target
            .binding()
            .and_then(|binding| binding.grants.get(&self.bank))
        else {
            return Ok(Vec::new());
        };
        let owner = grant.owner;
        let retired = self.exit_scope(program, owner)?;
        target.remove(self.bank);
        Ok(retired)
    }

    /// Checks the actual storage anchor as well as the issuing ledger record.
    /// Batch exits use this before taking any body or clearing any grant.
    fn validate_bound_scope(
        &self,
        program: &RuntimeProgramOwner,
        execution: ExecutionInstanceId,
        fiber: RuntimePersistentFiberId,
        target: PresentationScopeRef<'_>,
    ) -> Result<(), RuntimePresentationError> {
        self.validate_program(program)?;
        let Some(binding) = target.binding else {
            return Ok(());
        };
        if binding.grants.is_empty() {
            return Err(RuntimePresentationError::InvalidState);
        }
        if !binding.program.same_program(program) {
            return Err(RuntimePresentationError::Owner);
        }
        let Some(grant) = binding.grants.get(&self.bank) else {
            return Ok(());
        };
        if grant.owner.execution != execution
            || grant.owner.fiber != fiber
            || grant.anchor != target.anchor
            || self.state.scopes.get(&grant.owner.scope) != Some(&(execution, fiber, target.anchor))
        {
            return Err(RuntimePresentationError::Owner);
        }
        self.validate_scope(grant.owner)
    }

    fn validate_program(
        &self,
        program: &RuntimeProgramOwner,
    ) -> Result<(), RuntimePresentationError> {
        if !self.program.same_program(program) {
            return Err(RuntimePresentationError::Program);
        }
        if !self.bank.matches_artifact(self.state.artifact) {
            return Err(RuntimePresentationError::Execution);
        }
        Ok(())
    }

    fn validate_handle(
        &self,
        program: &RuntimeProgramOwner,
        token: &RuntimePresentationHandleToken,
        owner: Option<PresentationScopeOwner>,
    ) -> Result<NonZeroU64, RuntimePresentationError> {
        self.validate_program(program)?;
        if token.artifact() != &self.state.artifact || token.bank() != self.bank {
            return Err(RuntimePresentationError::Execution);
        }
        let Some(lease) = self.state.leases.get(&token.issuance()) else {
            return Err(RuntimePresentationError::UnknownHandle);
        };
        if lease.token.execution() != token.execution() {
            return Err(RuntimePresentationError::Execution);
        }
        if lease.token != *token {
            return Err(RuntimePresentationError::UnknownHandle);
        }
        if lease.owner != owner {
            return Err(RuntimePresentationError::Owner);
        }
        if let Some(owner) = owner {
            self.validate_scope(owner)?;
        }
        Ok(token.issuance())
    }

    /// Source/type/resource admission precedes this move. Ledger capacity and
    /// identity admission precede any mutation or publication of either body.
    pub(crate) fn register(
        &mut self,
        program: &RuntimeProgramOwner,
        owner: PresentationScopeOwner,
        site: PresentationRegistrationSite,
        body: V,
    ) -> Result<
        (
            RuntimePresentationHandleToken,
            Option<PresentationRetirement<V>>,
        ),
        PresentationRegisterError<V>,
    > {
        let prepared = (|| {
            self.validate_program(program)?;
            self.validate_scope(owner)?;
            if self.state.leases.len() >= self.limits.handles {
                return Err(RuntimePresentationError::Capacity);
            }
            let mut next = self.state.next;
            let issuance = next.take_next(RuntimeIdNamespace::AffineOwner)?;
            Ok((issuance, next))
        })();
        let (issuance, next) = match prepared {
            Ok(prepared) => prepared,
            Err(cause) => return Err(PresentationRegisterError { cause, body }),
        };
        let previous = match &site {
            PresentationRegistrationSite::Slot(address) => self.take_slot_body(address),
            PresentationRegistrationSite::Independent(_) => None,
        };
        let token = RuntimePresentationHandleToken::from_issued(
            self.state.artifact,
            site.resource(),
            owner.execution,
            owner.fiber,
            issuance,
            self.bank,
        );
        self.state.leases.insert(
            issuance,
            PresentationLease {
                token: token.clone(),
                owner: Some(owner),
                site,
                visibility_override: None,
                body: Some(body),
            },
        );
        self.state.next = next;
        Ok((token, previous))
    }

    pub(crate) fn clear(
        &mut self,
        program: &RuntimeProgramOwner,
        address: &PresentationSlotAddress,
    ) -> Result<Option<V>, RuntimePresentationError> {
        self.validate_program(program)?;
        Ok(self
            .take_slot_body(address)
            .map(PresentationRetirement::into_body))
    }

    fn take_slot_body(
        &mut self,
        address: &PresentationSlotAddress,
    ) -> Option<PresentationRetirement<V>> {
        self.state
            .leases
            .values_mut()
            .find_map(|lease| match &lease.site {
                PresentationRegistrationSite::Slot(actual) if actual == address => {
                    lease.body.take().map(|body| PresentationRetirement {
                        token: lease.token.clone(),
                        body,
                    })
                }
                _ => None,
            })
    }

    pub(crate) fn transfer(
        &mut self,
        program: &RuntimeProgramOwner,
        token: &RuntimePresentationHandleToken,
        current: Option<PresentationScopeOwner>,
        destination: Option<PresentationScopeOwner>,
    ) -> Result<(), RuntimePresentationError> {
        let issuance = self.validate_handle(program, token, current)?;
        if let Some(destination) = destination {
            self.validate_scope(destination)?;
        }
        self.state
            .leases
            .get_mut(&issuance)
            .expect("validated issuing record")
            .owner = destination;
        Ok(())
    }

    pub(crate) fn release(
        &mut self,
        program: &RuntimeProgramOwner,
        token: &RuntimePresentationHandleToken,
        owner: Option<PresentationScopeOwner>,
    ) -> Result<Option<V>, RuntimePresentationError> {
        let issuance = self.validate_handle(program, token, owner)?;
        Ok(self
            .state
            .leases
            .remove(&issuance)
            .expect("validated issuing record")
            .body)
    }

    /// Validates the original handle and its actual dynamic grant before any
    /// lifecycle mutation. A displaced handle retains only its release duty.
    fn preflight_handle_method(
        &self,
        program: &RuntimeProgramOwner,
        token: &RuntimePresentationHandleToken,
        owner: Option<PresentationScopeOwner>,
        method: arcweft_interaction_model::presentation::PresentationHandleMethodId,
    ) -> Result<NonZeroU64, RuntimePresentationError> {
        use arcweft_interaction_model::presentation::{
            PresentationHandleMethodId, PresentationHandleOperation,
        };
        let issuance = self.validate_handle(program, token, owner)?;
        if method == PresentationHandleMethodId::OverlayPop
            && token.resource() != PresentationResourceKind::Overlay
        {
            return Err(RuntimePresentationError::InvalidState);
        }
        if matches!(
            method.operation(),
            PresentationHandleOperation::SetVisibility(_)
        ) && self.state.leases[&issuance].body.is_none()
        {
            return Err(RuntimePresentationError::InvalidState);
        }
        Ok(issuance)
    }

    fn apply_handle_method(
        &mut self,
        program: &RuntimeProgramOwner,
        token: &RuntimePresentationHandleToken,
        owner: Option<PresentationScopeOwner>,
        method: arcweft_interaction_model::presentation::PresentationHandleMethodId,
    ) -> Result<Option<V>, RuntimePresentationError> {
        use arcweft_interaction_model::presentation::PresentationHandleOperation;
        let issuance = self.preflight_handle_method(program, token, owner, method)?;
        match method.operation() {
            PresentationHandleOperation::SetVisibility(visible) => {
                self.state
                    .leases
                    .get_mut(&issuance)
                    .expect("exclusive admission retains the original handle")
                    .visibility_override = Some(visible);
                Ok(None)
            }
            PresentationHandleOperation::Retire => self.release(program, token, owner),
        }
    }

    /// Exits only the exact dynamic owner, in deterministic issuance order.
    pub(crate) fn exit_scope(
        &mut self,
        program: &RuntimeProgramOwner,
        owner: PresentationScopeOwner,
    ) -> Result<Vec<PresentationRetirement<V>>, RuntimePresentationError> {
        self.validate_program(program)?;
        self.validate_scope(owner)?;
        let mut bodies = Vec::new();
        self.state.leases.retain(|_, lease| {
            if lease.owner != Some(owner) {
                return true;
            }
            if let Some(body) = lease.body.take() {
                bodies.push(PresentationRetirement {
                    token: lease.token.clone(),
                    body,
                });
            }
            false
        });
        self.state.scopes.remove(&owner.scope);
        Ok(bodies)
    }
}

#[cfg(test)]
mod tests;
