# Sans I/O Core

`arcweft-core` は副作用を実行しない。入力を受け取り、次の状態と要求を返す。

Core が扱う program、bytecode、manifest reference、save snapshot、diagnostic、trace はすべて pure data。Core は path を開かず、filesystem、network、wall-clock、GPU/audio/device handle、Wasm runtime、Cranelift runtime を保持しない。外部 adapter は bytes/string へ serialize された bundle や task result を `RuntimeStepInput` / `TaskEvent` として渡す。

## Runtime ID domains

Runtime lookup IDs are typed canonical paths, not raw public strings. A
`FlowRuntimeId`, `EntryRuntimeId`, `RuntimeLineId`, or `StreamRuntimeId` owns a
runtime path whose Rust type supplies the family. The path itself therefore
does not store source-family prefixes such as `flow`, `entry`, `say`, or
`stream`.

Arcweft keeps three ID domains separate:

- Source references live in parser/HIR/lowering while relative syntax is still
  meaningful. They may contain family-qualified source spelling such as
  `@flow.main` or current/parent-relative addressing.
- Canonical runtime IDs are execution lookup keys. Source `@flow.main` lowers
  to a `FlowRuntimeId` whose canonical path is `main`; `flow.main` is not the
  runtime lookup key.
- Public/debug labels are deliberate strings used in AWBC reports, manifests,
  logs, diagnostics, and user-facing output. Runtime code must not recover a
  lookup ID by splitting one of these labels.

If runtime-ID equality, hashing, or storage later becomes a measured hot path,
the canonical path representation may be interned behind the typed ID API. That
optimization must preserve the same source/runtime/public domain split and must
not make atom numbers part of save files, bundles, diagnostics, or authored
source.

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
    LoopNext { body: Arc<[FlowOp]> },
    While { condition: RuntimeExpr, body: Vec<FlowOp> },
    WhileNext { condition: RuntimeExpr, body: Arc<[FlowOp]> },
    WhileLet { pattern: RuntimePattern, expr: RuntimeExpr, guard: Option<RuntimeExpr>, body: Vec<FlowOp> },
    WhileLetNext { pattern: RuntimePattern, expr: RuntimeExpr, guard: Option<RuntimeExpr>, body: Arc<[FlowOp]> },
    For { pattern: RuntimePattern, source: RuntimeExpr, body: Vec<FlowOp> },
    ForNext { pattern: RuntimePattern, items: Arc<[RuntimeValue]>, index: usize, body: Arc<[FlowOp]> },
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
    pub control_stack: Vec<FlowControlStackEntry>,
    pub env: RuntimeEnv,
    pub observations: RuntimeObservationState,
    pub source_states: BTreeMap<SourceId, SourceRuntimeState>,
    pub stream_states: BTreeMap<StreamRuntimeId, StreamRuntimeState>,
    pub status: FlowFiberStatus,
}

