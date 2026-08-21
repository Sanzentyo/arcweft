# Identity, lifetime, ownership, and failure tables

## 1. Identity construction

### 1.1 Dialogue activation

```text
DialogueActivationId = {
  artifact      = accepted RuntimeArtifactFingerprint,
  owner_fiber   = persisted parent Flow fiber id,
  content       = RuntimeDialogueContentPlanId,
  occurrence    = parent.dialogue_occurrence_counter before increment,
}
```

Allocation is the first mutation of the dialogue transaction.  The occurrence
counter is checked, incremented once, and included in snapshots.  Host time,
thread id, pointer address, renderer id, random data, and source labels are not
inputs.

### 1.2 Handle token

```text
RuntimeLineHandleToken = {
  activation,
  site,
  issuance = ledger.issuance_by_site[site] before increment,
}
```

Issuance increments only when the complete resource/cue/voice lease can be
inserted.  Failed argument or capture evaluation therefore does not consume an
ordinal.  Host rejection does consume the issued identity and moves it to
`Failed`; replay observes the same identity and failure.

### 1.3 Multiple handles at one source site

| Situation | Identity result |
|---|---|
| same site, same activation, first execution | issuance `0` |
| same site, same activation, loop/re-entry | issuance `1`, `2`, ... |
| same source site, next dialogue occurrence | distinct activation occurrence; issuance starts at `0` |
| replay | restored counters and event order reproduce the same tokens |
| save/restore while pending | exact token and next issuance retained |
| hot replacement | active activation remains on old generation; new activation gets new artifact fingerprint |

## 2. Equality and nesting

A line handle compares equal only when all of the following are equal:

1. opaque producer;
2. opaque semantic identity;
3. opaque value class and persistence;
4. full `RuntimeLineHandleToken` payload.

Two handles to the same host voice or actor resource are not equal if their
lease tokens differ.  This avoids accidental authority aliasing.  Handles may
be nested in tuples, records, variants, sequences, nominal records, function
captures, and dialogue results.  Recursive ownership becomes affine as soon as
one nested handle is encountered.  Copy admission therefore rejects the whole
containing graph.

## 3. Owner-slot transitions

| Event | Required prior owner | New owner/state | Host action |
|---|---|---|---|
| operation issues unbound handle | none | current `LineScope` or `ChildScope` | kind-specific acquire/arm/start request if required |
| `let x = operation()` | implicit current scope | `ActivationLocal(x)` | none |
| ordinary expression statement | implicit current scope | unchanged implicit scope | none |
| explicit `let _ = operation()` | implicit current scope | `Released` | execute kind-specific typed drop |
| move local to local | source local | destination local | none |
| commit result containing handle | local/implicit line owner | `DialogueResult(path)` | none |
| successful result pattern bind | `DialogueResult(path)` | `ParentFiber(slot)` | none |
| successful result pattern `_` | `DialogueResult(path)` | `Released` | typed drop after full pattern validation |
| explicit `drop(x)` | owner slot for x | `Released` or `Cancelling` | kind-specific typed command |
| child capture by move | current owner | `ChildScope(work_tag)` | none |
| joined child end | child scope | line scope or released per lexical scope | typed drop for remaining child-owned leases |
| detached child | child scope only | detached child's own runtime owner; never line result | no implicit promotion |

A transfer checks producer, generation, activation, token existence, expected
owner slot, state, and destination legality before mutation.

## 4. Exit behavior

