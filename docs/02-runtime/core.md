# Sans I/O Core

`arcweft-core` は副作用を実行しない。入力を受け取り、次の状態と要求を返す。

Core が扱う program、bytecode、manifest reference、save snapshot、diagnostic、trace はすべて pure data。Core は path を開かず、filesystem、network、wall-clock、GPU/audio/device handle、Wasm runtime、Cranelift runtime を保持しない。外部 adapter は bytes/string へ serialize された bundle や task result を `FrameInput` / `TaskEvent` として渡す。

```rust
pub struct Engine {
    plan: RuntimePlan,
    fiber: FlowFiber,
}

pub struct RuntimePlan {
    pub entry_flow: Option<FlowRuntimeId>,
    pub flows: Vec<RuntimeFlow>,
    pub line_task_groups: Vec<LineTaskGroup>,
    pub stream_plans: Vec<StreamPlan>,
    pub source_plans: Vec<SourcePlan>,
}

pub struct RuntimeFlow {
    pub id: FlowRuntimeId,
    pub ops: Vec<FlowOp>,
}

pub enum FlowOp {
    Let { pattern: RuntimePattern, expr: RuntimeExpr },
    LetElse { pattern: RuntimePattern, expr: RuntimeExpr, else_ops: Vec<FlowOp> },
    Dialogue { line: RuntimeLineId, task_group: usize },
    Choice { id: Option<String>, options: Vec<ChoiceRuntimeOption> },
    Await { target: AwaitTarget, pending: Vec<LineEffectRequest> },
    If { condition: RuntimeExpr, then_ops: Vec<FlowOp>, else_ops: Vec<FlowOp> },
    IfLet { pattern: RuntimePattern, expr: RuntimeExpr, guard: Option<RuntimeExpr>, then_ops: Vec<FlowOp>, else_ops: Vec<FlowOp> },
    Match { scrutinee: RuntimeExpr, arms: Vec<RuntimeMatchArm> },
    Loop { body: Vec<FlowOp> },
    LetLoop { pattern: RuntimePattern, body: Vec<FlowOp> },
    LoopNext { body: Vec<FlowOp> },
    While { condition: RuntimeExpr, body: Vec<FlowOp> },
    WhileNext { condition: RuntimeExpr, body: Vec<FlowOp> },
    WhileLet { pattern: RuntimePattern, expr: RuntimeExpr, guard: Option<RuntimeExpr>, body: Vec<FlowOp> },
    WhileLetNext { pattern: RuntimePattern, expr: RuntimeExpr, guard: Option<RuntimeExpr>, body: Vec<FlowOp> },
    For { pattern: RuntimePattern, source: RuntimeExpr, body: Vec<FlowOp> },
    Scope(Vec<FlowOp>),
    LetScope { pattern: RuntimePattern, ops: Vec<FlowOp>, value: RuntimeExpr },
    Break(Option<RuntimeExpr>),
    Continue,
    Goto(FlowRuntimeId),
    GotoExpr(RuntimeExpr),
    Return(String),
    ReturnExpr(RuntimeExpr),
    Effect(LineEffectRequest),
    EnterScope,
    ExitScope,
    ExitScopeBind { pattern: RuntimePattern, expr: RuntimeExpr },
    Noop,
}

pub struct FlowFiber {
    pub line_cursor: usize,
    pub cursor: Option<FlowCursor>,
    pub pending_ops: VecDeque<FlowOp>,
    pub frames: Vec<RuntimeFrame>,
    pub env: RuntimeEnv,
    pub observations: RuntimeObservationState,
    pub source_states: BTreeMap<SourceId, SourceRuntimeState>,
    pub stream_states: BTreeMap<StreamRuntimeId, StreamRuntimeState>,
    pub status: FlowFiberStatus,
}

pub enum RuntimeFrameKind {
    Scope,
    Loop { body: Vec<FlowOp>, result: Option<RuntimePattern> },
    While { condition: RuntimeExpr, body: Vec<FlowOp> },
    WhileLet {
        pattern: RuntimePattern,
        expr: RuntimeExpr,
        guard: Option<RuntimeExpr>,
        body: Vec<FlowOp>,
    },
}

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
```

