# Ownership transaction and commit contract

## 1. State machine

```text
RuntimeOwnershipTransaction
        |
        | prepare(store)
        v
RuntimePreparedOwnershipTransaction
        |
        +-- abort(...) ------------------> RuntimeAbortedOwnershipTransaction
        |
        | try_commit(store)
        v
  acquire/revalidate all participants
        |
        +-- mismatch --------------------> RuntimeTransferCommitError
        |                                  owns RuntimeAbortedOwnershipTransaction
        |
        v
RuntimeCommitPermit
        |
        | commit_permit(store)  [infallible]
        v
RuntimeCommittedOwnershipTransaction
```

No owner has a backward transition. Prepared and aborted owners are not Clone
and cannot be serialized. A transaction ID is not reused.

## 2. Whole-slot rule

Copy, Move, and Drop transfer one complete storage slot value. A nested
`RuntimeValuePath` identifies an affine owner or graph error *inside* that value;
it is not a second storage location.

This preserves one storage authority and makes the slot revision the complete
race detector. Parent pattern/capture/aggregate plans may create typed temporary
slots and one transaction, but they must not add a partially moved parallel
runtime-value model or a path-keyed side table.

## 3. Preparation phases

Preparation is deterministic and leaves all values unchanged.

### P0 — allocate transaction-local containers

Using `try_reserve_exact`, reserve bounded capacity for:

- indexed plan steps;
- participant observations;
- prepared transfers;
- traversal stack and current path;
- affine-owner occurrence evidence;
- commit mutations; and
- committed evidence.

Every requested size is checked against hard and configured limits before
`try_reserve_exact`. Allocation failure maps to
`RuntimeOwnershipPrepareErrorKind::AllocationFailed`; no allocator message or
platform code enters diagnostics.

### P1 — normalize and index plan

Assign one-based `RuntimeTransferStepIndex` in vector order. Reject:

- empty plans;
- step count above limit;
- same source and destination;
- step index overflow;
- conflicting slot participation; and
- a participant whose execution differs from the transaction execution.

A slot may occur in more than one step only as `CopySource`, and only when every
occurrence has the same expected revision and accepted type. The participant
budget counts this coalesced source once; the step and staged-byte budgets count
every Copy step independently. Such occurrences
form one unique participant and one integrated source reservation while each
Copy step still stages its own complete duplicate and evidence. A destination
may occur only once. A Move or Drop source may occur only once and may not also
be a Copy source or destination. The conflicting-participant rule is evaluated
before any store lookup. The first step is the lower index, and the second step
reports the conflict.

### P2 — observe slots

Observe participants in canonical `RuntimeOwnedSlotId` order, not plan order.
For every participant capture:

- exact private storage handle;
- typed slot ID;
- declared type;
- revision;
- state kind;
- live value reference where required; and
- current reservation.

Reject missing/identity-mismatched storage as an identity error before revision
or occupancy checks.

### P3 — revision and reservation

For each participant:

1. compare expected and actual revision;
2. reject any existing reservation;
3. precompute `checked_next` for every mutation; and
4. retain the next revision.

No reservation is installed yet. Revision exhaustion precedes budget/allocation
errors discovered later but follows value/type/owner errors according to the
global precedence table; preparation therefore records all candidates and
selects after validation.

### P4 — source/destination state and type

Copy/Move/Drop source must be `Live`. Copy/Move destination must be `Vacant`.
The endpoint's accepted `RuntimeCheckedType` must equal the storage cell's
declared type. Copy/Move source and destination accepted types must also be
equal.

Type checks occur before value traversal. Diagnostic names are never consulted.

### P5 — one canonical value traversal

Traverse every unique live source exactly once in canonical slot order, using
the same path emitter as the shipped ownership classifier. Repeated compatible
Copy-source references share this observation and owner/path evidence. The traversal simultaneously:

- joins `RuntimeValueOwnership`;
- validates record/capture identity continuity;
- counts value nodes and path depth;
- collects every `RuntimeAffineOwnerOccurrence`;
- rejects duplicate affine-owner IDs across the full transaction;
- produces the first canonical invalid path/layout error; and
- computes the exact checked-duplication work for Copy.

A second owner scan, snapshot scan, or diagnostic scan is prohibited.

For Copy, the first affine occurrence produces `AffineCopy`. All occurrences are
still bounded and duplicate-checked so error precedence remains deterministic.

For Move/Drop, owner occurrences are retained in committed evidence.

### P6 — checked Copy staging

For each unrestricted Copy step, in original plan order, call the parent-owned
checked unrestricted duplication method with the remaining
value-node/path/staged-byte budget. Repeated compatible Copy-source references
therefore produce one independent staged value per destination without exposing
`Clone`. The destination remains vacant.

Move and Drop do not take or clone the source.

### P7 — build evidence and commit mutations

Construct, before reservation:

- Copy evidence;
- two independently owned, byte-identical Move evidence values: one source
  tombstone and one committed-result value;
