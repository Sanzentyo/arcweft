use self::virtualization::validate_virtual_list_scroll_owner;
use crate::clock::RuntimeClockStep;
use crate::dialogue::{
    BundlePresentationInput, BundlePresentationTransition, DialogueAdvanceTarget,
    DialoguePresentationStore,
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
    BundleSessionRuntimeSnapshot, BundleSessionSaveError, BundleSessionSavePayload,
    BundleSessionSnapshot, digest_label, validate_presentation_runtime_status,
    validate_presentation_snapshot, validate_product_awbc_snapshot,
};
use crate::swap::{
    GenerationBuildError, ProgramGeneration, SwapCompatibility, SwapError, SwapSession,
    classify_swap_for_entry,
};
use crate::task::{
    HostTaskDispatch, RuntimeTaskCancelOutcome, RuntimeTaskCancelTarget, RuntimeTaskListOptions,
    RuntimeTaskOwner, RuntimeTaskRecord, RuntimeTaskRegistry,
};
use crate::text_control_writeback::RuntimeTextControlWriteBack;
use crate::view_projection::{ViewProjectionInput, project_view_resources};
use crate::view_runtime::{
    BundleViewDiagnostic, BundleViewDiagnosticCode, BundleViewEventDispatchError,
    BundleViewRuntime, BundleViewRuntimeError, reconciled_root_handles_for_restore,
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
use arcweft_core::effect::{LineEffectRequest, RuntimeAssertionFailure};
use arcweft_core::engine::{FlowFiberStatus, FlowStatusLabelStyle};
use arcweft_core::executor::{
    ArcweftRuntimeExecutor, ArcweftRuntimeExecutorSnapshot, RuntimeExecutor,
};
use arcweft_core::observation::RuntimeObservationState;
use arcweft_core::plan::{EntryRuntimeId, FlowEvent};
use arcweft_core::pure::{RuntimePureCallBackend, VmRuntimePureCallBackend};
use arcweft_core::root::{RootEventInput, RootTransitionOutcome, RuntimeCommandEnvelope};
use arcweft_core::step::{
    RuntimeHostCallError, RuntimeHostCallErrorKind, RuntimeHostCallId, RuntimeHostCallRequest,
    RuntimeHostCallResult, RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode,
    RuntimeStepOptions, RuntimeStepStats, RuntimeStepStopReason,
};
use arcweft_core::task::GenerationId;
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
use arcweft_view::ViewHandlerInvocation;
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
    pub view_root_bindings: Vec<RuntimeBinding>,
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
            view_root_bindings: Vec::new(),
            root_command_host_calls: RootCommandHostCallCatalog::default(),
            engine_resource_types: Arc::new(ResourceTypeRegistry::empty()),
            presentation_environment: None,
        }
    }
}

