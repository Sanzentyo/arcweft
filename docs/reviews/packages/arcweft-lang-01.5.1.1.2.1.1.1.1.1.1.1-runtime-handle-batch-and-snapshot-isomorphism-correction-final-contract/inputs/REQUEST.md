# Lang-01.5.1.1.2.1.1.1.1.1.1.1 — runtime handle, batch, and snapshot isomorphism correction

## Sequence, inputs, and precedence

This is a mandatory nonnumeric correction to the returned runtime-task,
persistence, and Match-substrate package.

Required retained inputs are:

- the parent
  [runtime task persistence and Match substrate correction](2026-08-22-lang-01.5.1.1.2.1.1.1.1.1.1-runtime-task-persistence-and-match-substrate-correction.md);
- the retained returned archive
  [`arcweft-lang-01.5.1.1.2.1.1.1.1.1.1-runtime-task-persistence-and-match-substrate-correction-final-contract.zip`](../packages/zips/arcweft-lang-01.5.1.1.2.1.1.1.1.1.1-runtime-task-persistence-and-match-substrate-correction-final-contract.zip),
  SHA-256
  `034A2EEAB2D083B5BB4496F4EE63040B2F93B30ABDDA1B18E93138E28B65391B`;
- its searchable
  [frozen mirror](../packages/arcweft-lang-01.5.1.1.2.1.1.1.1.1.1-runtime-task-persistence-and-match-substrate-correction-final-contract/README.md);
- the
  [repository intake and reconciliation](../../implementation/2026-08-22-lang-01-5-1-1-2-1-1-1-1-1-1-task-substrate-return-intake.md);
- maintained
  [scheduler](../../02-runtime/async-scheduler.md),
  [Need timeout](../../02-runtime/need-timeout.md), and
  [AWBC runtime](../../02-runtime/executable-runtime-core.md) contracts; and
- current production at
  `3670625a02b9e7e8578b57fc7b148a1758a17dba`.

Current production, maintained stable documentation, this correction, and
later accepted contracts take precedence. Every Arcweft-owned version marker
remains exactly `1`. No compatibility reader, String fallback, identity alias,
dual carrier, second RuntimeValue digest grammar, or source-string
reconstruction is authorized.

## Accepted substrate that must not be redesigned

Retain the previous return's validated corrections:

1. Plain+SnapshotOnly has canonical value identity; constant publication is a
   separate explicit fence.
2. `TaskSpec` has one `TaskExecution::Host | Runtime` field.
3. Timeout and AwaitMany aggregate are runtime-owned execution variants.
4. commit is infallible and `TaskEnsureError::AdapterCommit` is absent.
5. NeedHandle semantic Eq/Hash/Ord is NeedId-only; ordinary use checks active
   generation.
6. Match lookup is compiler-local `HirSnapshotId + ExprId`.
7. persistent View rows contain no compiler-local IDs.
8. Predicate is a leaf; Shared rejects MissingRuntimeSnapshotOwner.
9. public RuntimeValue/Task/persistence publication remains atomic Cut 5.
10. all previously frozen Need/task identity roles and numeric AWBC allocation
    remain unchanged.

Do not reopen these choices without a new concrete source-evidenced flaw.

## Mandatory correction 1 — constructible reusable Join handle

Define the complete live carrier and both construction paths:

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

`try_reusable_join` accepts only a complete JoinSameKey TaskSpec, derives the
ordinal-zero correlation for the active generation, validates exact producer,
outcome, policy, execution, and request relationships, and does not mutate the
scheduler or adapter. `try_from_accepted_launch` accepts the complete committed
correlation and does not retain a reusable spec.

Await behavior is exact:

- ReusableJoin validates generation, submits the stored final TaskSpec through
  the sole scheduler, requires the returned correlation to equal the handle,
  then registers the observer;
- AcceptedLaunch validates generation and registers directly without
  rederivation or relaunch; and
- AlwaysStart can only produce AcceptedLaunch.

