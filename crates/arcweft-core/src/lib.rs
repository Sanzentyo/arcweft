use std::collections::{BTreeMap, BTreeSet, VecDeque};
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
    pub external_values: Vec<RuntimeBinding>,
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
    pub source_events: Vec<SourceEvent<String, String>>,
    pub stream_events: Vec<StreamEvent<String, String>>,
    pub source_close_requests: Vec<SourceId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeDiagnostic {
    pub message: String,
}

/// Named value provided by adapters or earlier runtime operations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeBinding {
    pub name: String,
    pub value: RuntimeValue,
}

/// Deterministic value domain used by the Sans I/O flow runtime.
///
/// Floats are preserved as source strings until a later numeric semantic pass
/// chooses exact representation and unit rules. That keeps this runtime model
/// deterministic across native and wasm targets.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeValue {
    Unit,
    Bool(bool),
    Int(i64),
    Float(String),
    String(String),
    Char(char),
    Duration(LogicalDuration),
    EntityRef(String),
    Tuple(Vec<RuntimeValue>),
    List(Vec<RuntimeValue>),
    Record(Vec<RuntimeFieldValue>),
    Variant {
        path: Option<String>,
        name: String,
        payload: Option<Box<RuntimeValue>>,
    },
}

/// One field inside a runtime record value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeFieldValue {
    pub name: String,
    pub value: RuntimeValue,
}

/// Expression subset executable by the Sans I/O flow runtime.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeExpr {
    Value(RuntimeValue),
    Local(String),
    EntityRef(String),
    Tuple(Vec<RuntimeExpr>),
    List(Vec<RuntimeExpr>),
    Record(Vec<RuntimeFieldExpr>),
    Variant {
        path: Option<String>,
        name: String,
        payload: Option<Box<RuntimeExpr>>,
    },
    Field {
        target: Box<RuntimeExpr>,
        field: String,
    },
    Unary {
        op: RuntimeUnaryOp,
        expr: Box<RuntimeExpr>,
    },
    Binary {
        lhs: Box<RuntimeExpr>,
        op: RuntimeBinaryOp,
        rhs: Box<RuntimeExpr>,
    },
    If {
        condition: Box<RuntimeExpr>,
        then_expr: Box<RuntimeExpr>,
        else_expr: Box<RuntimeExpr>,
    },
    Match {
        scrutinee: Box<RuntimeExpr>,
        arms: Vec<RuntimeExprMatchArm>,
    },
}

/// One value-producing `match` arm in a runtime expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeExprMatchArm {
    pub pattern: RuntimePattern,
    pub guard: Option<RuntimeExpr>,
    pub value: RuntimeExpr,
}

/// One field inside a runtime record expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeFieldExpr {
    pub name: String,
    pub value: RuntimeExpr,
}

/// Unary operator supported by the Sans I/O expression evaluator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeUnaryOp {
    Not,
    Neg,
}

/// Binary operator supported by the Sans I/O expression evaluator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeBinaryOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Add,
    Sub,
    Mul,
    Div,
    And,
    Or,
}

/// Pattern subset executable by the Sans I/O flow runtime.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimePattern {
    Ident(String),
    MutIdent(String),
    Discard,
    Literal(RuntimeValue),
    Entity(String),
    Tuple(Vec<RuntimePattern>),
    Record {
        path: Option<String>,
        fields: Vec<RuntimeRecordPatternField>,
        rest: bool,
    },
    List {
        items: Vec<RuntimePattern>,
        rest: Option<String>,
    },
    Variant {
        path: Option<String>,
        name: String,
        payload: Option<Box<RuntimePattern>>,
    },
    Whole {
        name: String,
        pattern: Box<RuntimePattern>,
    },
    Typed {
        name: String,
        ty: String,
    },
}

/// One field inside a runtime record pattern.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeRecordPatternField {
    pub name: String,
    pub pattern: RuntimePattern,
}

/// Lexical value environment for structured flow execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeEnv {
    scopes: Vec<BTreeMap<String, RuntimeValue>>,
}

/// Pure runtime program consumed by the minimal Sans I/O engine.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RuntimePlan {
    pub entry_flow: Option<FlowRuntimeId>,
    pub flows: Vec<RuntimeFlow>,
    pub line_task_groups: Vec<LineTaskGroup>,
    pub stream_plans: Vec<StreamPlan>,
    pub source_plans: Vec<SourcePlan>,
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

/// Runtime identifier for a lowered stream transform.
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StreamRuntimeId(pub String);

/// Lowered stream transform state machine.
///
/// The core runtime keeps this as deterministic data. Host adapters may execute
/// the state machine or replace it with an equivalent backend implementation,
/// but device acquisition never happens inside this plan.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StreamPlan {
    pub id: StreamRuntimeId,
    pub item_ty: String,
    pub error_ty: String,
    pub ops: Vec<StreamOp>,
}

/// One operation in a lowered stream transform.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StreamOp {
    Let {
        pattern: RuntimePattern,
        expr: RuntimeExpr,
    },
    ForNext {
        pattern: RuntimePattern,
        source: RuntimeExpr,
        body: Vec<StreamOp>,
    },
    Yield {
        expr: RuntimeExpr,
    },
    If {
        condition: RuntimeExpr,
        then_ops: Vec<StreamOp>,
        else_ops: Vec<StreamOp>,
    },
    Match {
        scrutinee: RuntimeExpr,
        arms: Vec<StreamMatchArm>,
    },
    Close {
        source: RuntimeExpr,
    },
    Return,
    Noop,
}

/// One stream `match` arm.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StreamMatchArm {
    pub pattern: RuntimePattern,
    pub guard: Option<RuntimeExpr>,
    pub ops: Vec<StreamOp>,
}

/// Lowered live source declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourcePlan {
    pub id: SourceId,
    pub item_ty: String,
    pub error_ty: String,
    pub from: RuntimeExpr,
    pub policy: SourcePolicy,
    pub handlers: Vec<SourceHandlerPlan>,
}

/// Handler for one live source event kind.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceHandlerPlan {
    Item {
        pattern: RuntimePattern,
        ops: Vec<SourceOp>,
    },
    Error {
        pattern: RuntimePattern,
        ops: Vec<SourceOp>,
    },
    Progress {
        pattern: RuntimePattern,
        ops: Vec<SourceOp>,
    },
    Disconnected {
        ops: Vec<SourceOp>,
    },
    PermissionRevoked {
        ops: Vec<SourceOp>,
    },
    End {
        ops: Vec<SourceOp>,
    },
}

/// Operation inside a source handler.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceOp {
    Yield(RuntimeExpr),
    Effect(LineEffectRequest),
    SignalWrite(RuntimeAssignment),
    Log(RuntimeLog),
    Close(SourceId),
    Noop,
}

/// One deterministic operation in a lowered flow program.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FlowOp {
    Bind(Vec<RuntimeBinding>),
    Let {
        pattern: RuntimePattern,
        expr: RuntimeExpr,
    },
    LetElse {
        pattern: RuntimePattern,
        expr: RuntimeExpr,
        else_ops: Vec<FlowOp>,
    },
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
    If {
        condition: RuntimeExpr,
        then_ops: Vec<FlowOp>,
        else_ops: Vec<FlowOp>,
    },
    IfLet {
        pattern: RuntimePattern,
        expr: RuntimeExpr,
        guard: Option<RuntimeExpr>,
        then_ops: Vec<FlowOp>,
        else_ops: Vec<FlowOp>,
    },
    Match {
        scrutinee: RuntimeExpr,
        arms: Vec<RuntimeMatchArm>,
    },
    Loop {
        body: Vec<FlowOp>,
    },
    LoopNext {
        body: Vec<FlowOp>,
    },
    While {
        condition: RuntimeExpr,
        body: Vec<FlowOp>,
    },
    WhileNext {
        condition: RuntimeExpr,
        body: Vec<FlowOp>,
    },
    WhileLet {
        pattern: RuntimePattern,
        expr: RuntimeExpr,
        guard: Option<RuntimeExpr>,
        body: Vec<FlowOp>,
    },
    WhileLetNext {
        pattern: RuntimePattern,
        expr: RuntimeExpr,
        guard: Option<RuntimeExpr>,
        body: Vec<FlowOp>,
    },
    For {
        pattern: RuntimePattern,
        source: RuntimeExpr,
        body: Vec<FlowOp>,
    },
    Scope(Vec<FlowOp>),
    Break(Option<RuntimeExpr>),
    Continue,
    Goto(FlowRuntimeId),
    GotoExpr(RuntimeExpr),
    Return(String),
    ReturnExpr(RuntimeExpr),
    Effect(LineEffectRequest),
    EnterScope,
    ExitScope,
    Noop,
}

