use crate::effect::LineEffectRequest;
use crate::entry::{RuntimeCallableExecutableCode, RuntimeCallableRole};
use crate::line_task::run_line_task_group_for_input;
use crate::observation::RuntimeObservationState;
use crate::pattern::{RuntimePattern, match_runtime_pattern};
use crate::plan::{
    ChoiceRuntimeOption, EntryRuntimeId, FlowEvent, FlowOp, FlowRuntimeId, RuntimeEntryTarget,
    RuntimeFlow, RuntimeMatchArm, RuntimeMatchSelection, RuntimePlan,
};
use crate::pure::{RuntimeCallBackend, VmPureFunctionScratch, VmRuntimePureCallBackend};
use crate::root::{
    RootCallableEvaluationError, RootCallableEvaluator, RootEventInput, RootRuntime,
    RootRuntimeError, RootStartupContract, RuntimeCommandEnvelope,
};
use crate::source::{
    RuntimeSourceEvent, SourceEventKind, SourceHandlerPlan, SourceId, SourceOp, SourcePlan,
    SourcePolicy, SourceRuntimeState, normalize_source_events,
};
use crate::step::{
    RuntimeDiagnostic, RuntimeDiagnosticCategory, RuntimeHostCallId, RuntimeStepInput,
    RuntimeStepMode, RuntimeStepOptions, RuntimeStepOutput, RuntimeStepResult, RuntimeStepStats,
    RuntimeStepStopReason,
};
use crate::stream::{
    RuntimeStreamEvent, StreamMatchArm, StreamOp, StreamRuntimeId, StreamRuntimeState,
};
use crate::task::{
    AwaitManyTarget, AwaitTarget, CancelScopeId, NeedId, TaskEvent, TaskEventKind, TaskId, TaskKey,
    TaskPolicy, TaskPriority, TaskSpec, normalize_task_events,
};
use crate::value::{
    RuntimeBinding, RuntimeEnv, RuntimeEvalError, RuntimeExpr, RuntimeExprMatchArm,
    RuntimeFieldValue, RuntimeFunctionValue, RuntimeISizeValue, RuntimeIterator, RuntimePayload,
    RuntimeSeq, RuntimeUSizeValue, RuntimeValue, evaluate_binary, evaluate_unary,
    runtime_sequence_dense_f32, runtime_sequence_dense_f64, runtime_sequence_dense_i8,
    runtime_sequence_dense_i16, runtime_sequence_dense_i32, runtime_sequence_dense_i64,
    runtime_sequence_dense_i128, runtime_sequence_dense_u8, runtime_sequence_dense_u16,
    runtime_sequence_dense_u32, runtime_sequence_dense_u64, runtime_sequence_dense_u128,
    runtime_sequence_from_literal_values, runtime_sequence_repeat_value, runtime_sequence_values,
    runtime_value_into_sequence_values, runtime_value_label, sum_i64_sequence_ref,
};
use std::collections::{BTreeMap, VecDeque};
use thiserror::Error;
pub mod aot;
pub mod audio;
pub mod eval;
pub(crate) use eval::evaluate_runtime_call;
pub mod flow;
pub mod line;
pub mod source;
pub mod stream;
pub mod suspend;

#[derive(Clone, Debug, PartialEq)]
pub struct Engine {
    plan: RuntimePlan,
    flow_positions: BTreeMap<FlowRuntimeId, usize>,
    main_started: bool,
    root: Option<RootRuntime>,
    root_flow_binding: Option<RuntimeBinding>,
    fiber: FlowFiber,
    child_fibers: VecDeque<FlowFiber>,
    run_child_next: bool,
    pure_i8_batch_inputs: Vec<i8>,
    pure_i8_batch_outputs: Vec<i8>,
    pure_i16_batch_inputs: Vec<i16>,
    pure_i16_batch_outputs: Vec<i16>,
    pure_i32_batch_inputs: Vec<i32>,
    pure_i128_batch_inputs: Vec<i128>,
    pure_i128_batch_outputs: Vec<i128>,
    pure_i32_batch_outputs: Vec<i32>,
    pure_u8_batch_inputs: Vec<u8>,
    pure_u8_batch_outputs: Vec<u8>,
    pure_u16_batch_inputs: Vec<u16>,
    pure_u16_batch_outputs: Vec<u16>,
    pure_u32_batch_inputs: Vec<u32>,
    pure_u32_batch_outputs: Vec<u32>,
    pure_u64_batch_inputs: Vec<u64>,
    pure_u64_batch_outputs: Vec<u64>,
    pure_u128_batch_inputs: Vec<u128>,
    pure_u128_batch_outputs: Vec<u128>,
    pure_isize_batch_inputs: Vec<RuntimeISizeValue>,
    pure_isize_batch_outputs: Vec<RuntimeISizeValue>,
    pure_usize_batch_inputs: Vec<RuntimeUSizeValue>,
    pure_usize_batch_outputs: Vec<RuntimeUSizeValue>,
    pure_f32_batch_inputs: Vec<f32>,
    pure_f32_batch_outputs: Vec<f32>,
    pure_f64_batch_inputs: Vec<f64>,
    pure_f64_batch_outputs: Vec<f64>,
    pure_i64_batch_inputs: Vec<i64>,
    pure_i64_batch_outputs: Vec<i64>,
    pure_helper_i32_call_shapes: Vec<bool>,
    pure_helper_u32_call_shapes: Vec<bool>,
    pure_helper_i64_call_shapes: Vec<bool>,
    pure_helper_f32_call_shapes: Vec<bool>,
    pure_helper_f64_call_shapes: Vec<bool>,
    audio_epoch: u64,
    next_audio_sequence: u64,
    next_host_call_sequence: u64,
}