`MakeNeedHandle + Host + JoinSameKey` means lazy construction of the first row,
not host launch. The execution truth table and opcode/VM behavior must state
this explicitly. NeedId-only canonical identity and semantic equality remain
unchanged.

The snapshot row uses the same closed state enum. A reusable row contains the
complete TaskSpec snapshot; an accepted-launch row does not. Restore validates
the same construction invariants.

## Mandatory correction 2 — rederivable AwaitMany request

Replace the nonverifiable source-items/finished-child-spec-only request with a
single owner that retains the source-order construction evidence:

```rust
pub struct RuntimeAwaitManyAggregateRequest {
    captured: Box<[RuntimeValue]>,
    source_items: Box<[RuntimeValue]>,
    child: Box<NeedProducerTemplate>,
    limit: NonZeroU32,
}
```

An equivalent final type name is allowed, but it must retain the exact captured
tuple and one typed child template capable of producing the final
`TaskExecution`. For source index `i`, the sole inherent constructor creates:

```text
RuntimeValue::Tuple([
  RuntimeValue::Tuple(captured),
  RuntimeValue::u32(i),
  source_items[i],
])
```

and derives the complete child TaskSpec. No caller supplies a child argument
digest. Construction and restore regenerate every child spec/digest from these
values and compare any persisted derived row. The aggregate base argument
transcript remains the previously frozen source-order tuple.

## Mandatory correction 3 — one whole child-launch batch transaction

Define a concrete scheduler-owned batch protocol. At minimum it contains:

```rust
struct EnsureBatchPlan {
    journal: RuntimeJournalBatchDelta,
    runtime: RuntimeTaskBatchDelta,
    observers: RuntimeObserverBatchDelta,
    prepared_host: Vec<PreparedLaunch>,
    results: Vec<(u32, RuntimeNeedHandle, TaskObserverId)>,
}
```

Exact visibility may remain private, but the fields/roles and transaction are
normative:

1. derive and validate every child in source-index order;
2. inspect all existing Join rows without mutation;
3. allocate candidate AlwaysStart ordinals without advancing counters;
4. stage every new journal/runtime/observer row and every counter value;
5. prepare every Host row, collecting owned tokens;
6. on any failure, rollback all prepared tokens in reverse preparation order
   and discard all deltas;
7. validate all cross-references and work limits;
8. atomically apply journal/runtime/observer/counter deltas;
9. infallibly commit all Host tokens; and
10. publish aggregate child handles/statuses together.

Per-child `ensure_task` commits are forbidden inside an aggregate batch. An
existing Join may be included as a nonmutating batch result. Failed batches
consume no task ordinal or observer ID and launch no worker-visible work.

## Mandatory correction 4 — persistent observer allocator

Each generation journal owns a monotonic observer allocator, for example:

```rust
next_observer_id: NonZeroU64
```

The exact typed ID owner, start value, overflow error, ordering, and domain are
required. The version-1 generation snapshot persists it. Restore requires it
to be strictly greater than every persisted observer ID and every observer ID
referenced by Need/runtime rows. Observer removal never rewinds it. Failed
single or batch registration consumes no ID.

## Mandatory correction 5 — complete Host cancellation transaction

The core-owned protocol trait adds:

```rust
type PreparedCancel;
type PrepareCancelError;

fn prepare_cancel(
    &mut self,
    batch: HostTaskCancelBatch,
) -> Result<Self::PreparedCancel, Self::PrepareCancelError>;
fn commit_cancel(&mut self, prepared: Self::PreparedCancel);
fn rollback_cancel(&mut self, prepared: Self::PreparedCancel);
```

Define the exact cancel batch/correlation/capability rows and idempotence rules.
Cancellation stages adapter, launch, Need, observer, runtime-task, and scope
changes in one scheduler transaction. Prepare refusal leaves everything
unchanged. Commit is infallible and only exposes already reserved cancel
commands. Cancellation never becomes a domain error or infrastructure payload.

