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

[`RuntimeExecutor`](../../crates/arcweft-core/src/executor.rs) defines the
Sans-I/O step boundary implemented by `Engine` and the shared executor facade.
Application crates select `RuntimePlanVm`, `StructuredAot`, or `AwbcProduct`
through `ArcweftRuntimeExecutor`; concrete executor construction remains owned
by core. Structured tiers are constructed from `RuntimePlan` or
`RuntimeFlowInvocation`; the product tier owns canonical AWBC.
See the [executable runtime contract](executable-runtime-core.md) and
[product AWFB contract](../05-build-and-security/product-awfb-bytecode.md).
The public snapshot enum has a Product AWBC variant; tiers without a typed save
contract return a structured unsupported-tier error. The tier and snapshot
contract is defined in [`executor.rs`](../../crates/arcweft-core/src/executor.rs).

## Executors

All tiers use the same typed step input, options, and result contract, owned by
[`step.rs`](../../crates/arcweft-core/src/step.rs). The facade also accepts an
adapter-provided `RuntimeCallBackend` for pure helper acceleration, keeping
backend execution outside `arcweft-core` while making it available to ordinary
flow calls; the backend boundary is defined in
[`pure.rs`](../../crates/arcweft-core/src/pure.rs).

Runtime external producers retain the same accepted resource generation as
their executable. Bundle admission binds the CharacterDialogue declaration,
validated Character catalog, accepted View definitions and actual Style digest
to Core's original Native or AWBC program owner. Accelerated headless source
runs, repeated Flow runs, bundle runners and retained sessions use that binding;
numeric acceleration and built-in data adapters keep their normal budgets and
dispatch. A retained session selects a producer by the exact calling executable
lease, including when an older fiber remains live after replacement. An absent
or foreign generation is refused before producing a CharacterDialogue value.
Callable expression results use the same admitted assignment relation as local
bindings and branches: producer-wide opaque and declared Choice results retain
the actual value's exact identity. This applies to function sites, pure helpers
and trait methods. Native source plans bind the existing accepted runtime-plan
artifact key before executable lowering; they do not derive a second identity.

`RuntimeStepStats` and `RuntimePureCallStats` report deterministic per-step
work, as defined in [`step.rs`](../../crates/arcweft-core/src/step.rs). These
counters describe the step even when an adapter retains a pure-helper compile
cache across steps.
For deterministic integer helpers, VM fallback evaluation also consumes the
fixed-stack argument pack directly; `arg_vec_allocations` therefore identifies
runtime call-site argument materialization rather than backend fallback
wrapping. Value-shaped VM fallback takes the caller's evaluated argument vector
by ownership transfer, including affine values, and reuses VM scratch root
bindings between calls.
`arg_bytes_borrowed` counts the backend's temporary slice inspection of that
owned vector; it does not represent a copy of the value graph.
Executor tier lowering preserves the runtime pure-call contract, so ordinary
flow calls can use the adapter-provided backend across the structured VM and
AOT tiers.
Batch helper execution reuses caller-owned slot storage; repeated evaluation
does not clone the compiled plan's local-slot vector for every item.

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

The VM defines the structured runtime's semantic reference. Pure helper AOT and
JIT acceleration is limited to deterministic functions and preserves VM
behavior. Unsupported helpers follow the selected backend's typed rejection or
VM fallback contract. A generated script
backend must implement the same `RuntimeExecutor` boundary. No executor touches
the filesystem, network, renderer, audio device, or wall clock: it returns
`RuntimeEffectBatch` and `HostRequestBatch` data for hosts to handle. See the
[executor API](../../crates/arcweft-core/src/executor.rs),
[pure-call API](../../crates/arcweft-core/src/pure.rs), and
[AOT artifact](../../crates/arcweft-core/src/aot.rs).

## Step options

`RuntimeStepOptions` combines the selected mode and operation budget with a cap
on newly emitted host task requests. Its typed shape and mode enum are
owned by [`step.rs`](../../crates/arcweft-core/src/step.rs).

Runtime execution drains internally according to the selected mode:

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

Native Await and Program fanout treat an accepted, unobserved Need publication
or a terminal local Program launch as internal work. A waiting parent can resume
that work within the same Drain step; it does not need an empty host turn after
its last child retires. Fiber selection and stop detection use that same
readiness, so an idle waiter neither consumes an operation nor hides a runnable
child. Resuming ready work still consumes the normal operation budget.

Retained child scopes remain executor work after the parent finishes, including
detached line work. Local join policy governs the parent return gate and does
not discard that executor ownership. A blocked child at the front of the queue
must not hide a later runnable child. Product execution polls at most the
retained queue length at that dispatch boundary; a blocked poll issues no VM
instruction and does not consume an operation charge.

## Stop reasons

`Blocked` covers runtime states that require later host input, such as choices or
awaited task results. `Output` covers host-visible output in `Game` mode and host
requests that require the adapter to act. `Done` and `Failed` are terminal hard
stops. The stop-reason enum is defined in
[`step.rs`](../../crates/arcweft-core/src/step.rs).

## Effect batch

The step output groups diagnostics, flow events, effect batches, host requests,
and other typed runtime results. [`step.rs`](../../crates/arcweft-core/src/step.rs)
owns the output and batch shapes. `LineEffectRequest` is pure effect data;
hosts perform presentation, audio, logging, signal, metric, and event side
effects outside `arcweft-core`, as defined by
[`effect.rs`](../../crates/arcweft-core/src/effect.rs).

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

The complete host request batch is owned by
[`step.rs`](../../crates/arcweft-core/src/step.rs); it carries typed requests
for host-owned task, content, audio, host-call, presentation, and root work.

## Structured payloads

[`RuntimePayload`](../../crates/arcweft-core/src/value.rs) retains a structured
`RuntimeValue`, and [`RuntimeStreamEvent`](../../crates/arcweft-core/src/stream.rs)
carries typed payloads for external-capability stream events. Display tooling
may derive a payload label, but the runtime boundary preserves the structured
value.

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