struct PureHelperCallShapes {
    i32: Vec<bool>,
    u32: Vec<bool>,
    i64: Vec<bool>,
    f32: Vec<bool>,
    f64: Vec<bool>,
}

/// Current flow execution cursor.
#[derive(Clone, Debug, PartialEq)]
pub struct FlowFiber {
    pub line_cursor: usize,
    pub cursor: Option<FlowCursor>,
    pub pending_ops: VecDeque<FlowOp>,
    pub control_stack: Vec<FlowControlStackEntry>,
    pub root_cleanups: Vec<FlowScopeCleanup>,
    pub env: RuntimeEnv,
    pub observations: RuntimeObservationState,
    pub source_states: BTreeMap<SourceId, SourceRuntimeState>,
    pub stream_states: BTreeMap<StreamRuntimeId, StreamRuntimeState>,
    pub status: FlowFiberStatus,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FlowControlStackEntry {
    pub kind: FlowControlStackEntryKind,
}

/// One deterministic cleanup effect registered against a lexical flow scope.
#[derive(Clone, Debug, PartialEq)]
pub struct FlowScopeCleanup {
    pub key: String,
    pub effect: LineEffectRequest,
}

impl FlowScopeCleanup {
    pub fn new(key: impl Into<String>, effect: LineEffectRequest) -> Self {
        Self {
            key: key.into(),
            effect,
        }
    }
}

/// Structured frame kind for the minimal flow executor.
#[derive(Clone, Debug, PartialEq)]
pub enum FlowControlStackEntryKind {
    Scope {
        cleanups: Vec<FlowScopeCleanup>,
    },
    Loop {
        body: std::sync::Arc<[FlowOp]>,
        result: Option<RuntimePattern>,
    },
    While {
        condition: RuntimeExpr,
        body: std::sync::Arc<[FlowOp]>,
    },
    WhileLet {
        pattern: RuntimePattern,
        expr: RuntimeExpr,
        guard: Option<Box<RuntimeExpr>>,
        body: std::sync::Arc<[FlowOp]>,
    },
}

/// Position in a lowered flow program.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FlowCursor {
    pub flow_index: usize,
    pub op_index: usize,
}

/// Failure to select an explicit flow program before execution.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum EngineStartError {
    #[error("runtime flow `{flow}` does not exist")]
    MissingFlow { flow: String },
    #[error("runtime entry `{entry}` does not exist")]
    MissingEntry { entry: String },
    #[error("runtime entry `{entry}` does not select one flow")]
    EntryDoesNotSelectFlow { entry: String },
    #[error("runtime engine already has a selected flow")]
    AlreadyStarted,
    #[error("runtime entry `{entry}` failed root startup validation: {message}")]
    InvalidRootStartup { entry: String, message: String },
}

struct StructuredRootEvaluator<'a> {
    plan: &'a RuntimePlan,
    scratch: VmPureFunctionScratch,
}

impl<'a> StructuredRootEvaluator<'a> {
    fn new(plan: &'a RuntimePlan) -> Self {
        Self {
            plan,
            scratch: VmPureFunctionScratch::default(),
        }
    }
}

impl RootCallableEvaluator for StructuredRootEvaluator<'_> {
    fn evaluate_root_callable(
        &mut self,
        callable: &RuntimeCallableRole,
        args: &[RuntimeValue],
    ) -> Result<RuntimeValue, RootCallableEvaluationError> {
        let executable = self
            .plan
            .callable_executables
            .iter()
            .find(|executable| {
                executable.callable == callable.callable && executable.contract == callable.contract
            })
            .ok_or_else(|| {
                RootCallableEvaluationError::new(format!(
                    "missing executable callable `{}`",
                    callable.callable.as_str()
                ))
            })?;
        let RuntimeCallableExecutableCode::PureHelper(helper) = executable.code else {
            return Err(RootCallableEvaluationError::new(format!(
                "callable `{}` is not a pure root callable",
                callable.callable.as_str()
            )));
        };
        let helper = self
            .plan
            .pure_helpers
            .iter()
            .find(|candidate| candidate.id == helper)
            .ok_or_else(|| {
                RootCallableEvaluationError::new(format!(
                    "callable `{}` maps to a missing pure helper",
                    callable.callable.as_str()
                ))
            })?;
        self.scratch
            .evaluate_values(helper, args)
            .map_err(|error| RootCallableEvaluationError::new(error.to_string()))
    }
}

/// High-level flow status for the minimal runtime spine.
#[derive(Clone, Debug, PartialEq)]
pub enum FlowFiberStatus {
    Running,
    Dialogue(DialogueState),
    Waiting(AwaitState),
    NeedWaiting(NeedId),
    WaitingMany(Box<AwaitManyState>),
    HostCall(HostCallState),
    Choice(ChoiceState),
    Done(FlowExit),
    Failed(String),
}

