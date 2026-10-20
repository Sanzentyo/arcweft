//! Versioned root replay data and public diagnostics.

use super::super::{
    BundleSessionArtifactIdentity, BundleSessionStep, BundleStepInput, EntryRuntimeId,
    RuntimeClockStep, RuntimeHostCallId,
};
use arcweft_core::entry::{
    EntryBindingIdentity, RuntimeNominalTypeId, RuntimeStatefulEntryRoles, RuntimeValueDigest,
    TypeLayoutHash,
};
use arcweft_core::plan::RuntimeEntryKind;
use arcweft_core::root::TransitionSequence;
use arcweft_core::value::{AwbcRuntimeValueSnapshot, RuntimeFormatDataIdentity};
use arcweft_id::LocaleTag;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use thiserror::Error;

pub const ROOT_REPLAY_SCHEMA_VERSION: u32 = 1;
pub const ROOT_REPLAY_ENGINE_IDENTITY: &str = "arcweft.root-replay.v1";
pub const ROOT_REPLAY_SAVE_SCHEMA_ID: &str = "arcweft.runtime.root-replay";

/// Immutable identity shared by a pending recorder and a completed trace.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RootReplayIdentityV1 {
    pub schema_version: u32,
    pub engine_identity: String,
    pub artifact: BundleSessionArtifactIdentity,
    pub active_locale: LocaleTag,
    pub format_data_identity: RuntimeFormatDataIdentity,
    pub entry: EntryRuntimeId,
    pub entry_kind: RuntimeEntryKind,
    pub binding: EntryBindingIdentity,
    pub state_identity: RuntimeNominalTypeId,
    pub state_layout: TypeLayoutHash,
    pub event_identity: RuntimeNominalTypeId,
    pub event_layout: TypeLayoutHash,
}

/// A complete transcript always has an observed initializer result.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RootReplayTraceV1 {
    #[serde(flatten)]
    pub identity: RootReplayIdentityV1,
    pub initializer_state_digest: RuntimeValueDigest,
    pub transitions: Vec<RecordedRootTransitionV1>,
    pub external_outcomes: Vec<RecordedExternalOutcome>,
}

/// One expected root transition. The sequence is asserted, never injected.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RecordedRootTransitionV1 {
    pub sequence: TransitionSequence,
    pub event: AwbcRuntimeValueSnapshot,
    pub event_digest: RuntimeValueDigest,
    pub outcome: RecordedRootOutcomeV1,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum RecordedRootOutcomeV1<Digests = Vec<RuntimeValueDigest>> {
    Committed {
        state_digest: RuntimeValueDigest,
        command_digests: Digests,
    },
    Rejected {
        error_digest: RuntimeValueDigest,
    },
    Trapped {
        failure_digest: RuntimeValueDigest,
    },
}

/// Replay position of one recorded host result.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RecordedExternalOutcomePositionV1 {
    BeforeTransition(TransitionSequence),
    AfterTransitions,
}

