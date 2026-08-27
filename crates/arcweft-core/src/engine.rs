use crate::effect::LineEffectRequest;
use crate::entry::{RuntimeCallableExecutableCode, RuntimeCallableRole};
use crate::line_task::{ChildCancelPolicy, ChildJoinPolicy, LineTaskGroup, LineTaskWorkTag};
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
use crate::runtime_id::{DialogueActivationId, RuntimePersistentFiberId};
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
    TaskPolicy, TaskPriority, TaskPublicationCursor, TaskSpec, normalize_task_events,
};
use crate::value::{
    RuntimeEnv, RuntimeEvalError, RuntimeExpr, RuntimeExprMatchArm, RuntimeFlowParameterBinding,
    RuntimeIterator, RuntimeLocalBinding, RuntimePayload, RuntimeSeq, RuntimeValue,
    evaluate_binary, evaluate_unary, runtime_sequence_dense_i64,
    runtime_sequence_from_literal_values, runtime_sequence_repeat_value, runtime_sequence_values,
    runtime_value_into_sequence_values, runtime_value_label, sum_i64_sequence_ref,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;
use thiserror::Error;
pub mod aot;
pub mod audio;
pub mod dialogue;
pub mod eval;
pub(crate) use eval::evaluate_runtime_call;
pub mod flow;
pub mod line;
pub mod stream;
pub mod suspend;

#[derive(Clone, Debug, PartialEq)]
pub struct Engine {
    plan: Arc<RuntimePlan>,
    flow_positions: BTreeMap<FlowRuntimeId, usize>,
    main_started: bool,
    root: Option<RootRuntime>,
    fiber: FlowFiber,
    child_fibers: VecDeque<FlowFiber>,
    task_publications: BTreeMap<TaskId, TaskPublicationCursor>,
    next_fiber_id: u64,
    dialogue_occurrences: BTreeMap<
        (
            RuntimePersistentFiberId,
            crate::runtime_id::RuntimeDialogueContentPlanId,
        ),
        u64,
    >,
    dialogue_activations: dialogue::DialogueActivationStore,
    run_child_next: bool,
    pure_i64_batch_inputs: Vec<i64>,
    pure_i64_batch_outputs: Vec<i64>,
    pure_u32_batch_inputs: Vec<u32>,
    pure_helper_u32_call_shapes: Vec<bool>,
    pure_helper_i64_call_shapes: Vec<bool>,
    audio_epoch: u64,
    next_audio_sequence: u64,
    next_host_call_sequence: u64,
}

pub(super) struct NativeLineTaskExecutionBatch {
    child_fibers: VecDeque<FlowFiber>,
    next_fiber_id: u64,
    run_child_next: bool,
}

struct PureHelperCallShapes {
    u32: Vec<bool>,
    i64: Vec<bool>,
}

/// Current flow execution cursor.
#[derive(Clone, Debug, PartialEq)]
pub struct FlowFiber {
    pub line_cursor: usize,
    pub cursor: Option<FlowCursor>,
    pub pending_ops: VecDeque<FlowOp>,
    pub control_stack: Vec<FlowControlStackEntry>,
    pub await_observer: Option<Box<AwaitState>>,
    pub root_cleanups: Vec<FlowScopeCleanup>,
    pub env: RuntimeEnv,
    pub observations: RuntimeObservationState,
    pub stream_states: BTreeMap<StreamRuntimeId, StreamRuntimeState>,
    pub id: FlowFiberId,
    pub persistent_id: RuntimePersistentFiberId,
    pub execution: crate::runtime_id::ExecutionInstanceId,
    pub(crate) owner: FlowFiberOwner,
    pub status: FlowFiberStatus,
}

/// Stable executor-local identity. It is allocated independently from a
/// plan node so an old child completion cannot be attributed to a later run.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct FlowFiberId(u64);

impl FlowFiberId {
    #[must_use]
    pub(crate) const fn from_executor_ordinal(ordinal: u64) -> Self {
        Self(ordinal)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum FlowFiberOwner {
    Executor,
    LineTask(LineTaskFiberOwner),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LineTaskFiberOwner {
    pub(crate) tag: LineTaskWorkTag,
    pub(crate) join_policy: ChildJoinPolicy,
    pub(crate) cancel_policy: ChildCancelPolicy,
    pub(crate) closing: bool,
}

impl FlowFiberOwner {
    fn has_joined_work(&self) -> bool {
        !matches!(
            self,
            Self::LineTask(LineTaskFiberOwner {
                join_policy: ChildJoinPolicy::Detached,
                ..
            })
        )
    }

    fn requests_line_task_close(&self) -> bool {
        matches!(
            self,
            Self::LineTask(LineTaskFiberOwner { closing: true, .. })
        )
    }
}

fn flow_fiber_line_handle_tokens(
    fiber: &FlowFiber,
) -> Result<BTreeSet<crate::runtime_id::RuntimeLineHandleToken>, crate::line_task::LineRuntimeError>
{
    let mut tokens = BTreeSet::new();
    for value in fiber.env.values() {
        for handle in value
            .affine_line_handles()
            .map_err(|_| crate::line_task::LineRuntimeError::InvalidHandlePayload)?
        {
            if !tokens.insert(handle.token().clone()) {
                return Err(crate::line_task::LineRuntimeError::DuplicateHandleOccurrence);
            }
        }
    }
    Ok(tokens)
}

fn flow_fiber_line_handle_owners(
    fiber: &FlowFiber,
) -> Result<
    BTreeMap<
        crate::runtime_id::RuntimeLineHandleToken,
        crate::value::ownership::RuntimeOwnedSlotId,
    >,
    crate::line_task::LineRuntimeError,
> {
    let mut owners = BTreeMap::new();
    for binding in fiber.env.bindings() {
        let owner = crate::value::ownership::RuntimeOwnedSlotId::environment_local(
            fiber.execution,
            binding.local,
        );
        for handle in binding
            .value
            .affine_line_handles()
            .map_err(|_| crate::line_task::LineRuntimeError::InvalidHandlePayload)?
        {
            if owners.insert(handle.token().clone(), owner).is_some() {
                return Err(crate::line_task::LineRuntimeError::DuplicateHandleOccurrence);
            }
        }
    }
    Ok(owners)
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
    #[error("runtime Flow invocation is invalid: {message}")]
    InvalidFlowInvocation { message: String },
    #[error("runtime entry `{entry}` failed root startup validation: {message}")]
    InvalidRootStartup { entry: String, message: String },
}

struct StructuredRootEvaluator<'a> {
    plan: &'a Arc<RuntimePlan>,
    scratch: VmPureFunctionScratch,
}

impl<'a> StructuredRootEvaluator<'a> {
    fn new(plan: &'a Arc<RuntimePlan>) -> Self {
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
        self.scratch
            .evaluate_values(self.plan, helper, args)
            .map_err(|error| RootCallableEvaluationError::new(error.to_string()))
    }
}

/// High-level flow status for the minimal runtime spine.
#[derive(Clone, Debug, PartialEq)]
pub enum FlowFiberStatus {
    Running,
    Dialogue(DialogueActivationId),
    Waiting(Box<AwaitState>),
    NeedWaiting(NeedId),
    WaitingMany(Box<AwaitManyState>),
    HostCall(HostCallState),
    Choice(ChoiceState),
    Done(FlowExit),
    Failed(String),
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
    pub observers: Vec<crate::plan::RuntimeAwaitPendingObserver>,
    pub resume: Option<FlowCursor>,
    pub observed_through: Option<TaskPublicationCursor>,
    pub queued: VecDeque<TaskEvent>,
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
            Self::Dialogue(_) => "dialogue".to_owned(),
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
            Self::Dialogue(_) => "dialogue".to_owned(),
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
            await_observer: None,
            root_cleanups: Vec::new(),
            env: RuntimeEnv::default(),
            observations: RuntimeObservationState::default(),
            stream_states: BTreeMap::new(),
            id: FlowFiberId::default(),
            persistent_id: RuntimePersistentFiberId::default(),
            execution: crate::runtime_id::ExecutionInstanceId::from_allocated(
                std::num::NonZeroU64::MIN,
            ),
            owner: FlowFiberOwner::Executor,
            status: FlowFiberStatus::Done(FlowExit::Done),
        }
    }
}

fn pure_helper_call_shapes(plan: &RuntimePlan) -> PureHelperCallShapes {
    PureHelperCallShapes {
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
    }
}

impl Engine {
    pub(super) fn allocate_dialogue_activation(
        &mut self,
        content: crate::runtime_id::RuntimeDialogueContentPlanId,
    ) -> Result<DialogueActivationId, crate::line_task::LineRuntimeError> {
        let artifact = self
            .plan
            .artifact()
            .ok_or(crate::line_task::LineRuntimeError::UnboundArtifact)?;
        let key = (self.fiber.persistent_id, content);
        let occurrence = self.dialogue_occurrences.get(&key).copied().unwrap_or(0);
        let next = occurrence
            .checked_add(1)
            .ok_or(crate::line_task::LineRuntimeError::DialogueOccurrenceOverflow)?;
        let activation =
            DialogueActivationId::new(artifact, self.fiber.persistent_id, content, occurrence);
        self.dialogue_occurrences.insert(key, next);
        Ok(activation)
    }

    /// Creates an engine without implicitly selecting a flow.
    ///
    /// Flow-bearing plans remain dormant until [`Self::start_flow`] or
    /// [`Self::start_entry`] is called. Plans that contain only line tasks,
    /// streams remain directly executable.
    pub fn new(plan: RuntimePlan) -> Self {
        let plan = Arc::new(plan);
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
        let stream_states = plan
            .stream_plans
            .iter()
            .map(|plan| {
                (
                    plan.id().clone(),
                    StreamRuntimeState::new(plan.id().clone()),
                )
            })
            .collect();
        let call_shapes = pure_helper_call_shapes(&plan);
        Self {
            plan,
            flow_positions,
            main_started,
            root: None,
            fiber: FlowFiber {
                line_cursor: 0,
                cursor: None,
                pending_ops: VecDeque::new(),
                control_stack: Vec::new(),
                await_observer: None,
                root_cleanups: Vec::new(),
                env: RuntimeEnv::default(),
                observations: RuntimeObservationState::default(),
                stream_states,
                id: FlowFiberId::default(),
                persistent_id: RuntimePersistentFiberId::from_allocated(1),
                execution: crate::runtime_id::ExecutionInstanceId::from_allocated(
                    std::num::NonZeroU64::MIN,
                ),
                owner: FlowFiberOwner::Executor,
                status,
            },
            child_fibers: VecDeque::new(),
            task_publications: BTreeMap::new(),
            next_fiber_id: 1,
            dialogue_occurrences: BTreeMap::new(),
            dialogue_activations: dialogue::DialogueActivationStore::default(),
            run_child_next: false,
            pure_i64_batch_inputs: Vec::new(),
            pure_i64_batch_outputs: Vec::new(),
            pure_u32_batch_inputs: Vec::new(),
            pure_helper_u32_call_shapes: call_shapes.u32,
            pure_helper_i64_call_shapes: call_shapes.i64,
            audio_epoch: 0,
            next_audio_sequence: 0,
            next_host_call_sequence: 0,
        }
    }

    /// Creates an engine and selects the requested flow exactly.
    pub fn for_flow(plan: RuntimePlan, flow: &FlowRuntimeId) -> Result<Self, EngineStartError> {
        let invocation = plan
            .seal_flow_invocation(flow.clone(), [])
            .map_err(|error| EngineStartError::InvalidFlowInvocation {
                message: error.to_string(),
            })?;
        Self::for_flow_invocation(invocation)
    }

    /// Creates an engine from one complete plan-owned Flow invocation.
    pub fn for_flow_invocation(
        invocation: crate::plan::RuntimeFlowInvocation,
    ) -> Result<Self, EngineStartError> {
        let (plan, flow, bindings) = invocation.into_parts();
        let mut engine = Self::new(plan);
        engine.start_flow_cursor(&flow)?;
        let admitted = engine
            .admit_current_flow_parameter_bindings(bindings.iter())
            .map_err(|error| EngineStartError::InvalidFlowInvocation {
                message: error.to_string(),
            })?
            .into_iter()
            .map(|(_, binding)| binding);
        engine.fiber.env.bind_all_root(admitted);
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
        let schema = self
            .plan
            .flow_schemas
            .iter()
            .find(|candidate| candidate.flow == *flow)
            .ok_or_else(|| EngineStartError::InvalidFlowInvocation {
                message: format!("Flow `{flow}` has no invocation schema"),
            })?;
        if !schema.parameters.is_empty() {
            return Err(EngineStartError::InvalidFlowInvocation {
                message: format!(
                    "Flow `{flow}` requires an explicit coordinate-addressed invocation"
                ),
            });
        }
        self.start_flow_cursor(flow)
    }

    fn start_flow_cursor(&mut self, flow: &FlowRuntimeId) -> Result<(), EngineStartError> {
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
            let initial_state_binding = self
                .admit_current_flow_parameter_bindings(std::iter::once(
                    &startup.initial_state_binding,
                ))
                .and_then(|mut bindings| {
                    bindings.pop().map(|(_, binding)| binding).ok_or_else(|| {
                        RuntimeEvalError::UnknownFlowBinding {
                            flow: startup.initial_flow.canonical_label(),
                            binding: format!(
                                "#{}",
                                startup.initial_state_binding.parameter.position()
                            ),
                        }
                    })
                })
                .map_err(|error| EngineStartError::InvalidRootStartup {
                    entry: entry.canonical_label(),
                    message: error.to_string(),
                })?;
            self.fiber.env.set_root(
                initial_state_binding.local,
                initial_state_binding.value.clone(),
            );
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

    fn admit_current_flow_parameter_bindings<'a>(
        &self,
        bindings: impl IntoIterator<Item = &'a RuntimeFlowParameterBinding>,
    ) -> Result<Vec<(crate::entry::FlowParameterCoordinate, RuntimeLocalBinding)>, RuntimeEvalError>
    {
        let mut bindings = bindings.into_iter().peekable();
        let Some(first) = bindings.peek() else {
            return Ok(Vec::new());
        };
        let binding_label = format!("#{}", first.parameter.position());
        let Some(cursor) = self.fiber.cursor.as_ref() else {
            return Err(RuntimeEvalError::MissingFlowBindingTarget {
                flow: "<none>".to_owned(),
                binding: binding_label,
            });
        };
        let Some(flow) = self.plan.flows.get(cursor.flow_index) else {
            return Err(RuntimeEvalError::MissingFlowBindingTarget {
                flow: format!("#{}", cursor.flow_index),
                binding: binding_label,
            });
        };
        let flow_label = flow.id.canonical_label();
        let Some(schema) = self
            .plan
            .flow_schemas
            .iter()
            .find(|candidate| candidate.flow == flow.id)
        else {
            return Err(RuntimeEvalError::MissingFlowBindingTarget {
                flow: flow_label,
                binding: binding_label,
            });
        };

        let mut admitted = Vec::new();
        let mut unique = BTreeSet::new();
        for binding in bindings {
            if !unique.insert(binding.parameter) {
                return Err(RuntimeEvalError::DuplicateFlowParameterBinding {
                    flow: flow_label,
                    parameter: binding.parameter,
                });
            }
            let position = binding.parameter.index().map_err(|_| {
                RuntimeEvalError::InvalidFlowParameterCoordinate {
                    flow: flow_label.clone(),
                    parameter: binding.parameter,
                }
            })?;
            let Some(parameter) = schema
                .parameters
                .get(position)
                .filter(|parameter| parameter.coordinate == binding.parameter)
            else {
                return Err(RuntimeEvalError::UnknownFlowParameterBinding {
                    flow: flow_label,
                    parameter: binding.parameter,
                });
            };
            let local = flow.params.get(position).copied().ok_or_else(|| {
                RuntimeEvalError::MissingFlowParameterLocal {
                    flow: flow_label.clone(),
                    position,
                }
            })?;
            let declaration = self
                .plan
                .local_declarations
                .get(local)
                .ok_or(RuntimeEvalError::UnknownLocal(local))?;
            if !self
                .plan
                .value_matches_type(declaration.ty(), &binding.value)?
            {
                return Err(RuntimeEvalError::FlowParameterBindingType {
                    flow: flow_label,
                    parameter: parameter.coordinate,
                    local,
                    expected: declaration.ty(),
                });
            }
            admitted.push((
                binding.parameter,
                RuntimeLocalBinding {
                    local,
                    value: binding.value.clone(),
                },
            ));
        }
        Ok(admitted)
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
        mut input: RuntimeStepInput,
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
        output
            .requests
            .root_events_next_step
            .extend(deferred_root_events);
        let dialogue_content_events = std::mem::take(&mut input.dialogue_content_events);
        let dialogue_advances = std::mem::take(&mut input.dialogue_advances);
        let line_outcomes = std::mem::take(&mut input.line_outcomes);
        let dialogue_ingress = match self.dialogue_activations.latch_step_input(
            input.dt,
            &dialogue_content_events,
            &dialogue_advances,
            &line_outcomes,
        ) {
            Ok(receipt) => receipt,
            Err(error) => {
                let activation = error.activation().cloned();
                let source = error.into_source();
                if let Some(activation) = activation.filter(|activation| {
                    matches!(
                        &self.fiber.status,
                        FlowFiberStatus::Dialogue(current) if current == activation
                    )
                }) {
                    match self.begin_dialogue_activation_transaction(&activation) {
                        Ok(transaction) => {
                            self.begin_dialogue_failure(transaction, source.into(), &mut output)
                        }
                        Err(begin_error) => self.fail_eval(begin_error, &mut output),
                    }
                } else {
                    let message = source.to_string();
                    output.diagnostics.push(RuntimeDiagnostic::categorized(
                        RuntimeDiagnosticCategory::Input,
                        message.clone(),
                    ));
                    self.fiber.status = FlowFiberStatus::Failed(message);
                }
                let stats = RuntimeStepStats {
                    executed_ops,
                    pending_ops_before,
                    pending_ops_after: self.pending_ops_len(),
                    child_fibers: self.child_fibers.len(),
                    pure: pure_backend.stats().saturating_delta(pure_stats_before),
                    root_events_in,
                    root_transitions: output.root_transitions.len(),
                    root_commands: output.root_commands.len(),
                    root_events_deferred: output.requests.root_events_next_step.len(),
                    diagnostics: output.diagnostics.len(),
                    ..RuntimeStepStats::default()
                };
                return self.step_result(output, options, stats);
            }
        };
        for diagnostic in dialogue_ingress.into_diagnostics() {
            output.diagnostics.push(RuntimeDiagnostic::categorized(
                RuntimeDiagnosticCategory::Host,
                diagnostic.to_string(),
            ));
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
        let events = normalize_task_events(std::mem::take(&mut input.task_events));
        let task_events_in = events.len();
        output.diagnostics.extend(events.iter().map(|event| {
            RuntimeDiagnostic::new(format!(
                "task {} sequence {} delivered",
                event.task_id.0, event.sequence.0
            ))
        }));
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
            root_events_in,
            root_transitions: output.root_transitions.len(),
            root_commands: output.root_commands.len(),
            root_events_deferred: output.requests.root_events_next_step.len(),
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
        self.main_fiber_can_attempt_runtime_op() || self.has_executor_work()
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
        self.latch_active_await_observer_events(events);
        if self.resume_suspended(input, events, output, pure_backend) {
            return;
        }
        if !matches!(self.fiber.status, FlowFiberStatus::Running) {
            return;
        }
        if self.fiber.cursor.is_some() {
            self.step_main_flow_transaction(output, pure_backend);
        } else {
            self.step_line_only(input, output, pure_backend);
        }
    }

    fn step_main_flow_transaction(
        &mut self,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        let before = match flow_fiber_line_handle_owners(&self.fiber) {
            Ok(owners) => owners,
            Err(error) => {
                self.fail_eval(error, output);
                return;
            }
        };
        let mut candidate = self.clone();
        let mut staged_output = RuntimeStepOutput::default();
        let mut drop_policy = None;
        candidate.step_flow(&mut staged_output, pure_backend, &mut drop_policy);
        let after = match flow_fiber_line_handle_owners(&candidate.fiber) {
            Ok(owners) => owners,
            Err(error) => {
                self.fail_eval(error, output);
                return;
            }
        };
        let receipt = match candidate.dialogue_activations.reconcile_parent_fiber(
            candidate.fiber.execution,
            &before,
            &after,
            drop_policy,
        ) {
            Ok(receipt) => receipt,
            Err(error) => {
                self.fail_eval(error, output);
                return;
            }
        };
        staged_output
            .requests
            .line_commands
            .extend(receipt.into_commands());
        *self = candidate;
        output.merge(staged_output);
    }

    fn main_fiber_can_attempt_runtime_op(&self) -> bool {
        self.main_started
            && !matches!(
                self.fiber.status,
                FlowFiberStatus::Done(_) | FlowFiberStatus::Failed(_)
            )
    }

    /// Joined work controls parent flow completion. Detached line work remains
    /// executor work but is intentionally excluded from this local join.
    pub(super) fn has_joined_work(&self) -> bool {
        self.child_fibers
            .iter()
            .any(|child| child.owner.has_joined_work())
    }

    /// Executor work controls scheduling. It includes detached line fibers so
    /// their scopes are still unwound by their owning executor.
    pub(super) fn has_executor_work(&self) -> bool {
        !self.child_fibers.is_empty()
    }

    /// Requests cancellation without deleting a queued child. Cancel-and-join
    /// transitions the owner into Closing; the scheduler subsequently enters
    /// that fiber and performs its lexical cleanup before reporting completion.
    pub(super) fn request_line_task_cancellation(&mut self) {
        for child in &mut self.child_fibers {
            let FlowFiberOwner::LineTask(owner) = &mut child.owner else {
                continue;
            };
            match owner.cancel_policy {
                ChildCancelPolicy::Finish => {}
                // Builder admission rejects Detach until it has a proved
                // ownership-transfer target. A malformed in-memory plan must
                // fail closed rather than silently changing ownership.
                ChildCancelPolicy::CancelAndJoin | ChildCancelPolicy::Detach => {
                    owner.closing = true;
                }
            }
        }
    }

    fn pending_ops_len(&self) -> usize {
        self.fiber.pending_ops.len()
            + self
                .child_fibers
                .iter()
                .map(|fiber| fiber.pending_ops.len())
                .sum::<usize>()
    }

    fn allocate_fiber_identity(
        &mut self,
    ) -> Result<
        (
            FlowFiberId,
            RuntimePersistentFiberId,
            crate::runtime_id::ExecutionInstanceId,
        ),
        RuntimeEvalError,
    > {
        let ordinal = self.next_fiber_id;
        let allocated = ordinal
            .checked_add(1)
            .and_then(std::num::NonZeroU64::new)
            .ok_or(RuntimeEvalError::FiberIdentityOverflow)?;
        self.next_fiber_id = self
            .next_fiber_id
            .checked_add(1)
            .ok_or(RuntimeEvalError::FiberIdentityOverflow)?;
        Ok((
            FlowFiberId(ordinal),
            RuntimePersistentFiberId::from_allocated(allocated.get()),
            crate::runtime_id::ExecutionInstanceId::from_allocated(allocated),
        ))
    }

    pub(super) fn spawn_child_fiber(&mut self, body: Vec<FlowOp>) -> Result<(), RuntimeEvalError> {
        let mut pending_ops = VecDeque::with_capacity(body.len().saturating_add(2));
        if !body.is_empty() {
            pending_ops.push_front(FlowOp::ExitScope);
            for op in body.into_iter().rev() {
                pending_ops.push_front(op);
            }
            pending_ops.push_front(FlowOp::EnterScope);
        }
        let (id, persistent_id, execution) = self.allocate_fiber_identity()?;
        self.child_fibers.push_back(FlowFiber {
            line_cursor: 0,
            cursor: None,
            pending_ops,
            control_stack: Vec::new(),
            await_observer: None,
            root_cleanups: Vec::new(),
            env: self.fiber.env.clone(),
            observations: RuntimeObservationState::default(),
            stream_states: BTreeMap::new(),
            id,
            persistent_id,
            execution,
            owner: FlowFiberOwner::Executor,
            status: FlowFiberStatus::Running,
        });
        self.run_child_next = true;
        Ok(())
    }

    pub(super) fn capture_line_task_locals(
        &self,
        group: &LineTaskGroup,
    ) -> Result<Box<[RuntimeLocalBinding]>, crate::line_task::LineRuntimeError> {
        group
            .captures()
            .iter()
            .map(|local| {
                let value = self.fiber.env.get(*local).ok_or(
                    crate::line_task::LineRuntimeError::UnknownOwnedLocal { local: *local },
                )?;
                if !value.ownership().permits_copy() {
                    return Err(crate::line_task::LineRuntimeError::AffineGroupCapture);
                }
                Ok(RuntimeLocalBinding {
                    local: *local,
                    value: value.clone(),
                })
            })
            .collect::<Result<Vec<_>, crate::line_task::LineRuntimeError>>()
            .map(Vec::into_boxed_slice)
    }

    pub(super) fn prepare_line_task_commands(
        &self,
        transaction: &mut dialogue::DialogueActivationTransaction,
        group: &LineTaskGroup,
        activation: crate::line_task::LineTaskActivation,
        captures: &[RuntimeLocalBinding],
        request_cancellation: bool,
    ) -> Result<NativeLineTaskExecutionBatch, dialogue::DialogueExecutionError> {
        let activation_id = transaction.activation().clone();
        let mut batch = NativeLineTaskExecutionBatch {
            child_fibers: self.child_fibers.clone(),
            next_fiber_id: self.next_fiber_id,
            run_child_next: self.run_child_next,
        };
        if request_cancellation {
            for child in &mut batch.child_fibers {
                let FlowFiberOwner::LineTask(owner) = &mut child.owner else {
                    continue;
                };
                if owner.tag.activation_id() != &activation_id {
                    continue;
                }
                match owner.cancel_policy {
                    ChildCancelPolicy::Finish => {}
                    ChildCancelPolicy::CancelAndJoin => owner.closing = true,
                    ChildCancelPolicy::Detach => {
                        return Err(
                            crate::line_task::LineRuntimeError::InvalidActivationOperation.into(),
                        );
                    }
                }
            }
        }
        for completion in activation.scheduled_completions {
            transaction
                .line_mut()
                .complete_unstarted_scheduled(&completion)?;
        }
        for command in activation.commands {
            match command {
                crate::line_task::LineTaskCommand::Run { tag, policy } => {
                    let ops = group.command_ops(&tag);
                    let mut pending_ops = VecDeque::with_capacity(ops.len().saturating_add(2));
                    pending_ops.push_front(FlowOp::ExitScope);
                    for op in ops.iter().rev().cloned() {
                        pending_ops.push_front(op);
                    }
                    pending_ops.push_front(FlowOp::EnterScope);
                    let selected_captures = if let Some(token) = tag.scheduled_token().cloned() {
                        transaction
                            .line_mut()
                            .take_scheduled_capture_packet(&token)?
                            .into_vec()
                    } else {
                        if captures
                            .iter()
                            .any(|capture| !capture.value.ownership().permits_copy())
                        {
                            return Err(
                                crate::line_task::LineRuntimeError::AffineGroupCapture.into()
                            );
                        }
                        captures.to_vec()
                    };
                    let mut env = RuntimeEnv::default();
                    env.bind_all(selected_captures);
                    let ordinal = batch.next_fiber_id;
                    let allocated = ordinal
                        .checked_add(1)
                        .and_then(std::num::NonZeroU64::new)
                        .ok_or(RuntimeEvalError::FiberIdentityOverflow)?;
                    batch.next_fiber_id = batch
                        .next_fiber_id
                        .checked_add(1)
                        .ok_or(RuntimeEvalError::FiberIdentityOverflow)?;
                    batch.child_fibers.push_back(FlowFiber {
                        line_cursor: 0,
                        cursor: None,
                        pending_ops,
                        control_stack: Vec::new(),
                        await_observer: None,
                        root_cleanups: Vec::new(),
                        env,
                        observations: RuntimeObservationState::default(),
                        stream_states: BTreeMap::new(),
                        id: FlowFiberId(ordinal),
                        persistent_id: RuntimePersistentFiberId::from_allocated(allocated.get()),
                        execution: crate::runtime_id::ExecutionInstanceId::from_allocated(
                            allocated,
                        ),
                        owner: FlowFiberOwner::LineTask(LineTaskFiberOwner {
                            tag,
                            join_policy: policy.join,
                            cancel_policy: policy.cancel,
                            closing: false,
                        }),
                        status: FlowFiberStatus::Running,
                    });
                    batch.run_child_next = true;
                }
                crate::line_task::LineTaskCommand::Cancel { tag } => {
                    for child in &mut batch.child_fibers {
                        let FlowFiberOwner::LineTask(owner) = &mut child.owner else {
                            continue;
                        };
                        if owner.tag == tag {
                            match owner.cancel_policy {
                                ChildCancelPolicy::Finish => {}
                                ChildCancelPolicy::CancelAndJoin => owner.closing = true,
                                ChildCancelPolicy::Detach => {
                                    return Err(crate::line_task::LineRuntimeError::InvalidActivationOperation.into());
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(batch)
    }

    pub(super) fn commit_line_task_execution_batch(&mut self, batch: NativeLineTaskExecutionBatch) {
        self.child_fibers = batch.child_fibers;
        self.next_fiber_id = batch.next_fiber_id;
        self.run_child_next = batch.run_child_next;
    }

    fn step_next_child_fiber(
        &mut self,
        input: &RuntimeStepInput,
        events: &[TaskEvent],
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> bool {
        let mut candidate = self.clone();
        let mut staged_output = RuntimeStepOutput::default();
        match candidate.step_next_child_fiber_candidate(
            input,
            events,
            &mut staged_output,
            pure_backend,
        ) {
            Ok(progressed) => {
                *self = candidate;
                output.merge(staged_output);
                progressed
            }
            Err(error) => {
                self.fail_eval(error, output);
                true
            }
        }
    }

    fn step_next_child_fiber_candidate(
        &mut self,
        input: &RuntimeStepInput,
        events: &[TaskEvent],
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<bool, dialogue::DialogueExecutionError> {
        let Some(mut child) = self.child_fibers.pop_front() else {
            return Ok(false);
        };
        let owner = child.owner.clone();
        let before_tokens = matches!(&owner, FlowFiberOwner::LineTask(_))
            .then(|| flow_fiber_line_handle_tokens(&child))
            .transpose()?;
        std::mem::swap(&mut self.fiber, &mut child);
        let mut drop_policy = None;
        self.step_active_child_fiber(input, events, output, pure_backend, &mut drop_policy);
        self.finish_active_child_if_exhausted();
        std::mem::swap(&mut self.fiber, &mut child);
        let live_tokens = matches!(&owner, FlowFiberOwner::LineTask(_))
            .then(|| flow_fiber_line_handle_tokens(&child))
            .transpose()?;
        if let FlowFiberOwner::LineTask(owner) = &owner {
            let mut transaction = self
                .dialogue_activations
                .begin_transaction(owner.tag.activation_id())?;
            transaction.line_mut().reconcile_child_scope_step(
                &owner.tag,
                before_tokens
                    .as_ref()
                    .ok_or(crate::line_task::LineRuntimeError::InvalidActivationOperation)?,
                live_tokens
                    .as_ref()
                    .ok_or(crate::line_task::LineRuntimeError::InvalidActivationOperation)?,
                drop_policy,
            )?;
            let receipt = self.dialogue_activations.commit_transaction(transaction)?;
            self.publish_dialogue_line_receipt(receipt.into_line(), output);
        }
        match child.status {
            FlowFiberStatus::Done(_) => {
                if let FlowFiberOwner::LineTask(owner) = owner {
                    let live_tokens = live_tokens
                        .ok_or(crate::line_task::LineRuntimeError::InvalidActivationOperation)?;
                    let returned_bindings = std::mem::take(&mut child.env)
                        .into_bindings()
                        .into_boxed_slice();
                    self.complete_line_task_work(
                        owner.tag,
                        returned_bindings,
                        live_tokens,
                        false,
                        owner.closing,
                        owner.join_policy == ChildJoinPolicy::Join,
                    )?;
                }
            }
            FlowFiberStatus::Failed(message) => {
                let failed_activation = match &owner {
                    FlowFiberOwner::LineTask(owner)
                        if owner.join_policy == ChildJoinPolicy::Join =>
                    {
                        Some(owner.tag.activation_id().clone())
                    }
                    FlowFiberOwner::Executor | FlowFiberOwner::LineTask(_) => None,
                };
                if let FlowFiberOwner::LineTask(owner) = owner {
                    let live_tokens = live_tokens
                        .ok_or(crate::line_task::LineRuntimeError::InvalidActivationOperation)?;
                    let returned_bindings = std::mem::take(&mut child.env)
                        .into_bindings()
                        .into_boxed_slice();
                    self.complete_line_task_work(
                        owner.tag,
                        returned_bindings,
                        live_tokens,
                        true,
                        false,
                        owner.join_policy == ChildJoinPolicy::Join,
                    )?;
                }
                if let Some(activation) = failed_activation {
                    let transaction = self.dialogue_activations.begin_transaction(&activation)?;
                    self.begin_dialogue_failure(
                        transaction,
                        dialogue::DialogueExecutionError::ChildFailed { message },
                        output,
                    );
                }
            }
            _ => self.child_fibers.push_back(child),
        }
        Ok(true)
    }

    fn complete_line_task_work(
        &mut self,
        tag: LineTaskWorkTag,
        returned_bindings: Box<[RuntimeLocalBinding]>,
        live_tokens: BTreeSet<crate::runtime_id::RuntimeLineHandleToken>,
        failed: bool,
        cancelled: bool,
        joined: bool,
    ) -> Result<(), dialogue::DialogueExecutionError> {
        let content = self
            .plan
            .dialogue_content()
            .get(tag.activation_id().content())
            .ok_or(crate::line_task::LineRuntimeError::UnknownContentPlan)?;
        let group_id = content
            .line_task_group()
            .ok_or(crate::line_task::LineRuntimeError::MissingTaskGroup)?;
        let group = self
            .plan
            .line_task_groups()
            .get(group_id.index())
            .cloned()
            .ok_or(crate::line_task::LineRuntimeError::UnknownTaskGroup)?;
        match &self.fiber.status {
            FlowFiberStatus::Dialogue(activation) if activation == tag.activation_id() => {}
            _ => return Err(crate::line_task::LineRuntimeError::StaleCommandOutcome.into()),
        }
        let mut transaction = self
            .dialogue_activations
            .begin_transaction(tag.activation_id())?;
        let (captures, mut live) = {
            let frame = transaction.frame();
            let dialogue::DialogueLineTaskState::Live(live) = &frame.line_task else {
                return Err(crate::line_task::LineRuntimeError::InvalidScheduledWorkState.into());
            };
            (frame.captures.clone(), live.clone())
        };
        let next = if joined {
            crate::line_task::complete_live_line_task_work(&group, &mut live, tag.clone(), failed)?
        } else {
            crate::line_task::LineTaskActivation::default()
        };
        if let Some(token) = tag.scheduled_token().cloned() {
            let terminal = if failed {
                crate::line_task::RuntimeScheduledState::Failed
            } else if cancelled {
                crate::line_task::RuntimeScheduledState::Cancelled
            } else {
                crate::line_task::RuntimeScheduledState::Completed
            };
            let mut returned_tokens = BTreeSet::new();
            for binding in &returned_bindings {
                for handle in binding
                    .value
                    .affine_line_handles()
                    .map_err(|_| crate::line_task::LineRuntimeError::InvalidScheduledCaptureGraph)?
                {
                    if !returned_tokens.insert(handle.token().clone()) {
                        return Err(
                            crate::line_task::LineRuntimeError::DuplicateHandleOccurrence.into(),
                        );
                    }
                }
            }
            transaction.line_mut().finish_child_scope(
                &tag,
                &live_tokens,
                &returned_tokens,
                crate::effect::RuntimeDropPolicy::Default,
            )?;
            transaction.line_mut().admit_scheduled_child_bindings(
                &token,
                returned_bindings,
                terminal,
            )?;
            transaction
                .line_mut()
                .complete_scheduled_work(&token, failed, cancelled)?;
        }
        if joined {
            transaction.frame_mut().line_task = dialogue::DialogueLineTaskState::Live(live);
        }
        let batch =
            self.prepare_line_task_commands(&mut transaction, &group, next, &captures, false)?;
        let receipt = self.dialogue_activations.commit_transaction(transaction)?;
        if !receipt.into_line().into_commands().is_empty() {
            return Err(crate::line_task::LineRuntimeError::UnexpectedPreparedCommands.into());
        }
        self.commit_line_task_execution_batch(batch);
        Ok(())
    }

    fn step_active_child_fiber(
        &mut self,
        input: &RuntimeStepInput,
        events: &[TaskEvent],
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
        drop_policy: &mut Option<crate::effect::RuntimeDropPolicy>,
    ) {
        if self.fiber.owner.requests_line_task_close() {
            self.close_active_line_task_fiber(output, pure_backend);
            return;
        }
        self.latch_active_await_observer_events(events);
        if self.resume_suspended(input, events, output, pure_backend) {
            return;
        }
        if !matches!(self.fiber.status, FlowFiberStatus::Running) {
            return;
        }
        if self.fiber.cursor.is_some() || !self.fiber.pending_ops.is_empty() {
            self.step_flow(output, pure_backend, drop_policy);
        }
    }

    fn close_active_line_task_fiber(
        &mut self,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        let frames = std::mem::take(&mut self.fiber.control_stack);
        for frame in frames.into_iter().rev() {
            if let FlowControlStackEntryKind::Scope { cleanups } = frame.kind {
                self.emit_scope_cleanups(cleanups, output, pure_backend);
            }
        }
        self.drain_root_cleanups(output, pure_backend);
        self.fiber.cursor = None;
        self.fiber.pending_ops.clear();
        self.fiber.await_observer = None;
        self.fiber.status = FlowFiberStatus::Done(FlowExit::Done);
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
        if self.has_executor_work()
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
        if self.has_executor_work()
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
