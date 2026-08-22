# Final design

## 1. Precedence and retained substrate

This design applies to production at
`9168c8ac7285c6b44f29018626a0e7c1b0059796`. Current production and maintained
documentation outrank returned package prose. The focused correction request
outranks the rejected coordinator archive.

The following accepted parent decisions are unchanged:

- core owns `RuntimeGenerationJournal`, `JournalTransaction`,
  `SealedJournalAfterImage`, `AppliedJournalBatch`, journal rows, observers,
  accepted-launch receipts, and live Need-handle construction;
- `TaskLaunchAdapter` has concrete associated token types and exactly the
  launch/restore/rebind/cancel prepare/commit/rollback families;
- adapter prepare is fallible; adapter commit and rollback are infallible;
- the scheduler owns one private runtime after-image and performs an
  infallible whole-state swap;
- `RuntimeGenerationJournal::apply_after_image` checks generation/revision
  before mutation and is the last fallible apply operation;
- handles, receipts, and restored values are not observable before core and
  scheduler apply plus adapter commit; and
- all version markers remain `1`, with no compatibility reader.

This correction supplies the missing current owner and two-phase API without
changing those semantics.

## 2. One host-owned generic scheduler

`arcweft-runtime-scheduler` owns the type definition
`RuntimeTaskScheduler<A: TaskLaunchAdapter>`. The crate remains Sans I/O and
continues to depend only on `arcweft-core`; `TaskLaunchAdapter` and all crossing
batch/token wrappers therefore remain core-owned protocols.

One scheduler value owns:

- one concrete adapter `A`;
- one core `RuntimeGenerationJournal`;
- one scheduler-private runtime task state;
- one normalized pending observer-event queue; and
- immutable configuration/limits.

It does not own filesystem/network persistence, a compiler/View catalog,
runtime-driver session state, or a second journal. `RuntimeTaskScheduler<A>` is
not stored inside `BundleSession` and `BundleSession` is not generic over `A`.

Concrete ownership is:

- `NativeTaskBridge` owns
  `RuntimeTaskScheduler<RegistryTaskLaunchAdapter>`;
- the existing `BundleRunnerSession` headless path owns that same concrete
  scheduler through its `NativeTaskBridge` field; it does not add another
  headless scheduler wrapper; and
- `BrowserTaskBroker` owns
  `RuntimeTaskScheduler<BrowserTaskLaunchAdapter>`.

The current immediate `RuntimeScheduler`, driver `RuntimeTaskRegistry`, and
host-specific dispatch queues are deletion inputs, not additional layers.
`SchedulerRuntimeState::pending_events` is the sole event queue. Scheduler
`step` only enqueues normalized observer events and returns counts;
`TaskHost::poll_frame` is the sole drain and removes each event at most once.

## 3. Borrowed driver boundary

The existing `arcweft_core::task::TaskHost` evolves in place. It exposes
ensure, cancellation, observation, accepted event ingress, and polling using
core-owned values. Its associated `Error` may retain a host-specific typed
diagnostic, but no associated adapter type or prepared token appears.

`BundleSession::step_with_task_host` is method-generic over `H: TaskHost` and
borrows `&mut H` for one call. The session struct and its stored fields remain
non-generic. At the start of a step, driver asks the host to ingest
concrete-adapter completions and advance internal runtime tasks, then drains
normalized events once through `poll_frame`; at the end, it submits the exact
task/cancel/observe operations produced by the accepted runtime output. The
obsolete driver task registry,
`HostTaskDispatch`, task generation-pin side map, and returned
`requested_tasks`/`cancel_scopes` queues are removed once all callers use this
boundary.

Driver never imports `arcweft-runtime-scheduler` or `arcweft-host-adapter`.
Native/Web/headless host errors are mapped only at their application boundary;
semantic task identity never depends on an error string.

## 4. Pure decoded state

Outer application/save code reads a complete byte slice before core decoding.
`DecodedRuntimeTaskSnapshotV1` is an untrusted core DTO. It contains snapshot
rows and expected stable identities only. It has no live handle, receipt,
runtime value, observer reference, adapter token, journal row, or mutation
method.

Decode is pure and bounded. It accepts version `1` only, requires canonical
shortest varints, rejects trailing/unknown/duplicate fields, and checks work
limits before allocation. It neither consults an adapter nor mutates a
scheduler.

## 5. Prepared restore guard

`RuntimeTaskScheduler::prepare_restore` borrows `&mut self` and returns
`PreparedRuntimeTaskRestore<'scheduler, A>`. The guard is non-`Clone`,
nonserialized, has private fields, and retains the exclusive scheduler borrow.
It owns exactly:

- one `SealedJournalAfterImage` constructed by the core
  `JournalTransaction`;
- one scheduler-private `SchedulerRuntimeAfterImage`;
- canonical-order `PreparedRestoreBatch<A::PreparedRestoreToken>` values; and
- core-private applied-object construction inputs and preallocated backing
  storage, but no applied object.

Preparation validates the decoded graph through the borrowed
`RuntimeSnapshotAuthority`; that authority supplies the accepted
`TaskValidationAuthority` and exact structured/AWBC/Host/View joins. The
scheduler never names upper task-plan, Match, View, or nominal row types.

Preparation then builds the journal and scheduler after-images, calls adapter
prepare in canonical row order, validates each returned receipt through the
core transaction, and seals only after all receipts are accepted. Any failure
rolls back all earlier prepared adapter batches in reverse order. Dropping an
unapplied guard performs the same exact rollback through its retained
exclusive borrow. A token is consumed exactly once by commit or rollback.

No live handle, accepted-launch receipt, restored `RuntimeValue`, observer
publication, task row, or journal mutation is accessible from the guard.