/// Suspended dialogue line awaiting explicit host progression.
#[derive(Clone, Debug, PartialEq)]
pub struct DialogueState {
    pub line: crate::plan::RuntimeLineId,
    pub task_group: usize,
    pub resume: Option<FlowCursor>,
    pub started_nodes: std::collections::BTreeSet<usize>,
    /// Logical time accumulated while this line has been active.
    pub elapsed: crate::time::LogicalDuration,
}

/// String presentation style for high-level runtime flow status.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlowStatusLabelStyle {
    /// Stable runtime-facing status used by host/bundle/player observations.
    Runtime,
    /// Existing CLI/debug spelling that preserves `FlowExit`'s `Debug` form.
    Debug,
    /// Coarse status used when only the status kind should be exposed.
    Compact,
}

/// Suspended `await ... with` state.
#[derive(Clone, Debug, PartialEq)]
pub struct AwaitState {
    pub binding: Option<RuntimePattern>,
    pub target: AwaitTarget,
    pub resume: Option<FlowCursor>,
}

/// Suspended bounded fanout await state.
#[derive(Clone, Debug, PartialEq)]
pub struct AwaitManyState {
    pub binding: Option<RuntimePattern>,
    pub target: AwaitManyTarget,
    pub resume: Option<FlowCursor>,
    pub items: Vec<RuntimeValue>,
    pub next_index: usize,
    pub in_flight: Vec<AwaitManyInFlight>,
    pub results: Vec<Option<RuntimePayload>>,
}

/// One in-flight child task inside a bounded fanout await.
#[derive(Clone, Debug, PartialEq)]
pub struct AwaitManyInFlight {
    pub index: usize,
    pub task: TaskId,
    pub need: NeedId,
}

/// Suspended direct host call awaiting a typed host result.
#[derive(Clone, Debug, PartialEq)]
pub struct HostCallState {
    pub binding: Option<RuntimePattern>,
    pub target: crate::plan::RuntimeHostCallTarget,
    pub id: RuntimeHostCallId,
    pub resume: Option<FlowCursor>,
}

/// Suspended choice state.
#[derive(Clone, Debug, PartialEq)]
pub struct ChoiceState {
    pub id: Option<String>,
    pub options: Vec<ChoiceRuntimeOption>,
    pub resume: Option<FlowCursor>,
}

/// Terminal flow result observed by the minimal runtime.
#[derive(Clone, Debug, PartialEq)]
pub enum FlowExit {
    Done,
    Return(String),
}

impl FlowFiberStatus {
    pub fn status_label(&self, style: FlowStatusLabelStyle) -> String {
        match style {
            FlowStatusLabelStyle::Runtime => self.runtime_status_label(),
            FlowStatusLabelStyle::Debug => self.debug_status_label(),
            FlowStatusLabelStyle::Compact => self.compact_status_label(),
        }
    }

    fn runtime_status_label(&self) -> String {
        match self {
            Self::Running => "running".to_owned(),
            Self::Dialogue(state) => format!("dialogue {}", state.line),
            Self::Waiting(state) => format!("waiting {}", state.target.task.0),
            Self::NeedWaiting(need) => format!("need_waiting {}", need.0),
            Self::WaitingMany(state) => format!(
                "waiting_many {} {}/{}",
                state.target.task.0,
                state.results.iter().filter(|value| value.is_some()).count(),
                state.results.len()
            ),
            Self::HostCall(state) => format!("host_call {}", state.id.0),
            Self::Choice(state) => {
                format!("choice {}", state.id.as_deref().unwrap_or("-"))
            }
            Self::Done(exit) => exit.runtime_status_label(),
            Self::Failed(message) => format!("failed {message}"),
        }
    }

    fn debug_status_label(&self) -> String {
        match self {
            Self::Running => "running".to_owned(),
            Self::Dialogue(state) => format!("dialogue {}", state.line),
            Self::Waiting(state) => format!("waiting {}", state.target.task.0),
            Self::NeedWaiting(need) => format!("need_waiting {}", need.0),
            Self::WaitingMany(state) => format!(
                "waiting_many {} {}/{}",
                state.target.task.0,
                state.results.iter().filter(|value| value.is_some()).count(),
                state.results.len()
            ),
            Self::HostCall(state) => format!("host_call {}", state.id.0),
            Self::Choice(state) => {
                format!("choice {}", state.id.as_deref().unwrap_or("-"))
            }
            Self::Done(exit) => format!("done {exit:?}"),
            Self::Failed(message) => format!("failed {message}"),
        }
    }

    fn compact_status_label(&self) -> String {
        match self {
            Self::Running => "running".to_owned(),
            Self::Dialogue(_) => "dialogue".to_owned(),
            Self::Waiting(_) => "waiting".to_owned(),
            Self::NeedWaiting(_) => "need_waiting".to_owned(),
            Self::WaitingMany(_) => "waiting_many".to_owned(),
            Self::HostCall(_) => "host_call".to_owned(),
            Self::Choice(_) => "choice".to_owned(),
            Self::Done(exit) => exit.compact_status_label(),
            Self::Failed(message) => format!("failed:{message}"),
        }
    }
}

