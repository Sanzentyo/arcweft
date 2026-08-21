# Runtime state machines

## 1. Launch lifecycle

`TaskLifecycle` is the journal projection. Host/runtime executable state is
separate but must agree with it.

| Current | Input | Guard | Next | Publication |
|---|---|---|---|---|
| absent | accepted new Join/AlwaysStart spec | full ensure transaction succeeds | `Accepted` | group, launch, Need and runtime/host row together |
| `Accepted` | host dispatch acknowledged or first runtime step | correlation exact | `Running` | lifecycle only |
| `Accepted`/`Running` | `Progress` | cursor valid, nonterminal | same | Need `Pending(progress)` and observers |
| `Accepted`/`Running` | `Ready(payload)` | payload contract exact | `Ready` | Need `Ready(Payload)` once |
| `Accepted`/`Running` | `InfrastructureFailure` | typed/bounded | `InfrastructureFailed` | Need `Ready(InfrastructureFailure)` once |
| `Accepted`/`Running` | cancellation | scope/correlation exact | `Cancelled` | Need `Cancelled` once |
| terminal | exact duplicate cursor+digest | exact | terminal | no semantic mutation |
| terminal | any new publication | correlation first validates | error | none |

Join existing does not perform a lifecycle transition. It returns the existing
handle only after exact structural spec equality.

## 2. Event cursor machine

For each task, cursor state is absent or `(logical_epoch, sequence, digest)`.

| Incoming relation | Result |
|---|---|
| no stored cursor and accepted first cursor | apply |
| greater accepted next cursor | apply |
| same cursor and same digest | duplicate success; no mutation |
| same cursor and different digest | `EventConflict` |
| lower cursor | stale success/audit; no semantic mutation |
| gap not allowed by event contract | `CursorGap` |
| epoch regression | `EpochRegression` |
| complete correlation mismatch | typed correlation error before cursor comparison |

Event digest transcript is retained:

```text
BLAKE3(
  "arcweft.task.event.v1\0"
  || complete TaskCorrelation
  || logical_epoch:u64-le
  || sequence:u64-le
  || TaskEventKind transcript
)
```

## 3. AwaitMany aggregate

### 3.1 Owned state

The aggregate owns:

- its complete output correlation;
- exact `source_items` length and order;
- one complete child `TaskSpec` per source index;
- child observer/handle/correlation/status rows;
- bounded launch cursor and in-flight count;
- output slots keyed by source index;
- aggregate publication cursor;
- terminal state.

### 3.2 Child state

```text
NotLaunched
  --successful ensure + observer registration--> Waiting
Waiting
  --Ready(payload)--> Ready
Waiting
  --InfrastructureFailure--> InfrastructureFailed
Waiting
  --Cancelled--> Cancelled
```

No child row can revert or accept two different terminal events.

### 3.3 Step

1. If aggregate cancellation is pending, detach children and publish
   cancellation.
2. Apply normalized child terminal events in source-index order.
3. If any child is InfrastructureFailed, publish aggregate infrastructure
   failure according to the deterministic first failing source index.
4. If any child is Cancelled, publish aggregate cancellation according to the
   maintained cancellation contract.
5. If every child is Ready, materialize the output sequence in exact source
   order and publish Ready once.
6. Otherwise select source indices from `launch_cursor` while
   `in_flight < limit`, bounded by per-step work.
7. Atomically ensure/register the selected batch.
8. Publish Pending progress derived from completed count, without changing
   semantic identity.

The request's terminal precedence is expressed as:

```text
aggregate/scope cancellation
< normalized child cancellation or infrastructure terminal
< all-ready aggregate terminal
< Pending/progress
```

Within the child infrastructure set, lowest source index wins the aggregate
diagnostic; all child terminal rows remain persisted for audit/replay.

Children can be Host or Runtime because each child row is a complete
`TaskSpec`. The aggregate itself is never Host.

## 4. Timeout

