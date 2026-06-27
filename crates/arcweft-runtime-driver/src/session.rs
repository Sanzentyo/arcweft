use crate::clock::RuntimeClockStep;
use crate::display::{BundlePresentationSnapshot, resolve_display_frames};
use crate::swap::{
    GenerationBuildError, GenerationId, ProgramGeneration, SwapCompatibility, SwapError,
    SwapSession, classify_swap,
};
use crate::task::HostTaskDispatch;
use arcweft_bundle::container::{BundleDigest, BundleView, ReadBudget};
use arcweft_bundle::patch::{
    BundlePatchArtifact, PatchBundleError, PatchCompatibility, PatchValidationError,
    apply_patch_bundle, decode_patch_bundle,
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
use arcweft_core::plan::{FlowEvent, RuntimePlanError};
use arcweft_core::pure::VmRuntimePureCallBackend;
use arcweft_core::source::{RuntimeSourceEvent, SourceId};
use arcweft_core::step::{
    RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions, RuntimeStepStats,
    RuntimeStepStopReason,
};
use arcweft_core::task::{CancelScopeId, LogicalEpoch, TaskEvent, TaskSequence};
use arcweft_core::value::RuntimeBinding;
use arcweft_interaction_model::audio::{AudioCommandEnvelope, AudioEvent};
use arcweft_interaction_model::id::Identifier;
use arcweft_interaction_model::input::{
    InputEpoch, InputEventKind, InputSequence, InteractionTarget, RoutedInputEvent,
};
use arcweft_interaction_model::payload::InteractionPayload;
use arcweft_presentation::input::Action;
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
    pub flow_events: Vec<FlowEvent>,
    pub line_effects: Vec<LineEffectRequest>,
    pub presentation: BundlePresentationSnapshot,
    pub audio_commands: Vec<AudioCommandEnvelope>,
    pub requested_tasks: Vec<HostTaskDispatch>,
    pub cancel_scopes: Vec<CancelScopeId>,
    pub source_close: Vec<SourceId>,
    pub finished: bool,
}

/// Portable decoded bundle execution session.
#[derive(Clone, Debug)]
pub struct BundleSession {
    source_label: String,
    executor: ArcweftRuntimeExecutor,
    display: LineDisplayCatalog,
    image_objects: Vec<BundleImageObject>,
    options: BundleSessionOptions,
    pending_input_events: Vec<RoutedInputEvent>,
    presentation: BundlePresentationSnapshot,
    next_step_index: usize,
    next_task_sequence: u64,
    swap: SwapSession,
    runtime_generation_pin: Option<Arc<ProgramGeneration>>,
    task_generation_pins: BTreeMap<TaskSequence, Arc<ProgramGeneration>>,
    next_generation_id: u64,
    active_container_content_root: Option<BundleDigest>,
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

