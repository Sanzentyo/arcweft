use self::virtualization::validate_virtual_list_scroll_owner;
use crate::clock::RuntimeClockStep;
use crate::dialogue::{
    BundlePresentationInput, BundlePresentationTransition, DialogueAdvanceRejection,
    DialogueAdvanceTarget, DialoguePresentationStore,
};
use crate::display::{
    ActiveSessionLocale, BundlePresentationResources, BundlePresentationSnapshot,
    CatalogDialogueRuntimeContextProvider, DisplayResolution, resolve_display_frames,
};
use crate::fx_runtime::BundleFxRuntimeError;
use crate::generation_runtime::{
    GenerationRuntimeError, GenerationRuntimeImage, GenerationRuntimeTable,
};
use crate::session_save::{
    BUNDLE_SESSION_SAVE_SCHEMA_ID, BUNDLE_SESSION_SAVE_SCHEMA_VERSION,
    BundleSessionArtifactIdentity, BundleSessionCharacterPresentationSnapshot,
    BundleSessionExecutorSnapshot, BundleSessionGenerationSnapshot, BundleSessionPendingBlocker,
    BundleSessionRuntimeSnapshot, BundleSessionSaveError, BundleSessionSnapshot, digest_label,
    validate_presentation_runtime_status, validate_presentation_snapshot,
    validate_product_awbc_snapshot,
};
use crate::swap::{
    GenerationBuildError, GenerationId, ProgramGeneration, SwapCompatibility, SwapError,
    SwapSession, classify_swap_for_entry,
};
use crate::task::{
    HostTaskDispatch, RuntimeTaskCancelOutcome, RuntimeTaskCancelTarget, RuntimeTaskListOptions,
    RuntimeTaskOwner, RuntimeTaskRecord, RuntimeTaskRegistry,
};
use crate::text_control_writeback::RuntimeTextControlWriteBack;
use crate::view_projection::{ViewProjectionInput, project_view_resources};
use crate::view_runtime::{
    BundleViewDiagnostic, BundleViewDiagnosticCode, BundleViewRuntime, BundleViewRuntimeError,
    reconciled_root_handles_for_restore,
};
use arcweft_bundle::container::{ArtifactIdentity, BundleDigest, BundleView, ReadBudget};
use arcweft_bundle::fx_definitions::FxDefinitions;
use arcweft_bundle::patch::{
    BundlePatchArtifact, PatchBundleError, PatchCompatibility, PatchMaterializedTarget,
    PatchValidationError, apply_patch_bundle, decode_patch_bundle,
};
use arcweft_bundle::resource_codec::{
    ViewProgramResource, ViewRuntimeActionButton, ViewRuntimeFocusGroup,
    ViewRuntimeFocusNavigation, ViewRuntimeScrollRegion, ViewRuntimeSurface,
    ViewRuntimeTextControl, ViewRuntimeTextSelection,
};
use arcweft_bundle::{ArcweftBundle, BundleImageObject, BundleKind};
use arcweft_character::presentation_name::AcceptedCharacterPresentationCatalog;
use arcweft_core::awbc::{
    product_step::AwbcProductStepBuildError,
    schema::{AwbcEntryId, AwbcProgram},
};
use arcweft_core::bytecode::BytecodeVerificationError;
use arcweft_core::effect::{LineEffectRequest, RuntimeAssertionFailure};
use arcweft_core::engine::{FlowFiberStatus, FlowStatusLabelStyle};
use arcweft_core::executor::{
    ArcweftRuntimeExecutor, ArcweftRuntimeExecutorSnapshot, RuntimeExecutor,
};
use arcweft_core::observation::RuntimeObservationState;
use arcweft_core::plan::{EntryRuntimeId, FlowEvent, RuntimePlanError};
use arcweft_core::pure::VmRuntimePureCallBackend;
use arcweft_core::root::{RootEventInput, RootTransitionOutcome, RuntimeCommandEnvelope};
use arcweft_core::source::{RuntimeSourceEvent, SourceId};
use arcweft_core::step::{
    RuntimeHostCallError, RuntimeHostCallErrorKind, RuntimeHostCallId, RuntimeHostCallRequest,
    RuntimeHostCallResult, RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode,
    RuntimeStepOptions, RuntimeStepStats, RuntimeStepStopReason,
};
use arcweft_core::task::{
    CancelScopeId, LogicalEpoch, RuntimeNeedState, TaskEvent, TaskEventKind, TaskSequence,
};
use arcweft_core::value::{RuntimeBinding, RuntimePayload, RuntimeValue};
use arcweft_interaction_model::audio::{AudioCommandEnvelope, AudioEvent};
use arcweft_interaction_model::id::Identifier;
use arcweft_interaction_model::input::{
    InputEpoch, InputEventKind, InputSequence, InteractionTarget, RoutedInputEvent,
};
use arcweft_interaction_model::payload::InteractionPayload;
use arcweft_presentation::appearance::{
    PresentationEnvironment, PresentationEnvironmentField, PresentationEnvironmentOverrides,
    PresentationEnvironmentValue, PresentationEnvironmentValues, SystemPaletteSet,
};
use arcweft_presentation::input::Action;
use arcweft_presentation::text_input::TextControlWriteBack;
use arcweft_resource_model::registry::ResourceTypeRegistry;
use arcweft_text_model::DialogueContentCatalog;
use arcweft_view::{ViewStyleProgram, virtualization::ViewVirtualizationRuntime};
use std::collections::BTreeMap;
use std::sync::Arc;
use thiserror::Error;

mod axis_seed;
mod construction;
pub mod environment;
mod fx;
mod hot_swap;
mod lifecycle;
mod persistence;
mod replay;
mod root_command;
mod text_control;
mod virtualization;

