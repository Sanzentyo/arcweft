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
use arcweft_id::PublicId;
use arcweft_interaction_model::presentation::PresentationResourceKind;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::num::NonZeroU64;
mod banks;
mod storage;
pub(crate) use banks::PresentationBanks;
pub(crate) use banks::PresentationBanksSaveSnapshot;
pub(crate) use storage::{PresentationAdmission, PresentationBody};

/// Dynamic owner, never a scope name or a reusable static instruction coordinate.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PresentationScopeOwner {
    bank: RuntimePresentationBankId,
    execution: ExecutionInstanceId,
    fiber: RuntimePersistentFiberId,
    scope: RuntimeScopeInstanceId,
}

/// A native grant remains private with its original lexical storage. Public
/// AWBC scope images additionally bind the original dynamic frame and its
/// verified lexical declaration, so exchanging two headers cannot move custody.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", deny_unknown_fields, rename_all = "snake_case")]
pub(crate) enum PresentationScopeAnchor {
    Native,
    Awbc {
        frame: crate::runtime_id::RuntimeFrameInstanceId,
        scope: Option<crate::awbc::schema::AwbcScopeId>,
    },
}

pub(crate) trait PresentationScopeStorage {
    fn admit_awbc_scope(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
        fiber: crate::runtime_id::RuntimeFiberInstanceId,
        frame: crate::runtime_id::RuntimeFrameInstanceId,
        scope: Option<crate::awbc::schema::AwbcScopeId>,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError>;
}

impl PresentationScopeOwner {
    fn admit_metadata(
        &self,
        anchor: &PresentationScopeAnchor,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError> {
        admission.validate_metadata(&(self, anchor))
    }

    fn validate_awbc_anchor(
        self,
        anchor: PresentationScopeAnchor,
        fiber: crate::runtime_id::RuntimeFiberInstanceId,
        frame: crate::runtime_id::RuntimeFrameInstanceId,
        scope: Option<crate::awbc::schema::AwbcScopeId>,
    ) -> Result<(), RuntimePresentationError> {
        if self.execution.get() != fiber.get()
            || self.fiber.get() != fiber.get().get()
            || anchor != (PresentationScopeAnchor::Awbc { frame, scope })
        {
            return Err(RuntimePresentationError::Owner);
        }
        Ok(())
    }
}

/// An inert reference to the resource grant of one reached lexical frame.
/// The ledger, rather than this retained reference, owns its active lifetime.
/// Its program lease is retained only in memory; wire input cannot issue it.
///
/// ```compile_fail
/// use arcweft_core::presentation::RuntimePresentationScopeBinding;
/// let _: RuntimePresentationScopeBinding = serde_json::from_str("{}").unwrap();
/// ```
///
/// ```compile_fail
/// use arcweft_core::presentation::RuntimePresentationScopeBinding;
/// fn encode(binding: &RuntimePresentationScopeBinding) { let _ = serde_json::to_vec(binding); }
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimePresentationScopeBinding {
    program: RuntimeProgramOwner,
    owner: PresentationScopeOwner,
    anchor: PresentationScopeAnchor,
}

/// Inert coordinates of one reached scope. Decode does not issue a grant;
/// restoration requires its original program and the enclosing custody bank.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimePresentationScopeSnapshot {
    owner: PresentationScopeOwner,
    anchor: PresentationScopeAnchor,
}

impl RuntimePresentationScopeBinding {
    pub(crate) const fn bank(&self) -> RuntimePresentationBankId {
        self.owner.bank
    }