| Exit | Scheduled/joined children | Line-owned handles | Committed result | Parent result pattern |
|---|---|---|---|---|
| normal dialogue advance | drain due cues, cancel future cues, join required work, completed cleanup | reverse stable issuance order after excluding result-owned leases | retained through cleanup | publish once, then resume |
| completing cancellation rule | execute rule, close/join, cancelled cleanup | same typed unwind | rule/activation must have one committed R | publish once if rule contract is completing |
| non-completing cancellation | cancel/join, cancelled cleanup | typed unwind | abandon and drop all affine leaves | not evaluated |
| callback failure | mark failed, cancel/join siblings, failed cleanup | typed unwind | abandon | not evaluated; failure propagates |
| host rejection | same as operation/callback failure | typed unwind | abandon if already committed | not evaluated |
| `return` / `goto` from owning flow while suspended | close activation before control transfer | typed unwind | abandon | not evaluated |
| engine shutdown | deterministic cancelled close | typed unwind or snapshot transfer if saving | snapshot or abandon | not evaluated during shutdown |

Stable unwind order is:

1. reject new activation operations and cue issuance;
2. mark close reason;
3. cancel due/pending work in `(deadline, site, issuance, node)` order;
4. await joined cancellation;
5. run exactly one cleanup function for the exit reason;
6. drop remaining child-scope handles in reverse issuance order;
7. drop remaining line-scope handles in reverse issuance order;
8. publish or abandon result;
9. release dialogue presentation/voice owner;
10. resume or propagate control.

Result-owned handles are not touched by steps 6–7.  They are transferred or
dropped by step 8.

## 5. Kind-specific drop behavior

| Handle kind/state | Drop behavior |
|---|---|
| StageActor / Allocating | cancel acquire if supported, otherwise await reply then immediately release |
| StageActor / Active | enqueue typed `ReleaseActor`; mark `Released` after outcome |
| StageActor / Released | structured double-drop/use-after-move error if explicitly addressed again |
| Cue(schedule) / Pending | cancel scheduled child before fire and join cancellation if policy is joined |
| Cue(schedule) / Running | request child cancellation according to the node policy; joined owner waits |
| Cue(schedule) / Completed/Cancelled/Failed | release token only; no second callback action |
| Cue(stage look) / Allocating/Pending | enqueue typed `CancelCue`; preserve command ordering |
| Cue(stage look) / Completed | release token only |
| Voice / Active | release one lease; stop/fade only when policy says so and last lease is gone |
| Voice / Completed | release token only |

The dispatch key is `RuntimeHandleKind` plus the typed ledger resource/state.
Display labels are never inspected.

## 6. `scope=line`, export, and `_`

`scope=line` defines the default owner, not an unconditional destruction time.
A handle remains line-owned unless moved.  If `out` moves it to the result cell,
normal line cleanup excludes it.  Successful parent binding then extends its
lifetime to the receiving parent scope.  An outer `_` explicitly drops it at
that publication boundary.

Pattern matching is two-phase:

```text
phase A: validate shape, literals, types, all affine source paths, and all
         destination ownership slots without mutation;
phase B: apply ordinary bindings, affine transfers, and explicit drops in
         canonical RuntimeValuePath order.
```

A mismatch in phase A leaves the parent environment, ledger, result cell, and
host command queue unchanged.

## 7. Joined and detached work

- `Join` means dialogue close cannot publish `R` until the child and its typed
  cleanup are terminal.
- `Detached` means the child is removed from dialogue close accounting only
  after admission proves that it captures no line-affine handle, no line
  context capability, no result-cell authority, and no activation-local
  reference.
- `Finish` cancellation remains joined for publication accounting until the
  child finishes.
- A scheduled `at` child defaults to `Join + CancelAndJoin`.
- There is no implicit conversion from line handle to detached handle.  A
  future detach feature requires a distinct exact producer and is outside this
  cut.

## 8. Failure precedence at operation time

When an operation receives a handle/look/activation, checks run in this order:

1. runtime value family and exact checked type;
2. opaque producer;
3. opaque semantic identity/value class/persistence;
4. token payload shape;
5. artifact generation;
6. dialogue activation;
7. handle site and kind;
8. owner slot and move state;
9. resource state;
10. Character/look ownership;
11. operation-specific arguments;
12. host outcome.

The first failing category determines the diagnostic.  This order is shared by
structured and AWBC execution.
