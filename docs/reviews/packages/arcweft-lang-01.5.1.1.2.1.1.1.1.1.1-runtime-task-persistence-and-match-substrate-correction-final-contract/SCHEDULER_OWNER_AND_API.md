# RuntimeTaskScheduler owner and exact API/borrow flow

## 1. Sole owner

`arcweft-runtime-scheduler` replaces its current lightweight scheduler with:

```rust
pub struct RuntimeTaskScheduler<A: TaskLaunchAdapter> {
    config: RuntimeSchedulerConfig,
    journal: RuntimeTaskJournal,
    runtime_tasks: RuntimeTaskState,
    adapter: A,
    pending_events: BTreeMap<TaskEventOrderKey, TaskEvent>,
    ready_runtime_tasks: BTreeSet<TaskId>,
    metrics: RuntimeSchedulerMetrics,
    replacement: ReplacementState,
}
```

No field is mirrored by `arcweft-runtime-driver`. The driver retains only
engine/frame state and a `RuntimeTaskScheduler<A>` value.

`RuntimeTaskJournal` owns:

```rust
struct RuntimeTaskJournal {
    active_generation: GenerationId,
    generations: BTreeMap<GenerationId, RuntimeTaskGeneration>,
}

struct RuntimeTaskGeneration {
    ordinal_counters: BTreeMap<NeedProducerInstanceKey, NextAlwaysStartOrdinal>,
    groups: BTreeMap<TaskKey, TaskGroup>,
    launches: BTreeMap<TaskId, TaskLaunch>,
    needs: BTreeMap<NeedId, RuntimeNeedCell>,
    observers: BTreeMap<TaskObserverId, TaskObserver>,
    replay: TaskReplayState,
}
```

`RuntimeTaskState` owns a `BTreeMap<TaskId, RuntimeTask>`. The launch row is the
lifecycle/correlation/spec authority; the runtime state row is the executable
state for a launch whose `TaskExecution` is Runtime.

## 2. `ensure_task` transaction

### 2.1 Public signature

```rust
pub fn ensure_task(
    &mut self,
    spec: TaskSpec,
) -> Result<RuntimeNeedHandle, TaskEnsureError>;
```

The caller provides no NeedId, TaskKey, TaskId, generation or ordinal.

### 2.2 Borrow flow

1. Borrow `&self.config` and validate all value/work bounds.
2. Validate `spec` and compute `producer_instance`.
3. Read `active_generation`.
4. Borrow the generation immutably and call
   `RuntimeTaskJournal::inspect_ensure`. The result is an owned enum:

   ```rust
   enum EnsureInspection {
       ExistingJoin { handle: RuntimeNeedHandle },
       New {
           ordinal: TaskLaunchOrdinal,
           correlation: TaskCorrelation,
           delta: RuntimeJournalDelta,
           runtime_delta: Option<RuntimeTaskDelta>,
           host_launch: Option<HostTaskLaunchRequest>,
       },
   }
   ```

   The immutable borrow ends here.
5. Existing Join returns only after `structurally_eq_for_join` and complete
   handle validation. No adapter call occurs.
6. For a new Host row, borrow `&mut self.adapter` and call `prepare_launch`.
   Runtime rows skip this step.
7. Validate the owned deltas without borrowing published maps. Any failure
   rolls back the prepared token.
8. Borrow `&mut self.journal` and `&mut self.runtime_tasks` as disjoint fields
   and apply the already validated deltas with infallible `BTreeMap::insert`
   operations. The AlwaysStart counter and launch become visible together.
9. Borrow `&mut self.adapter` and call infallible `commit_launch`.
10. Build the returned handle from the committed correlation/spec. This is an
    infallible projection of validated owned values.

There is no fallible operation after step 7. Allocation needed by maps and
boxed rows occurs while constructing/staging the deltas. If implementation
uses standard `BTreeMap::insert`, OOM remains process-level rather than a typed
rollback branch; no `AdapterCommit` error is modeled.

## 3. Host event ingestion

```rust
pub fn ingest_host_events<I>(
    &mut self,
    events: I,
) -> Result<RuntimeEventApplyReport, RuntimeEventApplyError>
where
    I: IntoIterator<Item = TaskEvent>;
```

Flow:

1. collect at most the configured batch limit into an owned `Vec`;
2. validate each complete correlation and normalize its ordering key;
3. reject duplicate ordering keys with different event digest;
4. sort by `(generation, logical_epoch, sequence, task_id)`;
5. for each event, borrow the target generation immutably to build an owned
   `EventApplyDelta`;
6. validate observer fanout and terminal transition;
7. release immutable borrows;
8. apply launch/Need/observer changes together under disjoint mutable field
   borrows;
9. enqueue any dependent runtime tasks by TaskId.

Adapter code cannot mutate the journal directly and cannot call back into this
method during commit. Host events cross the boundary as ordinary owned values.