    pub(crate) fn snapshot_admitted(
        &self,
        program: &RuntimeProgramOwner,
    ) -> Result<RuntimePresentationScopeSnapshot, RuntimePresentationError> {
        if !self.program.same_program(program) {
            return Err(RuntimePresentationError::Program);
        }
        Ok(RuntimePresentationScopeSnapshot {
            owner: self.owner,
            anchor: self.anchor,
        })
    }
}

impl RuntimePresentationScopeSnapshot {
    pub(crate) fn into_admitted_live(
        self,
        program: &RuntimeProgramOwner,
    ) -> RuntimePresentationScopeBinding {
        RuntimePresentationScopeBinding {
            program: program.clone(),
            owner: self.owner,
            anchor: self.anchor,
        }
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

    pub(crate) fn admit_retained(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError> {
        self.owner.admit_metadata(&self.anchor, admission)?;
        admission.validate_presentation_scope_snapshot(self)
    }
}

impl PresentationScopeStorage for RuntimePresentationScopeBinding {
    fn admit_awbc_scope(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
        fiber: crate::runtime_id::RuntimeFiberInstanceId,
        frame: crate::runtime_id::RuntimeFrameInstanceId,
        scope: Option<crate::awbc::schema::AwbcScopeId>,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError> {
        self.owner
            .validate_awbc_anchor(self.anchor, fiber, frame, scope)?;
        self.admit_retained(admission)
    }
}

impl PresentationScopeStorage for RuntimePresentationScopeSnapshot {
    fn admit_awbc_scope(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
        fiber: crate::runtime_id::RuntimeFiberInstanceId,
        frame: crate::runtime_id::RuntimeFrameInstanceId,
        scope: Option<crate::awbc::schema::AwbcScopeId>,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError> {
        self.owner
            .validate_awbc_anchor(self.anchor, fiber, frame, scope)?;
        self.admit_retained(admission)
    }
}

/// Exclusive access to the grant slot of one reached execution scope. Resolving
/// the real frame may fail before this capability exists; installation after
/// ledger admission is infallible and cannot expose an unregistered grant.
pub(crate) struct PresentationScopeTarget<'scope> {
    binding: &'scope mut Option<RuntimePresentationScopeBinding>,
    anchor: PresentationScopeAnchor,
}

impl<'scope> PresentationScopeTarget<'scope> {
    pub(crate) const fn native(
        binding: &'scope mut Option<RuntimePresentationScopeBinding>,
    ) -> Self {
        Self {
            binding,
            anchor: PresentationScopeAnchor::Native,
        }
    }

    pub(crate) const fn awbc(
        binding: &'scope mut Option<RuntimePresentationScopeBinding>,
        frame: crate::runtime_id::RuntimeFrameInstanceId,
        scope: Option<crate::awbc::schema::AwbcScopeId>,
    ) -> Self {
        Self {
            binding,
            anchor: PresentationScopeAnchor::Awbc { frame, scope },
        }
    }

    fn binding(&self) -> Option<&RuntimePresentationScopeBinding> {
        self.binding.as_ref()
    }

    fn install(&mut self, binding: Option<RuntimePresentationScopeBinding>) {
        *self.binding = binding;
    }
}

impl RuntimePresentationScopeBinding {
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
        self.owner.admit_metadata(&self.anchor, admission)?;
        admission.validate_presentation_scope(self)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PresentationSlotAddress {
    #[serde(with = "crate::value::runtime_public_id")]
    target: PublicId,
    #[serde(with = "crate::value::runtime_public_id")]
    slot: PublicId,
    resource: PresentationResourceKind,
}

impl PresentationSlotAddress {
    pub(crate) fn try_new(
        target: PublicId,
        slot: PublicId,
        resource: PresentationResourceKind,
    ) -> Result<Self, RuntimePresentationError> {
        let address = Self {
            target,
            slot,
            resource,
        };
        address.validate()?;
        Ok(address)
    }

    fn validate(&self) -> Result<(), RuntimePresentationError> {
        if !self.target.as_str().starts_with("target.") || !self.slot.as_str().starts_with("slot.")
        {
            return Err(RuntimePresentationError::InvalidAddress);
        }
        Ok(())
    }
}

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
            Self::Slot(address) => address.resource,
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
    pub(crate) fn validate_retirement(
        &self,
        token: &RuntimePresentationHandleToken,
    ) -> Result<(), RuntimePresentationError> {
        if token.artifact() != &self.state.artifact
            || token.bank() != self.bank
            || self.state.leases.contains_key(&token.issuance())
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
    pub(crate) fn bound_scope_body_count(
        &self,
        binding: Option<&RuntimePresentationScopeBinding>,
    ) -> usize {
        binding.map_or(0, |binding| {
            self.state
                .leases
                .values()
                .filter(|lease| lease.owner == Some(binding.owner) && lease.body.is_some())
                .count()
        })
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
        self.validate_program(program)?;
        if let Some(binding) = target.binding() {
            if !binding.program.same_program(program)
                || binding.owner.execution != execution
                || binding.owner.fiber != fiber
                || binding.anchor != target.anchor
            {
                return Err(RuntimePresentationError::Owner);
            }
            self.validate_scope(binding.owner)?;
            return Ok(binding.owner);
        }
        if self.state.scopes.len() >= self.limits.scopes {
            return Err(RuntimePresentationError::ScopeCapacity);
        }
        let mut next = self.state.next_scope;
        let scope = RuntimeScopeInstanceId::from_allocated(
            next.take_next(RuntimeIdNamespace::ScopeInstance)?,
        );
        let owner = PresentationScopeOwner {
            bank: self.bank,
            execution,
            fiber,
            scope,
        };
        target.install(Some(RuntimePresentationScopeBinding {
            program: self.program.clone(),
            owner,
            anchor: target.anchor,
        }));
        self.state
            .scopes
            .insert(scope, (execution, fiber, target.anchor));
        self.state.next_scope = next;
        Ok(owner)
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
        self.validate_program(program)?;
        let Some(binding) = target.binding() else {
            return Ok(Vec::new());
        };
        if !binding.program.same_program(program)
            || binding.owner.execution != execution
            || binding.owner.fiber != fiber
            || binding.anchor != target.anchor
        {
            return Err(RuntimePresentationError::Owner);
        }
        let owner = binding.owner;
        let retired = self.exit_scope(program, owner)?;
        target.install(None);
        Ok(retired)
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
    ) -> Result<(RuntimePresentationHandleToken, Option<V>), PresentationRegisterError<V>> {
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
        Ok(self.take_slot_body(address))
    }

    fn take_slot_body(&mut self, address: &PresentationSlotAddress) -> Option<V> {
        self.state
            .leases
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

impl RuntimePresentationScopeBinding {
    pub(crate) fn matches_context(
        &self,
        program: &RuntimeProgramOwner,
        execution: ExecutionInstanceId,
        fiber: RuntimePersistentFiberId,
    ) -> bool {
        self.program.same_program(program)
            && self.owner.execution == execution
            && self.owner.fiber == fiber
            && self.anchor == PresentationScopeAnchor::Native
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
