//! Typed payload for quiescent bundle-session saves.
//!
//! Schema ID and version belong to the outer `arcweft-save` envelope. This
//! payload contains only restorable runtime state and one unambiguous artifact
//! binding; it deliberately has no nested schema marker or legacy identity.

use crate::display::BundlePresentationSnapshot;
use crate::task::RuntimeTaskStatus;
use crate::view_runtime::{BundleViewRuntimeSaveSnapshot, BundleViewRuntimeSnapshot};
use arcweft_bundle::container::BundleDigest;
use arcweft_bundle::fx_definitions::FxDefinitions;
use arcweft_character::presentation_name::CharacterPresentationSemanticDigest;
use arcweft_core::awbc::product_step::{
    AwbcProductExecutorSaveSnapshot, AwbcProductExecutorSnapshot,
};
use arcweft_core::engine::FlowFiberStatus;
pub use arcweft_core::entry::ActiveEntrySnapshotV1;
use arcweft_core::executor::ArcweftRuntimeExecutorSnapshotError;
use arcweft_core::task::{
    GenerationId, LogicalEpoch, NeedId, RuntimeProgramOwner, TaskId, TaskPublicationRevision,
    TaskSequence,
};
use arcweft_core::value::RuntimeBundleAssetOpaqueRole;
use arcweft_id::LocaleTag;
use arcweft_presentation::fx::FxDiagnostic;
use arcweft_view::{ViewId, virtualization::ViewVirtualizationSnapshot};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub use crate::swap::BundleSessionArtifactIdentity;

pub const BUNDLE_SESSION_SAVE_SCHEMA_ID: &str = "arcweft.bundle_session";
pub const BUNDLE_SESSION_SAVE_SCHEMA_VERSION: u32 = 1;

/// One session storage grammar. The live root has neither Clone nor a wire
/// reader; only the inert specialization can be copied and decoded.
///
/// ```compile_fail
/// use arcweft_runtime_driver::session_save::BundleSessionSnapshot;
/// fn copy(value: &BundleSessionSnapshot) { let _: BundleSessionSnapshot = value.clone(); }
/// ```
/// ```compile_fail
/// use arcweft_runtime_driver::session_save::BundleSessionSnapshot;
/// let _: BundleSessionSnapshot = serde_json::from_str("{}").unwrap();
/// ```
/// ```compile_fail
/// use arcweft_runtime_driver::session_save::BundleSessionSnapshot;
/// fn encode(value: &BundleSessionSnapshot) { serde_json::to_vec(value).unwrap(); }
/// ```
#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BundleSessionSnapshot<
    P = BundlePresentationSnapshot,
    V = BundleViewRuntimeSnapshot,
    S = AwbcProductExecutorSnapshot,
> {
    pub generation: BundleSessionGenerationSnapshot,
    pub active_locale: LocaleTag,
    #[serde(deserialize_with = "deserialize_required_option")]
    pub character_presentation: Option<BundleSessionCharacterPresentationSnapshot>,
    pub active_entry: ActiveEntrySnapshotV1,
    pub runtime: BundleSessionRuntimeSnapshot,
    pub executor: BundleSessionExecutorSnapshot<S>,
    pub presentation: P,
    /// Exact per-mount range/scroll state, including off-window item descriptors.
    pub view_virtualization: ViewVirtualizationSnapshot,
    /// Exact executable View mount graph, typed slots, clocks, and allocator cursor.
    pub view_runtime: V,
}

/// Wire payload for the bundle-session save envelope.
///
/// The in-memory snapshot also retains the inert AWBC value DTO. Decoding a
/// save never creates a second live affine owner beside the active executor.
pub(crate) type BundleSessionSavePayload = BundleSessionSnapshot<
    BundlePresentationSnapshot<arcweft_text_model::LineDisplayFrameSnapshot>,
    BundleViewRuntimeSaveSnapshot,
    AwbcProductExecutorSaveSnapshot,
>;

pub(crate) type BundleSessionBoundSavePayload = BundleSessionSnapshot<
    BundlePresentationSnapshot<arcweft_text_model::LineDisplayFrameSnapshot>,
    BundleViewRuntimeSaveSnapshot,