impl FlowExit {
    fn runtime_status_label(&self) -> String {
        match self {
            Self::Done => "done".to_owned(),
            Self::Return(value) => format!("done return {value}"),
        }
    }

    fn compact_status_label(&self) -> String {
        match self {
            Self::Done => "done".to_owned(),
            Self::Return(value) => format!("done return {value}"),
        }
    }
}

impl Default for FlowFiber {
    fn default() -> Self {
        Self {
            line_cursor: 0,
            cursor: None,
            pending_ops: VecDeque::new(),
            control_stack: Vec::new(),
            root_cleanups: Vec::new(),
            env: RuntimeEnv::default(),
            observations: RuntimeObservationState::default(),
            source_states: BTreeMap::new(),
            stream_states: BTreeMap::new(),
            status: FlowFiberStatus::Done(FlowExit::Done),
        }
    }
}

fn pure_helper_call_shapes(plan: &RuntimePlan) -> PureHelperCallShapes {
    PureHelperCallShapes {
        i32: plan
            .pure_helpers
            .iter()
            .map(eval::pure_helper_has_i32_call_shape)
            .collect(),
        u32: plan
            .pure_helpers
            .iter()
            .map(eval::pure_helper_has_u32_call_shape)
            .collect(),
        i64: plan
            .pure_helpers
            .iter()
            .map(eval::pure_helper_has_i64_call_shape)
            .collect(),
        f32: plan
            .pure_helpers
            .iter()
            .map(eval::pure_helper_has_f32_call_shape)
            .collect(),
        f64: plan
            .pure_helpers
            .iter()
            .map(eval::pure_helper_has_f64_call_shape)
            .collect(),
    }
}

impl Engine {
    /// Creates an engine without implicitly selecting a flow.
    ///
    /// Flow-bearing plans remain dormant until [`Self::start_flow`] or
    /// [`Self::start_entry`] is called. Plans that contain only line tasks,
    /// sources, or streams remain directly executable.
    pub fn new(plan: RuntimePlan) -> Self {
        let flow_positions: BTreeMap<_, _> = plan
            .flows
            .iter()
            .enumerate()
            .map(|(index, flow)| (flow.id.clone(), index))
            .collect();
        let main_started = plan.flows.is_empty();
        let status = if plan.is_empty() {
            FlowFiberStatus::Done(FlowExit::Done)
        } else {
            FlowFiberStatus::Running
        };
        let source_states = plan
            .source_plans
            .iter()
            .map(|plan| {
                (
                    plan.id.clone(),
                    SourceRuntimeState::new(plan.id.clone(), plan.policy.clone()),
                )
            })
            .collect();
        let stream_states = plan
            .stream_plans
            .iter()
            .map(|plan| (plan.id.clone(), StreamRuntimeState::new(plan.id.clone())))
            .collect();
        let call_shapes = pure_helper_call_shapes(&plan);
        Self {
            plan,
            flow_positions,
            main_started,
            root: None,
            root_flow_binding: None,
            fiber: FlowFiber {
                line_cursor: 0,
                cursor: None,
                pending_ops: VecDeque::new(),
                control_stack: Vec::new(),
                root_cleanups: Vec::new(),
                env: RuntimeEnv::default(),
                observations: RuntimeObservationState::default(),
                source_states,
                stream_states,
                status,
            },
            child_fibers: VecDeque::new(),
            run_child_next: false,
            pure_i8_batch_inputs: Vec::new(),
            pure_i8_batch_outputs: Vec::new(),
            pure_i16_batch_inputs: Vec::new(),
            pure_i16_batch_outputs: Vec::new(),
            pure_i32_batch_inputs: Vec::new(),
            pure_i128_batch_inputs: Vec::new(),
            pure_i128_batch_outputs: Vec::new(),
            pure_i32_batch_outputs: Vec::new(),
            pure_u8_batch_inputs: Vec::new(),
            pure_u8_batch_outputs: Vec::new(),
            pure_u16_batch_inputs: Vec::new(),
            pure_u16_batch_outputs: Vec::new(),
            pure_u32_batch_inputs: Vec::new(),
            pure_u32_batch_outputs: Vec::new(),
            pure_u64_batch_inputs: Vec::new(),
            pure_u64_batch_outputs: Vec::new(),
            pure_u128_batch_inputs: Vec::new(),
            pure_u128_batch_outputs: Vec::new(),
            pure_isize_batch_inputs: Vec::new(),
            pure_isize_batch_outputs: Vec::new(),
            pure_usize_batch_inputs: Vec::new(),
            pure_usize_batch_outputs: Vec::new(),
            pure_f32_batch_inputs: Vec::new(),
            pure_f32_batch_outputs: Vec::new(),
            pure_f64_batch_inputs: Vec::new(),
            pure_f64_batch_outputs: Vec::new(),
            pure_i64_batch_inputs: Vec::new(),
            pure_i64_batch_outputs: Vec::new(),
            pure_helper_i32_call_shapes: call_shapes.i32,
            pure_helper_u32_call_shapes: call_shapes.u32,
            pure_helper_i64_call_shapes: call_shapes.i64,
            pure_helper_f32_call_shapes: call_shapes.f32,
            pure_helper_f64_call_shapes: call_shapes.f64,
            audio_epoch: 0,
            next_audio_sequence: 0,
            next_host_call_sequence: 0,
        }
    }

