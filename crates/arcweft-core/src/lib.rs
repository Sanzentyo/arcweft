pub mod prelude {
    pub use arcweft_dialogue::{
        CancelAction, CancelOnDrop, CancelRule, CancelScope, CancelTrigger, Cue, CueAction,
        DialogueBuildError, DialogueBuildErrorKind, DialogueContent, DialogueContentPart,
        DialogueLine, DialogueLineBuilder, DialogueOptions, DialogueTag, InputEventKind, LineExit,
        LinePlan, LinePlanBuilder, LinePlanStep, OutPayload, PlanArg, PlanCall, PlanExpr,
        SayOptions, SpeakerPreset, SpeakerRef, TagArg, TextBoxRef, TimelineAnchor, TimelineCue,
        VoicePolicy, VoiceRef, character, line_id, textbox,
    };
    pub use arcweft_id::{EntityId, IdError, IdErrorKind, PublicId, TextKey};
    pub use arcweft_need::{Need, Progress, ProgressError};
    pub use arcweft_source::{SourceAnchor, SourceName, SourcePosition};
}

use thiserror::Error;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TickId(pub u64);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LogicalDuration {
    nanos: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FrameInput {
    pub tick: TickId,
    pub dt: LogicalDuration,
    pub input_events: Vec<InputEvent>,
    pub task_events: Vec<TaskEvent>,
    pub ui_events: Vec<UiEvent>,
    pub audio_events: Vec<AudioEvent>,
    pub source_events: Vec<SourceEvent<String, String>>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FrameOutput {
    pub diagnostics: Vec<RuntimeDiagnostic>,
    pub flow_events: Vec<FlowEvent>,
    pub line_effects: Vec<LineEffectRequest>,
    pub task_requests: Vec<TaskSpec>,
    pub cancel_requests: Vec<CancelScopeId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeDiagnostic {
    pub message: String,
}

/// Pure runtime program consumed by the minimal Sans I/O engine.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RuntimePlan {
    pub entry_flow: Option<FlowRuntimeId>,
    pub flows: Vec<RuntimeFlow>,
    pub line_task_groups: Vec<LineTaskGroup>,
}

/// Runtime identifier for a lowered flow.
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FlowRuntimeId(pub String);

/// Runtime identifier for a lowered dialogue line.
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeLineId(pub String);

/// Lowered flow program.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RuntimeFlow {
    pub id: FlowRuntimeId,
    pub ops: Vec<FlowOp>,
}

/// One deterministic operation in a lowered flow program.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FlowOp {
    Dialogue {
        line: RuntimeLineId,
        task_group: usize,
    },
    Choice {
        id: Option<String>,
        options: Vec<ChoiceRuntimeOption>,
    },
    Await {
        target: AwaitTarget,
        pending: Vec<LineEffectRequest>,
    },
    Goto(FlowRuntimeId),
    Return(String),
    Effect(LineEffectRequest),
    Noop,
}

/// Runtime choice option visible to adapters and selectable from `FrameInput`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChoiceRuntimeOption {
    pub id: Option<String>,
    pub label: String,
    pub target: Option<FlowRuntimeId>,
    pub out: Option<LineOutRequest>,
    pub effects: Vec<LineEffectRequest>,
}

/// Replay-observable flow event emitted by the core runtime.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FlowEvent {
    DialogueLine { line: RuntimeLineId },
    LineCancelled { trigger: String },
    ChoicePresented { id: Option<String> },
    ChoiceSelected { id: Option<String>, option: String },
    AwaitStarted { need: NeedId, task: TaskId },
    AwaitReady { need: NeedId, value: String },
    AwaitProgress { need: NeedId, progress: String },
    Goto { target: FlowRuntimeId },
    Return { value: String },
    Done,
}

/// Minimal deterministic engine over `FrameInput` and `FrameOutput`.
///
/// This is intentionally a data-model executor, not a backend runtime. It does
/// not spawn threads, read clocks, play audio, render frames, or perform file
/// I/O. It only advances a cursor through lowered line task groups and returns
/// effect/task requests for adapters or tests to observe.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Engine {
    plan: RuntimePlan,
    fiber: FlowFiber,
}

/// Current flow execution cursor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FlowFiber {
    pub line_cursor: usize,
    pub cursor: Option<FlowCursor>,
    pub status: FlowFiberStatus,
}

/// Position in a lowered flow program.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FlowCursor {
    pub flow: FlowRuntimeId,
    pub op_index: usize,
}

/// High-level flow status for the minimal runtime spine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FlowFiberStatus {
    Running,
    Waiting(AwaitState),
    Choice(ChoiceState),
    Done(FlowExit),
    Failed(String),
}

/// Suspended `await ... with` state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AwaitState {
    pub target: AwaitTarget,
    pub resume: FlowCursor,
}

/// Suspended choice state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChoiceState {
    pub id: Option<String>,
    pub options: Vec<ChoiceRuntimeOption>,
    pub resume: FlowCursor,
}

/// Terminal flow result observed by the minimal runtime.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FlowExit {
    Done,
    Return(String),
}

/// Error returned by checked runtime-plan construction helpers.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum RuntimePlanError {
    #[error("entry flow `{0}` does not exist in runtime plan")]
    MissingEntryFlow(String),
}

