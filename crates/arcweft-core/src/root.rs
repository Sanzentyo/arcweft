//! Deterministic durable root-state transitions for stateful entries.
//!
//! Root work is a phase of the existing runtime step. This module owns the
//! transactional state boundary but performs no host I/O and dispatches no
//! command.

use crate::entry::{
    EntryBindingIdentity, RootExecutionLimits, RuntimeCallableRole, RuntimeCommandConstructorId,
    RuntimeCommandContract, RuntimeCommandTargetId, RuntimeFlowExecutable, RuntimeSchemaError,
    RuntimeSchemaLimits, RuntimeStatefulEntryRoles, RuntimeValueDigest, TypeLayoutHash,
    canonical_runtime_value_bytes,
};
use crate::pattern::RuntimeVariantIdentity;
use crate::plan::{
    EntryRuntimeId, FlowRuntimeId, RuntimeEntryRoles, RuntimePlan, RuntimePlanError,
};
use crate::value::{RuntimeAgentValue, RuntimeBinding, RuntimePayload, RuntimeValue};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
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

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
struct SequencedRootEvent {
    sequence: TransitionSequence,
    payload: RuntimePayload,
}

/// Opaque replay-safe command value produced by a reducer.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimeCommand {
    constructor: RuntimeCommandConstructorId,
    target: RuntimeCommandTargetId,
    payload: RuntimePayload,
}

impl RuntimeCommand {
    #[must_use]
    pub const fn constructor(&self) -> &RuntimeCommandConstructorId {
        &self.constructor
    }

    #[must_use]
    pub const fn target(&self) -> &RuntimeCommandTargetId {
        &self.target
    }