        Ok(Self {
            source_label: runtime.source_label,
            executor: runtime.executor,
            display: runtime.display,
            image_objects: runtime.image_objects,
            options,
            pending_input_events: Vec::new(),
            presentation: BundlePresentationSnapshot::default(),
            next_step_index: 0,
            next_task_sequence: 0,
            swap: SwapSession::new(generation.clone()),
            runtime_generation_pin: Some(generation),
            task_generation_pins: BTreeMap::new(),
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

    pub fn pin_active_generation(&self) -> Arc<ProgramGeneration> {
        self.swap.pin_active_generation()
    }

    pub fn retired_generation_count(&self) -> usize {
        self.swap.retired().len()
    }

    pub fn retire_unused_generations(&mut self) {
        self.swap.retire_unused();
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
        if matches!(
            compatibility,
            SwapCompatibility::RestartRequired | SwapCompatibility::CodeGenerational
        ) {
            return Err(BundleHotSwapError::RestartRequired { compatibility });
        }

        let runtime = (compatibility == SwapCompatibility::CodeCompatible)
            .then(|| build_session_runtime(bundle, &self.options))
            .transpose()?;

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
            }
            SwapCompatibility::CodeCompatible => {
                let Some(runtime) = runtime else {
                    return Err(BundleHotSwapError::RestartRequired { compatibility });
                };
                self.source_label = runtime.source_label;
                self.executor = runtime.executor;
                self.display = runtime.display;
                self.image_objects = runtime.image_objects;
                self.pending_input_events.clear();
                self.presentation = BundlePresentationSnapshot::default();
            }
            SwapCompatibility::CodeGenerational | SwapCompatibility::RestartRequired => {
                unreachable!("restart-required compatibilities returned before prepare")
            }
        }

        let committed = self.swap.commit().map_err(BundleHotSwapError::Commit)?;
        if committed == SwapCompatibility::CodeCompatible {
            self.runtime_generation_pin = Some(self.swap.pin_active_generation());
        }
        self.swap.retire_unused();
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
        if matches!(
            compatibility,
            SwapCompatibility::RestartRequired | SwapCompatibility::CodeGenerational
        ) {
            return Err(BundleHotSwapError::RestartRequired { compatibility });
        }

        let runtime = (compatibility == SwapCompatibility::CodeCompatible)
            .then(|| build_session_runtime(bundle, &self.options))
            .transpose()?;

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
            }
            SwapCompatibility::CodeCompatible => {
                let Some(runtime) = runtime else {
                    return Err(BundleHotSwapError::RestartRequired { compatibility });
                };
                self.source_label = runtime.source_label;
                self.executor = runtime.executor;
                self.display = runtime.display;
                self.image_objects = runtime.image_objects;
                self.pending_input_events.clear();
                self.presentation = BundlePresentationSnapshot::default();
            }
            SwapCompatibility::CodeGenerational | SwapCompatibility::RestartRequired => {
                unreachable!("restart-required compatibilities returned before prepare")
            }
        }

        let committed = self.swap.commit().map_err(BundleHotSwapError::Commit)?;
        if committed == SwapCompatibility::CodeCompatible {
            self.runtime_generation_pin = Some(self.swap.pin_active_generation());
        }
        self.swap.retire_unused();
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
        if matches!(
            declared_compatibility,
            SwapCompatibility::CodeGenerational | SwapCompatibility::RestartRequired
        ) {
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
        self.release_completed_task_generation_pins(&input.task_events);
        input.input_events.append(&mut self.pending_input_events);
        let runtime_input = RuntimeStepInput {
            tick: clock.tick(),
            dt: clock.dt(),
            bindings: input.bindings,
            input_events: input.input_events,
            task_events: input.task_events,
            audio_events: input.audio_events,
            source_events: input.source_events,
            host_call_results: Vec::new(),
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
        self.presentation.update(
            &display,
            &result.fiber_status,
            &line_effects,
            &self.image_objects,
        );

        let requested_tasks = self.dispatch_requested_tasks(clock, output.requests.tasks);
        let audio_commands = output.requests.audio;
        let finished = matches!(
            &result.fiber_status,
            FlowFiberStatus::Done(_) | FlowFiberStatus::Failed(_)
        );
        if finished {
            self.runtime_generation_pin = None;
            self.swap.retire_unused();
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
            flow_events,
            line_effects,
            presentation: self.presentation.clone(),
            audio_commands,
            requested_tasks,
            cancel_scopes: output.requests.cancel_scopes,
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
                HostTaskDispatch {
                    logical_epoch: LogicalEpoch(clock.tick().0),
                    sequence,
                    task,
                }
            })
            .collect()
    }

    fn release_completed_task_generation_pins(&mut self, task_events: &[TaskEvent]) {
        if task_events.is_empty() {
            return;
        }
        for event in task_events {
            self.task_generation_pins.remove(&event.sequence);
        }
        self.swap.retire_unused();
    }
}

#[derive(Debug)]
struct SessionRuntime {
    source_label: String,
    executor: ArcweftRuntimeExecutor,
    display: LineDisplayCatalog,
    image_objects: Vec<BundleImageObject>,
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

    Ok(SessionRuntime {
        source_label: bundle.manifest.source_label.clone(),
        executor: ArcweftRuntimeExecutor::from_awbc_product(program, entry)?,
        display: bundle.display.clone(),
        image_objects: bundle.image_objects.clone(),
    })
}

fn selected_awbc_entry(
    program: &AwbcProgram,
    bundle: &ArcweftBundle,
    options: &BundleSessionOptions,
) -> Result<AwbcEntryId, BundleSessionError> {
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
