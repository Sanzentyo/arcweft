# Event order and restartable snapshot policy

## Order key

For one active generation:

```text
(logical_epoch, task_id, sequence)
```

For a shared pending collection spanning retained generations:

```text
(generation, logical_epoch, task_id, sequence)
```

Comparison is lexicographic in exactly that order. `sequence` is a per-task
tie-breaker after `TaskId`, not a global priority over task identity.

The same tuple is used by:

- live pending-event `BTreeMap` keys;
- normalization of adapter/runtime events;
- replay merge;
- generation snapshots;
- restore validation;
- deterministic test fixtures; and
- machine data.

A snapshot with sequence before TaskId rejects `InvalidTaskEventOrderVersion1`.

## Prepared transaction barrier

Any prepared but uncommitted launch, restore, rebind or cancel token blocks
snapshot. The scheduler reports a typed `PreparedAdapterTransaction` snapshot
barrier and does not ask the adapter to serialize reservations. This keeps the
snapshot boundary before prepare or after commit/rollback, never inside it.

## Host row policy

| Host row state/policy | Snapshot decision |
|---|---|
| terminal | persist terminal journal/Need state; no active launch |
| active `MustBeQuiescent` | block snapshot |
| active `Restartable` | persist complete original TaskSpec, complete correlation, operation catalog join, launch capability and cross-references |
| prepared transaction of either policy | block snapshot |

There is no rule that rejects every active or nonterminal Host task.

## Restartable restore transaction

1. decode and validate the entire generation snapshot without publication;
2. validate `next_observer_id`, task ordinal counters, event order and every
   cross-reference;
3. regenerate AwaitMany children and reusable-handle TaskSpecs;
4. validate each restartable Host row against the current same-cut operation
   catalog;
5. prepare all Host restore route groups, collecting owned tokens;
6. on refusal, roll back tokens in reverse order and discard decoded after-images;
7. atomically install journal/runtime/Need/observer/scope/event after-images;
8. commit all Host restore tokens infallibly; and
9. expose the restored generation.

An actual I/O failure after commit is a later
`TaskEvent::InfrastructureFailure`. It does not roll the snapshot restore back
or become a domain `Result`/`Option` payload.
