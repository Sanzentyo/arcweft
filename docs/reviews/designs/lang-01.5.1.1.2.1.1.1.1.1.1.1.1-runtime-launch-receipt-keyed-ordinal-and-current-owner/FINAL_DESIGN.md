# Final accepted design

## 1. Precedence and retained substrate

This design applies to production at
`61779d1432b902efc2d19041a7326f3c1319828a`. Current production and maintained
runtime documentation outrank the frozen returned package. The correction
request outranks conflicting rows in that package.

The following decisions are retained without alteration:

- one `TaskExecution::Host | Runtime` field in `TaskSpec`;
- `RuntimeNeedHandleState::{ReusableJoin { spec }, AcceptedLaunch}` and
  NeedId-only semantic equality, hashing, and ordering;
- AwaitMany's captured values, source-order items, typed child template,
  `Tuple([Tuple(captured), UInt::U32(index), item])` child argument, and
  source-order aggregate base tuple;
- one whole AwaitMany launch transaction with reverse rollback;
- generation-owned persistent observer allocation;
- fallible adapter prepare, infallible commit/rollback, and post-commit
  infrastructure failures as `TaskEvent::InfrastructureFailure`;
- runtime-owned AwaitMany aggregate and timeout requests;
- current 38-family generic Match inventory, compiler-local
  `HirSnapshotId + ExprId` lookup, and HIR-free persistent View products;
- event order `(logical_epoch, task_id, sequence)`, with `generation` prefixed
  only for retained-generation collections;
- prepared transactions blocking snapshot, active `MustBeQuiescent` Host rows
  blocking snapshot, and complete active `Restartable` Host rows restoring
  through prepare/apply/commit; and
- version markers fixed at `1`, no compatibility reader, and no numeric AWBC
  allocation changes.

## 2. Final execution truth table

`NeedProducerFamily::validate_execution` owns one exhaustive match. Debug
labels, Host operation spelling, and request variants do not select a family.

| Family | Execution | Policy |
|---|---|---|
| `StructuredTaskPlan` | Host | JoinSameKey or AlwaysStart |
| `AwbcTaskPlan` | Host | JoinSameKey or AlwaysStart |
| `ViewMatchSubscription` | Host | JoinSameKey |
| `AwaitManyBase` | Runtime AwaitManyAggregate | JoinSameKey |
| `AwaitManyChild` | Host, Runtime AwaitManyAggregate, or Runtime Timeout | JoinSameKey or AlwaysStart |
| `Timeout` | Runtime Timeout | JoinSameKey |
| `LineTask` | Host | AlwaysStart |
| `HostAdapterTask` | Host | JoinSameKey or AlwaysStart |
| `MakeNeedHandle` | **Host only** | **JoinSameKey only** |

`MakeNeedHandle` validates the complete Host `TaskSpec`, constructs
`ReusableJoin { spec }`, and mutates neither journal nor adapter. Runtime or
AlwaysStart forms reject before identity allocation. AlwaysStart produces a
handle only after another authorized launch has committed and the scheduler
obtains an accepted-launch receipt.

## 3. Accepted launch and handle construction

The core-owned `RuntimeGenerationJournal` is the only committed-row authority.
After atomic state application, the scheduler calls
`accepted_launch_receipt`. That method looks up both rows, validates exact
correlation, spec, producer, outcome, generation, lifecycle and policy/ordinal,
then returns a public proof with private fields. An uncommitted delta cannot
borrow a receipt and no raw-field constructor exists.

`RuntimeNeedHandle::try_from_accepted_launch` is public because its caller is a
different crate. Safety comes from the unforgeable private-field receipt, not
from crate-private visibility. Restore first builds and validates a private
journal after-image. It cannot obtain an accepted receipt from that
uncommitted image. Core constructs live handles/proofs only while applying the
sealed image; the scheduler returns them only after its own infallible swap and
adapter commit complete.

Reusable Await validates the active generation, ensures the stored complete
spec through the sole scheduler transaction, requires exact correlation
equality, and registers an observer. Accepted Await resolves the committed row
and registers directly; it never rederives or launches.

## 4. Keyed AlwaysStart ordinals and observer IDs

Each generation journal stores a `BTreeMap<NeedProducerInstanceKey,
NonZeroU64>` of next candidates. Absence means `1`. Join never reads or writes
this map. AlwaysStart allocates the current value, stages its checked successor,
and publishes the launch plus counter after-image atomically. `u64::MAX` is not
issuable because no representable next candidate would remain.

