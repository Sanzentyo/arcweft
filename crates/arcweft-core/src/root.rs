//! Deterministic durable root-state transitions for stateful entries.
//!
//! Root work is a phase of the existing runtime step. This module owns the
//! transactional state boundary but performs no host I/O and dispatches no
//! command.

mod snapshot;
pub use snapshot::{RootStateSaveSnapshotError, RootStateSaveSnapshotV1};

use crate::entry::{
    EntryBindingIdentity, RootExecutionLimits, RuntimeCallableRole, RuntimeCommandContract,
    RuntimeSchemaLimits, RuntimeStatefulEntryRoles, RuntimeValueDigest, TypeLayoutHash,
    canonical_runtime_value_bytes,
};
use crate::pattern::{RuntimeBuiltinVariantCaseIdentity, RuntimeVariantIdentity};
use crate::plan::{
    EntryRuntimeId, FlowRuntimeId, RuntimeEntryRoles, RuntimePlan, RuntimePlanError,
};
use crate::value::{
    RuntimeFlowParameterBinding, RuntimePayload, RuntimeReductionProducer, RuntimeReductionValue,
    RuntimeScalarView, RuntimeValue, RuntimeValueView,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};
use std::fmt;
use thiserror::Error;

#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[repr(transparent)]
pub struct TransitionSequence(u64);

impl TransitionSequence {
    pub const ZERO: Self = Self(0);
    pub const TERMINAL: Self = Self(u64::MAX);

    #[must_use]
    pub const fn from_u64(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    fn next(self) -> Option<Self> {
        self.0.checked_add(1).map(Self)
    }
}

impl fmt::Display for TransitionSequence {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RootEventInput {
    pub payload: RuntimePayload,
}

impl RootEventInput {
    #[must_use]
    pub const fn new(payload: RuntimePayload) -> Self {
        Self { payload }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct SequencedRootEvent<P = RuntimePayload> {
    sequence: TransitionSequence,
    payload: P,
}

/// An admitted reducer invocation. Inputs move to the execution frame once;
/// the root keeps the committed state until that frame completes.
#[derive(Debug)]
pub struct RootReducerInvocation {
    callable: RuntimeCallableRole,
    args: Vec<RuntimeValue>,
}

impl RootReducerInvocation {
    #[must_use]
    pub fn into_parts(self) -> (RuntimeCallableRole, Vec<RuntimeValue>) {
        (self.callable, self.args)
    }
}

/// The root's execution phase and its sole committed state, when one exists.
/// Initializer execution never manufactures a placeholder state value.
#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "phase", rename_all = "snake_case", deny_unknown_fields)]
#[serde(bound(
    serialize = "P: crate::value::InertRuntimeValueStorage",
    deserialize = "P: crate::value::InertRuntimeValueStorage"
))]
pub enum RootPhase<P> {
    Initializing {
        flow: FlowRuntimeId,
        state_parameter: crate::entry::FlowParameterCoordinate,
    },
    Initialized {
        value: P,
        reducer: RootReducerPhase,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RootReducerPhase {
    Ready,
    Evaluating,
}

#[derive(Serialize)]
#[serde(tag = "phase", rename_all = "snake_case")]
enum RootPhaseHeader<'a> {
    Initializing {
        flow: &'a FlowRuntimeId,
        state_parameter: crate::entry::FlowParameterCoordinate,
    },
    Initialized {
        reducer: RootReducerPhase,
    },
}

impl<P> RootPhase<P> {
    #[must_use]
    pub const fn state_value(&self) -> Option<&P> {
        match self {
            Self::Initializing { .. } => None,
            Self::Initialized { value, .. } => Some(value),
        }
    }

    const fn header(&self) -> RootPhaseHeader<'_> {
        match self {
            Self::Initializing {
                flow,
                state_parameter,
            } => RootPhaseHeader::Initializing {
                flow,
                state_parameter: *state_parameter,
            },
            Self::Initialized { reducer, .. } => RootPhaseHeader::Initialized { reducer: *reducer },
        }
    }

    fn validate_initialization(
        &self,
        contract: &RootStartupContract,
        sequence: TransitionSequence,
    ) -> Result<(), RootRuntimeError> {
        if let Self::Initializing {
            flow,
            state_parameter,
        } = self
            && (flow != &contract.initial_flow
                || *state_parameter != contract.initial_state_parameter
                || sequence != TransitionSequence::ZERO)
        {
            return Err(RootRuntimeError::SnapshotRoleMismatch("initializer phase"));
        }
        Ok(())
    }

    fn try_map<Q, E>(self, map: impl FnOnce(P) -> Result<Q, E>) -> Result<RootPhase<Q>, E> {
        Ok(match self {
            Self::Initializing {
                flow,
                state_parameter,
            } => RootPhase::Initializing {
                flow,
                state_parameter,
            },
            Self::Initialized { value, reducer } => RootPhase::Initialized {
                value: map(value)?,
                reducer,
            },
        })
    }