pub use self::environment::{
    PresentationEnvironmentUpdate, PresentationEnvironmentUpdateError, SessionEnvironmentState,
};
pub use self::replay::{
    ROOT_REPLAY_ENGINE_IDENTITY, ROOT_REPLAY_SCHEMA_VERSION, RecordedExternalOutcome,
    RecordedExternalOutcomePositionV1, RecordedExternalOutcomeResultV1,
    RecordedHostCallErrorKindV1, RecordedRootOutcomeV1, RecordedRootTransitionV1, RootReplayError,
    RootReplayRecorderV1, RootReplayRecordingError, RootReplayReportV1, RootReplayTraceV1,
};
pub use self::root_command::{
    RootCommandHostArgument, RootCommandHostCallBinding, RootCommandHostCallCatalog,
    RootCommandHostCallCatalogError, RootCommandHostCallEndpoint, RootCommandHostResultRoute,
};
pub use self::virtualization::BundleVirtualListMountError;
use construction::{
    SessionRuntime, build_session_runtime, build_session_runtime_preserving_executor,
};
use text_control::apply_text_control_write_back_to_controls;

/// Host-selected options for a portable bundle session.
#[derive(Clone, Debug, PartialEq)]
pub struct BundleSessionOptions {
    pub entry: Option<EntryRuntimeId>,
    pub mode: RuntimeStepMode,
    pub max_ops: usize,
    pub root_bindings: Vec<RuntimeBinding>,
    pub root_command_host_calls: RootCommandHostCallCatalog,
    /// Immutable engine-owned resource types used as the base when an AWFB
    /// publishes extension manifests.
    pub engine_resource_types: Arc<ResourceTypeRegistry>,
    /// Complete host provider snapshot, or `None` when no provider is available.
    pub presentation_environment: Option<PresentationEnvironmentValues>,
}

impl Default for BundleSessionOptions {
    fn default() -> Self {
        Self {
            entry: None,
            mode: RuntimeStepMode::Game,
            max_ops: 64,
            root_bindings: Vec::new(),
            root_command_host_calls: RootCommandHostCallCatalog::default(),
            engine_resource_types: Arc::new(ResourceTypeRegistry::empty()),
            presentation_environment: None,
        }
    }
}

/// Host data supplied to one portable runtime step, excluding logical time.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BundleStepInput {
    pub bindings: Vec<RuntimeBinding>,
    pub root_events: Vec<RootEventInput>,
    /// Typed later-phase/Agent events that become root ingress next step.
    pub deferred_root_events: Vec<RootEventInput>,
    pub presentation_inputs: Vec<BundlePresentationInput>,
    pub input_events: Vec<RoutedInputEvent>,
    pub need_states: Vec<RuntimeNeedState>,
    pub task_events: Vec<TaskEvent>,
    pub audio_events: Vec<AudioEvent>,
    pub source_events: Vec<RuntimeSourceEvent>,
    pub host_call_results: Vec<RuntimeHostCallResult>,
}

/// Runtime-ready input after pending host work and presentation routing are resolved.
struct PreparedBundleStepInput {
    runtime: RuntimeStepInput,
    routed_input_events: Vec<RoutedInputEvent>,
    presentation_transitions: Vec<BundlePresentationTransition>,
    text_control_write_backs: Vec<RuntimeTextControlWriteBack>,
    diagnostics: Vec<String>,
}

/// One deterministic VM step plus the host work and presentation state it emitted.
#[derive(Clone, Debug, PartialEq)]
pub struct BundleSessionStep {
    pub index: usize,
    pub clock: RuntimeClockStep,
    pub stop_reason: RuntimeStepStopReason,
    pub stop_reason_label: String,
    pub fiber_status: FlowFiberStatus,
    pub status_label: String,
    pub stats: RuntimeStepStats,
    pub diagnostics: Vec<String>,
    /// Typed assertion failures admitted at this host boundary. Successful
    /// assertions were omitted by the runtime and never enter this vector.
    pub assertion_failures: Vec<RuntimeAssertionFailure>,
    pub observations: RuntimeObservationState,
    pub flow_events: Vec<FlowEvent>,
    pub root_transitions: Vec<RootTransitionOutcome>,
    pub root_commands: Vec<RuntimeCommandEnvelope>,
    pub deferred_root_events: Vec<RootEventInput>,
    pub requested_host_calls: Vec<RuntimeHostCallRequest>,
    pub line_effects: Vec<LineEffectRequest>,
    pub presentation_transitions: Vec<BundlePresentationTransition>,
    pub presentation: BundlePresentationSnapshot,
    pub text_control_write_backs: Vec<RuntimeTextControlWriteBack>,
    pub audio_commands: Vec<AudioCommandEnvelope>,
    pub requested_tasks: Vec<HostTaskDispatch>,
    pub cancel_scopes: Vec<CancelScopeId>,
    pub source_close: Vec<SourceId>,
    pub finished: bool,
}

/// Foreground-entry start request for the current single-fiber runtime driver.
///
/// `SessionDefault` reuses the entry selected when the generation runtime image
/// was built from `BundleSessionOptions`. `Entry` is an explicit AWBC entry table
/// id supplied by a caller that already resolved public launch metadata.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum BundleEntryStart {
    #[default]
    SessionDefault,
    Entry(AwbcEntryId),
}

impl BundleEntryStart {
    #[must_use]
    pub const fn session_default() -> Self {
        Self::SessionDefault
    }

    #[must_use]
    pub const fn entry(entry: AwbcEntryId) -> Self {
        Self::Entry(entry)
    }
}

impl From<AwbcEntryId> for BundleEntryStart {
    fn from(entry: AwbcEntryId) -> Self {
        Self::Entry(entry)
    }
}

fn dialogue_fx_instances(
    presentations: &DialoguePresentationStore,
) -> std::collections::BTreeSet<arcweft_presentation::fx::FxInstanceId> {
    presentations
        .iter()
        .flat_map(|dialogue| {
            dialogue.entries().iter().flat_map(move |entry| {
                entry
                    .frame()
                    .fx_applications()
                    .map(move |application| entry.fx_instance_id(dialogue.id(), application))
            })
        })
        .collect()
}

