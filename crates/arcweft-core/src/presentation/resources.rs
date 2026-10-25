//! General presentation custody. Token decoding never creates an entry here.
//!
//! The ledger owns the surface body, including its captures. Replaced or cleared
//! bodies move to their caller; an outstanding old handle keeps only its release
//! obligation and cannot clear the next occupant. There is no second slot index.

use super::RuntimePresentationError;
use crate::effect::RuntimeArtifactFingerprint;
use crate::runtime_id::{
    ExecutionInstanceId, RuntimeIdCursor, RuntimeIdExhausted, RuntimeIdNamespace,
    RuntimePersistentFiberId, RuntimePresentationHandleToken, RuntimeScopeInstanceId,
};
use crate::task::RuntimeProgramOwner;
use arcweft_id::PublicId;
use arcweft_interaction_model::presentation::PresentationResourceKind;
use std::collections::BTreeMap;
use std::num::NonZeroU64;
mod storage;
pub(crate) use storage::{PresentationAdmission, PresentationBody};

/// Dynamic owner, never a scope name or a reusable static instruction coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PresentationScopeOwner {
    execution: ExecutionInstanceId,
    fiber: RuntimePersistentFiberId,
    scope: RuntimeScopeInstanceId,
}

/// An inert reference to the resource grant of one reached lexical frame.
/// The ledger, rather than this retained reference, owns its active lifetime.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PresentationScopeBinding {
    program: RuntimeProgramOwner,
    owner: PresentationScopeOwner,
}

/// One installation authorized by the issuing ledger. Inert scope references
/// can be copied, but cannot construct or duplicate this installation proof.
pub(crate) struct PresentationScopeMutation(Option<PresentationScopeBinding>);

impl PresentationScopeMutation {
    pub(crate) fn into_binding(self) -> Option<PresentationScopeBinding> {
        self.0
    }
}

