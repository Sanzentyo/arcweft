# Sans I/O Core

`arcweft-core` は副作用を実行しない。入力を受け取り、次の状態と要求を返す。

Core が扱う program、bytecode、manifest reference、save snapshot、diagnostic、trace はすべて pure data。Core は path を開かず、filesystem、network、wall-clock、GPU/audio/device handle、Wasm runtime、Cranelift runtime を保持しない。外部 adapter は bytes/string へ serialize された bundle や task result を `FrameInput` / `TaskEvent` として渡す。

```rust
pub struct Engine {
    world: World,
    program: ProgramId,
    deterministic_clock: LogicalClock,
    flows: FlowSet,
}

pub struct FrameInput {
    pub tick: TickId,
    pub dt: LogicalDuration,
    pub input_events: Vec<InputEvent>,
    pub task_events: Vec<TaskEvent>,
    pub ui_events: Vec<UiEvent>,
    pub audio_events: Vec<AudioEvent>,
}

pub struct FrameOutput {
    pub state_hash: StateHash,
    pub render: RenderSpec,
    pub ui: UiSpec,
    pub audio: AudioDesiredState,
    pub effects: Vec<EffectRequest>,
    pub line_effects: Vec<LineEffectRequest>,
    pub diagnostics: Vec<RuntimeDiagnostic>,
}
```

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
    pub init: Vec<LineEffectRequest>,
    pub init_defer_stack: Vec<Vec<LineEffectRequest>>,
    pub children: Vec<LineChildTask>,
    pub defer_stack: Vec<Vec<LineEffectRequest>>,
    pub finally: Vec<LineEffectRequest>,
    pub cleanup: LineCleanupPolicy,
}

pub struct LineChildTask {
    pub name: Option<String>,
    pub body: Vec<LineEffectRequest>,
    pub defer_stack: Vec<Vec<LineEffectRequest>>,
}

pub enum LineEffectRequest {
    RegisterHandle { key: String, handle: String },
    DropHandle { key: String },
    WaitMark(String),
    Wait(LogicalDuration),
    EmitSignal(String),
}
```

Current Phase 1.5 lowering maps checked HIR dialogue plans into this data model:

```text
init statements             -> LineTaskGroup.init
defer in init               -> LineTaskGroup.init_defer_stack
thread name { ... }         -> LineTaskGroup.children
defer in thread/on handler  -> LineChildTask.defer_stack
defer in line scope         -> LineTaskGroup.defer_stack
on .mark { ... }            -> child task that waits for .mark, then runs body
finally { ... }             -> LineTaskGroup.finally
wait mark .x                -> LineEffectRequest::WaitMark(".x")
wait 0.35s                  -> LineEffectRequest::Wait(...)
'line.key <- expr           -> LineEffectRequest::RegisterHandle
'line.key |> drop           -> LineEffectRequest::DropHandle
call-like expression stmt   -> LineEffectRequest::EmitSignal
```

Unsupported or raw line-plan statements fail lowering. They are not reparsed or
silently accepted.

`defer` is not thread-specific syntax. It registers cleanup on the current
runtime scope. In the Phase 1.5 line-plan model, the line scope, `init` scope,
thread scopes, and event-handler scopes each have a cleanup stack. Defers inside
`finally` are lowered into the end of the finalizer body in reverse registration
order because `finally` itself is already cleanup code.

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

Line-level `finally` runs after line-scoped child tasks are cancelled/joined and
their defer stacks have run, but before any remaining automatic line-registry
drops. Scoped cleanup is represented by `defer { ... }`, so flow-level threads,
line-plan threads, handler tasks, and ordinary lexical runtime scopes share the
same cleanup model.

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
