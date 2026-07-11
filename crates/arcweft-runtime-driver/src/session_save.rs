//! Typed payload for quiescent bundle-session saves.
//!
//! Schema ID and version belong to the outer `arcweft-save` envelope. This
//! payload contains only restorable runtime state and one unambiguous artifact
//! binding; it deliberately has no nested schema marker or legacy identity.

use crate::display::BundlePresentationSnapshot;
use crate::swap::GenerationId;
use arcweft_bundle::container::{ArtifactIdentity, BundleDigest};
use arcweft_bundle::fx_definitions::FxDefinitions;
use arcweft_bundle::logical_identity::LogicalBundleIdentity;
use arcweft_core::awbc::fiber::{FiberState, FiberStateError};
use arcweft_core::awbc::product_step::AwbcProductExecutorSnapshot;
use arcweft_core::awbc::schema::AwbcProgram;
use arcweft_core::engine::FlowFiberStatus;
use arcweft_core::executor::ArcweftRuntimeExecutorSnapshotError;
use arcweft_presentation::fx::FxDiagnostic;
use arcweft_view::virtualization::ViewVirtualizationSnapshot;
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
    /// Exact per-mount range/scroll state, including off-window item descriptors.
    pub view_virtualization: ViewVirtualizationSnapshot,
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
    #[error("invalid Fx runtime snapshot in session save: {diagnostic:?}")]
    Fx { diagnostic: Box<FxDiagnostic> },
    #[error("invalid Product AWBC fiber snapshot in session save: {message}")]
    Fiber { message: String },
    #[error("invalid runtime value in session save at {path}: {message}")]
    InvalidRuntimeValue { path: String, message: String },
    #[error("invalid retained View virtualization snapshot: {message}")]
    ViewVirtualization { message: String },
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
    definitions: &FxDefinitions,
) -> Result<(), BundleSessionSaveError> {
    snapshot
        .fx
        .validate_for_definitions(definitions)
        .map_err(|error| BundleSessionSaveError::Fx {
            diagnostic: Box::new(error.diagnostic()),
        })?;
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

pub(crate) fn validate_product_awbc_snapshot(
    snapshot: &AwbcProductExecutorSnapshot,
    program: &AwbcProgram,
) -> Result<(), BundleSessionSaveError> {
    validate_fiber_snapshot("executor.product_awbc.fiber", &snapshot.fiber, program)?;
    for (index, fiber) in snapshot.child_fibers.iter().enumerate() {
        validate_fiber_snapshot(
            &format!("executor.product_awbc.child_fibers[{index}]"),
            fiber,
            program,
        )?;
    }
    Ok(())
}

fn validate_fiber_snapshot(
    path: &str,
    fiber: &FiberState,
    program: &AwbcProgram,
) -> Result<(), BundleSessionSaveError> {
    fiber
        .validate_for_program(program)
        .map_err(|error| match error {
            FiberStateError::InvalidRuntimeValue {
                path: value_path,
                reason,
            } => BundleSessionSaveError::InvalidRuntimeValue {
                path: format!("{path}.{value_path}"),
                message: reason,
            },
            error => BundleSessionSaveError::Fiber {
                message: format!("{path}: {error}"),
            },
        })
}

pub(crate) fn digest_label(value: &BundleDigest) -> String {
    format!("{value:?}")
}