Phase 2.0 の `Engine` は headless structured-control-flow runtime slice であり、まだ完全な story VM ではない。
`Engine::step(FrameInput) -> FrameOutput` は lowered flow op を 1 frame に
最大 1 つ進める。`Dialogue` は line task group を実行し、`Choice` は入力待ち、
`Await` は `TaskSpec` を出して `TaskEvent` で再開する。`Let` / `LetElse` /
`If` / `IfLet` / `Match` / `Loop` / `LetLoop` / `While` / `WhileLet` / `For` / `LetScope` は
`RuntimeValue` / `RuntimeExpr` / `RuntimePattern` と `RuntimeEnv` で評価され、
選択された block は `pending_ops` queue に積まれて次 frame 以降で実行される。
`FlowFiber::frames` は lexical scope と loop continuation を明示的に保持し、
`break` / `continue` が body 内の残り op と scope-local binding を破棄して
最も近い loop/while/while-let continuation へ移るために使われる。
`let name = scope { ... }` は final expression を scope 内で評価してから外側の
pattern に束縛し、`let name = loop { break expr }` は `break expr` の値を
loop result pattern に束縛する。
`FrameInput::external_values` は ambient input として root runtime scope に束縛される。
branch / match / while-let pattern binding は選択された block scope にだけ束縛され、
guard 評価後や block 終了後には外側へ漏れない。
`Goto` / `GotoExpr` / `Return` / `ReturnExpr` は `FlowFiber` の cursor/status を更新する。
`FrameInput.source_events` は replay-stable order に正規化され、`SourcePlan` handler と
`StreamPlan` によって queue state と `StreamEvent` に反映される。`FlowFiber.observations`
は emitted log / signal / metric / event を累積し、CLI/LSP/test/replay tooling が
JSON で観測できる。
実 thread、wall-clock、renderer、audio、device、filesystem は adapter 側の責務である。
`ScopeExit::Completed | Cancelled | Failed` は outcome-guarded `defer` stack の
選択に使う。

## EffectRequest

`EffectRequest` は実行済みの副作用ではなく、host adapter への要求である。たとえば `SaveCheckpoint` は checkpoint data を作る要求であり、実際の file write、compression、encryption、cloud sync は build/player/CLI adapter が行う。

```rust
pub enum EffectRequest {
    EnsureAsset(AssetRequest),
    EnsureShader(ShaderRequest),
    EnsureAudio(AudioRequest),
    EnsureBgm(BgmRequest),
    SynthesizeSpeech(TtsRequest),
    CallWasm(WasmCallRequest),
    StartActivity(ActivityStartRequest),
    EnsureHtmlPanel(HtmlPanelSpec),
    Audio(AudioCommand),
    SaveCheckpoint(SaveRequest),
    Log(StructuredLog),
    Signal(SignalUpdate),
}
```

## Dialogue line task groups

A dialogue `with` block lowers to a line-scoped task group. This model stays
Sans I/O: `thread name:` creates a child task in VM/runtime data, not an OS
thread. Presentation, audio, wait, and signal work is emitted as effect requests
for adapters to perform at frame boundaries.

```rust
pub struct LineTaskGroup {
    pub root: LineTaskScope,
    pub options: Vec<LineOptionRequest>,
    pub bindings: Vec<LineBindingRequest>,
    pub out: Vec<LineOutRequest>,
    pub cancel_rules: Vec<LineCancelRuleRequest>,
    pub memo: Vec<LineMemoRequest>,
    pub assertions: Vec<LineAssertionRequest>,
    pub cleanup: LineCleanupPolicy,
}

pub struct LineTaskScope {
    pub node: LineTaskNode,
    pub defer_stack: Vec<Vec<LineEffectRequest>>,
    pub completed_defer_stack: Vec<Vec<LineEffectRequest>>,
    pub cancelled_defer_stack: Vec<Vec<LineEffectRequest>>,
    pub failed_defer_stack: Vec<Vec<LineEffectRequest>>,
}

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

pub enum LineTaskTrigger {
    Immediate,
    Mark(String),
    Delay(LogicalDuration),
}

pub enum LineEffectRequest {
    RegisterHandle { key: String, handle: String },
    DropHandle { key: String },
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
    Ensure { condition: String, message: String },
    Close(String),
    Select(String),
    Break { label: Option<String>, value: Option<String> },
    Continue { label: Option<String> },
}
```

Current Phase 2.0 lowering maps checked HIR dialogue plans into this data model:

```text
init statements             -> LineTaskGroup.root.node effects
defer in init/line scope    -> LineTaskGroup.root defer stacks
thread name { ... }         -> LineTaskNode::Child with trigger Immediate
defer in thread/on handler  -> child LineTaskScope defer stacks
defer on completed          -> current scope completed_defer_stack
defer on cancelled          -> current scope cancelled_defer_stack
defer on failed             -> current scope failed_defer_stack
start { ... }               -> LineTaskNode::Start
together { ... }            -> LineTaskNode::Parallel(JoinAll)
at(0.35s) { ... }           -> child task with trigger Delay(0.35s)
on .mark { ... }            -> child task with trigger Mark(".mark")
wait mark .x                -> LineEffectRequest::WaitMark(".x")
wait 0.35s                  -> LineEffectRequest::Wait(...)
'line.key <- expr           -> LineEffectRequest::RegisterHandle
'line.key |> drop           -> LineEffectRequest::DropHandle
call-like expression stmt   -> LineEffectRequest::Call
log.info(...)               -> LineEffectRequest::Log
signal.set(target, value)   -> LineEffectRequest::SignalWrite
metric.set(target, value)   -> LineEffectRequest::MetricWrite
event.emit(Event, fields)   -> LineEffectRequest::EmitEvent
out expr                    -> LineEffectRequest::Out and LineTaskGroup.out
cancel on ... { ... }       -> LineTaskGroup.cancel_rules
memo name(...)              -> LineTaskGroup.memo
assert expr                 -> LineTaskGroup.assertions
```

`yield` is not a line effect. It lowers only through stream/source generation
plans; a dialogue line plan must use `out` for line-scope values.

`on` and `at` do not lower by inserting synthetic `wait` effects. They become
task triggers so scheduling and replay can reason about when a child task starts.
`together` preserves the parallel boundary and the lowering pass rejects
obvious deterministic conflicts such as two children writing the same signal or
line `out` value unless the effect category is append-only, such as structured
logs or event emission.

The minimal runtime spine treats child tasks as Sans I/O requests. When a child
trigger is ready, the engine emits a `TaskSpec` and also exposes the child scope
body as deterministic effect data for tests and future adapters. Native workers,
cooperative jobs, and web workers consume those requests outside `arcweft-core`.

Flow-level execution now has a small Sans I/O vertical slice. HIR runtime
lowering converts checked flows to `RuntimeFlow` / `FlowOp` data, while dialogue
line plans continue to lower to `LineTaskGroup` and are referenced by index from
`FlowOp::Dialogue`.

```text
dialogue line              -> FlowOp::Dialogue + LineTaskGroup
choice                     -> FlowOp::Choice + ChoiceRuntimeOption list
await ... with             -> FlowOp::Await + pending LineEffectRequest list
let / let else             -> FlowOp::Let / FlowOp::LetElse
if / if let / match        -> structured FlowOp nodes with RuntimePattern arms
loop / while / while let   -> structured FlowOp nodes
for PAT in EXPR            -> FlowOp::For over RuntimeValue::List
scope / bare block         -> FlowOp::Scope
goto @flow.x / goto route  -> FlowOp::GotoExpr
return expr                -> FlowOp::ReturnExpr
out / log / signal / call  -> FlowOp::Effect or line effect
cancel on input .SkipLine  -> line cancel rule selected from FrameInput
```

This slice is intentionally strict: runtime lowering errors on unsupported flow
items instead of converting them to `Noop`. Expression values are still stable
labels in the runtime data; the later typed VM/HIR evaluator should replace
those labels with typed value nodes.

Raw line-plan statements fail lowering. They are not reparsed, silently accepted,
or dropped from the runtime plan. Phase 2.0 keeps expression payloads as stable
labels inside Sans I/O data; later HIR execution work should replace those labels
with typed expression/runtime nodes without changing the effect categories.

`defer` is not thread-specific syntax. It registers cleanup on the current
runtime scope. In the Phase 2.0 line-plan model, the line/root scope, child
thread scopes, and event-handler scopes each have a cleanup stack. A bare
`defer` must run when its owning scope exits, including normal completion,
early control transfer, line cancellation, and child-task cancellation.
Outcome-guarded forms `defer on completed`, `defer on cancelled`, and
`defer on failed` are kept in separate deterministic stacks so adapters can run
only the cleanup appropriate for the scope exit.

