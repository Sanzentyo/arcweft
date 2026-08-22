# State machine and persistence protocol

## 1. Semantic state machines

### 1.1 Prepared object

`Building → Validated → ConsumedByCommit` or `Building/Validated → Dropped`.

Only `Validated` can be returned. `ConsumedByCommit` is represented by Rust move semantics, not a mutable boolean.

### 1.2 Durable journal reducer

| Prior state | Record | New state | Validity |
|---|---|---|---|
| none | `PREPARED(id,digest,...)` | prepared | valid |
| prepared same id/digest | identical `PREPARED` | prepared | idempotent duplicate may be coalesced |
| prepared same id/digest | `COMMITTED(id,digest,epoch)` | committed | valid |
| committed same id/digest | identical `COMMITTED` | committed | idempotent replay |
| any state | same id, different digest | corrupt | hard error |
| committed | `PREPARED` | corrupt/out-of-order | reject |
| none | `COMMITTED` | corrupt/missing prepare | reject unless a versioned migration explicitly permits a self-contained commit record |

An optional `APPLIED_ACK` may support compaction/metrics. It is not a third decision phase and must never be required to prove the restore was committed.

## 2. Two-phase sequence

```mermaid
sequenceDiagram
    participant Caller
    participant C as RuntimeTaskCoordinator
    participant P as TaskPersistence/Journal
    participant R as Published Root
    participant Q as Runnable Queue

    Caller->>C: prepare_restore(snapshot, context)
    C->>P: read snapshot envelope
    P-->>C: canonical bytes
    C->>C: decode + verify seals/digests/graph/match
    C-->>Caller: PreparedTaskRestore (no visibility)
    Caller->>C: commit_restore(prepared)
    C->>C: acquire restore_serial; recheck epoch/conflicts
    C->>P: append+sync PREPARED
    C->>C: install hidden pending aggregate
    C->>P: append+sync COMMITTED
    C->>R: atomic complete-root publication
    C->>Q: enqueue runnable seed / release deferred wakes
    C-->>Caller: stable RestoreReceipt
```

## 3. Canonical journal grammar

The logical grammar is normative. Physical encoding must use arcweft's existing canonical encoder, semantic-child framing, integer endianness, digest, and seal primitives.

```text
RestoreJournalEnvelope(v1) :=
    domain_tag("arcweft.runtime.task.restore-journal")
    schema_version(u16 = 1)
    record_kind(u8)
    semantic_payload_len(canonical_u64)
    semantic_payload(bytes)
    payload_digest(existing Digest type)
    envelope_seal(existing Seal type)

RestorePreparedPayload(v1) :=
    restore_id(16 bytes)
    coordinator_id(canonical coordinator identity)
    base_epoch(canonical u64)
    snapshot_id(canonical snapshot identity)
    snapshot_digest(existing digest)
    catalog_digest(existing digest)
    abi_digest(existing digest)
    feature_set(canonical sorted feature IDs)
    task_count(canonical u32)
    batch_digest(existing digest)
    handle_batch_digest(existing digest)
    match_coverage_seal(existing seal)

RestoreCommittedPayload(v1) :=
    restore_id(16 bytes)
    coordinator_id(canonical coordinator identity)
    base_epoch(canonical u64)
    published_epoch(canonical u64 = base_epoch + 1)
    snapshot_id(canonical snapshot identity)
    batch_digest(existing digest)
    prepared_record_digest(existing digest)
```

Rules:

- no map/hash iteration order enters bytes; sequences are canonical slot order;
- reserved bits/fields must be zero and rejected otherwise;
- unknown record kinds and unknown mandatory features are errors;
- length arithmetic is checked before allocation;
- duplicate semantic child labels are rejected;
- trailing bytes are rejected;
- the same domain tags are used by encode, decode, digest, and golden tests;
- decode limits task count, plan depth, capture size, and transcript size using existing repository limits.

## 4. Snapshot normalization

Supported old snapshot versions decode into `NormalizedTaskRestoreSnapshot`. Normalization is pure and records the source version. All current validation and digest recomputation operate on the normalized semantic form. Unknown/newer versions fail before coordinator mutation. No migration writes back into the source snapshot during prepare.