    /// Creates an engine and selects the requested flow exactly.
    pub fn for_flow(plan: RuntimePlan, flow: &FlowRuntimeId) -> Result<Self, EngineStartError> {
        let mut engine = Self::new(plan);
        engine.start_flow(flow)?;
        Ok(engine)
    }

    /// Creates an engine and selects the requested entry exactly.
    pub fn for_entry(plan: RuntimePlan, entry: &EntryRuntimeId) -> Result<Self, EngineStartError> {
        let mut engine = Self::new(plan);
        engine.start_entry(entry)?;
        Ok(engine)
    }

    /// Selects one flow before the first flow execution step.
    pub fn start_flow(&mut self, flow: &FlowRuntimeId) -> Result<(), EngineStartError> {
        if self.main_started {
            return Err(EngineStartError::AlreadyStarted);
        }
        let flow_index = self
            .flow_index(flow)
            .ok_or_else(|| EngineStartError::MissingFlow {
                flow: flow.canonical_label(),
            })?;
        self.fiber.cursor = Some(FlowCursor {
            flow_index,
            op_index: 0,
        });
        self.fiber.status = FlowFiberStatus::Running;
        self.main_started = true;
        Ok(())
    }

    /// Selects the single flow named by an exact entry identity.
    pub fn start_entry(&mut self, entry: &EntryRuntimeId) -> Result<(), EngineStartError> {
        if self.main_started {
            return Err(EngineStartError::AlreadyStarted);
        }
        let target = self
            .plan
            .entries
            .iter()
            .find(|candidate| candidate.id == *entry)
            .map(|candidate| candidate.target.clone())
            .ok_or_else(|| EngineStartError::MissingEntry {
                entry: entry.canonical_label(),
            })?;
        let flow = match target {
            RuntimeEntryTarget::Flow(flow) | RuntimeEntryTarget::Controller(flow) => flow,
            RuntimeEntryTarget::Routes(_) => {
                return Err(EngineStartError::EntryDoesNotSelectFlow {
                    entry: entry.canonical_label(),
                });
            }
        };
        if matches!(
            self.plan
                .entries
                .iter()
                .find(|candidate| candidate.id == *entry)
                .map(|candidate| &candidate.roles),
            Some(crate::entry::RuntimeEntryRoles::Stateful(_))
        ) {
            let contract =
                RootStartupContract::from_runtime_plan(&self.plan, entry).map_err(|error| {
                    EngineStartError::InvalidRootStartup {
                        entry: entry.canonical_label(),
                        message: error.to_string(),
                    }
                })?;
            let mut evaluator = StructuredRootEvaluator::new(&self.plan);
            let startup = RootRuntime::start(contract, &mut evaluator).map_err(|error| {
                EngineStartError::InvalidRootStartup {
                    entry: entry.canonical_label(),
                    message: error.to_string(),
                }
            })?;
            let flow_index = self.flow_index(&startup.initial_flow).ok_or_else(|| {
                EngineStartError::MissingFlow {
                    flow: startup.initial_flow.canonical_label(),
                }
            })?;
            self.fiber.cursor = Some(FlowCursor {
                flow_index,
                op_index: 0,
            });
            self.fiber.status = FlowFiberStatus::Running;
            self.fiber.env.set_root(
                startup.initial_state_binding.name.clone(),
                startup.initial_state_binding.value.clone(),
            );
            self.root_flow_binding = Some(startup.initial_state_binding);
            self.root = Some(startup.root);
            self.main_started = true;
            return Ok(());
        }
        self.start_flow(&flow)
    }

    #[must_use]
    pub const fn root(&self) -> Option<&RootRuntime> {
        self.root.as_ref()
    }

    pub fn acknowledge_root_commands(
        &mut self,
        accepted: &[RuntimeCommandEnvelope],
    ) -> Result<(), RootRuntimeError> {
        match self.root.as_mut() {
            Some(root) => root.acknowledge_published_commands(accepted),
            None if accepted.is_empty() => Ok(()),
            None => Err(RootRuntimeError::CommandAcknowledgementMismatch),
        }
    }

    pub const fn fiber(&self) -> &FlowFiber {
        &self.fiber
    }

    pub(super) fn flow_at_cursor(&self, cursor: &FlowCursor) -> Option<&RuntimeFlow> {
        self.plan.flows.get(cursor.flow_index)
    }

    pub(super) fn flow_index(&self, flow: &FlowRuntimeId) -> Option<usize> {
        self.flow_positions.get(flow).copied()
    }

    pub fn child_fiber_count(&self) -> usize {
        self.child_fibers.len()
    }

    pub fn step(
        &mut self,
        input: RuntimeStepInput,
        options: RuntimeStepOptions,
    ) -> RuntimeStepResult {
        let mut pure_backend = VmRuntimePureCallBackend::default();
        self.step_with_pure_backend(input, options, &mut pure_backend)
    }