### 4.1 State

```text
NotStarted {
  remaining = requested_limit
  source_observer = None
}
Waiting {
  remaining
  source_observer = Some
}
Resolved {
  terminal
}
```

### 4.2 First demand

The first runtime step/observer demand:

1. validates output and source handle structures;
2. validates source generation against active scheduler generation;
3. reads the source Need before time subtraction;
4. if source is terminal, resolves immediately;
5. otherwise registers a `TimeoutSource` observer and enters Waiting.

### 4.3 Waiting step

The only clock input is `RuntimeStepInput.dt`.

For every step:

```text
1. wrapper/scope cancellation
2. normalized source terminal publication
3. subtract dt using checked/saturating logical-duration arithmetic
4. expiration when remaining reaches zero
5. Pending publication
```

For zero duration, step 2 still occurs before expiration. Source Ready payload
passes through. Source infrastructure failure remains typed. Expiration
publishes the timeout domain result required by the maintained
`need-timeout.md` contract. Cancelling the wrapper detaches its observer but
does not cancel the source.

### 4.4 Snapshot invariants

- `remaining <= requested_limit`;
- NotStarted has no source observer/cursor/terminal;
- Waiting has one valid source observer and no terminal;
- Resolved has a terminal and no future stepping;
- output/source typed identities and timeout contract digest revalidate.

## 5. Cancellation transaction

Cancellation input is `(generation, cancel_scope)`.

1. validate generation and scope;
2. select affected launches/Needs/observers/runtime tasks in canonical TaskId
   order;
3. build a bounded cancellation delta;
4. detach observer links and mark runtime tasks terminal;
5. publish all Need cancellations and launch lifecycle transitions together;
6. issue any Host-only adapter cancellation action through a separately
   prepared/infallible transaction if the adapter contract owns cancellation.

Cancellation never fabricates a domain payload or infrastructure failure. A
later event validates correlation and then fails as post-terminal.

## 6. Snapshot/restore machine

```text
Live
  --snapshot request, no prepared adapter token, all required host rows quiescent-->
ProjectedV1
  --strict encode--> BytesV1

BytesV1
  --strict decode--> PrivateRows
  --identity/join/runtime validation--> ValidatedRestore
  --adapter prepare--> PreparedRestore
  --atomic state swap + adapter commit--> LiveRestored
```

Any error before the final arrow leaves the prior Live state unchanged.
PreparedRestore is rolled back. There is no partial publication.

## 7. Replay machine

```text
EnvelopeV1
  -> version/generation
  -> event digest
  -> complete correlation
  -> cursor relation
  -> normal live EventApplyDelta
  -> atomic apply
```

Replay does not bypass terminal or observer rules.

## 8. Replacement machine

```text
Idle
  --validate request/products/mappings--> Validated(plan)
Validated
  --snapshot permitted--> Validated snapshot row
Validated
  --quiescent barrier + stage generation--> Staged
Staged
  --adapter prepare rebind--> Prepared (not snapshot-persistable)
Prepared
  --precommit failure--> rollback -> Validated/Idle with old live generation
Prepared
  --atomic scheduler swap + infallible adapter commit--> Idle on new generation
```

A mapping row must preserve producer instance, policy, ordinal and NeedId.
Generation, TaskKey and TaskId must be the exact rederived new values.
Revision equality is not required; accepted revision validity and semantic
product joins are required.

## 9. Failure category table

| Source | Stored Need state | Await behavior |
|---|---|---|
| successful/domain-success payload | `Ready(Payload)` | returns payload |
| domain `Result::Err` or `Option::None` | `Ready(Payload)` | returns the ordinary typed carrier; `try` may propagate |
| host/runtime infrastructure failure | `Ready(InfrastructureFailure)` | typed runtime failure |
| cancellation | `Cancelled` | nonreturning cancellation |
| pending work | `Pending(Progress)` | suspension |

No conversion between these categories is permitted.
