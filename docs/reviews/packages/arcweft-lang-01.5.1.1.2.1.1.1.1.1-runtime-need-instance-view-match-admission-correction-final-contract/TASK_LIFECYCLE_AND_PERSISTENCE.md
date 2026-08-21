# Task lifecycle, event correlation, journal, save/replay, and replacement

## 1. `ensure_task` transaction

`TaskHost::ensure_task` is the only public launch boundary.

### 1.1 Validation before mutation

The implementation performs these checks in order:

1. validate `TaskSpec` structure, request limits, outcome contract, and debug
   label limits;
2. validate the producer instance fields by recomputing
   `NeedProducerInstanceKey`;
3. validate producer-policy restrictions, including
   `MakeNeedHandle -> JoinSameKey`;
4. validate journal capacity using checked counters before allocation;
5. derive `TaskKey`;
6. for Join, look up and compare the complete existing final `TaskSpec`;
7. for AlwaysStart, read but do not yet increment the scoped counter;
8. derive ordinal, NeedId, and TaskId;
9. build the complete `TaskCorrelation`;
10. ask the adapter to prepare the derived launch envelope.

No journal field or counter is changed before adapter preparation succeeds.

### 1.2 JoinSameKey

```text
ordinal = 0
task_key = derive(generation, producer key, JoinSameKey)
need_id  = derive(producer key, JoinSameKey, 0)
task_id  = derive(task_key, 0)
```

If the exact TaskKey exists:

- policy, complete producer instance, class, priority, cancellation scope,
  outcome, request, and nonidentity scheduling semantics must agree;
- debug label differences are tolerated only as diagnostics and do not replace
  the stored label;
- the existing correlation is returned;
- no adapter prepare occurs;
- the `joined` metric increments outside semantic identity;
- observer registration remains a separate transaction.

A differing final spec under the same key is
`TaskEnsureError::JoinSpecificationConflict`; no observer or task state is
changed.

If the key does not exist, the adapter prepares one launch. Journal group,
task, Need cell, and adapter acceptance commit together.

### 1.3 AlwaysStart

The counter key is exactly:

```text
(GenerationId, NeedProducerInstanceKey)
```

The stored value is the last successfully committed ordinal. Absence means no
launch has committed; the candidate ordinal is `1`.

After adapter prepare:

1. stage the new counter value;
2. stage/validate TaskGroup launch row;
3. stage TaskJournal entry;
4. stage an initial `RuntimeNeedState` with `cursor=None` and
   `Need::Pending(Progress::indeterminate())`; this initial state is not a
   public TaskEvent;
5. atomically commit journal maps and counter;
6. invoke the adapter's infallible commit token;
7. make the task visible to polling/observers.

If any staged insertion or validation fails, call adapter rollback and discard
all staged maps. The counter remains unchanged. If adapter prepare fails, no
staging occurs.

The design requires the adapter's post-prepare commit to be infallible. An
adapter unable to meet that contract must keep all fallible work in prepare.

## 2. Concrete handle construction

### 2.1 Reusable Join handle

`RuntimeNeedHandle::try_reusable_join` derives correlation with ordinal `0`,
validates `TaskPolicy::JoinSameKey`, and stores the complete final `TaskSpec`.
It does not launch work. Observing `NotStarted` later submits the stored spec
through `ensure_task`.

### 2.2 Accepted-launch handle

`TaskHandle::try_into_runtime_need_handle(spec)` validates the returned
correlation against the exact submitted spec. This route is required for every
AlwaysStart handle and may also be used for newly accepted Join launches.

`RuntimeNeedHandleOrigin::AcceptedLaunch` cannot expose a reusable start spec.
Direct Await and timeout may consume it because they use the concrete NeedId.

### 2.3 Direct Await

Direct Await:

1. reads and validates `handle.correlation()` and `handle.need_id()`;
2. if the handle is a reusable Join handle, calls `ensure_task` with the stored
   spec and requires the returned correlation to equal the handle correlation;
3. registers the typed observer against that exact accepted correlation;
4. reads the correlated Need state;
5. never hashes, parses, or reconstructs the NeedId; and
6. validates every publication against the handle's outcome contract.

There is no direct-Await String surrogate.

## 3. Event validation and cursor precedence

A `TaskEvent` is applied in this order:

1. generation exists;
2. task ID exists;
3. entire `TaskCorrelation` byte-for-byte equals the journal task row;
4. producer instance key and producer contract agree with the stored `TaskSpec`;
5. Need row exists under the exact NeedId and has the same correlation;
6. event kind validates against outcome/request/lifecycle;
7. cursor relation is evaluated;
8. event and Need state update are staged;
9. observer invalidations are staged;
10. all staged updates commit together.