The snapshot stores unique ascending `{ producer, next }` rows. For each
producer, restore requires `next` to be greater than every persisted launch
ordinal. A missing counter is valid only if that producer has no persisted
AlwaysStart launch. Batch deltas contain after-images only for touched keys.
Unrelated producer interleaving therefore cannot change another producer's
ordinal, NeedId, or TaskId.

`next_observer_id` remains one per-generation `NonZeroU64`, begins at `1`, and
is persisted. Candidate `u64::MAX` is unallocatable. Removal never rewinds;
failed single or batch work consumes no observer ID.

## 5. Adapter receipt and transaction protocol

`arcweft_core::task::TaskLaunchAdapter` owns the Sans-I/O protocol. The
scheduler continues to depend only on core. Host implementations depend upward
and own unpublished queue reservations and their private token types.

Prepare returns `PreparedLaunchBatch<Token>` containing an inspectable
core-owned receipt and the adapter-private token. Its constructor validates
generation, canonical source-index order, cardinality, unique indices,
correlations, and operations against the exact input batch. The scheduler then
validates each capability's route and active uniqueness against
`HostOperationCatalog` and current journal rows before applying state.

`HostLaunchCapability` and `HostCancellationCapability` are `(HostRouteId,
NonZeroU64)` newtypes. A full capability is unique among active launches for
that route. Each route owns monotonic next-candidate allocators inside its
unpublished reservation state. Rollback restores both candidates, so repeating
the same prepare with no intervening committed route work returns the same
pair; prepare failure consumes neither ID. After apply, the capabilities are
immutable journal evidence. Restore must return the exact persisted pair.
Rebind retains the pair while changing correlation. Cancel consumes the pair
as authorization but does not delete it until terminal acknowledgement makes
the launch row terminal.

`prepare_restore` reserves each persisted pair exactly and stages each route's
next candidate above every restored active capability. A duplicate persisted
pair or collision with an already active route reservation rejects and rolls
back. Thus process restart cannot reset an allocator into an active ID, and
future launch preparation remains monotonic after restore.

### Cross-crate create/read/mutate authority

Every protocol struct crossing core, runtime-scheduler, or host-adapter keeps
private fields. `TaskSpec::try_new` performs the sole full admission before
returning a value; TaskSpec, producer, and correlation expose read-only typed
getters. Host launch/restore/rebind/cancel batch and row constructors accept
only complete typed inputs and every field required by an adapter has a
getter. Catalog rows likewise expose identity, capability, request, route,
restart, and cancellation getters. No `pub(crate)` seam or public-field escape
exists. HIR child edges separately expose public `child()` and `role()` so sema
can enrich them without seeing HIR-private fields.

Committed journal rows are different from transport rows: scheduler code may
read `TaskJournalRow`, `RuntimeNeedCell`, observer, scope, and accepted Host
evidence through getters, but cannot construct or mutate them. Core owns
`JournalTransaction`. It clones the current generation/revision into a private
complete after-image, validates each requested TaskSpec through the borrowed
authority, derives correlation/ordinals, allocates observers, and builds typed
adapter batches. `RuntimeJournalBatchDelta` and `RuntimeObserverBatchDelta` do
not exist in the scheduler.

The revision is an in-process optimistic apply guard, not semantic runtime
state: snapshot requires no open transaction and restore initializes revision
zero. It never enters Need/Task identity, event ordering, or persisted bytes.
Its checked successor is staged before adapter prepare, so revision overflow is
a pre-mutation transaction error.

The exact launch/restore/rebind/cancel sequence is:

1. create a core `JournalTransaction` from the committed journal and matching
   `TaskValidationAuthority`;
2. request ensure/restore/rebind/cancel changes and stage the scheduler-private
   `SchedulerRuntimeAfterImage` from typed transaction results;
3. prepare adapter route groups in canonical order and retain every opaque
   token; no committed state has changed;
4. feed inspectable launch/restore/rebind receipts back into the transaction,
   validate the scheduler image, then call `seal`; any error rolls prepared
   tokens back in reverse order;
5. call `RuntimeGenerationJournal::apply_after_image`. It first checks the base
   generation and revision without mutation. Failure rolls every prepared
   token back in reverse order;
6. after its successful complete journal/observer/scope/event swap, invoke the
   scheduler-private infallible runtime-image swap;
7. commit prepared tokens in canonical order through infallible adapter
   methods; and
8. only now return core-built `AppliedEnsureResult` handles or expose an
   `AcceptedTaskLaunchReceipt` borrowed from committed rows.

