# Transaction and state projection

## 1. Restore typestate

The public semantic states are:

```text
complete bytes
  -> DecodedRuntimeTaskSnapshotV1             (untrusted, pure)
  -> PreparedRuntimeTaskRestore<'_, A>        (validated, reserved, not applied)
  -> AppliedRuntimeTaskRestore                (applied and exposed)

PreparedRuntimeTaskRestore --Drop--> reverse adapter rollback
PreparedRuntimeTaskRestore --apply conflict--> reverse adapter rollback
```

There is no persisted intermediate state. `Decoded` and `Prepared` are Rust
typestates, not Wire record kinds. A process loss discards both; a later process
starts a new ordinary restore from the immutable snapshot.

## 2. Pure decode precedence

Decode is deterministic first-error. The following order is normative:

1. outer byte-count limit;
2. version value, which must equal `1`;
3. canonical field/tag framing and shortest varints;
4. checked integer conversion and per-field length limits;
5. total rows/nodes/depth/transcript-byte work limits;
6. unknown fields/tags;
7. duplicate fields within one row;
8. trailing bytes.

The decoder processes fields and source-ordered rows in byte order. Within one
precedence class, the lowest byte offset wins. It performs checked `u64`
accounting before allocation/descent. A failure returns no partial decoded
value.

## 3. Restore preparation transcript

Preparation holds the scheduler's exclusive mutable borrow. It runs these
stages in order:

1. consume `DecodedRuntimeTaskSnapshotV1`;
2. validate scheduler configuration and exact active generation;
3. construct/validate `RuntimeSnapshotAuthority` joins, including exact
   structured plan, AWBC program, Host catalog, and retained View authority;
4. validate snapshot row order, uniqueness, and all local work limits;
5. rederive producer instance, policy/ordinal, NeedId, TaskKey, and TaskId for
   each row in source index order;
6. validate plan/argument/outcome/Host route/View mapping and complete restored
   values through the authority;
7. validate task/Need/observer/scope and runtime-task cross-references;
8. validate lifecycle/current-outcome/pending-event consistency;
9. validate `MustBeQuiescent` versus complete `Restartable` Host rows;
10. begin one core `JournalTransaction` and call its restore planner;
11. construct the complete scheduler-private runtime after-image and reserve
    every post-apply queue/result allocation;
12. call `TaskLaunchAdapter::prepare_restore` in canonical batch/source order;
13. validate each inspectable adapter receipt against its exact input batch and
    accept it into the same core transaction; and
14. seal the core journal after-image and return the prepared guard.

Failures in stages 1–11 perform no adapter call and mutate no live journal or
scheduler state. Failure at adapter row `i` rolls back all successfully
prepared rows `< i` in reverse order. Receipt or seal failure also rolls back
the complete prepared list in reverse order. Adapter rollback is infallible and
restores the adapter's externally reserved state exactly.

Preparation never obtains an accepted-launch receipt from the staged
after-image. The sealed image retains only private construction inputs and
preallocated backing storage. It never constructs or exposes a live handle,
restored `RuntimeValue`, observer mutation, applied receipt, or applied result.

## 4. Apply transcript

The prepared guard owns the exclusive scheduler borrow, so no recheck can race
except the generation/revision comparison deliberately retained by the core
apply proof.

```text
PreparedRuntimeTaskRestore
  -> RuntimeGenerationJournal::apply_after_image
       Err: journal unchanged
            scheduler runtime unchanged
            reverse adapter rollback
            return JournalApplyError
       Ok:  core journal and observer/Need/task rows swapped atomically;
            core constructs AppliedJournalBatch from prevalidated inputs
            with preallocated storage and no further failure edge
  -> RuntimeTaskScheduler::apply_runtime_after_image       [infallible]
  -> TaskLaunchAdapter::commit_restore, canonical order    [infallible]
  -> return core-built AppliedRuntimeTaskRestore           [move only]
```

`apply_after_image` is the last `Result`. Its internal generation/revision
check happens before mutation. After success, the remaining path contains:

- no allocation or capacity growth;
- no hashing/digest computation;
- no catalog, map, generation, or receipt validation;
- no conversion from a receipt into a handle;
- no lock acquisition that can poison/fail;
- no user callback, worker poll, task body, wake delivery, or event-loop
  reentry;