/// Minimal observable handle for a newly started foreground entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StartedForegroundEntry {
    pub generation: GenerationId,
    pub entry: AwbcEntryId,
}

/// Portable decoded bundle execution session.
#[derive(Clone, Debug)]
pub struct BundleSession {
    source_label: String,
    executor: ArcweftRuntimeExecutor,
    runtime_images: GenerationRuntimeTable<SessionRuntime>,
    dialogue_content: DialogueContentCatalog,
    character_presentation: Option<AcceptedCharacterPresentationCatalog>,
    active_locale: Option<ActiveSessionLocale>,
    image_objects: Vec<BundleImageObject>,
    text_inputs: Vec<ViewRuntimeTextControl>,
    action_buttons: Vec<ViewRuntimeActionButton>,
    scroll_regions: Vec<ViewRuntimeScrollRegion>,
    surfaces: Vec<ViewRuntimeSurface>,
    focus_groups: Vec<ViewRuntimeFocusGroup>,
    focus_navigation: Vec<ViewRuntimeFocusNavigation>,
    fx_definitions: FxDefinitions,
    view_runtime: BundleViewRuntime,
    environment: SessionEnvironmentState,
    view_style_palettes: SystemPaletteSet,
    engine_resource_types: Arc<ResourceTypeRegistry>,
    resource_types: Arc<ResourceTypeRegistry>,
    options: BundleSessionOptions,
    pending_input_events: Vec<RoutedInputEvent>,
    pending_presentation_inputs: Vec<BundlePresentationInput>,
    pending_text_control_write_backs: Vec<RuntimeTextControlWriteBack>,
    pending_host_call_results: Vec<RuntimeHostCallResult>,
    pending_deferred_root_events: Vec<RootEventInput>,
    pending_root_command_results: BTreeMap<RuntimeHostCallId, RootCommandHostResultRoute>,
    waiting_action_receive_calls: Vec<PendingActionReceiveCall>,
    presentation: BundlePresentationSnapshot,
    view_virtualization: ViewVirtualizationRuntime,
    next_step_index: usize,
    next_task_sequence: u64,
    swap: SwapSession,
    runtime_generation_pin: Option<Arc<ProgramGeneration>>,
    task_generation_pins: BTreeMap<TaskSequence, Arc<ProgramGeneration>>,
    tasks: RuntimeTaskRegistry,
    next_generation_id: u64,
    active_artifact_identity: BundleSessionArtifactIdentity,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PendingActionReceiveCall {
    request: RuntimeHostCallId,
    action_id: String,
}

/// Error raised before a portable session can start.
#[derive(Debug, Error, PartialEq)]
pub enum BundleSessionError {
    #[error("bundle kind `{0:?}` is not supported by the game session")]
    UnsupportedBundleKind(BundleKind),
    #[error("an exact entry selection is required to start a bundle session")]
    MissingEntrySelection,
    #[error("invalid canonical entry selection `{entry}`: {message}")]
    InvalidEntrySelection { entry: String, message: String },
    #[error("unknown entry `{entry}`")]
    UnknownEntry { entry: String },
    #[error("entry `{entry}` does not select a single runnable flow")]
    NonFlowEntry { entry: String },
    #[error(transparent)]
    RootCommandHostCatalog(#[from] RootCommandHostCallCatalogError),
    #[error("failed to decode bundle bytecode: {0}")]
    DecodeBytecode(#[from] RuntimePlanError),
    #[error("failed to verify bundle bytecode: {0}")]
    VerifyBytecode(#[from] BytecodeVerificationError),
    #[error("product bundle is missing canonical AWBC executable payload")]
    MissingProductAwbc,
    #[error("failed to verify product AWBC generation: {message}")]
    ProductAwbcVerification { message: String },
    #[error(transparent)]
    ProductAwbcRuntime(#[from] AwbcProductStepBuildError),
    #[error("product AWBC entry `{entry}` does not exist")]
    ProductAwbcEntry { entry: String },
    #[error("failed to fingerprint bundle generation: {message}")]
    GenerationFingerprint { message: String },
    #[error("failed to decode bundle container: {message}")]
    DecodeBundle { message: String },
    #[error("invalid Character presentation product: {message}")]
    CharacterPresentation { message: String },
    #[error("unsupported semantic action `{action}` at the game runtime boundary")]
    UnsupportedSemanticAction { action: String },
    #[error("semantic action `{action}` is missing its option payload")]
    MissingSemanticActionPayload { action: String },
    #[error(
        "runtime text-control write-back target `{target}` with session {session} is not active"
    )]
    UnknownTextControlWriteBackTarget { target: String, session: u64 },
    #[error(transparent)]
    ViewRuntime(#[from] BundleViewRuntimeError),
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum BundleEntryStartError {
    #[error("AWBC entry {entry:?} does not exist in the active generation")]
    UnknownEntry { entry: AwbcEntryId },
    #[error("AWBC entry {entry:?} does not select a single runnable flow")]
    NonFlowEntry { entry: AwbcEntryId },
    #[error("generation runtime table failed: {0}")]
    GenerationRuntime(#[from] GenerationRuntimeError),
    #[error(transparent)]
    ProductAwbcRuntime(#[from] AwbcProductStepBuildError),
    #[error(transparent)]
    RootCommandHostCatalog(#[from] RootCommandHostCallCatalogError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BundleHotSwapReport {
    pub generation: GenerationId,
    pub compatibility: SwapCompatibility,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BundlePatchReadiness {
    Noop,
    TargetBundleRequired { operations: usize },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BundlePatchReadinessReport {
    pub base_generation: GenerationId,
    pub base_content_root: BundleDigest,
    pub target_content_root: BundleDigest,
    /// Compatibility declared by the decoded patch manifest.
    ///
    /// This value is suitable for inspection only. Applying a patch requires
    /// the verified compatibility returned after bundle materialization.
    pub declared_compatibility: PatchCompatibility,
    pub readiness: BundlePatchReadiness,
}

#[derive(Debug, Error)]
pub enum BundleHotSwapError {
    #[error("failed to build bundle generation: {0}")]
    BuildGeneration(#[from] GenerationBuildError),
    #[error("failed to prepare hot swap: {0}")]
    Prepare(#[source] SwapError),
    #[error("failed to commit hot swap: {0}")]
    Commit(#[source] SwapError),
    #[error("bundle session cannot apply `{compatibility}` without host restart")]
    RestartRequired { compatibility: SwapCompatibility },
    #[error("failed to build replacement session runtime: {0}")]
    Session(#[from] BundleSessionError),
    #[error("failed to inspect active entry for hot swap: {message}")]
    ActiveEntry { message: String },
    #[error(
        "hot swap requires empty root work queues (reducer_active={reducer_active}, \
         pending_events={pending_events}, pending_commands={pending_commands}, \
         pending_command_results={pending_command_results})"
    )]
    PendingRootWork {
        reducer_active: bool,
        pending_events: usize,
        pending_commands: u32,
        pending_command_results: usize,
    },
    #[error("failed to decode AWFB patch bundle: {0}")]
    DecodePatch(#[source] PatchBundleError),
    #[error("invalid AWFB patch artifact: {0}")]
    InvalidPatch(#[source] PatchBundleError),
    #[error("patch does not apply to the active generation: {0}")]
    WrongPatchBase(#[source] PatchValidationError),
    #[error("patch base artifact mismatch: active {active:?}, expected {expected:?}")]
    WrongPatchBaseArtifact {
        active: Box<ArtifactIdentity>,
        expected: Box<ArtifactIdentity>,
    },
    #[error("active session was not created from an AWFB container")]
    MissingActiveContainerIdentity,
    #[error("failed to materialize AWFB patch: {0}")]
    MaterializePatch(#[source] PatchBundleError),
    #[error("failed to decode materialized AWFB patch target: {message}")]
    DecodePatchTarget { message: String },
    #[error("generation runtime table failed: {0}")]
    GenerationRuntime(#[from] GenerationRuntimeError),
    #[error("hot-swap View virtualization contract changed: {message}")]
    ViewVirtualization { message: String },
    #[error("hot-swap executable View state is incompatible: {message}")]
    ViewRuntime { message: String },
    #[error("hot-swap Character presentation state is incompatible: {message}")]
    CharacterPresentation { message: String },
    #[error(transparent)]
    Environment(#[from] PresentationEnvironmentUpdateError),
    #[error(transparent)]
    FxRuntime(#[from] BundleFxRuntimeError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RuntimeSemanticAction {
    ChoiceSelect,
}

impl RuntimeSemanticAction {
    const fn as_str(self) -> &'static str {
        match self {
            Self::ChoiceSelect => "action.choice.select",
        }
    }

    fn from_action(action: &Action) -> Result<Self, BundleSessionError> {
        let kind = action.kind().as_str();
        match kind {
            "action.choice.select" => Ok(Self::ChoiceSelect),
            _ => Err(BundleSessionError::UnsupportedSemanticAction {
                action: kind.to_owned(),
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RuntimeInputTargetKind {
    Runtime,
}

impl RuntimeInputTargetKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Runtime => "runtime",
        }
    }

    fn target(self) -> InteractionTarget {
        InteractionTarget::new(self.as_str()).expect("static runtime input target is non-empty")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RuntimeInputKind {
    Choice,
    ActionInvoke,
}

impl RuntimeInputKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Choice => "choice",
            Self::ActionInvoke => "action.invoke",
        }
    }

    fn event_kind(self) -> InputEventKind {
        InputEventKind::Custom {
            name: Identifier::new(self.as_str()).expect("static runtime input name is non-empty"),
        }
    }
}

impl BundleSession {
    /// Queues a core input event produced by a platform/presentation adapter.
    pub fn queue_input(&mut self, event: RoutedInputEvent) {
        self.pending_input_events.push(event);
    }

    /// Final semantic bridge from Arcweft presentation actions to core input
    /// events. DOM controls are never involved.
    pub fn queue_semantic_action(&mut self, action: &Action) -> Result<(), BundleSessionError> {
        match RuntimeSemanticAction::from_action(action) {
            Ok(RuntimeSemanticAction::ChoiceSelect) => {
                let option = action.payload().cloned().ok_or_else(|| {
                    BundleSessionError::MissingSemanticActionPayload {
                        action: RuntimeSemanticAction::ChoiceSelect.as_str().to_owned(),
                    }
                })?;
                self.queue_choice_selection(option);
            }
            Err(BundleSessionError::UnsupportedSemanticAction { .. }) => {
                self.queue_action_invoke(action)?;
            }
            Err(error) => return Err(error),
        }
        Ok(())
    }

    fn queue_action_invoke(&mut self, action: &Action) -> Result<(), BundleSessionError> {
        let target = InteractionTarget::new(action.kind().as_str()).map_err(|_| {
            BundleSessionError::UnsupportedSemanticAction {
                action: action.kind().as_str().to_owned(),
            }
        })?;
        let event = RoutedInputEvent::new(
            InputEpoch::default(),
            InputSequence::default(),
            target,
            RuntimeInputKind::ActionInvoke.event_kind(),
        );
        let event = if let Some(payload) = action.payload() {
            event.with_payload(InteractionPayload::Text(payload.clone()))
        } else {
            event
        };
        self.queue_input(event);
        self.resolve_waiting_action_receive_calls(
            action.kind().as_str(),
            action.payload().map(String::as_str),
        );
        Ok(())
    }

    pub fn queue_choice_selection(&mut self, option: impl Into<String>) {
        self.queue_input(
            RoutedInputEvent::new(
                InputEpoch::default(),
                InputSequence::default(),
                RuntimeInputTargetKind::Runtime.target(),
                RuntimeInputKind::Choice.event_kind(),
            )
            .with_payload(InteractionPayload::Text(option.into())),
        );
    }

    /// Queues a stale-safe advance for the observed dialogue occurrence/stage.
    pub fn queue_dialogue_advance(&mut self, target: DialogueAdvanceTarget) {
        self.pending_presentation_inputs
            .push(BundlePresentationInput::advance_dialogue(target));
    }

    pub fn queue_text_control_write_back(
        &mut self,
        write_back: &TextControlWriteBack,
    ) -> Result<(), BundleSessionError> {
        let runtime_write_back = match apply_text_control_write_back_to_controls(
            &mut self.presentation.text_inputs,
            write_back,
        ) {
            Ok(write_back) => write_back,
            Err(BundleSessionError::UnknownTextControlWriteBackTarget { .. }) => {
                apply_text_control_write_back_to_controls(&mut self.text_inputs, write_back)?
            }
            Err(error) => return Err(error),
        };
        if let Some(source_control) = self.text_inputs.iter_mut().find(|control| {
            control.target == runtime_write_back.target()
                && control.session == runtime_write_back.session()
        }) {
            runtime_write_back
                .value()
                .as_str()
                .clone_into(&mut source_control.value);
            source_control.selection = runtime_write_back.selection();
        }
        self.resolve_text_control_submit_action(&runtime_write_back);
        self.pending_text_control_write_backs
            .push(runtime_write_back);
        Ok(())
    }

    pub fn queue_text_control_write_backs<I>(
        &mut self,
        write_backs: I,
    ) -> Result<(), BundleSessionError>
    where
        I: IntoIterator<Item = TextControlWriteBack>,
    {
        write_backs
            .into_iter()
            .try_for_each(|write_back| self.queue_text_control_write_back(&write_back))
    }

    pub fn pending_text_control_write_backs(&self) -> &[RuntimeTextControlWriteBack] {
        &self.pending_text_control_write_backs
    }

    fn resolve_text_control_submit_action(&mut self, write_back: &RuntimeTextControlWriteBack) {
        if !write_back.is_submit() {
            return;
        }
        let Some(handler) = write_back.handler() else {
            return;
        };
        if handler.handler_id.starts_with("action.") {
            self.resolve_waiting_action_receive_calls(
                &handler.handler_id,
                Some(write_back.value().as_str()),
            );
        }
    }

    fn resolve_waiting_action_receive_calls(&mut self, action_id: &str, payload: Option<&str>) {
        let mut index = 0;
        while index < self.waiting_action_receive_calls.len() {
            if self.waiting_action_receive_calls[index].action_id == action_id {
                let call = self.waiting_action_receive_calls.remove(index);
                self.pending_host_call_results.push(RuntimeHostCallResult {
                    id: call.request,
                    outcome: action_receive_payload(action_id, payload),
                });
            } else {
                index += 1;
            }
        }
    }

    /// Executes exactly one VM step using explicit, non-zero logical time.
    #[allow(
        clippy::too_many_lines,
        reason = "one session step is the atomic owner of VM execution, output publication, presentation update, and lifecycle finalization"
    )]
    pub fn step_with_clock(
        &mut self,
        clock: RuntimeClockStep,
        input: BundleStepInput,
    ) -> BundleSessionStep {
        let mut view_bindings = self.options.root_bindings.clone();
        view_bindings.extend(input.bindings.iter().cloned());
        self.presentation.advance_fx_clock(clock.dt_millis());
        let view_clock_error = self.view_runtime.advance_millis(clock.dt_millis()).err();
        self.swap.enter_runtime_step();
        let PreparedBundleStepInput {
            runtime,
            routed_input_events,
            presentation_transitions,
            text_control_write_backs,
            diagnostics: input_diagnostics,
        } = self.prepare_step_input(clock, input);
        let mut pure_backend = VmRuntimePureCallBackend::default();
        let result = self.executor.step_with_root_bindings_and_pure_backend(
            runtime,
            &self.options.root_bindings,
            RuntimeStepOptions {
                mode: self.options.mode,
                budget: RuntimeStepBudget {
                    max_ops: self.options.max_ops,
                },
            },
            &mut pure_backend,
        );
        self.swap.finish_runtime_step();

        let mut output = result.output;
        let root_transitions = std::mem::take(&mut output.root_transitions);
        let root_commands = std::mem::take(&mut output.root_commands);
        let deferred_root_events = std::mem::take(&mut output.requests.root_events_next_step);
        self.pending_deferred_root_events
            .extend(deferred_root_events.iter().cloned());
        let flow_events = std::mem::take(&mut output.flow_events);
        let line_effects = std::mem::take(&mut output.effects.line);
        let assertion_failures = line_effects
            .iter()
            .filter_map(|effect| match effect {
                LineEffectRequest::Assert(assertion) => {
                    Some(RuntimeAssertionFailure::new(assertion.clone()))
                }
                _ => None,
            })
            .collect();
        let context_provider = self
            .character_presentation
            .as_ref()
            .zip(self.active_locale.as_ref())
            .map(|(catalog, locale)| CatalogDialogueRuntimeContextProvider::new(catalog, locale));
        let display = resolve_display_frames(
            &self.dialogue_content,
            &flow_events,
            context_provider
                .as_ref()
                .map(|provider| provider as &dyn crate::display::DialogueRuntimeContextProvider),
        );
        let mut diagnostics = output
            .diagnostics
            .into_iter()
            .map(|diagnostic| diagnostic.message)
            .collect::<Vec<_>>();
        diagnostics.extend(input_diagnostics);
        diagnostics.extend(
            presentation_transitions
                .iter()
                .filter_map(presentation_transition_diagnostic),
        );
        diagnostics.extend(display.diagnostics.iter().cloned());
        let mut requested_host_calls =
            self.publish_and_acknowledge_root_commands(&root_commands, &mut diagnostics);
        let previous_text_inputs = self.presentation.text_inputs.clone();
        self.update_presentation_snapshot(
            &display,
            &result.fiber_status,
            &line_effects,
            &mut diagnostics,
        );
        self.update_view_presentation(
            &view_bindings,
            &previous_text_inputs,
            view_clock_error,
            &mut diagnostics,
        );
        self.append_fx_diagnostics(&mut diagnostics);
        let observations = self.executor.fiber().observations.clone();

        let requested_tasks = self.dispatch_requested_tasks(clock, output.requests.tasks);
        requested_host_calls.extend(self.capture_view_host_calls(
            output.requests.host_calls,
            &routed_input_events,
            &text_control_write_backs,
        ));
        let cancel_scopes = output.requests.cancel_scopes;
        self.apply_task_cancellations(&cancel_scopes);
        let audio_commands = output.requests.audio;
        let (index, finished) = self.finish_step_lifecycle(&result.fiber_status);

        BundleSessionStep {
            index,
            clock,
            stop_reason: result.stop_reason,
            stop_reason_label: format!("{:?}", result.stop_reason),
            status_label: result
                .fiber_status
                .status_label(FlowStatusLabelStyle::Runtime),
            fiber_status: result.fiber_status,
            stats: result.stats,
            diagnostics,
            assertion_failures,
            observations,
            flow_events,
            root_transitions,
            root_commands,
            deferred_root_events,
            requested_host_calls,
            line_effects,
            presentation_transitions,
            presentation: self.presentation.clone(),
            text_control_write_backs,
            audio_commands,
            requested_tasks,
            cancel_scopes,
            source_close: output.requests.source_close,
            finished,
        }
    }

    fn finish_step_lifecycle(&mut self, fiber_status: &FlowFiberStatus) -> (usize, bool) {
        let finished = matches!(
            fiber_status,
            FlowFiberStatus::Done(_) | FlowFiberStatus::Failed(_)
        );
        if finished {
            self.runtime_generation_pin = None;
            self.retire_unused_generations();
        }
        let index = self.next_step_index;
        self.next_step_index = self.next_step_index.saturating_add(1);
        (index, finished)
    }

    fn apply_task_cancellations(&mut self, cancel_scopes: &[CancelScopeId]) {
        for scope in cancel_scopes {
            self.tasks
                .cancel(&RuntimeTaskCancelTarget::Scope(scope.0.clone()));
        }
    }

    fn prepare_step_input(
        &mut self,
        clock: RuntimeClockStep,
        mut input: BundleStepInput,
    ) -> PreparedBundleStepInput {
        let mut diagnostics = Vec::new();
        let mut root_events = std::mem::take(&mut self.pending_deferred_root_events);
        root_events.append(&mut input.root_events);
        input.root_events = root_events;
        let ordinary_host_call_results = self.route_host_call_results(
            input.host_call_results,
            &mut input.root_events,
            &mut diagnostics,
        );
        input.task_events.extend(self.tasks.drain_task_events());
        let task_events = self.tasks.apply_task_events(input.task_events);
        self.release_completed_task_generation_pins(&task_events);
        input.input_events.append(&mut self.pending_input_events);
        input
            .presentation_inputs
            .append(&mut self.pending_presentation_inputs);
        let presentation_transitions =
            self.route_presentation_inputs(input.presentation_inputs, &mut input.input_events);
        let routed_input_events = input.input_events.clone();
        let text_control_write_backs = std::mem::take(&mut self.pending_text_control_write_backs);
        PreparedBundleStepInput {
            runtime: RuntimeStepInput {
                tick: clock.tick(),
                dt: clock.dt(),
                bindings: input.bindings,
                root_events: input.root_events,
                deferred_root_events: input.deferred_root_events,
                input_events: input.input_events,
                need_states: input.need_states,
                task_events,
                audio_events: input.audio_events,
                source_events: input.source_events,
                host_call_results: self
                    .pending_host_call_results
                    .drain(..)
                    .chain(ordinary_host_call_results)
                    .collect(),
            },
            routed_input_events,
            presentation_transitions,
            text_control_write_backs,
            diagnostics,
        }
    }

    fn route_presentation_inputs(
        &mut self,
        inputs: Vec<BundlePresentationInput>,
        runtime_inputs: &mut Vec<RoutedInputEvent>,
    ) -> Vec<BundlePresentationTransition> {
        let mut transitions = Vec::new();
        runtime_inputs.retain(|event| {
            if runtime_input_is_untargeted_dialogue_advance(event) {
                transitions.push(BundlePresentationTransition::DialogueAdvanceRejected {
                    target: None,
                    reason: DialogueAdvanceRejection::UntargetedRuntimeInput,
                });
                false
            } else {
                true
            }
        });

        for input in inputs {
            match input {
                BundlePresentationInput::AdvanceDialogue { target } => {
                    let (transition, runtime_line) = self.presentation.advance_dialogue(target);
                    if let Some(line) = runtime_line {
                        runtime_inputs.push(RuntimeStepInput::dialogue_advance_event(&line));
                    }
                    transitions.push(transition);
                }
            }
        }
        transitions
    }

    pub fn is_finished(&self) -> bool {
        matches!(
            &self.executor.fiber().status,
            FlowFiberStatus::Done(_) | FlowFiberStatus::Failed(_)
        )
    }

    fn update_presentation_snapshot(
        &mut self,
        display: &DisplayResolution,
        status: &FlowFiberStatus,
        line_effects: &[LineEffectRequest],
        diagnostics: &mut Vec<String>,
    ) {
        let previous_dialogue_fx = self.presentation.dialogue.clone();
        let presentation_handle_diagnostics = match self.presentation.update(
            display,
            status,
            line_effects,
            BundlePresentationResources {
                image_objects: &self.image_objects,
                text_inputs: &self.text_inputs,
                action_buttons: &self.action_buttons,
                scroll_regions: &self.scroll_regions,
                surfaces: &self.surfaces,
                focus_groups: &self.focus_groups,
                focus_navigation: &self.focus_navigation,
            },
        ) {
            Ok(diagnostics) => diagnostics,
            Err(error) => {
                diagnostics.push(error.to_string());
                Vec::new()
            }
        };
        diagnostics.extend(
            presentation_handle_diagnostics
                .iter()
                .map(ToString::to_string),
        );
        self.reconcile_dialogue_fx(&dialogue_fx_instances(&previous_dialogue_fx));
    }

    fn reconcile_dialogue_fx(
        &mut self,
        previous: &std::collections::BTreeSet<arcweft_presentation::fx::FxInstanceId>,
    ) {
        let applications = self
            .presentation
            .dialogue
            .iter()
            .flat_map(|dialogue| {
                dialogue.entries().iter().flat_map(move |entry| {
                    entry
                        .frame()
                        .fx_applications()
                        .cloned()
                        .map(move |application| {
                            (
                                entry.fx_instance_id(dialogue.id(), &application),
                                application,
                            )
                        })
                })
            })
            .collect::<Vec<_>>();
        let current = applications
            .iter()
            .map(|(instance, _)| *instance)
            .collect::<std::collections::BTreeSet<_>>();
        let before = self.presentation.fx.clone();
        for instance in previous.difference(&current) {
            self.presentation.fx.remove_instance(*instance);
        }
        let mut failures = self.presentation.fx_diagnostics.clone();
        for (instance, application) in applications {
            if let Err(error) = self.presentation.fx.retain_instance(
                &self.fx_definitions,
                application.definition(),
                instance,
                application.parameters().to_vec(),
                arcweft_presentation::fx::FxGraphChildPath::default(),
                None,
            ) {
                let diagnostic = error.diagnostic();
                if !failures.contains(&diagnostic) {
                    failures.push(diagnostic);
                }
            }
        }
        if self.presentation.fx != before {
            self.presentation.revision = self.presentation.revision.saturating_add(1);
        }
        if self.presentation.fx_diagnostics != failures {
            self.presentation.fx_diagnostics = failures;
            self.presentation.revision = self.presentation.revision.saturating_add(1);
        }
    }

    fn update_view_presentation(
        &mut self,
        bindings: &[RuntimeBinding],
        previous_text_inputs: &[ViewRuntimeTextControl],
        clock_error: Option<BundleViewRuntimeError>,
        diagnostics: &mut Vec<String>,
    ) {
        let previous_fx = self
            .presentation
            .view
            .mounts
            .iter()
            .flat_map(|mount| mount.fx.iter().map(|application| application.instance))
            .collect::<std::collections::BTreeSet<_>>();
        let mut frame = self.view_runtime.evaluate_with_dialogue(
            &self.presentation.presentation_handles,
            &self.presentation.dialogue.view_inputs(),
            bindings,
            self.environment.effective().reduced_motion(),
        );
        if let Some(error) = clock_error {
            frame.diagnostics.push(BundleViewDiagnostic {
                code: BundleViewDiagnosticCode::InvalidValueProgram,
                handle: None,
                mount: None,
                view: None,
                instruction: None,
                message: error.to_string(),
            });
        }
        diagnostics.extend(frame.diagnostics.iter().map(ToString::to_string));
        if self.view_runtime.has_program() {
            let projected = project_view_resources(
                &frame,
                &ViewProjectionInput {
                    executable_definitions: &self.view_runtime.definition_ids(),
                    current_images: &self.presentation.images,
                    current_text_inputs: previous_text_inputs,
                    images: &self.image_objects,
                    text_inputs: &self.text_inputs,
                    action_buttons: &self.action_buttons,
                    scroll_regions: &self.scroll_regions,
                    surfaces: &self.surfaces,
                    focus_groups: &self.focus_groups,
                    focus_navigation: &self.focus_navigation,
                },
            );
            self.presentation.replace_view_resources(projected);
        }
        self.presentation.replace_view_frame(frame);
        self.reconcile_view_fx(&previous_fx);
    }

    fn reconcile_view_fx(
        &mut self,
        previous: &std::collections::BTreeSet<arcweft_presentation::fx::FxInstanceId>,
    ) {
        let applications = self
            .presentation
            .view
            .mounts
            .iter()
            .flat_map(|mount| mount.fx.iter().cloned())
            .collect::<Vec<_>>();
        let current = applications
            .iter()
            .map(|application| application.instance)
            .collect::<std::collections::BTreeSet<_>>();
        let before = self.presentation.fx.clone();
        for instance in previous.difference(&current) {
            self.presentation.fx.remove_instance(*instance);
        }
        let mut failures = self.presentation.fx_diagnostics.clone();
        for application in applications {
            let parameters = self
                .fx_definitions
                .get(&application.definition)
                .map(|definition| {
                    definition
                        .parameters()
                        .iter()
                        .filter_map(|parameter| {
                            application
                                .arguments
                                .iter()
                                .find(|argument| argument.parameter == parameter.name())
                                .map(|argument| argument.value)
                                .or_else(|| parameter.default().copied())
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if let Err(error) = self.presentation.fx.retain_instance(
                &self.fx_definitions,
                &application.definition,
                application.instance,
                parameters,
                application.child_path,
                None,
            ) {
                let diagnostic = error.diagnostic();
                if !failures.contains(&diagnostic) {
                    failures.push(diagnostic);
                }
            }
        }
        if self.presentation.fx != before {
            self.presentation.revision = self.presentation.revision.saturating_add(1);
        }
        if self.presentation.fx_diagnostics != failures {
            self.presentation.fx_diagnostics = failures;
            self.presentation.revision = self.presentation.revision.saturating_add(1);
        }
    }

    fn dispatch_requested_tasks(
        &mut self,
        clock: RuntimeClockStep,
        tasks: Vec<arcweft_core::task::TaskSpec>,
    ) -> Vec<HostTaskDispatch> {
        let generation = self
            .runtime_generation_pin
            .clone()
            .unwrap_or_else(|| self.swap.pin_active_generation());
        tasks
            .into_iter()
            .map(|task| {
                let sequence = TaskSequence(self.next_task_sequence);
                self.next_task_sequence = self.next_task_sequence.saturating_add(1);
                self.task_generation_pins
                    .insert(sequence, generation.clone());
                let dispatch = HostTaskDispatch {
                    generation: generation.id,
                    logical_epoch: LogicalEpoch(clock.tick().0),
                    sequence,
                    task,
                };
                self.tasks.register_dispatch(&dispatch);
                dispatch
            })
            .collect()
    }

    fn capture_view_host_calls(
        &mut self,
        requests: Vec<RuntimeHostCallRequest>,
        step_input_events: &[RoutedInputEvent],
        text_control_write_backs: &[RuntimeTextControlWriteBack],
    ) -> Vec<RuntimeHostCallRequest> {
        let mut external = Vec::new();
        for request in requests {
            if request.capability == "view.action" && request.operation == "await" {
                match action_receive_action_id(&request) {
                    Some(action_id) => {
                        if let Some(invocation) = action_invocation_from_step_inputs(
                            &action_id,
                            step_input_events,
                            text_control_write_backs,
                        ) {
                            self.pending_host_call_results.push(RuntimeHostCallResult {
                                id: request.id,
                                outcome: action_receive_payload(
                                    &action_id,
                                    invocation.payload.as_deref(),
                                ),
                            });
                        } else {
                            self.waiting_action_receive_calls
                                .push(PendingActionReceiveCall {
                                    request: request.id,
                                    action_id,
                                });
                        }
                    }
                    None => self.pending_host_call_results.push(RuntimeHostCallResult {
                        id: request.id,
                        outcome: Err(RuntimeHostCallError {
                            kind: RuntimeHostCallErrorKind::Rejected,
                            message: "view.action.await requires one action target".to_owned(),
                        }),
                    }),
                }
            } else {
                external.push(request);
            }
        }
        external
    }

    fn release_completed_task_generation_pins(&mut self, task_events: &[TaskEvent]) {
        if task_events.is_empty() {
            return;
        }
        for event in task_events {
            if matches!(event.kind, TaskEventKind::Progress(_)) {
                continue;
            }
            self.task_generation_pins.remove(&event.sequence);
        }
        self.retire_unused_generations();
    }
}

fn action_receive_action_id(request: &RuntimeHostCallRequest) -> Option<String> {
    let value = request.args.first()?.value();
    match value {
        RuntimeValue::EntityRef(value) | RuntimeValue::String(value) => Some(value.to_owned()),
        _ => None,
    }
}

struct ActionInvocation {
    payload: Option<String>,
}

fn action_invocation_from_step_inputs(
    action_id: &str,
    step_input_events: &[RoutedInputEvent],
    text_control_write_backs: &[RuntimeTextControlWriteBack],
) -> Option<ActionInvocation> {
    text_control_write_backs
        .iter()
        .find_map(|write_back| {
            let handler = write_back.handler()?;
            (write_back.is_submit() && handler.handler_id == action_id).then(|| ActionInvocation {
                payload: Some(write_back.value().as_str().to_owned()),
            })
        })
        .or_else(|| {
            step_input_events
                .iter()
                .find_map(|event| action_invocation_from_input_event(action_id, event))
        })
}

fn action_invocation_from_input_event(
    action_id: &str,
    event: &RoutedInputEvent,
) -> Option<ActionInvocation> {
    let InputEventKind::Custom { name } = &event.event else {
        return None;
    };
    if name.as_str() != RuntimeInputKind::ActionInvoke.as_str()
        || event.target.as_str() != action_id
    {
        return None;
    }
    let payload = match event.payload.as_ref() {
        Some(InteractionPayload::Text(value)) => Some(value.clone()),
        Some(InteractionPayload::Entity(value)) => Some(value.as_str().to_owned()),
        Some(
            InteractionPayload::Null
            | InteractionPayload::Bool(_)
            | InteractionPayload::I64(_)
            | InteractionPayload::U64(_)
            | InteractionPayload::F64(_)
            | InteractionPayload::List(_)
            | InteractionPayload::Map(_),
        )
        | None => None,
    };
    Some(ActionInvocation { payload })
}

fn action_receive_payload(
    action_id: &str,
    payload: Option<&str>,
) -> Result<RuntimePayload, RuntimeHostCallError> {
    RuntimeValue::try_record(vec![
        (
            "action".to_owned(),
            RuntimeValue::EntityRef(action_id.to_owned()),
        ),
        (
            "value".to_owned(),
            RuntimeValue::String(payload.unwrap_or_default().to_owned()),
        ),
    ])
    .map(RuntimePayload::from)
    .map_err(|error| RuntimeHostCallError {
        kind: RuntimeHostCallErrorKind::Failed,
        message: error.to_string(),
    })
}

fn runtime_input_is_untargeted_dialogue_advance(event: &RoutedInputEvent) -> bool {
    matches!(
        &event.event,
        InputEventKind::Custom { name }
            if matches!(name.as_str(), "advance" | "dialogue.advance")
    )
}

fn presentation_transition_diagnostic(transition: &BundlePresentationTransition) -> Option<String> {
    let BundlePresentationTransition::DialogueAdvanceRejected { target, reason } = transition
    else {
        return None;
    };
    Some(format!(
        "dialogue advance rejected for {target:?}: {reason:?}"
    ))
}
