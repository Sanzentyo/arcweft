# Snapshot, activation, restore, replay, and hot-replacement contract

## 1. Exclusivity scope

The exclusivity scope is one shared `RuntimeExecutionDomain` owned by the
runtime host. It spans every `RuntimeDriver`/session created by that host.

A candidate is not active merely because it owns a decoded session. It becomes
active only when its non-Clone domain reservation is consumed under the domain
lock. This closes the parent defect in which two copied images could be
installed into two distinct drivers.

The domain may contain at most one reservation and one active record. A
replacement reservation may name the active execution, but no second active
record is ever published.

## 2. Snapshot envelope

The existing parent save-schema-2 image is extended directly with:

```text
RuntimeExecutionDomainSnapshotV2
  next_execution
  activation_epoch
  active: RuntimeExecutionIdentitySnapshotV2
    execution
    next_occurrence
    next_local_slot
    next_ownership_transaction
    next_affine_owner
```

The active execution payload retains the parent-owned whole-execution snapshot,
generation pins, deterministic replay evidence, and save version. The identity
envelope is inside the canonical digest and inside the same atomic save object.

No adjacent identity sidecar, second save file, compatibility alias, optional
legacy field, or inferred cursor is permitted.

## 3. Persisted versus rebuilt state

### Persisted

- execution ID;
- activation epoch;
- domain next-execution cursor;
- occurrence, local-slot, transaction, and affine-owner cursors;
- every dynamic scope/closure/fiber/frame/mailbox/child/transfer/cleanup ID;
- every environment local slot ID and declaration ID;
- every capture slot and dynamic closure ID;
- every AWBC register/frame-local owner occurrence;
- mailbox lane, child packet, transfer packet, and cleanup-slot IDs;
- slot revision;
- Vacant/Live/Moved/Dropped state;
- moved/dropped transaction evidence;
- affine-owner IDs and canonical paths where evidence stores them;
- record-field IDs;
- existing parent execution payload, generation pins, and deterministic replay
  facts.

### Rebuilt

- `Arc`, mutex, and condition-variable identity;
- private storage handles and vector indices;
- name-to-binding and ID-to-index lookup maps;
- BTree/hash caches and diagnostic rendering caches;
- spare scope/frame/vector capacity;
- live transaction reservations;
- prepared transactions, commit permits, staged duplicates, and transient
  allocation buffers;
- HIR-to-runtime-plan projection maps;
- source span decoration indexes;
- provider/socket/file handles;
- runtime thread/task handles; and
- any iterator implementation cache not part of the parent semantic state.

Rebuilt state cannot influence canonical digest or owner identity.

## 4. Save admission

Save runs only through `RuntimeActiveExecution`.

Before snapshot construction:

1. verify the domain active record matches the active owner;
2. reject any nonzero prepared/reserved ownership transaction count using
   `OwnershipTransactionActive { count }`;
3. require the parent safe-point and closed-host-work conditions;
4. freeze the executor under its existing exclusive snapshot boundary; and
5. traverse the complete live graph once.

Save never serializes live `RuntimeBinding` or `RuntimeValue` directly. It
projects to the parent closed snapshot carriers and this correction's identity
evidence.

A blocked save performs no partial output publication and does not consume the
active execution.

## 5. Floating snapshot equality

Live `f32`/`f64` values project to `RuntimeSnapshotF32Bits(u32)` and
`RuntimeSnapshotF64Bits(u64)`. The exact bit patterns persist:

- positive/negative zero remain distinct;
- NaN payload and sign bits remain distinct;
- infinities round-trip;
- no textual decimal normalization occurs.

The closed `RuntimeValueSnapshotV2` derives/implements `PartialEq`, not `Eq`.
Snapshot/digest tests compare canonical bytes and bit wrappers, not live float
`==`.

## 6. Twelve-stage restore/replay candidate validation

Stages are strict and stop at the first error in this exact order. No stage
mutates the active domain/session.

### Stage 1 — envelope and size

- verify outer save version and required sections;
- reject old/new unknown version;
- check section counts/length arithmetic;
- reject duplicate, missing, unknown, or trailing sections;
- enforce configured/hard byte limits.

### Stage 2 — canonical scalar decode

- decode strict decimal strings/fixed-width integers;
- reject zero nonzero IDs, overflow, malformed JSON, invalid UTF-8/BOM,
  duplicate/unknown fields, and unknown enum tags;
- reject non-canonical alternative encodings.

### Stage 3 — immutable program/bundle identity

- verify existing bundle/program/artifact fingerprints;
- verify parent ABI/codec/save compatibility;
- verify generation pins and immutable catalog identities;
- reject hot-replacement program mismatch according to the parent rules.