## Mandatory correction 6 — preserve Sans-I/O dependency direction

`TaskLaunchAdapter` and all Host launch/restore/rebind/cancel envelopes belong
to the lower Sans-I/O protocol owner, preferably `arcweft_core::task`.
`arcweft-runtime-scheduler` continues to depend only on core and implements the
generic deterministic state machine. `arcweft-host-adapter` and native/Web/
headless hosts depend upward on core/scheduler as legitimate trait
implementers; scheduler must not depend on a host-adapter crate.

Adapter implementation protocol is fixed:

- prepare performs route/capability validation, capacity checks, allocation,
  and reservation of an unpublished queue slot only;
- prepare starts no worker, network, filesystem, audio, or other external
  side effect;
- commit only makes the prepared command visible to the worker queue and is
  infallible;
- rollback drops an unpublished reservation and is infallible; and
- actual post-commit I/O failures publish
  `TaskEvent::InfrastructureFailure`.

Inventory and replace current `HostAdapter::submit`/`cancel -> bool` paths;
wrapping them without changing their timing is not accepted evidence.

## Mandatory correction 7 — snapshot schemas isomorphic to live carriers

The final snapshot owner may evolve the existing
`AwbcRuntimeValueSnapshot` in place or replace it atomically, but it must be a
lossless, exhaustive projection of the final live RuntimeValue algebra. No
second reader or compatibility row remains.

At minimum the exact final schemas must preserve:

- opaque producer, semantic identity, class, persistence, and recursive boxed
  RuntimeValue payload;
- iterator variants `Values { items, index }`, `Range(exact range iterator)`,
  and `Witness { state, next: RuntimeTraitMethodId }`;
- sequence variants `Values`, every `DenseSeq` case, `TupleColumns { len,
  columns }`, and `RecordColumns { len, fields with identity/name/values }`;
- reduction owner, recursive state, ordered RuntimeCommand rows, constructors,
  targets, and payloads;
- Agent value and predicate variants with all recursive operands;
- function `Structured` versus `Awbc` bodies, owning executable authority or
  strict rejection where current restore cannot rebind it, function site,
  remaining parameters, captures, and bound arguments; and
- final NeedHandle state from mandatory correction 1.

Do not use a generic `{ kind, items }`, `{ source, cursor }`, opaque bytes, or
callable/captures summary where current variants contain more information.

Define every referenced projection enum, including the complete
`RuntimeCheckedTypeProjectionV1`, `RuntimeAgentValueProjectionV1`, all dense
sequence cases, and all accepted projection newtypes. `RuntimeHostOperationId`
must either receive one exact current/same-cut typed catalog owner and
constructor or be replaced by an existing accepted host-operation identity.

The package validator must compare every live enum/struct inventory and field
shape with its snapshot row, not merely verify that referenced type names
exist.

## Mandatory correction 8 — constructible Match callable and role transcripts

Retain current checked variant inventories, but add the missing exact owners:

- define the complete `CheckedExpressionChildRole` enum and stable numeric
  semantic tags;
- enumerate every current `HirExprKind` Structural family, its semantic tag,
  and ordered child roles;
- assign exact tags and payload order to every expression, value, select,
  pattern, pattern-family, literal, guard, and coverage constructor row; and
- include work limits and first-error behavior for role-path construction.

For checked callable facts, use current authority:

- `CheckedCallableId`/`CheckedCallableDigest` and the accepted checked callable
  catalog are the source join;
- a same-cut projection to RuntimeCallableId/CallableContractHash is allowed
  only when its constructor validates that exact catalog join;
- unit `Call` resolution uses the separate current call-target facts; and
- selected Method evidence must resolve its current HirName through the
  checked callable/receiver catalog before any transcript is emitted.

Missing joins reject. HirName/source spelling is never semantic identity.
Differential tests must vary source spelling and arena IDs while keeping the
accepted callable catalog constant, and must change digest when the accepted
callable digest changes.

