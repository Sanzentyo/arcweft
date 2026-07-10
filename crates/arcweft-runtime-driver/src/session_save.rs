//! Typed payload for quiescent bundle-session saves.
//!
//! Schema ID and version belong to the outer `arcweft-save` envelope. This
//! payload contains only restorable runtime state and one unambiguous artifact
//! binding; it deliberately has no nested schema marker or legacy identity.

use crate::display::BundlePresentationSnapshot;
use crate::swap::GenerationId;
use arcweft_bundle::container::{ArtifactIdentity, BundleDigest};
use arcweft_bundle::logical_identity::LogicalBundleIdentity;
use arcweft_core::awbc::fiber::{
    FiberAwaitManyState, FiberFrame, FiberScope, FiberScopeCleanup, FiberSourceState, FiberState,
    FiberStreamState, FiberSuspensionReason, FiberTerminalValue,
};
use arcweft_core::awbc::product_step::AwbcProductExecutorSnapshot;
use arcweft_core::engine::FlowFiberStatus;
use arcweft_core::executor::ArcweftRuntimeExecutorSnapshotError;
use arcweft_core::value::{RuntimeIterator, RuntimeSeq, RuntimeValue};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const BUNDLE_SESSION_SAVE_SCHEMA_ID: &str = "arcweft.bundle_session";
pub const BUNDLE_SESSION_SAVE_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct BundleSessionSnapshot {
    pub generation: BundleSessionGenerationSnapshot,
    pub runtime: BundleSessionRuntimeSnapshot,
    pub executor: BundleSessionExecutorSnapshot,
    pub presentation: BundlePresentationSnapshot,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BundleSessionGenerationSnapshot {
    pub active_generation: GenerationId,
    pub artifact: BundleSessionArtifactIdentity,
    pub bytecode_abi: u32,
    pub adapter_requirements: BundleDigest,
}

/// Identity required to restore a session against the artifact that created it.
///
/// Both variants cover complete state: logical bundle identity includes the
/// typed manifest and resources, while AWFB identity includes the manifest
/// digest and section content root. There is no root-only identity variant.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BundleSessionArtifactIdentity {
    LogicalBundle { identity: LogicalBundleIdentity },
    AwfbContainer { identity: ArtifactIdentity },
}