/// One external result injected through the existing deterministic result path.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RecordedExternalOutcome {
    pub position: RecordedExternalOutcomePositionV1,
    pub request: RuntimeHostCallId,
    pub outcome: RecordedExternalOutcomeResultV1,
    pub root_event_sequence: Option<TransitionSequence>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum RecordedExternalOutcomeResultV1 {
    Success(AwbcRuntimeValueSnapshot),
    Failure {
        kind: RecordedHostCallErrorKindV1,
        message: String,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RecordedHostCallErrorKindV1 {
    UnsupportedCapability,
    Rejected,
    Failed,
}

/// Production recorder for root replay traces.
#[derive(Clone, Debug)]
pub struct RootReplayRecorderV1 {
    pub(super) identity: RootReplayIdentityV1,
    pub(super) initializer_state_digest: Option<RuntimeValueDigest>,
    pub(super) transitions: Vec<RecordedRootTransitionV1>,
    pub(super) external_outcomes: Vec<RecordedExternalOutcome>,
    pub(super) roles: RuntimeStatefulEntryRoles,
    pub(super) pending_events: VecDeque<AwbcRuntimeValueSnapshot>,
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum RootReplayRecordingError {
    #[error("Root snapshot inspection failed: {0}")]
    RootState(#[source] Box<arcweft_core::root::RootRuntimeError>),
    #[error("root replay snapshot admission failed: {0}")]
    Snapshot(#[source] Box<arcweft_core::value::AwbcRuntimeValueSnapshotError>),
    #[error("root replay initializer has not completed")]
    InitializationPending,
    #[error("root replay initialization does not match the selected entry")]
    InitializationMismatch,
    #[error("root replay recording could not inspect the active runtime: {message}")]
    RuntimeInspection { message: String },
    #[error("root replay recording requires a stateful entry")]
    EntryNotStateful,
    #[error("root replay recording requires a durable root")]
    MissingRoot,
    #[error("root replay recording must begin before any root ingress or transition")]
    AlreadyAdvanced,
    #[error("root replay recording session identity changed")]
    SessionIdentityChanged,
    #[error("root replay recording event is invalid at transition {transition}: {message}")]
    InvalidEvent { transition: u64, message: String },
    #[error("root replay recording observed {outcomes} outcomes for only {events} queued events")]
    OutcomeCardinality { outcomes: usize, events: usize },
    #[error("root replay recording input batch was rejected before root ingress")]
    RootIngressRejected,
    #[error("root replay recording observed sequence {actual}, expected {expected}")]
    SequenceMismatch { expected: u64, actual: u64 },
    #[error("root replay recording left {count} root events unprocessed")]
    PendingEvents { count: usize },
}

impl From<arcweft_core::value::AwbcRuntimeValueSnapshotError> for RootReplayRecordingError {
    fn from(error: arcweft_core::value::AwbcRuntimeValueSnapshotError) -> Self {
        Self::Snapshot(Box::new(error))
    }
}

/// A recording failure preserves custody at the actual execution boundary.
/// The recorder is unchanged by the failed call. An executed step may already
/// have produced host work; its owned output must be handled before continuing.
#[derive(Debug, Error, PartialEq)]
pub enum RootReplayStepError {
    #[error("root replay input was rejected before execution: {error}")]
    BeforeStep {
        clock: RuntimeClockStep,
        input: Box<BundleStepInput>,
        #[source]
        error: RootReplayRecordingError,
    },
    #[error("root replay recording failed after execution: {error}")]
    AfterStep {
        step: Box<BundleSessionStep>,
        #[source]
        error: RootReplayRecordingError,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RootReplayReportV1 {
    pub entry: EntryRuntimeId,
    pub transitions_verified: usize,
    pub external_outcomes_injected: usize,
    pub suppressed_host_requests: usize,
    pub terminal_trap: bool,
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum RootReplayError {
    #[error("Root snapshot inspection failed: {0}")]
    RootState(#[source] Box<arcweft_core::root::RootRuntimeError>),
    #[error("root replay codec rejected the trace: {0}")]
    Codec(#[source] Box<arcweft_data::DataError>),
    #[error("root replay retained value admission failed: {0}")]
    Snapshot(#[source] Box<arcweft_core::value::AwbcRuntimeValueSnapshotError>),
    #[error("unsupported root replay schema version {actual}; expected exactly 1")]
    UnsupportedSchema { actual: u32 },
    #[error("root replay engine identity `{actual}` does not equal `{expected}`")]
    EngineIdentity {
        expected: &'static str,
        actual: String,
    },
    #[error("failed to inspect replay artifact: {message}")]
    ArtifactInspection { message: String },
    #[error("root replay artifact identity does not match the selected artifact")]
    ArtifactMismatch,
    #[error("root replay active locale does not match the requested locale")]
    LocaleMismatch,
    #[error("root replay locale data identity does not match this engine")]
    FormatDataMismatch,
    #[error("root replay entry `{recorded}` does not equal selected entry `{selected}`")]
    EntryMismatch { recorded: String, selected: String },
    #[error("selected root replay entry kind does not match the trace")]
    EntryKindMismatch,
    #[error("selected root replay entry binding does not match the trace")]
    BindingMismatch,
    #[error("selected root replay state identity/layout does not match the trace")]
    StateRoleMismatch,
    #[error("selected root replay event identity/layout does not match the trace")]
    EventRoleMismatch,
    #[error("selected replay entry `{entry}` is not stateful")]
    EntryNotStateful { entry: String },
    #[error("selected replay entry kind cannot be decoded")]
    InvalidEntryKind,
    #[error("failed to start root replay session: {message}")]
    SessionStart { message: String },
    #[error("root replay session did not start a durable root")]
    MissingRoot,
    #[error("root replay initializer state is invalid: {message}")]
    InvalidInitializerState { message: String },
    #[error("root replay initializer digest diverged")]
    InitializerDigestMismatch,
    #[error(
        "root replay sequence diverged for entry `{entry}` at transition {transition}: expected {expected}, actual {actual}"
    )]
    SequenceMismatch {
        entry: String,
        transition: u64,
        expected: u64,
        actual: u64,
    },
    #[error("root replay event diverged for entry `{entry}` at transition {transition}: {message}")]
    EventDivergence {
        entry: String,
        transition: u64,
        message: String,
    },
    #[error(
        "root replay outcome diverged for entry `{entry}` at transition {transition}: {message}"
    )]
    OutcomeDivergence {
        entry: String,
        transition: u64,
        message: String,
    },
    #[error(
        "root replay command diverged for entry `{entry}` at transition {transition}, command {command_index}"
    )]
    CommandDivergence {
        entry: String,
        transition: u64,
        command_index: usize,
    },
    #[error(
        "root replay external outcome `{request}` has an invalid position or root-event correlation: {message}"
    )]
    ExternalOutcome { request: String, message: String },
    #[error("root replay contains duplicate external outcome `{request}`")]
    DuplicateExternalOutcome { request: String },
    #[error("root replay trap at transition {transition} is not terminal")]
    NonTerminalTrap { transition: u64 },
    #[error("root replay has no recorded outcome for transition {transition}")]
    MissingOutcome { transition: u64 },
    #[error("root replay produced an unrecorded transition after the trace ended")]
    UnexpectedTransition,
}