Correlation errors precede cursor errors.

### 3.1 Cursor rules

`TaskPublicationCursor.sequence` is task-local and starts at `0`.

| Incoming cursor | Existing cursor | Result |
|---|---|---|
| any `(epoch, 0)` | `None` | first event, if task accepted |
| exact same | same event digest | duplicate no-op |
| exact same | different event digest | conflict; rollback |
| lexicographically lower | any | stale no-op plus bounded audit counter |
| same/higher epoch and sequence exactly previous+1 | previous | accept |
| sequence gap | previous | `CursorGap`; rollback |
| epoch regression | previous | `EpochRegression`; rollback |
| greater after terminal | terminal cursor | `PostTerminalPublication`; rollback |

Same-step Progress then Ready uses two consecutive task-local sequences.
Normalization order remains `(logical_epoch, task_id, sequence)`.

A duplicate terminal event with the same cursor and event digest is idempotent.
A different terminal event at the same cursor is conflict. A later terminal or
progress event after terminal is rejected.

## 4. Event-to-Need mapping

| Event | Runtime Need transition |
|---|---|
| `Progress(p)` | `NotStarted/Pending -> Pending(canonical p)` |
| `Ready(payload)` | `NotStarted/Pending -> Ready(Value(payload))` |
| `InfrastructureFailure(f)` | `NotStarted/Pending -> Ready(InfrastructureFailure(f))` |
| `Cancelled` | nonterminal state -> `Need::Cancelled` |

A publication cannot leave a terminal state. The payload is validated before
the journal writes any state.

Domain errors stay inside `RuntimePayload`, for example:

```text
Ready(Value(RuntimePayload(Result::Err(domain_error))))
```

An adapter must use `InfrastructureFailure` only for host/runtime failure.
Cancellation is not an infrastructure failure and has no payload.

## 5. Terminal correlation and observer fanout

The terminal idempotence/conflict tuple is exactly:

```text
(
  GenerationId,
  NeedId,
  NeedProducerContractDigest,
  TaskPublicationCursor
)
```

The full event validation additionally requires the complete TaskCorrelation,
so a tampered TaskKey/TaskId/ordinal fails before terminal comparison.

Different AlwaysStart launches have different NeedIds and may publish
different values without conflict. Join observers reference the same one Need
row. Observer-local fields are:

- last observed cursor;
- queued invalidation;
- mount/fiber local retained state; and
- detached/cancelled status.

They never affect task identity or create another task event stream.

Observer registration and removal are bounded journal transactions. A
duplicate observer registration with identical correlation is idempotent. The
same observer key pointed at a different correlation is a conflict and rolls
back.

## 6. AwaitMany

### 6.1 Start

AwaitMany evaluates source and captured arguments exactly once. Before launch
it validates:

- source length fits `u32`;
- `limit > 0`;
- `limit` does not exceed the configured fanout limit;
- base and child producer templates are complete;
- base template family is `AwaitManyBase`;
- child template family is `AwaitManyChild`;
- payload/outcome types are compatible; and
- all canonical argument digests fit work limits.

It then constructs the source-order base argument tuple and one aggregate
`JoinSameKey` `TaskSpec`. The base Need is the concrete AwaitMany result cell.

### 6.2 Children

For each source index selected by bounded fanout, the child argument tuple
contains:

```text
(captured tuple, exact u32 index, item)
```

The child request is instantiated from the item and accepted template. Policy
is applied normally:

- Join child duplicates share only when their complete indexed instance key is
  equal;
- equal item values at different indexes do not share because index differs;
- AlwaysStart children allocate distinct ordinals per indexed instance.

`FiberAwaitManyInFlight.children` is keyed by source index, not NeedId or
iteration order. Outputs are installed only at that index. Completion order
therefore cannot reorder the final result.

### 6.3 Aggregate publication

Progress is computed from source-order child states under the retained parent
contract. The aggregate base Need owns one event stream. Child publications do
not masquerade as aggregate events.

When all children are terminal:

- all Value outcomes are validated and assembled source order;
- any InfrastructureFailure produces aggregate infrastructure failure under
  the retained parent precedence;
- any cancellation performs the retained nonreturning cancellation transfer;
- one aggregate Ready/Failure/Cancelled event is published.

The entire completion installation is rollback-safe. No partially filled final
result is visible.

## 7. Timeout

Timeout creates its own producer instance and Join cell. It observes but never
mutates/cancels the source.

The same-step order remains the maintained contract:

