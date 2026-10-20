# RuntimeStep, executors, interpreter/AOT coexistence

## RuntimeStep boundary

Core runtime uses `RuntimeStep*`. Game/render/audio adapters may use `Frame`
terminology, but the VM boundary does not. The boundary is Sans I/O: a host
passes pure data into the runtime and receives pure output batches plus a stop
reason.

A flow Match guard retains its original candidate, declared result type,
operations, optional final condition and Copy bindings in one transaction across
construction and admitted storage. A returning guard must produce the declared
Bool type; a guard without a final condition must declare Never. The executable
semantic transcript commits that result type even when the guard transfers
control, so later Source construction does not infer it from an absent value.

`Shared` and `Reference` qualify the original executable type; they do not add
a runtime value wrapper. Live and inert value admission follow these type heads
under the same scope authority and metadata work/depth allowance, then validate
the materialized referent. A qualifier-only cycle cannot manufacture an admitted
value. The referent contributes its value nodes and canonical bytes once; qualifier traversal
does not create another value bank or grant Copy or persistence rights.

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
wrapping. Value-shaped VM fallback takes the caller's evaluated argument vector
by ownership transfer, including affine values, and reuses VM scratch root
bindings between calls.
`arg_bytes_borrowed` counts the backend's temporary slice inspection of that
owned vector; it does not represent a copy of the value graph.
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

Repeated i64 rows cross that boundary as a checked capability retaining the
original function and one borrowed complete input row. Header admission checks
the whole input count, every input ABI, the output ABI, and the bounded row count
before execution, promotion, or counters. An empty batch performs no work and
borrows no bytes. Each nonempty row evaluates the original function once and
contributes to a wrapping sum; identical inputs do not authorize multiplying one
evaluation's result. Call counters include only attempted rows, including a
failing row, while batch item counts describe the admitted request. A failure
stops the suffix without retry on another backend. Borrowed-byte counters cover
the single retained row once per batch. This physical capability does not admit
the source caller's operation budget or establish callback totality.

Numeric Map batches with a homogeneous scalar signature retain the original
function, complete input layout, row
count and result mode in one admitted packet. The packet validates every item
and retained constant before allocating its physical argument storage. Repeated
rows retain one complete row; distinct floating-point zero signs and NaN payloads
remain distinct physical representations. Unused source items need no numeric
projection, while their ownership proof is still required. Zero-argument targets
retain the source row count, and empty packets execute no backend work. A backend
that requires dense input materializes that representation only from the admitted
packet. i64 repeated reductions borrow the retained row directly. These storage
choices preserve the existing complete source-span budget and totality admission.

A first-class source Map also owns a closed kernel boundary. In Drain mode a
capture-free, fully applied callback may execute that Map as one control
operation when its entire ordinary body matches the selected collection family
and the original target has total numeric completion proof. Every row still
evaluates the original function with every whole formal. Kernel admission checks
the exact source/result layouts, source ownership, complete physical packet and
backend capability before consuming the prepared source. It cannot suspend or
publish a partial result. Captured callbacks and OneOp retain the ordinary body
and its original per-application budget. The separate optimizer for an already
expanded ApplyGroup/iterator span still requires its full scheduled cost to fit.
A consuming Sum may fuse only with its existing totality and later-observer
proof; its actual continuation operations also have to fit the remaining budget.
The compiled Map control retains those aliases and Sum in its ordinary body.
An admitted fused kernel consumes one plan node but charges the original Map
operation plus every owned continuation operation. If only mapping can batch,
the same consuming suffix remains scheduled after the collection is bound.

The complete Map schedule has one typed owner in construction and admitted
phases. Construction retains original local handles, the mapping body and any
owned Sum consumption. Body transformations preserve that boundary when their
operation counts change. An explicit Sum source is definition data: the
builder validates its entire consumption with the same sequence-Sum grammar
used by ordinary normalization, against the actual result declaration. Missing
consumption, another receiver, or an unmatched suffix is rejected before plan
publication. This construction does not grant numeric-kernel admission; the
runtime proofs and original callback/budget requirements above still apply.

Numeric Map admission borrows the immutable source and every original whole
formal before allocating physical argument storage. Its selected completion
permits Sum only when the output domain fits i64 or conservative integer bounds
prove that every result of the original admitted function fits. The proof reads
that body's typed operations and uses the shared lexical/Unit/move bindings;
it executes no callback. Uncertain native wrapping or narrowing retains the
collection and its original consuming Sum. Unused formals still undergo complete
ABI validation. The caller admits the original cost of the selected completion
before materializing arguments or preparing/promoting a backend.

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

Dialogue activation retains an executor-owned frame while the parent is
suspended. That outward suspension does not create a host boundary when the
admitted activation or its owned cleanup child has internal work. Pending host
results, command acknowledgements, and live presentation input remain real
waits. An instruction that opens a host wait still consumes its operation
budget; its external wait does not erase the instruction's charge. A compact
activation's VM quantum is
replenished only at the next host step, preserving its exact preemption cursor
and the caller's remaining `max_ops` boundary.

Retained child scopes remain executor work after the parent finishes, including
detached line work. Local join policy governs the parent return gate and does
not discard that executor ownership. A blocked child at the front of the queue
must not hide a later runnable child. Product execution polls at most the
retained queue length at that dispatch boundary; a blocked poll issues no VM
instruction and does not consume an operation charge.

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

Direct Host calls retain their original producer coordinate, request identity,
source-ordered positional/named/spread arguments, result type, mode and
determinism in the same target grammar through construction and admission.
An optional result binding must have that exact admitted result type. A mismatch
rejects the body before publication and leaves construction terminally rejected.
The private version-1 Source codec admits the complete borrowed framing and
aggregate argument/value storage before reconstruction. Decoding requires a
real reservation from the receiving builder; an ordinal or equal task key does
not grant another program's construction handle. Source carries definitions;
the actual builder and common executable seal issue the completed task identity.

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

Host `Ready` and HostCall results transfer their admitted `RuntimePayload` into
the runtime owner. `FlowEvent::AwaitReady` includes a value only when that exact
payload is deeply Copy; an affine Ready value still reaches its destination
without a second observer copy.
An Await progress observer returns its pending `Need` handle from suspension
to the verified frame before the observer runs; a re-await then consumes that
same handle again. The Ready branch does not restore the handle.

## Product AWBC parity contract

Canonical Game-product execution through this boundary is specified in
[`product-awbc-runtime-step-parity.md`](product-awbc-runtime-step-parity.md).
That contract defines explicit progression, typed host handshakes, ordering,
diagnostics, statistics, and the differential completion gate without restoring
a structured product fallback.