impl BundleSessionArtifactIdentity {
    #[must_use]
    pub(crate) const fn awfb_container(self) -> Option<ArtifactIdentity> {
        match self {
            Self::AwfbContainer { identity } => Some(identity),
            Self::LogicalBundle { .. } => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BundleSessionRuntimeSnapshot {
    pub source_label: String,
    pub next_step_index: u64,
    pub next_task_sequence: u64,
    pub next_generation_id: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_generation_pin: Option<GenerationId>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct BundleSessionExecutorSnapshot {
    /// Generation that owns the Product AWBC fiber state.
    pub generation: GenerationId,
    /// Current Product AWBC executor state. Other executor tiers cannot be saved.
    pub state: AwbcProductExecutorSnapshot,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BundleSessionPendingBlocker {
    PendingPresentationInputs { count: usize },
    PendingInputEvents { count: usize },
    PendingTextControlWriteBacks { count: usize },
    PendingHostCallResults { count: usize },
    WaitingActionReceiveCalls { count: usize },
    HostTasks { active: usize, queued_events: usize },
    TaskGenerationPins { count: usize },
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum BundleSessionSaveError {
    #[error("bundle session save point is not quiescent: {blockers:?}")]
    NonQuiescent {
        blockers: Vec<BundleSessionPendingBlocker>,
    },
    #[error("unsupported executor tier `{tier}` for bundle session save")]
    UnsupportedExecutorTier { tier: String },
    #[error("bundle session save generation mismatch for {field}: saved {saved}, actual {actual}")]
    GenerationMismatch {
        field: &'static str,
        saved: String,
        actual: String,
    },
    #[error("invalid presentation snapshot in session save: {message}")]
    Presentation { message: String },
    #[error("invalid Product AWBC fiber snapshot in session save: {message}")]
    Fiber { message: String },
    #[error("unsupported runtime value `{kind}` in session save at {path}")]
    UnsupportedRuntimeValue { path: String, kind: &'static str },
    #[error("session save counter `{field}` value {value} does not fit this platform")]
    CounterOutOfRange { field: &'static str, value: u64 },
    #[error("failed to encode bundle session save: {message}")]
    Encode { message: String },
    #[error("failed to decode bundle session save: {message}")]
    Decode { message: String },
}

impl From<ArcweftRuntimeExecutorSnapshotError> for BundleSessionSaveError {
    fn from(error: ArcweftRuntimeExecutorSnapshotError) -> Self {
        match error {
            ArcweftRuntimeExecutorSnapshotError::UnsupportedTier { tier }
            | ArcweftRuntimeExecutorSnapshotError::TierMismatch { actual: tier, .. } => {
                Self::UnsupportedExecutorTier {
                    tier: tier.to_owned(),
                }
            }
            ArcweftRuntimeExecutorSnapshotError::ProductAwbc { message } => Self::Fiber { message },
        }
    }
}

pub(crate) fn validate_presentation_snapshot(
    snapshot: &BundlePresentationSnapshot,
) -> Result<(), BundleSessionSaveError> {
    if let Some(dialogue) = &snapshot.dialogue {
        dialogue
            .frame()
            .validate()
            .map_err(|error| BundleSessionSaveError::Presentation {
                message: format!(
                    "dialogue occurrence {} has an invalid display frame: {error}",
                    dialogue.instance().get(),
                ),
            })?;
        if dialogue.current_stage().is_none() {
            return Err(BundleSessionSaveError::Presentation {
                message: format!(
                    "dialogue occurrence {} has out-of-range stage {} for {} stage(s)",
                    dialogue.instance().get(),
                    dialogue.stage_index().get(),
                    dialogue.frame().stage_count(),
                ),
            });
        }
    }
    let mut ids = BTreeSet::new();
    for handle in &snapshot.presentation_handles {
        if !ids.insert(handle.id.as_str().to_owned()) {
            return Err(BundleSessionSaveError::Presentation {
                message: format!("duplicate presentation handle `{}`", handle.id),
            });
        }
        if handle.created_epoch > handle.updated_epoch {
            return Err(BundleSessionSaveError::Presentation {
                message: format!(
                    "presentation handle `{}` has created_epoch {} after updated_epoch {}",
                    handle.id, handle.created_epoch, handle.updated_epoch
                ),
            });
        }
        if handle.updated_epoch > snapshot.presentation_handle_epoch {
            return Err(BundleSessionSaveError::Presentation {
                message: format!(
                    "presentation handle `{}` updated at epoch {} beyond table epoch {}",
                    handle.id, handle.updated_epoch, snapshot.presentation_handle_epoch
                ),
            });
        }
    }
    Ok(())
}

pub(crate) fn validate_presentation_runtime_status(
    snapshot: &BundlePresentationSnapshot,
    status: &FlowFiberStatus,
) -> Result<(), BundleSessionSaveError> {
    match status {
        FlowFiberStatus::Dialogue(state) => {
            let Some(dialogue) = &snapshot.dialogue else {
                return Err(BundleSessionSaveError::Presentation {
                    message: format!(
                        "runtime waits for dialogue `{}` but the presentation has no dialogue",
                        state.line
                    ),
                });
            };
            if dialogue.frame().line != state.line || !dialogue.is_waiting_for_advance() {
                return Err(BundleSessionSaveError::Presentation {
                    message: format!(
                        "runtime waits for dialogue `{}` but presentation occurrence {} retains `{}` with waiting={}",
                        state.line,
                        dialogue.instance().get(),
                        dialogue.frame().line,
                        dialogue.is_waiting_for_advance(),
                    ),
                });
            }
        }
        _ => {
            if let Some(dialogue) = &snapshot.dialogue
                && dialogue.is_waiting_for_advance()
            {
                return Err(BundleSessionSaveError::Presentation {
                    message: format!(
                        "presentation occurrence {} is actionable while runtime status is {status:?}",
                        dialogue.instance().get(),
                    ),
                });
            }
        }
    }
    Ok(())
}

pub(crate) fn validate_product_awbc_runtime_values(
    snapshot: &AwbcProductExecutorSnapshot,
) -> Result<(), BundleSessionSaveError> {
    validate_fiber_runtime_values("executor.product_awbc.fiber", &snapshot.fiber)?;
    for (index, fiber) in snapshot.child_fibers.iter().enumerate() {
        validate_fiber_runtime_values(
            &format!("executor.product_awbc.child_fibers[{index}]"),
            fiber,
        )?;
    }
    Ok(())
}

fn validate_fiber_runtime_values(
    path: &str,
    fiber: &FiberState,
) -> Result<(), BundleSessionSaveError> {
    for (index, frame) in fiber.frames.iter().enumerate() {
        validate_frame_runtime_values(&format!("{path}.frames[{index}]"), frame)?;
    }
    if let Some(suspension) = &fiber.suspension {
        validate_suspension_runtime_values(&format!("{path}.suspension"), &suspension.reason)?;
    }
    if let Some(terminal) = &fiber.terminal {
        validate_terminal_runtime_values(&format!("{path}.terminal"), terminal)?;
    }
    for (index, source) in fiber.sources.iter().enumerate() {
        validate_source_runtime_values(&format!("{path}.sources[{index}]"), source)?;
    }
    for (index, stream) in fiber.streams.iter().enumerate() {
        validate_stream_runtime_values(&format!("{path}.streams[{index}]"), stream)?;
    }
    Ok(())
}

fn validate_frame_runtime_values(
    path: &str,
    frame: &FiberFrame,
) -> Result<(), BundleSessionSaveError> {
    for (index, value) in frame.registers.iter().enumerate() {
        if let Some(value) = value {
            validate_runtime_value(&format!("{path}.registers[{index}]"), value)?;
        }
    }
    for (index, cleanup) in frame.root_cleanups.iter().enumerate() {
        validate_cleanup_runtime_values(&format!("{path}.root_cleanups[{index}]"), cleanup)?;
    }
    for (index, scope) in frame.scopes.iter().enumerate() {
        validate_scope_runtime_values(&format!("{path}.scopes[{index}]"), scope)?;
    }
    Ok(())
}

fn validate_scope_runtime_values(
    path: &str,
    scope: &FiberScope,
) -> Result<(), BundleSessionSaveError> {
    for (index, cleanup) in scope.cleanups.iter().enumerate() {
        validate_cleanup_runtime_values(&format!("{path}.cleanups[{index}]"), cleanup)?;
    }
    Ok(())
}

fn validate_cleanup_runtime_values(
    path: &str,
    cleanup: &FiberScopeCleanup,
) -> Result<(), BundleSessionSaveError> {
    for (index, value) in cleanup.args.iter().enumerate() {
        validate_runtime_value(&format!("{path}.args[{index}]"), value)?;
    }
    Ok(())
}

fn validate_suspension_runtime_values(
    path: &str,
    reason: &FiberSuspensionReason,
) -> Result<(), BundleSessionSaveError> {
    match reason {
        FiberSuspensionReason::Dialogue { .. }
        | FiberSuspensionReason::Choice { .. }
        | FiberSuspensionReason::BudgetYield => Ok(()),
        FiberSuspensionReason::Await { task, .. } => {
            validate_runtime_value(&format!("{path}.await.task"), task)
        }
        FiberSuspensionReason::AwaitMany(state) => {
            validate_await_many_runtime_values(&format!("{path}.await_many"), state)
        }
        FiberSuspensionReason::HostCall { args, .. } => {
            for (index, value) in args.iter().enumerate() {
                validate_runtime_value(&format!("{path}.host_call.args[{index}]"), value)?;
            }
            Ok(())
        }
    }
}

fn validate_await_many_runtime_values(
    path: &str,
    state: &FiberAwaitManyState,
) -> Result<(), BundleSessionSaveError> {
    for (index, value) in state.items.iter().enumerate() {
        validate_runtime_value(&format!("{path}.items[{index}]"), value)?;
    }
    for (index, value) in state.results.iter().enumerate() {
        if let Some(value) = value {
            validate_runtime_value(&format!("{path}.results[{index}]"), value)?;
        }
    }
    Ok(())
}

fn validate_terminal_runtime_values(
    path: &str,
    terminal: &FiberTerminalValue,
) -> Result<(), BundleSessionSaveError> {
    match terminal {
        FiberTerminalValue::Returned(Some(value)) => {
            validate_runtime_value(&format!("{path}.returned"), value)
        }
        FiberTerminalValue::Returned(None) | FiberTerminalValue::Trapped(_) => Ok(()),
    }
}

fn validate_source_runtime_values(
    path: &str,
    source: &FiberSourceState,
) -> Result<(), BundleSessionSaveError> {
    for (index, value) in source.queue.iter().enumerate() {
        validate_runtime_value(&format!("{path}.queue[{index}]"), value)?;
    }
    if let Some(value) = &source.last_error {
        validate_runtime_value(&format!("{path}.last_error"), value)?;
    }
    Ok(())
}

fn validate_stream_runtime_values(
    path: &str,
    stream: &FiberStreamState,
) -> Result<(), BundleSessionSaveError> {
    for (index, value) in stream.queue.iter().enumerate() {
        validate_runtime_value(&format!("{path}.queue[{index}]"), value)?;
    }
    Ok(())
}

fn validate_runtime_value(path: &str, value: &RuntimeValue) -> Result<(), BundleSessionSaveError> {
    match value {
        RuntimeValue::Function(_) => Err(BundleSessionSaveError::UnsupportedRuntimeValue {
            path: path.to_owned(),
            kind: "function",
        }),
        RuntimeValue::Tuple(items) => {
            for (index, value) in items.iter().enumerate() {
                validate_runtime_value(&format!("{path}.tuple[{index}]"), value)?;
            }
            Ok(())
        }
        RuntimeValue::Seq(sequence) => validate_runtime_sequence(path, sequence),
        RuntimeValue::Record(fields) => {
            for (index, field) in fields.iter().enumerate() {
                validate_runtime_value(&format!("{path}.record[{index}].value"), &field.value)?;
            }
            Ok(())
        }
        RuntimeValue::Iterator(iterator) => validate_runtime_iterator(path, iterator),
        RuntimeValue::Variant {
            payload: Some(payload),
            ..
        } => validate_runtime_value(&format!("{path}.variant.payload"), payload),
        RuntimeValue::Variant { payload: None, .. }
        | RuntimeValue::Unit
        | RuntimeValue::Bool(_)
        | RuntimeValue::Int(_)
        | RuntimeValue::UInt(_)
        | RuntimeValue::F32(_)
        | RuntimeValue::F64(_)
        | RuntimeValue::MatrixF32(_)
        | RuntimeValue::MatrixF64(_)
        | RuntimeValue::TensorF32(_)
        | RuntimeValue::TensorF64(_)
        | RuntimeValue::String(_)
        | RuntimeValue::Char(_)
        | RuntimeValue::Duration(_)
        | RuntimeValue::Range(_)
        | RuntimeValue::EntityRef(_) => Ok(()),
    }
}

fn validate_runtime_sequence(
    path: &str,
    sequence: &RuntimeSeq,
) -> Result<(), BundleSessionSaveError> {
    match sequence {
        RuntimeSeq::Values(values) => {
            for (index, value) in values.iter().enumerate() {
                validate_runtime_value(&format!("{path}.seq[{index}]"), value)?;
            }
            Ok(())
        }
        RuntimeSeq::TupleColumns(columns) => {
            for (index, column) in columns.columns().iter().enumerate() {
                validate_runtime_sequence(&format!("{path}.tuple_columns[{index}]"), column)?;
            }
            Ok(())
        }
        RuntimeSeq::RecordColumns(records) => {
            for (index, field) in records.fields().iter().enumerate() {
                validate_runtime_sequence(
                    &format!("{path}.record_columns[{index}]"),
                    &field.values,
                )?;
            }
            Ok(())
        }
        RuntimeSeq::Dense(_) => Ok(()),
    }
}

fn validate_runtime_iterator(
    path: &str,
    iterator: &RuntimeIterator,
) -> Result<(), BundleSessionSaveError> {
    match iterator {
        RuntimeIterator::Values { items, .. } => {
            for (index, value) in items.iter().enumerate() {
                validate_runtime_value(&format!("{path}.iterator.items[{index}]"), value)?;
            }
            Ok(())
        }
        RuntimeIterator::Witness { state, .. } => {
            validate_runtime_value(&format!("{path}.iterator.witness.state"), state)
        }
        RuntimeIterator::Range(_) => Ok(()),
    }
}

pub(crate) fn digest_label(value: &BundleDigest) -> String {
    format!("{value:?}")
}
