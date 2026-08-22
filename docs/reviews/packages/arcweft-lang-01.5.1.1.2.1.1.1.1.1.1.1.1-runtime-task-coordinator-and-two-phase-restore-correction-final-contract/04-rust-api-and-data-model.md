# Rust API and data model

## 1. Placement and visibility

The names below are normative. If current source already owns an equivalent enum/type under a different module, add the variants/methods to that original owner and provide a narrow re-export; do not duplicate the type.

| Item | Visibility | Owner |
|---|---|---|
| `RuntimeTaskCoordinator` | `pub(crate)` or existing runtime-public boundary | `crates/arcweft-runtime/src/task/coordinator.rs` |
| restore methods | same `impl RuntimeTaskCoordinator` | coordinator owner module; implementation may delegate to private sibling module |
| `PreparedTaskRestore` | `pub(crate)`, fields private, non-`Clone` | coordinator restore module |
| `RuntimeTaskRestoreSnapshot` decoder | existing persistence visibility | `crates/arcweft-runtime/src/task/persistence.rs` |
| restore journal record variants | existing project-owned persistence enum/`impl` | persistence owner module |
| prepared/published handle batch conversion | existing batch owner | `crates/arcweft-runtime/src/task/handle.rs` |
| match detached builder/seal | existing match owner | `crates/arcweft-runtime/src/task/match_substrate.rs` |

## 2. Coordinator API

```rust
impl RuntimeTaskCoordinator {
    /// Phase A. Reads and validates a persisted batch without changing live state.
    pub(crate) async fn prepare_restore(
        &self,
        persistence: &dyn TaskPersistence,
        snapshot: RuntimeTaskSnapshotId,
        context: &RuntimeTaskRestoreContext,
    ) -> Result<PreparedTaskRestore, RestorePrepareError>;

    /// Phase B. Consumes the prepared batch and publishes it atomically.
    pub(crate) async fn commit_restore(
        &self,
        persistence: &dyn TaskPersistence,
        prepared: PreparedTaskRestore,
    ) -> Result<RestoreReceipt, RestoreCommitError>;

    /// Convenience only; preserves the exact two-phase implementation and errors.
    pub(crate) async fn restore(
        &self,
        persistence: &dyn TaskPersistence,
        snapshot: RuntimeTaskSnapshotId,
        context: &RuntimeTaskRestoreContext,
    ) -> Result<RestoreReceipt, TaskRestoreError> {
        let prepared = self
            .prepare_restore(persistence, snapshot, context)
            .await
            .map_err(TaskRestoreError::Prepare)?;
        self.commit_restore(persistence, prepared)
            .await
            .map_err(TaskRestoreError::Commit)
    }
}
```

The convenience method is not a separate semantic path. Tests must prove it delegates exactly once to the same prepare and commit machinery.

## 3. Prepared typestate

```rust
#[must_use = "a prepared restore must be committed or deliberately dropped before any durable decision"]
pub(crate) struct PreparedTaskRestore {
    coordinator_id: RuntimeTaskCoordinatorId,
    restore_id: RestoreId,
    base_epoch: CoordinatorEpoch,
    source_snapshot: RuntimeTaskSnapshotId,
    source_digest: RuntimeTaskSnapshotDigest,
    batch_digest: RuntimeTaskBatchDigest,
    canonical_feature_set: RuntimeFeatureSet,
    tasks: Box<[DetachedRuntimeTask]>,
    identity_index: PreparedTaskIdentityIndex,
    handles: PreparedRuntimeHandleBatch,
    match_substrate: PreparedRuntimeMatchSubstrate,
    runnable_seed: Box<[RuntimeTaskId]>,
    journal_payload: PreparedRestoreJournalPayload,
}
```

Required trait properties:

- no `Clone` or `Copy`;
- `Debug` must redact captured values and persistence payloads;
- `Send` only if every detached field already satisfies the runtime's cross-thread contract;
- do not add an unsafe manual `Send`/`Sync` implementation;
- fields remain private so no caller can extract a handle or task cell before commit;
- `Drop` releases detached allocations only and performs no I/O.

## 4. Identity, handle, and receipt types

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct RestoreId([u8; 16]);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct CoordinatorEpoch(u64);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct RuntimeTaskIdentity {
    pub task_id: RuntimeTaskId,
    pub generation: RuntimeTaskGeneration,
}

pub(crate) struct PreparedRuntimeHandleSlot {
    identity: RuntimeTaskIdentity,
    canonical_slot: u32,
    capability: RuntimeTaskCapability,
}

