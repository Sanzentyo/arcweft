# Product AWBC `RuntimeStepResult` Parity Contract

## Status and scope

This document defines the durable product-runtime contract for executing canonical
AWBC through the shared `RuntimeStepResult` boundary. Canonical AWBC remains the
only executable payload of a decoded Game-product AWFB. Structured bytecode
remains a development/source compatibility tier and is not consulted as product
executable truth.

The contract is Sans I/O. `arcweft-core` advances deterministic state and emits
typed requests. Runtime-driver, runtime-host, and player adapters perform host
work and return typed results in a later step.

## Ownership

| Concern | Owner | Reason |
|---|---|---|
| Canonical executable tables, IDs, resume points, maps | `arcweft-core::awbc` | AWBC is a core data/runtime ABI. |
| Compact fiber registers, frames, suspension state | `arcweft-core::awbc::fiber` | The state is part of canonical execution, not host orchestration. |
| Opcode and terminator execution | `arcweft-core::awbc::vm` | VM behavior is independent of product hosts. |
| Projection into `RuntimeStepResult` | `arcweft-core::awbc::product_step` | One adapter owns ordering, status, requests, diagnostics, and statistics. |
| Runtime-plan to AWBC lowering | `arcweft-runtime-plan::awbc_lower` | Lowering requires inventory, source maps, display maps, and diagnostics. |
| I/O and capability fulfillment | runtime host/player adapters | Core emits data; adapters perform side effects. |

`arcweft-bundle` remains a data/codec crate. It neither lowers source nor drives
execution.

## Entry ABI and root bindings

The selected `AwbcEntry` identifies a function signature. Before the first
instruction executes, the product executor transactionally binds the effective
root bindings:

1. constructor-supplied root bindings establish defaults;
2. `RuntimeStepInput::bindings` override bindings with the same name;
3. if every provided binding name matches a parameter name, bindings are treated
   as named-equivalent arguments;
4. otherwise an equally sized list is positional;
5. missing, extra, duplicate, unknown, or type-invalid arguments produce a typed
   input/type diagnostic without mutating registers;
6. a later step may retry with corrected bindings.

Call frames use the same verified positional parameter binding rule. Tail calls
replace the active frame only after argument validation succeeds.

## Pure helpers and intrinsics

Product execution receives the caller-provided `RuntimeCallBackend` through the
same facade method as structured execution.

- Intrinsics are resolved from their canonical AWBC public identity and routed
  through the existing runtime call evaluator.
- Pure helpers expose a `RuntimeCompactPureHelper` descriptor containing stable
  helper ID, public name, arity, and scalar-evaluation capability.
- A backend may return `Some(result)` from `call_compact_values` to select an
  accelerated implementation.
- Returning `None` selects the verified compact-function fallback.
- Backend failures remain deterministic runtime failures.
- Per-step backend deltas and compact-fallback counters are combined with
  saturating arithmetic in `RuntimeStepStats::pure`.

No product adapter reconstructs a structured helper expression and no product
player depends on compiler crates.

## Explicit progression

Dialogue and choice never auto-resume.

### Dialogue

A dialogue terminator:

- emits the line event and mapped line effects once;
- records `FlowFiberStatus::Dialogue` with line identity and task-group state;
- starts eligible line-task nodes in deterministic node order;
- returns `RuntimeStepStopReason::Output` when presentation or host work is
  emitted;
- resumes only after an explicit routed input whose trigger denotes dialogue
  advance and whose optional payload matches the active line;
- runs cancel/cleanup nodes exactly once when progression cancels the active
  line-task group.

### Choice

A choice terminator:

- evaluates every option guard in declaration order;
- presents the complete filtered option vector with public IDs, labels, and
  mapped effects;
- stores the option-to-table-index mapping in the active choice state;
- exposes `FlowFiberStatus::Choice` until explicit selection;
- accepts selection by public ID or label and optionally checks the choice public
  ID carried by the routed input;
- diagnoses invalid or stale selections without resuming;
- writes the selected canonical value and resumes exactly once.

## Await and await-many

`AwbcTaskPlan` carries both a stable task public ID and a stable need ID. This is
part of AWBC codec version 1.

Single await:

- emits a task request at most once for a stable task ID;
- preserves task and need IDs in flow events/status;
- consumes normalized progress, ready, error, and cancellation events;
- binds ready values through the suspended pattern transactionally;
- retains the suspension on non-terminal progress;
- maps terminal errors/cancellation to deterministic diagnostics and status.

Await-many:

- owns item order, result slots, next index, in-flight entries, and concurrency
  limit in compact fiber state;
- fills free slots in ascending item index order;
- correlates events by stable task ID;
- retains partial results and deterministic progress;
- resumes only when the configured completion rule is satisfied;
- writes the result sequence in input item order.

## Host calls and effects

`RuntimeStepInput` carries typed `RuntimeHostCallResult` values and
`HostRequestBatch` carries typed `RuntimeHostCallRequest` values. A request has a
stable generated ID, capability, operation, arguments, mode, and deterministic
flag.

- Immediate mode may be fulfilled from the current input or suspends if no
  matching result is available.
- Suspending mode always returns control to the host until a matching result is
  supplied.
- Stale results do not resume a different call.
- Unsupported, rejected, and failed outcomes retain their typed error kind and
  become host/capability diagnostics.

Every `AwbcEffectKind` is handled explicitly by its inherent mapping method.
Existing line-effect variants are projected directly. Effects requiring a host
capability but lacking a typed payload produce a typed unsupported-capability
diagnostic; no variant is silently dropped.

## Content and stream state

Ensure-content observations are de-duplicated by canonical content ID and emit a
typed request containing public content identity and resource metadata.

External capability operations returning `Stream<T, E>` are ordinary host-call
requests. Capability adapters normalize permission, cancellation, queue, and
replay behavior before returning typed stream events; the product step does not
own a second Source state machine or handler table.

Stream behavior:

- every stream owns a monotonic sequence counter;
- yield and close observations are projected in VM observation order;
- close is idempotent in both compact and facade state;
- resuming after a budget or host suspension does not re-emit prior stream
  observations.

## Budget and stop reasons

Host `max_ops` and compact fiber quantum are separate limits.

- The product adapter asks the VM to execute one compact instruction at a time so
  output and suspension boundaries can be observed without duplication.
- Fiber quantum may cause the VM to emit `BudgetYield`; host `max_ops` limits the
  amount of work performed by one `RuntimeStepResult` call.
- `OneOp` returns after one attempted operation.
- drain/game/server modes stop at visible output, host request, blocking
  suspension, terminal state, or host budget exhaustion according to their
  policy.
- `BudgetYield` returns `BudgetExhausted`, resumes through its canonical resume
  point on the next step, replenishes the quantum, and does not replay prior
  observations.

The only valid stop reasons are `OneOp`, `Output`, `Blocked`,
`BudgetExhausted`, `Done`, and `Failed`.

## Traps, maps, and partial output

VM runtime failures are converted into a typed `FiberTrap`. A trap records the
compact source-map ID attached to the current block/instruction when available.
The product adapter resolves that ID through AWBC source tables and produces a
`RuntimeDiagnostic` containing:

- stable category (`Input`, `Type`, `Pattern`, `Host`, `Capability`, `Budget`,
  `Internal`, or ordinary runtime);
- message;
- source label;
- byte range;
- optional source anchor.

Output already emitted before a verifier-safe runtime trap is retained in the
same `RuntimeStepResult`; the fiber becomes failed and cannot accidentally
resume through a stale host event.

## Statistics and facade state

After every step, the adapter synchronizes the shared `FlowFiber` facade:

- status;
- root environment;
- observation state;
- stream states;
- line cursor.

`RuntimeStepStats` reports executed operations, pending work before/after, child
fibers, pure/backend counters, input event counts, emitted stream/line and audio
counts, and diagnostics. Counts are derived after output projection so
host-visible vectors and counters cannot disagree.

## Product safety gates

A product merge must retain all of these gates:

1. decoded Game-product AWFB execution selects `ArcweftExecutionTier::AwbcProduct`;
2. product execution never reads `bundle.bytecode.program` as executable truth;
3. product hosts do not construct `BytecodeVmExecutor` directly;
4. `arcweft-bundle` remains Sans I/O and compiler-independent;
5. ordinary source fixtures lower to AWBC with an empty product-step blocker
   inventory;
6. every unblocked family has differential coverage at the
   `RuntimeStepResult` boundary.