/// Scope exit reason used to select outcome-guarded cleanup stacks.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ScopeExit {
    #[default]
    Completed,
    Cancelled,
    Failed,
}

/// Sans I/O runtime model for a dialogue line's scoped task group.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LineTaskGroup {
    /// Root runtime scope for init work, child tasks, and grouped timeline work.
    pub root: LineTaskScope,
    /// Line option assignments such as `voice = auto`.
    pub options: Vec<LineOptionRequest>,
    /// Bindings introduced by `let PAT = EXPR` in a line plan.
    pub bindings: Vec<LineBindingRequest>,
    /// Values exported from the line plan with `out`.
    pub out: Vec<LineOutRequest>,
    /// Cancellation branches attached to this line.
    pub cancel_rules: Vec<LineCancelRuleRequest>,
    /// Memoization directives local to this line plan.
    pub memo: Vec<LineMemoRequest>,
    /// Runtime-checkable assertions attached to this line plan.
    pub assertions: Vec<LineAssertionRequest>,
    /// Automatic cleanup policy for line-owned handles and child tasks.
    pub cleanup: LineCleanupPolicy,
}

/// Runtime scope with a task graph and deterministic cleanup stacks.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LineTaskScope {
    pub node: LineTaskNode,
    pub defer_stack: Vec<Vec<LineEffectRequest>>,
    pub completed_defer_stack: Vec<Vec<LineEffectRequest>>,
    pub cancelled_defer_stack: Vec<Vec<LineEffectRequest>>,
    pub failed_defer_stack: Vec<Vec<LineEffectRequest>>,
}

/// Structured line-plan runtime graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LineTaskNode {
    Seq(Vec<LineTaskNode>),
    Start(Vec<LineTaskNode>),
    Parallel {
        policy: ParallelPolicy,
        children: Vec<LineTaskNode>,
    },
    Child(LineChildTask),
    Effect(LineEffectRequest),
}

impl Default for LineTaskNode {
    fn default() -> Self {
        Self::Seq(Vec::new())
    }
}

/// Parallel group execution policy.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ParallelPolicy {
    #[default]
    JoinAll,
}

/// A child task declared by `thread name { ... }` inside a line plan.
///
/// Thread-local cleanup is modeled as a scoped defer stack, not as line-level
/// `finally`. That keeps cancellation semantics identical for flow, handler,
/// and line-plan threads.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineChildTask {
    pub id: TaskId,
    pub key: Option<TaskKey>,
    pub name: Option<String>,
    pub trigger: LineTaskTrigger,
    pub priority: TaskPriority,
    pub join_policy: ChildJoinPolicy,
    pub cancel_policy: ChildCancelPolicy,
    pub scope: Box<LineTaskScope>,
}

/// Condition that starts a line-scoped child task.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum LineTaskTrigger {
    #[default]
    Immediate,
    Mark(String),
    Delay(LogicalDuration),
}

/// Whether the parent waits for a child task result.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ChildJoinPolicy {
    #[default]
    Join,
    Detached,
}

/// How a child task exits when its owning scope is cancelled.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ChildCancelPolicy {
    #[default]
    CancelAndJoin,
    Finish,
    Detach,
}

/// Option assignment preserved from a line plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineOptionRequest {
    pub name: String,
    pub value: String,
}

/// Binding preserved from a line plan before full HIR execution exists.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineBindingRequest {
    pub pattern: String,
    pub value: String,
}

/// `out` value exported from a line plan or cancel branch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineOutRequest {
    pub label: Option<String>,
    pub value: String,
}

/// Runtime representation of `cancel on ... { ... }`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineCancelRuleRequest {
    pub trigger: String,
    pub action: Vec<LineEffectRequest>,
}

/// Line-local memo directive.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineMemoRequest {
    pub name: String,
    pub options: Vec<RuntimeField>,
}

/// Runtime-checkable line assertion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineAssertionRequest {
    pub debug: bool,
    pub expr: String,
}

/// Declarative cleanup policy applied when the line scope exits.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LineCleanupPolicy {
    pub child_tasks: ChildTaskCleanup,
    pub presentation: PresentationCleanup,
    pub audio: AudioCleanup,
}

/// How line-scoped child tasks are treated on cleanup.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ChildTaskCleanup {
    #[default]
    CancelAndJoin,
    Detach,
    Finish,
}

/// How presentation handles registered in the line lifetime are cleaned up.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PresentationCleanup {
    #[default]
    DropRegistered,
    KeepRegistered,
}

/// How line-scoped audio handles are cleaned up.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AudioCleanup {
    #[default]
    StopRegistered,
    FadeRegistered,
    KeepRegistered,
}

/// Effect request emitted by core runtime without performing the effect itself.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LineEffectRequest {
    RegisterHandle {
        key: String,
        handle: String,
    },
    DropHandle {
        key: String,
    },
    WaitMark(String),
    Wait(LogicalDuration),
    Call(RuntimeCall),
    Log(RuntimeLog),
    SignalWrite(RuntimeAssignment),
    MetricWrite(RuntimeAssignment),
    EmitEvent(RuntimeEvent),
    Command(RuntimeCommand),
    Out(LineOutRequest),
    Return(String),
    Goto(String),
    Yield(String),
    Panic(String),
    Fail(String),
    Bail(String),
    Ensure {
        condition: String,
        message: String,
    },
    Close(String),
    Select(String),
    Break {
        label: Option<String>,
        value: Option<String>,
    },
    Continue {
        label: Option<String>,
    },
}