pub enum FlowControlStackEntryKind {
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

pub struct RuntimeStepInput {
    pub tick: TickId,
    pub dt: LogicalDuration,
    pub bindings: Vec<RuntimeBinding>,
    pub input_events: Vec<InputEvent>,
    pub task_events: Vec<TaskEvent>,
    pub audio_events: Vec<AudioEvent>,
    pub source_events: Vec<RuntimeSourceEvent>,
}

pub struct RuntimeStepOutput {
    pub diagnostics: Vec<RuntimeDiagnostic>,
    pub flow_events: Vec<FlowEvent>,
    pub effects: RuntimeEffectBatch,
    pub requests: HostRequestBatch,
}

pub struct RuntimeEffectBatch {
    pub line: Vec<LineEffectRequest>,
    pub source_events: Vec<RuntimeSourceEvent>,
    pub stream_events: Vec<RuntimeStreamEvent>,
}

pub struct HostRequestBatch {
    pub tasks: Vec<TaskSpec>,
    pub cancel_scopes: Vec<CancelScopeId>,
    pub source_close: Vec<SourceId>,
}

pub struct RuntimeStepResult {
    pub output: RuntimeStepOutput,
    pub fiber_status: FlowFiberStatus,
    pub stop_reason: RuntimeStepStopReason,
}
```

Phase 2.0 の `Engine` は headless structured-control-flow runtime slice であり、まだ完全な story VM ではない。
`Engine::step(RuntimeStepInput, RuntimeStepOptions) -> RuntimeStepResult` は
`RuntimeStepMode` と `RuntimeStepBudget::max_ops` に従って lowered flow op を
内部 drain する。`OneOp` は最大 1 op、`Drain` / `Server` は blocked/done/failed
または budget まで進め、`Game` は presentation-visible output で host に制御を返す。
`Dialogue` は line task group を実行し、`Choice` は入力待ち、`Await` は `TaskSpec`
を出して `TaskEvent` で再開する。`Let` / `LetElse` /
`If` / `IfLet` / `Match` / `Loop` / `LetLoop` / `While` / `WhileLet` / `For` / `LetScope` は
`RuntimeValue` / `RuntimeExpr` / `RuntimePattern` と `RuntimeEnv` で評価され、
選択された block は `pending_ops` queue に積まれて以降の runtime op として実行される。
`FlowFiber::control_stack` は lexical scope と loop continuation を明示的に保持し、
`break` / `continue` が body 内の残り op と scope-local binding を破棄して
最も近い loop/while/while-let continuation へ移るために使われる。
`let name = scope { ... }` は final expression を scope 内で評価してから外側の
pattern に束縛し、`let name = loop { break expr }` は `break expr` の値を
loop result pattern に束縛する。
`RuntimeStepInput::bindings` は ambient input として root runtime scope に束縛される。
branch / match / while-let pattern binding は選択された lexical scope にだけ束縛される。
Match はコンテナ全体を覆う共通 Block を作らず、通常の arm はそれぞれ独立した
`MatchArm` scope を所有し、Thread の braced arm だけが単一の `Block` scope を所有する。
binding は guard 評価後や arm 終了後には外側や sibling arm へ漏れない。
`Goto` / `GotoExpr` / `Return` / `ReturnExpr` は `FlowFiber` の cursor/status を更新する。
`RuntimeStepInput.source_events` は replay-stable order に正規化され、`SourcePlan` handler と
`StreamPlan` によって queue state と `RuntimeStreamEvent` に反映される。source / stream
payload は `RuntimePayload` として `RuntimeValue` shape を保持し、CLI 表示だけが
human-readable label に変換する。`FlowFiber.observations`
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
    Wait(RuntimeWaitTarget),
    Call(RuntimeCall),
    Log(RuntimeLog),
    SignalWrite(RuntimeAssignment),
    MetricWrite(RuntimeAssignment),
    EmitEvent(RuntimeEvent),
    Out(LineOutRequest),
    Return(String),
    Goto(String),
    Panic(String),
    Fail(String),
    Bail(String),
    Ensure { condition: String, message: String },
    Assert(RuntimeAssertion),
    Close(String),
    Select(String),
    Break { label: Option<String>, value: Option<String> },
    Continue { label: Option<String> },
}

pub struct RuntimeAssertion {
    guard: RuntimeAssertionGuardId,
    condition: String,
    message: String,
    profile: RuntimeAssertionProfile,
}

pub struct RuntimeAssertionFailure {
    assertion: RuntimeAssertion,
}

pub enum RuntimeAssertionProfile {
    Always,
    DebugOnly,
}
```

`RuntimeEffectExpr::Assert` evaluates its condition as a typed `Bool` inside
the Sans-I/O runtime. `true` produces no host request. Only `false`
materializes `LineEffectRequest::Assert`, so hosts treat that request as an
already-established failure and never parse `condition` text to decide whether
it failed. A non-`Bool` condition is a typed runtime materialization error.

The persisted failure identity is the checked 16-byte
`RuntimeAssertionGuardId`; condition and message strings are presentation data.
Hosts return `RuntimeAssertionFailure` as core data. A fresh compiler session
may join that guard to the exact statement, zero-based condition index, mode,
and revision-bound source span through a non-serialized runtime-plan inventory
bound to the exact runtime-plan artifact fingerprint. Without that exact
association, CLI, LSP, Agent, and debug presentation use persisted evidence and
must not fabricate HIR or syntax identity. Both paths emit the stable code
`runtime.assertion_failed`.

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
on mark(.mark) { ... }            -> child task with trigger Mark(".mark")
wait(mark(.x))              -> LineEffectRequest::Wait(RuntimeWaitTarget::Mark(".x"))
wait(0.35s)                 -> LineEffectRequest::Wait(RuntimeWaitTarget::Duration(...))
'line.key <- expr           -> LineEffectRequest::RegisterHandle
'line.key |> drop           -> LineEffectRequest::DropHandle
call-like expression stmt   -> LineEffectRequest::Call
log.info(...)               -> LineEffectRequest::Log
signal.set(target, value)   -> LineEffectRequest::SignalWrite
metric.set(target, value)   -> LineEffectRequest::MetricWrite
event.emit(Event, fields)   -> LineEffectRequest::EmitEvent
ensure(cond, msg)           -> LineEffectRequest::Ensure
assert.check(cond, ...)     -> LineEffectRequest::Assert(profile=Always)
assert.debug(cond, ...)     -> LineEffectRequest::Assert(profile=DebugOnly)
out expr                    -> LineEffectRequest::Out and LineTaskGroup.out
cancel on ... { ... }       -> LineTaskGroup.cancel_rules
memo name(...)              -> LineTaskGroup.memo
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
for PAT in EXPR            -> FlowOp::For over RuntimeValue::Seq
scope / bare block         -> FlowOp::Scope
goto @flow.x / goto route  -> FlowOp::GotoExpr
return expr                -> FlowOp::ReturnExpr
out / log / signal / call  -> FlowOp::Effect or line effect
cancel on input(.SkipLine)  -> line cancel rule selected from RuntimeStepInput
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

```arcw
flow opening(state: GameState)
effects { state.write('flow), state.write('global) }
{
    alice[見たことにする。[mark .seen][p]]
    with {
        on mark(.seen) {
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
- replay は `RuntimeStepInput` 列と task/audio/view result を記録。

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



## Dispatch and cache integration

Owner-local handlers do not directly mutate durable state; each dispatch point
returns only the typed outputs allowed by its owner.
Phase 2.0 の `RuntimeStepOutput` は diagnostics、flow events、`RuntimeEffectBatch`、
`HostRequestBatch` に分かれ、render/audio/device の実行は adapter 側に残る。将来の
presentation runtime は、この境界から render/audio/view desired state を導出する。

```rust
pub struct RuntimeStepOutput {
    pub diagnostics: Vec<RuntimeDiagnostic>,
    pub flow_events: Vec<FlowEvent>,
    pub effects: RuntimeEffectBatch,
    pub requests: HostRequestBatch,
}
```

Pure-evaluation reuse and task joining are owned separately by the VM/compiler
and scheduler. Cache hit/miss は性能には影響するが、ゲーム意味論と
`state_hash` には影響してはならない。

## Dispatch and caches in core

`arcweft-core` は OS callback を直接副作用として実行しない。Typed owner-local
handler output is committed at its phase boundary as semantic action, `Command`,
or scoped update.

Subsystem caching は `arcweft-core` の意味論を変えない。cache hit/miss は state
hash に含めず、pure computation の結果だけを再利用する。詳細は
[Runtime Dispatch and Caches](hooks-memoization.md) を参照。

