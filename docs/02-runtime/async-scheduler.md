# 非同期・scheduler・Need

## TaskSpec

```rust
pub struct TaskSpec {
    pub id: TaskId,
    pub key: TaskKey,
    pub class: TaskClass,
    pub priority: TaskPriority,
    pub cancel_scope: CancelScopeId,
    pub policy: TaskPolicy,
    pub request: HostTaskRequest,
    pub debug_label: String,
}
```

Runtime adapter tasks are created through `ensure_task`; same-key tasks join
instead of creating duplicate backend work. Arcweft source-level `thread`
blocks are VM-scoped fibers and lower to effect requests rather than directly
creating OS tasks.

```rust
pub trait TaskHost {
    fn ensure_task(&mut self, spec: TaskSpec) -> TaskHandle;
    fn cancel_scope(&mut self, scope: CancelScopeId);
    fn poll_frame(&mut self, budget: SchedulerBudget) -> Vec<TaskEvent>;
}
```

`arcweft-core` keeps this interface as Sans I/O data. The task body lives in the
compiled runtime plan or in a host adapter table; `TaskSpec` identifies the
work by typed `HostTaskRequest`, class, priority, cancellation scope, and stable
key. `debug_label` is for diagnostics only and is never an execution
discriminator. Backends such as Tokio, Rayon, web workers, or a cooperative test
executor sit outside core.

```rust
pub enum HostTaskRequest {
    FileReadText(FileReadTextRequest),
    FileReadBytes(FileReadBytesRequest),
    FileWriteText(FileWriteTextRequest),
    FileWriteBytes(FileWriteBytesRequest),
    HttpFetch(HttpFetchRequest),
    HttpRespond(HttpRespondRequest),
    ProcessRun(ProcessRunRequest),
    AssetLoad(AssetRequest),
    ShaderCompile(ShaderRequest),
    AudioDecode(AudioDecodeRequest),
    TtsSynthesis(TtsRequest),
    WasmCall(WasmCallRequest),
    Custom {
        capability: HostCapabilityId,
        operation: String,
        args: Vec<RuntimePayload>,
    },
}
```

## Task class

```rust
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
```

## 完了順の正規化

```rust
pub fn normalize_task_events(mut events: Vec<TaskEvent>) -> Vec<TaskEvent> {
    events.sort_by_key(|event| (
        event.logical_epoch,
        event.task_id.clone(),
        event.sequence,
    ));
    events
}
```

The normalized event envelope is:

```rust
pub struct TaskEvent {
    pub logical_epoch: LogicalEpoch,
    pub task_id: TaskId,
    pub sequence: TaskSequence,
    pub kind: TaskEventKind,
}

pub enum TaskEventKind {
    Ready(String),
    Err(String),
    Cancelled,
    Progress(String),
}
```

## single-thread と multi-thread

- native multi-thread: Tokio/Rayon 等の worker。
- native single-thread: cooperative job。
- web single-thread: requestAnimationFrame + cooperative job。
- web multi-thread: optional SharedArrayBuffer/worker pool。

## CooperativeJob

```rust
pub trait CooperativeJob {
    type Output;
    fn resume(&mut self, budget: JobBudget) -> JobPoll<Self::Output>;
}
```

## Need handling

`Need<T, E>` は `T` へ暗黙変換しない。

flow:

```arcw
let data = try await load() with {
    pending p => scene.show(@scene.loading); progress.set(p.ratio)
}
```

UI:

```arcw
AwaitView(load_avatar(user)) {
    pending _ => SkeletonCircle()
    ready img => Image(img)
    error _ => Icon(@vector.avatar_fallback)
}
```

In the Phase 2.0 Sans I/O runtime slice, `await ... with` lowers to a
`FlowOp::Await` containing an `AwaitTarget` and pending effects. `AwaitTarget`
stores the `NeedId`, `TaskId`, and typed `HostTaskRequest` that the host adapter
must execute. Entering the op emits a `TaskSpec` using that request and switches
`FlowFiberStatus` to `Waiting`. The runtime resumes only when a matching
`TaskEvent` arrives in a later `RuntimeStepInput`.

```text
TaskEventKind::Ready(value)    -> resume the flow after the await op
TaskEventKind::Progress(value) -> keep waiting and emit an await-progress event
TaskEventKind::Err(error)      -> mark the fiber failed
TaskEventKind::Cancelled       -> mark the fiber failed
```

Actual asset loading, worker execution, clocks, renderer progress UI, and audio
work remain outside `arcweft-core`. Core only produces deterministic task
requests and consumes deterministic task events.

Recognized capability calls such as `fs.read_text(...)`, `http.fetch(...)`,
`asset.image(...)`, `shader.compile(...)`, `audio.decode(...)`,
`tts.synthesize(...)`, `process.run(...)`, and `wasm.call(...)` lower to
dedicated `HostTaskRequest` variants. Unknown awaited call namespaces lower to
`HostTaskRequest::Custom` with structured argument payloads.