>;

pub(crate) type BundleSessionExecutorSavePayload =
    BundleSessionExecutorSnapshot<AwbcProductExecutorSaveSnapshot>;

impl<P, V> BundleSessionSnapshot<P, V> {
    pub(crate) fn admit_executor(
        &self,
        owner: &RuntimeProgramOwner,
        admission: &mut arcweft_core::value::RuntimeRetainedValueAdmission,
    ) -> Result<(), BundleSessionSaveError> {
        let RuntimeProgramOwner::Awbc(program) = owner else {
            return Err(BundleSessionSaveError::Root {
                message: "session snapshot requires its original Product AWBC owner".to_owned(),
            });
        };
        if self.executor.generation != self.generation.active_generation {
            return Err(BundleSessionSaveError::GenerationMismatch {
                field: "executor_generation",
                saved: format!("{:?}", self.executor.generation),
                actual: format!("{:?}", self.generation.active_generation),
            });
        }
        if self.executor.state.runtime_generation > self.executor.generation {
            return Err(BundleSessionSaveError::GenerationMismatch {
                field: "executor_runtime_generation",
                saved: format!("{:?}", self.executor.state.runtime_generation),
                actual: format!("at most {:?}", self.executor.generation),
            });
        }
        if match self.runtime.runtime_generation_pin {
            Some(pin) => {
                pin != self.executor.generation || pin != self.executor.state.runtime_generation
            }
            None => self.executor.state.requires_runtime_generation_pin(),
        } {
            return Err(BundleSessionSaveError::GenerationMismatch {
                field: "runtime_generation_pin",
                saved: format!("{:?}", self.runtime.runtime_generation_pin),
                actual: format!(
                    "execution {:?} in storage {:?}",
                    self.executor.state.runtime_generation, self.executor.generation
                ),
            });
        }
        let selected = self
            .executor
            .state
            .active_entry_snapshot_identity(program)
            .map_err(|error| BundleSessionSaveError::Root {
                message: error.to_string(),
            })?;
        if selected != self.active_entry {
            return Err(BundleSessionSaveError::Root {
                message: "session active entry differs from its complete executor snapshot"
                    .to_owned(),
            });
        }
        self.executor
            .state
            .admit_retained(owner, admission)
            .map_err(|error| {
                BundleSessionSaveError::from(ArcweftRuntimeExecutorSnapshotError::from(error))
            })
    }
}

impl Clone for BundleSessionSavePayload {
    fn clone(&self) -> Self {
        Self {
            generation: self.generation.clone(),
            active_locale: self.active_locale.clone(),
            character_presentation: self.character_presentation.clone(),
            active_entry: self.active_entry.clone(),
            runtime: self.runtime.clone(),
            executor: self.executor.clone(),
            presentation: self.presentation.clone(),
            view_virtualization: self.view_virtualization.clone(),
            view_runtime: self.view_runtime.clone(),
        }
    }
}

