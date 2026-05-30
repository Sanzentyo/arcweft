# RuntimeStep, executors, interpreter/AOT coexistence

## RuntimeStep boundary

Core runtime uses `RuntimeStep*`. Game/render/audio adapters may use `Frame`
terminology, but the VM boundary does not. The boundary is Sans I/O: a host
passes pure data into the runtime and receives pure output batches plus a stop
reason.

```rust
pub trait RuntimeExecutor {
    fn step(
        &mut self,
        input: RuntimeStepInput,
        options: RuntimeStepOptions,
    ) -> RuntimeStepResult;

    fn fiber(&self) -> &FlowFiber;
}

pub struct RuntimeStepResult {
    pub output: RuntimeStepOutput,
    pub fiber_status: FlowFiberStatus,
    pub stop_reason: RuntimeStepStopReason,
    pub stats: RuntimeStepStats,
}
```

`Engine` implements `RuntimeExecutor` directly. `VmExecutor` is the current
semantic executor wrapper used by CLI tooling; it delegates to `Engine` and keeps
the VM as the source of truth. `BytecodeProgram` is the pure data bundle between
runtime-plan lowering and VM/AOT/JIT execution. `BytecodeVmExecutor` executes a
bytecode bundle through the semantic VM so bytecode generation can be tested
before a separate dispatch loop exists. `AotExecutor` also implements
`RuntimeExecutor` today, but its Phase 2 implementation deliberately delegates to
`VmExecutor` so the public AOT boundary can be tested without creating a second
semantic engine.

Snapshot/restore remains a future shared contract. It must use executor-neutral
data when added so VM, AOT, replay, and LSP tooling can compare equivalent
runtime states.

## Executors

```rust
pub struct VmExecutor {
    engine: Engine,
}

pub struct AotExecutor {
    vm: VmExecutor,
}

pub struct BytecodeVmExecutor {
    program: BytecodeProgram,
    vm: VmExecutor,
}

pub struct HybridExecutor {
    vm: VmExecutor,
    pure_cache: PureFunctionBackend,
}
```

All executors use the same `RuntimeStepInput`, `RuntimeStepOptions`, and
`RuntimeStepResult`. VM, bytecode VM, and the current AOT executor can also be
stepped with an adapter-provided `RuntimePureCallBackend`; this keeps pure
helper acceleration outside `arcweft-core` while allowing ordinary flow code to
benefit from AOT/JIT pure helpers automatically.

`RuntimeStepStats.pure` reports per-step pure helper calls, batch calls, batch
item counts, JIT/AOT/VM call counts, fixed-stack argument packs, copied
argument/result byte counts, thread-pool jobs, Vec argument allocations, and
fallback counts. These counters are step-local even when an adapter keeps a
persistent JIT/AOT compile cache across steps. Executor-level stats report the
selected pure backend, worker policy, batch threshold, helper acceleration
summary, compile attempts, cache hits/misses, and compile elapsed time.
For deterministic integer helpers, VM fallback evaluation also consumes the
fixed-stack argument pack directly; `arg_vec_allocations` therefore identifies
value-slice fallback calls rather than normal scalar or batch integer calls.
Bytecode VM programs preserve the runtime plan's pure-helper table, so executor
artifact lowering does not change whether ordinary flow calls can use the
adapter-provided pure backend.

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
  - current AOT helper compiles deterministic i64 expressions to a typed plan

Full script AOT
  - late release backend
  - generated state machine implements RuntimeExecutor
```

AOT never directly touches filesystem, network, renderer, audio, or wall-clock.
It emits `RuntimeEffectBatch` and `HostRequestBatch` just like the VM.
Generated AOT dispatch remains future work; until then, `AotExecutor` is a
VM-equivalent conformance boundary.
Pure helper AOT is narrower and already executable: `AotPureFunctionBackend`
rejects unsupported helpers instead of falling back, then compares its typed
integer plan against `VmPureFunctionBackend` in tests. Batch AOT calls reuse
caller-owned slot storage, so repeated runtime helper evaluation does not clone
the compiled plan's local slot vector for every item.

## Step options

```rust
pub enum RuntimeStepMode {
    OneOp,
    Drain,
    Game,
    Server,
}

pub struct RuntimeStepBudget {
    pub max_ops: usize,
}

pub struct RuntimeStepOptions {
    pub mode: RuntimeStepMode,
    pub budget: RuntimeStepBudget,
}
```

`Engine::step` drains internally according to the selected mode:

- `OneOp`: execute at most one runtime operation.
- `Drain`: execute until blocked, done, failed, or `max_ops` is exhausted.
- `Game`: drain internal bookkeeping but return on presentation-visible output.
- `Server`: drain like server-side automation; pure observations do not stop the
  step.

`max_ops` is enforced inside the VM loop. If the budget is reached before a hard
stop, the result reports `RuntimeStepStopReason::BudgetExhausted`.

## Stop reasons

```rust
pub enum RuntimeStepStopReason {
    OneOp,
    Blocked,
    Output,
    BudgetExhausted,
    Done,
    Failed,
}
```

`Blocked` covers runtime states that require later host input, such as choices or
awaited task results. `Output` covers host-visible output in `Game` mode and host
requests that require the adapter to act. `Done` and `Failed` are terminal hard
stops.

## Effect batch

```rust
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
```

`LineEffectRequest` is still the current Phase 2.0 line/runtime effect IR. Hosts
must treat it as pure data and perform actual presentation, audio, logging,
signal, or metric side effects outside `arcweft-core`.

## Host requests

```rust
pub struct HostRequestBatch {
    pub tasks: Vec<TaskSpec>,
    pub cancel_scopes: Vec<CancelScopeId>,
    pub source_close: Vec<SourceId>,
}
```

## Structured payloads

```rust
pub struct RuntimePayload(pub RuntimeValue);
```

`SourceEvent<String, String>` and `StreamEvent<String, String>` are removed.
Use `RuntimeSourceEvent` and `RuntimeStreamEvent`. Display tooling may derive a
payload label, but the runtime boundary preserves the structured value.