There is no fallible operation after step 5 succeeds. The four private
`RuntimeScheduler::apply_*_plan` coordinators encode this ordering directly.
No callback or returned handle exposes the intermediate point between the two
infallible swaps and adapter commit. Restore, replacement rebind, cancel, one
launch, and AwaitMany batches all use the same owner and sequence.
[CROSS_CRATE_REACHABILITY.md](CROSS_CRATE_REACHABILITY.md) is the normative
type-by-type create/read/mutate inventory.

The same plan/apply engine handles one launch and AwaitMany batches. It never
calls public per-child `ensure_task` from a batch. Cancellation deduplicates
complete correlations before prepare, uses one deterministic
`HostCancelCommandId` per correlation, returns typed absent/terminal/already-
requested dispositions without adapter work, and never produces a domain
payload.

### Zero-domain policy

`GenerationId` is exactly a `u64`; generation zero is valid. It denotes the
first generation and is never an absence sentinel. `TaskLaunchOrdinal(0)` is
likewise valid for `JoinSameKey`; `AlwaysStart` ordinals and their persisted
next candidates are nonzero.

The all-zero byte string is rejected by the fixed identity owners
`NeedProducerInstanceKey`, `NeedId`, `TaskKey`, `TaskId`, and
`HostCancelCommandId`. Derivation returns a typed identity error if the sole
canonical transcript hash is all zero; it does not salt, retry, or silently
change the transcript. Route, operation, observer, launch-capability,
cancellation-capability, and next-counter scalar IDs use `NonZeroU32` or
`NonZeroU64` as shown in the Rust schema.

By contrast, semantic digests accept every 32-byte result, including all zero:
`NeedProducerContractDigest`, `TaskPlanSemanticDigest`,
`RuntimeTypeSemanticDigest`, `NeedTimeoutContractDigest`,
`HostOperationCatalogDigest`, current `AwbcDigest`, and current
`RuntimeValueDigest`. Absence is represented only by `Option`, never by a zero
generation, ordinal, ID, capability, or digest. This split is one owner-level
policy; consumers do not add their own zero checks.

`NeedProducerSpec::new` is therefore infallible: all inputs are already typed,
`producer_site` spans all `u32` values, and `RuntimeValueDigest` also accepts an
all-zero digest. There is no `NeedProducerSpecError` and no zero-digest success
branch to reject. Only `instance_key()` can fail, when its derived fixed
identity is all zero. `TaskValidationAuthorityError::GenerationMismatch`
denotes disagreement between joined owners; it never means generation zero is
invalid.

All new private-field scalar/digest newtypes have one reachable owner API.
`TaskLaunchOrdinal::JOIN` publicly supplies the Join zero ordinal; positive
AlwaysStart ordinals remain journal-only. `TaskPriority`, `HostRouteId`, and
`HostOperationId` expose typed `new`/`get` APIs, with Host IDs accepting only
`NonZeroU32`. Semantic digest owners expose `from_bytes`/`as_bytes` and accept
every byte array. Fixed producer/Need/task/cancel-command identities expose no
raw constructor and remain derivation-only; observer IDs remain journal-only.

The remaining newtype domains were audited separately: `TaskPriority(i32)`
accepts zero as an ordinary priority, while both HIR and checked nested paths
require at least one typed segment and reject an empty boxed slice in their
public constructors. They are structural paths, not numeric absence sentinels.

## 6. One task-validation authority

`TaskValidationAuthority<'a>` is core-owned, borrowed, nonserialized, and not
cloneable. It contains:

- the active `GenerationId`;
- `&RuntimePlan`, the current structured plan owner;
- `&AwbcProgram`, the current AWBC plan owner, together with the executable
  identity verified by the constructor;
- `&HostOperationCatalog`, the sole typed Host operation/route/capability
  authority; and
- `&dyn ViewTaskPlanAuthority`, a core-owned protocol implemented by the
  accepted upper View product owner.

Core does not import `arcweft-view` or `arcweft-bundle`. The View protocol
accepts only core-owned identity/digest projections and validates against the
actual retained product; it exposes no copied row collection. `RuntimePlan`
gains a task-plan table inside the existing owner, and `AwbcProgram` continues
to use its existing `task_plans` table. There is no `TaskContractCatalogV1`.

`TaskSpec::validate(authority)` performs, in order: scalar/limit validation;
producer identity recomputation; family/execution/policy match; payload type
and outcome equality; family-specific plan/site lookup; Host operation/request/
route/capability contract lookup or closed Runtime request validation; and
complete argument-digest validation. Reusable handles, template
instantiation, ensure, snapshot, and restore call this one method.

