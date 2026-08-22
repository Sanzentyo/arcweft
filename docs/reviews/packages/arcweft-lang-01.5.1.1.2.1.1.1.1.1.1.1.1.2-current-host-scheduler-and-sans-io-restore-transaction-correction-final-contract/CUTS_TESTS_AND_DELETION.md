# Cuts, tests, and deletion

## 1. Compile-clean implementation graph

These are ordered subcuts of the accepted atomic Cut 5 public switch. A task
branch may compile between private subcuts, but no provisional public contract
is integrated to `main` before the deletion set is complete.

### Prerequisite P — typed semantic authorities

Land and validate the accepted outputs required by
`TaskValidationAuthority`/`RuntimeSnapshotAuthority`:

1. `.1.2` complete generic Match/path authority;
2. `.1.4` retained View operation/product, after `.1.2`;
3. `.1.3.1` final task-plan owner/seal, after `.1.4`;
4. `.1.1.1.1` accepted structural nominal carrier; and
5. accepted parent Cut 4 identity/catalog substrate.

No scheduler-local placeholder receives partial credit for this prerequisite.

### Subcut A — core protocol and state algebra

In `arcweft-core`:

- publish the accepted parent journal, transaction, sealed after-image, applied
  proof, Need handle, receipt, observer, adapter batch/token, validation, and
  snapshot authorities;
- place `TaskLaunchAdapter` beside its core batch types;
- evolve `TaskHost` to the exact borrowed boundary in `SCHEMAS.md`;
- replace `TaskEventKind::Failed(String)` and snapshot event variants with the
  four-outcome algebra;
- add distinct lifecycle transition/stage and Progress Need-cell state;
- add pure `DecodedRuntimeTaskSnapshotV1` codec and applied restore result; and
- keep all version markers at `1`.

Gate: core check/tests/trybuild, snapshot codec goldens and negatives, journal
transaction tests, enum exhaustiveness, and dependency graph proving core has
no upward/I/O edge.

### Subcut B — generic scheduler and prepared guards

In `arcweft-runtime-scheduler`:

- replace/fold current `RuntimeScheduler` into
  `RuntimeTaskScheduler<A: TaskLaunchAdapter>`;
- retain deterministic join/dispatch/event normalization under the new state;
- implement complete runtime after-image construction and infallible swap;
- implement ensure and observer transactions;
- implement decoded → prepared guard → applied restore;
- implement rebind and cancel guards with operation-matched Drop rollback;
- reserve every result/event/queue capacity before journal apply; and
- add structural proof that only core apply returns `Result` after prepare.

Gate: scheduler focused tests, compile-fail guard privacy/non-Clone tests,
fault hooks around every prepare/apply boundary, and Cargo metadata proving the
crate still depends only on core.

### Subcut C — concrete adapter facades and host composition

In `arcweft-host-adapter`, `arcweft-runtime-host`, and
`arcweft-player-web`:

- replace immediate registry submit/boolean cancel with a concrete
  registry-backed `TaskLaunchAdapter` implementation;
- migrate `NativeTaskBridge` to
  `RuntimeTaskScheduler<RegistryTaskLaunchAdapter>`;
- retain the existing `BundleRunnerSession -> NativeTaskBridge` headless path;
  do not add a second headless scheduler wrapper;
- migrate `BrowserTaskBroker` to
  `RuntimeTaskScheduler<BrowserTaskLaunchAdapter>`;
- implement the same core `TaskHost` on all three compositions; and
- keep worker completion collection outside `TaskLaunchAdapter`, converting
  completions to canonical core events before scheduler step.

Gate: native/Web/headless API parity, concrete-token compile tests, adapter
prepare/rollback/commit order tests, and absence of a driver task DTO import in
host code.

### Subcut D — driver borrowing and deletion

In `arcweft-runtime-driver`:

- replace `step_with_clock` task routing with `step_with_task_host`;
- call `step_tasks`, drain once through `poll_frame` before the runtime step,
  and submit ensure/observe/cancel operations afterward in source order;
- remove `RuntimeTaskRegistry`, `HostTaskDispatch`, task generation-pin map,
  requested-task/cancel-scope output queues, and duplicate list/cancel APIs;
- route task observation diagnostics through the host/journal authority; and
- update hot-swap/replacement to call the concrete outer host rebind path.

Gate: driver check/tests, no dependency on scheduler/host-adapter/runtime-host,
and native/Web/headless end-to-end step parity.

### Subcut E — outer snapshot/save/restore

In CLI/native/Web application composition and existing session persistence:

- quiesce both session and concrete scheduler;
- build session and task snapshot DTOs without I/O callbacks;
- encode/write only after both snapshots succeed;
- read complete bytes before decode;
- build the complete session after-image before task prepare;
- hold exclusive session/host borrows through scheduler apply and infallible
  session swap; and
- deliver no worker event until the next step.