/// One executable `match` arm in the runtime flow model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeMatchArm {
    pub pattern: RuntimePattern,
    pub guard: Option<RuntimeExpr>,
    pub ops: Vec<FlowOp>,
}

type RuntimeMatchSelection = Option<(Vec<RuntimeBinding>, Vec<FlowOp>)>;

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
    pub pending_ops: VecDeque<FlowOp>,
    pub frames: Vec<RuntimeFrame>,
    pub env: RuntimeEnv,
    pub source_states: BTreeMap<SourceId, SourceRuntimeState>,
    pub stream_states: BTreeMap<StreamRuntimeId, StreamRuntimeState>,
    pub status: FlowFiberStatus,
}

/// Replay-observable state for one active Sans I/O source queue.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceRuntimeState {
    pub id: SourceId,
    pub policy: SourcePolicy,
    pub queue: VecDeque<String>,
    pub closed: bool,
    pub last_error: Option<String>,
    pub overflow_count: u64,
}

/// Replay-observable state for one derived stream queue.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StreamRuntimeState {
    pub id: StreamRuntimeId,
    pub queue: VecDeque<String>,
    pub closed: bool,
    pub emitted_count: u64,
}

/// Runtime stack frame used to make scope exit and loop transfer explicit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeFrame {
    pub kind: RuntimeFrameKind,
}

/// Structured frame kind for the minimal flow executor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeFrameKind {
    Scope,
    Loop {
        body: Vec<FlowOp>,
    },
    While {
        condition: RuntimeExpr,
        body: Vec<FlowOp>,
    },
    WhileLet {
        pattern: RuntimePattern,
        expr: RuntimeExpr,
        guard: Option<RuntimeExpr>,
        body: Vec<FlowOp>,
    },
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