    #[must_use]
    pub const fn payload(&self) -> &RuntimePayload {
        &self.payload
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimeCommandEnvelope {
    pub transition: TransitionSequence,
    pub index: u32,
    pub command: RuntimeCommand,
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

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ActiveRootState {
    pub entry: EntryRuntimeId,
    pub binding: EntryBindingIdentity,
    pub state_identity: crate::entry::RuntimeNominalTypeId,
    pub state_layout: TypeLayoutHash,
    pub event_identity: crate::entry::RuntimeNominalTypeId,
    pub event_layout: TypeLayoutHash,
    pub value: RuntimePayload,
    pub next_sequence: TransitionSequence,
    #[serde(skip)]
    queued_events: VecDeque<SequencedRootEvent>,
    #[serde(skip)]
    committed_commands: VecDeque<RuntimeCommandEnvelope>,
    #[serde(skip)]
    reducer_active: bool,
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
        self.reducer_active
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RootRuntime {
    active: ActiveRootState,
    roles: RuntimeStatefulEntryRoles,
    reducer: RuntimeCallableRole,
    limits: RootExecutionLimits,
    failure: Option<RootRuntimeFailure>,
}

/// Fully verified metadata needed to construct one durable root transaction
/// owner without coupling it to a concrete executable tier.
#[derive(Clone, Debug, PartialEq)]
pub struct RootStartupContract {
    pub entry: EntryRuntimeId,
    pub roles: RuntimeStatefulEntryRoles,
    pub initial_flow: RuntimeFlowExecutable,
}

#[derive(Clone, Debug)]
pub struct RootStartup {
    pub root: RootRuntime,
    pub initial_flow: FlowRuntimeId,
    pub initial_state_binding: RuntimeBinding,
    pub initializer_state_digest: RuntimeValueDigest,
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
    #[error("runtime entry `{0}` references a missing reducer helper")]
    MissingReducer(String),
    #[error("runtime entry `{0}` references missing initial-flow executable metadata")]
    MissingInitialFlowExecutable(String),
    #[error("entry initializer failed: {0}")]
    Initializer(#[source] RootCallableEvaluationError),
    #[error("entry initializer returned an invalid root value: {0}")]
    InvalidInitialValue(#[source] RuntimeSchemaError),
    #[error("saved root metadata does not match the selected {0} role")]
    SnapshotRoleMismatch(&'static str),
    #[error("saved root value is invalid: {0}")]
    InvalidSnapshotValue(#[source] RuntimeSchemaError),
    #[error("root-event batch is not valid: {0}")]
    InvalidEvent(#[source] RuntimeSchemaError),
    #[error("root-event queue exceeds the selected runtime limit of {limit}")]
    EventQueueLimit { limit: usize },
    #[error("root-event transition sequence is exhausted")]
    TransitionSequenceExhausted,
    #[error("committed root commands must be accepted by the driver before another step")]
    PendingCommandsAwaitingDispatch,
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
        args: &[RuntimeValue],
    ) -> Result<RuntimeValue, RootCallableEvaluationError>;
}

impl RootStartupContract {
    /// Resolves exact stateful-entry metadata from a verified structured plan.
    pub fn from_runtime_plan(
        plan: &RuntimePlan,
        entry_id: &EntryRuntimeId,
    ) -> Result<Self, RootRuntimeError> {
        plan.verify()?;
        let entry = plan
            .entries
            .iter()
            .find(|entry| entry.id == *entry_id)
            .ok_or_else(|| RootRuntimeError::MissingEntry(entry_id.canonical_label()))?;
        let RuntimeEntryRoles::Stateful(roles) = &entry.roles else {
            return Err(RootRuntimeError::NotStateful(entry.id.canonical_label()));
        };
        let initial_flow = plan
            .flow_executables
            .iter()
            .find(|flow| {
                flow.flow == roles.initial_flow.flow && flow.contract == roles.initial_flow.contract
            })
            .cloned()
            .ok_or_else(|| {
                RootRuntimeError::MissingInitialFlowExecutable(entry.id.canonical_label())
            })?;
        Ok(Self {
            entry: entry.id.clone(),
            roles: roles.as_ref().clone(),
            initial_flow,
        })
    }
}

impl RootRuntime {
    /// Executes the state initializer and constructs the root/flow candidates.
    ///
    /// The caller atomically installs both candidates only after its flow
    /// constructor succeeds.
    pub fn start(
        contract: RootStartupContract,
        evaluator: &mut impl RootCallableEvaluator,
    ) -> Result<RootStartup, RootRuntimeError> {
        let RootStartupContract {
            entry,
            roles,
            initial_flow,
        } = contract;
        let [state_parameter] = initial_flow.parameters.as_slice() else {
            return Err(RootRuntimeError::MissingInitialFlowExecutable(
                entry.canonical_label(),
            ));
        };
        let value = evaluator
            .evaluate_root_callable(&roles.initializer, &[])
            .map_err(RootRuntimeError::Initializer)?;
        let payload = RuntimePayload(value);
        let limits = roles.command_policy.root_limits;
        let initializer_state_digest = roles
            .state
            .schema
            .validate_payload(&payload, limits.schema)
            .map_err(RootRuntimeError::InvalidInitialValue)?;
        let active = ActiveRootState {
            entry: entry.clone(),
            binding: roles.binding,
            state_identity: roles.state.identity.clone(),
            state_layout: roles.state.layout,
            event_identity: roles.event.identity.clone(),
            event_layout: roles.event.layout,
            value: payload.clone(),
            next_sequence: TransitionSequence::ZERO,
            queued_events: VecDeque::new(),
            committed_commands: VecDeque::new(),
            reducer_active: false,
        };
        let root = Self {
            active,
            reducer: roles.reducer.clone(),
            roles,
            limits,
            failure: None,
        };
        Ok(RootStartup {
            initial_flow: initial_flow.flow,
            initial_state_binding: RuntimeBinding {
                name: state_parameter.name.clone(),
                value: payload.0.clone(),
            },
            root,
            initializer_state_digest,
        })
    }

    #[must_use]
    pub const fn active(&self) -> &ActiveRootState {
        &self.active
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
        RootStateSnapshotV1 {
            state_identity: self.active.state_identity.clone(),
            state_layout: self.active.state_layout,
            event_identity: self.active.event_identity.clone(),
            event_layout: self.active.event_layout,
            value: self.active.value.clone(),
            next_sequence: self.active.next_sequence,
        }
    }

    #[must_use]
    pub fn save_blockers(&self) -> RootSaveBlockers {
        RootSaveBlockers {
            reducer_active: self.active.reducer_active,
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
    ) -> Result<Self, RootRuntimeError> {
        let RootStartupContract {
            entry,
            roles,
            initial_flow,
        } = contract;
        let [state_parameter] = initial_flow.parameters.as_slice() else {
            return Err(RootRuntimeError::MissingInitialFlowExecutable(
                entry.canonical_label(),
            ));
        };
        if state_parameter.mode != crate::entry::RuntimeFlowParameterMode::Owned
            || state_parameter.nominal != roles.state.identity
            || state_parameter.layout != roles.state.layout
        {
            return Err(RootRuntimeError::MissingInitialFlowExecutable(
                entry.canonical_label(),
            ));
        }
        if snapshot.state_identity != roles.state.identity
            || snapshot.state_layout != roles.state.layout
        {
            return Err(RootRuntimeError::SnapshotRoleMismatch("state"));
        }
        if snapshot.event_identity != roles.event.identity
            || snapshot.event_layout != roles.event.layout
        {
            return Err(RootRuntimeError::SnapshotRoleMismatch("event"));
        }
        let limits = roles.command_policy.root_limits;
        roles
            .state
            .schema
            .validate_payload(&snapshot.value, limits.schema)
            .map_err(RootRuntimeError::InvalidSnapshotValue)?;
        Ok(Self {
            active: ActiveRootState {
                entry,
                binding: roles.binding,
                state_identity: snapshot.state_identity,
                state_layout: snapshot.state_layout,
                event_identity: snapshot.event_identity,
                event_layout: snapshot.event_layout,
                value: snapshot.value,
                next_sequence: snapshot.next_sequence,
                queued_events: VecDeque::new(),
                committed_commands: VecDeque::new(),
                reducer_active: false,
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
    ) -> Result<RootStepResult, RootRuntimeError> {
        if !self.active.committed_commands.is_empty() {
            return Err(RootRuntimeError::PendingCommandsAwaitingDispatch);
        }
        self.ingress(events)?;
        let mut result = RootStepResult::default();
        while let Some(event) = self.active.queued_events.front().cloned() {
            match self.reduce_front(&event, evaluator) {
                Ok(RootReductionDisposition::Committed(outcome)) => {
                    result.outcomes.push(outcome);
                }
                Ok(RootReductionDisposition::Rejected(outcome)) => {
                    result.outcomes.push(outcome);
                    result.stopped_after_rejection = true;
                    break;
                }
                Err(failure) => {
                    let outcome = match &failure {
                        RootRuntimeFailure::Transition {
                            sequence,
                            message,
                            digest,
                        } => RootTransitionOutcome::Trapped {
                            sequence: *sequence,
                            failure_digest: *digest,
                            message: message.clone(),
                        },
                    };
                    self.active.reducer_active = false;
                    self.active.queued_events.clear();
                    self.failure = Some(failure);
                    result.outcomes.push(outcome);
                    result
                        .commands
                        .extend(self.active.committed_commands.iter().cloned());
                    result.failed = true;
                    return Ok(result);
                }
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

    pub fn ingress(&mut self, events: Vec<RootEventInput>) -> Result<(), RootRuntimeError> {
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
        if pending_events > self.limits.max_pending_events {
            return Err(RootRuntimeError::EventQueueLimit {
                limit: self.limits.max_pending_events,
            });
        }
        for event in &events {
            self.roles
                .event
                .schema
                .validate_payload(&event.payload, self.limits.schema)
                .map_err(RootRuntimeError::InvalidEvent)?;
        }
        let first = match self.active.queued_events.back() {
            Some(event) => event
                .sequence
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

    fn reduce_front(
        &mut self,
        event: &SequencedRootEvent,
        evaluator: &mut impl RootCallableEvaluator,
    ) -> Result<RootReductionDisposition, RootRuntimeFailure> {
        if event.sequence != self.active.next_sequence {
            return Err(Self::failure_for(
                event.sequence,
                "root event sequence does not equal the transition cursor",
            ));
        }
        self.active.reducer_active = true;
        let returned = evaluator.evaluate_root_callable(
            &self.reducer,
            &[self.active.value.0.clone(), event.payload.0.clone()],
        );
        let returned = match returned {
            Ok(value) => value,
            Err(error) => {
                self.active.reducer_active = false;
                return Err(Self::failure_for(event.sequence, &error.to_string()));
            }
        };
        match parse_reducer_result(returned) {
            Ok(ParsedReducerResult::Committed { state, commands }) => {
                self.commit_reduction(event, state, commands)
            }
            Ok(ParsedReducerResult::Rejected {
                code,
                message,
                value,
            }) => self.reject_reduction(event, code, message, value),
            Err(message) => {
                self.active.reducer_active = false;
                Err(Self::failure_for(event.sequence, &message))
            }
        }
    }

    fn commit_reduction(
        &mut self,
        event: &SequencedRootEvent,
        state: RuntimeValue,
        commands: Vec<RuntimeCommand>,
    ) -> Result<RootReductionDisposition, RootRuntimeFailure> {
        let state = RuntimePayload(state);
        let state_digest = self
            .roles
            .state
            .schema
            .validate_payload(&state, self.limits.schema)
            .map_err(|error| Self::failure_for(event.sequence, &error.to_string()))?;
        let command_digests =
            validate_commands(&commands, &self.roles.command_policy.admitted, self.limits)
                .map_err(|message| Self::failure_for(event.sequence, &message))?;
        let envelopes = commands
            .into_iter()
            .enumerate()
            .map(|(index, command)| {
                u32::try_from(index)
                    .map(|index| RuntimeCommandEnvelope {
                        transition: event.sequence,
                        index,
                        command,
                    })
                    .map_err(|_| {
                        Self::failure_for(event.sequence, "command vector index does not fit u32")
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let pending_commands = self
            .active
            .committed_commands
            .len()
            .checked_add(envelopes.len())
            .ok_or_else(|| {
                Self::failure_for(event.sequence, "pending root command count overflows usize")
            })?;
        if pending_commands > self.limits.max_pending_commands {
            return Err(Self::failure_for(
                event.sequence,
                "pending root commands exceed the selected runtime limit",
            ));
        }
        let next_sequence = event.sequence.next().ok_or_else(|| {
            Self::failure_for(
                event.sequence,
                "ingress admitted an unconsumable terminal transition sequence",
            )
        })?;
        self.active.value = state;
        self.active.queued_events.pop_front();
        self.active.next_sequence = next_sequence;
        self.active.committed_commands.extend(envelopes);
        self.active.reducer_active = false;
        Ok(RootReductionDisposition::Committed(
            RootTransitionOutcome::Committed {
                sequence: event.sequence,
                state_digest,
                command_digests,
            },
        ))
    }

    fn reject_reduction(
        &mut self,
        event: &SequencedRootEvent,
        code: String,
        message: String,
        value: RuntimeValue,
    ) -> Result<RootReductionDisposition, RootRuntimeFailure> {
        let error_digest = validate_replay_safe_payload(&RuntimePayload(value), self.limits.schema)
            .map_err(|message| Self::failure_for(event.sequence, &message))?;
        let next_sequence = event.sequence.next().ok_or_else(|| {
            Self::failure_for(
                event.sequence,
                "ingress admitted an unconsumable terminal transition sequence",
            )
        })?;
        self.active.queued_events.pop_front();
        self.active.next_sequence = next_sequence;
        self.active.reducer_active = false;
        Ok(RootReductionDisposition::Rejected(
            RootTransitionOutcome::Rejected {
                sequence: event.sequence,
                code,
                message,
                error_digest,
            },
        ))
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

enum RootReductionDisposition {
    Committed(RootTransitionOutcome),
    Rejected(RootTransitionOutcome),
}

enum ParsedReducerResult {
    Committed {
        state: RuntimeValue,
        commands: Vec<RuntimeCommand>,
    },
    Rejected {
        code: String,
        message: String,
        value: RuntimeValue,
    },
}

fn parse_reducer_result(value: RuntimeValue) -> Result<ParsedReducerResult, String> {
    let RuntimeValue::Variant {
        owner: RuntimeVariantIdentity::Result,
        ordinal,
        name,
        payload: Some(payload),
        ..
    } = value
    else {
        return Err("reducer must return Result<Reduction<State>, ReducerError>".to_owned());
    };
    match (ordinal, name.as_str()) {
        (0, "Ok") => parse_reduction(*payload),
        (1, "Err") => parse_reducer_error(*payload),
        _ => Err(format!("reducer returned unknown result variant `{name}`")),
    }
}

fn parse_reduction(value: RuntimeValue) -> Result<ParsedReducerResult, String> {
    let value = unwrap_named_variant(value, "Reduction")?;
    let fields = record_fields(value, "Reduction")?;
    let state = fields
        .get("state")
        .cloned()
        .ok_or_else(|| "Reduction is missing `state`".to_owned())?;
    let commands = fields
        .get("commands")
        .cloned()
        .ok_or_else(|| "Reduction is missing `commands`".to_owned())?;
    let RuntimeValue::Seq(commands) = commands else {
        return Err("Reduction.commands must be a sequence".to_owned());
    };
    let commands = commands
        .into_values()
        .into_iter()
        .map(parse_command)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ParsedReducerResult::Committed { state, commands })
}

fn parse_reducer_error(value: RuntimeValue) -> Result<ParsedReducerResult, String> {
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

fn parse_command(value: RuntimeValue) -> Result<RuntimeCommand, String> {
    let value = unwrap_named_variant(value, "Command")?;
    let fields = record_fields(value, "Command")?;
    let constructor = match fields.get("constructor") {
        Some(RuntimeValue::String(value) | RuntimeValue::EntityRef(value)) => {
            RuntimeCommandConstructorId::try_new(value.clone())
                .map_err(|error| error.to_string())?
        }
        _ => return Err("Command.constructor must be a non-empty stable identity".to_owned()),
    };
    let target = match fields.get("target") {
        Some(RuntimeValue::String(value) | RuntimeValue::EntityRef(value)) if !value.is_empty() => {
            RuntimeCommandTargetId::try_new(value.clone()).map_err(|error| error.to_string())?
        }
        _ => return Err("Command.target must be a non-empty stable identity".to_owned()),
    };
    let payload = fields
        .get("payload")
        .cloned()
        .ok_or_else(|| "Command is missing `payload`".to_owned())?;
    Ok(RuntimeCommand {
        constructor,
        target,
        payload: RuntimePayload(payload),
    })
}

fn unwrap_named_variant(value: RuntimeValue, expected: &str) -> Result<RuntimeValue, String> {
    match value {
        RuntimeValue::Variant {
            owner: RuntimeVariantIdentity::Nominal { nominal, .. },
            ordinal: 0,
            name,
            payload: Some(payload),
        } if nominal.as_str() == expected && name == expected => Ok(*payload),
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
    if commands.len() > limits.max_commands_per_transition {
        return Err("command count exceeds the per-transition budget".to_owned());
    }
    let mut encoded_bytes = 0_usize;
    let mut digests = Vec::with_capacity(commands.len());
    for command in commands {
        let contract = contracts
            .iter()
            .find(|contract| {
                contract.constructor == command.constructor && contract.target == command.target
            })
            .ok_or_else(|| {
                format!(
                    "command constructor `{}` and target `{}` are not admitted by the selected adapter policy",
                    command.constructor.as_str(),
                    command.target.as_str()
                )
            })?;
        contract
            .payload_schema
            .validate_payload(&command.payload, limits.schema)
            .map_err(|error| error.to_string())?;
        let encoded = canonical_command_bytes(command, limits.schema.max_encoded_bytes)?;
        encoded_bytes = encoded_bytes
            .checked_add(encoded.len())
            .ok_or_else(|| "command byte count overflows usize".to_owned())?;
        if encoded_bytes > limits.max_command_bytes_per_transition {
            return Err("command bytes exceed the per-transition budget".to_owned());
        }
        digests.push(RuntimeValueDigest::from_bytes(
            blake3::hash(&encoded).into(),
        ));
    }
    Ok(digests)
}

fn canonical_command_bytes(
    command: &RuntimeCommand,
    max_encoded_bytes: usize,
) -> Result<Vec<u8>, String> {
    let payload = canonical_runtime_value_bytes(&command.payload.0, max_encoded_bytes)
        .map_err(|error| error.to_string())?;
    let constructor = command.constructor.as_str().as_bytes();
    let target = command.target.as_str().as_bytes();
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
    let mut nodes = 0_usize;
    validate_replay_safe_value(&payload.0, limits, 0, &mut nodes)?;
    let bytes = canonical_runtime_value_bytes(&payload.0, limits.max_encoded_bytes)
        .map_err(|error| error.to_string())?;
    Ok(RuntimeValueDigest::from_bytes(blake3::hash(&bytes).into()))
}

fn validate_replay_safe_value(
    value: &RuntimeValue,
    limits: RuntimeSchemaLimits,
    depth: usize,
    nodes: &mut usize,
) -> Result<(), String> {
    if depth > limits.max_depth {
        return Err("replay-safe payload exceeds depth budget".to_owned());
    }
    *nodes = nodes
        .checked_add(1)
        .ok_or_else(|| "replay-safe node count overflows usize".to_owned())?;
    if *nodes > limits.max_nodes {
        return Err("replay-safe payload exceeds node budget".to_owned());
    }
    match value {
        RuntimeValue::F32(value) if !value.is_finite() => {
            Err("replay-safe payload contains non-finite f32".to_owned())
        }
        RuntimeValue::F64(value) if !value.is_finite() => {
            Err("replay-safe payload contains non-finite f64".to_owned())
        }
        RuntimeValue::String(value) | RuntimeValue::EntityRef(value)
            if value.len() > limits.max_string_bytes =>
        {
            Err("replay-safe payload exceeds string byte budget".to_owned())
        }
        RuntimeValue::Tuple(values) => {
            for value in values {
                validate_replay_safe_value(value, limits, depth + 1, nodes)?;
            }
            Ok(())
        }
        RuntimeValue::Seq(values) => {
            let values = values.clone().into_values();
            if values.len() > limits.max_sequence_items {
                return Err("replay-safe payload exceeds sequence item budget".to_owned());
            }
            for value in &values {
                validate_replay_safe_value(value, limits, depth + 1, nodes)?;
            }
            Ok(())
        }
        RuntimeValue::Record(fields) => {
            let mut names = BTreeSet::new();
            for field in fields {
                if !names.insert(field.name()) {
                    return Err(format!(
                        "replay-safe payload contains duplicate field `{}`",
                        field.name()
                    ));
                }
                validate_replay_safe_value(field.value(), limits, depth + 1, nodes)?;
            }
            Ok(())
        }
        RuntimeValue::NominalRecord(record) => {
            if record.fields().len() > limits.max_sequence_items {
                return Err("replay-safe payload exceeds nominal field budget".to_owned());
            }
            for field in record.fields() {
                validate_replay_safe_value(field, limits, depth + 1, nodes)?;
            }
            Ok(())
        }
        RuntimeValue::Opaque(value) => {
            validate_replay_safe_value(value.payload(), limits, depth + 1, nodes)
        }
        RuntimeValue::Agent(value) => validate_replay_safe_agent_value(value, limits, depth, nodes),
        RuntimeValue::Variant { payload, .. } => {
            if let Some(payload) = payload {
                validate_replay_safe_value(payload, limits, depth + 1, nodes)?;
            }
            Ok(())
        }
        RuntimeValue::Function(_)
        | RuntimeValue::Iterator(_)
        | RuntimeValue::Range(_)
        | RuntimeValue::MatrixF32(_)
        | RuntimeValue::MatrixF64(_)
        | RuntimeValue::TensorF32(_)
        | RuntimeValue::TensorF64(_) => Err("runtime-only value is not replay-safe".to_owned()),
        RuntimeValue::Unit
        | RuntimeValue::Bool(_)
        | RuntimeValue::Int(_)
        | RuntimeValue::UInt(_)
        | RuntimeValue::F32(_)
        | RuntimeValue::F64(_)
        | RuntimeValue::String(_)
        | RuntimeValue::Char(_)
        | RuntimeValue::Duration(_)
        | RuntimeValue::EntityRef(_) => Ok(()),
    }
}

fn validate_replay_safe_agent_value(
    value: &RuntimeAgentValue,
    limits: RuntimeSchemaLimits,
    depth: usize,
    nodes: &mut usize,
) -> Result<(), String> {
    if depth.saturating_add(value.structural_nesting_depth()) > limits.max_depth {
        return Err("replay-safe payload exceeds depth budget".to_owned());
    }
    *nodes = nodes
        .checked_add(value.additional_structural_node_count())
        .ok_or_else(|| "replay-safe node count overflows usize".to_owned())?;
    if *nodes > limits.max_nodes {
        return Err("replay-safe payload exceeds node budget".to_owned());
    }
    if value
        .text_values()
        .into_iter()
        .any(|value| value.len() > limits.max_string_bytes)
    {
        return Err("replay-safe payload exceeds string byte budget".to_owned());
    }
    if value
        .predicate_collection_lengths()
        .into_iter()
        .any(|length| length > limits.max_sequence_items)
    {
        return Err("replay-safe payload exceeds sequence item budget".to_owned());
    }
    for (offset, nested) in value.nested_runtime_values_with_depth() {
        validate_replay_safe_value(nested, limits, depth.saturating_add(offset), nodes)?;
    }
    Ok(())
}

/// Final in-place session payload for one durable root state.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RootStateSnapshotV1 {
    pub state_identity: crate::entry::RuntimeNominalTypeId,
    pub state_layout: TypeLayoutHash,
    pub event_identity: crate::entry::RuntimeNominalTypeId,
    pub event_layout: TypeLayoutHash,
    pub value: RuntimePayload,
    pub next_sequence: TransitionSequence,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RootSaveBlockers {
    pub reducer_active: bool,
    pub pending_events: u32,
    pub pending_commands: u32,
}

#[cfg(test)]
mod save_blocker_tests {
    use super::*;
    use crate::entry::{
        CallableContractHash, FlowContractHash, RootExecutionLimits, RuntimeCallableId,
        RuntimeCommandPolicy, RuntimeFlowExecutableParameter, RuntimeFlowParameterMode,
        RuntimeFlowRole, RuntimeNominalRole, RuntimeTypeSchema,
    };

    struct Initializer;

    impl RootCallableEvaluator for Initializer {
        fn evaluate_root_callable(
            &mut self,
            _callable: &RuntimeCallableRole,
            _args: &[RuntimeValue],
        ) -> Result<RuntimeValue, RootCallableEvaluationError> {
            Ok(RuntimeValue::i64(1))
        }
    }

    #[test]
    fn save_002_active_reducer_reports_exact_blocker() {
        let entry =
            EntryRuntimeId::from_source_entity_body("entry.save_blocker").expect("entry ID");
        let flow = FlowRuntimeId::from_runtime_target_value("flow.save_blocker").expect("flow ID");
        let state_schema = RuntimeTypeSchema::I64;
        let event_schema = RuntimeTypeSchema::I64;
        let state_layout = state_schema.try_layout_hash().expect("state layout");
        let event_layout = event_schema.try_layout_hash().expect("event layout");
        let initializer = RuntimeCallableRole {
            callable: RuntimeCallableId::try_new("save_blocker.initial").expect("callable ID"),
            contract: CallableContractHash::from_bytes([1; 32]),
        };
        let reducer = RuntimeCallableRole {
            callable: RuntimeCallableId::try_new("save_blocker.reduce").expect("callable ID"),
            contract: CallableContractHash::from_bytes([2; 32]),
        };
        let initial_flow = RuntimeFlowRole {
            flow: flow.clone(),
            contract: FlowContractHash::from_bytes([3; 32]),
        };
        let contract = RootStartupContract {
            entry,
            roles: RuntimeStatefulEntryRoles {
                binding: EntryBindingIdentity::from_bytes([4; 32]),
                state: RuntimeNominalRole {
                    identity: crate::entry::RuntimeNominalTypeId::try_new("SaveState")
                        .expect("state ID"),
                    layout: state_layout,
                    schema: state_schema,
                },
                initializer,
                event: RuntimeNominalRole {
                    identity: crate::entry::RuntimeNominalTypeId::try_new("SaveEvent")
                        .expect("event ID"),
                    layout: event_layout,
                    schema: event_schema,
                },
                reducer,
                initial_flow: initial_flow.clone(),
                command_policy: RuntimeCommandPolicy::deny_all(
                    RootExecutionLimits::engine_default(),
                ),
            },
            initial_flow: RuntimeFlowExecutable {
                flow,
                contract: initial_flow.contract,
                parameters: vec![RuntimeFlowExecutableParameter {
                    position: 0,
                    name: "state".to_owned(),
                    mode: RuntimeFlowParameterMode::Owned,
                    nominal: crate::entry::RuntimeNominalTypeId::try_new("SaveState")
                        .expect("state ID"),
                    layout: state_layout,
                }],
                controller: None,
            },
        };
        let mut root = RootRuntime::start(contract, &mut Initializer)
            .expect("root starts")
            .root;
        root.active.reducer_active = true;

        assert_eq!(
            root.save_blockers(),
            RootSaveBlockers {
                reducer_active: true,
                pending_events: 0,
                pending_commands: 0,
            }
        );
    }
}
