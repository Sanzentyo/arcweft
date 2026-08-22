# Lang-01.5.1.1.2.1.1.1.1.1.1.1.1.2 — current host scheduler and Sans-I/O restore transaction correction

Status: `RESOLVED_BY_ACCEPTED_DESIGN`

## Parent, rejected return, and precedence

This is a focused correction of the still-open
[`runtime task coordinator and two-phase restore correction`](2026-08-22-lang-01.5.1.1.2.1.1.1.1.1.1.1.1-runtime-task-coordinator-and-two-phase-restore-correction.md).
It consumes, without reopening, the accepted
[`runtime launch receipt, keyed ordinal, and current-owner design`](../designs/lang-01.5.1.1.2.1.1.1.1.1.1.1.1-runtime-launch-receipt-keyed-ordinal-and-current-owner/README.md).

The returned coordinator archive is not accepted. It had no inspected Git SHA
or source inventory, targeted a nonexistent `arcweft-runtime` crate, omitted
the required host-owned generic scheduler and borrowed `TaskHost`, and made a
new durable `TaskPersistence` journal the semantic commit authority. Its
synced `COMMITTED` record preceded core/runtime publication and therefore
created a committed-but-unapplied state requiring crash replay. That ordering
contradicts the accepted core `JournalTransaction` /
`SealedJournalAfterImage` / `apply_after_image` authority and cannot recover
concrete adapter-private prepared tokens after a process loss.

Current source at full Git commit
`9168c8ac7285c6b44f29018626a0e7c1b0059796`, maintained documentation, this
request, and the accepted parent design take precedence over the rejected
archive. Accepted task-plan, View, Match, and nominal authorities remain
predecessors; this correction may consume their typed lower protocols but may
not invent stand-in identities, rows, or digests.

## Fixed correction direction

Return one coherent design with these non-negotiable choices:

1. `RuntimeTaskScheduler<A: TaskLaunchAdapter>` is the final scheduler type in
   `arcweft-runtime-scheduler`; native, Web, and headless host compositions own
   concrete instances. `arcweft-runtime-driver` never owns a scheduler and is
   not generic as stored state over `A`.
2. The existing core `TaskHost` evolves into the narrow driver-facing borrowed
   boundary. A driver step borrows `&mut impl TaskHost`; no adapter token or
   associated adapter type crosses this trait.
3. Snapshot bytes are read and written by the outer application/session/save
   owner. Core and the scheduler remain Sans I/O. No `TaskPersistence`,
   `TaskRestoreJournal`, durable `PREPARED`/`COMMITTED` record, fsync, recovery
   replay obligation, or disk-selected runtime truth may exist.
4. Restore uses three typed states: untrusted decoded snapshot, fully validated
   and adapter-prepared restore, and applied result. Decoded/prepared states
   expose no live `RuntimeNeedHandle`, launch receipt, restored runtime value,
   observer mutation, journal mutation, or public task row.
5. `PreparedRuntimeTaskRestore<A>` retains the concrete
   `A::PreparedRestoreToken` through the accepted `PreparedRestoreBatch` and
   owns exactly one core sealed journal after-image plus one scheduler-private
   runtime after-image. Abandonment and every pre-apply error roll adapter
   reservations back exactly once in reverse order.
6. Restore, ensure, replacement/rebind, and cancellation use the same typed
   transaction ordering with operation-specific token wrappers. No generic
   trait-object token erasure or copied scheduler/journal side table exists.
7. The last fallible operation is
   `RuntimeGenerationJournal::apply_after_image`. Failure rolls all prepared
   adapter tokens back and leaves journal and scheduler runtime unchanged.
   Success is followed only by an infallible scheduler after-image swap and
   infallible adapter commits in canonical order.
8. Applied handles, accepted-launch receipts, restored runtime values, observer
   effects, and public restore receipts are exposed only after both infallible
   post-apply operations complete. There is no allocation, validation lookup,
   callback, queue admission that can fail, logging, formatting, or `Result`
   edge after successful journal apply.
9. Lifecycle transitions (`launch accepted`, `execution started`, cancellation
   requested) are not observer outcome events. Live/event/snapshot outcome
   projection is exhaustively `Progress`, `Ready`,
   `InfrastructureFailure`, and `Cancelled`; pending/lifecycle state is stored
   in its own typed owner.
10. All error precedence, quiescence/restart checks, generation/revision
    rechecks, adapter rollback ordering, constructor visibility, and
    native/Web/headless ownership paths are exact and testable.

Every Arcweft-owned version marker remains exactly `1`. Private Wire integers
use the canonical shortest base-128 varint. There is no old reader, migration
record, optional compatibility field, `V2` type, or version bump.

## Decisions required

The returned design must close:

1. exact crate ownership, fields, constructors, and all public methods of
   `RuntimeTaskScheduler<A>`;