/// Error produced while evaluating runtime expressions.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum RuntimeEvalError {
    #[error("unknown runtime binding `{0}`")]
    UnknownBinding(String),
    #[error("expected bool expression, found {0}")]
    ExpectedBool(String),
    #[error("expected integer expression, found {0}")]
    ExpectedInt(String),
    #[error("expected entity reference expression, found {0}")]
    ExpectedEntityRef(String),
    #[error("expected list expression, found {0}")]
    ExpectedList(String),
    #[error("field `{field}` does not exist on {value}")]
    MissingField { field: String, value: String },
    #[error("operator `{op}` is not supported for {lhs} and {rhs}")]
    UnsupportedBinary {
        op: &'static str,
        lhs: String,
        rhs: String,
    },
    #[error("operator `{op}` is not supported for {value}")]
    UnsupportedUnary { op: &'static str, value: String },
    #[error("pattern did not match {0}")]
    PatternMismatch(String),
    #[error("pattern binds `{0}` more than once")]
    DuplicateBinding(String),
    #[error("loop control `{0}` reached a non-loop runtime context")]
    MisplacedLoopControl(&'static str),
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

/// Returns source events in replay-stable frame-boundary order.
pub fn normalize_source_events<T, E>(mut events: Vec<SourceEvent<T, E>>) -> Vec<SourceEvent<T, E>> {
    events.sort_by_key(|event| (event.source.clone(), event.sequence));
    events
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourcePolicy {
    pub backpressure: BackpressurePolicy,
    pub replay: ReplayPolicy,
    pub privacy: PrivacyPolicy,
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
    EventOnly,
    None,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PrivacyPolicy {
    Transient,
    Redacted,
    Recordable,
    Private,
}

impl Default for SourcePolicy {
    fn default() -> Self {
        Self {
            backpressure: BackpressurePolicy::LatestOnly,
            replay: ReplayPolicy::EventOnly,
            privacy: PrivacyPolicy::Transient,
            max_queue: 1,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceEvent<T, E> {
    pub source: SourceId,
    pub sequence: TaskSequence,
    pub kind: SourceEventKind<T, E>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StreamEvent<T, E> {
    pub stream: StreamRuntimeId,
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

impl SourceRuntimeState {
    pub fn new(id: SourceId, policy: SourcePolicy) -> Self {
        Self {
            id,
            policy,
            queue: VecDeque::new(),
            closed: false,
            last_error: None,
            overflow_count: 0,
        }
    }

    pub fn apply_event(&mut self, event: SourceEvent<String, String>) -> Option<String> {
        match event.kind {
            SourceEventKind::Item(item) => self.push_item(item),
            SourceEventKind::Error(error) => {
                self.last_error = Some(error.clone());
                Some(format!("source {} error: {error}", self.id.0))
            }
            SourceEventKind::Disconnected
            | SourceEventKind::PermissionRevoked
            | SourceEventKind::End => {
                self.closed = true;
                None
            }
            SourceEventKind::Progress(_) => None,
        }
    }

    pub fn close(&mut self) {
        self.closed = true;
        self.queue.clear();
    }

    fn push_item(&mut self, item: String) -> Option<String> {
        if self.closed {
            return Some(format!("source {} received item after close", self.id.0));
        }
        let backpressure = self.policy.backpressure.clone();
        match &backpressure {
            BackpressurePolicy::LatestOnly => {
                self.queue.clear();
                self.queue.push_back(item);
                None
            }
            BackpressurePolicy::BoundedQueue {
                capacity,
                on_overflow,
            } => self.push_bounded_item(*capacity, on_overflow, item),
            BackpressurePolicy::BlockingNotAllowed => {
                if self.queue.is_empty() {
                    self.queue.push_back(item);
                    None
                } else {
                    self.overflow_count += 1;
                    Some(format!(
                        "source {} overflowed a blocking-not-allowed queue",
                        self.id.0
                    ))
                }
            }
        }
    }

    fn push_bounded_item(
        &mut self,
        capacity: usize,
        on_overflow: &OverflowPolicy,
        item: String,
    ) -> Option<String> {
        if capacity == 0 {
            self.overflow_count += 1;
            return Some(format!("source {} has zero queue capacity", self.id.0));
        }
        if self.queue.len() < capacity {
            self.queue.push_back(item);
            return None;
        }
        self.overflow_count += 1;
        match on_overflow {
            OverflowPolicy::DropOldest => {
                self.queue.pop_front();
                self.queue.push_back(item);
                None
            }
            OverflowPolicy::DropNewest => None,
            OverflowPolicy::Error => Some(format!("source {} queue overflow", self.id.0)),
            OverflowPolicy::Coalesce => {
                self.queue.pop_back();
                self.queue.push_back(item);
                None
            }
        }
    }
}

impl StreamRuntimeState {
    pub fn new(id: StreamRuntimeId) -> Self {
        Self {
            id,
            queue: VecDeque::new(),
            closed: false,
            emitted_count: 0,
        }
    }

    pub fn push_item(&mut self, item: String) -> TaskSequence {
        let sequence = TaskSequence(self.emitted_count);
        self.emitted_count += 1;
        if !self.closed {
            self.queue.push_back(item);
        }
        sequence
    }

    pub fn close(&mut self) {
        self.closed = true;
        self.queue.clear();
    }
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
            stream_plans: Vec::new(),
            source_plans: Vec::new(),
        })
    }

    #[must_use]
    pub fn with_generation_plans(
        mut self,
        stream_plans: Vec<StreamPlan>,
        source_plans: Vec<SourcePlan>,
    ) -> Self {
        self.stream_plans = stream_plans;
        self.source_plans = source_plans;
        self
    }

    pub fn lines_only(line_task_groups: Vec<LineTaskGroup>) -> Self {
        Self {
            entry_flow: None,
            flows: Vec::new(),
            line_task_groups,
            stream_plans: Vec::new(),
            source_plans: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.flows.is_empty()
            && self.line_task_groups.is_empty()
            && self.stream_plans.is_empty()
            && self.source_plans.is_empty()
    }

    pub fn entry_cursor(&self) -> Option<FlowCursor> {
        self.entry_flow.as_ref().map(|flow| FlowCursor {
            flow: flow.clone(),
            op_index: 0,
        })
    }
}

impl Default for RuntimeEnv {
    fn default() -> Self {
        Self {
            scopes: vec![BTreeMap::new()],
        }
    }
}

impl RuntimeEnv {
    pub fn push_scope(&mut self) {
        self.scopes.push(BTreeMap::new());
    }

    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        } else if let Some(scope) = self.scopes.last_mut() {
            scope.clear();
        }
    }

    pub fn set(&mut self, name: impl Into<String>, value: RuntimeValue) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.into(), value);
        }
    }

    pub fn set_root(&mut self, name: impl Into<String>, value: RuntimeValue) {
        if let Some(scope) = self.scopes.first_mut() {
            scope.insert(name.into(), value);
        }
    }

    pub fn get(&self, name: &str) -> Option<&RuntimeValue> {
        self.scopes.iter().rev().find_map(|scope| scope.get(name))
    }

    pub fn bind_all(&mut self, bindings: impl IntoIterator<Item = RuntimeBinding>) {
        for binding in bindings {
            self.set(binding.name, binding.value);
        }
    }

    pub fn bind_all_root(&mut self, bindings: impl IntoIterator<Item = RuntimeBinding>) {
        for binding in bindings {
            self.set_root(binding.name, binding.value);
        }
    }
}

impl Default for FlowFiber {
    fn default() -> Self {
        Self {
            line_cursor: 0,
            cursor: None,
            pending_ops: VecDeque::new(),
            frames: Vec::new(),
            env: RuntimeEnv::default(),
            source_states: BTreeMap::new(),
            stream_states: BTreeMap::new(),
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

impl Default for FlowCursor {
    fn default() -> Self {
        Self {
            flow: FlowRuntimeId(String::new()),
            op_index: 0,
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
        Self {
            plan,
            fiber: FlowFiber {
                line_cursor: 0,
                cursor,
                pending_ops: VecDeque::new(),
                frames: Vec::new(),
                env: RuntimeEnv::default(),
                source_states,
                stream_states,
                status,
            },
        }
    }

    pub const fn fiber(&self) -> &FlowFiber {
        &self.fiber
    }

    pub fn step(&mut self, mut input: FrameInput) -> FrameOutput {
        let mut output = FrameOutput::default();
        self.fiber
            .env
            .bind_all_root(input.external_values.iter().cloned());
        let events = normalize_task_events(std::mem::take(&mut input.task_events));
        let source_events = normalize_source_events(std::mem::take(&mut input.source_events));
        output
            .diagnostics
            .extend(events.iter().map(|event| RuntimeDiagnostic {
                message: format!(
                    "task {} sequence {} delivered",
                    event.task_id.0, event.sequence.0
                ),
            }));
        self.apply_source_events(source_events, &mut output);
        self.step_stream_plans(&mut output);

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

    fn apply_source_events(
        &mut self,
        events: Vec<SourceEvent<String, String>>,
        output: &mut FrameOutput,
    ) {
        for event in events {
            output.source_events.push(event.clone());
            let plan = self
                .plan
                .source_plans
                .iter()
                .find(|plan| plan.id == event.source)
                .cloned();
            if let Some(plan) = plan {
                self.dispatch_source_event(&plan, event, output);
            } else {
                self.apply_unhandled_source_event(event, output);
            }
        }
    }

    fn dispatch_source_event(
        &mut self,
        plan: &SourcePlan,
        event: SourceEvent<String, String>,
        output: &mut FrameOutput,
    ) {
        self.record_source_event_state(&event, output);
        let mut handled = false;
        for handler in &plan.handlers {
            let Some((bindings, ops)) = source_handler_match(handler, &event.kind) else {
                continue;
            };
            handled = true;
            self.execute_source_ops(&plan.id, ops, bindings, output);
        }
        if !handled && matches!(event.kind, SourceEventKind::Item(_)) {
            self.apply_unhandled_source_event(event, output);
        }
    }

    fn apply_unhandled_source_event(
        &mut self,
        event: SourceEvent<String, String>,
        output: &mut FrameOutput,
    ) {
        let state = self
            .fiber
            .source_states
            .entry(event.source.clone())
            .or_insert_with(|| {
                SourceRuntimeState::new(event.source.clone(), SourcePolicy::default())
            });
        if let Some(message) = state.apply_event(event) {
            output.diagnostics.push(RuntimeDiagnostic { message });
        }
    }

    fn record_source_event_state(
        &mut self,
        event: &SourceEvent<String, String>,
        output: &mut FrameOutput,
    ) {
        let state = self
            .fiber
            .source_states
            .entry(event.source.clone())
            .or_insert_with(|| {
                SourceRuntimeState::new(event.source.clone(), SourcePolicy::default())
            });
        match &event.kind {
            SourceEventKind::Error(error) => {
                state.last_error = Some(error.clone());
                output.diagnostics.push(RuntimeDiagnostic {
                    message: format!("source {} error: {error}", state.id.0),
                });
            }
            SourceEventKind::Disconnected
            | SourceEventKind::PermissionRevoked
            | SourceEventKind::End => state.close(),
            SourceEventKind::Item(_) | SourceEventKind::Progress(_) => {}
        }
    }

    fn execute_source_ops(
        &mut self,
        source: &SourceId,
        ops: &[SourceOp],
        bindings: Vec<RuntimeBinding>,
        output: &mut FrameOutput,
    ) {
        let previous = self.fiber.env.clone();
        self.fiber.env.push_scope();
        self.fiber.env.bind_all(bindings);
        for op in ops {
            self.execute_source_op(source, op, output);
        }
        self.fiber.env = previous;
    }

    fn execute_source_op(&mut self, source: &SourceId, op: &SourceOp, output: &mut FrameOutput) {
        match op {
            SourceOp::Yield(expr) => match self.evaluate_expr(expr) {
                Ok(value) => self.push_source_item(source, runtime_value_label(&value), output),
                Err(error) => Self::diagnose_runtime_error(error, output),
            },
            SourceOp::Effect(effect) => output.line_effects.push(effect.clone()),
            SourceOp::SignalWrite(write) => output
                .line_effects
                .push(LineEffectRequest::SignalWrite(write.clone())),
            SourceOp::Log(log) => output
                .line_effects
                .push(LineEffectRequest::Log(log.clone())),
            SourceOp::Close(target) => self.close_source(target, output),
            SourceOp::Noop => {}
        }
    }

    fn push_source_item(&mut self, source: &SourceId, item: String, output: &mut FrameOutput) {
        let state = self
            .fiber
            .source_states
            .entry(source.clone())
            .or_insert_with(|| SourceRuntimeState::new(source.clone(), SourcePolicy::default()));
        if let Some(message) = state.push_item(item) {
            output.diagnostics.push(RuntimeDiagnostic { message });
        }
    }

    fn close_source(&mut self, source: &SourceId, output: &mut FrameOutput) {
        if let Some(state) = self.fiber.source_states.get_mut(source) {
            state.close();
        }
        output.source_close_requests.push(source.clone());
    }

    fn step_stream_plans(&mut self, output: &mut FrameOutput) {
        let stream_plans = self.plan.stream_plans.clone();
        for plan in stream_plans {
            let mut budget = 64usize;
            if !self.execute_stream_ops(&plan.id, &plan.ops, &mut budget, output) {
                continue;
            }
            if budget == 0 {
                output.diagnostics.push(RuntimeDiagnostic {
                    message: format!("stream {} exhausted frame budget", plan.id.0),
                });
            }
        }
    }

    fn execute_stream_ops(
        &mut self,
        stream: &StreamRuntimeId,
        ops: &[StreamOp],
        budget: &mut usize,
        output: &mut FrameOutput,
    ) -> bool {
        for op in ops {
            if *budget == 0 {
                return true;
            }
            *budget -= 1;
            if !self.execute_stream_op(stream, op, budget, output) {
                return false;
            }
        }
        true
    }

    fn execute_stream_op(
        &mut self,
        stream: &StreamRuntimeId,
        op: &StreamOp,
        budget: &mut usize,
        output: &mut FrameOutput,
    ) -> bool {
        match op {
            StreamOp::Let { pattern, expr } => self.bind_stream_let(pattern, expr, output),
            StreamOp::ForNext {
                pattern,
                source,
                body,
            } => self.execute_stream_for_next(stream, pattern, source, body, budget, output),
            StreamOp::Yield { expr } => self.yield_stream_item(stream, expr, output),
            StreamOp::If {
                condition,
                then_ops,
                else_ops,
            } => match self.evaluate_bool(condition) {
                Ok(true) => self.execute_stream_ops(stream, then_ops, budget, output),
                Ok(false) => self.execute_stream_ops(stream, else_ops, budget, output),
                Err(error) => {
                    Self::diagnose_runtime_error(error, output);
                    true
                }
            },
            StreamOp::Match { scrutinee, arms } => {
                self.execute_stream_match(stream, scrutinee, arms, budget, output)
            }
            StreamOp::Close { source } => {
                self.close_stream_source(source, output);
                true
            }
            StreamOp::Return => false,
            StreamOp::Noop => true,
        }
    }

    fn bind_stream_let(
        &mut self,
        pattern: &RuntimePattern,
        expr: &RuntimeExpr,
        output: &mut FrameOutput,
    ) -> bool {
        match self.evaluate_expr(expr) {
            Ok(value) => match self.try_bind_pattern(pattern, &value) {
                Ok(true) => true,
                Ok(false) => {
                    output.diagnostics.push(RuntimeDiagnostic {
                        message: format!(
                            "stream pattern did not match {}",
                            runtime_value_label(&value)
                        ),
                    });
                    true
                }
                Err(error) => {
                    Self::diagnose_runtime_error(error, output);
                    true
                }
            },
            Err(error) => {
                Self::diagnose_runtime_error(error, output);
                true
            }
        }
    }

    fn execute_stream_for_next(
        &mut self,
        stream: &StreamRuntimeId,
        pattern: &RuntimePattern,
        source: &RuntimeExpr,
        body: &[StreamOp],
        budget: &mut usize,
        output: &mut FrameOutput,
    ) -> bool {
        let Ok(source_key) = self.evaluate_queue_target(source) else {
            return true;
        };
        while let Some(item) = self.pop_queue_item(&source_key) {
            let previous = self.fiber.env.clone();
            self.fiber.env.push_scope();
            match match_runtime_pattern(pattern, &RuntimeValue::String(item)) {
                Ok(Some(bindings)) => {
                    self.fiber.env.bind_all(bindings);
                    if !self.execute_stream_ops(stream, body, budget, output) {
                        self.fiber.env = previous;
                        return false;
                    }
                }
                Ok(None) => output.diagnostics.push(RuntimeDiagnostic {
                    message: format!("stream for-next pattern did not match {source_key}"),
                }),
                Err(error) => Self::diagnose_runtime_error(error, output),
            }
            self.fiber.env = previous;
            if *budget == 0 {
                break;
            }
        }
        true
    }

    fn yield_stream_item(
        &mut self,
        stream: &StreamRuntimeId,
        expr: &RuntimeExpr,
        output: &mut FrameOutput,
    ) -> bool {
        match self.evaluate_expr(expr) {
            Ok(value) => {
                let item = runtime_value_label(&value);
                let state = self
                    .fiber
                    .stream_states
                    .entry(stream.clone())
                    .or_insert_with(|| StreamRuntimeState::new(stream.clone()));
                let sequence = state.push_item(item.clone());
                output.stream_events.push(StreamEvent {
                    stream: stream.clone(),
                    sequence,
                    kind: SourceEventKind::Item(item),
                });
            }
            Err(error) => Self::diagnose_runtime_error(error, output),
        }
        true
    }

    fn execute_stream_match(
        &mut self,
        stream: &StreamRuntimeId,
        scrutinee: &RuntimeExpr,
        arms: &[StreamMatchArm],
        budget: &mut usize,
        output: &mut FrameOutput,
    ) -> bool {
        let value = match self.evaluate_expr(scrutinee) {
            Ok(value) => value,
            Err(error) => {
                Self::diagnose_runtime_error(error, output);
                return true;
            }
        };
        for arm in arms {
            let Ok(Some(bindings)) = match_runtime_pattern(&arm.pattern, &value) else {
                continue;
            };
            let previous = self.fiber.env.clone();
            self.fiber.env.bind_all(bindings);
            let guard_matches = arm
                .guard
                .as_ref()
                .map_or(Ok(true), |guard| self.evaluate_bool(guard));
            if matches!(guard_matches, Ok(true)) {
                let should_continue = self.execute_stream_ops(stream, &arm.ops, budget, output);
                self.fiber.env = previous;
                return should_continue;
            }
            if let Err(error) = guard_matches {
                Self::diagnose_runtime_error(error, output);
            }
            self.fiber.env = previous;
        }
        true
    }

    fn close_stream_source(&mut self, source: &RuntimeExpr, output: &mut FrameOutput) {
        match self.evaluate_queue_target(source) {
            Ok(target) => {
                if let Some(source) = target.strip_prefix("source:") {
                    self.close_source(&SourceId(source.to_owned()), output);
                } else if let Some(stream) = target.strip_prefix("stream:") {
                    if let Some(state) = self
                        .fiber
                        .stream_states
                        .get_mut(&StreamRuntimeId(stream.to_owned()))
                    {
                        state.close();
                    }
                }
            }
            Err(error) => Self::diagnose_runtime_error(error, output),
        }
    }

    fn evaluate_queue_target(&mut self, expr: &RuntimeExpr) -> Result<String, RuntimeEvalError> {
        match self.evaluate_expr(expr)? {
            RuntimeValue::EntityRef(target) | RuntimeValue::String(target) => {
                if self
                    .fiber
                    .source_states
                    .contains_key(&SourceId(target.clone()))
                {
                    Ok(format!("source:{target}"))
                } else if self
                    .fiber
                    .stream_states
                    .contains_key(&StreamRuntimeId(target.clone()))
                {
                    Ok(format!("stream:{target}"))
                } else {
                    Ok(format!("source:{target}"))
                }
            }
            value => Err(RuntimeEvalError::ExpectedEntityRef(runtime_value_label(
                &value,
            ))),
        }
    }

    fn pop_queue_item(&mut self, key: &str) -> Option<String> {
        if let Some(source) = key.strip_prefix("source:") {
            return self
                .fiber
                .source_states
                .get_mut(&SourceId(source.to_owned()))
                .and_then(|state| state.queue.pop_front());
        }
        key.strip_prefix("stream:").and_then(|stream| {
            self.fiber
                .stream_states
                .get_mut(&StreamRuntimeId(stream.to_owned()))
                .and_then(|state| state.queue.pop_front())
        })
    }

    fn diagnose_runtime_error(error: impl std::fmt::Display, output: &mut FrameOutput) {
        output.diagnostics.push(RuntimeDiagnostic {
            message: error.to_string(),
        });
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

    // Keep the opcode dispatcher contiguous while the Phase 1 runtime surface is
    // still changing; extracting each arm now would obscure grammar coverage.
    #[allow(clippy::too_many_lines)]
    fn step_flow(&mut self, input: &FrameInput, output: &mut FrameOutput) {
        let (op, next) = if let Some(op) = self.fiber.pending_ops.pop_front() {
            (op, None)
        } else {
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
            (op, Some(next))
        };
        match op {
            FlowOp::Bind(bindings) => {
                self.fiber.env.bind_all(bindings);
                self.advance_if_needed(next);
            }
            FlowOp::Let { pattern, expr } => {
                self.evaluate_let(&pattern, &expr, output);
                self.advance_if_needed(next);
            }
            FlowOp::LetElse {
                pattern,
                expr,
                else_ops,
            } => {
                match self.evaluate_expr(&expr).and_then(|value| {
                    self.try_bind_pattern(&pattern, &value)
                        .map(|matched| (matched, value))
                }) {
                    Ok((true, _)) => self.advance_if_needed(next),
                    Ok((false, value)) => {
                        self.advance_if_needed(next);
                        self.push_ops(else_ops);
                        output.diagnostics.push(RuntimeDiagnostic {
                            message: format!(
                                "let-else pattern did not match {}",
                                runtime_value_label(&value)
                            ),
                        });
                    }
                    Err(error) => self.fail_eval(error, output),
                }
            }
            FlowOp::Dialogue { line, task_group } => {
                output.flow_events.push(FlowEvent::DialogueLine { line });
                let Some(group) = self.plan.line_task_groups.get(task_group) else {
                    self.fiber.status =
                        FlowFiberStatus::Failed(format!("missing line task group {task_group}"));
                    return;
                };
                output.merge(run_line_task_group_for_input(group, input));
                if !self.apply_control_effects(output) {
                    self.advance_if_needed(next);
                }
            }
            FlowOp::Choice { id, options } => {
                output
                    .flow_events
                    .push(FlowEvent::ChoicePresented { id: id.clone() });
                self.fiber.status = FlowFiberStatus::Choice(ChoiceState {
                    id,
                    options,
                    resume: next
                        .or_else(|| self.fiber.cursor.clone())
                        .unwrap_or_default(),
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
                    resume: next
                        .or_else(|| self.fiber.cursor.clone())
                        .unwrap_or_default(),
                });
            }
            FlowOp::If {
                condition,
                then_ops,
                else_ops,
            } => match self.evaluate_bool(&condition) {
                Ok(true) => {
                    self.advance_if_needed(next);
                    self.push_scoped_ops(then_ops);
                }
                Ok(false) => {
                    self.advance_if_needed(next);
                    self.push_scoped_ops(else_ops);
                }
                Err(error) => self.fail_eval(error, output),
            },
            FlowOp::IfLet {
                pattern,
                expr,
                guard,
                then_ops,
                else_ops,
            } => match self.evaluate_if_let(&pattern, &expr, guard.as_ref()) {
                Ok(Some(bindings)) => {
                    self.advance_if_needed(next);
                    self.push_scoped_ops_with_bindings(bindings, then_ops);
                }
                Ok(None) => {
                    self.advance_if_needed(next);
                    self.push_scoped_ops(else_ops);
                }
                Err(error) => self.fail_eval(error, output),
            },
            FlowOp::Match { scrutinee, arms } => match self.evaluate_match(&scrutinee, &arms) {
                Ok(Some((bindings, ops))) => {
                    self.advance_if_needed(next);
                    self.push_scoped_ops_with_bindings(bindings, ops);
                }
                Ok(None) => self.fail_eval(
                    RuntimeEvalError::PatternMismatch(expr_runtime_label(&scrutinee)),
                    output,
                ),
                Err(error) => self.fail_eval(error, output),
            },
            FlowOp::Loop { body } => {
                self.advance_if_needed(next);
                self.fiber.frames.push(RuntimeFrame {
                    kind: RuntimeFrameKind::Loop { body: body.clone() },
                });
                self.push_loop_iteration(body);
            }
            FlowOp::LoopNext { body } => {
                self.push_loop_iteration(body);
            }
            FlowOp::While { condition, body } => match self.evaluate_bool(&condition) {
                Ok(true) => {
                    self.advance_if_needed(next);
                    self.fiber.frames.push(RuntimeFrame {
                        kind: RuntimeFrameKind::While {
                            condition: condition.clone(),
                            body: body.clone(),
                        },
                    });
                    self.push_while_iteration(condition, body);
                }
                Ok(false) => self.advance_if_needed(next),
                Err(error) => self.fail_eval(error, output),
            },
            FlowOp::WhileNext { condition, body } => match self.evaluate_bool(&condition) {
                Ok(true) => {
                    self.push_while_iteration(condition, body);
                }
                Ok(false) => {
                    self.pop_loop_frame();
                }
                Err(error) => self.fail_eval(error, output),
            },
            FlowOp::WhileLet {
                pattern,
                expr,
                guard,
                body,
            } => match self.evaluate_if_let(&pattern, &expr, guard.as_ref()) {
                Ok(Some(bindings)) => {
                    self.advance_if_needed(next);
                    self.fiber.frames.push(RuntimeFrame {
                        kind: RuntimeFrameKind::WhileLet {
                            pattern: pattern.clone(),
                            expr: expr.clone(),
                            guard: guard.clone(),
                            body: body.clone(),
                        },
                    });
                    self.push_while_let_iteration(pattern, expr, guard, body, bindings);
                }
                Ok(None) => self.advance_if_needed(next),
                Err(error) => self.fail_eval(error, output),
            },
            FlowOp::WhileLetNext {
                pattern,
                expr,
                guard,
                body,
            } => match self.evaluate_if_let(&pattern, &expr, guard.as_ref()) {
                Ok(Some(bindings)) => {
                    self.push_while_let_iteration(pattern, expr, guard, body, bindings);
                }
                Ok(None) => {
                    self.pop_loop_frame();
                }
                Err(error) => self.fail_eval(error, output),
            },
            FlowOp::For {
                pattern,
                source,
                body,
            } => {
                self.advance_if_needed(next);
                match self.evaluate_expr(&source) {
                    Ok(RuntimeValue::List(items)) => {
                        let mut ops = Vec::new();
                        for item in items {
                            ops.push(FlowOp::EnterScope);
                            ops.push(FlowOp::Let {
                                pattern: pattern.clone(),
                                expr: RuntimeExpr::Value(item),
                            });
                            ops.extend(body.clone());
                            ops.push(FlowOp::ExitScope);
                        }
                        self.push_ops(ops);
                    }
                    Ok(value) => {
                        self.fail_eval(
                            RuntimeEvalError::ExpectedList(runtime_value_label(&value)),
                            output,
                        );
                    }
                    Err(error) => self.fail_eval(error, output),
                }
            }
            FlowOp::Scope(ops) => {
                self.advance_if_needed(next);
                self.push_scoped_ops(ops);
            }
            FlowOp::Break(expr) => {
                if let Some(expr) = expr {
                    match self.evaluate_expr(&expr) {
                        Ok(value) => output.diagnostics.push(RuntimeDiagnostic {
                            message: format!("break {}", runtime_value_label(&value)),
                        }),
                        Err(error) => self.fail_eval(error, output),
                    }
                }
                if self.break_nearest_loop() {
                    self.advance_if_needed(next);
                } else {
                    self.fail_eval(RuntimeEvalError::MisplacedLoopControl("break"), output);
                }
            }
            FlowOp::Continue => {
                if self.continue_nearest_loop(output) {
                    self.advance_if_needed(next);
                } else {
                    self.fail_eval(RuntimeEvalError::MisplacedLoopControl("continue"), output);
                }
            }
            FlowOp::Goto(target) => self.goto(target, output),
            FlowOp::GotoExpr(expr) => match self.evaluate_entity_target(&expr) {
                Ok(target) => self.goto(FlowRuntimeId(target), output),
                Err(error) => self.fail_eval(error, output),
            },
            FlowOp::Return(value) => self.return_value(value, output),
            FlowOp::ReturnExpr(expr) => match self.evaluate_expr(&expr) {
                Ok(value) => self.return_value(runtime_value_label(&value), output),
                Err(error) => self.fail_eval(error, output),
            },
            FlowOp::Effect(effect) => {
                output.line_effects.push(effect);
                if !self.apply_control_effects(output) {
                    self.advance_if_needed(next);
                }
            }
            FlowOp::EnterScope => {
                self.fiber.env.push_scope();
                self.fiber.frames.push(RuntimeFrame {
                    kind: RuntimeFrameKind::Scope,
                });
                self.advance_if_needed(next);
            }
            FlowOp::ExitScope => {
                self.pop_scope_frame();
                self.advance_if_needed(next);
            }
            FlowOp::Noop => {
                self.advance_if_needed(next);
            }
        }
    }

    fn advance_if_needed(&mut self, next: Option<FlowCursor>) {
        if let Some(next) = next {
            self.fiber.cursor = Some(next);
        }
    }

    fn push_ops(&mut self, ops: Vec<FlowOp>) {
        for op in ops.into_iter().rev() {
            self.fiber.pending_ops.push_front(op);
        }
    }

    fn scoped_ops(mut ops: Vec<FlowOp>) -> Vec<FlowOp> {
        if ops.is_empty() {
            return Vec::new();
        }
        ops.insert(0, FlowOp::EnterScope);
        ops.push(FlowOp::ExitScope);
        ops
    }

    fn push_scoped_ops(&mut self, ops: Vec<FlowOp>) {
        self.push_ops(Self::scoped_ops(ops));
    }

    fn push_scoped_ops_with_bindings(
        &mut self,
        bindings: Vec<RuntimeBinding>,
        mut ops: Vec<FlowOp>,
    ) {
        if bindings.is_empty() && ops.is_empty() {
            return;
        }
        ops.insert(0, FlowOp::Bind(bindings));
        self.push_scoped_ops(ops);
    }

    fn push_loop_iteration(&mut self, body: Vec<FlowOp>) {
        let mut ops = Self::scoped_ops(body.clone());
        ops.push(FlowOp::LoopNext { body });
        self.push_ops(ops);
    }

    fn push_while_iteration(&mut self, condition: RuntimeExpr, body: Vec<FlowOp>) {
        let mut ops = Self::scoped_ops(body.clone());
        ops.push(FlowOp::WhileNext { condition, body });
        self.push_ops(ops);
    }

    fn push_while_let_iteration(
        &mut self,
        pattern: RuntimePattern,
        expr: RuntimeExpr,
        guard: Option<RuntimeExpr>,
        body: Vec<FlowOp>,
        bindings: Vec<RuntimeBinding>,
    ) {
        let mut scoped = body.clone();
        scoped.insert(0, FlowOp::Bind(bindings));
        let mut ops = Self::scoped_ops(scoped);
        ops.push(FlowOp::WhileLetNext {
            pattern,
            expr,
            guard,
            body,
        });
        self.push_ops(ops);
    }

    fn pop_scope_frame(&mut self) {
        if matches!(
            self.fiber.frames.last(),
            Some(RuntimeFrame {
                kind: RuntimeFrameKind::Scope
            })
        ) {
            self.fiber.frames.pop();
            self.fiber.env.pop_scope();
        }
    }

    fn pop_scope_frames_until_loop(&mut self) {
        while matches!(
            self.fiber.frames.last(),
            Some(RuntimeFrame {
                kind: RuntimeFrameKind::Scope
            })
        ) {
            self.pop_scope_frame();
        }
    }

    fn pop_loop_frame(&mut self) -> Option<RuntimeFrameKind> {
        self.pop_scope_frames_until_loop();
        match self.fiber.frames.pop() {
            Some(RuntimeFrame {
                kind:
                    kind @ (RuntimeFrameKind::Loop { .. }
                    | RuntimeFrameKind::While { .. }
                    | RuntimeFrameKind::WhileLet { .. }),
            }) => Some(kind),
            _ => None,
        }
    }

    fn discard_pending_until_loop_next(&mut self) {
        while let Some(op) = self.fiber.pending_ops.pop_front() {
            if matches!(
                op,
                FlowOp::LoopNext { .. } | FlowOp::WhileNext { .. } | FlowOp::WhileLetNext { .. }
            ) {
                break;
            }
        }
    }

    fn break_nearest_loop(&mut self) -> bool {
        self.discard_pending_until_loop_next();
        self.pop_loop_frame().is_some()
    }

    fn continue_nearest_loop(&mut self, output: &mut FrameOutput) -> bool {
        self.pop_scope_frames_until_loop();
        self.discard_pending_until_loop_next();
        let Some(kind) = self.fiber.frames.last().map(|frame| frame.kind.clone()) else {
            return false;
        };
        match kind {
            RuntimeFrameKind::Loop { body } => self.push_loop_iteration(body),
            RuntimeFrameKind::While { condition, body } => {
                self.push_ops(vec![FlowOp::WhileNext { condition, body }]);
            }
            RuntimeFrameKind::WhileLet {
                pattern,
                expr,
                guard,
                body,
            } => {
                self.push_ops(vec![FlowOp::WhileLetNext {
                    pattern,
                    expr,
                    guard,
                    body,
                }]);
            }
            RuntimeFrameKind::Scope => {
                self.fail_eval(RuntimeEvalError::MisplacedLoopControl("continue"), output);
                return false;
            }
        }
        true
    }

    fn evaluate_let(
        &mut self,
        pattern: &RuntimePattern,
        expr: &RuntimeExpr,
        output: &mut FrameOutput,
    ) {
        match self.evaluate_expr(expr).and_then(|value| {
            self.try_bind_pattern(pattern, &value)
                .map(|matched| (matched, value))
        }) {
            Ok((true, _)) => {}
            Ok((false, value)) => {
                self.fail_eval(
                    RuntimeEvalError::PatternMismatch(runtime_value_label(&value)),
                    output,
                );
            }
            Err(error) => self.fail_eval(error, output),
        }
    }

    fn evaluate_if_let(
        &mut self,
        pattern: &RuntimePattern,
        expr: &RuntimeExpr,
        guard: Option<&RuntimeExpr>,
    ) -> Result<Option<Vec<RuntimeBinding>>, RuntimeEvalError> {
        let value = self.evaluate_expr(expr)?;
        let Some(bindings) = match_runtime_pattern(pattern, &value)? else {
            return Ok(None);
        };
        if let Some(guard) = guard {
            let previous = self.fiber.env.clone();
            self.fiber.env.bind_all(bindings.clone());
            let matched = self.evaluate_bool(guard);
            self.fiber.env = previous;
            let matched = matched?;
            if !matched {
                return Ok(None);
            }
        }
        Ok(Some(bindings))
    }

    fn evaluate_match(
        &mut self,
        scrutinee: &RuntimeExpr,
        arms: &[RuntimeMatchArm],
    ) -> Result<RuntimeMatchSelection, RuntimeEvalError> {
        let value = self.evaluate_expr(scrutinee)?;
        for arm in arms {
            let Some(bindings) = match_runtime_pattern(&arm.pattern, &value)? else {
                continue;
            };
            let previous = self.fiber.env.clone();
            self.fiber.env.bind_all(bindings.clone());
            if let Some(guard) = arm.guard.as_ref()
                && !match self.evaluate_bool(guard) {
                    Ok(matched) => matched,
                    Err(error) => {
                        self.fiber.env = previous;
                        return Err(error);
                    }
                }
            {
                self.fiber.env = previous;
                continue;
            }
            self.fiber.env = previous;
            return Ok(Some((bindings, arm.ops.clone())));
        }
        Ok(None)
    }

    fn evaluate_expr(&mut self, expr: &RuntimeExpr) -> Result<RuntimeValue, RuntimeEvalError> {
        match expr {
            RuntimeExpr::Value(value) => Ok(value.clone()),
            RuntimeExpr::Local(name) => self
                .fiber
                .env
                .get(name)
                .cloned()
                .ok_or_else(|| RuntimeEvalError::UnknownBinding(name.clone())),
            RuntimeExpr::EntityRef(target) => Ok(RuntimeValue::EntityRef(target.clone())),
            RuntimeExpr::Tuple(items) => items
                .iter()
                .map(|item| self.evaluate_expr(item))
                .collect::<Result<Vec<_>, _>>()
                .map(RuntimeValue::Tuple),
            RuntimeExpr::List(items) => items
                .iter()
                .map(|item| self.evaluate_expr(item))
                .collect::<Result<Vec<_>, _>>()
                .map(RuntimeValue::List),
            RuntimeExpr::Record(fields) => fields
                .iter()
                .map(|field| {
                    Ok(RuntimeFieldValue {
                        name: field.name.clone(),
                        value: self.evaluate_expr(&field.value)?,
                    })
                })
                .collect::<Result<Vec<_>, _>>()
                .map(RuntimeValue::Record),
            RuntimeExpr::Variant {
                path,
                name,
                payload,
            } => Ok(RuntimeValue::Variant {
                path: path.clone(),
                name: name.clone(),
                payload: payload
                    .as_ref()
                    .map(|expr| self.evaluate_expr(expr).map(Box::new))
                    .transpose()?,
            }),
            RuntimeExpr::Field { target, field } => {
                let value = self.evaluate_expr(target)?;
                match value {
                    RuntimeValue::Record(fields) => fields
                        .into_iter()
                        .find(|candidate| candidate.name == *field)
                        .map(|field| field.value)
                        .ok_or_else(|| RuntimeEvalError::MissingField {
                            field: field.clone(),
                            value: "record".to_owned(),
                        }),
                    value => Err(RuntimeEvalError::MissingField {
                        field: field.clone(),
                        value: runtime_value_label(&value),
                    }),
                }
            }
            RuntimeExpr::Unary { op, expr } => {
                let value = self.evaluate_expr(expr)?;
                evaluate_unary(*op, value)
            }
            RuntimeExpr::Binary { lhs, op, rhs } => {
                let lhs = self.evaluate_expr(lhs)?;
                let rhs = self.evaluate_expr(rhs)?;
                evaluate_binary(lhs, *op, rhs)
            }
            RuntimeExpr::If {
                condition,
                then_expr,
                else_expr,
            } => {
                if self.evaluate_bool(condition)? {
                    self.evaluate_expr(then_expr)
                } else {
                    self.evaluate_expr(else_expr)
                }
            }
            RuntimeExpr::Match { scrutinee, arms } => self.evaluate_match_expr(scrutinee, arms),
        }
    }

    fn evaluate_match_expr(
        &mut self,
        scrutinee: &RuntimeExpr,
        arms: &[RuntimeExprMatchArm],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let value = self.evaluate_expr(scrutinee)?;
        for arm in arms {
            let Some(bindings) = match_runtime_pattern(&arm.pattern, &value)? else {
                continue;
            };
            let previous = self.fiber.env.clone();
            self.fiber.env.bind_all(bindings);
            if let Some(guard) = arm.guard.as_ref()
                && !match self.evaluate_bool(guard) {
                    Ok(matched) => matched,
                    Err(error) => {
                        self.fiber.env = previous;
                        return Err(error);
                    }
                }
            {
                self.fiber.env = previous;
                continue;
            }
            let result = self.evaluate_expr(&arm.value);
            self.fiber.env = previous;
            return result;
        }
        Err(RuntimeEvalError::PatternMismatch(runtime_value_label(
            &value,
        )))
    }

    fn evaluate_bool(&mut self, expr: &RuntimeExpr) -> Result<bool, RuntimeEvalError> {
        match self.evaluate_expr(expr)? {
            RuntimeValue::Bool(value) => Ok(value),
            value => Err(RuntimeEvalError::ExpectedBool(runtime_value_label(&value))),
        }
    }

    fn evaluate_entity_target(&mut self, expr: &RuntimeExpr) -> Result<String, RuntimeEvalError> {
        match self.evaluate_expr(expr)? {
            RuntimeValue::EntityRef(target) | RuntimeValue::String(target) => Ok(target),
            value => Err(RuntimeEvalError::ExpectedEntityRef(runtime_value_label(
                &value,
            ))),
        }
    }

    fn try_bind_pattern(
        &mut self,
        pattern: &RuntimePattern,
        value: &RuntimeValue,
    ) -> Result<bool, RuntimeEvalError> {
        let Some(bindings) = match_runtime_pattern(pattern, value)? else {
            return Ok(false);
        };
        self.fiber.env.bind_all(bindings);
        Ok(true)
    }

    fn fail_eval(&mut self, error: impl std::fmt::Display, output: &mut FrameOutput) {
        let message = error.to_string();
        self.fiber.status = FlowFiberStatus::Failed(message.clone());
        output.diagnostics.push(RuntimeDiagnostic { message });
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
        self.fiber.pending_ops.clear();
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
        self.fiber.pending_ops.clear();
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
        self.source_events.extend(other.source_events);
        self.stream_events.extend(other.stream_events);
        self.source_close_requests
            .extend(other.source_close_requests);
    }
}

enum FlowControl {
    Goto(String),
    Return(String),
    Failed(String),
}

fn source_handler_match<'a>(
    handler: &'a SourceHandlerPlan,
    event: &SourceEventKind<String, String>,
) -> Option<(Vec<RuntimeBinding>, &'a [SourceOp])> {
    match (handler, event) {
        (SourceHandlerPlan::Item { pattern, ops }, SourceEventKind::Item(item))
        | (SourceHandlerPlan::Error { pattern, ops }, SourceEventKind::Error(item))
        | (SourceHandlerPlan::Progress { pattern, ops }, SourceEventKind::Progress(item)) => {
            let bindings = match_runtime_pattern(pattern, &RuntimeValue::String(item.clone()))
                .ok()
                .flatten()?;
            Some((bindings, ops))
        }
        (SourceHandlerPlan::Disconnected { ops }, SourceEventKind::Disconnected)
        | (SourceHandlerPlan::PermissionRevoked { ops }, SourceEventKind::PermissionRevoked)
        | (SourceHandlerPlan::End { ops }, SourceEventKind::End) => Some((Vec::new(), ops)),
        _ => None,
    }
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

fn match_runtime_pattern(
    pattern: &RuntimePattern,
    value: &RuntimeValue,
) -> Result<Option<Vec<RuntimeBinding>>, RuntimeEvalError> {
    let mut bindings = Vec::new();
    if collect_pattern_bindings(pattern, value, &mut bindings)? {
        reject_duplicate_bindings(&bindings)?;
        Ok(Some(bindings))
    } else {
        Ok(None)
    }
}

fn reject_duplicate_bindings(bindings: &[RuntimeBinding]) -> Result<(), RuntimeEvalError> {
    let mut seen = BTreeSet::<&str>::new();
    for binding in bindings {
        if !seen.insert(binding.name.as_str()) {
            return Err(RuntimeEvalError::DuplicateBinding(binding.name.clone()));
        }
    }
    Ok(())
}

fn collect_pattern_bindings(
    pattern: &RuntimePattern,
    value: &RuntimeValue,
    bindings: &mut Vec<RuntimeBinding>,
) -> Result<bool, RuntimeEvalError> {
    match pattern {
        RuntimePattern::Ident(name)
        | RuntimePattern::MutIdent(name)
        | RuntimePattern::Typed { name, .. } => {
            bindings.push(RuntimeBinding {
                name: name.clone(),
                value: value.clone(),
            });
            Ok(true)
        }
        RuntimePattern::Discard => Ok(true),
        RuntimePattern::Literal(expected) => Ok(expected == value),
        RuntimePattern::Entity(expected) => {
            Ok(matches!(value, RuntimeValue::EntityRef(actual) if actual == expected))
        }
        RuntimePattern::Tuple(patterns) => {
            let RuntimeValue::Tuple(values) = value else {
                return Ok(false);
            };
            if patterns.len() != values.len() {
                return Ok(false);
            }
            collect_pattern_list(patterns, values, bindings)
        }
        RuntimePattern::Record { fields, rest, .. } => {
            let RuntimeValue::Record(values) = value else {
                return Ok(false);
            };
            if !rest && fields.len() != values.len() {
                return Ok(false);
            }
            for field in fields {
                let Some(value_field) =
                    values.iter().find(|candidate| candidate.name == field.name)
                else {
                    return Ok(false);
                };
                if !collect_pattern_bindings(&field.pattern, &value_field.value, bindings)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        RuntimePattern::List { items, rest } => {
            let RuntimeValue::List(values) = value else {
                return Ok(false);
            };
            if rest.is_none() && items.len() != values.len() {
                return Ok(false);
            }
            if rest.is_some() && items.len() > values.len() {
                return Ok(false);
            }
            if !collect_pattern_list(items, &values[..items.len()], bindings)? {
                return Ok(false);
            }
            if let Some(name) = rest {
                bindings.push(RuntimeBinding {
                    name: name.clone(),
                    value: RuntimeValue::List(values[items.len()..].to_vec()),
                });
            }
            Ok(true)
        }
        RuntimePattern::Variant {
            path,
            name,
            payload,
        } => {
            let RuntimeValue::Variant {
                path: actual_path,
                name: actual_name,
                payload: actual_payload,
            } = value
            else {
                return Ok(false);
            };
            if path != actual_path || name != actual_name {
                return Ok(false);
            }
            match (payload, actual_payload) {
                (Some(pattern), Some(value)) => collect_pattern_bindings(pattern, value, bindings),
                (None, None | Some(_)) => Ok(true),
                (Some(_), None) => Ok(false),
            }
        }
        RuntimePattern::Whole { name, pattern } => {
            if !collect_pattern_bindings(pattern, value, bindings)? {
                return Ok(false);
            }
            bindings.push(RuntimeBinding {
                name: name.clone(),
                value: value.clone(),
            });
            Ok(true)
        }
    }
}

fn collect_pattern_list(
    patterns: &[RuntimePattern],
    values: &[RuntimeValue],
    bindings: &mut Vec<RuntimeBinding>,
) -> Result<bool, RuntimeEvalError> {
    for (pattern, value) in patterns.iter().zip(values) {
        if !collect_pattern_bindings(pattern, value, bindings)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn evaluate_unary(
    op: RuntimeUnaryOp,
    value: RuntimeValue,
) -> Result<RuntimeValue, RuntimeEvalError> {
    match (op, value) {
        (RuntimeUnaryOp::Not, RuntimeValue::Bool(value)) => Ok(RuntimeValue::Bool(!value)),
        (RuntimeUnaryOp::Neg, RuntimeValue::Int(value)) => Ok(RuntimeValue::Int(-value)),
        (op, value) => Err(RuntimeEvalError::UnsupportedUnary {
            op: runtime_unary_op_label(op),
            value: runtime_value_label(&value),
        }),
    }
}

fn evaluate_binary(
    lhs: RuntimeValue,
    op: RuntimeBinaryOp,
    rhs: RuntimeValue,
) -> Result<RuntimeValue, RuntimeEvalError> {
    match op {
        RuntimeBinaryOp::Eq => Ok(RuntimeValue::Bool(lhs == rhs)),
        RuntimeBinaryOp::Ne => Ok(RuntimeValue::Bool(lhs != rhs)),
        RuntimeBinaryOp::And => match (lhs, rhs) {
            (RuntimeValue::Bool(lhs), RuntimeValue::Bool(rhs)) => {
                Ok(RuntimeValue::Bool(lhs && rhs))
            }
            (lhs, rhs) => unsupported_binary(op, &lhs, &rhs),
        },
        RuntimeBinaryOp::Or => match (lhs, rhs) {
            (RuntimeValue::Bool(lhs), RuntimeValue::Bool(rhs)) => {
                Ok(RuntimeValue::Bool(lhs || rhs))
            }
            (lhs, rhs) => unsupported_binary(op, &lhs, &rhs),
        },
        RuntimeBinaryOp::Lt | RuntimeBinaryOp::Le | RuntimeBinaryOp::Gt | RuntimeBinaryOp::Ge => {
            match (lhs, rhs) {
                (RuntimeValue::Int(lhs), RuntimeValue::Int(rhs)) => {
                    Ok(RuntimeValue::Bool(match op {
                        RuntimeBinaryOp::Lt => lhs < rhs,
                        RuntimeBinaryOp::Le => lhs <= rhs,
                        RuntimeBinaryOp::Gt => lhs > rhs,
                        RuntimeBinaryOp::Ge => lhs >= rhs,
                        _ => unreachable!(),
                    }))
                }
                (lhs, rhs) => unsupported_binary(op, &lhs, &rhs),
            }
        }
        RuntimeBinaryOp::Add
        | RuntimeBinaryOp::Sub
        | RuntimeBinaryOp::Mul
        | RuntimeBinaryOp::Div => match (lhs, rhs) {
            (RuntimeValue::Int(lhs), RuntimeValue::Int(rhs)) => Ok(RuntimeValue::Int(match op {
                RuntimeBinaryOp::Add => lhs + rhs,
                RuntimeBinaryOp::Sub => lhs - rhs,
                RuntimeBinaryOp::Mul => lhs * rhs,
                RuntimeBinaryOp::Div => lhs / rhs,
                _ => unreachable!(),
            })),
            (lhs, rhs) => unsupported_binary(op, &lhs, &rhs),
        },
    }
}

fn unsupported_binary(
    op: RuntimeBinaryOp,
    lhs: &RuntimeValue,
    rhs: &RuntimeValue,
) -> Result<RuntimeValue, RuntimeEvalError> {
    Err(RuntimeEvalError::UnsupportedBinary {
        op: runtime_binary_op_label(op),
        lhs: runtime_value_label(lhs),
        rhs: runtime_value_label(rhs),
    })
}

fn runtime_unary_op_label(op: RuntimeUnaryOp) -> &'static str {
    match op {
        RuntimeUnaryOp::Not => "!",
        RuntimeUnaryOp::Neg => "-",
    }
}

fn runtime_binary_op_label(op: RuntimeBinaryOp) -> &'static str {
    match op {
        RuntimeBinaryOp::Eq => "==",
        RuntimeBinaryOp::Ne => "!=",
        RuntimeBinaryOp::Lt => "<",
        RuntimeBinaryOp::Le => "<=",
        RuntimeBinaryOp::Gt => ">",
        RuntimeBinaryOp::Ge => ">=",
        RuntimeBinaryOp::Add => "+",
        RuntimeBinaryOp::Sub => "-",
        RuntimeBinaryOp::Mul => "*",
        RuntimeBinaryOp::Div => "/",
        RuntimeBinaryOp::And => "&&",
        RuntimeBinaryOp::Or => "||",
    }
}

fn expr_runtime_label(expr: &RuntimeExpr) -> String {
    match expr {
        RuntimeExpr::Value(value) => runtime_value_label(value),
        RuntimeExpr::Local(name) => name.clone(),
        RuntimeExpr::EntityRef(target) => format!("@{target}"),
        RuntimeExpr::Tuple(items) => format!("tuple/{}", items.len()),
        RuntimeExpr::List(items) => format!("list/{}", items.len()),
        RuntimeExpr::Record(fields) => format!("record/{}", fields.len()),
        RuntimeExpr::Variant { name, .. } => format!(".{name}"),
        RuntimeExpr::Field { field, .. } => format!(".{field}"),
        RuntimeExpr::Unary { op, .. } => runtime_unary_op_label(*op).to_owned(),
        RuntimeExpr::Binary { op, .. } => runtime_binary_op_label(*op).to_owned(),
        RuntimeExpr::If { .. } => "if".to_owned(),
        RuntimeExpr::Match { .. } => "match".to_owned(),
    }
}

fn runtime_value_label(value: &RuntimeValue) -> String {
    match value {
        RuntimeValue::Unit => "()".to_owned(),
        RuntimeValue::Bool(value) => value.to_string(),
        RuntimeValue::Int(value) => value.to_string(),
        RuntimeValue::Float(value)
        | RuntimeValue::String(value)
        | RuntimeValue::EntityRef(value) => value.clone(),
        RuntimeValue::Char(value) => value.to_string(),
        RuntimeValue::Duration(value) => format!("{}ns", value.as_nanos()),
        RuntimeValue::Tuple(values) => format!("tuple/{}", values.len()),
        RuntimeValue::List(values) => format!("list/{}", values.len()),
        RuntimeValue::Record(fields) => format!("record/{}", fields.len()),
        RuntimeValue::Variant { name, payload, .. } => {
            if payload.is_some() {
                format!(".{name}(...)")
            } else {
                format!(".{name}")
            }
        }
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
mod tests;