- no formatting, logging, metrics exporter, or debug assertion that can panic;
- no adapter `Result`; and
- no persistence call.

`AppliedRuntimeTaskRestore` is a core-owned private-field value issued during
that successful apply. The scheduler only retains and later returns it; it
does not construct or validate it.

Metrics observe the operation only after the applied result is returned to the
host composition. They cannot become a post-apply failure edge.

## 5. Prepared-guard abandonment

Each prepared guard stores its transaction in `Option`. Commit takes the value
before journal apply. `Drop` takes it only when still present and invokes the
operation-matched rollback in reverse order. Therefore:

| Path | Token consumption | Live state |
|---|---|---|
| prepare error before token | none | unchanged |
| prepare error after tokens | reverse rollback once | unchanged |
| guard dropped | reverse rollback once | unchanged |
| journal apply error | reverse rollback once | unchanged |
| journal apply success | canonical commit once | complete new state |
| guard drop after apply takes transaction | no-op | complete new state |

`mem::forget`, unsafe lifetime extension, token extraction, public fields, and
`Clone` are forbidden by API and structural tests.

## 6. Common operation matrix

| Operation | Core planner | Adapter batch/token | Applied result |
|---|---|---|---|
| ensure | `JournalTransaction::ensure_task` | `PreparedLaunchBatch<A::PreparedLaunchToken>` | `AppliedEnsureResult` / live handle |
| restore | `JournalTransaction::plan_restore` | `PreparedRestoreBatch<A::PreparedRestoreToken>` | `AppliedRuntimeTaskRestore` |
| rebind | `JournalTransaction::plan_rebind` | `PreparedRebindBatch<A::PreparedRebindToken>` | `AppliedRuntimeTaskRebind` |
| cancel | `JournalTransaction::plan_cancel` | `PreparedCancelBatch<A::PreparedCancelToken>` | `RuntimeTaskCancelReceipt` |
| observe | journal observer planner | none | `TaskObserverId` |

Every adapter-bearing operation uses the prepare → receipt validation → seal →
apply → scheduler swap → commit ordering. Separate wrappers prevent a restore
token from being supplied to rebind/cancel/launch. The common transcript is an
implementation invariant, not a public erased transaction enum.

## 7. Lifecycle versus observer outcome

Lifecycle answers where execution is. Current outcome answers what observers
can read. Observer events announce outcome publications. These roles are not
interchangeable.

| Trigger | Lifecycle transition | Current Need-cell state | Observer `TaskEventKind` | Terminal |
|---|---|---|---|---:|
| accepted launch committed | `LaunchAccepted` → `Accepted` | `Pending` | none | no |
| worker/runtime execution begins | `ExecutionStarted` → `Running` | unchanged | none | no |
| progress publication | stage remains `Running` | `Progress { cursor, progress }` | `Progress(progress)` | no |
| successful result | stage → `Terminal` | `Ready { cursor, value }` | `Ready(value)` | yes |
| infrastructure failure | stage → `Terminal` | `InfrastructureFailure { cursor, failure }` | `InfrastructureFailure(failure)` | yes |
| cancellation requested | `CancellationRequested` (stage remains accepted/running until terminal) | `CancellationRequested` | none | no |
| cancellation completes | stage → `Terminal` | `Cancelled { cursor }` | `Cancelled` | yes |

`TaskLifecycleTransition` belongs to the core journal transaction. It is not a
driver-visible task event and is not encoded in the pending observer event
queue. `TaskLifecycleStage` is the folded current stage stored on the task row.

## 8. Live/snapshot projection

| Live owner | Snapshot owner | Restore rule |
|---|---|---|
| `TaskLifecycleStage` | same closed enum field on `TaskJournalRowSnapshotV1` | exact value, then cross-check against Need state |
| `RuntimeNeedCellState::Pending` | `Pending` | only Accepted/Running task stage |
| `Progress` | `Progress` with same cursor/value | Running only; cursor greater than previous publication |
| `Ready` | `Ready(AwbcRuntimeValueSnapshot)` | value restores through outer `RuntimeSnapshotAuthority`; Terminal only |
| `InfrastructureFailure` | same typed failure | Terminal only |
| `CancellationRequested` | same state | Accepted/Running; matching cancel journal row required |
| `Cancelled` | same cursor | Terminal only |
| pending `TaskEventKind::Progress` | `TaskEventKindSnapshotV1::Progress` | nonterminal and exact cursor ordering |
| pending `Ready` | snapshot `Ready` | matches terminal cell value identity |
| pending `InfrastructureFailure` | same failure | matches terminal cell failure |
| pending `Cancelled` | `Cancelled` | matches terminal cell cursor |

