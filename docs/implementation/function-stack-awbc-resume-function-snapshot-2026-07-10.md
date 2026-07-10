# Function stack AWBC resume and function snapshots — 2026-07-10

## Outcome

Dynamic `ApplyFunction` now enters the target AWBC function on the same
`FiberState` call stack as a static call. The caller stores the exact next
instruction cursor and destination register. A callee may therefore cross an
Await, host call, explicit safe point, or automatic budget preemption without
using a synchronous child fiber or a hidden instruction budget.

Product session saves now preserve AWBC-backed `RuntimeValue::Function` values.
The previous blanket rejection was removed rather than versioned or retained as
a compatibility path; no released save consumer exists. Structured-expression
function bodies remain invalid in a Product AWBC fiber because that VM cannot
execute them.

## Resume model

- Static calls resolve their declared resume point into the same exact
  `FiberReturnPoint` representation used by dynamic calls.
- Dynamic calls store the caller cursor immediately after `ApplyFunction`.
- Declared suspension terminators retain `FiberResumeTarget::Declared`.
- Automatic instruction-budget preemption retains
  `FiberResumeTarget::Exact`, preventing replay of the instruction at which the
  fiber was preempted.
- Partial application remains a value operation and does not push a frame.
- Over-application is rejected before the callee is entered.

## Snapshot validation

Every runtime function reachable through registers, cleanup arguments,
suspensions, await-many state, queues, terminal values, tuples, records,
sequences, variants, iterators, and nested captures is checked against the
artifact's canonical AWBC program. Validation covers:

- function-table and frame-layout identity;
- signature and parameter-slot arity;
- stable capture/remaining-parameter names and ordering;
- parameter-slot/signature type agreement;
- capture value types;
- recursively captured function values.

`FiberState::validate_for_program` owns this traversal and also checks the
declared runtime types of cleanup, host-call, await-many, source, and stream
payloads. Session save/load only adds the Product executor path to the typed
fiber error; it does not maintain a second value walker. This keeps direct
executor snapshot restoration and bundle-session restoration on the same
acceptance boundary.

The AWBC verifier now requires `MakeFunction` binding names and capture types to
agree with the target function frame. This makes a reordered same-typed capture
detectable during save import instead of silently changing closure meaning.

## Validation

```bash
cargo test -p arcweft-core
cargo test -p arcweft-runtime-codegen
cargo test -p arcweft-runtime-driver --test awbc_product_session session_save_
cargo check -p arcweft-core -p arcweft-runtime-codegen -p arcweft-runtime-driver --all-targets --all-features
cargo clippy -p arcweft-core -p arcweft-runtime-codegen -p arcweft-runtime-driver --all-targets --all-features
cargo fmt --all -- --check
git diff --check
```

Focused regressions cover Await/host-call/budget suspension inside dynamic
callees, exact automatic-budget resume, execution beyond the removed 4096-op
limit, partial and over-application, `MakeFunction` name mismatch, captured
function save/import round-trip, structured-body rejection, and stale function
ids.
