# Final contract

## 1. One live Need handle, two constructible states

The final live carrier is exactly:

```rust
pub struct RuntimeNeedHandle {
    correlation: TaskCorrelation,
    producer: NeedProducerSpec,
    outcome: TaskOutcomeContract,
    state: RuntimeNeedHandleState,
}

pub enum RuntimeNeedHandleState {
    ReusableJoin { spec: Box<TaskSpec> },
    AcceptedLaunch,
}
```

`RuntimeNeedHandle::try_reusable_join` accepts a complete final `TaskSpec`,
requires `JoinSameKey`, validates the exact producer/outcome/policy/execution/
request/catalog relationships, derives the active-generation ordinal-zero
correlation, and mutates neither scheduler nor adapter.

`RuntimeNeedHandle::try_from_accepted_launch` is `pub(crate)` and accepts a
sealed `AcceptedTaskLaunch<'_>` journal proof. It stores no reusable spec and
does not rederive the committed correlation. This prevents public construction
of a forged accepted launch.

Awaiting a reusable handle validates generation, passes the stored spec through
the sole scheduler transaction, requires exact correlation equality, and then
registers an observer. Awaiting an accepted handle validates generation and
the committed journal/Need row and registers directly. It never relaunches.

Semantic equality, hashing and ordering use `NeedId` only. The state, generation,
producer, outcome and complete correlation are structural/use evidence.

## 2. Exact opcode/execution truth table

| Operation | Execution | Policy | Construction | Host side effect at construction |
|---|---|---|---|---|
| `StartTask` | Host | `JoinSameKey` | scheduler ensure now → `AcceptedLaunch` | prepare/atomic apply/commit now if no existing Join row |
| `StartTask` | Runtime | `JoinSameKey` | scheduler ensure now → `AcceptedLaunch` | none |
| `StartTask` | Host/Runtime | `AlwaysStart` | scheduler allocates a positive ordinal and returns `AcceptedLaunch` | Host commits now; Runtime row publishes now |
| `MakeNeedHandle` | Host | `JoinSameKey` | lazy `ReusableJoin { complete spec }` | **none** |
| `MakeNeedHandle` | Runtime | `JoinSameKey` | lazy `ReusableJoin { complete spec }` | none |
| `MakeNeedHandle` | Host/Runtime | `AlwaysStart` | scheduler ensure now → `AcceptedLaunch` | follows execution; AlwaysStart is never reusable |

The existing AWBC numeric allocation is unchanged. `MakeNeedHandle` does not
mean “start a Host task” for the Host+Join row.

## 3. Rederivable AwaitMany request

The request retains the source-order construction evidence:

```rust
pub struct RuntimeAwaitManyAggregateRequest {
    captured: Box<[RuntimeValue]>,
    source_items: Box<[RuntimeValue]>,
    child: Box<NeedProducerTemplate>,
    limit: NonZeroU32,
}
```

For source index `i`, the sole inherent constructor forms:

```text
Tuple([
  Tuple(captured),
  UInt(U32(i)),
  source_items[i],
])
```

and asks the typed template to instantiate the complete child `TaskSpec`,
including its final `TaskExecution`. The caller supplies neither child digest
nor child spec. The aggregate base argument remains `Tuple(source_items)` in
source order. Snapshot restore regenerates each spec/digest and compares every
persisted derived child row before state publication.

## 4. One aggregate child-launch transaction

`RuntimeTaskScheduler::ensure_task_batch` owns the whole transaction. It
derives children in source-index order, observes existing Join rows without
mutation, stages AlwaysStart ordinals and observer IDs, stages journal/runtime/
observer/scope/counter after-images, prepares every Host route group, validates
all cross-references and limits, installs all after-images atomically, commits
all adapter tokens infallibly, and finally replaces the aggregate child status
vector.

Failure before installation rolls prepared tokens back in reverse preparation
order and discards all deltas. No task ordinal, observer ID, worker-visible
command or aggregate child status is consumed. Calling the public per-child
`ensure_task` inside this path is forbidden.

## 5. Persistent observer allocator

Every `RuntimeGenerationJournal` persists
`next_observer_id: NonZeroU64`. The first candidate is `1`; IDs are ordered
numerically within a generation and globally by `(generation, id)`. A candidate
of `u64::MAX` returns `ObserverIdOverflow` before assignment, making
`u64::MAX - 1` the largest issuable value while retaining a representable
strictly-greater next candidate.

Restore requires `next_observer_id` to exceed every observer row and every
observer reference in Need/runtime/scope rows. Removal never rewinds. Single
and batch failures do not consume a candidate.

## 6. Core-owned adapter protocol and complete cancellation