pub(crate) struct PreparedRuntimeHandleBatch {
    slots: Box<[PreparedRuntimeHandleSlot]>,
    digest: RuntimeHandleBatchDigest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RestoreReceipt {
    pub restore_id: RestoreId,
    pub snapshot: RuntimeTaskSnapshotId,
    pub batch_digest: RuntimeTaskBatchDigest,
    pub published_epoch: CoordinatorEpoch,
    pub task_count: u32,
}
```

Public handles are minted from `PreparedRuntimeHandleSlot` only inside `PublishedRuntimeTaskBatch::try_from_prepared`. That constructor verifies slot/index equality and returns an error before the durable commit point. The post-commit publication operation is therefore infallible.

## 5. Match substrate API

```rust
impl RuntimeMatchSubstrate {
    pub(crate) fn prepare_from_snapshot(
        snapshot: &PersistedMatchSnapshot,
        tasks: &PreparedTaskIdentityIndex,
        carriers: &RuntimeCarrierCatalog,
    ) -> Result<PreparedRuntimeMatchSubstrate, RestorePrepareError>;
}

impl PreparedRuntimeMatchSubstrate {
    pub(crate) fn verify_complete_transcript_and_seal(
        self,
        expected: MatchCoverageSeal,
    ) -> Result<Self, RestorePrepareError>;

    fn publish(self) -> RuntimeMatchSubstrate;
}
```

`publish` is private and infallible. Missing behavior on an arcweft-owned carrier/match enum is implemented in that enum's original `impl`.

## 6. Persistence API

```rust
#[allow(async_fn_in_trait)] // use the repository's existing async-trait convention instead if one exists
pub(crate) trait TaskPersistence {
    async fn read_task_snapshot(
        &self,
        id: RuntimeTaskSnapshotId,
    ) -> Result<PersistedTaskSnapshotEnvelope, TaskPersistenceReadError>;

    async fn inspect_restore(
        &self,
        coordinator: RuntimeTaskCoordinatorId,
        restore_id: RestoreId,
    ) -> Result<Option<DurableRestoreState>, TaskPersistenceReadError>;

    async fn append_restore_prepared(
        &self,
        record: &RestorePreparedRecord,
    ) -> Result<JournalPosition, TaskPersistenceWriteError>;

    async fn append_restore_committed(
        &self,
        record: &RestoreCommittedRecord,
    ) -> Result<JournalPosition, TaskPersistenceWriteError>;
}
```

Do not introduce this trait if current source already has a persistence owner. Extend its original trait/enum and `impl` instead. The signatures above specify semantics and data flow, not permission to create duplicate abstractions.

## 7. Error taxonomy and mapping

```rust
#[derive(Debug)]
pub(crate) enum TaskRestoreError {
    Prepare(RestorePrepareError),
    Commit(RestoreCommitError),
}

#[derive(Debug)]
pub(crate) enum RestorePrepareError {
    SnapshotRead(TaskPersistenceReadError),
    UnsupportedVersion { found: u16 },
    UnsupportedFeature(RuntimeFeature),
    FramingCorruption,
    ChecksumMismatch,
    SnapshotDigestMismatch,
    CatalogDigestMismatch,
    AbiDigestMismatch,
    PlanSealMismatch { task: RuntimeTaskIdentity },
    InvalidTaskReference { owner: RuntimeTaskIdentity, target: RuntimeTaskIdentity },
    DuplicateTaskIdentity(RuntimeTaskIdentity),
    HandleBatchNotIsomorphic,
    MatchTranscriptIncomplete,
    MatchCoverageSealMismatch,
    RuntimeCarrierRejected,
    TaskIdentityConflict(RuntimeTaskIdentity),
    CoordinatorMismatch,
}

#[derive(Debug)]
pub(crate) enum RestoreCommitError {
    CoordinatorShuttingDown,
    RestoreBusy,
    StaleCoordinatorEpoch { prepared: CoordinatorEpoch, current: CoordinatorEpoch },
    TaskIdentityConflict(RuntimeTaskIdentity),
    RestoreTokenDigestMismatch,
    JournalRead(TaskPersistenceReadError),
    JournalWrite(TaskPersistenceWriteError),
    InternalInvariant(RuntimeTaskRestoreInvariantError),
}
```

Mapping rules:

- corruption, unsupported schema/features, plan/match/carrier rejection: prepare error, no live mutation;
- epoch/identity race discovered after prepare: commit error, no durable commit and no publication;
- failed `PREPARED`/`COMMITTED` append: commit I/O error; no visibility;
- after a synced `COMMITTED` record, failures are not returned as an ordinary rollback-capable error. The coordinator records `publish_required`, completes in a cancellation-shielded section, or terminates so startup replay completes it;
- user task code never receives persistence corruption as `Result<T, E>`/`Option<T>`; it is a runtime admission failure.

## 8. Internal coordinator fields

```rust
struct RuntimeTaskCoordinatorInner {
    id: RuntimeTaskCoordinatorId,
    epoch: AtomicU64,
    lifecycle: AtomicCoordinatorLifecycle,
    restore_serial: AsyncMutex<RestoreSerialState>,
    publication: PublishedRuntimeTaskRoot,
    pending_publication: Mutex<Option<PendingTaskPublication>>,
    runnable: RuntimeRunnableQueue,
}

enum RestoreSerialState {
    Idle,
    Preparing { restore_id: RestoreId },
    Committing { restore_id: RestoreId },
    PublishRequired { restore_id: RestoreId },
}
```

Use the synchronization types already standardized in the runtime crate. `PublishedRuntimeTaskRoot` may be an existing `RwLock<Arc<_>>`, epoch snapshot cell, or atomic-arc abstraction. Do not add a dependency merely to use a fashionable primitive; the normative requirement is single atomic root visibility.