impl BundleSessionSavePayload {
    pub(crate) fn from_snapshot(
        snapshot: &BundleSessionSnapshot,
        program: &arcweft_core::awbc::product_step::AwbcProductProgram,
        dialogue_schema: Option<&arcweft_dialogue::CharacterDialogueRuntimeSchema>,
    ) -> Result<Self, String> {
        let program_owner = &RuntimeProgramOwner::Awbc(program.program_arc());
        let mut admission = program
            .retained_value_admission(arcweft_core::entry::RuntimeSchemaLimits::engine_default());
        snapshot
            .runtime
            .admit_metadata(&mut admission)
            .map_err(|error| error.to_string())?;
        snapshot
            .view_runtime
            .admit_resources(&mut admission)
            .map_err(|error| error.to_string())?;
        snapshot
            .admit_executor(program_owner, &mut admission)
            .map_err(|error| error.to_string())?;
        snapshot
            .presentation
            .admit_retained(&mut admission, dialogue_schema)?;
        Ok(Self {
            generation: snapshot.generation.clone(),
            active_locale: snapshot.active_locale.clone(),
            character_presentation: snapshot.character_presentation.clone(),
            active_entry: snapshot.active_entry.clone(),
            runtime: snapshot.runtime.clone(),
            executor: BundleSessionExecutorSavePayload {
                generation: snapshot.executor.generation,
                state: AwbcProductExecutorSaveSnapshot::from_live(
                    &snapshot.executor.state,
                    program,
                )
                .map_err(|error| error.to_string())?,
            },
            presentation: snapshot.presentation.clone().try_map_frames(|frame| {
                if dialogue_schema.is_some() && frame.effective.admitted().is_none() {
                    return Err(
                        "session display frame has no generation-admitted CharacterDialogue"
                            .to_owned(),
                    );
                }
                arcweft_text_model::LineDisplayFrameSnapshot::from_live(
                    &frame,
                    Some(program_owner),
                    dialogue_schema,
                )
            })?,
            view_virtualization: snapshot.view_virtualization.clone(),
            view_runtime: BundleViewRuntimeSaveSnapshot::from_live(
                &snapshot.view_runtime,
                program_owner,
            )?,
        })
    }

    pub(crate) fn bind_executor(
        self,
        context: &arcweft_core::executor::ArcweftRuntimeExecutor,
    ) -> Result<BundleSessionBoundSavePayload, BundleSessionSaveError> {
        let state = context
            .bind_product_save_snapshot(self.executor.state)
            .map_err(BundleSessionSaveError::from)?;
        Ok(BundleSessionBoundSavePayload {
            generation: self.generation,
            active_locale: self.active_locale,
            character_presentation: self.character_presentation,
            active_entry: self.active_entry,
            runtime: self.runtime,
            executor: BundleSessionExecutorSnapshot {
                generation: self.executor.generation,
                state,
            },
            presentation: self.presentation,
            view_virtualization: self.view_virtualization,
            view_runtime: self.view_runtime,
        })
    }
}

impl BundleSessionBoundSavePayload {
    pub(crate) fn admit_retained(
        &self,
        program_owner: &RuntimeProgramOwner,
        admission: &mut arcweft_core::value::RuntimeRetainedValueAdmission,
        dialogue_schema: Option<&arcweft_dialogue::CharacterDialogueRuntimeSchema>,
    ) -> Result<(), BundleSessionSaveError> {
        self.runtime
            .admit_metadata(admission)
            .map_err(|error| BundleSessionSaveError::Decode {
                message: error.to_string(),
            })?;
        self.view_runtime
            .admit_resources(admission)
            .map_err(|error| BundleSessionSaveError::Decode {
                message: error.to_string(),
            })?;
        self.admit_executor(program_owner, admission)?;
        self.presentation
            .try_visit_frames(|frame| frame.admit_retained_metadata(admission))
            .map_err(|message| BundleSessionSaveError::Decode { message })?;
        self.presentation
            .try_visit_frames(|frame| frame.admit_retained_values(admission, dialogue_schema))
            .map_err(|message| BundleSessionSaveError::Decode { message })
    }

