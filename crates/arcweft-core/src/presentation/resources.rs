//! General presentation custody. Token decoding never creates an entry here.
//!
//! The ledger owns the surface body, including its captures. Replaced or cleared
//! bodies move to their caller; an outstanding old handle keeps only its release
//! obligation and cannot clear the next occupant. There is no second slot index.

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
use thiserror::Error;

/// Dynamic owner, never a scope name or a reusable static instruction coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PresentationScopeOwner {
    pub fiber: RuntimePersistentFiberId,
    pub scope: RuntimeScopeInstanceId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
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
    ) -> Result<Self, PresentationLedgerError> {
        if !target.as_str().starts_with("target.") || !slot.as_str().starts_with("slot.") {
            return Err(PresentationLedgerError::InvalidAddress);
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

/// Live resources have no Clone or wire reader. Rollback must admit the owning
/// executable and every retained body before projecting inert storage.
#[derive(Debug, PartialEq)]
pub(crate) struct PresentationLedger<V> {
    program: RuntimeProgramOwner,
    artifact: RuntimeArtifactFingerprint,
    execution: ExecutionInstanceId,
    next: RuntimeIdCursor,
    capacity: usize,
    leases: BTreeMap<NonZeroU64, PresentationLease<V>>,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum PresentationLedgerError {
    #[error("presentation operation belongs to another executable lease")]
    Program,
    #[error("presentation handle belongs to another execution or artifact")]
    Execution,
    #[error("presentation handle has no exact live issuing record")]
    UnknownHandle,
    #[error("presentation handle is not owned by the selected dynamic scope")]
    Owner,
    #[error("presentation target/slot is outside its public identity family")]
    InvalidAddress,
    #[error("presentation live handle limit is exhausted")]
    Capacity,
    #[error(transparent)]
    Identity(#[from] RuntimeIdExhausted),
}

/// A rejected registration returns the untouched owned body to its caller.
#[derive(Debug, PartialEq)]
pub(crate) struct PresentationRegisterError<V> {
    pub cause: PresentationLedgerError,
    pub body: V,
}

impl<V> PresentationLedger<V> {
    pub(crate) fn new(
        program: RuntimeProgramOwner,
        artifact: RuntimeArtifactFingerprint,
        execution: ExecutionInstanceId,
        capacity: usize,
    ) -> Self {
        Self {
            program,
            artifact,
            execution,
            next: RuntimeIdCursor::initial(),
            capacity,
            leases: BTreeMap::new(),
        }
    }

    fn validate_program(
        &self,
        program: &RuntimeProgramOwner,
    ) -> Result<(), PresentationLedgerError> {
        if !self.program.same_program(program) {
            return Err(PresentationLedgerError::Program);
        }
        Ok(())
    }

    fn validate_handle(
        &self,
        program: &RuntimeProgramOwner,
        token: &RuntimePresentationHandleToken,
        owner: Option<PresentationScopeOwner>,
    ) -> Result<NonZeroU64, PresentationLedgerError> {
        self.validate_program(program)?;
        if token.execution() != self.execution || token.artifact() != &self.artifact {
            return Err(PresentationLedgerError::Execution);
        }
        let Some(lease) = self.leases.get(&token.issuance()) else {
            return Err(PresentationLedgerError::UnknownHandle);
        };
        if lease.token != *token {
            return Err(PresentationLedgerError::UnknownHandle);
        }
        if lease.owner != owner {
            return Err(PresentationLedgerError::Owner);
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
            if self.leases.len() >= self.capacity {
                return Err(PresentationLedgerError::Capacity);
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
            self.execution,
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
    ) -> Result<Option<V>, PresentationLedgerError> {
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
    ) -> Result<(), PresentationLedgerError> {
        let issuance = self.validate_handle(program, token, Some(current))?;
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
    ) -> Result<Option<V>, PresentationLedgerError> {
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
    ) -> Result<Vec<V>, PresentationLedgerError> {
        self.validate_program(program)?;
        let mut bodies = Vec::new();
        self.leases.retain(|_, lease| {
            if lease.owner != Some(owner) {
                return true;
            }
            bodies.extend(lease.body.take());
            false
        });
        Ok(bodies)
    }
}

#[cfg(test)]
mod tests;