impl PresentationScopeBinding {
    pub(crate) fn admit_retained(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError> {
        if admission
            .program_owner()
            .is_none_or(|owner| !owner.same_program(&self.program))
        {
            return Err(crate::value::AwbcRuntimeValueSnapshotError::Message {
                message: "presentation scope requires its original executable lease".into(),
            });
        }
        admission.validate_metadata(&(self.owner.execution, self.owner.fiber, self.owner.scope))?;
        admission.validate_presentation_scope(self)
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct PresentationSlotAddress {
    target: PublicId,
    slot: PublicId,
    resource: PresentationResourceKind,
}

impl PresentationSlotAddress {
    pub(crate) fn try_new(
        target: PublicId,
        slot: PublicId,
        resource: PresentationResourceKind,
    ) -> Result<Self, RuntimePresentationError> {
        if !target.as_str().starts_with("target.") || !slot.as_str().starts_with("slot.") {
            return Err(RuntimePresentationError::InvalidAddress);
        }
        Ok(Self {
            target,
            slot,
            resource,
        })
    }
}

/// Slot replacement is meaningful only for a slot-bound registration. An
/// independent object or controller has its own issuance and never displaces
/// another resource merely because their resource families are equal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PresentationRegistrationSite {
    Slot(PresentationSlotAddress),
    Independent(PresentationResourceKind),
}

impl PresentationRegistrationSite {
    fn resource(&self) -> PresentationResourceKind {
        match self {
            Self::Slot(address) => address.resource,
            Self::Independent(resource) => *resource,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
struct PresentationLease<V> {
    token: RuntimePresentationHandleToken,
    owner: Option<PresentationScopeOwner>,
    site: PresentationRegistrationSite,
    body: Option<V>,
}

#[derive(Debug, PartialEq)]
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

/// Live resources have no Clone or wire reader. Rollback must admit the owning
/// executable and every retained body before projecting inert storage.
#[derive(Debug, PartialEq)]
pub(crate) struct PresentationLedger<V> {
    program: RuntimeProgramOwner,
    artifact: RuntimeArtifactFingerprint,
    next: RuntimeIdCursor,
    next_scope: RuntimeIdCursor,
    scopes: BTreeMap<RuntimeScopeInstanceId, (ExecutionInstanceId, RuntimePersistentFiberId)>,
    limits: PresentationLedgerLimits,
    leases: BTreeMap<NonZeroU64, PresentationLease<V>>,
}

/// A rejected registration returns the untouched owned body to its caller.
#[derive(Debug, PartialEq)]
pub(crate) struct PresentationRegisterError<V, E = RuntimePresentationError> {
    pub cause: E,
    pub body: V,
}

impl<V> PresentationLedger<V> {
    pub(crate) fn validate_retirement(
        &self,
        token: &RuntimePresentationHandleToken,
    ) -> Result<(), RuntimePresentationError> {
        if token.artifact() != &self.artifact
            || self.leases.contains_key(&token.issuance())
            || self
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
        if self.leases.len() >= self.limits.handles {
            return Err(RuntimePresentationError::Capacity);
        }
        let mut next = self.next;
        next.take_next(RuntimeIdNamespace::AffineOwner)?;
        Ok(())
    }
    pub(crate) fn bound_scope_body_count<VS>(&self, env: &crate::value::RuntimeEnv<VS>) -> usize {
        env.presentation_scope_binding().map_or(0, |binding| {
            self.leases
                .values()
                .filter(|lease| lease.owner == Some(binding.owner) && lease.body.is_some())
                .count()
        })
    }
    pub(crate) fn new(
        program: RuntimeProgramOwner,
        artifact: RuntimeArtifactFingerprint,
        limits: PresentationLedgerLimits,
    ) -> Self {
        Self {
            program,
            artifact,
            next: RuntimeIdCursor::initial(),
            next_scope: RuntimeIdCursor::initial(),
            scopes: BTreeMap::new(),
            limits,
            leases: BTreeMap::new(),
        }
    }

    /// Issues a resource grant only when a real lexical frame first needs it.
    /// Entering a resource-free pure scope requires no resource identity.
    pub(crate) fn bind_scope<VS>(
        &mut self,
        program: &RuntimeProgramOwner,
        execution: ExecutionInstanceId,
        fiber: RuntimePersistentFiberId,
        env: &mut crate::value::RuntimeEnv<VS>,
    ) -> Result<PresentationScopeOwner, RuntimePresentationError> {
        self.validate_program(program)?;
        if let Some(binding) = env.presentation_scope_binding() {
            if !binding.program.same_program(program)
                || binding.owner.execution != execution
                || binding.owner.fiber != fiber
            {
                return Err(RuntimePresentationError::Owner);
            }
            self.validate_scope(binding.owner)?;
            return Ok(binding.owner);
        }
        if self.scopes.len() >= self.limits.scopes {
            return Err(RuntimePresentationError::ScopeCapacity);
        }
        let mut next = self.next_scope;
        let scope = RuntimeScopeInstanceId::from_allocated(
            next.take_next(RuntimeIdNamespace::ScopeInstance)?,
        );
        let owner = PresentationScopeOwner {
            execution,
            fiber,
            scope,
        };
        env.bind_presentation_scope(PresentationScopeMutation(Some(PresentationScopeBinding {
            program: self.program.clone(),
            owner,
        })))?;
        self.scopes.insert(scope, (execution, fiber));
        self.next_scope = next;
        Ok(owner)
    }

    fn validate_scope(
        &self,
        owner: PresentationScopeOwner,
    ) -> Result<(), RuntimePresentationError> {
        if self.scopes.get(&owner.scope) != Some(&(owner.execution, owner.fiber)) {
            return Err(RuntimePresentationError::Owner);
        }
        Ok(())
    }

    pub(crate) fn exit_bound_scope<VS>(
        &mut self,
        program: &RuntimeProgramOwner,
        execution: ExecutionInstanceId,
        fiber: RuntimePersistentFiberId,
        env: &mut crate::value::RuntimeEnv<VS>,
    ) -> Result<Vec<PresentationRetirement<V>>, RuntimePresentationError> {
        self.validate_program(program)?;
        let Some(binding) = env.presentation_scope_binding() else {
            return Ok(Vec::new());
        };
        if !binding.program.same_program(program)
            || binding.owner.execution != execution
            || binding.owner.fiber != fiber
        {
            return Err(RuntimePresentationError::Owner);
        }
        let owner = binding.owner;
        let retired = self.exit_scope(program, owner)?;
        env.bind_presentation_scope(PresentationScopeMutation(None))?;
        Ok(retired)
    }

    fn validate_program(
        &self,
        program: &RuntimeProgramOwner,
    ) -> Result<(), RuntimePresentationError> {
        if !self.program.same_program(program) {
            return Err(RuntimePresentationError::Program);
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
        if token.artifact() != &self.artifact {
            return Err(RuntimePresentationError::Execution);
        }
        let Some(lease) = self.leases.get(&token.issuance()) else {
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
    ) -> Result<(RuntimePresentationHandleToken, Option<V>), PresentationRegisterError<V>> {
        let prepared = (|| {
            self.validate_program(program)?;
            self.validate_scope(owner)?;
            if self.leases.len() >= self.limits.handles {
                return Err(RuntimePresentationError::Capacity);
            }
            let mut next = self.next;
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
            self.artifact,
            site.resource(),
            owner.execution,
            owner.fiber,
            issuance,
        );
        self.leases.insert(
            issuance,
            PresentationLease {
                token: token.clone(),
                owner: Some(owner),
                site,
                body: Some(body),
            },
        );
        self.next = next;
        Ok((token, previous))
    }

    pub(crate) fn clear(
        &mut self,
        program: &RuntimeProgramOwner,
        address: &PresentationSlotAddress,
    ) -> Result<Option<V>, RuntimePresentationError> {
        self.validate_program(program)?;
        Ok(self.take_slot_body(address))
    }

    fn take_slot_body(&mut self, address: &PresentationSlotAddress) -> Option<V> {
        self.leases
            .values_mut()
            .find_map(|lease| match &lease.site {
                PresentationRegistrationSite::Slot(actual) if actual == address => {
                    lease.body.take()
                }
                _ => None,
            })
    }

    pub(crate) fn transfer(
        &mut self,
        program: &RuntimeProgramOwner,
        token: &RuntimePresentationHandleToken,
        current: PresentationScopeOwner,
        destination: Option<PresentationScopeOwner>,
    ) -> Result<(), RuntimePresentationError> {
        let issuance = self.validate_handle(program, token, Some(current))?;
        if let Some(destination) = destination {
            self.validate_scope(destination)?;
        }
        self.leases
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
            .leases
            .remove(&issuance)
            .expect("validated issuing record")
            .body)
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
        self.leases.retain(|_, lease| {
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
        self.scopes.remove(&owner.scope);
        Ok(bodies)
    }
}

impl PresentationScopeBinding {
    pub(crate) fn matches_context(
        &self,
        program: &RuntimeProgramOwner,
        execution: ExecutionInstanceId,
        fiber: RuntimePersistentFiberId,
    ) -> bool {
        self.program.same_program(program)
            && self.owner.execution == execution
            && self.owner.fiber == fiber
    }
    pub(crate) fn scope_key(
        &self,
    ) -> (
        ExecutionInstanceId,
        RuntimePersistentFiberId,
        RuntimeScopeInstanceId,
    ) {
        (self.owner.execution, self.owner.fiber, self.owner.scope)
    }
}

#[cfg(test)]
mod tests;
