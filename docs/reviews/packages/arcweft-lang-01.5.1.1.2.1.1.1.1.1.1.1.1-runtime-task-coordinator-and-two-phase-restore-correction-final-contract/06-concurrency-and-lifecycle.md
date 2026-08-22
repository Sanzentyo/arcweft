# Concurrency and lifecycle contract

## 1. Concurrency objective

Readers, scheduler workers, match operations, cancellation, and shutdown may run concurrently with prepare. None may observe a prepared batch. Commit is serialized per coordinator and publishes exactly one immutable root transition.

## 2. Synchronization roles

| Primitive/role | Protects | Does not protect |
|---|---|---|
| `restore_serial` async mutex/gate | one commit/recovery decision at a time; pending publication token | normal task polling/read-only lookup |
| journal append lock | total record order and fsync boundary | runtime task maps |
| published root primitive | atomic old-root/new-root observation | disk I/O or detached construction |
| `pending_publication` mutex | hidden, preconstructed aggregate between durable substeps | public liveness |
| task-local synchronization | execution state of one published task | restore batch identity or coordinator epoch |
| startup/shutdown gate | scheduler open/closed and mandatory recovery completion | snapshot decoding |

## 3. Lock/operation order

Normative order:

```text
startup/shutdown lifecycle read
  → restore_serial
    → journal append lock (released after each synced append)
      → pending_publication lock (short, no I/O)
        → published root write/replace (short, infallible)
          → task-local lock only after publication when normal scheduling begins
```

Rules:

- never acquire `restore_serial` from a task-local lock;
- never perform snapshot read/decode, fsync, allocator-heavy construction, task poll, callback, or `.await` while the published-root write guard is held;
- journal implementation may await I/O while `restore_serial` is held because the gate serializes restore decisions, but it must not hold runtime map/task locks;
- capacity for runnable admission and indexes is reserved before the durable commit point;
- lock poisoning/panic behavior follows the runtime crate's existing policy; no restore-only silent recovery.

## 4. Reader visibility

Every read API captures one `PublishedRuntimeTaskRoot` guard/snapshot and resolves task, handle generation, and match substrate from that same root. It may not load each independently, because that could mix epochs.

A lookup concurrent with publication has exactly two legal results:

1. old epoch: restored task absent and old match substrate;
2. new epoch: restored task present with corresponding handle batch and match substrate.

“Task present, handle absent” and “new match state with old task index” are forbidden.

## 5. Concurrent restore attempts

| Attempt A | Attempt B | Outcome |
|---|---|---|
| prepare | prepare | may run concurrently because both are detached; each captures base epoch |
| commit token X | commit token X/same digest | one performs work, other waits/observes and returns same receipt |
| commit token X | commit token X/different digest | token-digest corruption |
| commit token X | commit token Y | serialize; Y rechecks epoch and normally returns stale/conflict, then must re-prepare |
| recovery commit X | caller commit X | recovery owns gate; caller receives/waits for same stable receipt |
| normal task admission | restore commit | whichever changes epoch first wins; other rechecks and either merges only if explicitly supported or rejects stale |

This contract chooses **reject stale and re-prepare**, not implicit merge. It keeps digest, handle slots, and match coverage deterministic.

## 6. Cancellation

- Cancellation before `commit_restore` begins: drop prepared value, no effect.
- Cancellation while waiting for `restore_serial`: return cancellation/busy according to existing runtime convention, no effect.
- Cancellation before synced `COMMITTED`: operation may return cancellation only after ensuring no publication and a reducer-valid journal state.
- Cancellation after synced `COMMITTED`: suppressed for the critical completion; the future may detach an internal completion guard only if shutdown/startup recovery owns it. It must not return a rollback-looking cancellation result.
- Cancellation of an individual restored task is admitted only after the batch is published. A cancellation racing publication resolves against one root epoch and is never applied to a detached cell.

## 7. Shutdown

Shutdown sets the lifecycle gate before accepting new commits.

- prepare already running may finish but its result will fail commit with `CoordinatorShuttingDown` unless shutdown policy explicitly allows startup restore;
- commit before durable decision stops cleanly with no publication;
- durable committed work is mandatory: shutdown either completes publication and then cancels tasks normally, or records/retains a replayable committed state before process exit;
- scheduler workers do not begin normal polling on startup until all committed restore records are published;
- optional journal compaction can be skipped during shutdown without correctness loss.

## 8. Panic and fatal invariant policy

Any panic before durable commit unwinds/drops detached state and leaves no visible batch. A panic after durable commit is not converted to a normal `RestoreCommitError`; the process/coordinator enters fatal `PublishRequired` recovery. This avoids returning an error while durable truth says success.

The implementation should minimize the post-commit span to non-panicking root replacement, pre-reserved queue splice, and atomic state update. No formatting, logging allocation, user hook, or debug assertion that can panic belongs there.

## 9. Memory reclamation

Atomic publication may leave old roots alive for concurrent readers. Reclamation is reference/epoch based using the existing root primitive. Restored task cells are not placed into both the pending and published owners after the move; the pending slot becomes `None` in the same critical section. No self-referential pointer may depend on a `Vec` address before final boxing/pinning strategy is applied.

## 10. Observability

Metrics/events are emitted at semantic boundaries:

- `restore_prepare_started/completed/rejected` — detached only;
- `restore_journal_prepared_synced`;
- `restore_commit_decided` — after COMMITTED sync;
- `restore_batch_published` — includes restore ID, epoch, task count, digest prefix;
- `restore_replayed` and crash-point/recovery cause;
- `restore_conflict` with non-sensitive reason.

Metrics must not count tasks as live before `restore_batch_published`. Logs never include captured runtime values or complete digests where project policy treats them as sensitive.