There is no snapshot event variant named `Accepted`, `Running`,
`CancellationRequested`, `Failed`, or `InfrastructureFailed`. The selected
spelling is `InfrastructureFailure` in live and snapshot algebras.

Pending events are ordered by `(logical_epoch, task_id, sequence)`. A retained
generation collection prefixes `generation` before that tuple. Duplicate keys,
cursor regression, terminal-to-nonterminal transitions, multiple terminal
outcomes, and event/cell disagreement reject during preparation before adapter
work.

## 9. Restore semantic error precedence

After pure decode succeeds, preparation is first-error in this order:

```text
configuration/scalar limits
< active generation and snapshot authority construction
< canonical row order and duplicate identity
< producer instance / policy / launch ordinal rederivation
< NeedId / TaskKey / TaskId correlation joins
< plan and argument digests
< View program/product admission and accepted revision evidence
< Host operation/catalog/route/restart/cancellation contract
< payload/outcome/runtime value admission
< task/Need/observer/scope/runtime cross-references
< lifecycle/current outcome/pending event projection
< quiescence/restart completeness
< scheduler after-image construction and capacity reservation
< adapter prepare, source index order
< adapter receipt validation, source index order
< journal seal
< journal apply generation/revision
```

Within a row family, lowest canonical source index wins. Work-limit errors take
precedence at the exact point where the next checked unit would exceed the
limit; diagnostic counters never alter semantic success.

An adapter prepare/receipt error reports the failing input source index only
after rollback completes. Rollback failure is unrepresentable because rollback
has no return type and is required not to panic.

## 10. Replacement/rebind

Preparation validates old and new `TaskValidationAuthority` values and the
accepted replacement plan before constructing either after-image.

- generation-bound TaskKey/TaskId/correlation values are rederived;
- NeedId and launch ordinal survive only for the exact accepted retained
  producer instance and replacement class;
- Host operation/capability pairs are re-prepared through
  `prepare_rebind`;
- observers move only through journal-planned accepted mappings;
- stale/missing/extra mappings reject before adapter prepare; and
- old and new live graphs cannot coexist through a side table.

After successful core apply, the scheduler swap replaces the complete runtime
map; adapter commits then activate the already prepared bindings. No runtime
lookup can select an old row under a new generation.

## 11. Cancellation

The request is canonical-order, duplicate-free, and generation exact.
`JournalTransaction::plan_cancel` determines dispositions before adapter
prepare.

- `AlreadyRequested` is returned without adapter work;
- noncancellable rows reject before adapter work;
- a mixed batch prepares only newly requested cancellable Host rows;
- apply failure rolls those tokens back in reverse order; and
- successful apply makes journal/Need/observer cancellation state visible,
  then the scheduler swap and adapter commits complete infallibly.

Cancellation cannot target a decoded/prepared restore value because the
prepared guard holds the scheduler's exclusive mutable borrow and exposes no
live identity.

## 12. Outer save/restore state machine

Outer application composition, not scheduler/core, owns I/O:

```text
Running
  -> Quiescing
  -> PureSnapshotBuilt
  -> SaveBytesEncoded
  -> bytes written by application

complete bytes read by application
  -> SessionDecoded + TaskSnapshotDecoded
  -> SessionAfterImagePrepared
  -> PreparedRuntimeTaskRestore<'_, ConcreteAdapter>
  -> task core/scheduler/adapter apply
  -> infallible session after-image swap
  -> Running
```

The outer coordinator holds exclusive session and host borrows from task
prepare through the session swap. It does not poll workers, dispatch UI events,
or expose a receipt in that interval. Failure before core apply leaves both
live owners unchanged. After core apply, the task path and session swap are
infallible, so the caller observes one complete new state on return.

No save file contains `PREPARED`, `COMMITTED`, applied ACK, coordinator epoch,
restore token, or a claim that an in-memory restore was published.