### Stage 4 — execution envelope

- require exactly one active identity snapshot;
- verify every nested execution ID equals the envelope execution;
- validate activation epoch nonzero;
- validate domain next-execution cursor relative to execution ID: `Next(n)`
  requires `n > execution`, while `Exhausted` requires
  `execution == u64::MAX`;
- validate source mode is Restore or Replay for preserved images.

### Stage 5 — owner-domain structure

- validate unique scope/closure/fiber/frame/mailbox/child/transfer/cleanup IDs;
- validate owner-local lane/packet/slot IDs are contiguous/unique where the
  owning aggregate requires it;
- validate every `RuntimeOwnedSlotId` variant resolves to exactly one storage
  occurrence;
- reject reduced/unknown variants.

### Stage 6 — local/capture/record structure

- validate runtime local slot uniqueness across all scopes;
- validate declaration IDs resolve in the accepted runtime plan;
- validate shadowing through scope order, not duplicate-name rejection across
  scopes;
- validate capture IDs against each closure plan;
- validate anonymous/column record field IDs and duplicate names;
- validate nominal record arity/layout field IDs.

### Stage 7 — revisions and slot states

- require every slot revision nonzero;
- require one well-formed Vacant/Live/Moved/Dropped state;
- validate tombstone evidence refers to that slot and that the cell revision is
  exactly the checked successor of the evidence's pre-commit source revision;
- reject a live value plus tombstone, missing state, or duplicate state;
- validate mutable/immutable metadata against the plan.

### Stage 8 — canonical value graph and affine owners

Using the one canonical visitor:

- validate path depth and aggregate shapes;
- enumerate live affine owners;
- reject duplicate affine owner IDs;
- validate each owner execution;
- validate moved/dropped owner occurrence evidence;
- reject affine owner evidence absent from its owning value/tombstone;
- reject extra live values not represented by storage inventory.

### Stage 9 — allocator cursor continuation

For each namespace, compute the maximum persisted *currently represented*
ordinal and compare it with the cursor's authoritative high-water mark.

- `Next(n)` requires `n > max_used`; `last_issued()` is `None` for `Next(1)`
  and otherwise `n - 1`.
- `Exhausted` has the intrinsic high-water mark `u64::MAX` and is valid even
  when the storage occurrence that consumed `u64::MAX` has since exited,
  dropped, or been compacted. The exhausted state itself is persisted and
  covered by the canonical digest; it is never inferred from the live maximum.

Empty/no-owner state requires `Next(1)` for a brand-new execution; preserved
executions may have a later or exhausted cursor because retired identities are
never reusable. A `Next` cursor equal to `max_used`, below it, zero, or guessed
from vector length is rejected.

The affine cursor check is mandatory. It fixes the parent post-restore owner
collision gap.

### Stage 10 — replay and generation consistency

- validate parent replay sequence/effect evidence;
- validate every replay record execution ID;
- validate no replay event mints an execution, slot, transaction, or owner ID;
- validate restart/replay preserves identity cursors;
- validate parent generation pins for affine external partials/handles even
  though no handle is constructible in G1.2.

### Stage 11 — digest and canonical re-encoding

- recompute the existing canonical digest including the new identity section;
- compare exact digest bytes;
- encode the typed candidate and require byte equality with the canonical input
  representation;
- reject tampered/reordered/duplicate evidence even when semantic payload looks
  equivalent.

### Stage 12 — dormant candidate build

- build all lookup indexes and exact-capacity storage;
- bind execution identity to the dormant `BundleSession`;
- ensure zero ownership reservations/prepared transactions;
- ensure the session cannot execute directly;
- produce the core-owned `RuntimeExecutionIdentitySnapshotV2`, the driver-owned
  domain epoch/envelope, identity state, and dormant session.

Only after Stage 12 may the driver request a domain reservation.

## 7. Reservation and activation after validation

### Empty activation

Under the domain lock:

1. require no reservation/active record;
2. require the domain next-execution cursor has not passed the preserved ID;
3. allocate an ephemeral reservation ID;
4. install the reservation;
5. adopt the greater validated next-execution cursor; and
6. return `RuntimeFreshExecution`.

`activate_empty` rechecks the reservation and empty domain, stores active
`(execution, fresh.activation_epoch)` in one mutation, disarms the reservation,
and returns `RuntimeActiveExecution`.

A newly created execution uses epoch 1. A preserved empty activation retains
the serialized epoch; replay does not increment it merely for deterministic
reconstruction.

### Replacement activation

Preparation requires exact active ID/epoch, requires the validated envelope
epoch to equal that active epoch, and installs a replacement reservation. `RuntimeActiveExecution::replace` consumes both owners and, under
one domain lock:

1. rechecks same domain, execution, epoch, reservation, and cursor monotonicity;
2. computes next epoch;
3. swaps the active record to `(execution, next_epoch)`;
4. disarms old active and candidate reservation owners; and
5. returns one new active owner with the candidate session.

The old session is dropped or returned to the caller only after the new active
record is installed. There is no moment with two active records.

Any error returns both the original active owner and fresh candidate, still
usable, with the domain record unchanged.

## 8. Restart

A restart is an in-execution state transition, not a new execution.

It preserves:

- execution ID;
- activation epoch unless restart is implemented as hot replacement;
- all identity cursors;
- already used slot/owner/transaction ordinals;
- parent generation history and replay coordinates.

Restart may allocate new occurrence/local/transaction IDs after it resumes.
It never resets a cursor to 1 or recomputes from currently live values.

## 9. Replay

Replay consumes a validated replay image and preserves the image execution ID,
epoch, and cursors. Replayed external outcomes do not call identity allocators
unless the original deterministic trace contains the corresponding language
event and the current cursor matches the recorded next ID.

A copied replay image may be decoded any number of times, but the shared domain
admits only one reservation/active execution. A second driver attempting the
same image receives a typed reservation/active collision and retains its
candidate input.

No “replay alongside active” API exists.

## 10. Hot replacement

Hot replacement uses `RuntimeActivationMode::Replace` only.

Candidate validation additionally requires:

- immutable program compatibility under existing parent rules;
- exact execution ID;
- no cursor regression;
- no live ownership transaction;
- valid generation pins;
- all replacement state owner IDs unique;
- replacement snapshot digest; and
- expected activation epoch.

Replacement does not rebind the same values to a new execution ID. It cannot
silently regenerate slot/owner IDs from names or HIR.

## 11. Failed restore/replay cleanup

Failure before reservation returns the existing typed restore/replay input
owner and drops only candidate allocations. Failure after reservation returns a
`RuntimeFreshExecution` or input owner whose `Drop` releases that exact
reservation.

Cleanup must not:

- release a different reservation;
- mutate active state;
- advance active revisions/cursors;
- emit provider requests;
- replay evaluation;
- close live host resources;
- delete save bytes; or
- publish a partial candidate.

The test oracle is canonical active snapshot equality before and after every
failure.

## 12. Tamper rejection order

Across restore stages, the earlier stage wins. Within a stage:

1. lower canonical section/slot/path order;
2. lower typed ID;
3. lower field index; and
4. lower byte offset where all typed coordinates are equal.

Representative ordering:

```text
malformed scalar
before wrong program digest
before wrong execution
before duplicate slot
before stale revision
before duplicate affine owner
before stale cursor
before replay mismatch
before final digest mismatch
```

This prevents a checksum-only implementation from hiding typed structural
errors, while still requiring final digest verification.

## 13. Snapshot digest section

The identity section contains:

1. canonical domain identity snapshot;
2. ordered storage-owner identities;
3. declaration/capture IDs;
4. slot revisions and state tags;
5. moved/dropped transaction and owner evidence;
6. record-field IDs; and
7. allocator cursors.

It excludes diagnostic names/source spans. Changing any included identity or
cursor changes the digest even when live value payload bytes are unchanged.

## 14. Current driver migration

The current driver/session APIs that construct or restore an independently
runnable `BundleSession`, clone active session state, or replace in place are
made crate-private and delegated through the execution owner.

`BundleSession` construction may still build a dormant value internally. Its
poll/step/save/replay/hot-reload entry points require
`RuntimeActiveExecution`/driver access. `RuntimeExecutionDomain` remains the
sole active map; no per-session mirror is retained.

## 15. Required tamper cases

At minimum:

- missing/extra/duplicate execution envelope;
- zero/duplicate/wrong-execution local, capture, occurrence, transaction, and
  affine-owner IDs;
- missing/extra/reordered record-field IDs;
- slot state with missing/duplicate revision;
- moved/dropped evidence referring to wrong source/destination/transaction;
- owner evidence wrong path;
- `Next(max_used)`, `Next(max_used-1)`, zero, cursor rollback, and an
  exhausted cursor whose authenticated snapshot bytes/digest disagree;
- affine cursor below a dropped owner;
- domain cursor below execution ID;
- replacement wrong epoch/domain/ID;
- two-driver activation attempt;
- active transaction blocker omission;
- float NaN/signed-zero bit tamper;
- identity section digest tamper; and
- canonical decode followed by noncanonical re-encoding.

Every case fails before activation and leaves active state unchanged.