    pub fn step_with_pure_backend(
        &mut self,
        input: RuntimeStepInput,
        options: RuntimeStepOptions,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> RuntimeStepResult {
        self.step_with_root_bindings_and_pure_backend(input, &[], options, pure_backend)
    }

    pub fn step_with_root_bindings_and_pure_backend(
        &mut self,
        mut input: RuntimeStepInput,
        root_bindings: &[RuntimeBinding],
        options: RuntimeStepOptions,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> RuntimeStepResult {
        let mut output = RuntimeStepOutput::default();
        let mut executed_ops = 0;
        let pure_stats_before = pure_backend.stats();
        let pending_ops_before = self.pending_ops_len();
        let root_events_in = input.root_events.len();
        let deferred_root_events = std::mem::take(&mut input.deferred_root_events);
        let need_states_in = input.need_states.len();
        let protected_root_flow_binding = self.root_flow_binding.as_ref().and_then(|binding| {
            self.fiber
                .env
                .get_cloned(&binding.name)
                .map(|value| RuntimeBinding {
                    name: binding.name.clone(),
                    value,
                })
        });
        self.fiber.env.bind_all_root_ref(root_bindings);
        self.fiber
            .env
            .bind_all_root(std::mem::take(&mut input.bindings));
        if let Some(binding) = protected_root_flow_binding {
            self.fiber.env.set_root(binding.name, binding.value);
        }
        if !self.run_root_phase(std::mem::take(&mut input.root_events), &mut output) {
            let stats = RuntimeStepStats {
                executed_ops,
                pending_ops_before,
                pending_ops_after: self.pending_ops_len(),
                child_fibers: self.child_fibers.len(),
                pure: pure_backend.stats().saturating_delta(pure_stats_before),
                root_events_in,
                root_transitions: output.root_transitions.len(),
                root_commands: output.root_commands.len(),
                diagnostics: output.diagnostics.len(),
                ..RuntimeStepStats::default()
            };
            return self.step_result(output, options, stats);
        }
        output
            .requests
            .root_events_next_step
            .extend(deferred_root_events);
        let events = normalize_task_events(std::mem::take(&mut input.task_events));
        let source_events = normalize_source_events(std::mem::take(&mut input.source_events));
        let task_events_in = events.len();
        let source_events_in = source_events.len();
        output.diagnostics.extend(events.iter().map(|event| {
            RuntimeDiagnostic::new(format!(
                "task {} sequence {} delivered",
                event.task_id.0, event.sequence.0
            ))
        }));
        self.apply_source_events(source_events, &mut output, pure_backend);
        self.step_stream_plans(&mut output, pure_backend);

        while executed_ops < options.budget.max_ops && self.can_attempt_runtime_op() {
            self.step_runtime_op(&input, &events, &mut output, pure_backend);
            executed_ops += 1;
            if self.should_return_to_host(options.mode, &output, executed_ops) {
                break;
            }
        }
        self.record_observations(&output.effects.line);
        let stats = RuntimeStepStats {
            executed_ops,
            pending_ops_before,
            pending_ops_after: self.pending_ops_len(),
            child_fibers: self.child_fibers.len(),
            pure: pure_backend.stats().saturating_delta(pure_stats_before),
            task_events_in,
            need_states_in,
            source_events_in,
            root_events_in,
            root_transitions: output.root_transitions.len(),
            root_commands: output.root_commands.len(),
            root_events_deferred: output.requests.root_events_next_step.len(),
            source_events_emitted: output.effects.source_events.len(),
            stream_events_emitted: output.effects.stream_events.len(),
            line_effects: output.effects.line.len(),
            audio_commands: output.requests.audio.len(),
            diagnostics: output.diagnostics.len(),
        };
        self.step_result(output, options, stats)
    }

    fn run_root_phase(
        &mut self,
        events: Vec<RootEventInput>,
        output: &mut RuntimeStepOutput,
    ) -> bool {
        let Some(root) = self.root.as_mut() else {
            if events.is_empty() {
                return true;
            }
            let message = "non-stateful runtime entry cannot accept root events".to_owned();
            output.diagnostics.push(RuntimeDiagnostic::categorized(
                RuntimeDiagnosticCategory::Input,
                message,
            ));
            return false;
        };
        let result = {
            let mut evaluator = StructuredRootEvaluator::new(&self.plan);
            root.step(events, &mut evaluator)
        };
        match result {
            Ok(result) => {
                let failed = result.failed;
                output.root_transitions.extend(result.outcomes);
                output.root_commands.extend(result.commands);
                if failed {
                    let message = root
                        .failure()
                        .map_or_else(|| "root reducer trapped".to_owned(), ToString::to_string);
                    self.fiber.status = FlowFiberStatus::Failed(message);
                    false
                } else {
                    true
                }
            }
            Err(error) => {
                let step_input_rejection = matches!(
                    &error,
                    RootRuntimeError::InvalidEvent(_)
                        | RootRuntimeError::EventQueueLimit { .. }
                        | RootRuntimeError::TransitionSequenceExhausted
                );
                let category = if step_input_rejection {
                    RuntimeDiagnosticCategory::Input
                } else {
                    RuntimeDiagnosticCategory::Internal
                };
                let message = error.to_string();
                output
                    .diagnostics
                    .push(RuntimeDiagnostic::categorized(category, message.clone()));
                if !step_input_rejection {
                    self.fiber.status = FlowFiberStatus::Failed(message);
                }
                false
            }
        }
    }

    fn can_attempt_runtime_op(&self) -> bool {
        self.main_fiber_can_attempt_runtime_op() || self.has_active_child_fibers()
    }

    fn step_runtime_op(
        &mut self,
        input: &RuntimeStepInput,
        events: &[TaskEvent],
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        if self.run_child_next && self.step_next_child_fiber(input, events, output, pure_backend) {
            self.run_child_next = false;
            return;
        }
        self.run_child_next = true;
        if !self.main_fiber_can_attempt_runtime_op() {
            self.step_next_child_fiber(input, events, output, pure_backend);
            return;
        }
        if self.resume_suspended(input, events, output, pure_backend) {
            return;
        }
        if !matches!(self.fiber.status, FlowFiberStatus::Running) {
            return;
        }
        if self.fiber.cursor.is_some() {
            self.step_flow(input, output, pure_backend);
        } else {
            self.step_line_only(input, output, pure_backend);
        }
    }

    fn main_fiber_can_attempt_runtime_op(&self) -> bool {
        self.main_started
            && !matches!(
                self.fiber.status,
                FlowFiberStatus::Done(_) | FlowFiberStatus::Failed(_)
            )
    }

    pub(super) fn has_active_child_fibers(&self) -> bool {
        !self.child_fibers.is_empty()
    }

    fn pending_ops_len(&self) -> usize {
        self.fiber.pending_ops.len()
            + self
                .child_fibers
                .iter()
                .map(|fiber| fiber.pending_ops.len())
                .sum::<usize>()
    }

    pub(super) fn spawn_child_fiber(&mut self, body: Vec<FlowOp>) {
        let mut pending_ops = VecDeque::with_capacity(body.len().saturating_add(2));
        if !body.is_empty() {
            pending_ops.push_front(FlowOp::ExitScope);
            for op in body.into_iter().rev() {
                pending_ops.push_front(op);
            }
            pending_ops.push_front(FlowOp::EnterScope);
        }
        self.child_fibers.push_back(FlowFiber {
            line_cursor: 0,
            cursor: None,
            pending_ops,
            control_stack: Vec::new(),
            root_cleanups: Vec::new(),
            env: self.fiber.env.clone(),
            observations: RuntimeObservationState::default(),
            source_states: BTreeMap::new(),
            stream_states: BTreeMap::new(),
            status: FlowFiberStatus::Running,
        });
        self.run_child_next = true;
    }

    fn step_next_child_fiber(
        &mut self,
        input: &RuntimeStepInput,
        events: &[TaskEvent],
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> bool {
        let Some(mut child) = self.child_fibers.pop_front() else {
            return false;
        };
        std::mem::swap(&mut self.fiber, &mut child);
        self.step_active_child_fiber(input, events, output, pure_backend);
        self.finish_active_child_if_exhausted();
        std::mem::swap(&mut self.fiber, &mut child);
        match child.status {
            FlowFiberStatus::Done(_) => {}
            FlowFiberStatus::Failed(message) => {
                self.fiber.status = FlowFiberStatus::Failed(message);
            }
            _ => self.child_fibers.push_back(child),
        }
        true
    }

    fn step_active_child_fiber(
        &mut self,
        input: &RuntimeStepInput,
        events: &[TaskEvent],
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        if self.resume_suspended(input, events, output, pure_backend) {
            return;
        }
        if !matches!(self.fiber.status, FlowFiberStatus::Running) {
            return;
        }
        if self.fiber.cursor.is_some() || !self.fiber.pending_ops.is_empty() {
            self.step_flow(input, output, pure_backend);
        }
    }

    fn finish_active_child_if_exhausted(&mut self) {
        if matches!(self.fiber.status, FlowFiberStatus::Running)
            && self.fiber.cursor.is_none()
            && self.fiber.pending_ops.is_empty()
        {
            self.fiber.status = FlowFiberStatus::Done(FlowExit::Done);
        }
    }

    fn should_return_to_host(
        &self,
        mode: RuntimeStepMode,
        output: &RuntimeStepOutput,
        executed_ops: usize,
    ) -> bool {
        if self.hard_stop_reason(output).is_some() {
            if matches!(mode, RuntimeStepMode::Drain | RuntimeStepMode::Server)
                && has_host_requests(output)
                && (self.main_fiber_can_attempt_runtime_op() || self.has_runnable_child_fibers())
            {
                return false;
            }
            return true;
        }
        match mode {
            RuntimeStepMode::OneOp => executed_ops > 0,
            RuntimeStepMode::Game => has_presentation_visible_output(output),
            RuntimeStepMode::Drain | RuntimeStepMode::Server => false,
        }
    }

    fn record_observations(&mut self, effects: &[LineEffectRequest]) {
        for effect in effects {
            self.fiber.observations.record_effect(effect);
        }
    }

    fn diagnose_runtime_error(error: impl std::fmt::Display, output: &mut RuntimeStepOutput) {
        output
            .diagnostics
            .push(RuntimeDiagnostic::new(error.to_string()));
    }

    fn step_result(
        &self,
        output: RuntimeStepOutput,
        options: RuntimeStepOptions,
        stats: RuntimeStepStats,
    ) -> RuntimeStepResult {
        let stop_reason = self
            .hard_stop_reason(&output)
            .unwrap_or_else(|| Self::running_stop_reason(options, stats.executed_ops, &output));
        RuntimeStepResult {
            output,
            fiber_status: self.effective_fiber_status(),
            stop_reason,
            stats,
        }
    }

    fn effective_fiber_status(&self) -> FlowFiberStatus {
        if self.has_active_child_fibers()
            && matches!(
                self.fiber.status,
                FlowFiberStatus::Done(_)
                    | FlowFiberStatus::Waiting(_)
                    | FlowFiberStatus::NeedWaiting(_)
                    | FlowFiberStatus::WaitingMany(_)
                    | FlowFiberStatus::HostCall(_)
                    | FlowFiberStatus::Dialogue(_)
                    | FlowFiberStatus::Choice(_)
            )
        {
            FlowFiberStatus::Running
        } else {
            self.fiber.status.clone()
        }
    }

    fn hard_stop_reason(&self, output: &RuntimeStepOutput) -> Option<RuntimeStepStopReason> {
        if self.has_active_child_fibers()
            && matches!(
                self.fiber.status,
                FlowFiberStatus::Done(_)
                    | FlowFiberStatus::Waiting(_)
                    | FlowFiberStatus::NeedWaiting(_)
                    | FlowFiberStatus::WaitingMany(_)
                    | FlowFiberStatus::HostCall(_)
                    | FlowFiberStatus::Dialogue(_)
                    | FlowFiberStatus::Choice(_)
            )
        {
            return None;
        }
        match self.fiber.status {
            FlowFiberStatus::Done(_) => Some(RuntimeStepStopReason::Done),
            FlowFiberStatus::Failed(_) => Some(RuntimeStepStopReason::Failed),
            FlowFiberStatus::HostCall(_) => Some(if has_host_requests(output) {
                RuntimeStepStopReason::Output
            } else {
                RuntimeStepStopReason::Blocked
            }),
            FlowFiberStatus::Dialogue(_)
            | FlowFiberStatus::Waiting(_)
            | FlowFiberStatus::WaitingMany(_)
            | FlowFiberStatus::Choice(_) => Some(if has_presentation_visible_output(output) {
                RuntimeStepStopReason::Output
            } else {
                RuntimeStepStopReason::Blocked
            }),
            FlowFiberStatus::NeedWaiting(_) => Some(RuntimeStepStopReason::Blocked),
            FlowFiberStatus::Running if has_host_requests(output) => {
                Some(RuntimeStepStopReason::Output)
            }
            FlowFiberStatus::Running => None,
        }
    }

    fn running_stop_reason(
        options: RuntimeStepOptions,
        executed_ops: usize,
        output: &RuntimeStepOutput,
    ) -> RuntimeStepStopReason {
        if options.mode == RuntimeStepMode::Game && has_presentation_visible_output(output) {
            return RuntimeStepStopReason::Output;
        }
        if options.mode == RuntimeStepMode::OneOp && executed_ops > 0 {
            return RuntimeStepStopReason::OneOp;
        }
        if executed_ops >= options.budget.max_ops {
            return RuntimeStepStopReason::BudgetExhausted;
        }
        RuntimeStepStopReason::OneOp
    }

    fn has_runnable_child_fibers(&self) -> bool {
        self.child_fibers.iter().any(|child| {
            matches!(child.status, FlowFiberStatus::Running)
                && (child.cursor.is_some() || !child.pending_ops.is_empty())
        })
    }
}

fn has_host_requests(output: &RuntimeStepOutput) -> bool {
    !output.requests.tasks.is_empty()
        || !output.requests.audio.is_empty()
        || !output.requests.cancel_scopes.is_empty()
        || !output.requests.source_close.is_empty()
        || !output.requests.ensure_content.is_empty()
        || !output.requests.host_calls.is_empty()
}

fn has_presentation_visible_output(output: &RuntimeStepOutput) -> bool {
    output.flow_events.iter().any(flow_event_is_visible)
        || output.effects.line.iter().any(line_effect_is_visible)
}

fn flow_event_is_visible(event: &FlowEvent) -> bool {
    matches!(
        event,
        FlowEvent::DialogueLine { .. }
            | FlowEvent::LineCancelled { .. }
            | FlowEvent::ChoicePresented { .. }
            | FlowEvent::ChoiceSelected { .. }
            | FlowEvent::AwaitStarted { .. }
            | FlowEvent::AwaitProgress { .. }
    )
}

fn line_effect_is_visible(effect: &LineEffectRequest) -> bool {
    !matches!(
        effect,
        LineEffectRequest::Log(_)
            | LineEffectRequest::SignalWrite(_)
            | LineEffectRequest::MetricWrite(_)
            | LineEffectRequest::EmitEvent(_)
    )
}
