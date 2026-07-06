use crate::display::BundlePresentationSnapshot;
use crate::swap::GenerationId;
use arcweft_bundle::container::BundleDigest;
use arcweft_core::awbc::product_step::AwbcProductExecutorSnapshot;
use arcweft_core::executor::{ArcweftExecutionTier, ArcweftRuntimeExecutorSnapshotError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const BUNDLE_SESSION_SAVE_SCHEMA_ID: &str = "arcweft.bundle_session";
pub const BUNDLE_SESSION_SAVE_SCHEMA_VERSION: u32 = 1;
pub const BUNDLE_SESSION_SAVE_CODEC_ID: &str = arcweft_save::TYPED_JSON_CODEC_ID;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BundleSessionSaveSchema {
    pub id: String,
    pub version: u32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct BundleSessionSnapshot {
    pub schema: BundleSessionSaveSchema,
    pub generation: BundleSessionGenerationSnapshot,
    pub runtime: BundleSessionRuntimeSnapshot,
    pub executor: BundleSessionExecutorSnapshot,
    pub presentation: BundlePresentationSnapshot,
    pub pending: BundleSessionPendingSnapshot,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BundleSessionGenerationSnapshot {
    pub active_generation: GenerationId,
    pub content_root: BundleDigest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_container_content_root: Option<BundleDigest>,
    pub bytecode_abi: u32,
    pub adapter_requirements: BundleDigest,
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
#[serde(tag = "tier", rename_all = "snake_case")]
pub enum BundleSessionExecutorSnapshot {
    ProductAwbc {
        generation: GenerationId,
        state: Box<AwbcProductExecutorSnapshot>,
    },
    StructuredVm,
    StructuredAot,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BundleSessionPendingSnapshot {
    Quiescent,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BundleSessionPendingBlocker {
    PendingInputEvents { count: usize },
    PendingTextControlWriteBacks { count: usize },
    PendingHostCallResults { count: usize },
    WaitingActionReceiveCalls { count: usize },
    HostTasks { active: usize, queued_events: usize },
    TaskGenerationPins { count: usize },
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum BundleSessionSaveError {
    #[error("bundle session save schema id `{actual}` does not match expected `{expected}`")]
    SchemaId { actual: String, expected: String },
    #[error("bundle session save schema version {actual} is not supported; expected {expected}")]
    SchemaVersion { actual: u32, expected: u32 },
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
    #[error("session save counter `{field}` value {value} does not fit this platform")]
    CounterOutOfRange { field: &'static str, value: u64 },
    #[error("failed to encode bundle session save: {message}")]
    Encode { message: String },
    #[error("failed to decode bundle session save: {message}")]
    Decode { message: String },
}

impl Default for BundleSessionSaveSchema {
    fn default() -> Self {
        Self {
            id: BUNDLE_SESSION_SAVE_SCHEMA_ID.to_owned(),
            version: BUNDLE_SESSION_SAVE_SCHEMA_VERSION,
        }
    }
}

impl BundleSessionSaveSchema {
    pub fn validate(&self) -> Result<(), BundleSessionSaveError> {
        if self.id != BUNDLE_SESSION_SAVE_SCHEMA_ID {
            return Err(BundleSessionSaveError::SchemaId {
                actual: self.id.clone(),
                expected: BUNDLE_SESSION_SAVE_SCHEMA_ID.to_owned(),
            });
        }
        if self.version != BUNDLE_SESSION_SAVE_SCHEMA_VERSION {
            return Err(BundleSessionSaveError::SchemaVersion {
                actual: self.version,
                expected: BUNDLE_SESSION_SAVE_SCHEMA_VERSION,
            });
        }
        Ok(())
    }
}

impl BundleSessionExecutorSnapshot {
    #[must_use]
    pub const fn execution_tier(&self) -> ArcweftExecutionTier {
        match self {
            Self::ProductAwbc { .. } => ArcweftExecutionTier::AwbcProduct,
            Self::StructuredVm => ArcweftExecutionTier::StructuredVm,
            Self::StructuredAot => ArcweftExecutionTier::StructuredAot,
        }
    }
}

impl BundleSessionPendingSnapshot {
    #[must_use]
    pub const fn quiescent() -> Self {
        Self::Quiescent
    }

    #[must_use]
    pub const fn is_quiescent(&self) -> bool {
        matches!(self, Self::Quiescent)
    }
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

pub(crate) fn digest_label(value: &BundleDigest) -> String {
    format!("{value:?}")
}
