# Lang-01.5.1.1.2.1.1.1.1.1.1.1.1 — runtime task coordinator and two-phase restore correction

Status: `OPEN_DESIGN_REQUEST`

## Parent, split reason, and precedence

This request is a design-gated child of the accepted runtime task persistence,
handle-batch, and snapshot-isomorphism corrections. It does not reopen numeric
AWBC allocation, fixed identity domains, Need producer identity, Match/View
semantic identity, or ownership classification.

The accepted result meaning is retained, but the returned Rust-shaped schemas
do not define one constructible coordinator/restore boundary for the current
repository. Implementing the sketches literally would require choosing between
different public owners and would permit a receipt or Need handle to be minted
from an uncommitted restore after-image.

Current maintained source, the crate map, and the following fixed owner decision
take precedence over older sketches:

```text
runtime-host composition owns RuntimeTaskScheduler<A>
  NativeTaskBridge / Web broker / headless host

runtime-driver does not own a scheduler
runtime-driver borrows &mut impl TaskHost at the step boundary
the obsolete driver RuntimeTaskRegistry is deleted
```

This follows the maintained player/host scheduler-lifecycle direction and the
current `NativeTaskBridge { registry, scheduler }`, windowed host composition,
and Web broker composition. Do not return a driver-owned scheduler, make the
driver/session generic over an adapter, or introduce a trait-object token
erasure layer.

## Current contradictions to close

1. The prose uses `RuntimeTaskScheduler<A: TaskLaunchAdapter>`, while the schema
   only sketches method-generic `impl RuntimeScheduler` functions and supplies
   no constructible public ensure/snapshot/restore owner. The adapter trait has
   associated token types, so an unspecified trait-object bridge is not viable.
2. `RuntimeSnapshotAuthority` is sketched as borrowing a committed journal and
   directly producing runtime values. The accepted transaction rule instead
   forbids receipts and accepted-launch Need handles from an uncommitted
   after-image and requires them to be constructed only by successful apply.
   No two-phase decoded/prepared/applied restore types are specified.
3. Snapshot event rows use lifecycle names such as `Accepted` and `Running`,
   while the final live accepted outcome/state algebra uses `Progress`, `Ready`,
   `InfrastructureFailure`, and `Cancelled`. The mapping and sole persistent
   authority are not defined.
4. Restore, replacement/rebind, cancellation, observer publication, adapter
   prepare/commit/rollback, journal mutation, and receipt construction do not
   share one stated commit point or failure precedence.

## Decisions required

Return one coherent design that closes all of the following without a
compatibility layer.

1. Define the final public `RuntimeTaskScheduler<A>` owned by host composition,
   including exact constructor, ensure, poll/step, cancel, observe, snapshot,
   prepare-restore, apply-restore, and replacement/rebind signatures.
2. Define the narrow borrowed `TaskHost` boundary consumed by runtime-driver.
   State how native, Web, and headless compositions implement it while keeping
   adapter tokens concrete and host-owned.
3. Define a two-phase restore algebra with untrusted decoded rows, a completely
   validated prepared after-image, and a sealed apply result. Decoding and
   preparation must not expose `RuntimeNeedHandle`, `TaskLaunchReceipt`,
   observer mutation, journal mutation, or adapter-private tokens.
4. Define the sole commit point and exact ordering for adapter batch commit,
   scheduler/journal publication, observer state, restored Need handles,
   receipts, and returned runtime values. A failure before the commit point must
   leave every owner byte-for-byte unchanged; a post-commit fallible step is
   forbidden.
5. Reconcile live task state, journal events, snapshot rows, and restored state
   into one exhaustive typed projection. Distinguish an event (`launch
   accepted`, `execution started`) from an accepted terminal/current outcome;
   do not reuse a similarly named enum as both.
6. Define validation and error precedence for version, canonical wire, duplicate
   identity, producer/policy/ordinal rederivation, TaskKey/TaskId/NeedId joins,
   plan and argument digests, View bundle/revision mapping, host catalog routes,
   adapter prepare, quiescence/restart policy, and apply.
7. Define replacement/rebind and cancellation through the same batch/after-image
   authority. NeedId and launch ordinal remain stable only where the accepted
   replacement rules permit; generation, TaskKey, and TaskId must be rederived.
8. Provide a deletion-driven compile-clean sequence that removes the driver
   registry, old String identities, old scheduler/receipt/observer/snapshot
   rows, direct runtime-value restore, and every superseded reader/writer in the
   same atomic public Cut 5 switch.

