use crate::clock::RuntimeClockStep;
use crate::display::{BundlePresentationSnapshot, resolve_display_frames};
use crate::task::HostTaskDispatch;
use arcweft_bundle::{ArcweftBundle, BundleImageObject, BundleKind};
use arcweft_core::bytecode::BytecodeProgram;
use arcweft_core::effect::LineEffectRequest;
use arcweft_core::engine::{FlowFiberStatus, FlowStatusLabelStyle};
use arcweft_core::executor::{BytecodeVmExecutor, RuntimeExecutor};
use arcweft_core::plan::{
    FlowEvent, FlowRuntimeId, RuntimeEntryTarget, RuntimePlan, RuntimePlanError,
};
use arcweft_core::pure::VmRuntimePureCallBackend;
use arcweft_core::source::{RuntimeSourceEvent, SourceId};
use arcweft_core::step::{
    RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions, RuntimeStepStats,
    RuntimeStepStopReason,
};
use arcweft_core::task::{CancelScopeId, LogicalEpoch, TaskEvent, TaskSequence};
use arcweft_core::value::RuntimeBinding;
use arcweft_interaction_model::audio::AudioEvent;
use arcweft_interaction_model::id::Identifier;
use arcweft_interaction_model::input::{
    InputEpoch, InputEventKind, InputSequence, InteractionTarget, RoutedInputEvent,
};
use arcweft_interaction_model::payload::InteractionPayload;
use arcweft_presentation::input::Action;
use arcweft_render_text::LineDisplayCatalog;
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
    pub requested_tasks: Vec<HostTaskDispatch>,
    pub cancel_scopes: Vec<CancelScopeId>,
    pub source_close: Vec<SourceId>,
    pub finished: bool,
}

/// Portable decoded bundle execution session.
#[derive(Clone, Debug, PartialEq)]
pub struct BundleSession {
    source_label: String,
    executor: BytecodeVmExecutor,
    display: LineDisplayCatalog,
    image_objects: Vec<BundleImageObject>,
    options: BundleSessionOptions,
    pending_input_events: Vec<RoutedInputEvent>,
    presentation: BundlePresentationSnapshot,
    next_step_index: usize,
    next_task_sequence: u64,
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
    #[error("unsupported semantic action `{action}` at the game runtime boundary")]
    UnsupportedSemanticAction { action: String },
    #[error("semantic action `{action}` is missing its option payload")]
    MissingSemanticActionPayload { action: String },
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
}

impl RuntimeInputKind {
    const fn as_str(self) -> &'static str {
        match self {
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
        if bundle.bundle_kind != BundleKind::Game {
            return Err(BundleSessionError::UnsupportedBundleKind(
                bundle.bundle_kind,
            ));
        }

        let mut plan = bundle.bytecode.program.clone().into_runtime_plan()?;
        apply_entry_selection(
            &mut plan,
            selected_entry(bundle, &options),
            options.flow.as_deref(),
        )?;
        let bytecode = BytecodeProgram::from_runtime_plan(plan.clone());

        Ok(Self {
            source_label: bundle.manifest.source_label.clone(),
            executor: BytecodeVmExecutor::from_parts(bytecode, plan),
            display: bundle.display.clone(),
            image_objects: bundle.image_objects.clone(),
            options,
            pending_input_events: Vec::new(),
            presentation: BundlePresentationSnapshot::default(),
            next_step_index: 0,
            next_task_sequence: 0,
        })
    }

    pub fn source_label(&self) -> &str {
        &self.source_label
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

    /// Executes exactly one VM step using explicit, non-zero logical time.
    pub fn step_with_clock(
        &mut self,
        clock: RuntimeClockStep,
        mut input: BundleStepInput,
    ) -> BundleSessionStep {
        input.input_events.append(&mut self.pending_input_events);
        let runtime_input = RuntimeStepInput {
            tick: clock.tick(),
            dt: clock.dt(),
            bindings: input.bindings,
            input_events: input.input_events,
            task_events: input.task_events,
            audio_events: input.audio_events,
            source_events: input.source_events,
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

        let requested_tasks = output
            .requests
            .tasks
            .into_iter()
            .map(|task| {
                let sequence = TaskSequence(self.next_task_sequence);
                self.next_task_sequence = self.next_task_sequence.saturating_add(1);
                HostTaskDispatch {
                    logical_epoch: LogicalEpoch(clock.tick().0),
                    sequence,
                    task,
                }
            })
            .collect();
        let finished = matches!(
            &result.fiber_status,
            FlowFiberStatus::Done(_) | FlowFiberStatus::Failed(_)
        );
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

fn apply_entry_selection(
    plan: &mut RuntimePlan,
    entry: Option<&str>,
    flow: Option<&str>,
) -> Result<(), BundleSessionError> {
    if entry.is_some() && flow.is_some() {
        return Err(BundleSessionError::ConflictingEntrySelection);
    }
    if let Some(flow) = flow {
        let flow = FlowRuntimeId(RuntimeEntityFamily::Flow.selector(flow));
        if !plan.flows.iter().any(|candidate| candidate.id == flow) {
            return Err(BundleSessionError::UnknownFlow { flow: flow.0 });
        }
        plan.entry_flow = Some(flow);
        return Ok(());
    }
    if let Some(entry) = entry {
        let entry = RuntimeEntityFamily::Entry.selector(entry);
        let Some(spec) = plan
            .entries
            .iter()
            .find(|candidate| candidate.id.0 == entry)
        else {
            return Err(BundleSessionError::UnknownEntry { entry });
        };
        let RuntimeEntryTarget::Flow(flow) = &spec.target else {
            return Err(BundleSessionError::NonFlowEntry { entry });
        };
        plan.entry_flow = Some(flow.clone());
    }
    Ok(())
}