/// Host data supplied to one portable runtime step, excluding logical time.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BundleStepInput {
    pub view_bindings: Vec<RuntimeBinding>,
    pub root_events: Vec<RootEventInput>,
    /// Typed later-phase/Agent events that become root ingress next step.
    pub deferred_root_events: Vec<RootEventInput>,
    pub presentation_inputs: Vec<BundlePresentationInput>,
    pub input_events: Vec<RoutedInputEvent>,
    pub need_states: Vec<RuntimeNeedState>,
    pub task_events: Vec<TaskEvent>,
    pub audio_events: Vec<AudioEvent>,
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

    /// Queues a stale-safe semantic reveal completion for the observed stage.
    pub fn queue_dialogue_reveal_completion(&mut self, target: DialogueAdvanceTarget) {
        self.pending_presentation_inputs
            .push(BundlePresentationInput::complete_dialogue_reveal(target));
    }

    /// Queues the typed presentation action sealed for one routed View invocation.
    pub fn queue_view_handler_invocation(
        &mut self,
        invocation: &ViewHandlerInvocation,
    ) -> Result<(), BundleViewEventDispatchError> {
        if let Some(input) = self.view_runtime.dispatch_invocation(invocation)? {
            self.pending_presentation_inputs.push(input);
        }
        Ok(())
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
        let mut view_bindings = self.options.view_root_bindings.clone();
        view_bindings.extend(input.view_bindings.iter().cloned());
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
        let result = self.executor.step_with_pure_backend(
            runtime,
            RuntimeStepOptions {
                mode: self.options.mode,
                budget: RuntimeStepBudget {
                    max_ops: self.options.max_ops,
                },
            },
            &mut pure_backend,
        );
        self.swap.finish_runtime_step();

        let mut stats = result.stats.clone();
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
        let view_pure_before = pure_backend.stats();
        self.update_view_presentation(
            &view_bindings,
            &previous_text_inputs,
            view_clock_error,
            &mut diagnostics,
            &mut pure_backend,
        );
        stats.pure = stats
            .pure
            .saturating_add(pure_backend.stats().saturating_delta(view_pure_before));
        self.append_fx_diagnostics(&mut diagnostics);
        let observations = self.executor.fiber().observations.clone();

        let requested_tasks = self.dispatch_requested_tasks(clock, output.requests.tasks);
        requested_host_calls
            .extend(self.capture_view_host_calls(output.requests.host_calls, &routed_input_events));
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
            stats,
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
        if let Err(error) = self.presentation.dialogue.advance_reveal(clock.dt()) {
            diagnostics.push(error.to_string());
        }
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
        let mut dialogue_advances = Vec::new();
        let presentation_transitions =
            self.route_presentation_inputs(input.presentation_inputs, &mut dialogue_advances);
        let routed_input_events = input.input_events.clone();
        let text_control_write_backs = std::mem::take(&mut self.pending_text_control_write_backs);
        let dialogue_content_events = self.presentation.dialogue.take_reached_content_events();
        PreparedBundleStepInput {
            runtime: RuntimeStepInput {
                tick: clock.tick(),
                dt: clock.dt(),
                root_events: input.root_events,
                deferred_root_events: input.deferred_root_events,
                input_events: input.input_events,
                dialogue_content_events,
                dialogue_advances,
                need_states: input.need_states,
                task_events,
                audio_events: input.audio_events,
                host_call_results: self
                    .pending_host_call_results
                    .drain(..)
                    .chain(ordinary_host_call_results)
                    .collect(),
                line_outcomes: Vec::new(),
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
        dialogue_advances: &mut Vec<arcweft_core::runtime_id::DialogueActivationId>,
    ) -> Vec<BundlePresentationTransition> {
        let mut transitions = Vec::new();
        for input in inputs {
            match input {
                BundlePresentationInput::CompleteDialogueReveal { target } => {
                    transitions.push(self.presentation.dialogue.complete_reveal(target));
                }
                BundlePresentationInput::AdvanceDialogue { target } => {
                    let (transition, activation) = self.presentation.advance_dialogue(target);
                    if let Some(activation) = activation {
                        dialogue_advances.push(activation);
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
        pure_backend: &mut impl arcweft_core::pure::RuntimeCallBackend,
    ) {
        let previous_fx = self
            .presentation
            .view
            .mounts
            .iter()
            .flat_map(|mount| mount.fx.iter().map(|application| application.instance))
            .collect::<std::collections::BTreeSet<_>>();
        let mut frame = self.view_runtime.evaluate_with_dialogue_and_backend(
            &self.presentation.presentation_handles,
            &self.presentation.dialogue.view_inputs(),
            bindings,
            self.environment.effective().reduced_motion(),
            pure_backend,
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
    ) -> Vec<RuntimeHostCallRequest> {
        let mut external = Vec::new();
        for request in requests {
            if request.capability == "view.action" && request.operation == "await" {
                match action_receive_action_id(&request) {
                    Some(action_id) => {
                        if let Some(invocation) =
                            action_invocation_from_step_inputs(&action_id, step_input_events)
                        {
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
        RuntimeValue::EntityRef(value) => Some(value.runtime_label()),
        RuntimeValue::String(value) => Some(value.to_owned()),
        _ => None,
    }
}

struct ActionInvocation {
    payload: Option<String>,
}

fn action_invocation_from_step_inputs(
    action_id: &str,
    step_input_events: &[RoutedInputEvent],
) -> Option<ActionInvocation> {
    step_input_events
        .iter()
        .find_map(|event| action_invocation_from_input_event(action_id, event))
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
    let payload = payload.ok_or_else(|| RuntimeHostCallError {
        kind: RuntimeHostCallErrorKind::Rejected,
        message: format!(
            "runtime action `{action_id}` is missing the text/entity payload required by action.receive"
        ),
    })?;
    RuntimeValue::try_record(vec![
        (
            "action".to_owned(),
            RuntimeValue::String(action_id.to_owned()),
        ),
        ("value".to_owned(), RuntimeValue::String(payload.to_owned())),
    ])
    .map(RuntimePayload::from)
    .map_err(|error| RuntimeHostCallError {
        kind: RuntimeHostCallErrorKind::Failed,
        message: error.to_string(),
    })
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

#[cfg(test)]
mod view_handler_queue_tests {
    use super::*;
    use arcweft_bundle::resource_codec::SourceMapSection;
    use arcweft_bundle::{BundleManifest, BundleRuntimeSummary};
    use arcweft_character::id::CharacterId;
    use arcweft_core::effect::RuntimeArtifactFingerprint;
    use arcweft_core::entry::{
        EntryBindingIdentity, FlowContractHash, RuntimeEntryRoles, RuntimeFlowExecutable,
        RuntimeFlowSchema,
    };
    use arcweft_core::plan::{
        EntryRuntimeId, FlowRuntimeId, RuntimeEntryKind, RuntimeEntrySpec, RuntimeEntryTarget,
        RuntimeFlowOpSeed, RuntimeFlowSeed, RuntimePlanBuilder,
    };
    use arcweft_dialogue::InlineFailurePolicy;
    use arcweft_id::TextKey;
    use arcweft_presentation::input::{InputEpoch, InputEvent};
    use arcweft_runtime_plan::awbc_lower::AwbcLowerer;
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
    use arcweft_text_model::{
        CharacterDialoguePresentationConfig, DialoguePresentationCharacter, LineDisplayFrame,
        RichTextDisplayMap,
    };
    use arcweft_view::{
        DialogueEntryId, DialogueInstanceId, DialoguePresentationId, DialogueRevision,
        DialogueStageIndex, ViewId,
    };

    #[test]
    fn action_receive_rejects_a_missing_payload_instead_of_synthesizing_empty_text() {
        let error = action_receive_payload("action.inspect", None)
            .expect_err("missing action payload is rejected");

        assert_eq!(error.kind, RuntimeHostCallErrorKind::Rejected);
        assert!(error.message.contains("action.inspect"));
    }

    #[test]
    fn action_receive_preserves_an_explicit_empty_text_payload() {
        let payload = action_receive_payload("action.inspect", Some(""))
            .expect("explicit empty text remains a valid payload");
        let RuntimeValue::Record(fields) = payload.value() else {
            panic!("action.receive payload is a record");
        };
        assert!(matches!(
            fields.iter().find(|field| field.name() == "value"),
            Some(field) if field.value() == &RuntimeValue::String(String::new())
        ));
    }

    #[test]
    fn session_queues_the_exact_token_selected_by_the_player_invocation() {
        let bundle = session_bundle();
        let mut session = BundleSession::new(&bundle, BundleSessionOptions::default())
            .expect("handler-capable bundle session");
        let view = ViewId::standard_dialogue();
        let frame = dialogue_frame(view.clone());
        let target = DialogueAdvanceTarget::new(
            DialoguePresentationId::new(31),
            DialogueEntryId::new(32),
            DialogueInstanceId::new(33),
            DialogueStageIndex::new(0),
            DialogueRevision::new(1),
        );
        let input = crate::dialogue::DialogueViewInput {
            handle: crate::presentation_handles::PresentationHandleId::try_new(
                "dialogue.session.handler",
            )
            .expect("dialogue handle"),
            view: &view,
            frame: &frame,
            state: crate::dialogue::DialogueViewState {
                occurrence: crate::dialogue::DialogueViewOccurrence {
                    presentation: DialoguePresentationId::new(31),
                    entry: DialogueEntryId::new(32),
                    instance: DialogueInstanceId::new(33),
                },
                stage: crate::dialogue::DialogueViewStage {
                    index: DialogueStageIndex::new(0),
                    page: crate::dialogue::DialoguePageIndex::new(0),
                    stage_count: 1,
                    page_count: 1,
                },
                reveal: crate::dialogue::DialogueViewReveal::complete(),
                primary_action: crate::dialogue::DialogueViewPrimaryAction {
                    target: Some(target),
                },
            },
        };
        let view_frame = session
            .view_runtime
            .evaluate_with_dialogue(&[], &[input], &[], false);
        assert!(view_frame.diagnostics.is_empty(), "{view_frame:#?}");
        let binding = &view_frame.mounts[0].events[0];
        let invocation = ViewHandlerInvocation::from_input(
            &InputEvent::activate(InputEpoch(1), binding.target().clone()),
            binding.event(),
            binding.route(),
        )
        .expect("player semantic Activate invocation");

        session
            .queue_view_handler_invocation(&invocation)
            .expect("exact mounted token queues");

        assert_eq!(
            session.pending_presentation_inputs,
            vec![BundlePresentationInput::advance_dialogue(target)]
        );
    }

    fn session_bundle() -> ArcweftBundle {
        let mut builder = RuntimePlanBuilder::new();
        let flow = FlowRuntimeId::from_checked_declaration_digest([0x61; 32], "flow.main")
            .expect("checked Flow identity");
        builder
            .push_flow_seed(RuntimeFlowSeed::new(
                flow.clone(),
                [],
                vec![RuntimeFlowOpSeed::Return("done".to_owned())],
            ))
            .expect("Flow admits");
        builder
            .push_flow_schema(RuntimeFlowSchema {
                flow: flow.clone(),
                parameters: Vec::new(),
            })
            .expect("Flow schema admits");
        builder
            .push_flow_executable(RuntimeFlowExecutable {
                flow: flow.clone(),
                contract: FlowContractHash::from_bytes([0x62; 32]),
                controller: None,
            })
            .expect("Flow executable admits");
        builder
            .push_entry(RuntimeEntrySpec {
                id: EntryRuntimeId::from_source_entity_body("entry.main").expect("Entry identity"),
                kind: RuntimeEntryKind::Cli,
                binding: EntryBindingIdentity::from_bytes([0x63; 32]),
                target: RuntimeEntryTarget::Flow(flow),
                roles: RuntimeEntryRoles::None,
            })
            .expect("Entry admits");
        let plan = builder.finish().expect("runtime plan seals");
        let dialogue = arcweft_text_model::DialogueContentCatalog::new();
        let awbc = AwbcLowerer::new(&plan, &dialogue, "session-view-handler.arcw")
            .lower()
            .expect("runtime plan lowers")
            .program;
        let source = SourceDocument::try_new(
            SourceDocumentId::try_new("runtime-driver-session-view-handler").expect("source ID"),
            SourceName::Memory,
            "flow main { return \"done\" }",
        )
        .expect("source document");
        ArcweftBundle::try_new(
            BundleManifest {
                profile_id: None,
                profile_kind: None,
                entry: Some("entry.main".to_owned()),
                adapter: None,
                adapter_manifest_ids: Vec::new(),
                required_host_calls: Vec::new(),
                runtime: BundleRuntimeSummary {
                    artifact_fingerprint: RuntimeArtifactFingerprint::try_from_bytes([0x64; 32])
                        .expect("artifact fingerprint"),
                    entry_flow: Some("flow.main".to_owned()),
                    flows: 1,
                    bytecode_instructions: awbc.instructions.len(),
                    line_task_groups: 0,
                    stream_plans: 0,
                },
            },
            SourceMapSection::try_from_documents(&[&source]).expect("source map"),
            awbc,
            dialogue,
        )
        .expect("standard handler bundle")
    }

    fn dialogue_frame(view: ViewId) -> LineDisplayFrame {
        LineDisplayFrame {
            line: arcweft_core::plan::RuntimeLineId::from_runtime_line_value(
                "say.session.view.handler",
            )
            .expect("line identity"),
            character: DialoguePresentationCharacter {
                id: CharacterId::try_new("character.session").expect("character identity"),
                display_name: "Session".to_owned(),
            },
            text_key: TextKey::try_new("text.session.view.handler").expect("text key"),
            effective: CharacterDialoguePresentationConfig {
                view,
                voice: None,
                look: None,
                stage: None,
                portrait: None,
                focus: None,
                cleanup: None,
                source_locale: None,
                hooks: Vec::new(),
                inline_failure: InlineFailurePolicy::FailLine,
                custom: BTreeMap::new(),
                config_digest: arcweft_core::entry::RuntimeValueDigest::ZERO,
            },
            text: String::new(),
            base_styles: Vec::new(),
            style_contributions: Vec::new(),
            nodes: Vec::new(),
            display_map: RichTextDisplayMap::default(),
            host_events: Vec::new(),
            inline_failures: Vec::new(),
            unresolved: Vec::new(),
        }
    }
}