Every Arcweft-owned version marker remains exactly `1`. Ordinary private Wire
integers use the maintained canonical shortest base-128 varint. Hash-transcript
little-endian widths are not a second wire format. Removed opcode/flag
tombstones remain rejected; numeric allocation is outside this request.

## Required Rust-shaped owners

Names may follow legitimate existing module ownership, but the returned design
must provide equivalent closed roles for:

```rust
pub struct RuntimeTaskScheduler<A: TaskLaunchAdapter> { /* host-owned */ }

pub trait TaskHost {
    // Narrow driver-facing task operations; no associated adapter token leaks.
}

pub struct DecodedRuntimeTaskSnapshotV1 { /* untrusted, no live handles */ }

pub struct PreparedRuntimeTaskRestore<A: TaskLaunchAdapter> {
    // validated scheduler/journal after-image plus sealed adapter batch
}

pub struct RuntimeTaskRestoreReceipt {
    // constructed only by successful apply from committed state
}
```

The return must state constructor visibility and prove which type is allowed to
construct `RuntimeNeedHandle`, `TaskLaunchReceipt`, and restored
`RuntimeValue::NeedHandle` values. A public raw-parts constructor is forbidden.

## Consumers to inventory

- `arcweft-runtime-host` native/Web/headless composition and adapter scheduler;
- `arcweft-runtime-driver` session stepping, replacement, save/restore, and the
  obsolete task registry;
- `arcweft-core` task identity/spec/execution, journal, observer, Need handle,
  RuntimeValue, AWBC, canonical value identity, and snapshot rows;
- compiler/runtime-plan task products and persistent View bundle rows;
- native and Web adapter prepare/commit/rollback tokens;
- player/application save, restore, hot-swap, cancellation, and diagnostics;
- private Wire codecs, generated schemas/fixtures, maintained documentation,
  and structural gates.

## Non-goals

- no production patch or implementation overlay in the returned archive;
- no driver-owned scheduler, adapter-generic driver/session, associated-token
  trait object, global singleton, or copied scheduler side table;
- no receipt or Need handle from decoded/prepared/uncommitted data;
- no string identity bridge, legacy reader, optional old field, V2 type,
  version bump, fallback resolver, or dual journal/snapshot model;
- no reopening of numeric opcode/flag allocation, Need producer instance
  identity, TaskKey/TaskId/NeedId transcripts, or accepted View identity; and
- no post-commit fallible projection or best-effort rollback.

## Required tests

- native, Web, and headless host compositions implement the same borrowed
  driver boundary without erasing adapter tokens;
- decode and prepare expose no handle/receipt/runtime value and mutate no live
  scheduler, journal, observer, registry, or adapter state;
- exact-limit and one-over snapshot/restore batches with deterministic first
  errors and zero partial publication;
- tampered version, noncanonical varint, duplicate/cross-generation IDs,
  producer/policy/ordinal, plan/argument/View admission/revision, host route,
  and unresolved cross-reference negatives;
- adapter prepare failure, commit failure, cancellation race, restartable and
  must-quiesce rows, plus byte-for-byte rollback evidence;
- successful apply publishes scheduler, journal, observers, handles, receipts,
  and runtime values as one isomorphic committed state;
- live state -> snapshot -> decode -> prepare -> apply -> snapshot equality,
  including `Progress`, `Ready`, `InfrastructureFailure`, and `Cancelled`;
- replacement/rebind preserves only accepted stable identities and rederives
  every generation-bound identity;
- compile-fail/exhaustive checks proving the driver registry, old String IDs,
  direct snapshot-to-runtime-value route, legacy readers, and parallel state
  enums are absent; and
- workspace checks, focused scheduler/driver/host/codec tests, deterministic
  generated artifact comparison, Clippy record, and structural gate.

## Required returned archive

Return exactly:

`arcweft-lang-01.5.1.1.2.1.1.1.1.1.1.1.1-runtime-task-coordinator-and-two-phase-restore-correction-final-contract.zip`

The archive must contain the complete final contract, Rust-shaped schemas,
host/driver/adapter ownership and dependency matrices, transaction and restore
state machines, event/state projection table, error precedence, compile-clean
deletion order, exhaustive test matrix, repository-aware validator and negative
self-tests, manifest, source inventory, `FINAL_STATUS`, and `OPEN_QUESTIONS`.
It may claim `READY_FOR_IMPLEMENTATION` only when every decision above is closed
and `OPEN_QUESTIONS` is exactly `none`.
