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

`Engine` implements `RuntimeExecutor` directly inside the core. Application
crates construct `ArcweftRuntimeExecutor`, selecting a typed
`ArcweftExecutionTier`; they do not construct the concrete VM, bytecode, AOT, or
product-AWBC executors. Those concrete executors are core-internal implementation
details, which keeps host wiring independent of backend reshaping.

`BytecodeProgram` remains the pure data bundle between runtime-plan lowering and
VM/AOT/JIT execution. Structured VM and AOT tiers use the same semantic state
machine, while the product tier owns the canonical AWBC executor. Snapshot and
restore are exposed through the facade only for tiers with a defined typed
snapshot contract; unsupported tiers return a structured error.

## Executors

```rust
pub enum ArcweftExecutionTier {
    StructuredVm,
    StructuredAot,
    AwbcProduct,
}

pub struct ArcweftRuntimeExecutor {
    inner: ArcweftRuntimeExecutorInner,
}

enum ArcweftRuntimeExecutorInner {
    StructuredVm(BytecodeVmExecutor),
    StructuredAot(AotExecutor),
    AwbcProduct(Box<AwbcProductExecutor>),
}
```

All tiers use the same `RuntimeStepInput`, `RuntimeStepOptions`, and
`RuntimeStepResult`. The facade can also be stepped with an adapter-provided
`RuntimePureCallBackend`; this keeps pure helper acceleration outside
`arcweft-core` while allowing ordinary flow code to benefit from AOT/JIT pure
helpers automatically.

`RuntimeStepStats.pure` reports per-step pure helper calls, batch calls, batch
item counts, JIT/AOT/VM call counts, fixed-stack argument packs, copied
argument/result byte counts, thread-pool jobs, Vec argument allocations, and
fallback counts. These counters are step-local even when an adapter keeps a
persistent JIT/AOT compile cache across steps. Executor-level stats report the
selected pure backend, worker policy, batch threshold, selected dense math
backend, math GPU auto threshold, helper acceleration summary, compile
attempts, cache hits/misses, and compile elapsed time.
For deterministic integer helpers, VM fallback evaluation also consumes the
fixed-stack argument pack directly; `arg_vec_allocations` therefore identifies
runtime call-site argument materialization rather than backend fallback
wrapping. Value-slice VM fallback receives the caller's argument slice by
borrow and reuses VM scratch root bindings between calls.
Bytecode VM programs preserve the runtime plan's pure-helper table, so executor
artifact lowering does not change whether ordinary flow calls can use the
adapter-provided pure backend.

The pure-call boundary includes both scalar fixed-argument calls and row-major
integer batch calls. `arcweft-core` provides a Sans I/O VM batch implementation
with deterministic counters; adapter crates can override the same trait method
with AOT, JIT, and worker-pool execution without changing VM semantics.
Bracket sequence expressions made only of the same statically integer-shaped
pure helper call are evaluated through that batch boundary, so ordinary
collection-style source can cross into AOT/JIT once per row group instead of
once per element when an adapter backend is installed.

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
  - current AOT helper compiles deterministic exact-width integer and
    floating-point expressions to typed plans

Full script AOT
  - late release backend
  - generated state machine implements RuntimeExecutor
```

AOT never directly touches filesystem, network, renderer, audio, or wall-clock.
It emits `RuntimeEffectBatch` and `HostRequestBatch` just like the VM.
Full generated state-machine AOT remains future work. The current
`AotExecutor` already owns a typed `AotProgram` with pre-lowered linear
operation blocks, executes fully linear flows and mixed-flow linear prefixes
through that artifact without cloning `FlowOp` values during each step, and
continues through the VM-compatible state machine in the same runtime step when
a mixed control-flow boundary is reached.
Pure helper AOT is narrower and already executable: `AotPureFunctionBackend`
rejects unsupported helpers instead of falling back, then compares its typed
integer and floating-point plans against `VmPureFunctionBackend` in tests.
Batch AOT calls reuse caller-owned slot storage, so repeated runtime helper
evaluation does not clone the compiled plan's local slot vector for every item.

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
    pub host_calls: Vec<RuntimeHostCallRequest>,
}
```

## Structured payloads

```rust
pub struct RuntimePayload(pub RuntimeValue);
```

`StreamEvent<String, String>` is removed. Use `RuntimeStreamEvent` for typed
external-capability stream events. Display tooling may derive a payload label,
but the runtime boundary preserves the structured value.

## Product AWBC parity contract

Canonical Game-product execution through this boundary is specified in
[`product-awbc-runtime-step-parity.md`](product-awbc-runtime-step-parity.md).
That contract defines explicit progression, typed host handshakes, ordering,
diagnostics, statistics, and the differential completion gate without restoring
a structured product fallback.
