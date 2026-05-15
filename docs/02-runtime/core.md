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
    pub children: Vec<LineChildTask>,
    pub cleanup: LineCleanupPolicy,
}

pub struct LineChildTask {
    pub name: Option<String>,
    pub body: Vec<LineEffectRequest>,
    pub finally: Vec<LineEffectRequest>,
}

pub enum LineEffectRequest {
    RegisterHandle { key: String, handle: String },
    DropHandle { key: String },
    WaitMark(String),
    Wait(LogicalDuration),
    EmitSignal(String),
}
```

Line lifetime registry paths such as `'line.focus` are static keys owned by the
line scope. Guaranteed keys can be read directly; optional reads use
`'line.focus?`. Drop operations remove registered values and run their adapter
cleanup policy through emitted requests.

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