1. scope cancellation;
2. source terminal publication;
3. timeout expiration;
4. Pending/progress publication.

The timeout argument digest commits exact source NeedId and limit value. A
different source NeedId or limit changes the instance. A different source
debug label, generation-only TaskKey, or accepted View revision does not.

Restore validates both source handle and timeout handle independently, then
validates the exact source/output relationship before publishing either row.

## 8. Journal invariants

For each generation:

1. every group key rederives from generation, producer key, and policy;
2. Join groups contain exactly ordinal `0` and at most one task;
3. AlwaysStart groups contain only nonzero ordinals and no duplicate ordinal;
4. every task ID rederives from group key and ordinal;
5. every NeedId rederives from producer key, policy, and ordinal;
6. every task/spec/correlation contract agrees;
7. every Need row references an existing task;
8. every observer references an existing Need row;
9. every AlwaysStart counter is at least the maximum committed ordinal in its
   group and equals it after canonical compaction;
10. every terminal task has exactly one terminal Need state;
11. no event cursor exceeds the task's retained last event;
12. no String/suffix identity field exists.

Validation iterates BTreeMap key order. It returns the first error in the order
above and publishes no partial journal.

## 9. Save and snapshot encoding

The version-1 snapshot envelope stores:

- generation journals;
- scoped AlwaysStart counters;
- group policy/producer/ordinal membership;
- complete TaskSpec and TaskCorrelation;
- last event/cursor;
- RuntimeNeedState;
- observer state;
- RuntimeNeedHandle snapshots embedded in runtime values/fibers;
- AwaitMany in-flight state; and
- replacement mappings where the parent save contract retains them.

Fixed identities are raw 32-byte fields. Integer wire encoding reuses the
maintained v1 private Wire owner. There is no old String reader, hex reader,
suffix parser, compatibility tag, or dual schema.

Restore is four-phase and atomic:

1. decode into bounded private DTOs;
2. validate ordering/duplicates/version/limits;
3. rederive all digests and identities and validate cross-references;
4. construct final maps and publish one complete runtime state.

Any error drops private DTOs and leaves the existing runtime untouched.

## 10. Replay

Replay envelopes contain complete TaskEvent correlation and event digest.
Replay:

1. resolves the exact generation;
2. rederives event digest;
3. applies the normal event validation path;
4. obtains the same duplicate/stale/conflict outcome as live execution; and
5. produces identical observer invalidations and Need state.

There is no replay-only identity conversion or relaxed cursor rule.

## 11. Hot replacement

### 11.1 Admission

An explicit old/new `CheckedViewMatchCoordinate` mapping is mandatory. Before
quiescing tasks, compare all fields in `ViewNeedRebindEvidence`.

Accepted revision equality is deliberately not required. Revision values are
validated as catalog/bundle revisions, recorded in the transaction, and kept
out of producer/task identity.

### 11.2 Rebind transaction

For every accepted live mapping:

1. pause event ingestion at a deterministic barrier;
2. compute the new generation;
3. preserve producer instance, policy, ordinal, NeedId, cursor, Need state, and
   observer-local retained state;
4. rederive TaskKey from the new generation;
5. rederive TaskId from the new TaskKey and preserved ordinal;
6. construct a new complete correlation;
7. ask the host adapter to prepare old->new correlation rebind;
8. stage updates to task, Need, group, observer, Await/AwaitMany, save, and View
   runtime rows;
9. commit all runtime rows and adapter rebind together;
10. remove the old correlation; no translation table survives commit.

For a terminal task, adapter rebind may be a validated no-op token. For an
in-flight task, an adapter that cannot prepare rebind causes the affected state
to cancel; it does not preserve an alias.

### 11.3 Mismatch behavior

| Mismatch | Result |
|---|---|
| revision only | retain/rebind |
| missing explicit site mapping | cancel affected live state |
| generic Match digest | cancel |
| View admission digest | cancel |
| producer admission digest | cancel |
| producer contract/family | cancel |
| payload type | cancel |
| plan digest | cancel |
| ownership evidence | cancel |
| resource dependency | cancel |
| arguments digest | cancel |
| adapter rebind refusal | cancel |

Cancellation and the replacement catalog publication form the retained parent
transaction. No partially rebound observer or task is visible.

## 12. Rollback guarantees

The following operations expose either the complete previous state or complete
new state:

- task ensure/ordinal allocation;
- event application;
- observer registration/fanout;
- AwaitMany child start and aggregate completion;
- timeout construction;
- save restore;
- replay event;
- View replacement/rebind.

Metrics and diagnostic counters update only after semantic commit or in a
separate nonidentity audit channel that cannot affect retry results.
