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
    pub source: TaskSource,
}
```

`spawn` ではなく `ensure_task` を使い、同じ task key は合流する。

```rust
pub trait TaskHost {
    fn ensure_task(&mut self, spec: TaskSpec, body: TaskBody) -> TaskHandle;
    fn cancel_scope(&mut self, scope: CancelScopeId);
    fn poll_frame(&mut self, budget: SchedulerBudget) -> Vec<TaskEvent>;
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
task_events.sort_by_key(|event| (
    event.logical_epoch,
    event.task_id,
    event.sequence,
));
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

```awft
let data = try await load() with {
    pending p => scene @scene.loading { progress p.ratio }
}
```

UI:

```awft
AwaitView(load_avatar(user)) {
    pending _ => SkeletonCircle()
    ready img => Image(img)
    error _ => Icon(#vector.avatar_fallback)
}
```

