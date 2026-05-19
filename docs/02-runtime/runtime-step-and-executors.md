# RuntimeStep, executors, interpreter/AOT coexistence

## RuntimeStep boundary

Core runtime uses `RuntimeStep*`. Game/render/audio adapters may use `Frame` terminology, but the VM boundary does not.

```rust
pub trait RuntimeExecutor {
    fn step(
        &mut self,
        input: RuntimeStepInput,
        options: RuntimeStepOptions,
    ) -> RuntimeStepResult;

    fn snapshot(&self) -> RuntimeSnapshot;
    fn restore(&mut self, snapshot: RuntimeSnapshot) -> Result<(), RestoreError>;
}
```

## Executors

```rust
pub struct VmExecutor {
    plan: RuntimePlan,
    fiber: FlowFiber,
}

pub struct AotExecutor {
    state: AotRuntimeState,
    dispatch: AotDispatchTable,
}

pub struct HybridExecutor {
    vm: VmExecutor,
    pure_cache: PureFunctionBackend,
}
```

All executors use the same `RuntimeStepInput` and `RuntimeStepResult`.

## Interpreter / compiled modes

```text
VM interpreter
  - semantic source of truth
  - dev/test/replay/LSP/Agent

AOT compiled player
  - native/web player is compiled
  - script remains bytecode bundle initially

Pure AOT/JIT helper
  - pure deterministic functions only
  - VM fallback and VM equivalence checks required

Full script AOT
  - late release backend
  - generated state machine implements RuntimeExecutor
```

AOT never directly touches filesystem, network, renderer, audio, or wall-clock. It emits `RuntimeEffectBatch` and `HostRequestBatch` just like the VM.

## Step options

```rust
pub enum RuntimeStepMode {
    OneOp,
    DrainUntilBlocked,
    DrainUntilOutput,
    DrainUntilPresentationChange,
    DrainUntilBudget,
}

pub struct RuntimeStepBudget {
    pub max_vm_ops: usize,
    pub max_effects: usize,
    pub max_task_requests: usize,
    pub max_source_events: usize,
    pub max_stream_ops: usize,
}

pub struct RuntimeStepOptions {
    pub mode: RuntimeStepMode,
    pub budget: RuntimeStepBudget,
}
```

## Effect batch

```rust
pub struct RuntimeEffectBatch {
    pub presentation: Vec<PresentationEffect>,
    pub audio: Vec<AudioEffect>,
    pub ui: Vec<UiEffect>,
    pub log: Vec<RuntimeLog>,
    pub signal: Vec<RuntimeAssignment>,
    pub metric: Vec<RuntimeAssignment>,
    pub event: Vec<RuntimeEvent>,
    pub debug: Vec<DebugEffect>,
    pub host: Vec<HostEffect>,
    pub control: Vec<RuntimeControlEffect>,
}
```

`LineEffectRequest` remains only as line-task-local IR. It is folded into `RuntimeEffectBatch` before leaving a runtime step.

## Host requests

```rust
pub struct HostRequestBatch {
    pub tasks: Vec<TaskSpec>,
    pub cancels: Vec<CancelScopeId>,
    pub source_close: Vec<SourceId>,
}
```

## Structured payloads

```rust
pub enum RuntimePayload {
    Unit,
    Bool(bool),
    Int(i64),
    Float(String),
    String(String),
    Bytes(Vec<u8>),
    EntityRef(String),
    Record(Vec<RuntimeFieldValue>),
    List(Vec<RuntimePayload>),
    RuntimeValue(RuntimeValue),
}
```

`SourceEvent<String, String>` and `StreamEvent<String, String>` are removed.

## Snapshot rule

Snapshots are executor-independent:

```rust
pub struct RuntimeSnapshot {
    pub fiber: RuntimeFiberSnapshot,
    pub env: RuntimeEnvSnapshot,
    pub source_states: Vec<SourceSnapshot>,
    pub stream_states: Vec<StreamSnapshot>,
    pub observations: RuntimeObservationState,
    pub version: SnapshotVersion,
}
```

VM snapshots and AOT snapshots must use the same representation.
