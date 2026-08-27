use super::{LineTaskLiveSnapshot, LineTaskScheduledCompletion, LineTaskWorkTag, ScopeExit};
use crate::effect::RuntimeDropPolicy;
use crate::pattern::RuntimeOpaqueTypeOwner;
use crate::presentation::{RuntimeCommandQueue, RuntimeStageRejectCode, RuntimeVoiceSessionId};
use crate::runtime_id::{
    DialogueActivationId, RuntimeLineHandleSiteId, RuntimeLineHandleToken, RuntimeLineTaskNodeId,
    RuntimeLocalDeclarationId,
};
use crate::time::LogicalDuration;
use crate::value::ownership::{RuntimeOwnedSlotId, RuntimeValuePath};
use crate::value::{
    RuntimeHandleKind, RuntimeLocalBinding, RuntimeOpaquePersistence, RuntimeOpaqueValue,
    RuntimeOpaqueValueClass, RuntimeValue,
};
use arcweft_character::id::CharacterId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

pub const MAX_LINE_HANDLE_SITES: usize = 128;
pub const MAX_LINE_LIVE_HANDLES: usize = 256;
pub const MAX_LINE_SCHEDULED_CALLBACKS: usize = 128;
pub const MAX_LINE_COMMAND_HISTORY: usize = 1_024;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeLineHandleScope {
    Line,
}

/// Closed producer role of one handle site. Cue's two runtime behaviors are
/// distinct here even though both yield the single source type `CueHandle`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeLineHandleSiteKind {
    StageActor,
    ScheduledCue,
    StageLookCue,
    Voice,
}

impl RuntimeLineHandleSiteKind {
    #[must_use]
    pub const fn handle_kind(self) -> RuntimeHandleKind {
        match self {
            Self::StageActor => RuntimeHandleKind::StageActor,
            Self::ScheduledCue | Self::StageLookCue => RuntimeHandleKind::Cue,
            Self::Voice => RuntimeHandleKind::Voice,
        }
    }
}

/// One dense typed handle-producing site in a sealed line task group.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeLineHandleSite {
    id: RuntimeLineHandleSiteId,
    source_ordinal: u32,
    kind: RuntimeLineHandleSiteKind,
    result_type: crate::runtime_id::RuntimePlanTypeId,
    character: Option<CharacterId>,
    scheduled_child: Option<RuntimeLineTaskNodeId>,
    opaque_owner: RuntimeOpaqueTypeOwner,
}

impl RuntimeLineHandleSite {
    pub(crate) fn new(
        id: RuntimeLineHandleSiteId,
        source_ordinal: u32,
        kind: RuntimeLineHandleSiteKind,
        result_type: crate::runtime_id::RuntimePlanTypeId,
        character: Option<CharacterId>,
        scheduled_child: Option<RuntimeLineTaskNodeId>,
        opaque_owner: RuntimeOpaqueTypeOwner,
    ) -> Result<Self, LineRuntimeError> {
        let handle_kind = kind.handle_kind();
        if opaque_owner.value_class() != RuntimeOpaqueValueClass::AffineHandle(handle_kind)
            || opaque_owner.persistence() != RuntimeOpaquePersistence::SnapshotOnly
            || opaque_owner.producer()
                != &handle_kind
                    .try_producer()
                    .map_err(|_| LineRuntimeError::WrongOpaqueProducer)?
        {
            return Err(LineRuntimeError::WrongOpaqueProducer);
        }
        let valid_shape = match kind {
            RuntimeLineHandleSiteKind::StageActor => {
                character.is_some() && scheduled_child.is_none()
            }
            RuntimeLineHandleSiteKind::ScheduledCue => {
                character.is_none() && scheduled_child.is_some()
            }
            RuntimeLineHandleSiteKind::StageLookCue => {
                character.is_some() && scheduled_child.is_none()
            }
            RuntimeLineHandleSiteKind::Voice => character.is_none() && scheduled_child.is_none(),
        };
        if !valid_shape {
            return Err(LineRuntimeError::InvalidHandleSite);
        }
        Ok(Self {
            id,
            source_ordinal,
            kind,
            result_type,
            character,
            scheduled_child,
            opaque_owner,
        })
    }

    #[must_use]
    pub const fn id(&self) -> RuntimeLineHandleSiteId {
        self.id
    }

    #[must_use]
    pub const fn source_ordinal(&self) -> u32 {
        self.source_ordinal
    }

    #[must_use]
    pub const fn site_kind(&self) -> RuntimeLineHandleSiteKind {
        self.kind
    }

    #[must_use]
    pub const fn kind(&self) -> RuntimeHandleKind {
        self.kind.handle_kind()
    }

    #[must_use]
    pub const fn result_type(&self) -> crate::runtime_id::RuntimePlanTypeId {
        self.result_type
    }

    #[must_use]
    pub const fn character(&self) -> Option<&CharacterId> {
        self.character.as_ref()
    }

    #[must_use]
    pub const fn scheduled_child(&self) -> Option<RuntimeLineTaskNodeId> {
        self.scheduled_child
    }