- two independently owned, byte-identical Drop evidence values: one source
  tombstone and one committed-result value;
- next revisions;
- exact private handles;
- commit mutations; and
- committed result vectors.

All vectors are exact-capacity and no later growth is needed.

### P8 — install integrated reservations

Install reservations in canonical slot order. Each reservation records
transaction ID, expected revision, and role in the actual storage cell.

If any install unexpectedly fails, clear only reservations installed for this
transaction in reverse canonical order, return the untouched transaction, and
select the canonical prepare error. Values and revisions remain unchanged.

After P8 succeeds, return `RuntimePreparedOwnershipTransaction`.

## 4. Prepare failure owner return

Every prepare error owns the original `RuntimeOwnershipTransaction`, including
its plan. The caller can inspect or correct it and start a *new* transaction
with a new ID. The failed transaction itself may be passed to `prepare` again
only when preparation installed no reservation; the public API does not expose
a mutable “resume preparation” state.

Error-to-owner behavior:

| Error | Reservations on return | Values/revisions | Returned owner |
|---|---|---|---|
| execution mismatch | none | unchanged | original transaction |
| conflicting participant | none | unchanged | original transaction |
| stale revision | none | unchanged | original transaction |
| existing reservation | none installed by caller | unchanged | original transaction |
| source not live | none | unchanged | original transaction |
| destination not empty | none | unchanged | original transaction |
| type mismatch | none | unchanged | original transaction |
| invalid path/layout | none | unchanged | original transaction |
| duplicate owner | none | unchanged | original transaction |
| affine Copy | none | unchanged | original transaction |
| revision/identity exhaustion | none | unchanged | original transaction |
| budget | none | unchanged | original transaction |
| allocation | none | unchanged | original transaction |
| reservation-install mismatch | caller reservations cleared | unchanged | original transaction |

## 5. Commit acquisition

`try_commit` first calls the sealed store's `acquire_commit_permit`.

Under the executor's exclusive mutation boundary it re-observes every
participant in canonical slot order and checks:

1. current execution equals transaction execution;
2. storage occurrence still exists;
3. private handle resolves to the same typed slot;
4. reservation equals `(transaction, expected revision, role)`;
5. revision equals the prepared revision;
6. source remains Live or destination remains Vacant as prepared;
7. declared type is unchanged; and
8. one canonical traversal of each Move/Drop source produces the exact prepared
   affine-owner IDs and paths.

The last check detects a core-internal mutation that bypassed revisioning. It is
not a second ordinary prepare scan: it is the commit mismatch assertion and is
bounded by the already reserved buffers.

If any check fails:

- no source has been taken;
- no destination has been written;
- all matching reservations are cleared;
- staged Copy values remain owned by the aborted owner;
- the exact mismatch is selected by commit precedence; and
- `RuntimeTransferCommitError` returns one
  `RuntimeAbortedOwnershipTransaction`.

The aborted owner has no commit method.

## 6. Permit construction — exact infallibility boundary

Only after all commit checks pass, `acquire_commit_permit` moves the already
prepared values/evidence/handles into `RuntimeCommitPermit`.

**The return of `Ok(RuntimeCommitPermit)` is the exact point after which commit
is infallible.**

From that point:

- participant identity is frozen by the executor's exclusive mutable access;
- every source is Live;
- every destination is Vacant;
- every revision has its precomputed successor;
- every Copy duplicate is already owned;
- every tombstone/evidence object is already owned;
- every vector has final length and capacity;
- no policy/type/path/owner validation remains; and
- no host/runtime callback can run.

## 7. Commit mutation order

Mutations execute in original plan step order, with precomputed disjoint
participants:

### Copy

1. move the staged duplicate into destination;
2. write destination next revision;
3. append/move prebuilt Copy evidence to the committed owner.

Source value and revision are unchanged by Copy.

### Move

1. `mem::replace` source Live state with the prebuilt Moved tombstone;
2. move the exact taken value into destination;
3. write source and destination next revisions;
4. move the separately prebuilt Move evidence into the committed owner.

There is no branch between step 1 and step 2. The destination has already been
validated and exclusively reserved.

### Drop

1. `mem::replace` source Live state with the prebuilt Dropped tombstone,
   dropping the exact taken value as part of the mutation's owned local;
2. write source next revision;
3. move the separately prebuilt Drop evidence into the committed owner.

### Reservation release

After all mutations complete, clear every unique participant reservation in the
permit's canonical reservation-handle order. Reservation release is not
interleaved with step mutation, so one source reservation safely covers any
number of compatible Copy steps. The release loop is infallible and performs no
lookup by public slot identity.

Destructors for current unrestricted values are inert Rust drops. A later
affine leaf must route external cleanup into pre-staged Sans-I/O cleanup
requests before permit construction; it may not make Drop commit fallible.

## 8. No-fallible-branch rule

The implementation review must inspect the concrete `commit_permit` call graph.
After permit construction, it must contain no:

- `Result`, `Option`-based failure, `?`, `return Err`, or semantic `match`;
- `try_reserve`, allocation, clone, checked duplicate, or string formatting;
- checked/saturating arithmetic;
- BTree/hash lookup by semantic ID;
- name lookup;
- type, layout, path, owner, budget, policy, or capability check;
- scheduler, provider, host, replay, View, or Stream callback;
- lock acquisition; or
- panic used as ordinary control flow.

Indexing by prevalidated private handles may use a private unchecked/invariant
primitive in the existing storage owner. Its safety contract is the permit; it
is not public and cannot be called before commit acquisition. Debug assertions
belong before the first take in debug-only permit validation, not between take
and install.

## 9. Commit mismatch owner return

| Commit mismatch | Value state on return | Reservation state | Owner returned |
|---|---|---|---|
| wrong execution | unchanged | caller reservations cleared | aborted transaction |
| slot missing | unchanged | remaining caller reservations cleared | aborted transaction |
| identity mismatch | unchanged | cleared | aborted transaction |
| reservation mismatch | unchanged | matching caller reservations cleared | aborted transaction |
| revision mismatch | unchanged | cleared | aborted transaction |
| occupancy mismatch | unchanged | cleared | aborted transaction |
| type mismatch | unchanged | cleared | aborted transaction |
| owner mismatch | unchanged | cleared | aborted transaction |

“Unchanged” means byte-identical canonical snapshot of all live values,
revisions, and persisted evidence from commit entry. Clearing transient
reservations is intentionally excluded from snapshot bytes.

## 10. Deterministic first error

Preparation records candidates then selects:

```text
rank 0: execution mismatch, conflicting participant
rank 1: stale revision, slot already reserved
rank 2: source not live
rank 3: destination not empty
rank 4: type mismatch, invalid path, invalid record layout
rank 5: duplicate affine owner
rank 6: affine Copy
rank 7: revision/identity exhaustion
rank 8: budget
rank 9: allocation
```

Commit mismatch selects:

```text
rank 0: wrong execution
rank 1: slot missing
rank 2: slot identity mismatch
rank 3: reservation mismatch
rank 4: revision mismatch
rank 5: occupancy mismatch
rank 6: type mismatch
rank 7: owner mismatch
```

Within a rank compare:

1. `RuntimeOwnedSlotId`;
2. `RuntimeValuePath`;
3. `RuntimeAffineOwnerId`;
4. step index.

Variants without a component use the minimum sentinel for that component.
Selection is an inherent method on the owning error enum.

## 11. Mutation revision rules

| Operation | Source revision | Destination revision |
|---|---|---|
| Copy | unchanged | +1 |
| Move | +1 | +1 |
| Drop | +1 | n/a |
| mutable assignment | +1 | n/a |
| read/borrow | unchanged | n/a |
| failed prepare | unchanged | unchanged |
| commit mismatch | unchanged | unchanged |
| abort prepared | unchanged | unchanged |

A moved/dropped tombstone stores evidence whose revision field names the
pre-commit source revision; the slot itself stores the post-commit revision.
This allows a diagnostic to state both the last live version and current
tombstone version without ambiguity.

## 12. Use-after-move and use-after-drop

`RuntimeBinding::value` and corresponding capture/AWBC/mailbox accessors inspect
the typed `RuntimeSlotState`.

- Moved reports slot, transaction, destination, and owner-path evidence.
- Dropped reports slot, transaction, reason, and owner-path evidence.
- Vacant reports an uninitialized destination, not a dropped value.

No diagnostic scans trace text or parses canonical rendering. Source maps may
decorate the typed error later but do not establish identity.

## 13. Suspension, unwind, child, transfer, and cleanup

A suspension may retain prepared transactions only if the parent safe-point
contract explicitly allows it. This correction selects the safer invariant:
**save and externally visible suspension boundaries require zero prepared or
reserved ownership transactions.**

Before suspension:

- ordinary transfer must commit or abort;
- unwind builds a Drop transaction in canonical cleanup-slot order;
- child completion transfers or drops every packet;
- mailbox handoff commits before the receiver becomes runnable; and
- cleanup requests are staged before Drop permit construction.

The existing save blocker reports the exact nonzero transaction count.

## 14. Allocation behavior

All recoverable allocations use checked reserve paths before reservation.
Global allocator abort remains outside the language contract; implementation
must not claim ordinary OOM recovery for allocation sites the standard library
cannot expose. The matrix requires injected/fallible allocator tests around the
explicit `try_reserve` and parent checked-duplication boundaries.

An allocation error never consumes a source, fills a destination, advances a
revision, or leaves a reservation.

## 15. Parity requirement

Structured execution, AWBC, compiled-region exchange, mailbox transfer,
child packets, and cleanup use the same transaction owner and evidence. They
may provide owner-specific storage-handle resolution through the sealed store,
but they cannot implement independent Copy/Move/Drop algorithms or reduced
slot enums.
