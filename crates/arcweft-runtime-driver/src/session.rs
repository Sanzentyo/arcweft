use crate::clock::RuntimeClockStep;
use crate::display::{
    BundlePresentationResources, BundlePresentationSnapshot, DisplayResolution,
    resolve_display_frames,
};
use crate::generation_runtime::{
    GenerationRuntimeError, GenerationRuntimeImage, GenerationRuntimeTable,
};
use crate::swap::{
    GenerationBuildError, GenerationId, ProgramGeneration, SwapCompatibility, SwapError,
    SwapSession, classify_swap,
};
use crate::task::{
    HostTaskDispatch, RuntimeTaskCancelOutcome, RuntimeTaskCancelTarget, RuntimeTaskListOptions,
    RuntimeTaskOwner, RuntimeTaskRecord, RuntimeTaskRegistry,
};
use crate::text_control_writeback::RuntimeTextControlWriteBack;
use arcweft_bundle::container::{BundleDigest, BundleView, ReadBudget};
use arcweft_bundle::patch::{
    BundlePatchArtifact, PatchBundleError, PatchCompatibility, PatchValidationError,
    apply_patch_bundle, decode_patch_bundle,
};
use arcweft_bundle::resource_codec::{
    UiProgramResource, UiRuntimeActionButton, UiRuntimeControlStyleDiagnostics,
    UiRuntimeFocusGroup, UiRuntimeFocusNavigation, UiRuntimeTextControl, UiRuntimeTextSelection,
};
use arcweft_bundle::{ArcweftBundle, BundleFormat, BundleImageObject, BundleKind};
use arcweft_core::awbc::{
    product_step::AwbcProductStepBuildError,
    schema::{AwbcEntryId, AwbcProgram},
};
use arcweft_core::bytecode::BytecodeVerificationError;
use arcweft_core::effect::LineEffectRequest;
use arcweft_core::engine::{FlowFiberStatus, FlowStatusLabelStyle};
use arcweft_core::executor::{ArcweftRuntimeExecutor, RuntimeExecutor};
use arcweft_core::observation::RuntimeObservationState;
use arcweft_core::plan::{FlowEvent, RuntimePlanError};
use arcweft_core::pure::VmRuntimePureCallBackend;
use arcweft_core::source::{RuntimeSourceEvent, SourceId};
use arcweft_core::step::{
    RuntimeHostCallError, RuntimeHostCallErrorKind, RuntimeHostCallId, RuntimeHostCallRequest,
    RuntimeHostCallResult, RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode,
    RuntimeStepOptions, RuntimeStepStats, RuntimeStepStopReason,
};
use arcweft_core::task::{CancelScopeId, LogicalEpoch, TaskEvent, TaskEventKind, TaskSequence};
use arcweft_core::value::{RuntimeBinding, RuntimePayload, RuntimeValue};
use arcweft_interaction_model::audio::{AudioCommandEnvelope, AudioEvent};
use arcweft_interaction_model::id::Identifier;
use arcweft_interaction_model::input::{
    InputEpoch, InputEventKind, InputSequence, InteractionTarget, RoutedInputEvent,
};
use arcweft_interaction_model::payload::InteractionPayload;
use arcweft_presentation::input::Action;
use arcweft_presentation::text_input::TextControlWriteBack;
use arcweft_render_text::LineDisplayCatalog;
use std::collections::BTreeMap;
use std::sync::Arc;
use thiserror::Error;

/// Host-selected options for a portable bundle session.
#[derive(Clone, Debug, PartialEq)]
pub struct BundleSessionOptions {
    pub entry: Option<String>,
    pub flow: Option<String>,
    pub mode: RuntimeStepMode,
    pub max_ops: usize,
    pub root_bindings: Vec<RuntimeBinding>,
}

impl Default for BundleSessionOptions {
    fn default() -> Self {
        Self {
            entry: None,
            flow: None,
            mode: RuntimeStepMode::Game,
            max_ops: 64,
            root_bindings: Vec::new(),
        }
    }
}

/// Host data supplied to one portable runtime step, excluding logical time.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BundleStepInput {
    pub bindings: Vec<RuntimeBinding>,
    pub input_events: Vec<RoutedInputEvent>,
    pub task_events: Vec<TaskEvent>,
    pub audio_events: Vec<AudioEvent>,
    pub source_events: Vec<RuntimeSourceEvent>,
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
    pub observations: RuntimeObservationState,
    pub flow_events: Vec<FlowEvent>,
    pub line_effects: Vec<LineEffectRequest>,
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
    display: LineDisplayCatalog,
    image_objects: Vec<BundleImageObject>,
    text_inputs: Vec<UiRuntimeTextControl>,
    action_buttons: Vec<UiRuntimeActionButton>,
    runtime_control_style_diagnostics: UiRuntimeControlStyleDiagnostics,
    focus_groups: Vec<UiRuntimeFocusGroup>,
    focus_navigation: Vec<UiRuntimeFocusNavigation>,
    options: BundleSessionOptions,
    pending_input_events: Vec<RoutedInputEvent>,
    pending_text_control_write_backs: Vec<RuntimeTextControlWriteBack>,
    pending_host_call_results: Vec<RuntimeHostCallResult>,
    waiting_text_submit_calls: Vec<PendingTextSubmitCall>,
    presentation: BundlePresentationSnapshot,
    next_step_index: usize,
    next_task_sequence: u64,
    swap: SwapSession,
    runtime_generation_pin: Option<Arc<ProgramGeneration>>,
    task_generation_pins: BTreeMap<TaskSequence, Arc<ProgramGeneration>>,
    tasks: RuntimeTaskRegistry,
    next_generation_id: u64,
    active_container_content_root: Option<BundleDigest>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PendingTextSubmitCall {
    request: RuntimeHostCallId,
    control_id: String,
}