`TaskLaunchAdapter` and all launch/restore/rebind/cancel envelopes live under
`arcweft_core::task`. The deterministic scheduler depends only on core. Adapter
registries and native/Web/headless hosts implement the protocol upward.

Prepare may validate, allocate, check capacity and reserve an unpublished queue
slot. It starts no worker and performs no network/filesystem/audio/other I/O.
Commit only reveals the reserved command and is infallible. Rollback drops an
unpublished reservation and is infallible. Post-commit I/O failure becomes
`TaskEvent::InfrastructureFailure`.

Cancellation has one scheduler transaction spanning adapter reservations,
launch rows, Need cancellation, observers, runtime tasks, scopes and pending
events. `HostCancelCommandId` is derived from the complete canonical
correlation, so one launch has one idempotent command. Duplicate input in one
batch rejects before prepare; a repeated committed request returns
`AlreadyRequested` without an adapter call. Terminal/absent rows return typed
dispositions without adapter work. Cancellation is never a domain payload or
infrastructure failure value.

The current immediate `HostAdapter::submit` and `cancel(&TaskId) -> bool` routes
are deleted, not wrapped.

## 7. One lossless snapshot owner

The existing `AwbcRuntimeValueSnapshot` is evolved in place and its codec is
replaced atomically at Cut 5. It remains the only RuntimeValue session-snapshot
owner. It exactly projects all accepted live variants and nested fields:
opaque owner/payload; iterator cursor/range/witness; every DenseSeq case;
tuple/record columns; reduction owner/state/ordered commands; Agent values and
recursive predicates; AWBC callable authority/remaining parameters/captures;
Variant owner/ordinal/name/payload; and the final NeedHandle state.

`RuntimeFunctionBody::Structured` is matched explicitly and rejected as
`UnrebindableStructuredFunction`, because the current production restore path
cannot rebind its owning `Arc<RuntimePlan>`. No bytes are emitted on rejection.
`RuntimeFunctionBody::Awbc` requires the exact pinned executable authority.

Unknown tags, unknown fields, duplicate fields, noncanonical lengths, trailing
bytes, invalid ordinals, mismatched catalog joins and invalid recursive
cross-references reject. There is no compatibility reader.

## 8. Match role/tag/callable authority

The current source contains 38, not 39, `HirExprKind` variants. Every one has a
stable `u16` semantic tag and an exact ordered direct-child role grammar
matching `HirExprKind::direct_expression_children`. Thread owns no direct
expression-arena child; its flow items remain roots in their typed inventory.
Choice and dialogue/line-plan walks reproduce their current helper order,
including the existing LIFO pending-body/group traversal.

The package also assigns exact tags/payload order to all current checked
expression, checked value, select, HIR pattern, checked pattern, literal, guard
and coverage constructors. Construction is bounded and first-error: preorder
owner order, then exact role order; failure emits no digest.

`CheckedCallableId` plus `CheckedCallableDigest` joined through
`CheckedCallableCatalogV1` is the callable authority. A unit `Call` first reads
the separate call-target fact and then validates that join. A selected Method
resolves its `HirName` through receiver type plus the checked receiver/callable
catalog before writing bytes. `HirName`, source spelling and arena IDs are
never semantic identity.

## 9. Exact ownership carriers

The 85-row matrix is exhaustive. Signed integer types use
`RuntimeValue::Int`/snapshot `Int`; unsigned types use `UInt`/`UInt`.
`IntOrUInt` does not exist.

Result and Option use the exact existing `Variant` owner/ordinal carrier;
Tuple uses `Tuple`; sequence families use `Seq`; exact accepted project
nominals use `NominalRecord`. Choice rejects because current direct-alternative
and tagged-variant representations do not provide one accepted carrier.
Agent/dialogue/character rows without a complete current accepted nominal/
case/field map reject `MissingRuntimeSnapshotOwner` instead of inventing a
source-name identity.

The Need ownership certificate is private at Cut 2 and becomes public only in
the atomic Cut 5 that publishes its snapshot owner.

## 10. Event ordering and restartable snapshots

The event key is `(logical_epoch, task_id, sequence)`. A shared retained-
generation collection prefixes only `generation`. Sequence never precedes
TaskId.

Any prepared adapter transaction blocks a snapshot. Active
`MustBeQuiescent` Host rows block. Active `Restartable` Host rows persist the
complete original spec/correlation/capability/catalog join and restore through
adapter prepare, atomic scheduler state installation and infallible commit.
The contract does not reject all active or nonterminal Host tasks.

## 11. Publication

Cuts 1–4 publish only owners that compile against same/earlier cuts. Cut 5
atomically publishes the final task/handle/batch/observer/adapter/snapshot/
ownership/event owners and deletes every superseded route. No public row cites
a later owner.