2. the exact evolved core `TaskHost` API and runtime-driver step signature;
3. native, Web, and headless concrete composition and adapter ownership;
4. decoded, prepared, and applied restore schemas and visibility;
5. abandonment-safe ownership of concrete prepared adapter tokens;
6. the common ensure/restore/rebind/cancel transaction state machine;
7. event, lifecycle, current outcome, Need-cell, and snapshot projection;
8. deterministic decode/prepare/apply error precedence and first-row order;
9. the pure snapshot/outer persistence boundary and application-level
   quiescent restore orchestration; and
10. compile-clean deletion of the driver registry, dispatch DTOs, immediate
    Host adapter submission/cancel API, durable-journal proposal, old event
    variants, old readers, and every duplicate scheduler state owner.

## Consumers to inventory

- `arcweft-core` task identities, `TaskHost`, validation/snapshot authorities,
  journal/Need/observer rows, runtime values, and snapshot DTOs;
- `arcweft-runtime-scheduler` current Sans-I/O state and final generic
  transaction coordinator;
- `arcweft-host-adapter` current registry and final concrete adapter facade;
- `arcweft-runtime-host` `NativeTaskBridge` and headless composition;
- `arcweft-player-web` `BrowserTaskBroker` and application loop;
- `arcweft-runtime-driver` step, task registry, generation pins, cancellation,
  save/restore, and hot-swap;
- outer CLI/player save I/O and `arcweft-save` codecs; and
- accepted task-plan/View/Match/nominal authorities, generated fixtures,
  maintained runtime docs, and structural dependency gates.

## Non-goals

- no production patch or implementation overlay in the returned archive;
- no driver-owned scheduler, adapter-generic stored driver/session, global
  coordinator, trait-object token erasure, or scheduler side table;
- no durable restore WAL, crash publication replay, I/O inside core/scheduler,
  or persistence-selected commit point;
- no handle, receipt, runtime value, observer mutation, or live row from
  decoded/prepared state;
- no post-apply failure, panic-capable hook, best-effort rollback, or adapter
  commit error;
- no redesign of task-plan, View, Match, nominal, identity, numeric AWBC, or
  canonical value authorities absent a repository-evidenced flaw; and
- no compatibility reader, alias, fallback, migration, version bump, or
  optional old field.

## Required implementation order

1. land every accepted typed predecessor consumed by `TaskValidationAuthority`
   and `RuntimeSnapshotAuthority` without creating scheduler-local stand-ins;
2. publish the core journal, snapshot typestates, adapter batches, and final
   `TaskHost` in the atomic Cut 5 boundary;
3. replace the scheduler with `RuntimeTaskScheduler<A>` and operation-specific
   prepared guards;
4. migrate native, headless, and Web compositions to concrete adapters;
5. move runtime-driver task traffic to a borrowed `TaskHost` and delete its
   registry/dispatch state;
6. integrate pure outer snapshot/restore orchestration; and
7. delete all superseded paths and validate the dependency graph in the same
   reviewable cut.

## Required tests

- native/Web/headless implementations of one borrowed `TaskHost`, with no
  driver edge to scheduler/adapter crates and no adapter-token erasure;
- decoded and prepared compile-fail/private-field tests proving no live
  handle/receipt/value/row construction;
- prepare failure at every adapter row, prepared-guard drop, journal
  generation/revision conflict, reverse rollback, and byte-for-byte unchanged
  pre-apply state;
- fault hooks proving `apply_after_image` is the final `Result` and all work
  after it is allocation-free, validation-free, callback-free, and infallible;
- ensure/restore/rebind/cancel order and token-family differentials;
- lifecycle/outcome/event/snapshot exhaustive projection and round trip for
  `Progress`, `Ready`, `InfrastructureFailure`, and `Cancelled`;
- exact-limit/one-over decode work, noncanonical varint, version, duplicate,
  generation/identity/catalog/View/plan/argument, quiescence, and restart
  negatives with deterministic precedence;
- outer save I/O tests proving bytes are complete before decode and no
  persistence call occurs during prepare/apply; and
- structural negatives for durable WAL symbols, legacy readers, version bumps,
  driver registry, immediate adapter submit/cancel, post-apply `Result`, and
  copied scheduler/journal models.

## Required returned archive

Return exactly:

`arcweft-lang-01.5.1.1.2.1.1.1.1.1.1.1.1.2-current-host-scheduler-and-sans-io-restore-transaction-correction-final-contract.zip`

The archive must contain the request copy, current full Git SHA and source
inventory, final design, exact Rust-shaped schemas, transaction and state
projection, dependency matrix, decision register, compile-clean cuts/deletions,
test matrix, repository-aware validator and negative self-tests, manifest with
member hashes, `FINAL_STATUS`, and `OPEN_QUESTIONS.md`. It may claim
`READY_FOR_IMPLEMENTATION` only when every result-changing choice is closed
and `OPEN_QUESTIONS.md` is exactly `none`.