    pub(crate) fn into_admitted_snapshot(
        self,
        program_owner: &RuntimeProgramOwner,
        dialogue_schema: Option<&arcweft_dialogue::CharacterDialogueRuntimeSchema>,
    ) -> Result<BundleSessionSnapshot, String> {
        Ok(BundleSessionSnapshot {
            generation: self.generation,
            active_locale: self.active_locale,
            character_presentation: self.character_presentation,
            active_entry: self.active_entry,
            runtime: self.runtime,
            executor: BundleSessionExecutorSnapshot {
                generation: self.executor.generation,
                state: self.executor.state,
            },
            presentation: self.presentation.try_map_frames(|frame| {
                let frame = frame.into_live(Some(program_owner), dialogue_schema)?;
                if dialogue_schema.is_some() && frame.effective.admitted().is_none() {
                    return Err(
                        "session display frame has no generation-admitted CharacterDialogue"
                            .to_owned(),
                    );
                }
                Ok(frame)
            })?,
            view_virtualization: self.view_virtualization,
            view_runtime: self.view_runtime.into_live(program_owner)?,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BundleSessionCharacterPresentationSnapshot {
    pub semantic_digest: CharacterPresentationSemanticDigest,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BundleSessionGenerationSnapshot {
    pub active_generation: GenerationId,
    pub artifact: BundleSessionArtifactIdentity,
    pub dialogue_content: BundleDigest,
    pub awbc_abi: u32,
    pub adapter_requirements: BundleDigest,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BundleSessionRuntimeSnapshot {
    pub source_label: String,
    pub next_step_index: u64,
    pub next_task_sequence: u64,
    pub next_dialogue_input_sequence: u64,
    pub next_generation_id: u64,
    #[serde(deserialize_with = "deserialize_required_option")]
    pub runtime_generation_pin: Option<GenerationId>,
    /// Driver-owned dispatch tuple for each active, Product-verified Restartable Need.
    pub restartable_tasks: Vec<BundleSessionTaskDispatchSnapshot>,
}

impl BundleSessionRuntimeSnapshot {
    pub(crate) fn admit_metadata(
        &self,
        admission: &mut arcweft_core::value::RuntimeRetainedValueAdmission,
    ) -> Result<(), arcweft_core::value::AwbcRuntimeValueSnapshotError> {
        admission.validate_metadata_string(&self.source_label)?;
        admission.validate_metadata_collection(self.restartable_tasks.len())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BundleSessionTaskDispatchSnapshot {
    pub correlation: arcweft_core::task::TaskCorrelation,
    pub logical_epoch: LogicalEpoch,
    pub sequence: TaskSequence,
    pub last_publication_revision: Option<TaskPublicationRevision>,
    pub status: RuntimeTaskStatus,
}

fn deserialize_required_option<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BundleSessionExecutorSnapshot<S = AwbcProductExecutorSnapshot> {
    /// Generation selected for session storage. A completed executor may retain
    /// its older execution generation without owning a runnable generation pin.
    pub generation: GenerationId,
    /// Inert Product AWBC state. It becomes live only after the old executor
    /// has relinquished sole ownership during restore.
    pub state: S,
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
    #[error("session save requires quiescence for active Need identities {needs:?}")]
    NeedsQuiescence { needs: Vec<NeedId> },
    #[error("invalid Restartable Need dispatch in session save: {message}")]
    TaskDispatch { message: String },
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
    #[error("bundle asset value {role:?} requires a catalog validator before session restore")]
    BundleAssetCatalogValidatorRequired { role: RuntimeBundleAssetOpaqueRole },
    #[error("saved runtime value failed catalog validation: {message}")]
    RuntimeValueValidation { message: String },
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
            ArcweftRuntimeExecutorSnapshotError::NeedsQuiescence { needs } => {
                Self::NeedsQuiescence { needs }
            }
            ArcweftRuntimeExecutorSnapshotError::ProductAwbc { message } => Self::Fiber { message },
            ArcweftRuntimeExecutorSnapshotError::Root { message } => Self::Root { message },
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
        FlowFiberStatus::Dialogue(dialogue_status)
            if dialogue_status.waiting_presentation_activation().is_some() =>
        {
            let activation = dialogue_status.activation();
            let mut waiting = snapshot.dialogue.waiting_entries();
            let Some((presentation, dialogue)) = waiting.next() else {
                return Err(BundleSessionSaveError::Presentation {
                    message: format!(
                        "runtime waits for dialogue activation {activation:?} but the presentation has no dialogue"
                    ),
                });
            };
            if waiting.next().is_some() {
                return Err(BundleSessionSaveError::Presentation {
                    message: "runtime has more than one actionable dialogue occurrence".to_owned(),
                });
            }
            if dialogue.activation() != activation {
                return Err(BundleSessionSaveError::Presentation {
                    message: format!(
                        "runtime waits for activation {activation:?} but presentation {} occurrence {} retains activation {:?} for `{}`",
                        presentation.id().get(),
                        dialogue.instance().get(),
                        dialogue.activation(),
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

pub(crate) fn digest_label(value: &BundleDigest) -> String {
    format!("{value:?}")
}