Gate: live → snapshot → bytes → decoded → prepared → applied → snapshot
equality, I/O call tracing, MustBeQuiescent/Restartable tests, and injected
pre-apply failure proving both live owners unchanged.

### Subcut F — final deletion and maintained documentation

Delete all items in the deletion inventory, update maintained scheduler/save
docs and generated v1 fixtures, then run the selected broad validation tiers.
No old overload/reader remains.

## 2. Same-cut deletion inventory

| Superseded item/path | Final action |
|---|---|
| `arcweft_runtime_scheduler::RuntimeScheduler` as nongeneric public owner | replace/fold into `RuntimeTaskScheduler<A>`; no alias |
| method-generic accepted-parent scheduler apply coordinator | replace with stored concrete `A` and prepared guards |
| core old `TaskHost { ensure_task -> TaskHandle, cancel_scope, poll_frame -> Vec }` | replace in place with final borrowed boundary |
| current `TaskHandle` surrogate | delete; `RuntimeNeedHandle` is sole Await carrier |
| `arcweft_runtime_driver::task::RuntimeTaskRegistry` | delete file/type/tests after host migration |
| `HostTaskDispatch` and driver dispatch sequencing | delete; core correlation/journal/scheduler own identity/order |
| `BundleSession.tasks` and `task_generation_pins` | delete |
| `BundleSessionStep.requested_tasks` and `cancel_scopes` | delete |
| driver task list/cancel duplicate authority | delete or project through borrowed host diagnostics owner, never retain rows |
| Browser cancelled-scope and queued-event authority duplicating scheduler | delete; retain only concrete adapter resources/completion source |
| `HostAdapter::submit`, `complete`, registry immediate dispatch, `cancel(&TaskId)->bool` timing | delete with direct-start semantics |
| `TaskEventKind::Failed(String)` | replace with typed `InfrastructureFailure` |
| snapshot event `Accepted`, `Running`, `CancellationRequested`, `InfrastructureFailed` | delete; lifecycle/current outcome owners replace them |
| scheduler/task current state without Progress | replace with exhaustive current outcome projection |
| session restore `RuntimeTaskRegistry::default()` reset | delete; outer scheduler restore owns task state |
| `TaskPersistence`, `TaskRestoreJournal`, durable restore records | absent/delete from any implementation attempt |
| `RuntimeTaskCoordinator`, coordinator ID/epoch, pending-publication root | absent/delete |
| PREPARED/COMMITTED/APPLIED_ACK codec/fixtures/replay | absent/delete |
| async/dyn persistence API inside scheduler | absent/delete |
| adapter commit error or post-apply queue/recovery error | absent/delete |
| old snapshot reader, migration map, optional old field, V2/V3 type | absent/delete |
| task-plan/View/nominal stand-in row or digest | absent/delete; consume accepted authority only |

## 3. Focused test matrix

### Ownership and dependency

- `native_task_host_owns_concrete_scheduler`
- `bundle_runner_headless_path_uses_native_bridge_concrete_scheduler`
- `browser_task_host_owns_concrete_scheduler`
- `all_compositions_implement_same_core_task_host`
- `runtime_driver_step_is_method_generic_not_session_generic`
- `runtime_driver_has_no_scheduler_or_host_adapter_dependency`
- `runtime_scheduler_depends_only_on_core`
- `core_has_no_scheduler_host_save_or_io_dependency`
- compile-fail: `TaskHost` implementation cannot expose an adapter token;
- compile-fail: no `dyn TaskLaunchAdapter` prepared-token erasure path.

### Decode and limits

- exact version `1` accepts; `0`, `2`, and overlong version encodings reject;
- shortest varint accepts and each overlong integer rejects;
- outer bytes, rows, nodes, depth, transcript bytes, fields, observers, events,
  and cross-reference exact-limit/one-over fixtures;
- unknown/duplicate field, unknown tag, integer overflow, truncated row, and
  trailing byte each return the selected first error;
- allocation counter proves no allocation before the corresponding checked
  count/length succeeds;
- every failure returns no `DecodedRuntimeTaskSnapshotV1`.

### Prepared typestate and visibility

- compile-fail field construction/read of decoded rows and prepared guards;
- compile-fail `Clone`, `Copy`, Serde, raw-parts, and token extraction;
- decoded/prepared values expose no handle, receipt, restored RuntimeValue,
  journal row, observer mutation, or task lookup;
- a prepared guard holds the exclusive mutable scheduler borrow;
- dropping a guard rolls every token back once in reverse order;
- applying a guard leaves Drop as a no-op;
- no use-after-apply or double apply compiles.

### Adapter preparation

- prepare failure at first, middle, and last source row;
- receipt missing, duplicate, reordered, wrong generation/correlation/route,
  wrong launch/cancellation capability, and foreign token cases;