## 4. Runtime task stepping

```rust
pub fn step_runtime_tasks(
    &mut self,
    input: RuntimeStepInput,
) -> Result<RuntimeTaskStepReport, RuntimeTaskStepError>;
```

`input.dt` is the sole timeout clock.

Borrow flow:

1. select up to `max_runtime_tasks_per_step` TaskIds from
   `ready_runtime_tasks`, producing an owned ordered list;
2. for each TaskId, temporarily remove its runtime task state from
   `runtime_tasks.tasks`. The launch/spec remains in the journal;
3. run the pure state decision:
   - Timeout reads source Need snapshot and `input.dt`;
   - AwaitMany returns a bounded list of child specs/source indices to launch
     plus pending publications;
4. for every selected child spec, call the scheduler's private
   `ensure_task_internal` while no runtime-task map entry is borrowed;
5. register child observers through the same owner;
6. apply the resulting handles/statuses back to the owned runtime task;
7. build an owned task/event delta, validate, reinsert nonterminal state or
   remove terminal state, and atomically apply publications.

This remove/decide/reinsert pattern uses ordinary ownership. If a child launch
fails, the aggregate state and selected child batch are restored from the owned
pre-step row; no child row or ordinal from a partially accepted batch is
published. The implementation may process the batch one child at a time only
if it records and rolls back every prepared launch in a single scheduler
transaction; the preferred implementation stages the whole bounded batch.

## 5. Observer registration

```rust
pub fn register_observer(
    &mut self,
    handle: &RuntimeNeedHandle,
    kind: TaskObserverKind,
) -> Result<TaskObserverId, TaskObserverError>;
```

Order:

1. `handle.validate_structure()`;
2. `handle.validate_use(active_generation)`;
3. Need lookup and exact producer/outcome match;
4. observer work/capacity limit;
5. allocate the next observer ID in the journal-owned observer allocator;
6. stage both forward observer row and reverse Need membership;
7. publish both together.

A stale handle fails at step 2, before observer ID allocation or mutation.

## 6. Snapshot

```rust
pub fn snapshot(
    &self,
) -> Result<RuntimeTaskSchedulerSnapshotV1, RuntimeSnapshotError>;
```

The immutable borrow guarantees no concurrent prepare/commit. Snapshot first
checks `replacement` and the adapter transaction state; any prepared token or
nonquiescent host task is an error. It then walks canonical BTree order and
constructs an owned projection. Metrics/debug-only counters are excluded unless
a named snapshot row explicitly owns them.

## 7. Restore

```rust
pub fn restore(
    &mut self,
    snapshot: RuntimeTaskSchedulerSnapshotV1,
) -> Result<(), RuntimeRestoreError>;
```

Flow:

1. decode/validate into an owned `ValidatedSchedulerRestore` before borrowing
   the live scheduler mutably beyond the method receiver;
2. build complete temporary journal/runtime/event/replacement maps;
3. construct `HostTaskRestoreBatch` only from Host rows whose restore policy
   permits it;
4. prepare the adapter restore token;
5. validate the final joined temporary state;
6. on failure, rollback token and leave `self` unchanged;
7. `mem::replace` journal/runtime/queues/replacement with temporary values;
8. infallibly commit adapter restore.

No decoded row is inserted incrementally into the live scheduler.

## 8. Replay

Replay uses the same event validation and delta application function as live
events. The only prelude is strict envelope/version/generation and stored event
digest verification. There is no relaxed replay path.

## 9. Replacement

`prepare_replacement` validates the compiler/View/bundle mapping and produces a
complete owned `ValidatedReplacementMapping`. It does not mutate runtime state.

`commit_replacement`:

1. checks a quiescent replacement barrier;
2. stages the new generation, rederived correlations, observer/Need mappings
   and runtime state;
3. creates a Host-only rebind batch;
4. prepares adapter rebind;
5. validates the complete staged join;
6. rolls back on any precommit failure;
7. publishes the new generation/mapping in one in-memory swap;
8. infallibly commits adapter rebind.

NeedId and ordinal are preserved. TaskKey and TaskId are rederived from the new
GenerationId. A prepared rebind token cannot be snapshotted.

## 10. Driver API

The driver's final surface is consumer-only:

```rust
pub struct RuntimeDriver<A: TaskLaunchAdapter> {
    // engine, frame and View state
    tasks: RuntimeTaskScheduler<A>,
}

impl<A: TaskLaunchAdapter> RuntimeDriver<A> {
    pub fn step(&mut self, input: RuntimeStepInput) -> Result<RuntimeStepOutput, RuntimeDriverError> {
        self.tasks.ingest_host_events(input.host_events)?;
        self.tasks.step_runtime_tasks(input)?;
        // run engine/View consumers against TaskHost
        ...
    }
}
```

There is no `RuntimeTaskRegistry`, no driver-owned GenerationId definition, no
counter and no cross-object rollback protocol.
