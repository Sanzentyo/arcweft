//! Typed payload for quiescent bundle-session saves.
//!
//! Schema ID and version belong to the outer `arcweft-save` envelope. This
//! payload contains only restorable runtime state and one unambiguous artifact
//! binding; it deliberately has no nested schema marker or legacy identity.

use crate::display::{ActiveSessionLocale, BundlePresentationSnapshot};
use crate::view_runtime::BundleViewRuntimeSnapshot;
use arcweft_bundle::container::{ArtifactIdentity, BundleDigest};
use arcweft_bundle::fx_definitions::FxDefinitions;
use arcweft_bundle::logical_identity::LogicalBundleIdentity;
use arcweft_character::presentation_name::{
    CharacterPresentationLocalePolicyDigest, CharacterPresentationSemanticDigest,
};
use arcweft_core::awbc::fiber::{FiberState, FiberStateError};
use arcweft_core::awbc::product_step::{
    AwbcProductExecutorSaveSnapshot, AwbcProductExecutorSnapshot,
};
use arcweft_core::awbc::schema::AwbcProgram;
use arcweft_core::engine::FlowFiberStatus;
pub use arcweft_core::entry::ActiveEntrySnapshotV1;
use arcweft_core::executor::ArcweftRuntimeExecutorSnapshotError;
pub use arcweft_core::root::RootStateSnapshotV1;
use arcweft_core::task::GenerationId;
use arcweft_presentation::fx::FxDiagnostic;
use arcweft_view::{ViewId, virtualization::ViewVirtualizationSnapshot};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const BUNDLE_SESSION_SAVE_SCHEMA_ID: &str = "arcweft.bundle_session";
pub const BUNDLE_SESSION_SAVE_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct BundleSessionSnapshot {
    pub generation: BundleSessionGenerationSnapshot,
    #[serde(deserialize_with = "deserialize_required_option")]
    pub character_presentation: Option<BundleSessionCharacterPresentationSnapshot>,
    pub active_entry: ActiveEntrySnapshotV1,
    pub root: Option<RootStateSnapshotV1>,
    pub runtime: BundleSessionRuntimeSnapshot,
    pub executor: BundleSessionExecutorSnapshot,
    pub presentation: BundlePresentationSnapshot,
    /// Exact per-mount range/scroll state, including off-window item descriptors.
    pub view_virtualization: ViewVirtualizationSnapshot,
    /// Exact executable View mount graph, typed slots, clocks, and allocator cursor.
    pub view_runtime: BundleViewRuntimeSnapshot,
}

/// Wire payload for the bundle-session save envelope.
///
/// This is intentionally separate from [`BundleSessionSnapshot`].  The
/// latter is a live, testable in-memory state projection; this payload owns
/// the AWBC-only recursive value DTO and never asks serde to materialize a
/// live `RuntimeValue` function.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BundleSessionSavePayload {
    pub generation: BundleSessionGenerationSnapshot,
    #[serde(deserialize_with = "deserialize_required_option")]
    pub character_presentation: Option<BundleSessionCharacterPresentationSnapshot>,
    pub active_entry: ActiveEntrySnapshotV1,
    pub root: Option<RootStateSnapshotV1>,
    pub runtime: BundleSessionRuntimeSnapshot,
    pub executor: BundleSessionExecutorSavePayload,
    pub presentation: BundlePresentationSnapshot,
    pub view_virtualization: ViewVirtualizationSnapshot,
    pub view_runtime: BundleViewRuntimeSnapshot,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BundleSessionExecutorSavePayload {
    pub generation: GenerationId,
    pub state: AwbcProductExecutorSaveSnapshot,
}

impl BundleSessionSavePayload {
    pub(crate) fn from_snapshot(snapshot: &BundleSessionSnapshot) -> Result<Self, String> {
        Ok(Self {
            generation: snapshot.generation.clone(),
            character_presentation: snapshot.character_presentation.clone(),
            active_entry: snapshot.active_entry.clone(),
            root: snapshot.root.clone(),
            runtime: snapshot.runtime.clone(),
            executor: BundleSessionExecutorSavePayload {
                generation: snapshot.executor.generation,
                state: AwbcProductExecutorSaveSnapshot::from_live(&snapshot.executor.state)?,
            },
            presentation: snapshot.presentation.clone(),
            view_virtualization: snapshot.view_virtualization.clone(),
            view_runtime: snapshot.view_runtime.clone(),
        })
    }