- all prior tokens roll back reverse-order and the failing token follows its
  adapter-defined failed-prepare ownership rule;
- journal/scheduler/observer/Need bytes equal their before-images on every
  prepare error;
- re-preparing after rollback yields identical canonical route/capability
  allocation absent intervening committed work.

### Last-fallible apply

- stale generation and stale revision at `apply_after_image`;
- core and scheduler after-images remain unchanged on apply error;
- tokens reverse-roll back on apply error;
- fault hooks prove the trace is exactly core apply → scheduler swap → adapter
  commit → exposure;
- sealed/prepared state contains construction inputs and reserved storage but
  no live handle, receipt, restored value, or applied result;
- successful core apply issues `AppliedRuntimeTaskRestore` without allocation;
- allocation, hash, catalog, lock, callback, formatting, logging, queue-reserve,
  and panic hooks have zero calls after successful core apply;
- source inspection/typed structural gate rejects any post-apply `?`, `Result`,
  adapter commit return, or fallible helper;
- worker-visible count is zero before adapter commit and exactly one after;
- handles/receipts/values cannot be observed before commit.

### Operation-family parity

- ensure/restore/rebind/cancel use their matching associated token type;
- cross-family token substitution fails to compile;
- all four share identical apply-order trace;
- observer-only transaction uses no adapter call;
- already-requested cancellation performs no adapter work;
- mixed cancellation prepares only new cancellable Host rows;
- rebind rederives generation/TaskKey/TaskId and preserves NeedId/ordinal only
  for accepted retained producers;
- stale/extra/missing replacement mapping rejects before prepare.

### Lifecycle/outcome/event/snapshot

- launch accepted and execution started mutate lifecycle only and emit no task
  outcome event;
- Progress remains nonterminal and may be followed by later Progress/terminal;
- Ready, InfrastructureFailure, and Cancelled are terminal exactly once;
- terminal-to-progress, terminal replacement, duplicate cursor, and cursor
  regression reject;
- live ↔ snapshot differential for all six Need-cell states and all four event
  variants;
- pending event and current cell must match cursor/value/failure;
- no snapshot/event enum contains Accepted, Running,
  CancellationRequested, Failed, or InfrastructureFailed;
- event order differential fixes `(logical_epoch, task_id, sequence)` across
  native/Web/headless.
- scheduler `step` enqueues and returns counts only; `poll_frame` is the sole
  drain, so every event is delivered exactly once.

### Authority and semantic joins

- producer/policy/ordinal/NeedId/TaskKey/TaskId tampering;
- plan/argument/outcome/Host operation/route/restart/cancellation tampering;
- stale/missing View program/product/revision mapping;
- unsupported Match/task-plan/nominal predecessor remains typed fail-closed
  before adapter prepare;
- scheduler snapshot/restore imports no HIR/sema/compiler/View row type;
- authority differential: one accepted product change alters exactly its
  expected validation result, not scheduler identity rules.

### Snapshot and outer I/O

- snapshot rejects a live prepared guard by borrow construction/compile proof;
- active MustBeQuiescent Host row blocks snapshot;
- complete Restartable row snapshots/restores through adapter prepare;
- application reads all bytes before decode and writes only after both session
  and scheduler snapshots complete;
- traced persistence calls are zero from decode through applied result;
- pre-apply injected failure leaves session and scheduler byte-for-byte equal;
- successful task apply followed by infallible session swap exposes one new
  complete state and delivers no worker event until next step;
- process restart performs ordinary snapshot restore and finds no WAL/replay
  record.

## 4. Structural negative gate

The repository-aware gate must parse Rust/Cargo structure, not rely on source
placement alone. It fails when a mutation:

- adds a driver dependency on scheduler/host-adapter/runtime-host;
- adds a scheduler dependency beyond core;
- adds an I/O/persistence parameter to core/scheduler restore;
- introduces any forbidden symbol from `SCHEMAS.md` section 10;
- makes a prepared/core protocol field `pub` or `pub(crate)`;
- derives Clone/Serde for a prepared guard;
- removes Drop rollback or changes reverse ordering;
- makes adapter commit/rollback return `Result`;
- moves receipt/handle/value construction after journal apply;
- reintroduces an old event variant, old reader, migration, or version other
  than `1`;
- preserves `RuntimeTaskRegistry` or `HostTaskDispatch`; or
- introduces a scheduler-local task-plan/View/nominal catalog.

## 5. Validation tiers for production

The final cross-crate public switch requires, without an explicit Cargo job
count:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

It additionally requires focused core/scheduler/host/driver/Web/native tests,
trybuild compile fixtures, deterministic private-Wire goldens, maintained
structure audit/gate, applicable runtime Tier 2 targets, and exact generated
artifact comparison.

This documentation cut did not run production fmt/check/Clippy/tests because
it changes no production Rust. Performed design validation is reported by the
handoff, not inferred from this planned command list.