## 5. Atomic publication construction

The fallible constructor runs before the durable commit point:

```rust
let publishable = PublishableRuntimeTaskBatch::try_from(prepared)?;
```

It performs allocations, index construction, handle wiring, and all assertions that can fail. After the synced commit record, only these operations remain:

```rust
let old = coordinator.publication.replace(publishable.into_published(epoch));
coordinator.runnable.admit_prevalidated(runnable_seed);
```

`replace` must have the repository's existing atomic root semantics. `admit_prevalidated` must not allocate or fail; reserve its capacity before durable commit. Old roots stay alive through `Arc`/read guards until readers finish.

## 6. Crash-point matrix

| ID | Injected crash point | Durable journal on restart | Was batch externally visible before crash? | Required restart action | Duplicate execution risk |
|---|---|---|---:|---|---|
| CP-00 | before snapshot read | none | no | no-op/retry from caller | none |
| CP-01 | during/truncated snapshot read | none | no | reject corruption or retry I/O | none |
| CP-02 | after decode, before validation complete | none | no | discard detached allocations | none |
| CP-03 | after validated prepare returned, before commit | none | no | caller may re-prepare | none |
| CP-04 | while appending PREPARED before sync | absent or torn record | no | truncate invalid tail; retry commit | none |
| CP-05 | after PREPARED sync | PREPARED | no | verify source/digest; resume or safely abandon | none |
| CP-06 | after hidden pending install | PREPARED | no | memory is gone; reconstruct from snapshot | none |
| CP-07 | while appending COMMITTED before sync | PREPARED or valid COMMITTED | no | journal reducer determines branch; never guess | none |
| CP-08 | after COMMITTED sync, before root swap | COMMITTED | no | mandatory publish on startup before normal scheduling | none if token/digest checked |
| CP-09 | immediately after atomic root swap | COMMITTED | yes in old process only | rebuild/publish same batch on startup | task polling prevented until queue seed step; process crash stops old execution |
| CP-10 | after runnable seed, before success reply | COMMITTED | yes | same token returns same receipt; no second admission | prevented by published token+epoch |
| CP-11 | after success reply, before optional ACK/compaction | COMMITTED | yes | ordinary replay/idempotent lookup | none |

Fault injection must occur at explicit hooks around each boundary; sleep-based race tests are insufficient.

## 7. Recovery algorithm

```text
for each coordinator restore journal in canonical order:
    reduce and validate records
    if state == PREPARED:
        expose no task; retain/retry according to startup policy
    if state == COMMITTED:
        read referenced snapshot
        run the same prepare validation
        require equal restore_id and batch_digest
        build publishable aggregate
        publish exactly once before opening normal scheduler admission
        synthesize/return the stable receipt for duplicate callers
only then transition coordinator startup gate to Ready
```

Recovery does not trust a persisted “runtime handle pointer”. It deterministically reconstructs handles and task cells from semantic identity.

## 8. Replay and idempotency keys

The primary idempotency key is `(CoordinatorId, RestoreId)`. `RuntimeTaskBatchDigest` is the equality witness. `SnapshotId` alone is insufficient because the same immutable snapshot may be intentionally restored into different coordinator instances only through an explicit migration/rebinding operation.

A `RestoreReceipt` can be recomputed from the committed record; it does not require a separately persisted success reply.

## 9. Failure cleanup

- Prepare failure/drop: destroy detached allocations, zeroize only fields already designated secret by existing types, no journal write.
- PREPARED write failure: no hidden install/publication; retry is safe.
- COMMITTED write failure: hidden candidate must not publish. It may remain cached only under the same restore gate and token; otherwise drop and reconstruct.
- Durable COMMITTED: rollback is forbidden. Mark `PublishRequired` and complete publication/recovery.
- Publication/root invariant failure after durable commit is a process-fatal internal invariant breach, because returning a normal error would lie about durable authority. Design makes this path unreachable via preconstruction.

## 10. Limits and denial-of-service resistance

All counts and lengths are bounded before allocation. Nested plan/capture/match depth uses the existing decoder budget. Restore never trusts `task_count` for unchecked `Vec::with_capacity`, never multiplies lengths without checked arithmetic, and never logs unbounded captured payloads on error.