## 7. One outer snapshot authority

`RuntimeSnapshotAuthority<'a>` is core-owned, borrowed, nonserialized, and the
only restore/admission context. It contains the active generation, exact
`&AwbcProgram`, `&RuntimePlan`, committed `&RuntimeGenerationJournal`,
`&HostOperationCatalog`, and `&dyn ViewTaskPlanAuthority`. Its constructor
validates the same generation and executable/task-plan joins as
`TaskValidationAuthority` and returns that authority by reference from
`task_validation()`; it does not copy catalog rows.

`AwbcRuntimeValueSnapshot` remains the single RuntimeValue snapshot enum and is
evolved in place with `NeedHandle`. It does not contain an authority reference.
The function row remains the current dormant `{ function, remaining_params,
captures }` shape. Projection and restore require the function ID to exist in
the outer AWBC program. Structured functions reject before output is exposed.
The enclosing fiber still calls `FiberState::validate_for_program`; no
per-function program/generation authority is serialized.

The existing `DenseSeq` is reused directly by
`AwbcRuntimeSeqSnapshot::Dense(DenseSeq)`. `Units(usize)` and
`Bool(DenseSeqStorage<bool>)` remain exact. The purpose-built codec encodes a
Units length as bounded canonical `u64` and performs checked `u64`/`usize`
conversion. There is no dense projection enum.

Accepted-launch handle restore resolves its committed journal row through the
outer authority. Reusable handles store a complete `TaskSpecSnapshotV1` and
revalidate it through the same task authority. Unknown/duplicate fields,
noncanonical lengths, trailing bytes, invalid cross-references, and work-limit
exhaustion reject before any live graph is published. Version remains `1` and
there is no old reader.

## 8. Match callable authority and child-role layering

Match uses current `FinalSemanticAnalysis` APIs:

1. `call(expr)` yields current call facts;
2. a selected nonintrinsic `ResolvedCallable::checked()` yields the
   `CheckedCallableId`;
3. `checked_callables().callable(id)` validates the current generation/catalog
   row; and
4. `id.semantic_digest()` supplies the digest after signature, receiver mode,
   effects, and instantiation have matched the call facts.

Intrinsic callables use the existing typed intrinsic ID/family and a closed
semantic tag. They are never fabricated into `CheckedCallableCatalog`. There
is no `CheckedCallableCatalogV1`.

HIR owns `HirExpressionChildEdge` and the HIR-only role/path vocabulary.
`direct_expression_children()` is exactly `child_edges().map(|edge|
edge.child())`, so ordering has one owner. Sema enriches each edge through the
current checked expression/call/record/choice/dialogue facts into
`CheckedExpressionChildRole`. Missing or ambiguous evidence rejects before a
transcript sink is exposed. HIR imports no core or sema type.

The 38 family tags and constructor/role tags retained by the parent package
remain unchanged. [MATCH_CHILD_EDGES.md](MATCH_CHILD_EDGES.md) defines the
complete two-stage inventory.

## 9. Ownership and variant conventions

Option is exactly `Some = 0` with one payload and `None = 1` without a payload.
Result remains `Ok = 0`, `Err = 1`. The ownership classifier and snapshot
restore call `RuntimeCheckedType::variant_case`; they do not repeat ordinals in
a second table.

`TypeKind::AgentBuiltin` is destructured exhaustively. Diagnostics and
ViewportPoint use their existing `RuntimeAgentValue` and
`AwbcRuntimeAgentSnapshot` carriers. Each unsupported sibling returns its own
`MissingRuntimeSnapshotOwner`. Other payload-bearing TypeKind families are
also destructured explicitly, even when every child has the same rejection.
[OWNERSHIP_MATRIX.md](OWNERSHIP_MATRIX.md) is normative.

## 10. Failure precedence

Construction is first-error and produces no partial digest, handle, receipt,
snapshot, or state publication:

```text
limits/scalars
< producer/correlation identity
< family/execution/policy
< plan/site/catalog generation
< payload/outcome/request contract
< adapter receipt shape
< capability route/uniqueness
< scheduler after-image cross-references
< atomic apply
< infallible adapter commit
```

Snapshot decode performs canonical wire/limit checks before semantic joins.
Batch derivation is source-index ordered. Receipt mismatch rolls back the token
that carried it and every previously prepared token in reverse order.

## 11. Readiness

All result-changing choices are closed. Implementation proceeds only in the
compile-clean order in `CUTS_TESTS_AND_DELETION.md`; no production patch is
part of this design. `OPEN_QUESTIONS.md` is exactly `none`.