Lifetime registry paths are typed static keys, not stringly dynamic maps.
The core model keeps the data Sans I/O; host backends receive deterministic
effect requests instead of direct mutation.

```text
'frame
'tick
'cue <= 'line <= 'scene <= 'flow <= 'session <= 'global
'persistent
```

`'persistent` is storage-backed and is checked separately from ordinary runtime
memory. Reads from an upper scope are allowed when the key is guaranteed, or
when the access is optional such as `'flow.flags?`. Writes to upper scopes lower
to replayable state-update events and require explicit capabilities.

```awft
flow @flow.opening opening(state: GameState)
effects { state.write('flow), state.write('global) }
{
    alice[見たことにする。[mark .seen][p]]
    with {
        on .seen {
            'flow.flags.seen_alice_intro <- true
            'global.settings.skip_seen <- true
        }
    }
}
```

Line lifetime registry paths such as `'line.focus` are owned by the line scope.
Guaranteed keys can be read directly; optional reads use `'line.focus?`. Drop
operations remove registered values and run their adapter cleanup policy through
emitted requests. Lower-scope keys such as `'line.*` are not available outside
their active scope or across a thread boundary unless an explicit move/share or
detach operation makes the capture safe.

The checker treats lifetime operations as intrinsics with typestate effects:

```text
drop
drop_optional
on_drop
expose
share
detach
promote
promote_unchecked
clone_owned
```

Safe `promote('target)` requires owned data, no shorter lifetime references, and
replay-safe serialization when the target is `session`, `global`, or
`persistent`. `promote_unchecked` is an unsafe-like lifetime proof escape hatch:
it requires an explicit `unsafe lifetime` region, a reason string, and a project
capability. It cannot bypass determinism or the Sans I/O boundary.

Line-level cleanup is represented by line-scope `defer`; there is no separate
line-plan cleanup keyword. Bare `defer` runs after line-scoped child tasks are
cancelled/joined and their defer stacks have run, but before any remaining
automatic line-registry drops. Flow-level threads, line-plan threads, handler
tasks, and ordinary lexical runtime scopes share the same cleanup model. The VM
must treat cancellation as scope exit for cleanup: child task cancellation first
unwinds that task's defer stack, then the line task group runs the matching
line-scope defer stacks.

## Determinism

- wall-clock を state に入れない。
- random は seeded RNG capability。
- task 完了は frame boundary で正規化。
- replay は `FrameInput` 列と task/audio/ui result を記録。

## Flow fiber

Flow/dialogue/choice/`Need`/effect emission は bytecode VM が意味論の正本として処理する。Cranelift JIT や generated Rust/Wasm は pure function の最適化または release packaging backend であり、FlowFiber の control transfer や awaiting semantics を置き換えない。

```rust
pub enum FlowFiber {
    Running { pc: ProgramCounter, stack: ValueStack, locals: LocalSlots },
    Waiting { await_target: AwaitTarget, continuation: ContinuationId, locals: LocalSlots },
    Done,
    Failed(RuntimeDiagnostic),
}
```

`await with` は FlowFiber の `Waiting` へ lowering する。



## Hooks and memo integration

`FrameOutput` には hook 由来の log / signal / diagnostics / command が含まれる。hook は直接 state を変更せず、phase ごとに許可された output だけを返す。

```rust
pub struct FrameOutput {
    pub state_hash: StateHash,
    pub render: RenderSpec,
    pub ui: UiSpec,
    pub audio: AudioDesiredState,
    pub effects: Vec<EffectRequest>,
    pub diagnostics: Vec<RuntimeDiagnostic>,
    pub hook_outputs: Vec<HookOutput>,
    pub memo_stats: Option<MemoFrameStats>,
}
```

Memoization は pure evaluation と task scheduling の両方に統合される。memo hit/miss は性能には影響するが、ゲーム意味論と `state_hash` には影響してはならない。

## Hooks and memoization in core

`arcweft-core` は hook を直接副作用として実行しない。各 phase で `HookOutput` を生成し、phase boundary で `GameEvent` / `Command` / `SignalUpdate` として取り込む。

Memoization は `arcweft-core` の意味論を変えない。cache hit/miss は state hash に含めず、pure computation の結果だけを再利用する。詳細は [Hook Runtime / Memo Runtime](hooks-memoization.md) を参照。