/// Access information used by static conflict checks for parallel regions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceAccess {
    pub key: String,
    pub mode: ResourceAccessMode,
    pub policy: ConflictPolicy,
}

/// Resource access kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceAccessMode {
    Read,
    Write,
    Drop,
    Append,
    Control,
}

/// Conflict resolution policy for resource accesses in a parallel region.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConflictPolicy {
    Error,
    Append,
    LastWriterWins { priority: i32 },
    MergePatch,
    Reduce { op: ReduceOp },
}

/// Deterministic reduce operator for mergeable parallel writes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReduceOp {
    Sum,
    Min,
    Max,
    And,
    Or,
}

/// Input event placeholder kept as Sans I/O data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InputEvent {
    pub kind: String,
    pub payload: Option<String>,
}

/// UI event placeholder kept as Sans I/O data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiEvent {
    pub kind: String,
    pub payload: Option<String>,
}

/// Audio event placeholder kept as Sans I/O data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AudioEvent {
    pub kind: String,
    pub payload: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskId(pub String);

#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskKey(pub String);

#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NeedId(pub String);

#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CancelScopeId(pub String);

#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceId(pub String);

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LogicalEpoch(pub u64);

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskSequence(pub u64);

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskPriority(pub i32);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AwaitTarget {
    pub need: NeedId,
    pub task: TaskId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskSpec {
    pub id: TaskId,
    pub key: TaskKey,
    pub class: TaskClass,
    pub priority: TaskPriority,
    pub cancel_scope: CancelScopeId,
    pub policy: TaskPolicy,
    pub source: TaskSource,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskHandle {
    pub id: TaskId,
    pub key: TaskKey,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SchedulerBudget {
    pub max_events: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TaskClass {
    LocalUi,
    Io,
    Cpu,
    GpuPrepare,
    ShaderCompile,
    WasmCall,
    AssetDecode,
    AudioDecode,
    AudioRender,
    TtsSynthesis,
    BgmPrecompose,
    Lsp,
    Background,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TaskPolicy {
    JoinSameKey,
    AlwaysStart,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskSource {
    pub label: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskEvent {
    pub logical_epoch: LogicalEpoch,
    pub task_id: TaskId,
    pub sequence: TaskSequence,
    pub kind: TaskEventKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TaskEventKind {
    Ready(String),
    Err(String),
    Cancelled,
    Progress(String),
}

pub trait TaskHost {
    fn ensure_task(&mut self, spec: TaskSpec) -> TaskHandle;
    fn cancel_scope(&mut self, scope: CancelScopeId);
    fn poll_frame(&mut self, budget: SchedulerBudget) -> Vec<TaskEvent>;
}

/// Returns task events in replay-stable completion order.
pub fn normalize_task_events(mut events: Vec<TaskEvent>) -> Vec<TaskEvent> {
    events.sort_by_key(|event| (event.logical_epoch, event.task_id.clone(), event.sequence));
    events
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourcePolicy {
    pub backpressure: BackpressurePolicy,
    pub replay: ReplayPolicy,
    pub max_queue: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BackpressurePolicy {
    LatestOnly,
    BoundedQueue {
        capacity: usize,
        on_overflow: OverflowPolicy,
    },
    BlockingNotAllowed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OverflowPolicy {
    DropOldest,
    DropNewest,
    Error,
    Coalesce,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReplayPolicy {
    Full,
    HashOnly,
    Summary,
    None,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceEvent<T, E> {
    pub source: SourceId,
    pub sequence: TaskSequence,
    pub kind: SourceEventKind<T, E>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceEventKind<T, E> {
    Item(T),
    Progress(String),
    Disconnected,
    PermissionRevoked,
    Error(E),
    End,
}

/// Call-shaped runtime request. The runtime adapter decides whether the callee
/// maps to presentation, audio, state, or user code.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeCall {
    pub callee: String,
    pub args: Vec<String>,
}

/// Structured log request preserved for defmt-style template interning later.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeLog {
    pub level: String,
    pub message: String,
    pub fields: Vec<RuntimeField>,
}

/// Assignment-like runtime request used by signal and metric updates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeAssignment {
    pub target: String,
    pub value: String,
}

/// Structured event emission request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeEvent {
    pub event: String,
    pub fields: Vec<RuntimeField>,
}

/// Statement-like command retained until the command family is canonicalized as
/// ordinary calls.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeCommand {
    pub name: String,
    pub args: Vec<String>,
}

/// Named expression payload preserved in runtime IR without performing I/O.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeField {
    pub name: String,
    pub value: String,
}

impl LogicalDuration {
    pub const fn from_nanos(nanos: u64) -> Self {
        Self { nanos }
    }

    pub const fn as_nanos(self) -> u64 {
        self.nanos
    }
}

impl Default for LogicalDuration {
    fn default() -> Self {
        Self::from_nanos(0)
    }
}

impl RuntimePlan {
    pub fn new(
        entry_flow: Option<FlowRuntimeId>,
        flows: Vec<RuntimeFlow>,
        line_task_groups: Vec<LineTaskGroup>,
    ) -> Result<Self, RuntimePlanError> {
        if let Some(entry) = entry_flow.as_ref()
            && !flows.iter().any(|flow| flow.id == *entry)
        {
            return Err(RuntimePlanError::MissingEntryFlow(entry.0.clone()));
        }
        Ok(Self {
            entry_flow,
            flows,
            line_task_groups,
        })
    }

    pub fn lines_only(line_task_groups: Vec<LineTaskGroup>) -> Self {
        Self {
            entry_flow: None,
            flows: Vec::new(),
            line_task_groups,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.flows.is_empty() && self.line_task_groups.is_empty()
    }

    pub fn entry_cursor(&self) -> Option<FlowCursor> {
        self.entry_flow.as_ref().map(|flow| FlowCursor {
            flow: flow.clone(),
            op_index: 0,
        })
    }
}

impl Default for FlowFiber {
    fn default() -> Self {
        Self {
            line_cursor: 0,
            cursor: None,
            status: FlowFiberStatus::Done(FlowExit::Done),
        }
    }
}

impl FlowCursor {
    fn advanced(&self) -> Self {
        Self {
            flow: self.flow.clone(),
            op_index: self.op_index + 1,
        }
    }
}

impl From<&str> for FlowRuntimeId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<&str> for RuntimeLineId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl Engine {
    pub fn new(plan: RuntimePlan) -> Self {
        let cursor = plan.entry_cursor();
        let status = if plan.is_empty() {
            FlowFiberStatus::Done(FlowExit::Done)
        } else {
            FlowFiberStatus::Running
        };
        Self {
            plan,
            fiber: FlowFiber {
                line_cursor: 0,
                cursor,
                status,
            },
        }
    }

    pub const fn fiber(&self) -> &FlowFiber {
        &self.fiber
    }

    pub fn step(&mut self, mut input: FrameInput) -> FrameOutput {
        let mut output = FrameOutput::default();
        let events = normalize_task_events(std::mem::take(&mut input.task_events));
        output
            .diagnostics
            .extend(events.iter().map(|event| RuntimeDiagnostic {
                message: format!(
                    "task {} sequence {} delivered",
                    event.task_id.0, event.sequence.0
                ),
            }));

        if self.resume_suspended(&input, &events, &mut output) {
            return output;
        }
        if !matches!(self.fiber.status, FlowFiberStatus::Running) {
            return output;
        }
        if self.fiber.cursor.is_some() {
            self.step_flow(&input, &mut output);
        } else {
            self.step_line_only(&input, &mut output);
        }
        output
    }

    fn resume_suspended(
        &mut self,
        input: &FrameInput,
        events: &[TaskEvent],
        output: &mut FrameOutput,
    ) -> bool {
        match self.fiber.status.clone() {
            FlowFiberStatus::Waiting(state) => {
                self.resume_await_state(state, events, output);
                true
            }
            FlowFiberStatus::Choice(state) => {
                self.resume_choice_state(state, input, output);
                true
            }
            FlowFiberStatus::Running | FlowFiberStatus::Done(_) | FlowFiberStatus::Failed(_) => {
                false
            }
        }
    }

    fn resume_await_state(
        &mut self,
        state: AwaitState,
        events: &[TaskEvent],
        output: &mut FrameOutput,
    ) {
        let Some(event) = events
            .iter()
            .find(|event| event.task_id == state.target.task)
            .cloned()
        else {
            self.fiber.status = FlowFiberStatus::Waiting(state);
            return;
        };
        match event.kind {
            TaskEventKind::Ready(value) => {
                output.flow_events.push(FlowEvent::AwaitReady {
                    need: state.target.need,
                    value,
                });
                self.fiber.cursor = Some(state.resume);
                self.fiber.status = FlowFiberStatus::Running;
            }
            TaskEventKind::Progress(progress) => {
                output.flow_events.push(FlowEvent::AwaitProgress {
                    need: state.target.need.clone(),
                    progress,
                });
                self.fiber.status = FlowFiberStatus::Waiting(state);
            }
            TaskEventKind::Err(error) => {
                self.fiber.status = FlowFiberStatus::Failed(error.clone());
                output.diagnostics.push(RuntimeDiagnostic {
                    message: format!("await task {} failed: {error}", state.target.task.0),
                });
            }
            TaskEventKind::Cancelled => {
                let message = format!("await task {} was cancelled", state.target.task.0);
                self.fiber.status = FlowFiberStatus::Failed(message.clone());
                output.diagnostics.push(RuntimeDiagnostic { message });
            }
        }
    }

    fn resume_choice_state(
        &mut self,
        state: ChoiceState,
        input: &FrameInput,
        output: &mut FrameOutput,
    ) {
        let Some(option) = state
            .options
            .iter()
            .find(|option| input_selects_choice(input, option))
            .cloned()
        else {
            self.fiber.status = FlowFiberStatus::Choice(state);
            return;
        };
        let selected = option.id.clone().unwrap_or_else(|| option.label.clone());
        output.flow_events.push(FlowEvent::ChoiceSelected {
            id: state.id.clone(),
            option: selected,
        });
        output.line_effects.extend(option.effects.clone());
        if let Some(out) = option.out {
            output.line_effects.push(LineEffectRequest::Out(out));
        }
        if let Some(target) = option.target {
            self.goto(target, output);
        } else {
            self.fiber.cursor = Some(state.resume);
            self.fiber.status = FlowFiberStatus::Running;
        }
    }

    fn step_flow(&mut self, input: &FrameInput, output: &mut FrameOutput) {
        let Some(cursor) = self.fiber.cursor.clone() else {
            return;
        };
        let Some(op) = self
            .plan
            .flows
            .iter()
            .find(|flow| flow.id == cursor.flow)
            .and_then(|flow| flow.ops.get(cursor.op_index))
            .cloned()
        else {
            self.finish(output);
            return;
        };
        let next = cursor.advanced();
        match op {
            FlowOp::Dialogue { line, task_group } => {
                output.flow_events.push(FlowEvent::DialogueLine { line });
                let Some(group) = self.plan.line_task_groups.get(task_group) else {
                    self.fiber.status =
                        FlowFiberStatus::Failed(format!("missing line task group {task_group}"));
                    return;
                };
                output.merge(run_line_task_group_for_input(group, input));
                if !self.apply_control_effects(output) {
                    self.fiber.cursor = Some(next);
                }
            }
            FlowOp::Choice { id, options } => {
                output
                    .flow_events
                    .push(FlowEvent::ChoicePresented { id: id.clone() });
                self.fiber.status = FlowFiberStatus::Choice(ChoiceState {
                    id,
                    options,
                    resume: next,
                });
            }
            FlowOp::Await { target, pending } => {
                output.flow_events.push(FlowEvent::AwaitStarted {
                    need: target.need.clone(),
                    task: target.task.clone(),
                });
                output.line_effects.extend(pending);
                output.task_requests.push(await_task_spec(&target));
                self.fiber.status = FlowFiberStatus::Waiting(AwaitState {
                    target,
                    resume: next,
                });
            }
            FlowOp::Goto(target) => self.goto(target, output),
            FlowOp::Return(value) => self.return_value(value, output),
            FlowOp::Effect(effect) => {
                output.line_effects.push(effect);
                if !self.apply_control_effects(output) {
                    self.fiber.cursor = Some(next);
                }
            }
            FlowOp::Noop => {
                self.fiber.cursor = Some(next);
            }
        }
    }

    fn step_line_only(&mut self, input: &FrameInput, output: &mut FrameOutput) {
        let Some(group) = self.plan.line_task_groups.get(self.fiber.line_cursor) else {
            self.finish(output);
            return;
        };
        output.merge(run_line_task_group_for_input(group, input));
        self.fiber.line_cursor += 1;
        if self.fiber.line_cursor >= self.plan.line_task_groups.len() {
            self.finish(output);
        }
    }

    fn apply_control_effects(&mut self, output: &mut FrameOutput) -> bool {
        let Some(control) = output.line_effects.iter().find_map(control_from_effect) else {
            return false;
        };
        match control {
            FlowControl::Goto(target) => self.goto(FlowRuntimeId(target), output),
            FlowControl::Return(value) => self.return_value(value, output),
            FlowControl::Failed(message) => self.fiber.status = FlowFiberStatus::Failed(message),
        }
        true
    }

    fn goto(&mut self, target: FlowRuntimeId, output: &mut FrameOutput) {
        output.flow_events.push(FlowEvent::Goto {
            target: target.clone(),
        });
        self.fiber.cursor = Some(FlowCursor {
            flow: target,
            op_index: 0,
        });
        self.fiber.status = FlowFiberStatus::Running;
    }

    fn return_value(&mut self, value: String, output: &mut FrameOutput) {
        output.flow_events.push(FlowEvent::Return {
            value: value.clone(),
        });
        self.fiber.status = FlowFiberStatus::Done(FlowExit::Return(value));
    }

    fn finish(&mut self, output: &mut FrameOutput) {
        output.flow_events.push(FlowEvent::Done);
        self.fiber.status = FlowFiberStatus::Done(FlowExit::Done);
    }
}

impl FrameOutput {
    fn merge(&mut self, other: Self) {
        self.diagnostics.extend(other.diagnostics);
        self.flow_events.extend(other.flow_events);
        self.line_effects.extend(other.line_effects);
        self.task_requests.extend(other.task_requests);
        self.cancel_requests.extend(other.cancel_requests);
    }
}

enum FlowControl {
    Goto(String),
    Return(String),
    Failed(String),
}

fn control_from_effect(effect: &LineEffectRequest) -> Option<FlowControl> {
    match effect {
        LineEffectRequest::Goto(target) => Some(FlowControl::Goto(target.clone())),
        LineEffectRequest::Return(value) => Some(FlowControl::Return(value.clone())),
        LineEffectRequest::Panic(message)
        | LineEffectRequest::Fail(message)
        | LineEffectRequest::Bail(message) => Some(FlowControl::Failed(message.clone())),
        _ => None,
    }
}

fn input_selects_choice(input: &FrameInput, option: &ChoiceRuntimeOption) -> bool {
    input.input_events.iter().any(|event| {
        let Some(payload) = event.payload.as_deref() else {
            return false;
        };
        matches!(event.kind.as_str(), "choice" | "select")
            && (option.id.as_deref() == Some(payload) || option.label == payload)
    })
}

fn await_task_spec(target: &AwaitTarget) -> TaskSpec {
    TaskSpec {
        id: target.task.clone(),
        key: TaskKey(target.task.0.clone()),
        class: TaskClass::Background,
        priority: TaskPriority(0),
        cancel_scope: CancelScopeId("flow".to_owned()),
        policy: TaskPolicy::JoinSameKey,
        source: TaskSource {
            label: format!("await {}", target.need.0),
        },
    }
}

/// Runs one line task group to its immediate Sans I/O requests.
///
/// Child tasks are represented both as `TaskSpec`s and as scoped effect bodies
/// so tests and future adapters can inspect the deterministic body plan without
/// requiring a native scheduler.
pub fn run_line_task_group(
    group: &LineTaskGroup,
    input: &FrameInput,
    exit: ScopeExit,
) -> FrameOutput {
    let mut output = FrameOutput::default();
    run_scope(&group.root, input, exit, &mut output);
    output
}

fn run_line_task_group_for_input(group: &LineTaskGroup, input: &FrameInput) -> FrameOutput {
    if let Some(rule) = group
        .cancel_rules
        .iter()
        .find(|rule| input_matches_trigger(input, &rule.trigger))
    {
        let mut output = FrameOutput::default();
        output.flow_events.push(FlowEvent::LineCancelled {
            trigger: rule.trigger.clone(),
        });
        output.line_effects.extend(rule.action.clone());
        run_scope_cleanup(&group.root, ScopeExit::Cancelled, &mut output);
        output
    } else {
        run_line_task_group(group, input, ScopeExit::Completed)
    }
}

fn input_matches_trigger(input: &FrameInput, trigger: &str) -> bool {
    input.input_events.iter().any(|event| {
        if event.kind == trigger {
            return true;
        }
        let Some(payload) = event.payload.as_deref() else {
            return false;
        };
        trigger == format!("{} {payload}", event.kind)
            || trigger == format!("{}:{payload}", event.kind)
    })
}

fn run_scope(scope: &LineTaskScope, input: &FrameInput, exit: ScopeExit, output: &mut FrameOutput) {
    run_node(&scope.node, input, output);
    run_scope_cleanup(scope, exit, output);
}

fn run_scope_cleanup(scope: &LineTaskScope, exit: ScopeExit, output: &mut FrameOutput) {
    output
        .line_effects
        .extend(flatten_defer_stack(&scope.defer_stack));
    output
        .line_effects
        .extend(flatten_defer_stack(outcome_defer_stack(scope, exit)));
}

fn run_node(node: &LineTaskNode, input: &FrameInput, output: &mut FrameOutput) {
    match node {
        LineTaskNode::Seq(nodes) | LineTaskNode::Start(nodes) => {
            for node in nodes {
                run_node(node, input, output);
            }
        }
        LineTaskNode::Parallel { children, .. } => {
            for child in children {
                run_node(child, input, output);
            }
        }
        LineTaskNode::Child(task) => run_child_task(task, input, output),
        LineTaskNode::Effect(effect) => output.line_effects.push(effect.clone()),
    }
}

fn run_child_task(task: &LineChildTask, input: &FrameInput, output: &mut FrameOutput) {
    if !trigger_is_ready(&task.trigger, input) {
        return;
    }
    output.task_requests.push(task_spec(task));
    run_scope(&task.scope, input, ScopeExit::Completed, output);
}

fn trigger_is_ready(trigger: &LineTaskTrigger, input: &FrameInput) -> bool {
    match trigger {
        LineTaskTrigger::Immediate => true,
        LineTaskTrigger::Mark(name) => input.input_events.iter().any(|event| {
            (event.kind == "mark" && event.payload.as_deref() == Some(name.as_str()))
                || event.kind == format!("mark:{name}")
        }),
        LineTaskTrigger::Delay(duration) => input.dt.as_nanos() >= duration.as_nanos(),
    }
}

fn task_spec(task: &LineChildTask) -> TaskSpec {
    let key = task
        .key
        .clone()
        .unwrap_or_else(|| TaskKey(task.id.0.clone()));
    TaskSpec {
        id: task.id.clone(),
        key,
        class: TaskClass::LocalUi,
        priority: task.priority,
        cancel_scope: CancelScopeId("line".to_owned()),
        policy: TaskPolicy::JoinSameKey,
        source: TaskSource {
            label: task
                .name
                .clone()
                .unwrap_or_else(|| "anonymous line task".to_owned()),
        },
    }
}

fn outcome_defer_stack(scope: &LineTaskScope, exit: ScopeExit) -> &[Vec<LineEffectRequest>] {
    match exit {
        ScopeExit::Completed => &scope.completed_defer_stack,
        ScopeExit::Cancelled => &scope.cancelled_defer_stack,
        ScopeExit::Failed => &scope.failed_defer_stack,
    }
}

fn flatten_defer_stack(stack: &[Vec<LineEffectRequest>]) -> Vec<LineEffectRequest> {
    stack.iter().rev().flatten().cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_task_events_by_replay_stable_keys() {
        let events = vec![
            TaskEvent {
                logical_epoch: LogicalEpoch(1),
                task_id: TaskId("b".to_owned()),
                sequence: TaskSequence(0),
                kind: TaskEventKind::Ready("b".to_owned()),
            },
            TaskEvent {
                logical_epoch: LogicalEpoch(0),
                task_id: TaskId("z".to_owned()),
                sequence: TaskSequence(9),
                kind: TaskEventKind::Ready("z".to_owned()),
            },
            TaskEvent {
                logical_epoch: LogicalEpoch(1),
                task_id: TaskId("a".to_owned()),
                sequence: TaskSequence(1),
                kind: TaskEventKind::Ready("a".to_owned()),
            },
        ];

        let normalized = normalize_task_events(events);
        let keys: Vec<_> = normalized
            .iter()
            .map(|event| {
                (
                    event.logical_epoch,
                    event.task_id.0.as_str(),
                    event.sequence,
                )
            })
            .collect();
        assert_eq!(
            keys,
            vec![
                (LogicalEpoch(0), "z", TaskSequence(9)),
                (LogicalEpoch(1), "a", TaskSequence(1)),
                (LogicalEpoch(1), "b", TaskSequence(0)),
            ]
        );
    }

    #[test]
    fn source_policy_is_pure_data() {
        let policy = SourcePolicy {
            backpressure: BackpressurePolicy::BoundedQueue {
                capacity: 8,
                on_overflow: OverflowPolicy::Coalesce,
            },
            replay: ReplayPolicy::HashOnly,
            max_queue: 8,
        };

        assert!(matches!(
            policy.backpressure,
            BackpressurePolicy::BoundedQueue {
                capacity: 8,
                on_overflow: OverflowPolicy::Coalesce,
            }
        ));
        assert_eq!(policy.replay, ReplayPolicy::HashOnly);
    }

    fn call(name: &str) -> LineEffectRequest {
        LineEffectRequest::Call(RuntimeCall {
            callee: name.to_owned(),
            args: Vec::new(),
        })
    }

    #[test]
    fn engine_steps_line_task_groups_as_sans_io_effects() {
        let group = LineTaskGroup {
            root: LineTaskScope {
                node: LineTaskNode::Seq(vec![LineTaskNode::Effect(call("line_start"))]),
                defer_stack: vec![vec![call("line_defer")]],
                completed_defer_stack: vec![vec![call("line_completed")]],
                ..LineTaskScope::default()
            },
            ..LineTaskGroup::default()
        };
        let mut engine = Engine::new(RuntimePlan::lines_only(vec![group]));

        let output = engine.step(FrameInput::default());

        assert_eq!(
            output.line_effects,
            vec![
                call("line_start"),
                call("line_defer"),
                call("line_completed")
            ]
        );
        assert_eq!(engine.fiber().status, FlowFiberStatus::Done(FlowExit::Done));
    }

    #[test]
    fn engine_steps_flow_ops_and_applies_goto() {
        let group = LineTaskGroup {
            root: LineTaskScope {
                node: LineTaskNode::Seq(vec![LineTaskNode::Effect(call("opening_line"))]),
                ..LineTaskScope::default()
            },
            ..LineTaskGroup::default()
        };
        let plan = RuntimePlan::new(
            Some(FlowRuntimeId("flow.opening".to_owned())),
            vec![
                RuntimeFlow {
                    id: FlowRuntimeId("flow.opening".to_owned()),
                    ops: vec![
                        FlowOp::Dialogue {
                            line: RuntimeLineId("say.opening.001".to_owned()),
                            task_group: 0,
                        },
                        FlowOp::Goto(FlowRuntimeId("flow.next".to_owned())),
                    ],
                },
                RuntimeFlow {
                    id: FlowRuntimeId("flow.next".to_owned()),
                    ops: vec![FlowOp::Return("Ok(FlowExit::Done)".to_owned())],
                },
            ],
            vec![group],
        )
        .expect("flow plan is valid");
        let mut engine = Engine::new(plan);

        let first = engine.step(FrameInput::default());
        assert_eq!(first.line_effects, vec![call("opening_line")]);
        assert!(matches!(
            first.flow_events.as_slice(),
            [FlowEvent::DialogueLine { .. }]
        ));

        let second = engine.step(FrameInput::default());
        assert_eq!(
            second.flow_events,
            vec![FlowEvent::Goto {
                target: FlowRuntimeId("flow.next".to_owned())
            }]
        );

        let third = engine.step(FrameInput::default());
        assert_eq!(
            third.flow_events,
            vec![FlowEvent::Return {
                value: "Ok(FlowExit::Done)".to_owned()
            }]
        );
        assert!(matches!(
            engine.fiber().status,
            FlowFiberStatus::Done(FlowExit::Return(_))
        ));
    }

    #[test]
    fn engine_waits_for_choice_input() {
        let option = ChoiceRuntimeOption {
            id: Some("choice.listen".to_owned()),
            label: "Listen".to_owned(),
            target: Some(FlowRuntimeId("flow.listen".to_owned())),
            out: None,
            effects: Vec::new(),
        };
        let plan = RuntimePlan::new(
            Some(FlowRuntimeId("flow.opening".to_owned())),
            vec![
                RuntimeFlow {
                    id: FlowRuntimeId("flow.opening".to_owned()),
                    ops: vec![FlowOp::Choice {
                        id: Some("choice.opening".to_owned()),
                        options: vec![option],
                    }],
                },
                RuntimeFlow {
                    id: FlowRuntimeId("flow.listen".to_owned()),
                    ops: vec![FlowOp::Return("listen".to_owned())],
                },
            ],
            Vec::new(),
        )
        .expect("choice plan is valid");
        let mut engine = Engine::new(plan);

        let presented = engine.step(FrameInput::default());
        assert_eq!(
            presented.flow_events,
            vec![FlowEvent::ChoicePresented {
                id: Some("choice.opening".to_owned())
            }]
        );
        assert!(matches!(engine.fiber().status, FlowFiberStatus::Choice(_)));

        let selected = engine.step(FrameInput {
            input_events: vec![InputEvent {
                kind: "choice".to_owned(),
                payload: Some("choice.listen".to_owned()),
            }],
            ..FrameInput::default()
        });
        assert_eq!(
            selected.flow_events,
            vec![
                FlowEvent::ChoiceSelected {
                    id: Some("choice.opening".to_owned()),
                    option: "choice.listen".to_owned()
                },
                FlowEvent::Goto {
                    target: FlowRuntimeId("flow.listen".to_owned())
                }
            ]
        );
    }

    #[test]
    fn engine_waits_for_await_task_event() {
        let target = AwaitTarget {
            need: NeedId("need.bg".to_owned()),
            task: TaskId("task.bg".to_owned()),
        };
        let plan = RuntimePlan::new(
            Some(FlowRuntimeId("flow.opening".to_owned())),
            vec![RuntimeFlow {
                id: FlowRuntimeId("flow.opening".to_owned()),
                ops: vec![
                    FlowOp::Await {
                        target: target.clone(),
                        pending: vec![call("show_loading")],
                    },
                    FlowOp::Return("ready".to_owned()),
                ],
            }],
            Vec::new(),
        )
        .expect("await plan is valid");
        let mut engine = Engine::new(plan);

        let waiting = engine.step(FrameInput::default());
        assert_eq!(waiting.line_effects, vec![call("show_loading")]);
        assert_eq!(waiting.task_requests[0].id, target.task);
        assert!(matches!(engine.fiber().status, FlowFiberStatus::Waiting(_)));

        let ready = engine.step(FrameInput {
            task_events: vec![TaskEvent {
                logical_epoch: LogicalEpoch(0),
                task_id: TaskId("task.bg".to_owned()),
                sequence: TaskSequence(0),
                kind: TaskEventKind::Ready("bg_handle".to_owned()),
            }],
            ..FrameInput::default()
        });
        assert!(ready.flow_events.iter().any(|event| matches!(
            event,
            FlowEvent::AwaitReady { value, .. } if value == "bg_handle"
        )));
        assert!(matches!(engine.fiber().status, FlowFiberStatus::Running));
    }

    #[test]
    fn line_cancel_rule_replaces_normal_line_body() {
        let group = LineTaskGroup {
            root: LineTaskScope {
                node: LineTaskNode::Seq(vec![LineTaskNode::Effect(call("normal"))]),
                cancelled_defer_stack: vec![vec![call("cancel_cleanup")]],
                ..LineTaskScope::default()
            },
            cancel_rules: vec![LineCancelRuleRequest {
                trigger: "input .SkipLine".to_owned(),
                action: vec![LineEffectRequest::Out(LineOutRequest {
                    label: None,
                    value: ".Skipped".to_owned(),
                })],
            }],
            ..LineTaskGroup::default()
        };
        let output = run_line_task_group_for_input(
            &group,
            &FrameInput {
                input_events: vec![InputEvent {
                    kind: "input".to_owned(),
                    payload: Some(".SkipLine".to_owned()),
                }],
                ..FrameInput::default()
            },
        );

        assert_eq!(
            output.flow_events,
            vec![FlowEvent::LineCancelled {
                trigger: "input .SkipLine".to_owned()
            }]
        );
        assert_eq!(
            output.line_effects,
            vec![
                LineEffectRequest::Out(LineOutRequest {
                    label: None,
                    value: ".Skipped".to_owned()
                }),
                call("cancel_cleanup")
            ]
        );
    }

    #[test]
    fn child_task_triggers_emit_task_request_and_scoped_body() {
        let child = LineChildTask {
            id: TaskId("line.task.0.mark".to_owned()),
            key: Some(TaskKey("line.task.mark".to_owned())),
            name: Some("mark".to_owned()),
            trigger: LineTaskTrigger::Mark(".seen".to_owned()),
            priority: TaskPriority(7),
            join_policy: ChildJoinPolicy::Join,
            cancel_policy: ChildCancelPolicy::CancelAndJoin,
            scope: Box::new(LineTaskScope {
                node: LineTaskNode::Seq(vec![LineTaskNode::Effect(call("handler"))]),
                defer_stack: vec![vec![call("handler_defer")]],
                ..LineTaskScope::default()
            }),
        };
        let group = LineTaskGroup {
            root: LineTaskScope {
                node: LineTaskNode::Seq(vec![LineTaskNode::Child(child)]),
                ..LineTaskScope::default()
            },
            ..LineTaskGroup::default()
        };
        let input = FrameInput {
            input_events: vec![InputEvent {
                kind: "mark".to_owned(),
                payload: Some(".seen".to_owned()),
            }],
            ..FrameInput::default()
        };

        let output = run_line_task_group(&group, &input, ScopeExit::Completed);

        assert_eq!(output.task_requests.len(), 1);
        assert_eq!(output.task_requests[0].priority, TaskPriority(7));
        assert_eq!(
            output.line_effects,
            vec![call("handler"), call("handler_defer")]
        );
    }
}