    #[must_use]
    pub const fn opaque_owner(&self) -> &RuntimeOpaqueTypeOwner {
        &self.opaque_owner
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeHandleOwnerSlot {
    LineScope,
    ActivationLocal(RuntimeOwnedSlotId),
    ChildScope(LineTaskWorkTag),
    DialogueResult(RuntimeValuePath),
    ParentFiber(RuntimeOwnedSlotId),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeHandleLeaseState {
    Allocating,
    Active,
    Pending,
    Running,
    Completed,
    Cancelling,
    Cancelled,
    Failed,
    Released,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeStageActorLease {
    character: CharacterId,
}

impl RuntimeStageActorLease {
    #[must_use]
    pub const fn new(character: CharacterId) -> Self {
        Self { character }
    }

    #[must_use]
    pub const fn character(&self) -> &CharacterId {
        &self.character
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeCueOrigin {
    Scheduled {
        child: RuntimeLineTaskNodeId,
        deadline: LogicalDuration,
    },
    StageLook,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeCueLease {
    origin: RuntimeCueOrigin,
}

impl RuntimeCueLease {
    #[must_use]
    pub const fn new(origin: RuntimeCueOrigin) -> Self {
        Self { origin }
    }

    #[must_use]
    pub const fn origin(&self) -> &RuntimeCueOrigin {
        &self.origin
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeVoiceLease {
    session: RuntimeVoiceSessionId,
    lease_ordinal: u32,
    stop_on_last_release: bool,
}

impl RuntimeVoiceLease {
    #[must_use]
    pub const fn new(
        session: RuntimeVoiceSessionId,
        lease_ordinal: u32,
        stop_on_last_release: bool,
    ) -> Self {
        Self {
            session,
            lease_ordinal,
            stop_on_last_release,
        }
    }

    #[must_use]
    pub const fn session(&self) -> &RuntimeVoiceSessionId {
        &self.session
    }

    #[must_use]
    pub const fn lease_ordinal(&self) -> u32 {
        self.lease_ordinal
    }

    #[must_use]
    pub const fn stop_on_last_release(&self) -> bool {
        self.stop_on_last_release
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeHandleResource {
    StageActor(RuntimeStageActorLease),
    Cue(RuntimeCueLease),
    Voice(RuntimeVoiceLease),
}

impl RuntimeHandleResource {
    #[must_use]
    pub const fn kind(&self) -> RuntimeHandleKind {
        match self {
            Self::StageActor(_) => RuntimeHandleKind::StageActor,
            Self::Cue(_) => RuntimeHandleKind::Cue,
            Self::Voice(_) => RuntimeHandleKind::Voice,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeHandleLease {
    token: RuntimeLineHandleToken,
    owner: RuntimeHandleOwnerSlot,
    state: RuntimeHandleLeaseState,
    resource: RuntimeHandleResource,
}

impl RuntimeHandleLease {
    #[must_use]
    pub const fn token(&self) -> &RuntimeLineHandleToken {
        &self.token
    }

    #[must_use]
    pub const fn owner(&self) -> &RuntimeHandleOwnerSlot {
        &self.owner
    }

    #[must_use]
    pub const fn state(&self) -> RuntimeHandleLeaseState {
        self.state
    }

    #[must_use]
    pub const fn resource(&self) -> &RuntimeHandleResource {
        &self.resource
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeLineHandleLedger {
    issuance_by_site: BTreeMap<RuntimeLineHandleSiteId, u32>,
    leases: BTreeMap<RuntimeLineHandleToken, RuntimeHandleLease>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeDialogueActivationState<T> {
    ledger: RuntimeLineHandleLedger,
    command_sequence: u64,
    issued_commands: BTreeMap<
        crate::presentation::RuntimeLineCommandId,
        crate::presentation::RuntimeLineHostCommand,
    >,
    superseded_commands: BTreeMap<
        crate::presentation::RuntimeLineCommandId,
        crate::presentation::RuntimeLineHostCommand,
    >,
    resolved_commands: std::collections::BTreeSet<crate::presentation::RuntimeLineCommandId>,
    scheduled: Vec<RuntimeScheduledLineTask>,
    result: RuntimeDialogueResultState<T>,
    frame_released: bool,
    prepared_commands: Vec<crate::presentation::RuntimeLineHostCommand>,
}

/// Post-publication affine authority retained only while parent-owned handles
/// remain live. Executable dialogue frame, reducer, schedule, and result state
/// are deliberately absent.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RuntimePublishedDialogueHandles {
    ledger: RuntimeLineHandleLedger,
    command_sequence: u64,
    issued_commands: BTreeMap<
        crate::presentation::RuntimeLineCommandId,
        crate::presentation::RuntimeLineHostCommand,
    >,
    resolved_commands: std::collections::BTreeSet<crate::presentation::RuntimeLineCommandId>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AwbcRuntimeDialogueActivationSnapshot<T> {
    ledger: RuntimeLineHandleLedger,
    command_sequence: u64,
    issued_commands: BTreeMap<
        crate::presentation::RuntimeLineCommandId,
        crate::presentation::RuntimeLineHostCommand,
    >,
    superseded_commands: BTreeMap<
        crate::presentation::RuntimeLineCommandId,
        crate::presentation::RuntimeLineHostCommand,
    >,
    resolved_commands: std::collections::BTreeSet<crate::presentation::RuntimeLineCommandId>,
    scheduled: Vec<AwbcRuntimeScheduledLineTaskSnapshot>,
    result: AwbcRuntimeDialogueResultSnapshot<T>,
    frame_released: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AwbcRuntimePublishedDialogueHandlesSnapshot {
    ledger: RuntimeLineHandleLedger,
    command_sequence: u64,
    issued_commands: BTreeMap<
        crate::presentation::RuntimeLineCommandId,
        crate::presentation::RuntimeLineHostCommand,
    >,
    resolved_commands: std::collections::BTreeSet<crate::presentation::RuntimeLineCommandId>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct AwbcRuntimeScheduledLineTaskSnapshot {
    token: RuntimeLineHandleToken,
    child: RuntimeLineTaskNodeId,
    work: LineTaskWorkTag,
    deadline: LogicalDuration,
    custody: AwbcRuntimeScheduledCaptureCustodySnapshot,
    state: RuntimeScheduledState,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct AwbcRuntimeLocalBindingSnapshot {
    local: crate::runtime_id::RuntimeLocalDeclarationId,
    value: crate::value::AwbcRuntimeValueSnapshot,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
enum AwbcRuntimeScheduledCaptureCustodySnapshot {
    Packet(Vec<AwbcRuntimeLocalBindingSnapshot>),
    ChildFiber(Vec<RuntimeLocalDeclarationId>),
    LineScope(Vec<AwbcRuntimeLocalBindingSnapshot>),
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
enum AwbcRuntimeDialogueResultSnapshot<T> {
    Uncommitted,
    Committed {
        ty: T,
        value: crate::value::AwbcRuntimeValueSnapshot,
    },
    Publishing {
        ty: T,
        value: crate::value::AwbcRuntimeValueSnapshot,
    },
    Published,
    Abandoned,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct RuntimeHandleDropReceipt {
    commands: Vec<crate::presentation::RuntimeLineHostCommand>,
}

impl RuntimeHandleDropReceipt {
    pub(crate) fn from_commands(
        commands: Vec<crate::presentation::RuntimeLineHostCommand>,
    ) -> Self {
        Self { commands }
    }

    pub(crate) fn into_commands(self) -> Vec<crate::presentation::RuntimeLineHostCommand> {
        self.commands
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct RuntimeDialogueCommitReceipt {
    commands: Vec<crate::presentation::RuntimeLineHostCommand>,
}

impl RuntimeDialogueCommitReceipt {
    pub(crate) fn into_commands(self) -> Vec<crate::presentation::RuntimeLineHostCommand> {
        self.commands
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeDialogueResultState<T> {
    Uncommitted,
    Committed { ty: T, value: RuntimeValue },
    Publishing { ty: T, value: RuntimeValue },
    Published,
    Abandoned,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimeDialogueTerminalKind {
    Published,
    Abandoned,
}

impl<T: Clone> RuntimeDialogueActivationState<T> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            ledger: RuntimeLineHandleLedger::default(),
            command_sequence: 0,
            issued_commands: BTreeMap::new(),
            superseded_commands: BTreeMap::new(),
            resolved_commands: std::collections::BTreeSet::new(),
            scheduled: Vec::new(),
            result: RuntimeDialogueResultState::Uncommitted,
            frame_released: false,
            prepared_commands: Vec::new(),
        }
    }

    pub(crate) fn restore_admit(
        &self,
        activation: &DialogueActivationId,
    ) -> Result<(), LineRuntimeError> {
        validate_restored_ledger(activation, &self.ledger)?;
        validate_restored_command_journal(
            activation,
            self.command_sequence,
            &self.issued_commands,
            &self.superseded_commands,
            &self.resolved_commands,
            &self.ledger,
        )?;
        let mut scheduled_tokens = std::collections::BTreeSet::new();
        let mut scheduled_capture_tokens = std::collections::BTreeSet::new();
        for scheduled in &self.scheduled {
            scheduled.validate_custody()?;
            if scheduled.token().activation() != activation
                || scheduled.work().activation_id() != activation
                || scheduled.work().scheduled_token() != Some(scheduled.token())
                || !scheduled_tokens.insert(scheduled.token().clone())
            {
                return Err(LineRuntimeError::InvalidRestoredScheduledState);
            }
            let lease = self
                .ledger
                .lease(scheduled.token())
                .ok_or(LineRuntimeError::InvalidRestoredScheduledState)?;
            let RuntimeHandleResource::Cue(cue) = lease.resource() else {
                return Err(LineRuntimeError::InvalidRestoredScheduledState);
            };
            if !matches!(
                cue.origin(),
                RuntimeCueOrigin::Scheduled { child, deadline }
                    if *child == scheduled.child() && *deadline == scheduled.deadline()
            ) {
                return Err(LineRuntimeError::InvalidRestoredScheduledState);
            }
            let lease_state_matches = match scheduled.state() {
                RuntimeScheduledState::Armed => lease.state() == RuntimeHandleLeaseState::Pending,
                RuntimeScheduledState::Running => lease.state() == RuntimeHandleLeaseState::Running,
                RuntimeScheduledState::Cancelling => matches!(
                    lease.state(),
                    RuntimeHandleLeaseState::Cancelling | RuntimeHandleLeaseState::Cancelled
                ),
                RuntimeScheduledState::Completed => matches!(
                    lease.state(),
                    RuntimeHandleLeaseState::Completed | RuntimeHandleLeaseState::Released
                ),
                RuntimeScheduledState::Cancelled => matches!(
                    lease.state(),
                    RuntimeHandleLeaseState::Cancelled | RuntimeHandleLeaseState::Released
                ),
                RuntimeScheduledState::Failed => matches!(
                    lease.state(),
                    RuntimeHandleLeaseState::Failed | RuntimeHandleLeaseState::Released
                ),
            };
            if !lease_state_matches {
                return Err(LineRuntimeError::InvalidRestoredScheduledState);
            }
            let expected_capture_owner = match &scheduled.custody {
                RuntimeScheduledCaptureCustody::Packet(_) => {
                    Some(RuntimeHandleOwnerSlot::ChildScope(scheduled.work().clone()))
                }
                RuntimeScheduledCaptureCustody::LineScope(_) => {
                    Some(RuntimeHandleOwnerSlot::LineScope)
                }
                RuntimeScheduledCaptureCustody::ChildFiber(_) => None,
            };
            if let Some(expected_owner) = expected_capture_owner {
                let captures = match &scheduled.custody {
                    RuntimeScheduledCaptureCustody::Packet(captures)
                    | RuntimeScheduledCaptureCustody::LineScope(captures) => captures,
                    RuntimeScheduledCaptureCustody::ChildFiber(_) => {
                        return Err(LineRuntimeError::InvalidRestoredScheduledState);
                    }
                };
                let mut expected_tokens = std::collections::BTreeSet::new();
                for capture in captures.iter() {
                    for handle in capture
                        .value
                        .affine_line_handles()
                        .map_err(|_| LineRuntimeError::InvalidRestoredScheduledState)?
                    {
                        if handle.token().activation() != activation
                            || !expected_tokens.insert(handle.token().clone())
                            || !scheduled_capture_tokens.insert(handle.token().clone())
                        {
                            return Err(LineRuntimeError::InvalidRestoredScheduledState);
                        }
                        let capture_lease = self
                            .ledger
                            .lease(handle.token())
                            .ok_or(LineRuntimeError::InvalidRestoredScheduledState)?;
                        if capture_lease.owner() != &expected_owner
                            || capture_lease.state() == RuntimeHandleLeaseState::Released
                        {
                            return Err(LineRuntimeError::InvalidRestoredScheduledState);
                        }
                    }
                }
                let child_owned = self
                    .ledger
                    .leases()
                    .values()
                    .filter_map(|lease| {
                        (lease.owner()
                            == &RuntimeHandleOwnerSlot::ChildScope(scheduled.work().clone()))
                            .then(|| lease.token().clone())
                    })
                    .collect::<std::collections::BTreeSet<_>>();
                if matches!(
                    &scheduled.custody,
                    RuntimeScheduledCaptureCustody::Packet(_)
                ) && child_owned != expected_tokens
                    || matches!(
                        &scheduled.custody,
                        RuntimeScheduledCaptureCustody::LineScope(_)
                    ) && !child_owned.is_empty()
                {
                    return Err(LineRuntimeError::InvalidRestoredScheduledState);
                }
            }
        }
        for lease in self.ledger.leases().values() {
            if matches!(
                lease.resource(),
                RuntimeHandleResource::Cue(RuntimeCueLease {
                    origin: RuntimeCueOrigin::Scheduled { .. },
                })
            ) && !scheduled_tokens.contains(lease.token())
            {
                return Err(LineRuntimeError::InvalidRestoredScheduledState);
            }
            if let RuntimeHandleOwnerSlot::ChildScope(work) = lease.owner()
                && let Some(token) = work.scheduled_token()
                && !self
                    .scheduled
                    .iter()
                    .any(|scheduled| scheduled.token() == token && scheduled.work() == work)
            {
                return Err(LineRuntimeError::InvalidRestoredScheduledState);
            }
        }
        let mut result_tokens = std::collections::BTreeSet::new();
        match &self.result {
            RuntimeDialogueResultState::Uncommitted => {
                if self.ledger.leases().values().any(|lease| {
                    matches!(
                        lease.owner(),
                        RuntimeHandleOwnerSlot::DialogueResult(_)
                            | RuntimeHandleOwnerSlot::ParentFiber(_)
                    )
                }) {
                    return Err(LineRuntimeError::InvalidRestoredResultState);
                }
            }
            RuntimeDialogueResultState::Committed { value, .. }
            | RuntimeDialogueResultState::Publishing { value, .. } => {
                for handle in value
                    .affine_line_handles()
                    .map_err(|_| LineRuntimeError::InvalidRestoredResultState)?
                {
                    if handle.token().activation() != activation {
                        return Err(LineRuntimeError::InvalidRestoredResultState);
                    }
                    if !result_tokens.insert(handle.token().clone()) {
                        return Err(LineRuntimeError::InvalidRestoredResultState);
                    }
                    let lease = self
                        .ledger
                        .lease(handle.token())
                        .ok_or(LineRuntimeError::InvalidRestoredResultState)?;
                    if lease.resource().kind() != handle.kind()
                        || lease.owner()
                            != &RuntimeHandleOwnerSlot::DialogueResult(handle.path().clone())
                    {
                        return Err(LineRuntimeError::InvalidRestoredResultState);
                    }
                }
                if matches!(&self.result, RuntimeDialogueResultState::Committed { .. })
                    && self.ledger.leases().values().any(|lease| {
                        matches!(lease.owner(), RuntimeHandleOwnerSlot::ParentFiber(_))
                    })
                {
                    return Err(LineRuntimeError::InvalidRestoredResultState);
                }
            }
            RuntimeDialogueResultState::Abandoned => {
                if self.ledger.leases().values().any(|lease| {
                    matches!(
                        lease.owner(),
                        RuntimeHandleOwnerSlot::DialogueResult(_)
                            | RuntimeHandleOwnerSlot::ParentFiber(_)
                    )
                }) {
                    return Err(LineRuntimeError::InvalidRestoredResultState);
                }
            }
            RuntimeDialogueResultState::Published => {
                return Err(LineRuntimeError::InvalidRestoredResultState);
            }
        }
        let ledger_result_tokens = self
            .ledger
            .leases()
            .values()
            .filter_map(|lease| {
                matches!(lease.owner(), RuntimeHandleOwnerSlot::DialogueResult(_))
                    .then(|| lease.token().clone())
            })
            .collect::<std::collections::BTreeSet<_>>();
        if ledger_result_tokens != result_tokens {
            return Err(LineRuntimeError::InvalidRestoredResultState);
        }
        if self.frame_released || !self.prepared_commands.is_empty() {
            return Err(LineRuntimeError::InvalidRestoredResultState);
        }
        Ok(())
    }

    pub(crate) const fn ledger(&self) -> &RuntimeLineHandleLedger {
        &self.ledger
    }

    pub(crate) fn restore_admit_reducer(
        &self,
        activation: &DialogueActivationId,
        reducer: &LineTaskLiveSnapshot,
    ) -> Result<(), LineRuntimeError> {
        if reducer.activation() != activation {
            return Err(LineRuntimeError::InvalidRestoredScheduledState);
        }
        let ready = reducer
            .scheduled_ready()
            .iter()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>();
        let lanes = reducer
            .scheduled_lanes()
            .iter()
            .map(|lane| lane.token().clone())
            .collect::<std::collections::BTreeSet<_>>();
        if ready.len() != reducer.scheduled_ready().len()
            || lanes.len() != reducer.scheduled_lanes().len()
            || ready.iter().any(|token| lanes.contains(token))
        {
            return Err(LineRuntimeError::InvalidRestoredScheduledState);
        }
        for scheduled in &self.scheduled {
            if scheduled.token().activation() != reducer.activation()
                || scheduled.work().scheduled_token() != Some(scheduled.token())
            {
                return Err(LineRuntimeError::InvalidRestoredScheduledState);
            }
            let (expects_ready, expects_lane) = match (&scheduled.state, &scheduled.custody) {
                (RuntimeScheduledState::Armed, RuntimeScheduledCaptureCustody::Packet(_)) => {
                    (false, false)
                }
                (RuntimeScheduledState::Running, RuntimeScheduledCaptureCustody::Packet(_)) => {
                    (true, false)
                }
                (
                    RuntimeScheduledState::Running | RuntimeScheduledState::Cancelling,
                    RuntimeScheduledCaptureCustody::ChildFiber(_),
                ) => (false, true),
                (
                    RuntimeScheduledState::Completed
                    | RuntimeScheduledState::Cancelled
                    | RuntimeScheduledState::Failed,
                    RuntimeScheduledCaptureCustody::LineScope(_),
                ) => (false, false),
                _ => return Err(LineRuntimeError::InvalidRestoredScheduledState),
            };
            if ready.contains(scheduled.token()) != expects_ready
                || lanes.contains(scheduled.token()) != expects_lane
            {
                return Err(LineRuntimeError::InvalidRestoredScheduledState);
            }
        }
        if ready.iter().chain(lanes.iter()).any(|token| {
            !self
                .scheduled
                .iter()
                .any(|scheduled| scheduled.token() == token)
        }) {
            return Err(LineRuntimeError::InvalidRestoredScheduledState);
        }
        Ok(())
    }

    pub(crate) const fn command_sequence(&self) -> u64 {
        self.command_sequence
    }

    #[must_use]
    pub(crate) fn has_pending_commands(&self) -> bool {
        !self.issued_commands.is_empty() || !self.superseded_commands.is_empty()
    }

    pub(crate) fn scheduled(&self) -> &[RuntimeScheduledLineTask] {
        &self.scheduled
    }

    pub(crate) fn take_scheduled_capture_packet(
        &mut self,
        token: &RuntimeLineHandleToken,
    ) -> Result<Box<[RuntimeLocalBinding]>, LineRuntimeError> {
        let packet = exact_scheduled_packet_mut(&mut self.scheduled, token)?;
        packet.take_packet_for_child_fiber()
    }

    pub(crate) fn admit_scheduled_child_bindings(
        &mut self,
        token: &RuntimeLineHandleToken,
        bindings: Box<[RuntimeLocalBinding]>,
        terminal: RuntimeScheduledState,
    ) -> Result<(), LineRuntimeError> {
        let packet = exact_scheduled_packet_mut(&mut self.scheduled, token)?;
        packet.admit_child_fiber_bindings(bindings, terminal)
    }

    pub(crate) fn scheduled_child_locals(
        &self,
        token: &RuntimeLineHandleToken,
    ) -> Result<Box<[RuntimeLocalDeclarationId]>, LineRuntimeError> {
        let packet = self
            .scheduled
            .iter()
            .find(|scheduled| scheduled.token() == token)
            .ok_or(LineRuntimeError::MissingScheduledWork)?;
        Ok(packet.child_fiber_locals()?.to_vec().into_boxed_slice())
    }

    pub(crate) fn scheduled_child_custody_keys(
        &self,
    ) -> Vec<(LineTaskWorkTag, RuntimeLineHandleToken)> {
        self.scheduled
            .iter()
            .filter_map(|scheduled| {
                matches!(
                    &scheduled.custody,
                    RuntimeScheduledCaptureCustody::ChildFiber(_)
                )
                .then(|| (scheduled.work().clone(), scheduled.token().clone()))
            })
            .collect()
    }

    pub(crate) fn terminalize_unstarted_scheduled_packet(
        &mut self,
        token: &RuntimeLineHandleToken,
        terminal: RuntimeScheduledState,
    ) -> Result<(), LineRuntimeError> {
        let packet = exact_scheduled_packet_mut(&mut self.scheduled, token)?;
        packet.move_packet_to_line_scope(terminal)
    }

    pub(crate) fn complete_unstarted_scheduled(
        &mut self,
        completion: &LineTaskScheduledCompletion,
    ) -> Result<(), LineRuntimeError> {
        let terminal = match completion.exit() {
            ScopeExit::Completed => RuntimeScheduledState::Completed,
            ScopeExit::Cancelled => RuntimeScheduledState::Cancelled,
            ScopeExit::Failed => RuntimeScheduledState::Failed,
        };
        self.terminalize_unstarted_scheduled_packet(completion.token(), terminal)?;
        self.complete_scheduled_work(
            completion.token(),
            completion.exit() == ScopeExit::Failed,
            completion.exit() == ScopeExit::Cancelled,
        )
    }

    pub(crate) const fn result(&self) -> &RuntimeDialogueResultState<T> {
        &self.result
    }

    pub(crate) fn take_commit_receipt(&mut self) -> RuntimeDialogueCommitReceipt {
        RuntimeDialogueCommitReceipt {
            commands: std::mem::take(&mut self.prepared_commands),
        }
    }

    pub(crate) fn replace_transaction_parts(
        &mut self,
        ledger: RuntimeLineHandleLedger,
        scheduled: Vec<RuntimeScheduledLineTask>,
    ) {
        self.ledger = ledger;
        self.scheduled = scheduled;
    }

    pub(crate) fn commit_ledger(&mut self, ledger: RuntimeLineHandleLedger) {
        self.ledger = ledger;
    }

    pub(crate) fn schedule(
        &mut self,
        scheduled: RuntimeScheduledLineTask,
    ) -> Result<(), LineRuntimeError> {
        if self.scheduled.len() >= MAX_LINE_SCHEDULED_CALLBACKS {
            return Err(LineRuntimeError::ScheduledCallbackLimitExceeded);
        }
        if self
            .scheduled
            .iter()
            .any(|existing| existing.token() == scheduled.token())
        {
            return Err(LineRuntimeError::DuplicateHandleToken);
        }
        self.scheduled.push(scheduled);
        Ok(())
    }

    pub(crate) fn arm_due_schedules(
        &mut self,
        elapsed: LogicalDuration,
    ) -> Result<Vec<RuntimeLineHandleToken>, LineRuntimeError> {
        let mut ledger = self.ledger.clone();
        let mut scheduled = self.scheduled.clone();
        let mut due = Vec::new();
        for callback in &mut scheduled {
            if callback.state() == RuntimeScheduledState::Armed && callback.deadline() <= elapsed {
                callback
                    .transition(RuntimeScheduledState::Armed, RuntimeScheduledState::Running)?;
                ledger.set_state(
                    callback.token(),
                    RuntimeHandleLeaseState::Pending,
                    RuntimeHandleLeaseState::Running,
                )?;
                due.push((
                    callback.deadline(),
                    callback.token().clone(),
                    callback.child(),
                ));
            }
        }
        self.ledger = ledger;
        self.scheduled = scheduled;
        due.sort_by(|left, right| left.cmp(right));
        Ok(due.into_iter().map(|(_, token, _)| token).collect())
    }

    pub(crate) fn complete_scheduled_work(
        &mut self,
        token: &RuntimeLineHandleToken,
        failed: bool,
        cancelled: bool,
    ) -> Result<(), LineRuntimeError> {
        let index = self
            .scheduled
            .iter()
            .position(|scheduled| scheduled.token() == token)
            .ok_or(LineRuntimeError::MissingScheduledWork)?;
        let mut ledger = self.ledger.clone();
        let mut scheduled = self.scheduled.clone();
        let packet = &mut scheduled[index];
        let (expected, terminal) = match packet.state() {
            RuntimeScheduledState::Running => (
                RuntimeScheduledState::Running,
                if failed {
                    RuntimeScheduledState::Failed
                } else if cancelled {
                    RuntimeScheduledState::Cancelled
                } else {
                    RuntimeScheduledState::Completed
                },
            ),
            RuntimeScheduledState::Cancelling => (
                RuntimeScheduledState::Cancelling,
                if failed {
                    RuntimeScheduledState::Failed
                } else {
                    RuntimeScheduledState::Cancelled
                },
            ),
            RuntimeScheduledState::Completed => (
                RuntimeScheduledState::Completed,
                RuntimeScheduledState::Completed,
            ),
            RuntimeScheduledState::Cancelled => (
                RuntimeScheduledState::Cancelled,
                RuntimeScheduledState::Cancelled,
            ),
            RuntimeScheduledState::Failed => {
                (RuntimeScheduledState::Failed, RuntimeScheduledState::Failed)
            }
            _ => return Err(LineRuntimeError::InvalidScheduledWorkState),
        };
        packet.require_line_scope()?;
        if expected != terminal {
            packet.transition(expected, terminal)?;
        }
        let lease = ledger
            .lease(packet.token())
            .ok_or(LineRuntimeError::UnknownHandle)?;
        let lease_state = lease.state();
        let terminal_lease = match terminal {
            RuntimeScheduledState::Completed => RuntimeHandleLeaseState::Completed,
            RuntimeScheduledState::Cancelled => RuntimeHandleLeaseState::Cancelled,
            RuntimeScheduledState::Failed => RuntimeHandleLeaseState::Failed,
            RuntimeScheduledState::Armed
            | RuntimeScheduledState::Running
            | RuntimeScheduledState::Cancelling => {
                return Err(LineRuntimeError::InvalidScheduledWorkState);
            }
        };
        ledger.set_state(packet.token(), lease_state, terminal_lease)?;
        let expected_owner = RuntimeHandleOwnerSlot::ChildScope(packet.work().clone());
        let mut tokens = std::collections::BTreeSet::new();
        for capture in packet.line_scope_captures()? {
            for handle in capture
                .value
                .affine_line_handles()
                .map_err(|_| LineRuntimeError::InvalidScheduledCaptureGraph)?
            {
                if !tokens.insert(handle.token().clone()) {
                    return Err(LineRuntimeError::DuplicateHandleOccurrence);
                }
                let lease = ledger
                    .lease(handle.token())
                    .ok_or(LineRuntimeError::UnknownHandle)?;
                if lease.state() == RuntimeHandleLeaseState::Released {
                    continue;
                }
                match lease.owner() {
                    owner if owner == &expected_owner => ledger.transfer(
                        handle.token(),
                        &expected_owner,
                        RuntimeHandleOwnerSlot::LineScope,
                    )?,
                    RuntimeHandleOwnerSlot::LineScope => {}
                    _ => return Err(LineRuntimeError::WrongOwner),
                }
            }
        }
        self.ledger = ledger;
        self.scheduled = scheduled;
        Ok(())
    }

    pub(crate) fn record_commands(
        &mut self,
        activation: &DialogueActivationId,
        queue: crate::presentation::RuntimeCommandQueue,
    ) -> Result<(), LineRuntimeError> {
        if queue.activation() != activation || queue.start_sequence() != self.command_sequence {
            return Err(LineRuntimeError::StaleCommandQueue);
        }
        let next_sequence = queue.next_sequence();
        let commands = queue.into_commands();
        if self
            .issued_commands
            .len()
            .checked_add(self.superseded_commands.len())
            .and_then(|count| count.checked_add(self.resolved_commands.len()))
            .and_then(|count| count.checked_add(commands.len()))
            .is_none_or(|count| count > MAX_LINE_COMMAND_HISTORY)
        {
            return Err(LineRuntimeError::CommandHistoryLimitExceeded);
        }
        let mut issued = self.issued_commands.clone();
        let mut superseded = self.superseded_commands.clone();
        for command in &commands {
            if let crate::presentation::RuntimeLineHostCommand::Stage(
                crate::presentation::RuntimeStageCommand::CancelCue { cue, .. },
            ) = command
            {
                let retired = issued
                    .iter()
                    .filter_map(|(id, prior)| {
                        matches!(
                            prior,
                            crate::presentation::RuntimeLineHostCommand::Stage(
                                crate::presentation::RuntimeStageCommand::SetCharacterLook {
                                    cue: prior_cue,
                                    ..
                                }
                            ) if prior_cue == cue
                        )
                        .then(|| id.clone())
                    })
                    .collect::<Vec<_>>();
                for id in retired {
                    if let Some(command) = issued.remove(&id) {
                        superseded.insert(id, command);
                    }
                }
            }
            if issued
                .insert(command.command().clone(), command.clone())
                .is_some()
                || superseded.contains_key(command.command())
                || self.resolved_commands.contains(command.command())
            {
                return Err(LineRuntimeError::DuplicateCommandIdentity);
            }
        }
        self.command_sequence = next_sequence;
        self.issued_commands = issued;
        self.superseded_commands = superseded;
        self.prepared_commands.extend(commands);
        Ok(())
    }

    /// Reconciles one running child instruction against the coarse but exact
    /// `ChildScope(tag)` ledger owner. Register-local moves are executor
    /// details; disappearance is a typed drop and newly appearing tokens are
    /// rejected.
    pub(crate) fn reconcile_child_scope_step(
        &mut self,
        tag: &LineTaskWorkTag,
        before: &std::collections::BTreeSet<RuntimeLineHandleToken>,
        after: &std::collections::BTreeSet<RuntimeLineHandleToken>,
        drop_policy: Option<RuntimeDropPolicy>,
    ) -> Result<(), LineRuntimeError> {
        if after.iter().any(|token| !before.contains(token)) {
            return Err(LineRuntimeError::UnexpectedChildHandleOccurrence);
        }
        let mut candidate = self.clone();
        let owner = RuntimeHandleOwnerSlot::ChildScope(tag.clone());
        let mut ledger = candidate.ledger.clone();
        let mut queue =
            RuntimeCommandQueue::new(tag.activation_id().clone(), candidate.command_sequence);
        for token in before {
            if token.activation() != tag.activation_id() {
                return Err(LineRuntimeError::WrongActivation);
            }
            let lease = ledger.lease(token).ok_or(LineRuntimeError::UnknownHandle)?;
            if lease.owner() != &owner {
                return Err(LineRuntimeError::WrongOwner);
            }
            if !after.contains(token) {
                let policy = drop_policy.ok_or(LineRuntimeError::UnjournaledHandleDrop)?;
                ledger.drop_owned_with_policy(token, &owner, policy, &mut queue)?;
            }
        }
        candidate.ledger = ledger;
        candidate.record_commands(tag.activation_id(), queue)?;
        *self = candidate;
        Ok(())
    }

    /// Closes child custody by moving only declared surviving capture tokens
    /// back to LineScope and dropping every other live token before the child
    /// carrier disappears.
    pub(crate) fn finish_child_scope(
        &mut self,
        tag: &LineTaskWorkTag,
        live: &std::collections::BTreeSet<RuntimeLineHandleToken>,
        returned: &std::collections::BTreeSet<RuntimeLineHandleToken>,
        policy: RuntimeDropPolicy,
    ) -> Result<(), LineRuntimeError> {
        if returned.iter().any(|token| !live.contains(token)) {
            return Err(LineRuntimeError::UnexpectedChildHandleOccurrence);
        }
        let mut candidate = self.clone();
        let owner = RuntimeHandleOwnerSlot::ChildScope(tag.clone());
        let mut ledger = candidate.ledger.clone();
        let mut queue =
            RuntimeCommandQueue::new(tag.activation_id().clone(), candidate.command_sequence);
        for token in live {
            if token.activation() != tag.activation_id() {
                return Err(LineRuntimeError::WrongActivation);
            }
            let lease = ledger.lease(token).ok_or(LineRuntimeError::UnknownHandle)?;
            if lease.owner() != &owner {
                return Err(LineRuntimeError::WrongOwner);
            }
            if returned.contains(token) {
                ledger.transfer(token, &owner, RuntimeHandleOwnerSlot::LineScope)?;
            } else {
                ledger.drop_owned_with_policy(token, &owner, policy, &mut queue)?;
            }
        }
        candidate.ledger = ledger;
        candidate.record_commands(tag.activation_id(), queue)?;
        *self = candidate;
        Ok(())
    }

    pub(crate) fn issued_command(
        &self,
        command: &crate::presentation::RuntimeLineCommandId,
    ) -> Option<&crate::presentation::RuntimeLineHostCommand> {
        self.issued_commands.get(command)
    }

    pub(crate) fn consume_issued_command(
        &mut self,
        command: &crate::presentation::RuntimeLineCommandId,
    ) -> Result<crate::presentation::RuntimeLineHostCommand, LineRuntimeError> {
        let command = self
            .issued_commands
            .remove(command)
            .ok_or(LineRuntimeError::UnknownCommandOutcome)?;
        self.resolved_commands.insert(command.command().clone());
        Ok(command)
    }

    pub(crate) fn superseded_command(
        &self,
        command: &crate::presentation::RuntimeLineCommandId,
    ) -> Option<&crate::presentation::RuntimeLineHostCommand> {
        self.superseded_commands.get(command)
    }

    pub(crate) fn resolve_superseded(
        &mut self,
        command: &crate::presentation::RuntimeLineCommandId,
    ) -> Option<crate::presentation::RuntimeLineHostCommand> {
        let resolved = self.superseded_commands.remove(command)?;
        self.resolved_commands.insert(command.clone());
        Some(resolved)
    }

    pub(crate) fn resolve_superseded_cue(&mut self, cue: &RuntimeLineHandleToken) {
        let commands = self
            .superseded_commands
            .iter()
            .filter_map(|(id, command)| {
                matches!(
                    command,
                    crate::presentation::RuntimeLineHostCommand::Stage(
                        crate::presentation::RuntimeStageCommand::SetCharacterLook {
                            cue: prior,
                            ..
                        }
                    ) if prior == cue
                )
                .then(|| id.clone())
            })
            .collect::<Vec<_>>();
        for command in commands {
            let _ = self.resolve_superseded(&command);
        }
    }

    pub(crate) fn resolve_issued_cancel_for_cue(&mut self, cue: &RuntimeLineHandleToken) {
        let commands = self
            .issued_commands
            .iter()
            .filter_map(|(id, command)| {
                matches!(
                    command,
                    crate::presentation::RuntimeLineHostCommand::Stage(
                        crate::presentation::RuntimeStageCommand::CancelCue {
                            cue: pending,
                            ..
                        }
                    ) if pending == cue
                )
                .then(|| id.clone())
            })
            .collect::<Vec<_>>();
        for command in commands {
            if self.issued_commands.remove(&command).is_some() {
                self.resolved_commands.insert(command);
            }
        }
    }

    pub(crate) fn is_resolved(&self, command: &crate::presentation::RuntimeLineCommandId) -> bool {
        self.resolved_commands.contains(command)
    }

    /// Reduces one non-activation host outcome against the sole command and
    /// lease journal. Pending Acquire/SetLook acceptance/voice-start tickets
    /// remain the executor frame's responsibility; completion, cancellation,
    /// release, cleanup rejection, and superseded-look races are shared here.
    pub(crate) fn accept_runtime_outcome(
        &mut self,
        outcome: &crate::presentation::RuntimeLineHostOutcome,
    ) -> Result<Option<LineRuntimeError>, LineRuntimeError> {
        let mut candidate = self.clone();
        let diagnostic = candidate.reduce_runtime_outcome(outcome)?;
        *self = candidate;
        Ok(diagnostic)
    }

    /// Reduces one host delivery batch atomically. Duplicate, stale, or
    /// mismatched evidence rejects the complete batch without advancing any
    /// lease or command journal row.
    pub(crate) fn accept_runtime_outcomes(
        &mut self,
        outcomes: &[crate::presentation::RuntimeLineHostOutcome],
    ) -> Result<Vec<LineRuntimeError>, LineRuntimeError> {
        let mut candidate = self.clone();
        let mut diagnostics = Vec::new();
        for outcome in outcomes {
            if let Some(diagnostic) = candidate.reduce_runtime_outcome(outcome)? {
                diagnostics.push(diagnostic);
            }
        }
        *self = candidate;
        Ok(diagnostics)
    }

    fn reduce_runtime_outcome(
        &mut self,
        outcome: &crate::presentation::RuntimeLineHostOutcome,
    ) -> Result<Option<LineRuntimeError>, LineRuntimeError> {
        let command_id = outcome.command();
        let Some(command) = self.issued_commands.get(command_id).cloned() else {
            if self.resolved_commands.contains(command_id) {
                return Err(LineRuntimeError::DuplicateCommandOutcome);
            }
            let Some(command) = self.superseded_commands.get(command_id).cloned() else {
                return Err(LineRuntimeError::UnknownCommandOutcome);
            };
            let crate::presentation::RuntimeLineHostCommand::Stage(
                crate::presentation::RuntimeStageCommand::SetCharacterLook { cue, .. },
            ) = command
            else {
                return Err(LineRuntimeError::StageOutcomeMismatch);
            };
            return match outcome {
                crate::presentation::RuntimeLineHostOutcome::Stage(
                    crate::presentation::RuntimeStageCommandOutcome::Accepted {
                        cue: echoed, ..
                    },
                ) if echoed == &cue => Ok(None),
                crate::presentation::RuntimeLineHostOutcome::Stage(
                    crate::presentation::RuntimeStageCommandOutcome::Completed {
                        cue: echoed, ..
                    },
                ) if echoed == &cue => {
                    self.ledger.set_state(
                        &cue,
                        RuntimeHandleLeaseState::Cancelling,
                        RuntimeHandleLeaseState::Released,
                    )?;
                    self.resolve_superseded(command_id);
                    self.resolve_issued_cancel_for_cue(&cue);
                    Ok(None)
                }
                crate::presentation::RuntimeLineHostOutcome::Stage(
                    crate::presentation::RuntimeStageCommandOutcome::Rejected { .. },
                ) => {
                    self.ledger.set_state(
                        &cue,
                        RuntimeHandleLeaseState::Cancelling,
                        RuntimeHandleLeaseState::Released,
                    )?;
                    self.resolve_superseded(command_id);
                    self.resolve_issued_cancel_for_cue(&cue);
                    Ok(None)
                }
                _ => Err(LineRuntimeError::StageOutcomeMismatch),
            };
        };

        let diagnostic = match (&command, outcome) {
            (
                crate::presentation::RuntimeLineHostCommand::Stage(
                    crate::presentation::RuntimeStageCommand::SetCharacterLook { cue, .. },
                ),
                crate::presentation::RuntimeLineHostOutcome::Stage(
                    crate::presentation::RuntimeStageCommandOutcome::Completed {
                        cue: echoed, ..
                    },
                ),
            ) if cue == echoed => {
                self.ledger.set_state(
                    cue,
                    RuntimeHandleLeaseState::Running,
                    RuntimeHandleLeaseState::Completed,
                )?;
                self.resolve_superseded_cue(cue);
                None
            }
            (
                crate::presentation::RuntimeLineHostCommand::Stage(
                    crate::presentation::RuntimeStageCommand::CancelCue { cue, .. },
                ),
                crate::presentation::RuntimeLineHostOutcome::Stage(
                    crate::presentation::RuntimeStageCommandOutcome::Cancelled {
                        cue: echoed, ..
                    },
                ),
            ) if cue == echoed => {
                self.ledger.set_state(
                    cue,
                    RuntimeHandleLeaseState::Cancelling,
                    RuntimeHandleLeaseState::Cancelled,
                )?;
                self.ledger.set_state(
                    cue,
                    RuntimeHandleLeaseState::Cancelled,
                    RuntimeHandleLeaseState::Released,
                )?;
                self.resolve_superseded_cue(cue);
                None
            }
            (
                crate::presentation::RuntimeLineHostCommand::Stage(
                    crate::presentation::RuntimeStageCommand::ReleaseActor { actor, .. },
                ),
                crate::presentation::RuntimeLineHostOutcome::Stage(
                    crate::presentation::RuntimeStageCommandOutcome::ReleasedActor {
                        actor: echoed,
                        ..
                    },
                ),
            ) if actor == echoed => {
                self.ledger.set_state(
                    actor,
                    RuntimeHandleLeaseState::Cancelling,
                    RuntimeHandleLeaseState::Released,
                )?;
                None
            }
            (
                crate::presentation::RuntimeLineHostCommand::Voice(
                    crate::presentation::RuntimeVoiceCommand::ReleaseDialogueVoice {
                        handle, ..
                    },
                ),
                crate::presentation::RuntimeLineHostOutcome::Voice(
                    crate::presentation::RuntimeVoiceCommandOutcome::Released {
                        handle: echoed,
                        ..
                    },
                ),
            ) if handle == echoed => {
                self.ledger.set_state(
                    handle,
                    RuntimeHandleLeaseState::Cancelling,
                    RuntimeHandleLeaseState::Released,
                )?;
                None
            }
            (
                crate::presentation::RuntimeLineHostCommand::Stage(command),
                crate::presentation::RuntimeLineHostOutcome::Stage(
                    crate::presentation::RuntimeStageCommandOutcome::Rejected { code, .. },
                ),
            ) => {
                fail_command_lease(&mut self.ledger, command)?;
                if let crate::presentation::RuntimeStageCommand::CancelCue { cue, .. } = command {
                    self.resolve_superseded_cue(cue);
                }
                Some(LineRuntimeError::StageCommandRejected { code: *code })
            }
            (
                crate::presentation::RuntimeLineHostCommand::Voice(command),
                crate::presentation::RuntimeLineHostOutcome::Voice(
                    crate::presentation::RuntimeVoiceCommandOutcome::Rejected { failure, .. },
                ),
            ) => {
                fail_voice_command_lease(&mut self.ledger, command)?;
                Some(LineRuntimeError::VoiceStartRejected {
                    failure: failure.clone(),
                })
            }
            _ => return Err(LineRuntimeError::StageOutcomeMismatch),
        };
        self.issued_commands.remove(command_id);
        self.resolved_commands.insert(command_id.clone());
        Ok(diagnostic)
    }

    pub(crate) fn commit_result(
        &mut self,
        ty: T,
        value: RuntimeValue,
    ) -> Result<(), LineRuntimeError> {
        if !matches!(self.result, RuntimeDialogueResultState::Uncommitted) {
            return Err(LineRuntimeError::ResultAlreadyCommitted);
        }
        self.result = RuntimeDialogueResultState::Committed { ty, value };
        Ok(())
    }

    pub(crate) fn begin_result_publication(&mut self) -> Result<(), LineRuntimeError> {
        let RuntimeDialogueResultState::Committed { ty, value } = &self.result else {
            return Err(LineRuntimeError::InvalidResultTransition);
        };
        self.result = RuntimeDialogueResultState::Publishing {
            ty: ty.clone(),
            value: value.clone(),
        };
        Ok(())
    }

    pub(crate) fn finish_result_publication(&mut self) -> Result<(), LineRuntimeError> {
        if !matches!(self.result, RuntimeDialogueResultState::Publishing { .. }) {
            return Err(LineRuntimeError::InvalidResultTransition);
        }
        self.result = RuntimeDialogueResultState::Published;
        Ok(())
    }

    pub(crate) fn abandon(&mut self) -> Result<(), LineRuntimeError> {
        match self.result {
            RuntimeDialogueResultState::Uncommitted
            | RuntimeDialogueResultState::Committed { .. }
            | RuntimeDialogueResultState::Publishing { .. } => {
                self.result = RuntimeDialogueResultState::Abandoned;
                Ok(())
            }
            RuntimeDialogueResultState::Abandoned => Ok(()),
            RuntimeDialogueResultState::Published => Err(LineRuntimeError::InvalidResultTransition),
        }
    }

    /// Prepares every activation-owned lease for deterministic close without
    /// touching executor-owned child fibers. Unstarted scheduled packets are
    /// first moved back to line scope so their value graph has one owner;
    /// running child custody remains `ChildScope` until that exact child
    /// reports completion. The complete ledger and command journal advance as
    /// one candidate transaction.
    pub(crate) fn prepare_handle_unwind(
        &mut self,
        activation: &DialogueActivationId,
        preserve_result: bool,
    ) -> Result<(), LineRuntimeError> {
        let mut candidate = self.clone();
        let unstarted = candidate
            .scheduled
            .iter()
            .filter_map(|scheduled| {
                matches!(
                    &scheduled.custody,
                    RuntimeScheduledCaptureCustody::Packet(_)
                )
                .then(|| scheduled.token().clone())
            })
            .collect::<Vec<_>>();
        for token in unstarted {
            candidate.complete_unstarted_scheduled(&LineTaskScheduledCompletion::new(
                token,
                ScopeExit::Cancelled,
            ))?;
        }

        let mut ledger = candidate.ledger.clone();
        let mut commands = crate::presentation::RuntimeCommandQueue::new(
            activation.clone(),
            candidate.command_sequence,
        );
        let mut leases = ledger.leases().values().cloned().collect::<Vec<_>>();
        leases.reverse();
        for lease in leases {
            if lease.state() == RuntimeHandleLeaseState::Released
                || matches!(lease.owner(), RuntimeHandleOwnerSlot::ChildScope(_))
                || matches!(
                    lease.state(),
                    RuntimeHandleLeaseState::Allocating | RuntimeHandleLeaseState::Cancelling
                )
            {
                continue;
            }
            if preserve_result
                && matches!(
                    lease.owner(),
                    RuntimeHandleOwnerSlot::DialogueResult(_)
                        | RuntimeHandleOwnerSlot::ParentFiber(_)
                )
            {
                continue;
            }
            ledger.drop_owned(lease.token(), lease.owner(), &mut commands)?;
        }
        candidate.ledger = ledger;
        candidate.record_commands(activation, commands)?;
        *self = candidate;
        Ok(())
    }

    pub(crate) fn release_frame(&mut self) -> Result<(), LineRuntimeError> {
        if self.frame_released {
            return Err(LineRuntimeError::DuplicateFrameRelease);
        }
        self.frame_released = true;
        Ok(())
    }

    #[must_use]
    pub(crate) fn failure_close_ready(&self) -> bool {
        self.issued_commands.is_empty()
            && self.superseded_commands.is_empty()
            && self.scheduled.iter().all(|scheduled| {
                matches!(
                    scheduled.state(),
                    RuntimeScheduledState::Completed
                        | RuntimeScheduledState::Cancelled
                        | RuntimeScheduledState::Failed
                )
            })
            && self
                .ledger
                .leases()
                .values()
                .all(|lease| lease.state() == RuntimeHandleLeaseState::Released)
    }

    #[must_use]
    pub(crate) fn successful_close_ready(&self) -> bool {
        self.issued_commands.is_empty()
            && self.superseded_commands.is_empty()
            && self.scheduled.iter().all(|scheduled| {
                matches!(
                    scheduled.state(),
                    RuntimeScheduledState::Completed
                        | RuntimeScheduledState::Cancelled
                        | RuntimeScheduledState::Failed
                )
            })
            && self.ledger.leases().values().all(|lease| {
                lease.state() == RuntimeHandleLeaseState::Released
                    || matches!(
                        lease.owner(),
                        RuntimeHandleOwnerSlot::DialogueResult(_)
                            | RuntimeHandleOwnerSlot::ParentFiber(_)
                    )
            })
    }

    #[must_use]
    pub(crate) fn is_terminal(&self) -> bool {
        self.terminal_kind().is_some() && self.failure_close_ready()
    }

    #[must_use]
    pub(crate) fn terminal_kind(&self) -> Option<RuntimeDialogueTerminalKind> {
        if !self.frame_released {
            return None;
        }
        match self.result {
            RuntimeDialogueResultState::Published => Some(RuntimeDialogueTerminalKind::Published),
            RuntimeDialogueResultState::Abandoned => Some(RuntimeDialogueTerminalKind::Abandoned),
            RuntimeDialogueResultState::Uncommitted
            | RuntimeDialogueResultState::Committed { .. }
            | RuntimeDialogueResultState::Publishing { .. } => None,
        }
    }

    pub(crate) fn into_published_handles(
        self,
    ) -> Result<RuntimePublishedDialogueHandles, LineRuntimeError> {
        if self.terminal_kind() != Some(RuntimeDialogueTerminalKind::Published)
            || !self.issued_commands.is_empty()
            || !self.superseded_commands.is_empty()
            || !self.prepared_commands.is_empty()
            || self.scheduled.iter().any(|scheduled| {
                !matches!(
                    scheduled.state(),
                    RuntimeScheduledState::Completed
                        | RuntimeScheduledState::Cancelled
                        | RuntimeScheduledState::Failed
                )
            })
            || self.ledger.leases().values().any(|lease| {
                lease.state() != RuntimeHandleLeaseState::Released
                    && !matches!(lease.owner(), RuntimeHandleOwnerSlot::ParentFiber(_))
            })
        {
            return Err(LineRuntimeError::TerminalDispositionMismatch);
        }
        Ok(RuntimePublishedDialogueHandles {
            ledger: self.ledger,
            command_sequence: self.command_sequence,
            issued_commands: self.issued_commands,
            resolved_commands: self.resolved_commands,
        })
    }
}

impl<T: Clone> AwbcRuntimeDialogueActivationSnapshot<T> {
    pub(crate) fn from_live(
        state: &RuntimeDialogueActivationState<T>,
    ) -> Result<Self, crate::value::AwbcRuntimeValueSnapshotError> {
        Ok(Self {
            ledger: state.ledger.clone(),
            command_sequence: state.command_sequence,
            issued_commands: state.issued_commands.clone(),
            superseded_commands: state.superseded_commands.clone(),
            resolved_commands: state.resolved_commands.clone(),
            scheduled: state
                .scheduled
                .iter()
                .map(AwbcRuntimeScheduledLineTaskSnapshot::from_live)
                .collect::<Result<_, _>>()?,
            result: AwbcRuntimeDialogueResultSnapshot::from_live(&state.result)?,
            frame_released: state.frame_released,
        })
    }

    pub(crate) fn into_live(
        self,
    ) -> Result<RuntimeDialogueActivationState<T>, crate::value::AwbcRuntimeValueSnapshotError>
    {
        Ok(RuntimeDialogueActivationState {
            ledger: self.ledger,
            command_sequence: self.command_sequence,
            issued_commands: self.issued_commands,
            superseded_commands: self.superseded_commands,
            resolved_commands: self.resolved_commands,
            scheduled: self
                .scheduled
                .into_iter()
                .map(AwbcRuntimeScheduledLineTaskSnapshot::into_live)
                .collect::<Result<_, _>>()?,
            result: self.result.into_live()?,
            frame_released: self.frame_released,
            prepared_commands: Vec::new(),
        })
    }
}

impl AwbcRuntimePublishedDialogueHandlesSnapshot {
    pub(crate) fn from_live(state: &RuntimePublishedDialogueHandles) -> Self {
        Self {
            ledger: state.ledger.clone(),
            command_sequence: state.command_sequence,
            issued_commands: state.issued_commands.clone(),
            resolved_commands: state.resolved_commands.clone(),
        }
    }

    pub(crate) fn into_live(self) -> RuntimePublishedDialogueHandles {
        RuntimePublishedDialogueHandles {
            ledger: self.ledger,
            command_sequence: self.command_sequence,
            issued_commands: self.issued_commands,
            resolved_commands: self.resolved_commands,
        }
    }
}

impl AwbcRuntimeScheduledLineTaskSnapshot {
    fn from_live(
        state: &RuntimeScheduledLineTask,
    ) -> Result<Self, crate::value::AwbcRuntimeValueSnapshotError> {
        state
            .validate_custody()
            .map_err(scheduled_custody_snapshot_error)?;
        Ok(Self {
            token: state.token.clone(),
            child: state.child,
            work: state.work.clone(),
            deadline: state.deadline,
            custody: AwbcRuntimeScheduledCaptureCustodySnapshot::from_live(&state.custody)?,
            state: state.state,
        })
    }

    fn into_live(
        self,
    ) -> Result<RuntimeScheduledLineTask, crate::value::AwbcRuntimeValueSnapshotError> {
        let custody = self.custody.into_live()?;
        RuntimeScheduledLineTask::try_from_parts(
            self.token,
            self.child,
            self.work,
            self.deadline,
            custody,
            self.state,
        )
        .map_err(scheduled_custody_snapshot_error)
    }
}

impl AwbcRuntimeScheduledCaptureCustodySnapshot {
    fn from_live(
        custody: &RuntimeScheduledCaptureCustody,
    ) -> Result<Self, crate::value::AwbcRuntimeValueSnapshotError> {
        Ok(match custody {
            RuntimeScheduledCaptureCustody::Packet(bindings) => {
                Self::Packet(snapshot_local_bindings(bindings)?)
            }
            RuntimeScheduledCaptureCustody::ChildFiber(locals) => Self::ChildFiber(locals.to_vec()),
            RuntimeScheduledCaptureCustody::LineScope(bindings) => {
                Self::LineScope(snapshot_local_bindings(bindings)?)
            }
        })
    }

    fn into_live(
        self,
    ) -> Result<RuntimeScheduledCaptureCustody, crate::value::AwbcRuntimeValueSnapshotError> {
        Ok(match self {
            Self::Packet(bindings) => {
                RuntimeScheduledCaptureCustody::Packet(live_local_bindings(bindings)?)
            }
            Self::ChildFiber(locals) => {
                RuntimeScheduledCaptureCustody::ChildFiber(locals.into_boxed_slice())
            }
            Self::LineScope(bindings) => {
                RuntimeScheduledCaptureCustody::LineScope(live_local_bindings(bindings)?)
            }
        })
    }
}

fn snapshot_local_bindings(
    bindings: &[RuntimeLocalBinding],
) -> Result<Vec<AwbcRuntimeLocalBindingSnapshot>, crate::value::AwbcRuntimeValueSnapshotError> {
    bindings
        .iter()
        .map(|capture| {
            Ok(AwbcRuntimeLocalBindingSnapshot {
                local: capture.local,
                value: crate::value::AwbcRuntimeValueSnapshot::from_runtime_value(&capture.value)?,
            })
        })
        .collect()
}

fn live_local_bindings(
    bindings: Vec<AwbcRuntimeLocalBindingSnapshot>,
) -> Result<Box<[RuntimeLocalBinding]>, crate::value::AwbcRuntimeValueSnapshotError> {
    bindings
        .into_iter()
        .map(|capture| {
            Ok(RuntimeLocalBinding {
                local: capture.local,
                value: capture.value.into_runtime_value()?,
            })
        })
        .collect::<Result<Vec<_>, crate::value::AwbcRuntimeValueSnapshotError>>()
        .map(Vec::into_boxed_slice)
}

fn scheduled_custody_snapshot_error(
    error: LineRuntimeError,
) -> crate::value::AwbcRuntimeValueSnapshotError {
    crate::value::AwbcRuntimeValueSnapshotError::Message {
        message: error.to_string(),
    }
}

fn exact_scheduled_packet_mut<'a>(
    scheduled: &'a mut [RuntimeScheduledLineTask],
    token: &RuntimeLineHandleToken,
) -> Result<&'a mut RuntimeScheduledLineTask, LineRuntimeError> {
    scheduled
        .iter_mut()
        .find(|scheduled| scheduled.token() == token)
        .ok_or(LineRuntimeError::MissingScheduledWork)
}

impl<T: Clone> AwbcRuntimeDialogueResultSnapshot<T> {
    fn from_live(
        state: &RuntimeDialogueResultState<T>,
    ) -> Result<Self, crate::value::AwbcRuntimeValueSnapshotError> {
        Ok(match state {
            RuntimeDialogueResultState::Uncommitted => Self::Uncommitted,
            RuntimeDialogueResultState::Committed { ty, value } => Self::Committed {
                ty: ty.clone(),
                value: crate::value::AwbcRuntimeValueSnapshot::from_runtime_value(value)?,
            },
            RuntimeDialogueResultState::Publishing { ty, value } => Self::Publishing {
                ty: ty.clone(),
                value: crate::value::AwbcRuntimeValueSnapshot::from_runtime_value(value)?,
            },
            RuntimeDialogueResultState::Published => Self::Published,
            RuntimeDialogueResultState::Abandoned => Self::Abandoned,
        })
    }

    fn into_live(
        self,
    ) -> Result<RuntimeDialogueResultState<T>, crate::value::AwbcRuntimeValueSnapshotError> {
        Ok(match self {
            Self::Uncommitted => RuntimeDialogueResultState::Uncommitted,
            Self::Committed { ty, value } => RuntimeDialogueResultState::Committed {
                ty,
                value: value.into_runtime_value()?,
            },
            Self::Publishing { ty, value } => RuntimeDialogueResultState::Publishing {
                ty,
                value: value.into_runtime_value()?,
            },
            Self::Published => RuntimeDialogueResultState::Published,
            Self::Abandoned => RuntimeDialogueResultState::Abandoned,
        })
    }
}

impl RuntimePublishedDialogueHandles {
    pub(crate) fn restore_admit(
        &self,
        activation: &DialogueActivationId,
    ) -> Result<(), LineRuntimeError> {
        validate_restored_ledger(activation, &self.ledger)?;
        validate_restored_command_journal(
            activation,
            self.command_sequence,
            &self.issued_commands,
            &BTreeMap::new(),
            &self.resolved_commands,
            &self.ledger,
        )?;
        if self.ledger.leases().values().any(|lease| {
            lease.state() != RuntimeHandleLeaseState::Released
                && !matches!(lease.owner(), RuntimeHandleOwnerSlot::ParentFiber(_))
        }) {
            return Err(LineRuntimeError::InvalidRestoredLedgerState);
        }
        Ok(())
    }

    #[must_use]
    pub(crate) fn has_live_leases(&self) -> bool {
        self.ledger
            .leases()
            .values()
            .any(|lease| lease.state() != RuntimeHandleLeaseState::Released)
    }

    pub(crate) fn accept_outcome(
        &mut self,
        outcome: &crate::presentation::RuntimeLineHostOutcome,
    ) -> Result<Option<LineRuntimeError>, LineRuntimeError> {
        let mut candidate = self.clone();
        let diagnostic = candidate.reduce_outcome(outcome)?;
        *self = candidate;
        Ok(diagnostic)
    }

    fn reduce_outcome(
        &mut self,
        outcome: &crate::presentation::RuntimeLineHostOutcome,
    ) -> Result<Option<LineRuntimeError>, LineRuntimeError> {
        let command_id = outcome.command();
        let Some(command) = self.issued_commands.remove(command_id) else {
            return Err(if self.resolved_commands.contains(command_id) {
                LineRuntimeError::DuplicateCommandOutcome
            } else {
                LineRuntimeError::UnknownCommandOutcome
            });
        };
        let rejected = match (&command, outcome) {
            (
                crate::presentation::RuntimeLineHostCommand::Stage(
                    crate::presentation::RuntimeStageCommand::ReleaseActor { actor, .. },
                ),
                crate::presentation::RuntimeLineHostOutcome::Stage(
                    crate::presentation::RuntimeStageCommandOutcome::ReleasedActor {
                        actor: echoed,
                        ..
                    },
                ),
            ) if actor == echoed => {
                self.ledger.set_state(
                    actor,
                    RuntimeHandleLeaseState::Cancelling,
                    RuntimeHandleLeaseState::Released,
                )?;
                None
            }
            (
                crate::presentation::RuntimeLineHostCommand::Stage(
                    crate::presentation::RuntimeStageCommand::CancelCue { cue, .. },
                ),
                crate::presentation::RuntimeLineHostOutcome::Stage(
                    crate::presentation::RuntimeStageCommandOutcome::Cancelled {
                        cue: echoed, ..
                    },
                ),
            ) if cue == echoed => {
                self.ledger.set_state(
                    cue,
                    RuntimeHandleLeaseState::Cancelling,
                    RuntimeHandleLeaseState::Cancelled,
                )?;
                self.ledger.set_state(
                    cue,
                    RuntimeHandleLeaseState::Cancelled,
                    RuntimeHandleLeaseState::Released,
                )?;
                None
            }
            (
                crate::presentation::RuntimeLineHostCommand::Voice(
                    crate::presentation::RuntimeVoiceCommand::ReleaseDialogueVoice {
                        handle, ..
                    },
                ),
                crate::presentation::RuntimeLineHostOutcome::Voice(
                    crate::presentation::RuntimeVoiceCommandOutcome::Released {
                        handle: echoed,
                        ..
                    },
                ),
            ) if handle == echoed => {
                self.ledger.set_state(
                    handle,
                    RuntimeHandleLeaseState::Cancelling,
                    RuntimeHandleLeaseState::Released,
                )?;
                None
            }
            (
                crate::presentation::RuntimeLineHostCommand::Stage(command),
                crate::presentation::RuntimeLineHostOutcome::Stage(
                    crate::presentation::RuntimeStageCommandOutcome::Rejected { code, .. },
                ),
            ) => {
                let token = published_command_token(command)?;
                self.ledger.set_state(
                    token,
                    RuntimeHandleLeaseState::Cancelling,
                    RuntimeHandleLeaseState::Failed,
                )?;
                self.ledger.set_state(
                    token,
                    RuntimeHandleLeaseState::Failed,
                    RuntimeHandleLeaseState::Released,
                )?;
                Some(LineRuntimeError::StageCommandRejected { code: *code })
            }
            (
                crate::presentation::RuntimeLineHostCommand::Voice(command),
                crate::presentation::RuntimeLineHostOutcome::Voice(
                    crate::presentation::RuntimeVoiceCommandOutcome::Rejected { failure, .. },
                ),
            ) => {
                let token = published_voice_command_token(command)?;
                self.ledger.set_state(
                    token,
                    RuntimeHandleLeaseState::Cancelling,
                    RuntimeHandleLeaseState::Failed,
                )?;
                self.ledger.set_state(
                    token,
                    RuntimeHandleLeaseState::Failed,
                    RuntimeHandleLeaseState::Released,
                )?;
                Some(LineRuntimeError::VoiceStartRejected {
                    failure: failure.clone(),
                })
            }
            _ => return Err(LineRuntimeError::StageOutcomeMismatch),
        };
        self.resolved_commands.insert(command_id.clone());
        Ok(rejected)
    }

    /// Atomically reconciles the exact parent-fiber slot graph produced by one
    /// AWBC instruction. A token may move between parent registers or leave
    /// the graph through a typed drop, but execution cannot manufacture a new
    /// published token or use same-execution ownership as a substitute for the
    /// exact source slot.
    pub(crate) fn reconcile_parent_owned(
        &mut self,
        activation: &DialogueActivationId,
        execution: crate::runtime_id::ExecutionInstanceId,
        before: &BTreeMap<RuntimeLineHandleToken, RuntimeOwnedSlotId>,
        after: &BTreeMap<RuntimeLineHandleToken, RuntimeOwnedSlotId>,
        drop_policy: Option<RuntimeDropPolicy>,
    ) -> Result<RuntimeHandleDropReceipt, LineRuntimeError> {
        if after.keys().any(|token| !before.contains_key(token)) {
            return Err(LineRuntimeError::UnexpectedParentHandleOccurrence);
        }
        let mut candidate = self.clone();
        let mut queue = RuntimeCommandQueue::new(activation.clone(), candidate.command_sequence);
        for (token, source) in before {
            if token.activation() != activation {
                return Err(LineRuntimeError::WrongActivation);
            }
            if source.execution() != execution
                || after
                    .get(token)
                    .is_some_and(|destination| destination.execution() != execution)
            {
                return Err(LineRuntimeError::WrongOwner);
            }
            let expected = RuntimeHandleOwnerSlot::ParentFiber(*source);
            match after.get(token) {
                Some(destination) if destination == source => {
                    let lease = candidate
                        .ledger
                        .lease(token)
                        .ok_or(LineRuntimeError::UnknownHandle)?;
                    if lease.owner() != &expected {
                        return Err(LineRuntimeError::WrongOwner);
                    }
                }
                Some(destination) => candidate.ledger.transfer(
                    token,
                    &expected,
                    RuntimeHandleOwnerSlot::ParentFiber(*destination),
                )?,
                None => {
                    let policy = drop_policy.ok_or(LineRuntimeError::UnjournaledHandleDrop)?;
                    candidate
                        .ledger
                        .drop_owned_with_policy(token, &expected, policy, &mut queue)?;
                }
            }
        }
        let commands = candidate.record_parent_drop_commands(activation, queue)?;
        *self = candidate;
        Ok(RuntimeHandleDropReceipt { commands })
    }

    fn record_parent_drop_commands(
        &mut self,
        activation: &DialogueActivationId,
        queue: RuntimeCommandQueue,
    ) -> Result<Vec<crate::presentation::RuntimeLineHostCommand>, LineRuntimeError> {
        if queue.activation() != activation || queue.start_sequence() != self.command_sequence {
            return Err(LineRuntimeError::StaleCommandQueue);
        }
        let next_sequence = queue.next_sequence();
        let commands = queue.into_commands();
        if self
            .issued_commands
            .len()
            .checked_add(self.resolved_commands.len())
            .and_then(|count| count.checked_add(commands.len()))
            .is_none_or(|count| count > MAX_LINE_COMMAND_HISTORY)
        {
            return Err(LineRuntimeError::CommandHistoryLimitExceeded);
        }
        let mut issued = self.issued_commands.clone();
        for command in &commands {
            if command.command().activation() != activation
                || issued
                    .insert(command.command().clone(), command.clone())
                    .is_some()
                || self.resolved_commands.contains(command.command())
            {
                return Err(LineRuntimeError::DuplicateCommandIdentity);
            }
        }
        self.command_sequence = next_sequence;
        self.issued_commands = issued;
        Ok(commands)
    }

    #[must_use]
    pub(crate) fn is_terminal(&self) -> bool {
        !self.has_live_leases() && self.issued_commands.is_empty()
    }
}

fn published_command_token(
    command: &crate::presentation::RuntimeStageCommand,
) -> Result<&RuntimeLineHandleToken, LineRuntimeError> {
    match command {
        crate::presentation::RuntimeStageCommand::ReleaseActor { actor, .. } => Ok(actor),
        crate::presentation::RuntimeStageCommand::CancelCue { cue, .. } => Ok(cue),
        crate::presentation::RuntimeStageCommand::AcquireActor { .. }
        | crate::presentation::RuntimeStageCommand::SetCharacterLook { .. } => {
            Err(LineRuntimeError::StageOutcomeMismatch)
        }
    }
}

fn published_voice_command_token(
    command: &crate::presentation::RuntimeVoiceCommand,
) -> Result<&RuntimeLineHandleToken, LineRuntimeError> {
    match command {
        crate::presentation::RuntimeVoiceCommand::ReleaseDialogueVoice { handle, .. } => Ok(handle),
        crate::presentation::RuntimeVoiceCommand::StartDialogueVoice { .. } => {
            Err(LineRuntimeError::StageOutcomeMismatch)
        }
    }
}

fn validate_restored_ledger(
    activation: &DialogueActivationId,
    ledger: &RuntimeLineHandleLedger,
) -> Result<(), LineRuntimeError> {
    for (site, next) in &ledger.issuance_by_site {
        if *next == 0
            || ledger
                .leases
                .keys()
                .filter(|token| token.site() == *site)
                .any(|token| token.issuance() >= *next)
        {
            return Err(LineRuntimeError::InvalidRestoredLedgerState);
        }
    }
    for (token, lease) in &ledger.leases {
        if token != lease.token()
            || token.activation() != activation
            || ledger
                .issuance_by_site
                .get(&token.site())
                .is_none_or(|next| token.issuance() >= *next)
        {
            return Err(LineRuntimeError::InvalidRestoredLedgerState);
        }
        match lease.owner() {
            RuntimeHandleOwnerSlot::ChildScope(work)
                if work.activation_id() != activation || !work.is_well_formed() =>
            {
                return Err(LineRuntimeError::InvalidRestoredLedgerState);
            }
            RuntimeHandleOwnerSlot::LineScope
            | RuntimeHandleOwnerSlot::ActivationLocal(_)
            | RuntimeHandleOwnerSlot::DialogueResult(_)
            | RuntimeHandleOwnerSlot::ChildScope(_)
            | RuntimeHandleOwnerSlot::ParentFiber(_) => {}
        }
        let valid_state = match lease.resource() {
            RuntimeHandleResource::StageActor(_) => matches!(
                lease.state(),
                RuntimeHandleLeaseState::Allocating
                    | RuntimeHandleLeaseState::Active
                    | RuntimeHandleLeaseState::Cancelling
                    | RuntimeHandleLeaseState::Failed
                    | RuntimeHandleLeaseState::Released
            ),
            RuntimeHandleResource::Cue(_) => matches!(
                lease.state(),
                RuntimeHandleLeaseState::Pending
                    | RuntimeHandleLeaseState::Running
                    | RuntimeHandleLeaseState::Completed
                    | RuntimeHandleLeaseState::Cancelling
                    | RuntimeHandleLeaseState::Cancelled
                    | RuntimeHandleLeaseState::Failed
                    | RuntimeHandleLeaseState::Released
            ),
            RuntimeHandleResource::Voice(_) => matches!(
                lease.state(),
                RuntimeHandleLeaseState::Active
                    | RuntimeHandleLeaseState::Completed
                    | RuntimeHandleLeaseState::Cancelling
                    | RuntimeHandleLeaseState::Failed
                    | RuntimeHandleLeaseState::Released
            ),
        };
        if !valid_state {
            return Err(LineRuntimeError::InvalidRestoredLedgerState);
        }
    }
    Ok(())
}

fn validate_restored_command_journal(
    activation: &DialogueActivationId,
    command_sequence: u64,
    issued: &BTreeMap<
        crate::presentation::RuntimeLineCommandId,
        crate::presentation::RuntimeLineHostCommand,
    >,
    superseded: &BTreeMap<
        crate::presentation::RuntimeLineCommandId,
        crate::presentation::RuntimeLineHostCommand,
    >,
    resolved: &std::collections::BTreeSet<crate::presentation::RuntimeLineCommandId>,
    ledger: &RuntimeLineHandleLedger,
) -> Result<(), LineRuntimeError> {
    if issued
        .len()
        .checked_add(superseded.len())
        .and_then(|count| count.checked_add(resolved.len()))
        .is_none_or(|count| {
            count > MAX_LINE_COMMAND_HISTORY
                || usize::try_from(command_sequence).ok() != Some(count)
        })
        || issued
            .keys()
            .any(|id| superseded.contains_key(id) || resolved.contains(id))
        || superseded.keys().any(|id| resolved.contains(id))
    {
        return Err(LineRuntimeError::InvalidRestoredCommandJournal);
    }
    for (id, command) in issued.iter().chain(superseded) {
        if id != command.command()
            || id.activation() != activation
            || id.sequence() >= command_sequence
            || !restored_command_references_are_valid(command, activation, ledger)
            || !restored_issued_command_state_is_valid(command, ledger)
        {
            return Err(LineRuntimeError::InvalidRestoredCommandJournal);
        }
    }
    if superseded.values().any(|command| {
        !matches!(
            command,
            crate::presentation::RuntimeLineHostCommand::Stage(
                crate::presentation::RuntimeStageCommand::SetCharacterLook { .. }
            )
        )
    }) || resolved
        .iter()
        .any(|id| id.activation() != activation || id.sequence() >= command_sequence)
    {
        return Err(LineRuntimeError::InvalidRestoredCommandJournal);
    }
    Ok(())
}

fn restored_issued_command_state_is_valid(
    command: &crate::presentation::RuntimeLineHostCommand,
    ledger: &RuntimeLineHandleLedger,
) -> bool {
    let state = |token: &RuntimeLineHandleToken| ledger.lease(token).map(RuntimeHandleLease::state);
    match command {
        crate::presentation::RuntimeLineHostCommand::Stage(command) => match command {
            crate::presentation::RuntimeStageCommand::AcquireActor { actor, .. } => {
                state(actor) == Some(RuntimeHandleLeaseState::Allocating)
            }
            crate::presentation::RuntimeStageCommand::SetCharacterLook { cue, .. } => matches!(
                state(cue),
                Some(RuntimeHandleLeaseState::Pending | RuntimeHandleLeaseState::Running)
            ),
            crate::presentation::RuntimeStageCommand::ReleaseActor { actor, .. } => {
                state(actor) == Some(RuntimeHandleLeaseState::Cancelling)
            }
            crate::presentation::RuntimeStageCommand::CancelCue { cue, .. } => {
                state(cue) == Some(RuntimeHandleLeaseState::Cancelling)
            }
        },
        crate::presentation::RuntimeLineHostCommand::Voice(command) => match command {
            crate::presentation::RuntimeVoiceCommand::StartDialogueVoice { .. } => true,
            crate::presentation::RuntimeVoiceCommand::ReleaseDialogueVoice { handle, .. } => {
                state(handle) == Some(RuntimeHandleLeaseState::Cancelling)
            }
        },
    }
}

fn restored_command_references_are_valid(
    command: &crate::presentation::RuntimeLineHostCommand,
    activation: &DialogueActivationId,
    ledger: &RuntimeLineHandleLedger,
) -> bool {
    let token_is = |token: &RuntimeLineHandleToken, kind| {
        token.activation() == activation
            && ledger
                .lease(token)
                .is_some_and(|lease| lease.resource().kind() == kind)
    };
    match command {
        crate::presentation::RuntimeLineHostCommand::Stage(command) => match command {
            crate::presentation::RuntimeStageCommand::AcquireActor { actor, .. }
            | crate::presentation::RuntimeStageCommand::ReleaseActor { actor, .. } => {
                token_is(actor, RuntimeHandleKind::StageActor)
            }
            crate::presentation::RuntimeStageCommand::SetCharacterLook { cue, actor, .. } => {
                token_is(cue, RuntimeHandleKind::Cue)
                    && token_is(actor, RuntimeHandleKind::StageActor)
            }
            crate::presentation::RuntimeStageCommand::CancelCue { cue, .. } => {
                token_is(cue, RuntimeHandleKind::Cue)
            }
        },
        crate::presentation::RuntimeLineHostCommand::Voice(command) => match command {
            crate::presentation::RuntimeVoiceCommand::StartDialogueVoice { .. } => true,
            crate::presentation::RuntimeVoiceCommand::ReleaseDialogueVoice { handle, .. } => {
                token_is(handle, RuntimeHandleKind::Voice)
            }
        },
    }
}

fn fail_command_lease(
    ledger: &mut RuntimeLineHandleLedger,
    command: &crate::presentation::RuntimeStageCommand,
) -> Result<(), LineRuntimeError> {
    let token = match command {
        crate::presentation::RuntimeStageCommand::AcquireActor { actor, .. }
        | crate::presentation::RuntimeStageCommand::ReleaseActor { actor, .. } => actor,
        crate::presentation::RuntimeStageCommand::SetCharacterLook { cue, .. }
        | crate::presentation::RuntimeStageCommand::CancelCue { cue, .. } => cue,
    };
    fail_lease(ledger, token)
}

fn fail_voice_command_lease(
    ledger: &mut RuntimeLineHandleLedger,
    command: &crate::presentation::RuntimeVoiceCommand,
) -> Result<(), LineRuntimeError> {
    match command {
        crate::presentation::RuntimeVoiceCommand::StartDialogueVoice { .. } => Ok(()),
        crate::presentation::RuntimeVoiceCommand::ReleaseDialogueVoice { handle, .. } => {
            fail_lease(ledger, handle)
        }
    }
}

fn fail_lease(
    ledger: &mut RuntimeLineHandleLedger,
    token: &RuntimeLineHandleToken,
) -> Result<(), LineRuntimeError> {
    let state = ledger
        .lease(token)
        .ok_or(LineRuntimeError::UnknownHandle)?
        .state();
    if state == RuntimeHandleLeaseState::Released {
        return Ok(());
    }
    ledger.set_state(token, state, RuntimeHandleLeaseState::Failed)?;
    ledger.set_state(
        token,
        RuntimeHandleLeaseState::Failed,
        RuntimeHandleLeaseState::Released,
    )
}

impl RuntimeLineHandleLedger {
    pub(crate) fn issue(
        &mut self,
        activation: &DialogueActivationId,
        site: &RuntimeLineHandleSite,
        resource: RuntimeHandleResource,
        owner: RuntimeHandleOwnerSlot,
    ) -> Result<RuntimeOpaqueValue, LineRuntimeError> {
        self.issue_exact(
            activation,
            site.id(),
            site.kind(),
            site.opaque_owner(),
            resource,
            owner,
        )
    }

    /// Issues one resource from executor-neutral, already-admitted site
    /// evidence. Structured plans and Product AWBC keep their own exact type
    /// coordinates; the ledger consumes only their common runtime site,
    /// handle-kind, and opaque-owner proof.
    pub(crate) fn issue_exact(
        &mut self,
        activation: &DialogueActivationId,
        site: RuntimeLineHandleSiteId,
        kind: RuntimeHandleKind,
        opaque_owner: &RuntimeOpaqueTypeOwner,
        resource: RuntimeHandleResource,
        owner: RuntimeHandleOwnerSlot,
    ) -> Result<RuntimeOpaqueValue, LineRuntimeError> {
        if resource.kind() != kind
            || opaque_owner.value_class() != RuntimeOpaqueValueClass::AffineHandle(kind)
            || opaque_owner.persistence() != RuntimeOpaquePersistence::SnapshotOnly
            || opaque_owner.producer()
                != &kind
                    .try_producer()
                    .map_err(|_| LineRuntimeError::WrongOpaqueProducer)?
        {
            return Err(LineRuntimeError::InvalidHandleSite);
        }
        if self.leases.len() >= MAX_LINE_LIVE_HANDLES {
            return Err(LineRuntimeError::LiveHandleLimitExceeded);
        }
        let issuance = self.issuance_by_site.get(&site).copied().unwrap_or(0);
        let next = issuance
            .checked_add(1)
            .ok_or(LineRuntimeError::HandleIssuanceOverflow)?;
        let token = RuntimeLineHandleToken::new(activation.clone(), site, issuance);
        if self.leases.contains_key(&token) {
            return Err(LineRuntimeError::DuplicateHandleToken);
        }
        let state = match &resource {
            RuntimeHandleResource::StageActor(_) => RuntimeHandleLeaseState::Allocating,
            RuntimeHandleResource::Cue(cue) => match cue.origin() {
                RuntimeCueOrigin::Scheduled { .. } => RuntimeHandleLeaseState::Pending,
                RuntimeCueOrigin::StageLook => RuntimeHandleLeaseState::Pending,
            },
            RuntimeHandleResource::Voice(_) => RuntimeHandleLeaseState::Active,
        };
        let opaque = RuntimeOpaqueValue::new_exact(opaque_owner, token.encode_payload());
        self.issuance_by_site.insert(site, next);
        self.leases.insert(
            token.clone(),
            RuntimeHandleLease {
                token,
                owner,
                state,
                resource,
            },
        );
        Ok(opaque)
    }

    pub(crate) fn transfer(
        &mut self,
        token: &RuntimeLineHandleToken,
        expected: &RuntimeHandleOwnerSlot,
        destination: RuntimeHandleOwnerSlot,
    ) -> Result<(), LineRuntimeError> {
        let lease = self
            .leases
            .get_mut(token)
            .ok_or(LineRuntimeError::UnknownHandle)?;
        if lease.owner != *expected {
            return Err(LineRuntimeError::WrongOwner);
        }
        if lease.state == RuntimeHandleLeaseState::Released {
            return Err(LineRuntimeError::ReleasedHandle);
        }
        if !owner_transition_is_legal(&lease.owner, &destination) {
            return Err(LineRuntimeError::IllegalOwnerTransition);
        }
        lease.owner = destination;
        Ok(())
    }

    pub(crate) fn drop_owned(
        &mut self,
        token: &RuntimeLineHandleToken,
        expected: &RuntimeHandleOwnerSlot,
        commands: &mut RuntimeCommandQueue,
    ) -> Result<(), LineRuntimeError> {
        self.drop_owned_with_policy(token, expected, RuntimeDropPolicy::Default, commands)
    }

    pub(crate) fn drop_owned_with_policy(
        &mut self,
        token: &RuntimeLineHandleToken,
        expected: &RuntimeHandleOwnerSlot,
        policy: RuntimeDropPolicy,
        commands: &mut RuntimeCommandQueue,
    ) -> Result<(), LineRuntimeError> {
        let lease = self
            .leases
            .get_mut(token)
            .ok_or(LineRuntimeError::UnknownHandle)?;
        if lease.owner != *expected {
            return Err(LineRuntimeError::WrongOwner);
        }
        if lease.state == RuntimeHandleLeaseState::Released {
            return Err(LineRuntimeError::ReleasedHandle);
        }
        match &lease.resource {
            RuntimeHandleResource::StageActor(_) => {
                if lease.state != RuntimeHandleLeaseState::Allocating {
                    commands.push_release_actor(token.clone())?;
                    lease.state = RuntimeHandleLeaseState::Cancelling;
                }
            }
            RuntimeHandleResource::Cue(cue) => match (cue.origin(), lease.state) {
                (
                    RuntimeCueOrigin::StageLook,
                    RuntimeHandleLeaseState::Pending | RuntimeHandleLeaseState::Running,
                ) => {
                    commands.push_cancel_cue(token.clone())?;
                    lease.state = RuntimeHandleLeaseState::Cancelling;
                }
                (RuntimeCueOrigin::Scheduled { .. }, RuntimeHandleLeaseState::Pending) => {
                    lease.state = RuntimeHandleLeaseState::Cancelled
                }
                (RuntimeCueOrigin::Scheduled { .. }, RuntimeHandleLeaseState::Running) => {
                    lease.state = RuntimeHandleLeaseState::Cancelling
                }
                (_, RuntimeHandleLeaseState::Completed | RuntimeHandleLeaseState::Cancelled) => {
                    lease.state = RuntimeHandleLeaseState::Released;
                }
                _ => return Err(LineRuntimeError::InvalidDropTransition),
            },
            RuntimeHandleResource::Voice(_) => {
                commands.push_release_voice(token.clone(), policy)?;
                lease.state = RuntimeHandleLeaseState::Cancelling;
            }
        }
        Ok(())
    }

    pub(crate) fn validate_value(
        &self,
        value: &RuntimeOpaqueValue,
        expected_kind: RuntimeHandleKind,
        activation: &DialogueActivationId,
    ) -> Result<&RuntimeHandleLease, LineRuntimeError> {
        if value.producer()
            != &expected_kind
                .try_producer()
                .map_err(|_| LineRuntimeError::WrongOpaqueProducer)?
            || value.value_class() != RuntimeOpaqueValueClass::AffineHandle(expected_kind)
            || value.persistence() != RuntimeOpaquePersistence::SnapshotOnly
        {
            return Err(LineRuntimeError::WrongOpaqueProducer);
        }
        let token = RuntimeLineHandleToken::try_decode_payload(value.payload())
            .map_err(|_| LineRuntimeError::InvalidHandlePayload)?;
        if token.activation().artifact() != activation.artifact() {
            return Err(LineRuntimeError::StaleGeneration);
        }
        if token.activation() != activation {
            return Err(LineRuntimeError::WrongActivation);
        }
        let lease = self
            .leases
            .get(&token)
            .ok_or(LineRuntimeError::UnknownHandle)?;
        if lease.resource.kind() != expected_kind {
            return Err(LineRuntimeError::InvalidHandleSite);
        }
        if lease.state == RuntimeHandleLeaseState::Released {
            return Err(LineRuntimeError::ReleasedHandle);
        }
        Ok(lease)
    }

    pub(crate) fn set_state(
        &mut self,
        token: &RuntimeLineHandleToken,
        expected: RuntimeHandleLeaseState,
        state: RuntimeHandleLeaseState,
    ) -> Result<(), LineRuntimeError> {
        let lease = self
            .leases
            .get_mut(token)
            .ok_or(LineRuntimeError::UnknownHandle)?;
        if lease.state != expected {
            return Err(LineRuntimeError::InvalidLeaseTransition {
                expected,
                actual: lease.state,
            });
        }
        if !lease_transition_is_legal(expected, state) {
            return Err(LineRuntimeError::InvalidLeaseTransition {
                expected,
                actual: state,
            });
        }
        lease.state = state;
        Ok(())
    }

    pub(crate) fn lease(&self, token: &RuntimeLineHandleToken) -> Option<&RuntimeHandleLease> {
        self.leases.get(token)
    }

    #[must_use]
    pub fn leases(&self) -> &BTreeMap<RuntimeLineHandleToken, RuntimeHandleLease> {
        &self.leases
    }

    pub(crate) fn next_voice_lease_ordinal(&self) -> Result<u32, LineRuntimeError> {
        let count = self
            .leases
            .values()
            .filter(|lease| matches!(lease.resource(), RuntimeHandleResource::Voice(_)))
            .count();
        u32::try_from(count).map_err(|_| LineRuntimeError::HandleIssuanceOverflow)
    }

    #[must_use]
    pub fn token_from_value(
        value: &RuntimeValue,
    ) -> Result<RuntimeLineHandleToken, LineRuntimeError> {
        let RuntimeValue::Opaque(value) = value else {
            return Err(LineRuntimeError::WrongOpaqueProducer);
        };
        RuntimeLineHandleToken::try_decode_payload(value.payload())
            .map_err(|_| LineRuntimeError::InvalidHandlePayload)
    }
}

fn owner_transition_is_legal(
    source: &RuntimeHandleOwnerSlot,
    destination: &RuntimeHandleOwnerSlot,
) -> bool {
    matches!(
        (source, destination),
        (
            RuntimeHandleOwnerSlot::LineScope,
            RuntimeHandleOwnerSlot::ActivationLocal(_)
                | RuntimeHandleOwnerSlot::ChildScope(_)
                | RuntimeHandleOwnerSlot::DialogueResult(_)
        ) | (
            RuntimeHandleOwnerSlot::ActivationLocal(_),
            RuntimeHandleOwnerSlot::ActivationLocal(_)
                | RuntimeHandleOwnerSlot::ChildScope(_)
                | RuntimeHandleOwnerSlot::DialogueResult(_)
        ) | (
            RuntimeHandleOwnerSlot::ChildScope(_),
            RuntimeHandleOwnerSlot::LineScope
        ) | (
            RuntimeHandleOwnerSlot::DialogueResult(_),
            RuntimeHandleOwnerSlot::ParentFiber(_)
        ) | (
            RuntimeHandleOwnerSlot::ParentFiber(_),
            RuntimeHandleOwnerSlot::ParentFiber(_)
        )
    )
}

const fn lease_transition_is_legal(
    source: RuntimeHandleLeaseState,
    destination: RuntimeHandleLeaseState,
) -> bool {
    matches!(
        (source, destination),
        (
            RuntimeHandleLeaseState::Allocating,
            RuntimeHandleLeaseState::Active
                | RuntimeHandleLeaseState::Cancelling
                | RuntimeHandleLeaseState::Failed
        ) | (
            RuntimeHandleLeaseState::Active,
            RuntimeHandleLeaseState::Cancelling
                | RuntimeHandleLeaseState::Completed
                | RuntimeHandleLeaseState::Failed
        ) | (
            RuntimeHandleLeaseState::Pending,
            RuntimeHandleLeaseState::Running
                | RuntimeHandleLeaseState::Cancelling
                | RuntimeHandleLeaseState::Cancelled
                | RuntimeHandleLeaseState::Failed
        ) | (
            RuntimeHandleLeaseState::Running,
            RuntimeHandleLeaseState::Completed
                | RuntimeHandleLeaseState::Cancelling
                | RuntimeHandleLeaseState::Cancelled
                | RuntimeHandleLeaseState::Failed
        ) | (
            RuntimeHandleLeaseState::Cancelling,
            RuntimeHandleLeaseState::Cancelled
                | RuntimeHandleLeaseState::Released
                | RuntimeHandleLeaseState::Failed
        ) | (
            RuntimeHandleLeaseState::Completed | RuntimeHandleLeaseState::Cancelled,
            RuntimeHandleLeaseState::Released
        ) | (
            RuntimeHandleLeaseState::Failed,
            RuntimeHandleLeaseState::Released
        )
    )
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeScheduledState {
    Armed,
    Running,
    Completed,
    Cancelling,
    Cancelled,
    Failed,
}

#[derive(Clone, Debug, PartialEq)]
enum RuntimeScheduledCaptureCustody {
    Packet(Box<[RuntimeLocalBinding]>),
    ChildFiber(Box<[RuntimeLocalDeclarationId]>),
    LineScope(Box<[RuntimeLocalBinding]>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeScheduledLineTask {
    token: RuntimeLineHandleToken,
    child: RuntimeLineTaskNodeId,
    work: LineTaskWorkTag,
    deadline: LogicalDuration,
    custody: RuntimeScheduledCaptureCustody,
    state: RuntimeScheduledState,
}

impl RuntimeScheduledLineTask {
    #[must_use]
    pub fn new(
        token: RuntimeLineHandleToken,
        child: RuntimeLineTaskNodeId,
        work: LineTaskWorkTag,
        deadline: LogicalDuration,
        captures: Box<[RuntimeLocalBinding]>,
    ) -> Result<Self, LineRuntimeError> {
        Self::try_from_parts(
            token,
            child,
            work,
            deadline,
            RuntimeScheduledCaptureCustody::Packet(captures),
            RuntimeScheduledState::Armed,
        )
    }

    #[must_use]
    pub const fn token(&self) -> &RuntimeLineHandleToken {
        &self.token
    }

    #[must_use]
    pub const fn child(&self) -> RuntimeLineTaskNodeId {
        self.child
    }

    #[must_use]
    pub const fn work(&self) -> &LineTaskWorkTag {
        &self.work
    }

    #[must_use]
    pub const fn deadline(&self) -> LogicalDuration {
        self.deadline
    }

    #[must_use]
    pub const fn state(&self) -> RuntimeScheduledState {
        self.state
    }

    pub(crate) fn take_packet_for_child_fiber(
        &mut self,
    ) -> Result<Box<[RuntimeLocalBinding]>, LineRuntimeError> {
        if self.state != RuntimeScheduledState::Running
            || !matches!(&self.custody, RuntimeScheduledCaptureCustody::Packet(_))
            || self.validate_custody().is_err()
        {
            return Err(LineRuntimeError::InvalidScheduledCaptureTransition);
        }

        let custody = std::mem::replace(
            &mut self.custody,
            RuntimeScheduledCaptureCustody::Packet(Vec::new().into_boxed_slice()),
        );
        match custody {
            RuntimeScheduledCaptureCustody::Packet(bindings) => {
                let locals = bindings
                    .iter()
                    .map(|binding| binding.local)
                    .collect::<Vec<_>>()
                    .into_boxed_slice();
                self.custody = RuntimeScheduledCaptureCustody::ChildFiber(locals);
                Ok(bindings)
            }
            custody => {
                self.custody = custody;
                Err(LineRuntimeError::InvalidScheduledCaptureTransition)
            }
        }
    }

    pub(crate) fn admit_child_fiber_bindings(
        &mut self,
        bindings: Box<[RuntimeLocalBinding]>,
        terminal: RuntimeScheduledState,
    ) -> Result<(), LineRuntimeError> {
        if !matches!(
            self.state,
            RuntimeScheduledState::Running | RuntimeScheduledState::Cancelling
        ) || !matches!(&self.custody, RuntimeScheduledCaptureCustody::ChildFiber(_))
            || self.validate_custody().is_err()
            || !is_terminal_scheduled_state(terminal)
            || !scheduled_transition_is_legal(self.state, terminal)
        {
            return Err(LineRuntimeError::InvalidScheduledCaptureTransition);
        }

        let original_locals = match &self.custody {
            RuntimeScheduledCaptureCustody::ChildFiber(locals) => locals.clone(),
            RuntimeScheduledCaptureCustody::Packet(_)
            | RuntimeScheduledCaptureCustody::LineScope(_) => {
                return Err(LineRuntimeError::InvalidScheduledCaptureTransition);
            }
        };
        let mut returned = BTreeMap::new();
        for binding in bindings {
            if returned.insert(binding.local, binding).is_some() {
                return Err(LineRuntimeError::InvalidScheduledCaptureTransition);
            }
        }
        if returned
            .keys()
            .any(|local| !original_locals.iter().any(|original| original == local))
        {
            return Err(LineRuntimeError::InvalidScheduledCaptureTransition);
        }
        let ordered = original_locals
            .iter()
            .filter_map(|local| returned.remove(local))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        self.custody = RuntimeScheduledCaptureCustody::LineScope(ordered);
        self.state = terminal;
        Ok(())
    }

    pub(crate) fn move_packet_to_line_scope(
        &mut self,
        terminal: RuntimeScheduledState,
    ) -> Result<(), LineRuntimeError> {
        if !matches!(&self.custody, RuntimeScheduledCaptureCustody::Packet(_))
            || !is_terminal_scheduled_state(terminal)
            || !scheduled_transition_is_legal(self.state, terminal)
            || self.validate_custody().is_err()
        {
            return Err(LineRuntimeError::InvalidScheduledCaptureTransition);
        }

        let custody = std::mem::replace(
            &mut self.custody,
            RuntimeScheduledCaptureCustody::Packet(Vec::new().into_boxed_slice()),
        );
        match custody {
            RuntimeScheduledCaptureCustody::Packet(bindings) => {
                self.custody = RuntimeScheduledCaptureCustody::LineScope(bindings);
                self.state = terminal;
                Ok(())
            }
            custody => {
                self.custody = custody;
                Err(LineRuntimeError::InvalidScheduledCaptureTransition)
            }
        }
    }

    pub(crate) fn line_scope_captures(&self) -> Result<&[RuntimeLocalBinding], LineRuntimeError> {
        self.validate_custody()?;
        match &self.custody {
            RuntimeScheduledCaptureCustody::LineScope(bindings) => Ok(bindings),
            RuntimeScheduledCaptureCustody::Packet(_)
            | RuntimeScheduledCaptureCustody::ChildFiber(_) => {
                Err(LineRuntimeError::InvalidScheduledCaptureTransition)
            }
        }
    }

    pub(crate) fn child_fiber_locals(
        &self,
    ) -> Result<&[RuntimeLocalDeclarationId], LineRuntimeError> {
        self.validate_custody()?;
        match &self.custody {
            RuntimeScheduledCaptureCustody::ChildFiber(locals) => Ok(locals),
            RuntimeScheduledCaptureCustody::Packet(_)
            | RuntimeScheduledCaptureCustody::LineScope(_) => {
                Err(LineRuntimeError::InvalidScheduledCaptureTransition)
            }
        }
    }

    fn require_line_scope(&self) -> Result<(), LineRuntimeError> {
        if matches!(&self.custody, RuntimeScheduledCaptureCustody::LineScope(_))
            && self.validate_custody().is_ok()
        {
            Ok(())
        } else {
            Err(LineRuntimeError::InvalidScheduledCaptureTransition)
        }
    }

    fn validate_custody(&self) -> Result<(), LineRuntimeError> {
        scheduled_custody_is_admissible(self.state, &self.custody)
            .then_some(())
            .ok_or(LineRuntimeError::InvalidScheduledCaptureTransition)
    }

    fn try_from_parts(
        token: RuntimeLineHandleToken,
        child: RuntimeLineTaskNodeId,
        work: LineTaskWorkTag,
        deadline: LogicalDuration,
        custody: RuntimeScheduledCaptureCustody,
        state: RuntimeScheduledState,
    ) -> Result<Self, LineRuntimeError> {
        if work.scheduled_token() != Some(&token)
            || !matches!(work.work(), super::LineTaskWork::Node(_))
        {
            return Err(LineRuntimeError::InvalidScheduledCaptureOwner);
        }
        let scheduled = Self {
            token,
            child,
            work,
            deadline,
            custody,
            state,
        };
        scheduled.validate_custody()?;
        Ok(scheduled)
    }

    pub(crate) fn transition(
        &mut self,
        expected: RuntimeScheduledState,
        state: RuntimeScheduledState,
    ) -> Result<(), LineRuntimeError> {
        if self.state != expected || !scheduled_transition_is_legal(expected, state) {
            return Err(LineRuntimeError::InvalidScheduledTransition {
                expected,
                actual: self.state,
            });
        }
        if self.validate_custody().is_err()
            || !scheduled_custody_is_admissible(state, &self.custody)
        {
            return Err(LineRuntimeError::InvalidScheduledCaptureTransition);
        }
        self.state = state;
        Ok(())
    }
}

fn scheduled_custody_is_admissible(
    state: RuntimeScheduledState,
    custody: &RuntimeScheduledCaptureCustody,
) -> bool {
    if !matches!(
        (state, custody),
        (
            RuntimeScheduledState::Armed,
            RuntimeScheduledCaptureCustody::Packet(_)
        ) | (
            RuntimeScheduledState::Running | RuntimeScheduledState::Cancelling,
            RuntimeScheduledCaptureCustody::Packet(_)
                | RuntimeScheduledCaptureCustody::ChildFiber(_)
        ) | (
            RuntimeScheduledState::Completed
                | RuntimeScheduledState::Cancelled
                | RuntimeScheduledState::Failed,
            RuntimeScheduledCaptureCustody::LineScope(_)
        )
    ) {
        return false;
    }
    custody_values_are_well_formed(custody)
}

const fn is_terminal_scheduled_state(state: RuntimeScheduledState) -> bool {
    matches!(
        state,
        RuntimeScheduledState::Completed
            | RuntimeScheduledState::Cancelled
            | RuntimeScheduledState::Failed
    )
}

fn custody_values_are_well_formed(custody: &RuntimeScheduledCaptureCustody) -> bool {
    match custody {
        RuntimeScheduledCaptureCustody::Packet(bindings)
        | RuntimeScheduledCaptureCustody::LineScope(bindings) => {
            local_bindings_are_unique(bindings)
        }
        RuntimeScheduledCaptureCustody::ChildFiber(locals) => {
            local_ids_are_unique(locals.iter().copied())
        }
    }
}

fn local_bindings_are_unique(bindings: &[RuntimeLocalBinding]) -> bool {
    local_ids_are_unique(bindings.iter().map(|binding| binding.local))
}

fn local_ids_are_unique<I>(locals: I) -> bool
where
    I: IntoIterator<Item = RuntimeLocalDeclarationId>,
{
    let mut seen = std::collections::BTreeSet::new();
    locals.into_iter().all(|local| seen.insert(local))
}

const fn scheduled_transition_is_legal(
    source: RuntimeScheduledState,
    destination: RuntimeScheduledState,
) -> bool {
    matches!(
        (source, destination),
        (
            RuntimeScheduledState::Armed,
            RuntimeScheduledState::Running
                | RuntimeScheduledState::Cancelling
                | RuntimeScheduledState::Cancelled
                | RuntimeScheduledState::Failed
        ) | (
            RuntimeScheduledState::Running,
            RuntimeScheduledState::Completed
                | RuntimeScheduledState::Cancelling
                | RuntimeScheduledState::Cancelled
                | RuntimeScheduledState::Failed
        ) | (
            RuntimeScheduledState::Cancelling,
            RuntimeScheduledState::Cancelled | RuntimeScheduledState::Failed
        )
    )
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum LineRuntimeError {
    #[error("runtime plan has no accepted artifact generation")]
    UnboundArtifact,
    #[error("dialogue occurrence identity overflowed")]
    DialogueOccurrenceOverflow,
    #[error("dialogue content has no result-producing line task group")]
    MissingTaskGroup,
    #[error("dialogue content references a missing line task group")]
    UnknownTaskGroup,
    #[error("dialogue activation references a missing content plan")]
    UnknownContentPlan,
    #[error("dialogue result target does not match its line task group")]
    DialogueResultTypeMismatch,
    #[error("line handle has the wrong opaque producer")]
    WrongOpaqueProducer,
    #[error("line handle payload is not the canonical token record")]
    InvalidHandlePayload,
    #[error("line handle generation is stale")]
    StaleGeneration,
    #[error("line handle belongs to a different activation")]
    WrongActivation,
    #[error("line handle site is invalid for its resource")]
    InvalidHandleSite,
    #[error("line handle token is duplicated")]
    DuplicateHandleToken,
    #[error("one affine line handle token occurs more than once in a runtime value graph")]
    DuplicateHandleOccurrence,
    #[error("line handle is not live in this activation")]
    UnknownHandle,
    #[error("line handle is owned by another runtime slot")]
    WrongOwner,
    #[error("line handle owner transition is not legal in the dialogue lifecycle")]
    IllegalOwnerTransition,
    #[error("line ownership references unknown runtime local declaration {local}")]
    UnknownOwnedLocal {
        local: crate::runtime_id::RuntimeLocalDeclarationId,
    },
    #[error("runtime environment slot identity overflowed")]
    OwnedSlotOverflow,
    #[error("line handle was already released")]
    ReleasedHandle,
    #[error("line handle lease transition expected {expected:?}, found {actual:?}")]
    InvalidLeaseTransition {
        expected: RuntimeHandleLeaseState,
        actual: RuntimeHandleLeaseState,
    },
    #[error("line handle cannot be dropped from its current resource state")]
    InvalidDropTransition,
    #[error("scheduled line task transition expected {expected:?}, found {actual:?}")]
    InvalidScheduledTransition {
        expected: RuntimeScheduledState,
        actual: RuntimeScheduledState,
    },
    #[error("scheduled capture custody transition is invalid")]
    InvalidScheduledCaptureTransition,
    #[error("stage actor belongs to a different Character")]
    WrongActorCharacter,
    #[error("look belongs to a different Character")]
    WrongLookOwner,
    #[error("active dialogue has no voice")]
    MissingActiveVoice,
    #[error("dialogue voice start was rejected: {failure:?}")]
    VoiceStartRejected {
        failure: crate::presentation::RuntimeVoiceFailure,
    },
    #[error("cue delay is negative")]
    NegativeCueDelay,
    #[error("cue delay is not a valid Duration")]
    InvalidCueDelay,
    #[error("cue delay or deadline overflowed")]
    CueDeadlineOverflow,
    #[error("dialogue logical elapsed time overflowed")]
    DialogueElapsedOverflow,
    #[error("line handle issuance ordinal overflowed")]
    HandleIssuanceOverflow,
    #[error("line activation program counter overflowed")]
    ActivationProgramCounterOverflow,
    #[error("line activation contains an operation outside its admitted role")]
    InvalidActivationOperation,
    #[error("stage command outcome does not match the pending typed operation")]
    StageOutcomeMismatch,
    #[error("actor look crossfade is not a valid Duration")]
    InvalidCrossfade,
    #[error("drop policy does not match its typed payload")]
    InvalidDropPolicy,
    #[error("scheduled line callback limit exceeded")]
    ScheduledCallbackLimitExceeded,
    #[error("scheduled callback does not have one exact child-scope capture owner")]
    InvalidScheduledCaptureOwner,
    #[error("scheduled callback work has no exact runtime packet")]
    MissingScheduledWork,
    #[error("scheduled callback runtime instance was already queued or activated")]
    DuplicateScheduledWorkInstance,
    #[error("scheduled callback work is not in a completable runtime state")]
    InvalidScheduledWorkState,
    #[error("scheduled callback capture graph violates its admitted ownership packet")]
    InvalidScheduledCaptureGraph,
    #[error("detached scheduled callback cannot capture an affine line handle")]
    DetachedAffineCapture,
    #[error("line-task group capture must be unrestricted because group work may fan out")]
    AffineGroupCapture,
    #[error("live line handle limit exceeded")]
    LiveHandleLimitExceeded,
    #[error(transparent)]
    CommandSequence(#[from] crate::presentation::RuntimeCommandQueueError),
    #[error("dialogue presentation command identity was issued twice")]
    DuplicateCommandIdentity,
    #[error("dialogue presentation command queue does not continue the activation sequence")]
    StaleCommandQueue,
    #[error("dialogue command history limit exceeded")]
    CommandHistoryLimitExceeded,
    #[error("dialogue transaction produced host commands at a non-publishing boundary")]
    UnexpectedPreparedCommands,
    #[error("host outcome does not belong to an issued command in this dialogue activation")]
    UnknownCommandOutcome,
    #[error("host outcome belongs to a stale dialogue activation")]
    StaleCommandOutcome,
    #[error("host outcome belongs to a command superseded by a later resource transaction")]
    SupersededCommandOutcome,
    #[error("host outcome repeats a command outcome already resolved by this activation")]
    DuplicateCommandOutcome,
    #[error(
        "dialogue content event belongs to a stale activation: expected {expected:?}, got {actual:?}"
    )]
    StaleContentEvent {
        expected: crate::runtime_id::DialogueActivationId,
        actual: crate::runtime_id::DialogueActivationId,
    },
    #[error(
        "dialogue content event coordinate is absent from the accepted content plan: {event:?}"
    )]
    UnknownContentEvent {
        event: crate::step::RuntimeDialogueContentEventKind,
    },
    #[error("dialogue content event repeats within one input batch: {event:?}")]
    DuplicateContentEvent {
        event: crate::step::RuntimeDialogueContentEventKind,
    },
    #[error("dialogue advance repeats for activation {activation:?}")]
    DuplicateDialogueAdvance {
        activation: crate::runtime_id::DialogueActivationId,
    },
    #[error("dialogue ingress targets stale activation {activation:?}")]
    StaleDialogueIngress {
        activation: crate::runtime_id::DialogueActivationId,
    },
    #[error("dialogue ingress targets activation {activation:?} outside its ready phase")]
    DialogueIngressNotReady {
        activation: crate::runtime_id::DialogueActivationId,
    },
    #[error("dialogue content event was already consumed by this activation: {event:?}")]
    ConsumedContentEvent {
        event: crate::step::RuntimeDialogueContentEventKind,
    },
    #[error("dialogue content event arrived outside a live line-task reducer: {event:?}")]
    ContentEventOutsideLiveLineTask {
        event: crate::step::RuntimeDialogueContentEventKind,
    },
    #[error("dialogue result was not committed")]
    ResultNotCommitted,
    #[error("dialogue result was committed twice")]
    ResultAlreadyCommitted,
    #[error("dialogue result failed its exact type or pattern")]
    ResultPatternOrTypeMismatch,
    #[error("dialogue result transaction cannot make this lifecycle transition")]
    InvalidResultTransition,
    #[error("dialogue activation frame was released twice")]
    DuplicateFrameRelease,
    #[error("dialogue publication left a live lease outside the parent-fiber owner")]
    UnownedLeaseAtPublish,
    #[error("parent fiber already owns a closed ledger for this dialogue activation")]
    DuplicateClosedLedger,
    #[error("dialogue activation registry already contains this activation")]
    DuplicateActivationLedger,
    #[error("dialogue activation registry does not contain this activation")]
    UnknownActivationLedger,
    #[error("dialogue activation frame has already published and only handle leases remain")]
    ActivationFrameReleased,
    #[error("parent-owned line handle was dropped before its dialogue result was published")]
    ParentHandleBeforePublication,
    #[error("parent AWBC execution introduced a line handle absent from its source fiber graph")]
    UnexpectedParentHandleOccurrence,
    #[error("line-task child execution introduced a line handle absent from its source graph")]
    UnexpectedChildHandleOccurrence,
    #[error(
        "affine line handle disappeared without the typed Drop instruction that owns its policy"
    )]
    UnjournaledHandleDrop,
    #[error("restored dialogue handle ledger violates activation/resource ownership invariants")]
    InvalidRestoredLedgerState,
    #[error("restored dialogue command journal is noncanonical or cross-activation")]
    InvalidRestoredCommandJournal,
    #[error("restored scheduled work is inconsistent with its cue resource or activation")]
    InvalidRestoredScheduledState,
    #[error("restored dialogue result is inconsistent with its ledger/frame phase")]
    InvalidRestoredResultState,
    #[error("dialogue activation transaction was superseded by a newer commit")]
    StaleActivationTransaction,
    #[error("dialogue activation transaction revision overflowed")]
    ActivationTransactionRevisionOverflow,
    #[error("dialogue activation transaction is not terminal")]
    ActivationTransactionNotTerminal,
    #[error("active dialogue transaction carries a terminal frame disposition")]
    UnexpectedTerminalDisposition,
    #[error("dialogue terminal disposition does not match the committed result state")]
    TerminalDispositionMismatch,
    #[error("host rejected a stage command: {code:?}")]
    StageCommandRejected { code: RuntimeStageRejectCode },
}

#[cfg(test)]
mod tests {
    use super::{
        AwbcRuntimeDialogueActivationSnapshot, LineRuntimeError, RuntimeCueLease, RuntimeCueOrigin,
        RuntimeDialogueActivationState, RuntimeHandleLease, RuntimeHandleLeaseState,
        RuntimeHandleOwnerSlot, RuntimeHandleResource, RuntimeScheduledLineTask,
    };
    use crate::line_task::{LineTaskWorkTag, RuntimeScheduledState};
    use crate::runtime_id::{
        DialogueActivationId, RuntimeDialogueContentPlanId, RuntimeLineHandleSiteId,
        RuntimeLineHandleToken, RuntimeLineTaskNodeId, RuntimePersistentFiberId, RuntimePlanTypeId,
    };
    use crate::time::LogicalDuration;
    use crate::value::RuntimeValue;
    use std::num::NonZeroU32;

    #[test]
    fn result_and_frame_transitions_are_typed_and_terminal() {
        let mut state = RuntimeDialogueActivationState::<RuntimePlanTypeId>::new();
        assert!(!state.is_terminal());
        state.abandon().expect("abandon uncommitted result");
        state.release_frame().expect("release frame");
        assert!(state.is_terminal());
        assert_eq!(
            state.release_frame(),
            Err(LineRuntimeError::DuplicateFrameRelease)
        );

        let mut published = RuntimeDialogueActivationState::new();
        published
            .commit_result(
                RuntimePlanTypeId::from_accepted_ordinal(NonZeroU32::MIN),
                RuntimeValue::Unit,
            )
            .expect("commit");
        published
            .begin_result_publication()
            .expect("begin publication");
        published
            .finish_result_publication()
            .expect("finish publication");
        assert_eq!(
            published.abandon(),
            Err(LineRuntimeError::InvalidResultTransition)
        );
    }

    fn scheduled_activation() -> DialogueActivationId {
        DialogueActivationId::new(
            crate::effect::RuntimeArtifactFingerprint::try_from_bytes([0x73; 32])
                .expect("artifact"),
            RuntimePersistentFiberId::from_allocated(1),
            RuntimeDialogueContentPlanId::from_accepted_ordinal(NonZeroU32::MIN),
            9,
        )
    }

    fn scheduled_state() -> (
        DialogueActivationId,
        RuntimeDialogueActivationState<RuntimePlanTypeId>,
    ) {
        let activation = scheduled_activation();
        let site = RuntimeLineHandleSiteId::from_zero_based(0);
        let child = RuntimeLineTaskNodeId::from_zero_based(1).expect("child");
        let action = RuntimeLineTaskNodeId::from_zero_based(2).expect("action");
        let deadline = LogicalDuration::from_nanos(7);
        let mut state = RuntimeDialogueActivationState::new();
        for issuance in 0..2 {
            let token = RuntimeLineHandleToken::new(activation.clone(), site, issuance);
            let work = LineTaskWorkTag::scheduled(token.clone(), action);
            state.ledger.leases.insert(
                token.clone(),
                RuntimeHandleLease {
                    token: token.clone(),
                    owner: RuntimeHandleOwnerSlot::LineScope,
                    state: RuntimeHandleLeaseState::Pending,
                    resource: RuntimeHandleResource::Cue(RuntimeCueLease::new(
                        RuntimeCueOrigin::Scheduled { child, deadline },
                    )),
                },
            );
            state.scheduled.push(
                RuntimeScheduledLineTask::new(token, child, work, deadline, Box::default())
                    .expect("scheduled packet"),
            );
        }
        state.ledger.issuance_by_site.insert(site, 2);
        (activation, state)
    }

    #[test]
    fn same_site_scheduled_packets_round_trip_by_exact_token() {
        let (activation, state) = scheduled_state();
        state
            .restore_admit(&activation)
            .expect("same-site issuance tokens are distinct packets");
        let snapshot = AwbcRuntimeDialogueActivationSnapshot::from_live(&state)
            .expect("snapshot exact packets");
        let restored = snapshot.into_live().expect("restore exact packets");
        restored
            .restore_admit(&activation)
            .expect("round-trip exact packets");
        assert_eq!(restored.scheduled.len(), 2);
        assert_ne!(restored.scheduled[0].token(), restored.scheduled[1].token());
        assert!(
            restored
                .scheduled
                .iter()
                .all(|scheduled| scheduled.state() == RuntimeScheduledState::Armed)
        );
    }

    #[test]
    fn restore_rejects_scheduled_cue_without_its_exact_packet() {
        let (activation, mut state) = scheduled_state();
        state.scheduled.pop();
        assert_eq!(
            state.restore_admit(&activation),
            Err(LineRuntimeError::InvalidRestoredScheduledState)
        );
    }

    #[test]
    fn restore_rejects_duplicate_scheduled_packet_token() {
        let (activation, mut state) = scheduled_state();
        state.scheduled.push(state.scheduled[0].clone());
        assert_eq!(
            state.restore_admit(&activation),
            Err(LineRuntimeError::InvalidRestoredScheduledState)
        );
    }

    #[test]
    fn failure_unwind_terminalizes_unstarted_scheduled_packets_exactly_once() {
        let (activation, mut state) = scheduled_state();
        state.abandon().expect("abandon result");

        state
            .prepare_handle_unwind(&activation, false)
            .expect("unstarted packets unwind");

        assert!(state.scheduled.iter().all(|scheduled| {
            scheduled.state() == RuntimeScheduledState::Cancelled
                && scheduled.line_scope_captures().is_ok()
        }));
        assert!(
            state
                .ledger
                .leases()
                .values()
                .all(|lease| { lease.state() == RuntimeHandleLeaseState::Released })
        );
        assert!(state.failure_close_ready());
    }

    #[test]
    fn failure_unwind_rejects_invalid_packet_without_mutation() {
        let (activation, mut state) = scheduled_state();
        let token = state.scheduled[0].token().clone();
        state
            .ledger
            .leases
            .get_mut(&token)
            .expect("fixture cue")
            .state = RuntimeHandleLeaseState::Active;
        let unchanged = state.clone();

        assert!(state.prepare_handle_unwind(&activation, false).is_err());
        assert_eq!(state, unchanged);
    }
}