    pub(crate) fn into_snapshot(self) -> Result<BundleSessionSnapshot, String> {
        Ok(BundleSessionSnapshot {
            generation: self.generation,
            character_presentation: self.character_presentation,
            active_entry: self.active_entry,
            root: self.root,
            runtime: self.runtime,
            executor: BundleSessionExecutorSnapshot {
                generation: self.executor.generation,
                state: self.executor.state.into_live()?,
            },
            presentation: self.presentation,
            view_virtualization: self.view_virtualization,
            view_runtime: self.view_runtime,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BundleSessionCharacterPresentationSnapshot {
    pub active_locale: ActiveSessionLocale,
    pub semantic_digest: CharacterPresentationSemanticDigest,
    pub locale_policy_digest: CharacterPresentationLocalePolicyDigest,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BundleSessionGenerationSnapshot {
    pub active_generation: GenerationId,
    pub artifact: BundleSessionArtifactIdentity,
    pub dialogue_content: BundleDigest,
    pub awbc_abi: u32,
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
    #[serde(deserialize_with = "deserialize_required_option")]
    pub runtime_generation_pin: Option<GenerationId>,
}

fn deserialize_required_option<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
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
    TransientDialogueViewOwners { views: Vec<ViewId> },
    PendingPresentationInputs { count: usize },
    PendingInputEvents { count: usize },
    PendingTextControlWriteBacks { count: usize },
    PendingHostCallResults { count: usize },
    WaitingActionReceiveCalls { count: usize },
    HostTasks { active: usize, queued_events: usize },
    TaskGenerationPins { count: usize },
    ReducerTransactionActive,
    PendingRootEvents { count: usize },
    PendingRootCommands { count: u32 },
}

impl BundleSessionPendingBlocker {
    #[must_use]
    pub const fn category(&self) -> &'static str {
        match self {
            Self::TransientDialogueViewOwners { .. } => "presentation",
            Self::ReducerTransactionActive
            | Self::PendingRootEvents { .. }
            | Self::PendingRootCommands { .. } => "root",
            Self::PendingPresentationInputs { .. }
            | Self::PendingInputEvents { .. }
            | Self::PendingTextControlWriteBacks { .. } => "input",
            Self::PendingHostCallResults { .. } | Self::WaitingActionReceiveCalls { .. } => "host",
            Self::HostTasks { .. } | Self::TaskGenerationPins { .. } => "task",
        }
    }
}

impl std::fmt::Display for BundleSessionPendingBlocker {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TransientDialogueViewOwners { views } => {
                write!(
                    formatter,
                    "{} transient dialogue View owners are active: {views:?}",
                    views.len()
                )
            }
            Self::PendingPresentationInputs { count } => {
                write!(formatter, "{count} pending presentation inputs")
            }
            Self::PendingInputEvents { count } => {
                write!(formatter, "{count} pending input events")
            }
            Self::PendingTextControlWriteBacks { count } => {
                write!(formatter, "{count} pending text-control write-backs")
            }
            Self::PendingHostCallResults { count } => {
                write!(formatter, "{count} pending host-call results")
            }
            Self::WaitingActionReceiveCalls { count } => {
                write!(formatter, "{count} waiting action receives")
            }
            Self::HostTasks {
                active,
                queued_events,
            } => write!(
                formatter,
                "{active} active host tasks and {queued_events} queued task events"
            ),
            Self::TaskGenerationPins { count } => {
                write!(formatter, "{count} task generation pins")
            }
            Self::ReducerTransactionActive => formatter.write_str("root reducer is active"),
            Self::PendingRootEvents { count } => {
                write!(formatter, "{count} pending root events")
            }
            Self::PendingRootCommands { count } => {
                write!(formatter, "{count} pending root commands")
            }
        }
    }
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
    #[error("invalid Character presentation snapshot in session save: {message}")]
    CharacterPresentation { message: String },
    #[error("invalid Fx runtime snapshot in session save: {diagnostic:?}")]
    Fx { diagnostic: Box<FxDiagnostic> },
    #[error("invalid Product AWBC fiber snapshot in session save: {message}")]
    Fiber { message: String },
    #[error("invalid root-state session snapshot: {message}")]
    Root { message: String },
    #[error("invalid runtime value in session save at {path}: {message}")]
    InvalidRuntimeValue { path: String, message: String },
    #[error("invalid retained View virtualization snapshot: {message}")]
    ViewVirtualization { message: String },
    #[error("invalid executable View runtime snapshot: {message}")]
    ViewRuntime { message: String },
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
    snapshot
        .dialogue
        .validate()
        .map_err(|error| BundleSessionSaveError::Presentation {
            message: error.to_string(),
        })?;
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
            let mut waiting = snapshot.dialogue.waiting_entries();
            let Some((presentation, dialogue)) = waiting.next() else {
                return Err(BundleSessionSaveError::Presentation {
                    message: format!(
                        "runtime waits for dialogue `{}` but the presentation has no dialogue",
                        state.line
                    ),
                });
            };
            if waiting.next().is_some() {
                return Err(BundleSessionSaveError::Presentation {
                    message: "runtime has more than one actionable dialogue occurrence".to_owned(),
                });
            }
            if dialogue.frame().line != state.line {
                return Err(BundleSessionSaveError::Presentation {
                    message: format!(
                        "runtime waits for dialogue `{}` but presentation {} occurrence {} retains `{}`",
                        state.line,
                        presentation.id().get(),
                        dialogue.instance().get(),
                        dialogue.frame().line,
                    ),
                });
            }
        }
        _ => {
            if let Some((presentation, dialogue)) = snapshot.dialogue.waiting_entries().next() {
                return Err(BundleSessionSaveError::Presentation {
                    message: format!(
                        "dialogue presentation {} occurrence {} is actionable while runtime status is {status:?}",
                        presentation.id().get(),
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
            &fiber.fiber,
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