## 6. Sole commit point and exposure

`PreparedRuntimeTaskRestore::apply` consumes the guard. Its exact order is:

1. call `RuntimeGenerationJournal::apply_after_image`;
2. on generation/revision failure, reverse-roll back prepared adapter batches
   and return the one apply error;
3. on success, swap the scheduler runtime after-image infallibly;
4. commit every prepared adapter batch infallibly in canonical order; and
5. move the core-built `AppliedRuntimeTaskRestore` to the caller.

`SealedJournalAfterImage` contains only private construction inputs and
preallocated backing storage; it contains no live handle, accepted receipt,
restored runtime value, or applied result. After its generation/revision
precheck succeeds, `apply_after_image` swaps the journal and constructs
`AppliedJournalBatch`/`AppliedRuntimeTaskRestore` during that successful apply
using only those prevalidated inputs and preallocated storage.

Step 1 is the last fallible operation. Steps 3–5 perform no allocation,
validation, lookup, callback, formatting, logging, queue reserve, lock
acquisition that can fail, or `Result` conversion. All vector/queue capacity,
construction inputs, observer rows, and event slots are prepared inside the
sealed transaction; live applied objects are issued only by the successful
core apply. The scheduler does not claim a second durable or semantic commit
point.

Core owns `AppliedRuntimeTaskRestore`. It exposes private-field
`RuntimeTaskRestoreReceipt`, restored handles/values, and applied revision only
after adapter commit. A caller cannot construct it from decoded/prepared data
or from a persisted marker.

## 7. Common operation family

Ensure, restore, rebind, and cancel use separate public/private typed guards
because their associated token types differ. They share one normative
transaction transcript:

```text
validate core/catalog/runtime inputs
  -> build core JournalTransaction and scheduler after-image
  -> adapter prepare in canonical order
  -> validate receipts into the same JournalTransaction
  -> seal
  -> core apply (last Result)
  -> scheduler swap (infallible)
  -> adapter commit (infallible)
  -> expose core-built applied result
```

Observer-only registration uses the same core transaction and scheduler swap
without an adapter batch. Rebind rederives generation-bound `TaskKey` and
`TaskId`; it preserves NeedId/launch ordinal only where the accepted parent
replacement rule allows. Cancel preserves the accepted idempotent
`AlreadyRequested` outcome and performs no adapter work for an already
committed cancellation.

No erased `Box<dyn Any>` token, global coordinator, generic durable record, or
parallel runtime graph exists.

## 8. Lifecycle, event, and current outcome

Lifecycle and observer publication are different algebras:

- `TaskLifecycleStage` records accepted/running/terminal progression;
- `TaskLifecycleTransition` records `LaunchAccepted`, `ExecutionStarted`, and
  `CancellationRequested` journal actions; and
- `TaskEventKind` / `TaskEventKindSnapshotV1` contain only `Progress`, `Ready`,
  `InfrastructureFailure`, and `Cancelled`.

`RuntimeNeedCellState` owns pending/current outcome and cursor state. A
progress publication updates its current progress and notifies observers; it
is nonterminal. Ready, infrastructure failure, and cancelled are terminal.
`Accepted`, `Running`, and `CancellationRequested` are never encoded as
observer task-event variants.

Snapshot stores the current lifecycle stage, current Need-cell outcome, and
pending observer event queue separately. Restore rederives their consistency
before adapter prepare. [TRANSACTION_AND_STATE_PROJECTION.md](TRANSACTION_AND_STATE_PROJECTION.md)
is the exhaustive projection table.

## 9. Snapshot and outer persistence boundary

`RuntimeTaskScheduler::snapshot` is a pure borrow that returns
`RuntimeSchedulerSnapshotV1`. It rejects prepared work and active
`MustBeQuiescent` Host rows; it captures complete `Restartable` rows using the
accepted journal/adapter receipt evidence. It performs no I/O.

The CLI/player application owns this sequence:

1. quiesce the driver and host composition;
2. obtain session and scheduler snapshot DTOs;
3. encode and write the complete save through `arcweft-save`; and
4. on restore, read complete bytes, decode/prepare all driver products, then
   prepare/apply the concrete scheduler while holding exclusive outer borrows.

The driver/session after-image is fully constructed before scheduler apply and
its later swap is infallible and not externally reentrant. No worker event is
delivered until the next driver step, so the outer caller observes either the
old complete state on pre-apply failure or the new complete state on return.

There is no durable restore decision record. A process loss before return
leaves no in-memory authority; a later process performs an ordinary restore
from the immutable save snapshot. Persisted bytes never claim that a scheduler
apply occurred.

## 10. Predecessor consumption

Task-plan, View, Match, and accepted structural nominal work remains ordered as
specified in [DEPENDENCIES.md](DEPENDENCIES.md). This design consumes only the
accepted core-facing roles:

- `TaskValidationAuthority` for `TaskSpec` and plan/catalog admission;
- `RuntimeSnapshotAuthority` for complete snapshot/value restore; and
- `ViewTaskPlanAuthority` for retained View validation.

No scheduler method accepts a raw task-plan digest, HIR/compiler ID, View site
placeholder, accepted nominal catalog row, source name, or copied side table.
If a predecessor remains typed fail-closed, restore returns that existing
typed admission error before adapter prepare; the scheduler does not weaken it.

## 11. Readiness

All result-changing choices in this correction are closed.
`OPEN_QUESTIONS.md` is exactly `none`. Implementation must follow the
predecessor and atomic-cut order in [CUTS_TESTS_AND_DELETION.md](CUTS_TESTS_AND_DELETION.md);
the readiness claim does not assert that those production types are already
landed.