/// Error raised before a portable session can start.
#[derive(Debug, Error, PartialEq)]
pub enum BundleSessionError {
    #[error("bundle kind `{0:?}` is not supported by the game session")]
    UnsupportedBundleKind(BundleKind),
    #[error("entry and flow are mutually exclusive")]
    ConflictingEntrySelection,
    #[error("unknown flow `{flow}`")]
    UnknownFlow { flow: String },
    #[error("unknown entry `{entry}`")]
    UnknownEntry { entry: String },
    #[error("entry `{entry}` does not select a single runnable flow")]
    NonFlowEntry { entry: String },
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
    #[error("unsupported semantic action `{action}` at the game runtime boundary")]
    UnsupportedSemanticAction { action: String },
    #[error("semantic action `{action}` is missing its option payload")]
    MissingSemanticActionPayload { action: String },
    #[error(
        "runtime text-control write-back target `{target}` with session {session} is not active"
    )]
    UnknownTextControlWriteBackTarget { target: String, session: u64 },
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
    pub compatibility: PatchCompatibility,
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
    #[error("failed to decode AWFB patch bundle: {0}")]
    DecodePatch(#[source] PatchBundleError),
    #[error("invalid AWFB patch artifact: {0}")]
    InvalidPatch(#[source] PatchBundleError),
    #[error("patch does not apply to the active generation: {0}")]
    WrongPatchBase(#[source] PatchValidationError),
    #[error("active session was not created from an AWFB container")]
    MissingActiveContainerRoot,
    #[error("failed to materialize AWFB patch: {0}")]
    MaterializePatch(#[source] PatchBundleError),
    #[error("failed to decode materialized AWFB patch target: {message}")]
    DecodePatchTarget { message: String },
    #[error("generation runtime table failed: {0}")]
    GenerationRuntime(#[from] GenerationRuntimeError),
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
    Advance,
    Choice,
}

impl RuntimeInputKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Advance => "advance",
            Self::Choice => "choice",
        }
    }

    fn event_kind(self) -> InputEventKind {
        InputEventKind::Custom {
            name: Identifier::new(self.as_str()).expect("static runtime input name is non-empty"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RuntimeEntityFamily {
    Entry,
    Flow,
}

impl RuntimeEntityFamily {
    const fn prefix(self) -> &'static str {
        match self {
            Self::Entry => "entry",
            Self::Flow => "flow",
        }
    }

    fn selector(self, value: &str) -> String {
        let value = value.trim().trim_start_matches('@');
        if value.contains('.') {
            value.to_owned()
        } else {
            format!("{}.{value}", self.prefix())
        }
    }
}

impl BundleSession {
    /// Builds a portable bytecode VM session without materializing bundle files.
    pub fn new(
        bundle: &ArcweftBundle,
        options: BundleSessionOptions,
    ) -> Result<Self, BundleSessionError> {
        Self::new_with_container_root(bundle, options, None)
    }

    pub fn from_awfb_bytes(
        bytes: &[u8],
        options: BundleSessionOptions,
    ) -> Result<Self, BundleSessionError> {
        let view = BundleView::parse(bytes, ReadBudget::default()).map_err(|error| {
            BundleSessionError::DecodeBundle {
                message: error.to_string(),
            }
        })?;
        let container_root = view.content_root();
        let bundle =
            ArcweftBundle::from_format_slice(BundleFormat::Awfb, bytes).map_err(|error| {
                BundleSessionError::DecodeBundle {
                    message: error.to_string(),
                }
            })?;
        Self::new_with_container_root(&bundle, options, Some(container_root))
    }

    fn new_with_container_root(
        bundle: &ArcweftBundle,
        options: BundleSessionOptions,
        active_container_content_root: Option<BundleDigest>,
    ) -> Result<Self, BundleSessionError> {
        let generation = Arc::new(initial_generation(bundle)?);
        let runtime = build_session_runtime(bundle, &options)?;
        let executor = runtime.executor.clone();
        let display = runtime.display.clone();
        let image_objects = runtime.image_objects.clone();
        let text_inputs = runtime.text_inputs.clone();
        let action_buttons = runtime.action_buttons.clone();
        let runtime_control_style_diagnostics = runtime.runtime_control_style_diagnostics.clone();
        let focus_groups = runtime.focus_groups.clone();
        let focus_navigation = runtime.focus_navigation.clone();
        let source_label = runtime.source_label.clone();

        Ok(Self {
            source_label,
            executor,
            runtime_images: GenerationRuntimeTable::new(GenerationRuntimeImage::new(
                generation.clone(),
                runtime,
            )),
            display,
            image_objects,
            text_inputs,
            action_buttons,
            runtime_control_style_diagnostics,
            focus_groups,
            focus_navigation,
            options,
            pending_input_events: Vec::new(),
            pending_text_control_write_backs: Vec::new(),
            pending_host_call_results: Vec::new(),
            waiting_text_submit_calls: Vec::new(),
            presentation: BundlePresentationSnapshot::default(),
            next_step_index: 0,
            next_task_sequence: 0,
            swap: SwapSession::new(generation.clone()),
            runtime_generation_pin: Some(generation),
            task_generation_pins: BTreeMap::new(),
            tasks: RuntimeTaskRegistry::default(),
            next_generation_id: 1,
            active_container_content_root,
        })
    }

    pub fn source_label(&self) -> &str {
        &self.source_label
    }

    pub fn active_generation(&self) -> &ProgramGeneration {
        self.swap.active()
    }

    /// Returns the generation currently bound to the active runtime fiber.
    pub fn current_fiber_generation(&self) -> Option<GenerationId> {
        self.runtime_generation_pin
            .as_ref()
            .map(|generation| generation.id)
    }

    /// Returns the generation that emitted an outstanding host task.
    pub fn task_generation(&self, sequence: TaskSequence) -> Option<GenerationId> {
        self.task_generation_pins
            .get(&sequence)
            .map(|generation| generation.id)
    }

    pub fn runtime_tasks(&self, options: RuntimeTaskListOptions) -> Vec<RuntimeTaskRecord> {
        self.tasks.list(options)
    }

    pub fn cancel_runtime_tasks(
        &mut self,
        target: &RuntimeTaskCancelTarget,
    ) -> RuntimeTaskCancelOutcome {
        self.tasks.cancel(target)
    }

    pub fn runtime_image_count(&self) -> usize {
        self.runtime_images.len()
    }

    pub fn has_runtime_image(&self, generation: GenerationId) -> bool {
        self.runtime_images.contains_generation(generation)
    }

    pub fn pin_active_generation(&self) -> Arc<ProgramGeneration> {
        self.swap.pin_active_generation()
    }

    pub fn retired_generation_count(&self) -> usize {
        self.swap.retired().len()
    }

    pub fn retire_unused_generations(&mut self) {
        self.release_table_only_retired_runtime_images();
        self.swap.retire_unused();
        self.prune_runtime_images();
    }

    pub const fn active_container_content_root(&self) -> Option<BundleDigest> {
        self.active_container_content_root
    }

    pub const fn presentation(&self) -> &BundlePresentationSnapshot {
        &self.presentation
    }

    /// Queues a core input event produced by a platform/presentation adapter.
    pub fn queue_input(&mut self, event: RoutedInputEvent) {
        self.pending_input_events.push(event);
    }

    /// Final semantic bridge from Arcweft presentation actions to the current
    /// core choice input format. DOM controls are never involved.
    pub fn queue_semantic_action(&mut self, action: &Action) -> Result<(), BundleSessionError> {
        let semantic_action = RuntimeSemanticAction::from_action(action)?;
        let option = action.payload().cloned().ok_or_else(|| {
            BundleSessionError::MissingSemanticActionPayload {
                action: semantic_action.as_str().to_owned(),
            }
        })?;
        self.queue_choice_selection(option);
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

    /// Queues the standard semantic advance input for the active dialogue line.
    pub fn queue_dialogue_advance(&mut self) {
        self.queue_input(RoutedInputEvent::new(
            InputEpoch::default(),
            InputSequence::default(),
            RuntimeInputTargetKind::Runtime.target(),
            RuntimeInputKind::Advance.event_kind(),
        ));
    }

    pub fn queue_text_control_write_back(
        &mut self,
        write_back: &TextControlWriteBack,
    ) -> Result<(), BundleSessionError> {
        let runtime_write_back =
            apply_text_control_write_back_to_controls(&mut self.text_inputs, write_back)?;
        self.resolve_waiting_text_submit_calls(&runtime_write_back);
        self.pending_text_control_write_backs
            .push(runtime_write_back);
        self.presentation.replace_text_inputs(&self.text_inputs);
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

    fn resolve_waiting_text_submit_calls(&mut self, write_back: &RuntimeTextControlWriteBack) {
        if !write_back.is_submit() {
            return;
        }
        let control_id = write_back.control_id();
        let mut index = 0;
        while index < self.waiting_text_submit_calls.len() {
            if self.waiting_text_submit_calls[index].control_id == control_id {
                let call = self.waiting_text_submit_calls.remove(index);
                self.pending_host_call_results.push(RuntimeHostCallResult {
                    id: call.request,
                    outcome: Ok(RuntimePayload::from(write_back.value().as_str().to_owned())),
                });
            } else {
                index += 1;
            }
        }
    }

    pub fn hot_swap_bundle(
        &mut self,
        bundle: &ArcweftBundle,
    ) -> Result<BundleHotSwapReport, BundleHotSwapError> {
        self.hot_swap_bundle_with_container_root(bundle, None)
    }

    fn hot_swap_bundle_with_container_root(
        &mut self,
        bundle: &ArcweftBundle,
        active_container_content_root: Option<BundleDigest>,
    ) -> Result<BundleHotSwapReport, BundleHotSwapError> {
        let next_id = GenerationId(self.next_generation_id);
        let next_generation = Arc::new(ProgramGeneration::from_bundle(next_id, bundle)?);
        let compatibility = classify_swap(self.swap.active(), &next_generation);
        if compatibility == SwapCompatibility::RestartRequired {
            return Err(BundleHotSwapError::RestartRequired { compatibility });
        }

        let mut next_runtime = build_session_runtime(bundle, &self.options)?;
        preserve_runtime_text_control_values(&self.text_inputs, &mut next_runtime.text_inputs);

        self.swap
            .prepare(next_generation)
            .map_err(BundleHotSwapError::Prepare)?;
        self.swap
            .begin_quiescence()
            .map_err(BundleHotSwapError::Prepare)?;

        match compatibility {
            SwapCompatibility::ContentOnly => {
                self.source_label.clone_from(&bundle.manifest.source_label);
                self.display = bundle.display.clone();
                self.image_objects.clone_from(&bundle.image_objects);
                self.text_inputs.clone_from(&next_runtime.text_inputs);
                self.action_buttons.clone_from(&next_runtime.action_buttons);
                self.runtime_control_style_diagnostics
                    .clone_from(&next_runtime.runtime_control_style_diagnostics);
                self.focus_groups.clone_from(&next_runtime.focus_groups);
                self.focus_navigation
                    .clone_from(&next_runtime.focus_navigation);
            }
            SwapCompatibility::CodeCompatible => {
                self.activate_runtime(next_runtime.clone());
                self.pending_input_events.clear();
                self.pending_host_call_results.clear();
                self.waiting_text_submit_calls.clear();
                self.presentation = BundlePresentationSnapshot::default();
            }
            SwapCompatibility::CodeGenerational => {
                // The current fiber keeps running on its existing executor. The
                // new runtime image is inserted after commit and becomes the
                // binding target for new entries.
            }
            SwapCompatibility::RestartRequired => {
                unreachable!("restart-required compatibility returned before prepare")
            }
        }

        let committed = self.swap.commit().map_err(BundleHotSwapError::Commit)?;
        self.runtime_images.insert(GenerationRuntimeImage::new(
            self.swap.active().clone(),
            next_runtime,
        ))?;
        if committed == SwapCompatibility::CodeCompatible {
            self.runtime_generation_pin = Some(self.swap.pin_active_generation());
        }
        self.retire_unused_generations();
        self.next_generation_id = self.next_generation_id.saturating_add(1);
        self.active_container_content_root = active_container_content_root;
        Ok(BundleHotSwapReport {
            generation: next_id,
            compatibility: committed,
        })
    }

    fn hot_swap_bundle_with_declared_compatibility(
        &mut self,
        bundle: &ArcweftBundle,
        active_container_content_root: Option<BundleDigest>,
        compatibility: SwapCompatibility,
    ) -> Result<BundleHotSwapReport, BundleHotSwapError> {
        let next_id = GenerationId(self.next_generation_id);
        let next_generation = Arc::new(ProgramGeneration::from_bundle(next_id, bundle)?);
        if compatibility == SwapCompatibility::RestartRequired {
            return Err(BundleHotSwapError::RestartRequired { compatibility });
        }

        let actual = classify_swap(self.swap.active(), &next_generation);
        if actual == SwapCompatibility::RestartRequired {
            return Err(BundleHotSwapError::RestartRequired {
                compatibility: actual,
            });
        }
        let mut next_runtime = build_session_runtime(bundle, &self.options)?;
        preserve_runtime_text_control_values(&self.text_inputs, &mut next_runtime.text_inputs);

        self.swap
            .prepare_with_compatibility(next_generation, compatibility)
            .map_err(BundleHotSwapError::Prepare)?;
        self.swap
            .begin_quiescence()
            .map_err(BundleHotSwapError::Prepare)?;

        match compatibility {
            SwapCompatibility::ContentOnly => {
                self.source_label.clone_from(&bundle.manifest.source_label);
                self.display = bundle.display.clone();
                self.image_objects.clone_from(&bundle.image_objects);
                self.text_inputs.clone_from(&next_runtime.text_inputs);
                self.action_buttons.clone_from(&next_runtime.action_buttons);
                self.runtime_control_style_diagnostics
                    .clone_from(&next_runtime.runtime_control_style_diagnostics);
                self.focus_groups.clone_from(&next_runtime.focus_groups);
                self.focus_navigation
                    .clone_from(&next_runtime.focus_navigation);
            }
            SwapCompatibility::CodeCompatible => {
                self.activate_runtime(next_runtime.clone());
                self.pending_input_events.clear();
                self.pending_host_call_results.clear();
                self.waiting_text_submit_calls.clear();
                self.presentation = BundlePresentationSnapshot::default();
            }
            SwapCompatibility::CodeGenerational => {
                // Keep current fiber on the old executor. New entries are bound
                // to the committed active generation through the runtime table.
            }
            SwapCompatibility::RestartRequired => {
                unreachable!("restart-required compatibility returned before prepare")
            }
        }

        let committed = self.swap.commit().map_err(BundleHotSwapError::Commit)?;
        self.runtime_images.insert(GenerationRuntimeImage::new(
            self.swap.active().clone(),
            next_runtime,
        ))?;
        if committed == SwapCompatibility::CodeCompatible {
            self.runtime_generation_pin = Some(self.swap.pin_active_generation());
        }
        self.retire_unused_generations();
        self.next_generation_id = self.next_generation_id.saturating_add(1);
        self.active_container_content_root = active_container_content_root;
        Ok(BundleHotSwapReport {
            generation: next_id,
            compatibility: committed,
        })
    }

    pub fn inspect_hot_swap_patch_bytes(
        &self,
        bytes: &[u8],
    ) -> Result<BundlePatchReadinessReport, BundleHotSwapError> {
        let artifact = decode_patch_bundle(bytes).map_err(BundleHotSwapError::DecodePatch)?;
        self.inspect_hot_swap_patch_artifact(&artifact)
    }

    pub fn hot_swap_patch_bytes(
        &mut self,
        base_awfb_bytes: &[u8],
        patch_awfb_bytes: &[u8],
    ) -> Result<BundleHotSwapReport, BundleHotSwapError> {
        let artifact =
            decode_patch_bundle(patch_awfb_bytes).map_err(BundleHotSwapError::DecodePatch)?;
        let readiness = self.inspect_hot_swap_patch_artifact(&artifact)?;
        if readiness.readiness == BundlePatchReadiness::Noop {
            return Ok(BundleHotSwapReport {
                generation: self.active_generation().id,
                compatibility: SwapCompatibility::ContentOnly,
            });
        }
        let declared_compatibility =
            SwapCompatibility::from_patch_compatibility(readiness.compatibility);
        if declared_compatibility == SwapCompatibility::RestartRequired {
            return Err(BundleHotSwapError::RestartRequired {
                compatibility: declared_compatibility,
            });
        }
        let materialized = apply_patch_bundle(base_awfb_bytes, &artifact)
            .map_err(BundleHotSwapError::MaterializePatch)?;
        let target_bytes = materialized.bytes;
        let target_view =
            BundleView::parse(&target_bytes, ReadBudget::default()).map_err(|error| {
                BundleHotSwapError::DecodePatchTarget {
                    message: error.to_string(),
                }
            })?;
        let target_container_root = target_view.content_root();
        let target_bundle = ArcweftBundle::from_format_slice(BundleFormat::Awfb, &target_bytes)
            .map_err(|error| BundleHotSwapError::DecodePatchTarget {
                message: error.to_string(),
            })?;
        self.hot_swap_bundle_with_declared_compatibility(
            &target_bundle,
            Some(target_container_root),
            declared_compatibility,
        )
    }

    pub fn inspect_hot_swap_patch_artifact(
        &self,
        artifact: &BundlePatchArtifact,
    ) -> Result<BundlePatchReadinessReport, BundleHotSwapError> {
        artifact
            .validate()
            .map_err(BundleHotSwapError::InvalidPatch)?;
        let active_container_root = self
            .active_container_content_root
            .ok_or(BundleHotSwapError::MissingActiveContainerRoot)?;
        artifact
            .plan
            .validate_base(active_container_root)
            .map_err(BundleHotSwapError::WrongPatchBase)?;
        let readiness = if artifact.plan.is_empty()
            && artifact.plan.target_content_root == active_container_root
        {
            BundlePatchReadiness::Noop
        } else {
            BundlePatchReadiness::TargetBundleRequired {
                operations: artifact.plan.operations.len(),
            }
        };
        Ok(BundlePatchReadinessReport {
            base_generation: self.active_generation().id,
            base_content_root: artifact.plan.base_content_root,
            target_content_root: artifact.plan.target_content_root,
            compatibility: artifact.manifest.compatibility,
            readiness,
        })
    }

    /// Executes exactly one VM step using explicit, non-zero logical time.
    pub fn step_with_clock(
        &mut self,
        clock: RuntimeClockStep,
        mut input: BundleStepInput,
    ) -> BundleSessionStep {
        self.swap.enter_runtime_step();
        input.task_events.extend(self.tasks.drain_task_events());
        let task_events = self.tasks.apply_task_events(input.task_events);
        self.release_completed_task_generation_pins(&task_events);
        input.input_events.append(&mut self.pending_input_events);
        let text_control_write_backs = std::mem::take(&mut self.pending_text_control_write_backs);
        let runtime_input = RuntimeStepInput {
            tick: clock.tick(),
            dt: clock.dt(),
            bindings: input.bindings,
            input_events: input.input_events,
            task_events,
            audio_events: input.audio_events,
            source_events: input.source_events,
            host_call_results: std::mem::take(&mut self.pending_host_call_results),
        };
        let mut pure_backend = VmRuntimePureCallBackend::default();
        let result = self.executor.step_with_root_bindings_and_pure_backend(
            runtime_input,
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
        let flow_events = std::mem::take(&mut output.flow_events);
        let line_effects = std::mem::take(&mut output.effects.line);
        let display = resolve_display_frames(&self.display, &flow_events);
        let mut diagnostics = output
            .diagnostics
            .into_iter()
            .map(|diagnostic| diagnostic.message)
            .collect::<Vec<_>>();
        diagnostics.extend(display.diagnostics.iter().cloned());
        diagnostics.extend(
            self.runtime_control_style_diagnostics
                .diagnostics
                .iter()
                .map(ToString::to_string),
        );
        self.update_presentation_snapshot(
            &display,
            &result.fiber_status,
            &line_effects,
            &mut diagnostics,
        );
        let observations = self.executor.fiber().observations.clone();

        let requested_tasks = self.dispatch_requested_tasks(clock, output.requests.tasks);
        self.capture_text_submit_host_calls(output.requests.host_calls, &mut diagnostics);
        let cancel_scopes = output.requests.cancel_scopes;
        for scope in &cancel_scopes {
            self.tasks
                .cancel(&RuntimeTaskCancelTarget::Scope(scope.0.clone()));
        }
        let audio_commands = output.requests.audio;
        let finished = matches!(
            &result.fiber_status,
            FlowFiberStatus::Done(_) | FlowFiberStatus::Failed(_)
        );
        if finished {
            self.runtime_generation_pin = None;
            self.retire_unused_generations();
        }
        let index = self.next_step_index;
        self.next_step_index = self.next_step_index.saturating_add(1);

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
            observations,
            flow_events,
            line_effects,
            presentation: self.presentation.clone(),
            text_control_write_backs,
            audio_commands,
            requested_tasks,
            cancel_scopes,
            source_close: output.requests.source_close,
            finished,
        }
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
        let presentation_handle_diagnostics = self.presentation.update(
            display,
            status,
            line_effects,
            BundlePresentationResources {
                image_objects: &self.image_objects,
                text_inputs: &self.text_inputs,
                action_buttons: &self.action_buttons,
                focus_groups: &self.focus_groups,
                focus_navigation: &self.focus_navigation,
            },
        );
        diagnostics.extend(
            presentation_handle_diagnostics
                .iter()
                .map(ToString::to_string),
        );
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

    fn capture_text_submit_host_calls(
        &mut self,
        requests: Vec<RuntimeHostCallRequest>,
        diagnostics: &mut Vec<String>,
    ) {
        for request in requests {
            if request.capability == "ui.text_input" && request.operation == "await_submit" {
                match text_submit_control_id(&request) {
                    Some(control_id) => {
                        self.waiting_text_submit_calls.push(PendingTextSubmitCall {
                            request: request.id,
                            control_id,
                        });
                    }
                    None => self.pending_host_call_results.push(RuntimeHostCallResult {
                        id: request.id,
                        outcome: Err(RuntimeHostCallError {
                            kind: RuntimeHostCallErrorKind::Rejected,
                            message: "ui.text_input.await_submit requires one text input target"
                                .to_owned(),
                        }),
                    }),
                }
            } else {
                diagnostics.push(format!(
                    "unsupported runtime host call {}.{}",
                    request.capability, request.operation
                ));
            }
        }
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

    /// Starts a fresh foreground entry on the currently committed active generation.
    ///
    /// This intentionally preserves the current single-foreground-fiber model:
    /// starting an entry replaces only the foreground executor. Retired generation
    /// images remain live while active task pins, explicit pins, or the old
    /// foreground fiber still hold them.
    pub fn start_foreground_entry_on_current_generation(
        &mut self,
        start: BundleEntryStart,
    ) -> Result<StartedForegroundEntry, BundleEntryStartError> {
        let generation = self.swap.active_generation_id();
        let runtime = self
            .runtime_images
            .get(generation)?
            .runtime()
            .start_entry(start)?;
        let entry = runtime.entry;
        self.activate_runtime(runtime);
        self.runtime_generation_pin = Some(self.swap.pin_active_generation());
        self.pending_input_events.clear();
        self.presentation = BundlePresentationSnapshot::default();
        self.retire_unused_generations();
        Ok(StartedForegroundEntry { generation, entry })
    }

    fn activate_runtime(&mut self, runtime: SessionRuntime) {
        self.source_label = runtime.source_label;
        self.executor = runtime.executor;
        self.display = runtime.display;
        self.image_objects = runtime.image_objects;
        self.text_inputs = runtime.text_inputs;
        self.action_buttons = runtime.action_buttons;
        self.runtime_control_style_diagnostics = runtime.runtime_control_style_diagnostics;
        self.focus_groups = runtime.focus_groups;
        self.focus_navigation = runtime.focus_navigation;
    }

    fn prune_runtime_images(&mut self) {
        let live = self.swap.live_generation_ids();
        self.runtime_images.retain_generations(&live);
    }

    fn release_table_only_retired_runtime_images(&mut self) {
        let table_only_generations = self
            .swap
            .retired()
            .iter()
            .filter(|generation| {
                self.runtime_images.contains_generation(generation.id)
                    && Arc::strong_count(generation) <= 2
            })
            .map(|generation| generation.id)
            .collect::<Vec<_>>();
        for generation in table_only_generations {
            self.runtime_images.remove(generation);
        }
    }
}

impl RuntimeTaskOwner for BundleSession {
    fn runtime_tasks(&self, options: RuntimeTaskListOptions) -> Vec<RuntimeTaskRecord> {
        BundleSession::runtime_tasks(self, options)
    }

    fn cancel_runtime_tasks(
        &mut self,
        target: RuntimeTaskCancelTarget,
    ) -> RuntimeTaskCancelOutcome {
        BundleSession::cancel_runtime_tasks(self, &target)
    }
}

fn apply_text_control_write_back_to_controls(
    text_inputs: &mut [UiRuntimeTextControl],
    write_back: &TextControlWriteBack,
) -> Result<RuntimeTextControlWriteBack, BundleSessionError> {
    let target = write_back.target().id().as_str().to_owned();
    let session = write_back.session().0;
    let Some(control) = text_inputs
        .iter_mut()
        .find(|control| control.target == target && control.session == session)
    else {
        return Err(BundleSessionError::UnknownTextControlWriteBackTarget { target, session });
    };
    write_back.value().as_str().clone_into(&mut control.value);
    control.selection = UiRuntimeTextSelection::new(
        write_back.selection().start().get(),
        write_back.selection().end().get(),
    );
    Ok(RuntimeTextControlWriteBack::from_control(
        write_back, control,
    ))
}

fn preserve_runtime_text_control_values(
    current: &[UiRuntimeTextControl],
    next: &mut [UiRuntimeTextControl],
) {
    for next_control in next.iter_mut() {
        if let Some(current_control) = current.iter().find(|current_control| {
            same_runtime_text_control_identity(current_control, next_control)
        }) {
            next_control.value.clone_from(&current_control.value);
            next_control.selection = current_control.selection;
        }
    }
}

fn same_runtime_text_control_identity(
    left: &UiRuntimeTextControl,
    right: &UiRuntimeTextControl,
) -> bool {
    left.public_id == right.public_id
        && left.target == right.target
        && left.session == right.session
}

#[cfg(test)]
mod text_control_writeback_tests {
    use super::*;
    use arcweft_bundle::resource_codec::ui::{
        CompositionOnBlurPolicy, EnterKeyHint, TextAssistPolicy, TextCapitalization, UiInputKind,
        UiInputPurpose, UiRuntimeControlStyle, UiRuntimeTextControlBounds,
        UiRuntimeTextControlHandlers, UiRuntimeTextControlOptions, UiSecureInputPolicy,
        UiTextSelectionPolicy, UiTextShortcutPolicy, UiTextTabPolicy,
        UiTextVerticalNavigationPolicy,
    };
    use arcweft_core::step::RuntimeHostCallMode;
    use arcweft_id::PublicId;
    use arcweft_presentation::input::InteractionTarget as PresentationTarget;
    use arcweft_presentation::text_input::{
        TextByteOffset, TextControlValue, TextInputSessionId, TextRange, TextRevision,
    };

    fn runtime_control(target: &str, session: u64, value: &str) -> UiRuntimeTextControl {
        UiRuntimeTextControl {
            public_id: target.to_owned(),
            target: target.to_owned(),
            session,
            value: value.to_owned(),
            selection: UiRuntimeTextSelection::collapsed_at_end(value),
            options: UiRuntimeTextControlOptions {
                purpose: UiInputPurpose::Text,
                autocorrect: TextAssistPolicy::PlatformDefault,
                spellcheck: TextAssistPolicy::PlatformDefault,
                capitalization: TextCapitalization::None,
                enter_key: EnterKeyHint::Default,
                multiline: false,
                selection_policy: UiTextSelectionPolicy::Enabled,
                shortcut_policy: UiTextShortcutPolicy::Enabled,
                tab_policy: UiTextTabPolicy::FocusNavigation,
                vertical_navigation_policy: UiTextVerticalNavigationPolicy::LogicalLine,
                secure_policy: UiSecureInputPolicy::Plain,
                composition_on_blur: CompositionOnBlurPolicy::Commit,
            },
            kind: UiInputKind::TextField,
            bounds: UiRuntimeTextControlBounds::from_px(0, 0, 100, 24),
            label: None,
            handlers: UiRuntimeTextControlHandlers::default(),
            style: UiRuntimeControlStyle::default(),
        }
    }

    #[test]
    fn write_back_updates_runtime_overlay_and_returns_typed_event() {
        let mut controls = vec![runtime_control("field.name", 7, "old")];
        let write_back = TextControlWriteBack::change(
            PresentationTarget::new(PublicId::try_new("field.name").unwrap()),
            TextInputSessionId(7),
            TextControlValue::plain("new"),
            TextRange::new(TextByteOffset(3), TextByteOffset(3)),
            TextRevision(1),
        );

        let event = apply_text_control_write_back_to_controls(&mut controls, &write_back).unwrap();

        assert_eq!(controls[0].value, "new");
        assert_eq!(event.value().as_str(), "new");
        assert!(event.is_change());
    }

    #[test]
    fn hot_swap_preserves_matching_runtime_text_value_and_drops_removed_controls() {
        let current = vec![runtime_control("field.name", 7, "edited")];
        let mut next = vec![runtime_control("field.name", 7, "default")];
        preserve_runtime_text_control_values(&current, &mut next);
        assert_eq!(next[0].value, "edited");

        let mut incompatible = vec![runtime_control("field.other", 8, "default")];
        preserve_runtime_text_control_values(&current, &mut incompatible);
        assert_eq!(incompatible[0].value, "default");
    }

    #[test]
    fn text_submit_host_call_keeps_input_family_target() {
        let request = RuntimeHostCallRequest {
            id: RuntimeHostCallId("host.text_submit.0".to_owned()),
            public_id: "host.text_submit.0".to_owned(),
            capability: "ui.text_input".to_owned(),
            operation: "await_submit".to_owned(),
            args: vec![RuntimePayload::from("input.feedback")],
            mode: RuntimeHostCallMode::Suspend,
            deterministic: true,
        };

        assert_eq!(
            text_submit_control_id(&request).as_deref(),
            Some("input.feedback")
        );
    }
}

#[derive(Clone, Debug)]
struct SessionRuntime {
    source_label: String,
    program: AwbcProgram,
    entry: AwbcEntryId,
    executor: ArcweftRuntimeExecutor,
    display: LineDisplayCatalog,
    image_objects: Vec<BundleImageObject>,
    text_inputs: Vec<UiRuntimeTextControl>,
    action_buttons: Vec<UiRuntimeActionButton>,
    runtime_control_style_diagnostics: UiRuntimeControlStyleDiagnostics,
    focus_groups: Vec<UiRuntimeFocusGroup>,
    focus_navigation: Vec<UiRuntimeFocusNavigation>,
}

#[derive(Clone, Debug)]
struct SessionRuntimeResources {
    display: LineDisplayCatalog,
    image_objects: Vec<BundleImageObject>,
    text_inputs: Vec<UiRuntimeTextControl>,
    action_buttons: Vec<UiRuntimeActionButton>,
    runtime_control_style_diagnostics: UiRuntimeControlStyleDiagnostics,
    focus_groups: Vec<UiRuntimeFocusGroup>,
    focus_navigation: Vec<UiRuntimeFocusNavigation>,
}

fn initial_generation(bundle: &ArcweftBundle) -> Result<ProgramGeneration, BundleSessionError> {
    ProgramGeneration::from_bundle(GenerationId(0), bundle).map_err(|error| match error {
        GenerationBuildError::UnsupportedBundleKind(kind) => {
            BundleSessionError::UnsupportedBundleKind(kind)
        }
        GenerationBuildError::VerifyBytecode(error) => BundleSessionError::VerifyBytecode(error),
        GenerationBuildError::ProductAwbcVerification { message } => {
            BundleSessionError::ProductAwbcVerification { message }
        }
        GenerationBuildError::EncodeFingerprint(error) => {
            BundleSessionError::GenerationFingerprint {
                message: error.to_string(),
            }
        }
        GenerationBuildError::AdapterRequirementFingerprint { message } => {
            BundleSessionError::GenerationFingerprint { message }
        }
    })
}

impl SessionRuntime {
    fn new(
        source_label: String,
        program: AwbcProgram,
        entry: AwbcEntryId,
        resources: SessionRuntimeResources,
    ) -> Result<Self, AwbcProductStepBuildError> {
        let executor = ArcweftRuntimeExecutor::from_awbc_product(program.clone(), entry)?;
        Ok(Self {
            source_label,
            program,
            entry,
            executor,
            display: resources.display,
            image_objects: resources.image_objects,
            text_inputs: resources.text_inputs,
            action_buttons: resources.action_buttons,
            runtime_control_style_diagnostics: resources.runtime_control_style_diagnostics,
            focus_groups: resources.focus_groups,
            focus_navigation: resources.focus_navigation,
        })
    }

    fn start_entry(&self, start: BundleEntryStart) -> Result<Self, BundleEntryStartError> {
        let entry = match start {
            BundleEntryStart::SessionDefault => self.entry,
            BundleEntryStart::Entry(entry) => entry,
        };
        ensure_start_awbc_entry_selects_flow(&self.program, entry)?;
        Self::new(
            self.source_label.clone(),
            self.program.clone(),
            entry,
            SessionRuntimeResources {
                display: self.display.clone(),
                image_objects: self.image_objects.clone(),
                text_inputs: self.text_inputs.clone(),
                action_buttons: self.action_buttons.clone(),
                runtime_control_style_diagnostics: self.runtime_control_style_diagnostics.clone(),
                focus_groups: self.focus_groups.clone(),
                focus_navigation: self.focus_navigation.clone(),
            },
        )
        .map_err(BundleEntryStartError::from)
    }
}

fn build_session_runtime(
    bundle: &ArcweftBundle,
    options: &BundleSessionOptions,
) -> Result<SessionRuntime, BundleSessionError> {
    if bundle.bundle_kind != BundleKind::Game {
        return Err(BundleSessionError::UnsupportedBundleKind(
            bundle.bundle_kind,
        ));
    }

    let program = bundle
        .product_awbc_program()
        .map_err(|_| BundleSessionError::MissingProductAwbc)?
        .clone();
    let entry = selected_awbc_entry(&program, bundle, options)?;
    ensure_session_awbc_entry_selects_flow(&program, entry)?;
    let text_controls = bundle
        .ui_input
        .as_ref()
        .map_or_else(Default::default, |input| {
            input.runtime_text_controls_with_style(
                bundle.ui_text.as_ref(),
                bundle.ui_program.as_ref(),
                bundle.ui_style.as_ref(),
            )
        });
    let action_button_controls =
        bundle
            .ui_program
            .as_ref()
            .map_or_else(Default::default, |program| {
                program.runtime_action_buttons_with_style(
                    bundle.ui_text.as_ref(),
                    bundle.ui_style.as_ref(),
                )
            });
    let text_inputs = text_controls.controls;
    let action_buttons = action_button_controls.controls;
    let mut runtime_control_style_diagnostics = text_controls.diagnostics;
    runtime_control_style_diagnostics.extend(action_button_controls.diagnostics);
    let focus_groups = bundle
        .ui_program
        .as_ref()
        .map_or_else(Vec::new, UiProgramResource::runtime_focus_groups);
    let focus_navigation = bundle
        .ui_program
        .as_ref()
        .map_or_else(Vec::new, UiProgramResource::runtime_focus_navigation);

    SessionRuntime::new(
        bundle.manifest.source_label.clone(),
        program,
        entry,
        SessionRuntimeResources {
            display: bundle.display.clone(),
            image_objects: bundle.image_objects.clone(),
            text_inputs,
            action_buttons,
            runtime_control_style_diagnostics,
            focus_groups,
            focus_navigation,
        },
    )
    .map_err(BundleSessionError::from)
}

fn text_submit_control_id(request: &RuntimeHostCallRequest) -> Option<String> {
    let value = request.args.first()?.value();
    match value {
        RuntimeValue::EntityRef(value) | RuntimeValue::String(value) => Some(value.to_owned()),
        _ => None,
    }
}

fn selected_awbc_entry(
    program: &AwbcProgram,
    bundle: &ArcweftBundle,
    options: &BundleSessionOptions,
) -> Result<AwbcEntryId, BundleSessionError> {
    if options.entry.is_some() && options.flow.is_some() {
        return Err(BundleSessionError::ConflictingEntrySelection);
    }
    if let Some(flow) = options.flow.as_deref() {
        return Err(BundleSessionError::UnknownFlow {
            flow: RuntimeEntityFamily::Flow.selector(flow),
        });
    }
    let Some(entry) = selected_entry(bundle, options) else {
        return Ok(AwbcEntryId(0));
    };
    let selected = RuntimeEntityFamily::Entry.selector(entry);
    program
        .entries
        .iter()
        .enumerate()
        .find_map(|(index, candidate)| {
            let public_id = program.strings.get(candidate.public_id.index())?;
            (public_id == entry || public_id == &selected)
                .then(|| AwbcEntryId(u32::try_from(index).unwrap_or(u32::MAX)))
        })
        .ok_or(BundleSessionError::ProductAwbcEntry { entry: selected })
}

fn selected_entry<'a>(
    bundle: &'a ArcweftBundle,
    options: &'a BundleSessionOptions,
) -> Option<&'a str> {
    options.entry.as_deref().or_else(|| {
        options
            .flow
            .is_none()
            .then_some(bundle.manifest.entry.as_deref())
            .flatten()
    })
}

fn ensure_session_awbc_entry_selects_flow(
    program: &AwbcProgram,
    entry: AwbcEntryId,
) -> Result<(), BundleSessionError> {
    if awbc_entry_selects_flow(program, entry) {
        Ok(())
    } else {
        Err(BundleSessionError::NonFlowEntry {
            entry: awbc_entry_label(program, entry),
        })
    }
}

fn ensure_start_awbc_entry_selects_flow(
    program: &AwbcProgram,
    entry: AwbcEntryId,
) -> Result<(), BundleEntryStartError> {
    if !awbc_entry_exists_or_empty_program_default(program, entry) {
        return Err(BundleEntryStartError::UnknownEntry { entry });
    }
    if awbc_entry_selects_flow(program, entry) {
        Ok(())
    } else {
        Err(BundleEntryStartError::NonFlowEntry { entry })
    }
}

fn awbc_entry_exists_or_empty_program_default(program: &AwbcProgram, entry: AwbcEntryId) -> bool {
    program.entries.get(entry.index()).is_some()
        || (program.entries.is_empty() && entry == AwbcEntryId(0))
}

fn awbc_entry_selects_flow(program: &AwbcProgram, entry: AwbcEntryId) -> bool {
    if program.entries.is_empty() && entry == AwbcEntryId(0) {
        return true;
    }
    let Some(entry) = program.entries.get(entry.index()) else {
        return false;
    };
    let Some(function) = entry.target.function() else {
        return false;
    };
    program
        .functions
        .get(function.index())
        .is_some_and(|function| function.kind.is_flow())
}

fn awbc_entry_label(program: &AwbcProgram, entry: AwbcEntryId) -> String {
    program
        .entries
        .get(entry.index())
        .and_then(|entry| program.strings.get(entry.public_id.index()).cloned())
        .unwrap_or_else(|| format!("entry#{}", entry.0))
}
