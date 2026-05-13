# save / replay / hot reload

## SaveSnapshot

```rust
pub struct SaveSnapshot {
    pub bundle_hash: BundleHash,
    pub program_version: Version,
    pub state: Value,
    pub logical_time: LogicalTime,
    pub active_scene: SceneId,
    pub flow_fibers: Vec<FlowFiberSnapshot>,
    pub active_activities: Vec<ActivitySnapshot>,
    pub audio_state: AudioSnapshot,
    pub awaited_tasks: Vec<AwaitedTaskSnapshot>,
}
```

## 未完了 task

生 Future は保存しない。保存するのは task key と continuation。

```rust
pub struct AwaitedTaskSnapshot {
    pub task_id: TaskId,
    pub key: TaskKey,
    pub class: TaskClass,
    pub source: TaskSource,
}
```

load 時は `ensure_task` で再登録。

## ReplayTrace

```rust
pub struct ReplayTrace {
    pub engine_version: String,
    pub bundle_hash: BundleHash,
    pub initial_state: StateSnapshot,
    pub frames: Vec<RecordedFrameInput>,
    pub task_responses: Vec<RecordedTaskResponse>,
    pub audio_events: Vec<RecordedAudioEvent>,
    pub agent_actions: Vec<RecordedAgentAction>,
}
```

## Hot reload

```text
incoming patch
  → parse
  → typecheck
  → contract check
  → shader validate
  → wasm/rust ABI check
  → state compatibility check
  → dry-run current continuation
  → commit at frame boundary
```

## Migration

```rust
migrate save from "1.2.0" to "1.3.0" {
    state.config.text_speed = 1.0
    state.flags = state.flags ?? {}
}
```

## Agent replay

Agent actions are replayed as semantic actions when possible.

```bash
arcw agent replay traces/bug.awfagent.ndjson --headless
```