    fn try_map_ref<Q, E>(&self, map: impl FnOnce(&P) -> Result<Q, E>) -> Result<RootPhase<Q>, E> {
        Ok(match self {
            Self::Initializing {
                flow,
                state_parameter,
            } => RootPhase::Initializing {
                flow: flow.clone(),
                state_parameter: *state_parameter,
            },
            Self::Initialized { value, reducer } => RootPhase::Initialized {
                value: map(value)?,
                reducer: *reducer,
            },
        })
    }
}

impl Clone for RootPhase<crate::value::AwbcRuntimeValueSnapshot> {
    fn clone(&self) -> Self {
        self.try_map_ref::<_, std::convert::Infallible>(|value| Ok(value.clone()))
            .unwrap_or_else(|never| match never {})
    }
}

pub use crate::value::RuntimeCommand;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimeCommandEnvelope<P = RuntimePayload> {
    pub transition: TransitionSequence,
    pub index: u32,
    pub command: RuntimeCommand<P>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum RootTransitionOutcome {
    Committed {
        sequence: TransitionSequence,
        state_digest: RuntimeValueDigest,
        command_digests: Vec<RuntimeValueDigest>,
    },
    Rejected {
        sequence: TransitionSequence,
        code: String,
        message: String,
        error_digest: RuntimeValueDigest,
    },
    Trapped {
        sequence: TransitionSequence,
        failure_digest: RuntimeValueDigest,
        message: String,
    },
}

impl RootTransitionOutcome {
    #[must_use]
    pub const fn sequence(&self) -> TransitionSequence {
        match self {
            Self::Committed { sequence, .. }
            | Self::Rejected { sequence, .. }
            | Self::Trapped { sequence, .. } => *sequence,
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct ActiveRootState<P = RuntimePayload> {
    pub entry: EntryRuntimeId,
    pub binding: EntryBindingIdentity,
    pub state_identity: crate::entry::RuntimeNominalTypeId,
    pub state_layout: TypeLayoutHash,
    pub event_identity: crate::entry::RuntimeNominalTypeId,
    pub event_layout: TypeLayoutHash,
    pub phase: RootPhase<P>,
    pub next_sequence: TransitionSequence,
    queued_events: VecDeque<SequencedRootEvent<P>>,
    committed_commands: VecDeque<RuntimeCommandEnvelope<P>>,
}

impl ActiveRootState {
    #[must_use]
    pub fn pending_event_count(&self) -> usize {
        self.queued_events.len()
    }

    #[must_use]
    pub fn pending_command_count(&self) -> usize {
        self.committed_commands.len()
    }

    #[must_use]
    pub const fn reducer_active(&self) -> bool {
        matches!(
            self.phase,
            RootPhase::Initialized {
                reducer: RootReducerPhase::Evaluating,
                ..
            }
        )
    }
}

/// Complete root storage. A live root has one execution owner.
///
/// ```compile_fail
/// use arcweft_core::root::RootRuntime;
/// fn copy(root: &RootRuntime) { let _: RootRuntime = root.clone(); }
/// ```
#[derive(Debug, PartialEq)]
pub struct RootRuntime<P = RuntimePayload> {
    active: ActiveRootState<P>,
    roles: RuntimeStatefulEntryRoles,
    reducer: RuntimeCallableRole,
    limits: RootExecutionLimits,
    failure: Option<RootRuntimeFailure>,
}

/// An in-memory rollback image of the complete root owner. All live payloads
/// are represented by inert value DTOs until the old runtime has been dropped.
pub type RootRuntimeRollbackImage = RootRuntime<crate::value::AwbcRuntimeValueSnapshot>;

impl Clone for ActiveRootState<crate::value::AwbcRuntimeValueSnapshot> {
    fn clone(&self) -> Self {
        Self {
            entry: self.entry.clone(),
            binding: self.binding,
            state_identity: self.state_identity.clone(),
            state_layout: self.state_layout,
            event_identity: self.event_identity.clone(),
            event_layout: self.event_layout,
            phase: self.phase.clone(),
            next_sequence: self.next_sequence,
            queued_events: self.queued_events.clone(),
            committed_commands: self.committed_commands.clone(),
        }
    }
}

impl Clone for RootRuntimeRollbackImage {
    fn clone(&self) -> Self {
        Self {
            active: self.active.clone(),
            roles: self.roles.clone(),
            reducer: self.reducer.clone(),
            limits: self.limits,
            failure: self.failure.clone(),
        }
    }
}

/// Fully verified metadata needed to construct one durable root transaction
/// owner without coupling it to a concrete executable tier.
#[derive(Clone, Debug, PartialEq)]
pub struct RootStartupContract {
    pub entry: EntryRuntimeId,
    pub roles: RuntimeStatefulEntryRoles,
    pub initial_flow: FlowRuntimeId,
    pub initial_state_parameter: crate::entry::FlowParameterCoordinate,
}

#[derive(Debug)]
pub struct RootStartup {
    pub root: RootRuntime,
    pub initial_flow: FlowRuntimeId,
    pub initial_state_binding: RuntimeFlowParameterBinding,
    pub initializer_state_digest: RuntimeValueDigest,
}

/// A rejected initializer retains its pending root and queued events.
#[derive(Debug, Error)]
#[error("{error}")]
pub struct RejectedRootInitialization {
    #[source]
    pub error: RootRuntimeError,
    pub root: RootRuntime,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RootStepResult {
    pub outcomes: Vec<RootTransitionOutcome>,
    pub commands: Vec<RuntimeCommandEnvelope>,
    pub stopped_after_rejection: bool,
    pub failed: bool,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum RootRuntimeFailure {
    #[error("root transition {sequence} failed: {message}")]
    Transition {
        sequence: TransitionSequence,
        message: String,
        digest: RuntimeValueDigest,
    },
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum RootRuntimeError {
    #[error(transparent)]
    InvalidPlan(#[from] RuntimePlanError),
    #[error("runtime entry `{0}` does not exist")]
    MissingEntry(String),
    #[error("runtime entry `{0}` is not stateful")]
    NotStateful(String),
    #[error("runtime entry `{0}` references a missing initializer helper")]
    MissingInitializer(String),
    #[error("root initialization is still pending")]
    InitializationPending,
    #[error("root initialization has already completed")]
    InitializationAlreadyCompleted,
    #[error("runtime entry `{0}` references a missing reducer helper")]
    MissingReducer(String),
    #[error("runtime entry `{0}` references missing initial-flow executable metadata")]
    MissingInitialFlowExecutable(String),
    #[error("entry initializer failed: {0}")]
    Initializer(#[source] RootCallableEvaluationError),
    #[error("root {role} role does not match the active program: {source}")]
    InvalidRole {
        role: &'static str,
        #[source]
        source: crate::program_types::RuntimeProgramTypeError,
    },
    #[error("entry initializer returned an invalid root value: {0}")]
    InvalidInitialValue(#[source] crate::program_types::RuntimeProgramTypeError),
    #[error("{phase} root state contains an affine value that root execution must copy")]
    AffineRootState { phase: &'static str },
    #[error("saved root metadata does not match the selected {0} role")]
    SnapshotRoleMismatch(&'static str),
    #[error("saved root value is invalid: {0}")]
    InvalidSnapshotValue(#[source] crate::program_types::RuntimeProgramTypeError),
    #[error("root-event batch is not valid: {0}")]
    InvalidEvent(#[source] crate::program_types::RuntimeProgramTypeError),
    #[error("root-event queue exceeds the selected runtime limit of {limit}")]
    EventQueueLimit { limit: u32 },
    #[error("root-event transition sequence is exhausted")]
    TransitionSequenceExhausted,
    #[error("committed root commands must be accepted by the driver before another step")]
    PendingCommandsAwaitingDispatch,
    #[error("a root reducer invocation is already active")]
    ReducerAlreadyActive,
    #[error("no root reducer invocation is active")]
    NoActiveReducer,
    #[error("root event sequence does not equal the transition cursor")]
    EventSequenceMismatch,
    #[error("driver command acknowledgement does not match the committed command prefix")]
    CommandAcknowledgementMismatch,
    #[error("root runtime has failed")]
    Failed,
}

/// Tier-neutral failure returned by the existing callable evaluator.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
#[error("{message}")]
pub struct RootCallableEvaluationError {
    message: String,
}

impl RootCallableEvaluationError {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// Existing structured or Product AWBC callable engine used by root
/// transactions. The root owner retains stable semantic identity and never a
/// tier-local dense function/helper index.
pub trait RootCallableEvaluator {
    fn evaluate_root_callable(
        &mut self,
        callable: &RuntimeCallableRole,
        args: Vec<RuntimeValue>,
    ) -> Result<RuntimeValue, RootCallableEvaluationError>;
}

impl RootStartupContract {
    fn validate_types(
        &self,
        program: crate::program_types::RuntimeProgramTypes<'_>,
    ) -> Result<(), RootRuntimeError> {
        for (name, role) in [("state", &self.roles.state), ("event", &self.roles.event)] {
            role.validate_for_program(program)
                .map_err(|source| RootRuntimeError::InvalidRole { role: name, source })?;
        }
        Ok(())
    }

    /// Resolves exact stateful-entry metadata from a verified structured plan.
    pub fn from_runtime_plan(
        plan: &RuntimePlan,
        entry_id: &EntryRuntimeId,
    ) -> Result<Self, RootRuntimeError> {
        plan.verify()?;
        Self::from_program(
            crate::program_types::RuntimeProgramTypes::Plan(plan),
            entry_id,
        )
    }

    /// Resolves the selected entry's exact startup ABI from either original inventory.
    pub fn from_program(
        program: crate::program_types::RuntimeProgramTypes<'_>,
        entry_id: &EntryRuntimeId,
    ) -> Result<Self, RootRuntimeError> {
        let entry = program
            .entry(entry_id)
            .ok_or_else(|| RootRuntimeError::MissingEntry(entry_id.canonical_label()))?;
        let RuntimeEntryRoles::Stateful(roles) = entry.roles() else {
            return Err(RootRuntimeError::NotStateful(entry_id.canonical_label()));
        };
        let missing = || RootRuntimeError::MissingInitialFlowExecutable(entry_id.canonical_label());
        let state_parameter = match program {
            crate::program_types::RuntimeProgramTypes::Plan(plan) => {
                plan.flow_executables()
                    .iter()
                    .find(|flow| {
                        flow.flow == roles.initial_flow.flow
                            && flow.contract == roles.initial_flow.contract
                    })
                    .ok_or_else(missing)?;
                let schema = plan
                    .flow_schemas()
                    .iter()
                    .find(|schema| schema.flow == roles.initial_flow.flow)
                    .ok_or_else(missing)?;
                let [parameter] = schema.parameters.as_slice() else {
                    return Err(missing());
                };
                parameter.coordinate
            }
            crate::program_types::RuntimeProgramTypes::Awbc(program) => {
                let executable = program
                    .flow_executables
                    .iter()
                    .find(|flow| {
                        flow.metadata.flow == roles.initial_flow.flow
                            && flow.metadata.contract == roles.initial_flow.contract
                    })
                    .ok_or_else(missing)?;
                let function = program
                    .functions
                    .get(executable.function.index())
                    .ok_or_else(missing)?;
                let signature = program
                    .signatures
                    .get(function.signature.index())
                    .ok_or_else(missing)?;
                if signature.params.len() != 1 {
                    return Err(missing());
                }
                crate::entry::FlowParameterCoordinate::from_position(0)
            }
        };
        Ok(Self {
            entry: entry_id.clone(),
            roles: roles.as_ref().clone(),
            initial_flow: roles.initial_flow.flow.clone(),
            initial_state_parameter: state_parameter,
        })
    }
}

impl<P> RootRuntime<P> {
    /// Borrows the complete retained root before capture or restoration.
    pub(crate) fn admit_retained(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError>
    where
        P: crate::value::RuntimeStoredValue,
    {
        admission.status()?;
        self.admit_retained_inner(admission)
            .inspect_err(|error| admission.retain_failure(error.clone()))
    }

    fn admit_retained_inner(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError>
    where
        P: crate::value::RuntimeStoredValue,
    {
        use crate::value::{AwbcRuntimeValueSnapshotError as Error, RuntimeRetainedValueAdmission};
        let invalid = |message| Error::Message { message };
        admission.validate_metadata(&(
            &self.active.entry,
            self.active.binding,
            &self.active.state_identity,
            self.active.state_layout,
            &self.active.event_identity,
            self.active.event_layout,
            self.active.next_sequence,
            self.active.phase.header(),
            &self.roles,
            &self.reducer,
            self.limits,
        ))?;
        if let Some(RootRuntimeFailure::Transition {
            sequence,
            message,
            digest,
        }) = &self.failure
        {
            admission.validate_metadata(&(sequence, message, digest))?;
        }
        admission.validate_metadata_collection(self.active.queued_events.len())?;
        admission.validate_metadata_collection(self.active.committed_commands.len())?;
        let owner = admission
            .program_owner()
            .cloned()
            .ok_or_else(|| invalid("root admission requires its original program".to_owned()))?;
        let entry = owner
            .types()
            .entry(&self.active.entry)
            .ok_or_else(|| invalid("root entry is absent from its original program".to_owned()))?;
        let roles = entry
            .roles()
            .stateful()
            .ok_or_else(|| invalid("root entry is not stateful".to_owned()))?;
        if entry.binding() != self.active.binding
            || roles.binding != self.active.binding
            || &self.roles != roles
            || self.reducer != roles.reducer
            || self.limits != roles.command_policy.root_limits
            || !self.limits.is_valid()
        {
            return Err(invalid(
                "root metadata differs from its original selected entry".to_owned(),
            ));
        }
        RootStateSaveSnapshotV1::validate_header(
            &self.active.state_identity,
            self.active.state_layout,
            &self.active.event_identity,
            self.active.event_layout,
            roles,
        )
        .map_err(|error| invalid(error.to_string()))?;
        roles.state.validate_for_program(owner.types())?;
        roles.event.validate_for_program(owner.types())?;
        if matches!(self.active.phase, RootPhase::Initializing { .. }) {
            let contract = RootStartupContract::from_program(owner.types(), &self.active.entry)
                .map_err(|error| invalid(error.to_string()))?;
            self.active
                .phase
                .validate_initialization(&contract, self.active.next_sequence)
                .map_err(|error| invalid(error.to_string()))?;
            if !self.active.committed_commands.is_empty() || self.failure.is_some() {
                return Err(invalid(
                    "uninitialized root contains a completed transition".to_owned(),
                ));
            }
        }
        if !self
            .limits
            .permits_pending_events(self.active.queued_events.len())
            || !self
                .limits
                .permits_pending_commands(self.active.committed_commands.len())
        {
            return Err(invalid(
                "root retained queue exceeds its selected entry limit".to_owned(),
            ));
        }
        let mut next = self
            .active
            .next_sequence
            .get()
            .checked_add(u64::from(matches!(
                self.active.phase,
                RootPhase::Initialized {
                    reducer: RootReducerPhase::Evaluating,
                    ..
                }
            )));
        for event in &self.active.queued_events {
            admission.validate_metadata(&event.sequence)?;
            if next != Some(event.sequence.get()) || event.sequence == TransitionSequence::TERMINAL
            {
                return Err(invalid(
                    "root event queue differs from its transition frontier".to_owned(),
                ));
            }
            next = event.sequence.get().checked_add(1);
        }
        let mut previous: Option<(TransitionSequence, u32)> = None;
        for envelope in &self.active.committed_commands {
            admission.validate_metadata(&(
                envelope.transition,
                envelope.index,
                envelope.command.constructor(),
                envelope.command.target(),
            ))?;
            if envelope.transition >= self.active.next_sequence
                || envelope.index >= self.limits.max_commands_per_transition
                || previous.is_some_and(|(transition, index)| {
                    envelope.transition < transition
                        || if envelope.transition == transition {
                            index.checked_add(1) != Some(envelope.index)
                        } else {
                            envelope.index != 0
                        }
                })
            {
                return Err(invalid(
                    "root committed command queue has an invalid transition order".to_owned(),
                ));
            }
            previous = Some((envelope.transition, envelope.index));
        }
        self.admit_values(roles, admission)?;
        let mut entry_policy =
            RuntimeRetainedValueAdmission::for_program(owner.clone(), self.limits.schema);
        self.admit_values(roles, &mut entry_policy)
            .inspect_err(|error| admission.retain_failure(error.clone()))
    }

    fn admit_values(
        &self,
        roles: &RuntimeStatefulEntryRoles,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError>
    where
        P: crate::value::RuntimeStoredValue,
    {
        admission.admit_composite(P::admission_origin(), |owner, resources| {
            let owner =
                owner.ok_or_else(|| crate::value::AwbcRuntimeValueSnapshotError::Message {
                    message: "root requires its original program".to_owned(),
                })?;
            if let Some(value) = self.active.phase.state_value() {
                let state = value.borrowed_view();
                state.admit_ownership(
                    crate::value::ownership::RuntimeValueOwnershipRequirement::Unrestricted,
                    resources,
                    0,
                )?;
                owner
                    .types()
                    .value_type(roles.state.semantic_identity)?
                    .validate_snapshot_storage(state, &resources.work(), P::admission_origin())?;
                crate::entry::schema::value_encoding::visit_admitted_storage(
                    state,
                    &resources.work(),
                    validate_replay_safe_node,
                )?;
                crate::value::AwbcRuntimeValueSnapshot::validate_admitted_storage_view(
                    state,
                    Some(owner),
                    resources,
                )?;
            }
            for event in &self.active.queued_events {
                let value = event.payload.borrowed_view();
                resources.validate_view(value, 0, None)?;
                owner
                    .types()
                    .value_type(roles.event.semantic_identity)?
                    .validate_snapshot_storage(value, &resources.work(), P::admission_origin())?;
                crate::value::AwbcRuntimeValueSnapshot::validate_admitted_storage_view(
                    value,
                    Some(owner),
                    resources,
                )?;
            }
            let mut transition = None;
            let mut command_bytes = 0_usize;
            for envelope in &self.active.committed_commands {
                let command = &envelope.command;
                let value = command.payload().borrowed_view();
                value.admit_ownership(
                    crate::value::ownership::RuntimeValueOwnershipRequirement::Unrestricted,
                    resources,
                    0,
                )?;
                crate::value::AwbcRuntimeValueSnapshot::validate_admitted_storage_view(
                    value,
                    Some(owner),
                    resources,
                )?;
                let contract = roles
                    .command_policy
                    .admitted
                    .iter()
                    .find(|contract| {
                        &contract.constructor == command.constructor()
                            && &contract.target == command.target()
                    })
                    .ok_or_else(|| crate::value::AwbcRuntimeValueSnapshotError::Message {
                        message: "root command is absent from its original selected policy"
                            .to_owned(),
                    })?;
                contract
                    .payload_schema
                    .validate_admitted_view(value, resources)?;
                crate::entry::schema::value_encoding::visit_admitted_storage(
                    value,
                    &resources.work(),
                    validate_replay_safe_node,
                )?;
                if transition != Some(envelope.transition) {
                    transition = Some(envelope.transition);
                    command_bytes = 0;
                }
                let encoded =
                    canonical_command_bytes(command, self.limits.schema.platform_encoded_bytes())
                        .map_err(
                        |message| crate::value::AwbcRuntimeValueSnapshotError::Message { message },
                    )?;
                command_bytes = command_bytes.checked_add(encoded.len()).ok_or_else(|| {
                    crate::value::AwbcRuntimeValueSnapshotError::Message {
                        message: "root command byte count overflows usize".to_owned(),
                    }
                })?;
                if !self.limits.permits_transition_command_bytes(command_bytes) {
                    return Err(crate::value::AwbcRuntimeValueSnapshotError::Message {
                        message: "root command bytes exceed the original per-transition limit"
                            .to_owned(),
                    });
                }
            }
            Ok(())
        })
    }
}

impl RootRuntime {
    pub fn inert_rollback_image(
        &self,
        owner: &crate::task::RuntimeProgramOwner,
    ) -> Result<RootRuntimeRollbackImage, String> {
        self.admit_retained(
            &mut crate::value::RuntimeRetainedValueAdmission::for_program(
                owner.clone(),
                RuntimeSchemaLimits::engine_default(),
            ),
        )
        .map_err(|error| error.to_string())?;
        self.snapshot_admitted(owner)
    }

    pub(crate) fn snapshot_admitted(
        &self,
        owner: &crate::task::RuntimeProgramOwner,
    ) -> Result<RootRuntimeRollbackImage, String> {
        let image = |value: &crate::value::RuntimeValue| {
            crate::value::AwbcRuntimeValueSnapshot::from_admitted_runtime_value(value, owner)
                .map_err(|error| error.to_string())
        };
        Ok(RootRuntimeRollbackImage {
            active: ActiveRootState {
                entry: self.active.entry.clone(),
                binding: self.active.binding,
                state_identity: self.active.state_identity.clone(),
                state_layout: self.active.state_layout,
                event_identity: self.active.event_identity.clone(),
                event_layout: self.active.event_layout,
                phase: self
                    .active
                    .phase
                    .try_map_ref(|payload| image(payload.value()))?,
                next_sequence: self.active.next_sequence,
                queued_events: self
                    .active
                    .queued_events
                    .iter()
                    .map(|event| {
                        Ok(SequencedRootEvent {
                            sequence: event.sequence,
                            payload: image(event.payload.value())?,
                        })
                    })
                    .collect::<Result<_, String>>()?,
                committed_commands: self
                    .active
                    .committed_commands
                    .iter()
                    .map(|envelope| {
                        Ok(RuntimeCommandEnvelope {
                            transition: envelope.transition,
                            index: envelope.index,
                            command: RuntimeCommand::new_accepted(
                                envelope.command.constructor().clone(),
                                envelope.command.target().clone(),
                                image(envelope.command.payload().value())?,
                            ),
                        })
                    })
                    .collect::<Result<_, String>>()?,
            },
            roles: self.roles.clone(),
            reducer: self.reducer.clone(),
            limits: self.limits,
            failure: self.failure.clone(),
        })
    }

    pub fn from_rollback_image(
        image: RootRuntimeRollbackImage,
        owner: &crate::task::RuntimeProgramOwner,
    ) -> Result<Self, String> {
        image
            .admit_retained(
                &mut crate::value::RuntimeRetainedValueAdmission::for_program(
                    owner.clone(),
                    RuntimeSchemaLimits::engine_default(),
                ),
            )
            .map_err(|error| error.to_string())?;
        Self::from_admitted_rollback_image(image, owner)
    }

    pub(crate) fn from_admitted_rollback_image(
        image: RootRuntimeRollbackImage,
        owner: &crate::task::RuntimeProgramOwner,
    ) -> Result<Self, String> {
        let RootRuntime {
            active: image,
            roles,
            reducer,
            limits,
            failure,
        } = image;
        let value = |image: crate::value::AwbcRuntimeValueSnapshot| {
            image
                .into_admitted_runtime_value(owner)
                .map_err(|error| error.to_string())
        };
        Ok(Self {
            active: ActiveRootState {
                entry: image.entry,
                binding: image.binding,
                state_identity: image.state_identity,
                state_layout: image.state_layout,
                event_identity: image.event_identity,
                event_layout: image.event_layout,
                phase: image
                    .phase
                    .try_map(|image| value(image).map(RuntimePayload))?,
                next_sequence: image.next_sequence,
                queued_events: image
                    .queued_events
                    .into_iter()
                    .map(|event| {
                        Ok(SequencedRootEvent {
                            sequence: event.sequence,
                            payload: RuntimePayload(value(event.payload)?),
                        })
                    })
                    .collect::<Result<_, String>>()?,
                committed_commands: image
                    .committed_commands
                    .into_iter()
                    .map(|envelope| {
                        let (constructor, target, payload) = envelope.command.into_parts();
                        Ok(RuntimeCommandEnvelope {
                            transition: envelope.transition,
                            index: envelope.index,
                            command: RuntimeCommand::new_accepted(
                                constructor,
                                target,
                                RuntimePayload(value(payload)?),
                            ),
                        })
                    })
                    .collect::<Result<_, String>>()?,
            },
            roles,
            reducer,
            limits,
            failure,
        })
    }
    /// Admits startup metadata without executing the initializer or creating state.
    pub fn prepare(
        contract: RootStartupContract,
        program: crate::program_types::RuntimeProgramTypes<'_>,
    ) -> Result<Self, RootRuntimeError> {
        contract.validate_types(program)?;
        let RootStartupContract {
            entry,
            roles,
            initial_flow,
            initial_state_parameter,
        } = contract;
        let limits = roles.command_policy.root_limits;
        let active = ActiveRootState {
            entry,
            binding: roles.binding,
            state_identity: roles.state.identity.clone(),
            state_layout: roles.state.layout,
            event_identity: roles.event.identity.clone(),
            event_layout: roles.event.layout,
            phase: RootPhase::Initializing {
                flow: initial_flow,
                state_parameter: initial_state_parameter,
            },
            next_sequence: TransitionSequence::ZERO,
            queued_events: VecDeque::new(),
            committed_commands: VecDeque::new(),
        };
        Ok(Self {
            active,
            reducer: roles.reducer.clone(),
            roles,
            limits,
            failure: None,
        })
    }

    /// The role whose ordinary executable frame initializes this root.
    pub fn initializer_callable(&self) -> Result<&RuntimeCallableRole, RootRuntimeError> {
        if matches!(self.active.phase, RootPhase::Initializing { .. }) {
            Ok(&self.roles.initializer)
        } else {
            Err(RootRuntimeError::InitializationAlreadyCompleted)
        }
    }

    /// Constructs root/flow candidates after the initializer frame returns.
    /// The executable owner publishes both only after its flow constructor succeeds.
    pub fn complete_initialization(
        mut self,
        value: RuntimeValue,
        program: crate::program_types::RuntimeProgramTypes<'_>,
    ) -> Result<RootStartup, Box<RejectedRootInitialization>> {
        let payload = RuntimePayload(value);
        let prepared = (|| {
            let RootPhase::Initializing {
                flow,
                state_parameter,
            } = &self.active.phase
            else {
                return Err(RootRuntimeError::InitializationAlreadyCompleted);
            };
            if !payload.0.ownership().permits_copy() {
                return Err(RootRuntimeError::AffineRootState {
                    phase: "initializer",
                });
            }
            let digest = self
                .roles
                .state
                .accepts_payload(program, &payload, self.limits.schema)
                .map_err(RootRuntimeError::InvalidInitialValue)?;
            Ok((flow.clone(), *state_parameter, digest))
        })();
        let (initial_flow, initial_state_parameter, initializer_state_digest) = match prepared {
            Ok(prepared) => prepared,
            Err(error) => {
                return Err(Box::new(RejectedRootInitialization { error, root: self }));
            }
        };
        let flow_value = payload.0.clone();
        self.active.phase = RootPhase::Initialized {
            value: payload,
            reducer: RootReducerPhase::Ready,
        };
        Ok(RootStartup {
            initial_flow,
            initial_state_binding: RuntimeFlowParameterBinding {
                parameter: initial_state_parameter,
                value: flow_value,
            },
            root: self,
            initializer_state_digest,
        })
    }

    pub fn start(
        contract: RootStartupContract,
        evaluator: &mut impl RootCallableEvaluator,
        program: crate::program_types::RuntimeProgramTypes<'_>,
    ) -> Result<RootStartup, RootRuntimeError> {
        let root = Self::prepare(contract, program)?;
        let value = evaluator
            .evaluate_root_callable(root.initializer_callable()?, Vec::new())
            .map_err(RootRuntimeError::Initializer)?;
        root.complete_initialization(value, program)
            .map_err(|rejected| rejected.error)
    }

    #[must_use]
    pub const fn active(&self) -> &ActiveRootState {
        &self.active
    }

    /// Visits the sole live root state, queued events and committed commands without
    /// manufacturing another owner. Save and rollback admission use this view.
    pub fn visit_runtime_values(&self, mut visitor: impl FnMut(&crate::value::RuntimeValue)) {
        if let Some(value) = self.active.phase.state_value() {
            visitor(value.value());
        }
        for event in &self.active.queued_events {
            visitor(event.payload.value());
        }
        for envelope in &self.active.committed_commands {
            visitor(envelope.command.payload().value());
        }
    }

    #[must_use]
    pub const fn roles(&self) -> &RuntimeStatefulEntryRoles {
        &self.roles
    }

    #[must_use]
    pub const fn failure(&self) -> Option<&RootRuntimeFailure> {
        self.failure.as_ref()
    }

    #[must_use]
    pub fn snapshot_state(&self) -> RootStateSnapshotV1 {
        let phase = self
            .active
            .phase
            .try_map_ref::<_, std::convert::Infallible>(|value| {
                assert!(
                    value.0.ownership().permits_copy(),
                    "admitted root state must remain recursively unrestricted"
                );
                Ok(value.clone())
            })
            .unwrap_or_else(|never| match never {});
        RootStateSnapshotV1 {
            state_identity: self.active.state_identity.clone(),
            state_layout: self.active.state_layout,
            event_identity: self.active.event_identity.clone(),
            event_layout: self.active.event_layout,
            phase,
            next_sequence: self.active.next_sequence,
        }
    }

    #[must_use]
    pub fn save_blockers(&self) -> RootSaveBlockers {
        RootSaveBlockers {
            reducer_active: self.active.reducer_active(),
            pending_events: u32::try_from(self.active.queued_events.len()).unwrap_or(u32::MAX),
            pending_commands: u32::try_from(self.active.committed_commands.len())
                .unwrap_or(u32::MAX),
        }
    }

    /// Constructs a candidate root from the final session payload after exact
    /// selected-entry metadata validation. No live runtime is mutated here.
    pub fn from_snapshot(
        contract: RootStartupContract,
        snapshot: RootStateSnapshotV1,
        program: crate::program_types::RuntimeProgramTypes<'_>,
    ) -> Result<Self, RootRuntimeError> {
        contract.validate_types(program)?;
        snapshot
            .phase
            .validate_initialization(&contract, snapshot.next_sequence)?;
        let RootStartupContract {
            entry,
            roles,
            initial_flow: _,
            initial_state_parameter: _,
        } = contract;
        if !roles
            .state
            .matches_snapshot_header(&snapshot.state_identity, snapshot.state_layout)
        {
            return Err(RootRuntimeError::SnapshotRoleMismatch("state"));
        }
        if !roles
            .event
            .matches_snapshot_header(&snapshot.event_identity, snapshot.event_layout)
        {
            return Err(RootRuntimeError::SnapshotRoleMismatch("event"));
        }
        let limits = roles.command_policy.root_limits;
        if let RootPhase::Initialized { value, reducer } = &snapshot.phase {
            if *reducer != RootReducerPhase::Ready {
                return Err(RootRuntimeError::SnapshotRoleMismatch("active reducer"));
            }
            if !value.0.ownership().permits_copy() {
                return Err(RootRuntimeError::AffineRootState { phase: "saved" });
            }
            roles
                .state
                .accepts_payload(program, value, limits.schema)
                .map_err(RootRuntimeError::InvalidSnapshotValue)?;
        }
        Ok(Self {
            active: ActiveRootState {
                entry,
                binding: roles.binding,
                state_identity: snapshot.state_identity,
                state_layout: snapshot.state_layout,
                event_identity: snapshot.event_identity,
                event_layout: snapshot.event_layout,
                phase: snapshot.phase,
                next_sequence: snapshot.next_sequence,
                queued_events: VecDeque::new(),
                committed_commands: VecDeque::new(),
            },
            reducer: roles.reducer.clone(),
            roles,
            limits,
            failure: None,
        })
    }

    pub fn step(
        &mut self,
        events: Vec<RootEventInput>,
        evaluator: &mut impl RootCallableEvaluator,
        program: crate::program_types::RuntimeProgramTypes<'_>,
    ) -> Result<RootStepResult, RootRuntimeError> {
        if !self.active.committed_commands.is_empty() {
            return Err(RootRuntimeError::PendingCommandsAwaitingDispatch);
        }
        if self.active.reducer_active() {
            return Err(RootRuntimeError::ReducerAlreadyActive);
        }
        self.ingress(events, program)?;
        let mut result = RootStepResult::default();
        while let Some(invocation) = self.begin_reduction()? {
            let (callable, args) = invocation.into_parts();
            let returned = evaluator.evaluate_root_callable(&callable, args);
            let completed = self.complete_reduction(returned, program)?;
            result.stopped_after_rejection =
                matches!(completed, RootTransitionOutcome::Rejected { .. });
            result.failed = matches!(completed, RootTransitionOutcome::Trapped { .. });
            result.outcomes.push(completed);
            if result.stopped_after_rejection || result.failed {
                break;
            }
        }
        result
            .commands
            .extend(self.active.committed_commands.iter().cloned());
        Ok(result)
    }

    /// Acknowledges the exact committed prefix after the existing driver has
    /// accepted it into its dispatch/result boundary.
    pub fn acknowledge_published_commands(
        &mut self,
        accepted: &[RuntimeCommandEnvelope],
    ) -> Result<(), RootRuntimeError> {
        if accepted.len() > self.active.committed_commands.len()
            || self
                .active
                .committed_commands
                .iter()
                .zip(accepted)
                .any(|(pending, accepted)| pending != accepted)
        {
            return Err(RootRuntimeError::CommandAcknowledgementMismatch);
        }
        self.active.committed_commands.drain(..accepted.len());
        Ok(())
    }

    pub fn ingress(
        &mut self,
        events: Vec<RootEventInput>,
        program: crate::program_types::RuntimeProgramTypes<'_>,
    ) -> Result<(), RootRuntimeError> {
        if self.failure.is_some() {
            return Err(RootRuntimeError::Failed);
        }
        if events.is_empty() {
            return Ok(());
        }
        let pending_events = self
            .active
            .queued_events
            .len()
            .checked_add(events.len())
            .ok_or(RootRuntimeError::EventQueueLimit {
                limit: self.limits.max_pending_events,
            })?;
        if !self.limits.permits_pending_events(pending_events) {
            return Err(RootRuntimeError::EventQueueLimit {
                limit: self.limits.max_pending_events,
            });
        }
        for event in &events {
            self.roles
                .event
                .accepts_live_payload(program, &event.payload, self.limits.schema)
                .map_err(RootRuntimeError::InvalidEvent)?;
        }
        let first = match self.active.queued_events.back() {
            Some(event) => event
                .sequence
                .next()
                .ok_or(RootRuntimeError::TransitionSequenceExhausted)?,
            None if self.active.reducer_active() => self
                .active
                .next_sequence
                .next()
                .ok_or(RootRuntimeError::TransitionSequenceExhausted)?,
            None => self.active.next_sequence,
        };
        let last_offset = u64::try_from(events.len() - 1)
            .map_err(|_| RootRuntimeError::TransitionSequenceExhausted)?;
        let last = first
            .get()
            .checked_add(last_offset)
            .ok_or(RootRuntimeError::TransitionSequenceExhausted)?;
        if last == u64::MAX {
            return Err(RootRuntimeError::TransitionSequenceExhausted);
        }
        let sequenced = events
            .into_iter()
            .enumerate()
            .map(|(index, event)| {
                let index = u64::try_from(index)
                    .map_err(|_| RootRuntimeError::TransitionSequenceExhausted)?;
                let sequence = first
                    .get()
                    .checked_add(index)
                    .ok_or(RootRuntimeError::TransitionSequenceExhausted)?;
                Ok(SequencedRootEvent {
                    sequence: TransitionSequence::from_u64(sequence),
                    payload: event.payload,
                })
            })
            .collect::<Result<Vec<_>, RootRuntimeError>>()?;
        self.active.queued_events.extend(sequenced);
        Ok(())
    }

    /// Moves the next admitted event into inputs for its executable frame.
    /// No callable is evaluated and the committed state remains unchanged.
    /// A yielded frame must complete before another invocation can begin.
    pub fn begin_reduction(&mut self) -> Result<Option<RootReducerInvocation>, RootRuntimeError> {
        if self.failure.is_some() {
            return Err(RootRuntimeError::Failed);
        }
        if self.active.reducer_active() {
            return Err(RootRuntimeError::ReducerAlreadyActive);
        }
        let RootPhase::Initialized { value, reducer } = &mut self.active.phase else {
            return Err(RootRuntimeError::InitializationPending);
        };
        if self
            .active
            .queued_events
            .front()
            .is_some_and(|event| event.sequence != self.active.next_sequence)
        {
            return Err(RootRuntimeError::EventSequenceMismatch);
        }
        let Some(event) = self.active.queued_events.pop_front() else {
            return Ok(None);
        };
        *reducer = RootReducerPhase::Evaluating;
        Ok(Some(RootReducerInvocation {
            callable: self.reducer.clone(),
            args: vec![value.0.clone(), event.payload.0],
        }))
    }

    /// Admits a complete reducer result before publishing any state or command.
    /// Evaluation failures and invalid results retain the old committed state.
    pub fn complete_reduction(
        &mut self,
        returned: Result<RuntimeValue, RootCallableEvaluationError>,
        program: crate::program_types::RuntimeProgramTypes<'_>,
    ) -> Result<RootTransitionOutcome, RootRuntimeError> {
        if self.failure.is_some() {
            return Err(RootRuntimeError::Failed);
        }
        if !self.active.reducer_active() {
            return Err(RootRuntimeError::NoActiveReducer);
        }
        let sequence = self.active.next_sequence;
        let disposition = returned
            .map_err(|error| Self::failure_for(sequence, &error.to_string()))
            .and_then(|value| {
                parse_reducer_result(value).map_err(|message| Self::failure_for(sequence, &message))
            })
            .and_then(|returned| match returned {
                ParsedReducerResult::Committed(reduction) => {
                    self.commit_reduction(sequence, reduction, program)
                }
                ParsedReducerResult::Rejected {
                    code,
                    message,
                    value,
                } => self.reject_reduction(sequence, code, message, value),
            });
        if let RootPhase::Initialized { reducer, .. } = &mut self.active.phase {
            *reducer = RootReducerPhase::Ready;
        }
        Ok(match disposition {
            Ok(outcome) => outcome,
            Err(failure) => {
                let RootRuntimeFailure::Transition {
                    sequence,
                    message,
                    digest,
                } = &failure;
                let outcome = RootTransitionOutcome::Trapped {
                    sequence: *sequence,
                    failure_digest: *digest,
                    message: message.clone(),
                };
                self.active.queued_events.clear();
                self.failure = Some(failure);
                outcome
            }
        })
    }

    fn commit_reduction(
        &mut self,
        sequence: TransitionSequence,
        reduction: RuntimeReductionValue,
        program: crate::program_types::RuntimeProgramTypes<'_>,
    ) -> Result<RootTransitionOutcome, RootRuntimeFailure> {
        let (state, commands) = reduction.into_parts();
        let commands = commands.into_vec();
        let state = RuntimePayload(state);
        if !state.0.ownership().permits_copy() {
            return Err(Self::failure_for(
                sequence,
                "reducer state contains an affine value that root execution must copy",
            ));
        }
        let state_digest = self
            .roles
            .state
            .accepts_payload(program, &state, self.limits.schema)
            .map_err(|error| Self::failure_for(sequence, &error.to_string()))?;
        let command_digests =
            validate_commands(&commands, &self.roles.command_policy.admitted, self.limits)
                .map_err(|message| Self::failure_for(sequence, &message))?;
        let envelopes = commands
            .into_iter()
            .enumerate()
            .map(|(index, command)| {
                u32::try_from(index)
                    .map(|index| RuntimeCommandEnvelope {
                        transition: sequence,
                        index,
                        command,
                    })
                    .map_err(|_| {
                        Self::failure_for(sequence, "command vector index does not fit u32")
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let pending_commands = self
            .active
            .committed_commands
            .len()
            .checked_add(envelopes.len())
            .ok_or_else(|| {
                Self::failure_for(sequence, "pending root command count overflows usize")
            })?;
        if !self.limits.permits_pending_commands(pending_commands) {
            return Err(Self::failure_for(
                sequence,
                "pending root commands exceed the selected runtime limit",
            ));
        }
        let next_sequence = sequence.next().ok_or_else(|| {
            Self::failure_for(
                sequence,
                "ingress admitted an unconsumable terminal transition sequence",
            )
        })?;
        let RootPhase::Initialized { value, .. } = &mut self.active.phase else {
            return Err(Self::failure_for(sequence, "root state is not initialized"));
        };
        *value = state;
        self.active.next_sequence = next_sequence;
        self.active.committed_commands.extend(envelopes);
        Ok(RootTransitionOutcome::Committed {
            sequence,
            state_digest,
            command_digests,
        })
    }

    fn reject_reduction(
        &mut self,
        sequence: TransitionSequence,
        code: String,
        message: String,
        value: RuntimeValue,
    ) -> Result<RootTransitionOutcome, RootRuntimeFailure> {
        let error_digest = validate_replay_safe_payload(&RuntimePayload(value), self.limits.schema)
            .map_err(|message| Self::failure_for(sequence, &message))?;
        let next_sequence = sequence.next().ok_or_else(|| {
            Self::failure_for(
                sequence,
                "ingress admitted an unconsumable terminal transition sequence",
            )
        })?;
        self.active.next_sequence = next_sequence;
        Ok(RootTransitionOutcome::Rejected {
            sequence,
            code,
            message,
            error_digest,
        })
    }

    fn failure_for(sequence: TransitionSequence, message: &str) -> RootRuntimeFailure {
        let digest = RuntimeValueDigest::from_bytes(blake3::hash(message.as_bytes()).into());
        RootRuntimeFailure::Transition {
            sequence,
            message: message.to_owned(),
            digest,
        }
    }
}

enum ParsedReducerResult {
    Committed(RuntimeReductionValue),
    Rejected {
        code: String,
        message: String,
        value: RuntimeValue,
    },
}

fn parse_reducer_result(value: RuntimeValue) -> Result<ParsedReducerResult, String> {
    let Ok((case, Some(payload))) = value.try_into_builtin_variant_case() else {
        return Err("reducer must return Result<Reduction<State>, ReducerError>".to_owned());
    };
    match case {
        RuntimeBuiltinVariantCaseIdentity::ResultOk => parse_reduction(payload),
        RuntimeBuiltinVariantCaseIdentity::ResultErr => parse_reducer_error(payload),
        _ => Err("reducer must return Result<Reduction<State>, ReducerError>".to_owned()),
    }
}

fn parse_reduction(value: RuntimeValue) -> Result<ParsedReducerResult, String> {
    let RuntimeValue::Reduction(reduction) = value else {
        return Err("reducer Ok payload must be one admitted Reduction value".to_owned());
    };
    if !RuntimeReductionProducer::accepts_exact_owner(reduction.owner()) {
        return Err("reducer Ok payload has a non-concrete or foreign Reduction owner".to_owned());
    }
    Ok(ParsedReducerResult::Committed(reduction))
}

fn parse_reducer_error(value: RuntimeValue) -> Result<ParsedReducerResult, String> {
    if !value.ownership().permits_copy() {
        return Err("reducer error contains an affine value that root execution must copy".into());
    }
    let original = value.clone();
    let value = unwrap_named_variant(value, "ReducerError")?;
    let fields = record_fields(value, "ReducerError")?;
    let code = required_string(&fields, "code", "ReducerError")?;
    let message = required_string(&fields, "message", "ReducerError")?;
    Ok(ParsedReducerResult::Rejected {
        code,
        message,
        value: original,
    })
}

fn unwrap_named_variant(value: RuntimeValue, expected: &str) -> Result<RuntimeValue, String> {
    match value {
        RuntimeValue::Variant(variant)
            if matches!(variant.owner(), RuntimeVariantIdentity::Nominal { nominal, .. } if nominal.as_str() == expected)
                && variant.ordinal() == 0
                && variant.name() == expected
                && variant.type_instantiation().is_none() =>
        {
            variant
                .into_payload()
                .ok_or_else(|| format!("expected `{expected}` payload"))
        }
        RuntimeValue::Record(_) => Ok(value),
        _ => Err(format!("expected `{expected}` value")),
    }
}

fn record_fields(
    value: RuntimeValue,
    owner: &str,
) -> Result<BTreeMap<String, RuntimeValue>, String> {
    let RuntimeValue::Record(fields) = value else {
        return Err(format!("{owner} payload must be a record"));
    };
    let mut map = BTreeMap::new();
    for field in fields {
        let name = field.name().to_owned();
        if map.insert(name.clone(), field.into_value()).is_some() {
            return Err(format!("{owner} contains duplicate field `{name}`"));
        }
    }
    Ok(map)
}

fn required_string(
    fields: &BTreeMap<String, RuntimeValue>,
    field: &str,
    owner: &str,
) -> Result<String, String> {
    match fields.get(field) {
        Some(RuntimeValue::String(value)) => Ok(value.clone()),
        _ => Err(format!("{owner}.{field} must be a string")),
    }
}

fn validate_commands(
    commands: &[RuntimeCommand],
    contracts: &[RuntimeCommandContract],
    limits: RootExecutionLimits,
) -> Result<Vec<RuntimeValueDigest>, String> {
    if !limits.permits_transition_commands(commands.len()) {
        return Err("command count exceeds the per-transition budget".to_owned());
    }
    let mut encoded_bytes = 0_usize;
    let mut digests = Vec::with_capacity(commands.len());
    for command in commands {
        let contract = contracts
            .iter()
            .find(|contract| {
                &contract.constructor == command.constructor() && &contract.target == command.target()
            })
            .ok_or_else(|| {
                format!(
                    "command constructor `{}` and target `{}` are not admitted by the selected adapter policy",
                    command.constructor().as_str(),
                    command.target().as_str()
                )
            })?;
        contract
            .payload_schema
            .validate_payload(command.payload(), limits.schema)
            .map_err(|error| error.to_string())?;
        let encoded = canonical_command_bytes(command, limits.schema.platform_encoded_bytes())?;
        encoded_bytes = encoded_bytes
            .checked_add(encoded.len())
            .ok_or_else(|| "command byte count overflows usize".to_owned())?;
        if !limits.permits_transition_command_bytes(encoded_bytes) {
            return Err("command bytes exceed the per-transition budget".to_owned());
        }
        digests.push(RuntimeValueDigest::from_bytes(
            blake3::hash(&encoded).into(),
        ));
    }
    Ok(digests)
}

fn canonical_command_bytes<P: crate::value::RuntimeStoredValue>(
    command: &RuntimeCommand<P>,
    max_encoded_bytes: usize,
) -> Result<Vec<u8>, String> {
    let payload = crate::entry::schema::canonical_runtime_value_view_bytes(
        command.payload().borrowed_view(),
        max_encoded_bytes,
    )
    .map_err(|error| error.to_string())?;
    let constructor = command.constructor().as_str().as_bytes();
    let target = command.target().as_str().as_bytes();
    let constructor_len = u32::try_from(constructor.len())
        .map_err(|_| "command constructor length does not fit u32".to_owned())?;
    let target_len = u32::try_from(target.len())
        .map_err(|_| "command target length does not fit u32".to_owned())?;
    let payload_len = u32::try_from(payload.len())
        .map_err(|_| "command payload length does not fit u32".to_owned())?;
    let capacity = b"arcweft.runtime-command\0"
        .len()
        .checked_add(4)
        .and_then(|size| size.checked_add(4 + constructor.len()))
        .and_then(|size| size.checked_add(4 + target.len()))
        .and_then(|size| size.checked_add(4 + payload.len()))
        .ok_or_else(|| "command encoding length overflows usize".to_owned())?;
    if capacity > max_encoded_bytes {
        return Err("command exceeds encoded byte budget".to_owned());
    }
    let mut bytes = Vec::with_capacity(capacity);
    bytes.extend_from_slice(b"arcweft.runtime-command\0");
    bytes.extend_from_slice(&1_u32.to_le_bytes());
    bytes.extend_from_slice(&constructor_len.to_le_bytes());
    bytes.extend_from_slice(constructor);
    bytes.extend_from_slice(&target_len.to_le_bytes());
    bytes.extend_from_slice(target);
    bytes.extend_from_slice(&payload_len.to_le_bytes());
    bytes.extend_from_slice(&payload);
    Ok(bytes)
}

/// Validates an opaque replay-safe payload without pretending it has a
/// user-declared nominal schema.
pub fn validate_replay_safe_payload(
    payload: &RuntimePayload,
    limits: RuntimeSchemaLimits,
) -> Result<RuntimeValueDigest, String> {
    let mut resources = crate::entry::schema::value_encoding::ValueResourceAdmission::snapshot(
        limits,
        crate::entry::schema::value_encoding::ValueAdmissionOrigin::Live,
    );
    resources
        .validate_view(payload.0.view(), 0, Some(&mut validate_replay_safe_node))
        .map_err(|error| error.to_string())?;
    let bytes = canonical_runtime_value_bytes(&payload.0, limits.platform_encoded_bytes())
        .map_err(|error| error.to_string())?;
    Ok(RuntimeValueDigest::from_bytes(blake3::hash(&bytes).into()))
}

fn validate_replay_safe_node(
    value: RuntimeValueView<'_>,
) -> Result<(), crate::entry::RuntimeSchemaError> {
    let message = match value {
        RuntimeValueView::Scalar(RuntimeScalarView::F32(value)) if !value.is_finite() => {
            Some("replay-safe payload contains non-finite f32")
        }
        RuntimeValueView::Scalar(RuntimeScalarView::F64(value)) if !value.is_finite() => {
            Some("replay-safe payload contains non-finite f64")
        }
        RuntimeValueView::Opaque(value)
            if value.persistence() == crate::value::RuntimeOpaquePersistence::SnapshotOnly
                || matches!(
                    value.value_class(),
                    crate::value::RuntimeOpaqueValueClass::AffineHandle(_)
                ) =>
        {
            Some("non-constant opaque value is not replay-safe")
        }
        RuntimeValueView::DenseNumeric(_)
        | RuntimeValueView::Range(_)
        | RuntimeValueView::Iterator(_)
        | RuntimeValueView::Need(_)
        | RuntimeValueView::Callable(_) => Some("runtime-only value is not replay-safe"),
        _ => None,
    };
    message.map_or(Ok(()), |message| {
        Err(crate::entry::RuntimeSchemaError::Encoding {
            message: message.to_owned(),
        })
    })
}

/// In-memory snapshot for one durable unrestricted root state.
///
/// ```compile_fail
/// use arcweft_core::root::RootStateSnapshotV1;
/// fn copy(value: &RootStateSnapshotV1) { let _: RootStateSnapshotV1 = value.clone(); }
/// ```
/// ```compile_fail
/// use arcweft_core::root::RootStateSnapshotV1;
/// let _: RootStateSnapshotV1 = serde_json::from_str("{}").unwrap();
/// ```
#[derive(Debug, serde::Deserialize, PartialEq, serde::Serialize)]
#[serde(deny_unknown_fields)]
#[serde(bound(
    serialize = "P: crate::value::InertRuntimeValueStorage",
    deserialize = "P: crate::value::InertRuntimeValueStorage"
))]
pub struct RootStateSnapshot<P> {
    pub state_identity: crate::entry::RuntimeNominalTypeId,
    pub state_layout: TypeLayoutHash,
    pub event_identity: crate::entry::RuntimeNominalTypeId,
    pub event_layout: TypeLayoutHash,
    pub phase: RootPhase<P>,
    pub next_sequence: TransitionSequence,
}

/// Live version-one root snapshot; constructing another owner requires
/// admission under its original selected entry.
pub type RootStateSnapshotV1 = RootStateSnapshot<RuntimePayload>;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RootSaveBlockers {
    pub reducer_active: bool,
    pub pending_events: u32,
    pub pending_commands: u32,
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod reducer_result_tests;