## Mandatory correction 9 — exact ownership carrier projection

Repair every successful ownership row:

- numeric signed types map to the exact `RuntimeValue::Int` and snapshot
  `Int`; unsigned types map to `UInt`; delete `IntOrUInt`;
- Result, Option, Tuple, Choice, records, and sequences select one exact carrier
  from their accepted runtime type projection, not “Tuple or Variant” prose;
- TextCluster/DisplayText, Agent protocol records/variants, and dialogue/
  character nominals receive closed projection enums, exact accepted
  nominal/case/field maps, constructors, lowering rules, and restore checks;
  otherwise those rows reject `MissingRuntimeSnapshotOwner`; and
- a Cut-2 success may cite an existing snapshot owner or a complete same-cut
  owner only. A row introduced in Cut 5 cannot retroactively prove a published
  Cut-2 certificate unless the certificate remains private until Cut 5.

Machine matrix, Rust schema, live carrier inventory, snapshot inventory,
lowering constructors, and tests must agree exactly.

## Mandatory correction 10 — preserve event ordering and close snapshot policy

Maintain the accepted normalization tuple:

```text
(logical_epoch, task_id, sequence)
```

If retained generations share one pending-event collection, the only permitted
prefix is:

```text
(generation, logical_epoch, task_id, sequence)
```

Sequence must not precede TaskId. Update live ordering keys, replay,
snapshots, tests, and machine data consistently.

Select the restartable snapshot rule:

- prepared adapter transactions always block snapshot;
- active `MustBeQuiescent` Host rows block snapshot;
- active `Restartable` Host rows are persisted with the complete original
  request/correlation and restored through adapter prepare/commit; and
- the package must contain no statement that rejects every active or
  nonterminal Host task.

## Compile-clean sequence

Retain the five cuts, with these additions:

1. Cut 1 includes exact child-role/HirExprKind tags and callable-catalog joins.
2. Cut 2 publishes only fully defined current/same-cut ownership projections;
   otherwise the affected certificate remains private until Cut 5.
3. Cut 3 remains compiler-local and task-type-free.
4. Cut 4 publishes only standalone identity/digest/sink infrastructure.
5. Cut 5 atomically publishes reusable/accepted handle states, batch ensure,
   observer allocator, cancel protocol, core-owned adapter trait/envelopes,
   isomorphic snapshots, final ownership carriers, maintained event order, and
   deletes every superseded task/snapshot/adapter route.

No public cut may cite a type or proof introduced only by a later cut.

## Required returned package

Return one independently throwable ZIP containing at least:

1. final decision register and Rust-shaped live schemas;
2. reusable/accepted NeedHandle constructors and state machine;
3. AwaitMany captured/template transcript and exact batch transaction;
4. observer allocator and persistence rules;
5. launch/restore/rebind/cancel adapter protocol and current-adapter migration;
6. layer/dependency proof preserving scheduler Sans-I/O;
7. live-value-to-snapshot isomorphism table and complete schemas;
8. exact Match role/tag/callable-join tables;
9. corrected ownership carrier/projection matrix;
10. maintained event ordering and one restartable snapshot policy;
11. real source/deletion inventory and corrected compile cuts;
12. focused/property/differential/tamper/rollback/restore tests; and
13. a read-only validator with negative self-tests for every blocker in this
    request.

The validator must fail for a reusable handle without TaskSpec, AwaitMany
without captured/template evidence, per-child committed batch, missing observer
allocator, missing cancel adapter methods, scheduler-to-host-adapter dependency,
lossy snapshot carrier, undefined projection, missing Match role/tag/callable
join, ambiguous ownership carrier, sequence-before-TaskId ordering, or blanket
active-Host snapshot rejection.

`READY_FOR_IMPLEMENTATION` is valid only when every live/snapshot projection is
constructible and lossless, every transaction has one executable owner and
rollback boundary, and every public cut compiles against already published
final owners.
