# Final contract

```text
SEQUENCE=Lang-01.3.1.2.3.2
STATUS=READY_FOR_IMPLEMENTATION
OPEN_QUESTIONS=0
OPEN_RESULT_CHANGING_DECISIONS=0
IMPLEMENTATION_PERFORMED=NO
PRODUCTION_OVERLAY_INCLUDED=NO
INSPECTED_MAIN=d8fbeaa5757fe5836fba17fca35fa104eeb72a1d
PRESERVED_CLASSIFIER=b76465c128322be2d5e66398bc6c30794ca0276f
```

## 1. Scope, precedence, and retained parents

This package is the mandatory narrow correction before G1.2. It supersedes only
the missing or internally unsafe identity, slot, transaction, path-order,
execution-activation, and cursor-restoration rows of Lang-01.3.1.2.3 and
Lang-01.3.1.2.3.1.

The parents remain authoritative for:

- the two-point `RuntimeValueOwnership` lattice;
- exhaustive value-graph traversal and checked unrestricted duplication;
- the single opaque affine-token direction;
- staged Copy/Move/Drop as the language/runtime transfer model;
- closed runtime payload and plan-constant eligibility;
- typed capture, pattern, and constant plans;
- whole-execution snapshot candidacy;
- retained/render View unrestricted-only admission and handler Move;
- the parent version migration and compile-clean interleave; and
- the rule that Stream handles and affine tokens are not constructible before
  their later publication cut.

The production classifier at `b76465c128322be2d5e66398bc6c30794ca0276f` is preserved byte-for-byte in
G1.1. This correction extends its path projection but does not alter
`RuntimeValue::ownership`, its lattice, or any current classification result.

The correction allocates no AWBC opcode, type tag, section, ABI, or codec
version. The parent target remains ABI 1 / codec 8 for the corrected affine/View
interleave, while current production observed at the inspected main may already
carry unrelated codec evolution. Implementation must rebase numeric version
facts at intake without using this identity correction to downgrade, dual-read,
or reopen an unrelated wire decision.

## 2. Sole-owner architecture

The accepted dependency direction is:

```text
arcweft-lang-hir
  typed LocalId/CaptureId and accepted generation
          |
          v  transient lowering maps only
arcweft-lang-sema / arcweft-runtime-plan
  RuntimeLocalDeclarationId + RuntimeCaptureSlotId plans
          |
          v
arcweft-core
  execution-scoped IDs, RuntimeBinding/RuntimeEnv storage,
  RuntimeOwnedSlotId diagnostic evidence, value paths,
  ownership transactions, snapshots
          |
          v
arcweft-runtime-driver
  one RuntimeExecutionDomain, reservation/activation,
  save/restore/replay/hot replacement
```

`arcweft-core` has no dependency on syntax, HIR, sema, runtime-plan, launch, or
driver. HIR identities are projected downward once; they are never stored in
core values or recovered from names/spans.

All scalar execution/runtime identity wrappers are owned by the existing
`arcweft_core::runtime_id` module. Record identity and value carriers are owned
by the existing `arcweft_core::value` module. Ownership evidence, path,
transaction, and inherent ordering/rendering behavior are owned by the existing
`arcweft_core::value::ownership` module. Runtime-driver activation is owned by a
new `arcweft_runtime_driver::execution` module that wraps the existing session
owner rather than introducing a second executor.

## 3. Execution-instance identity and domain authority

### 3.1 Representation

`ExecutionInstanceId` is a private-field `NonZeroU64` newtype. It is Copy,
stable-orderable, hashable, displayable, and serializable through one strict
manual codec. Zero is never representable. There is no public raw constructor,
`Default`, `From<u64>`, `TryFrom<u64>`, `FromStr`, random UUID constructor,
content-hash constructor, or host-supplied constructor.

The only mint authority is `RuntimeExecutionDomain`, through a private monotonic
`RuntimeIdCursor`. The first ID is 1. Every successful reservation consumes one
ID. IDs are never reused after cancellation, failed construction, deactivation,
restart, replay, restore, or replacement. `u64::MAX` is a valid last ID; after it
is issued the cursor becomes `Exhausted`.

### 3.2 Domain and one-active rule

One runtime host owns one shared `Arc<RuntimeExecutionDomain>`. Every production
driver/session constructor receives that same handle. The domain has:

- the next execution cursor;
- at most one reservation record;
- at most one active execution record; and
- one monotonic activation epoch.

The domain exposes no public independent production constructor. Test-only
construction is crate-private and cannot mint a token, affine owner, or fake
execution through the public API.

At every observable boundary:

```text
reserved_count <= 1
active_count <= 1
reserved.execution != active.execution
```

except replacement reservation, where the reserved and active IDs must be
equal and the reservation carries the exact expected activation epoch.

A `&mut RuntimeDriver` borrow is not the exclusivity proof. Atomic acquisition
of a non-Clone `RuntimeExecutionReservation` from the shared domain is the
proof. Its `Drop` releases an unactivated reservation. Successful activation
consumes it.

### 3.3 Fresh, restore, replay, and replacement

`RuntimeFreshExecution` is a dormant, non-Clone candidate containing:

- the exact execution ID;
- a complete core-owned `RuntimeExecutionIdentityState` with every allocator
  cursor;
- the driver-owned activation epoch;
- source kind (`New`, `Restore`, or `Replay`);
- activation mode (`Empty` or `Replace`);
- the fully built but not active `BundleSession`; and
- the live domain reservation.

A new execution obtains a newly allocated ID. Restore and replay preserve the
serialized ID and acquire a reservation for that exact ID; they do not mint a
replacement identity. Restart preserves the active ID and all cursors. Replay
reconstructs deterministic external outcomes but cannot run a second active
copy.

Empty activation requires no active execution. Replacement requires:

- one current active execution;
- candidate ID equal to active ID;
- expected epoch equal to active epoch;
- validated successor cursors that do not regress; and
- one atomic domain/session swap.

Any failure returns the entire fresh candidate to the caller or releases its
reservation according to the exact error owner in `RUST_OWNERS_AND_APIS.md`.
The active execution remains byte-identical.

## 4. Record-field identity

`RuntimeRecordFieldId` is a private-field `NonZeroU32` newtype. Its value is the
one-based accepted storage ordinal:

```text
encoded_value = accepted_zero_based_ordinal + 1
```

It is not a name hash and is never derived from source spans.

For anonymous records, the accepted order is authored field order after
duplicate-name rejection. For nominal records, the accepted order is the
validated nominal layout order already used by `RuntimeNominalRecordValue`.
Authored initializer order may differ; lowering projects it into accepted layout
order before runtime construction.

Every accepted record has contiguous IDs `1..=field_count`. Duplicate field
names fail before any ID is published. Unknown, missing, duplicated,
non-contiguous, zero, or out-of-range IDs fail before value-graph traversal or
activation.

`RuntimeFieldValue` gains the ID beside the diagnostic name. `RecordSeqField`
gains the same ID beside its name and column. `RuntimeNominalRecordValue`
continues to store one schema-ordered value vector and exposes
`field_id(ordinal)` through its own inherent `impl`; it does not add a parallel
ID table.

Canonical traversal and comparison use the field ID. Names are diagnostics only.

## 5. Runtime locals, captures, revisions, and shadowing

### 5.1 Static plan identities

`RuntimeLocalDeclarationId(NonZeroU32)` identifies one accepted runtime-plan
local declaration. It is assigned in deterministic declaration-publication
order within one executable definition. It is static, plan-owned, and may recur
in different dynamic activations.

`RuntimeCaptureSlotId(NonZeroU32)` identifies one slot in an accepted closure
capture plan. It is assigned in the capture plan's canonical order. It is not
the HIR `CaptureId`.

During lowering, runtime-plan owns transient maps:

```text
(LocalGeneration, LocalId) -> RuntimeLocalDeclarationId
CaptureId                  -> RuntimeCaptureSlotId
```

The maps are checked for one-to-one coverage and discarded after the plan is
built. No HIR identity crosses into `arcweft-core`.

### 5.2 Dynamic identities

`RuntimeLocalSlotId(NonZeroU64)` identifies one dynamic environment-local
storage occurrence. It is allocated from the execution-wide local-slot cursor.
It is never reused, including after scope exit or spare-scope vector reuse.
Nested shadowing always allocates a new slot. Name lookup selects the newest
live binding but returns its typed slot.

Other dynamic occurrence wrappers use one execution-wide occurrence cursor:
scope, closure, fiber, frame, mailbox, child, transfer, and cleanup-scope
instances. Their scalar values are never reused. Owner-local lane, packet,
capture, and cleanup-slot ordinals are typed `NonZeroU32` values.

### 5.3 Slot state and revision

Every mutable storage owner carries one `RuntimeSlotRevision(NonZeroU64)`.
Revision 1 is the initial state. A committed Copy destination, Move source,
Move destination, Drop source, or mutation increments the affected slot exactly
once. Reads do not increment. A no-op transaction has no participants and is
rejected. Revision exhaustion is a prepare-time error; no value is taken.

Every slot is exactly one of:

- `Vacant`;
- `Live(RuntimeValue)`;
- `Moved(RuntimeMovedValueEvidence)`; or
- `Dropped(RuntimeDroppedValueEvidence)`.

Moved and dropped tombstones retain the last committed revision and evidence.
Use-after-move and use-after-drop diagnostics therefore do not depend on names
or source reconstruction.

Scope exit converts each still-live local to `Dropped` in canonical slot order
through the ownership transaction. Only after committed cleanup may the backing
vector capacity enter the existing spare-scope pool. Reused capacity never
reuses identity.

Suspension, save, and restore preserve slot IDs, declaration IDs, revisions,
state, owner evidence, and every cursor. Capacity and lookup indexes are
rebuilt.

## 6. Complete diagnostic owner union

`RuntimeOwnedSlotId` is evidence, not a storage key. It is a closed enum covering
all required domains:

1. environment local;
2. closure capture;
3. AWBC register;
4. AWBC frame local;
5. mailbox lane;
6. child packet;
7. transfer packet; and
8. cleanup slot.

Its canonical variant tags are exactly 0 through 7 in that order. Ordering is
tag first and then field order. Every field contains `ExecutionInstanceId`, so
cross-execution evidence sorts by execution before domain-specific occurrence
identity.

Storage remains in the existing environment, closure, AWBC frame/fiber,
mailbox, scheduler/child, transfer, and cleanup owners. No
`BTreeMap<RuntimeOwnedSlotId, RuntimeValue>` and no side table is permitted.

The owning enum receives inherent `canonical_tag`, `execution`, and
`render_canonical` methods. No extension trait or endpoint helper owns these
behaviors.

## 7. Affine-owner and transaction identity

`RuntimeAffineOwnerId` is the complete pair:

```text
ExecutionInstanceId + NonZeroU64 owner ordinal
```

The affine-owner cursor is persisted and restored. G1.2 makes the identity and
cursor representable but does not expose an affine-token or Stream-handle
constructor. The first later affine mint must use this cursor.

`RuntimeOwnershipTransactionId` is likewise
`ExecutionInstanceId + NonZeroU64 transaction ordinal`. Every transaction,
including one that fails preparation, consumes one ordinal. Ordinals are never
reused. This makes diagnostics and replay stable without treating the
transaction ID as a storage owner.

## 8. Transaction model and failure atomicity

A transaction owns an ordered `RuntimeTransferPlan` of Copy, Move, and Drop
steps. A caller cannot prepare one value and commit another. Preparation
observes exact typed storage slots, installs integrated per-slot reservations,
and owns every allocation needed by commit.

- Copy preparation performs checked graph traversal and creates the complete
  unrestricted duplicate before reserving commit.
- Move preparation leaves the source value in its exact slot.
- Drop preparation leaves the source value in its exact slot and never accepts
  an unrelated `RuntimeValue`.
- Destination type/layout and vacancy are checked before any source take.
- Conflicting slot participation and duplicate affine-owner occurrence are
  checked over the whole transaction. Repeated source participation is accepted
  only for compatible Copy steps sharing one source reservation; every Copy
  destination still owns an independently staged duplicate.
- All next revisions, evidence objects, vectors, boxes, and staged bytes are
  allocated before commit.

Every prepare failure returns the untouched transaction and plan. All installed
reservations are cleared. Every commit mismatch returns one
`RuntimeAbortedOwnershipTransaction` that owns the prepared transaction and
records the exact mismatch; all reservations are cleared and values are
unchanged from commit entry. An aborted transaction cannot be committed again.

After `RuntimeCommitPermit` has been built, commit is infallible. From that point
through the first source take and final installation there is:

- no `Result` or `Option` failure branch;
- no allocation or collection growth;
- no checked arithmetic;
- no dynamic type/layout/policy check;
- no lookup by name or `RuntimeOwnedSlotId`;
- no user code, host callback, scheduler poll, or panic-capable formatting; and
- no owner-uniqueness scan.

The permit contains exact private storage handles, precomputed revisions,
prebuilt tombstones/evidence, staged duplicates, and one canonical list of
unique participant reservation handles. Commit consumes the permit, performs
all step mutations, clears reservations only after the final mutation, and
returns `RuntimeCommittedOwnershipTransaction` directly.

## 9. Error precedence and owner return

Preparation evaluates all observable failures without mutating values, then
selects the minimum canonical error by:

1. execution/transaction/participant identity;
2. stale revision or existing reservation;
3. source liveness;
4. destination vacancy;
5. type/layout/path/record validity;
6. duplicate affine owner;
7. affine Copy;
8. revision or identity-cursor exhaustion;
9. configured budget; and
10. allocation failure.

Within one rank, compare `RuntimeOwnedSlotId`, then `RuntimeValuePath`, then
`RuntimeAffineOwnerId`, then plan step index.

Commit mismatch precedence is:

1. wrong execution;
2. missing storage occurrence;
3. storage identity mismatch;
4. reservation mismatch;
5. revision mismatch;
6. occupancy mismatch;
7. type mismatch; and
8. affine-owner mismatch.

These ranks are encoded by inherent methods on the owning error enums.

## 10. Canonical value path

`RuntimeValuePath` is a boxed sequence of at most 64
`RuntimeValuePathSegment`s. Segment tags and order are:

| Tag | Segment |
|---:|---|
| 0 | tuple element |
| 1 | sequence element |
| 2 | tuple column |
| 3 | anonymous-record field |
| 4 | record column |
| 5 | nominal-record field |
| 6 | function capture |
| 7 | variant payload |
| 8 | iterator remainder |
| 9 | iterator witness state |

Comparison is lexicographic over `(tag, payload)` and a prefix sorts before its
descendant. Record segments use `RuntimeRecordFieldId`; capture segments use
`RuntimeCaptureSlotId`. Iterator remainder payload is the absolute original
item index, not a suffix-relative index.

The traversal exactly mirrors the shipped classifier:

- tuple and ordinary sequence vector order;
- dense scalar sequence has no child;
- tuple columns in stored column order, recursively;
- record columns in accepted field-ID order, recursively;
- anonymous records in accepted authored field-ID order;
- nominal records in accepted nominal layout order;
- function captures in capture-slot order;
- variants then payload;
- iterator `Values` only from current index to end, with absolute index;
- iterator `Witness` state;
- ranges have no child.

The same traversal produces ownership classification, affine-owner validation,
path diagnostics, snapshot candidate traversal, and deterministic first error.
A second recursive walk is prohibited.

## 11. Limits

The hard maxima are:

```text
MAX_OWNERSHIP_TRANSACTION_PARTICIPANTS = 4_096
MAX_OWNERSHIP_TRANSACTION_STEPS        = 4_096
MAX_OWNERSHIP_VALUE_NODES              = 1_048_576
MAX_RUNTIME_VALUE_PATH_SEGMENTS        = 64
MAX_OWNERSHIP_AFFINE_OWNERS            = 262_144
MAX_OWNERSHIP_STAGED_BYTES             = 67_108_864
```

Configured limits may tighten but never exceed these maxima. All arithmetic is
checked. Exact-limit inputs succeed; one-over inputs fail before reservation or
value mutation. Traversal work counts every visited runtime value node once and
every emitted path segment once.

## 12. Snapshot, codec, digest, and restore

The existing save-schema-2 candidate is extended directly. There is no sidecar,
legacy reader, serde alias, or schema fork.

Persisted identity includes:

- domain next-execution cursor;
- active execution ID and activation epoch;
- next occurrence, local-slot, ownership-transaction, and affine-owner cursors;
- every dynamic occurrence and slot ID;
- static local declaration and capture-slot IDs;
- every live/moved/dropped state and revision;
- affine-owner evidence;
- record-field IDs;
- ownership evidence needed for diagnostics; and
- existing parent generation pins and execution snapshot fields.

Rebuilt, non-semantic state includes:

- name-to-slot and occurrence lookup indexes;
- vector capacity, spare scopes, and caches;
- live transaction reservations and prepared permits;
- runtime-driver mutexes/Arcs;
- transient HIR-to-plan maps;
- diagnostic display indexes; and
- derived activation bookkeeping other than the persisted epoch.

Save is blocked while any ownership transaction is reserved or prepared. The
existing save-blocker enum gains an inherent variant
`OwnershipTransactionActive { count }`; no helper enum is introduced.

Floating snapshots use canonical bit carriers (`u32` for f32, `u64` for f64).
`RuntimeValueSnapshotV2` implements `PartialEq` but not `Eq`. NaN payload bits
and signed zero are preserved. This resolves the parent declaration conflict
without altering live-value equality.

Restore performs the 12 fixed stages in
`SNAPSHOT_ACTIVATION_AND_RESTORE.md`. All bytes, duplicates, IDs, cursors,
paths, owner evidence, and digests are validated before domain reservation and
before active mutation. Missing, extra, duplicate, zero, stale, regressing, or
wrong-execution evidence fails closed. A cursor in `Next(n)` must be strictly
greater than every currently represented ordinal in its namespace. For execution-local occurrence, local-slot,
transaction, and affine-owner cursors, persisted `Exhausted` is itself the
authoritative `u64::MAX` high-water state and remains valid after the occurrence
that consumed the maximum has retired. The domain execution cursor has the
stronger envelope invariant `Exhausted => execution == u64::MAX`, because no
new execution can be minted while the sole active execution exists. Every
cursor is covered by the identity digest and is never reconstructed from the
live maximum.

The canonical digest gains one domain-separated identity section:
`arcweft.runtime-ownership-identity.v1`. It is encoded after existing semantic
program identity and before mutable execution payload. Source names and spans
remain excluded.

## 13. Codec summary

- `u64` identity values in canonical JSON are strict decimal strings.
- `u32` ordinals/IDs in canonical JSON are JSON integers.
- Binary scalar identities use fixed-width little-endian integers.
- Enums use one unsigned byte tag followed by fields in declaration order.
- Vectors encode a checked `u32` count followed by elements.
- Decoders reject unknown fields/tags, duplicates, leading `+`, leading zero
  except `"0"` where zero is semantically allowed, whitespace, overflow,
  numeric JSON tokens where a decimal string is required, BOM, invalid UTF-8,
  and trailing bytes.
- Identity newtypes whose zero is invalid reject zero before allocation.
- Encode/decode/re-encode is byte-identical across native, Web, headless, and
  Agent consumers.

Worked bytes are fixed in `CODEC_GOLDENS.md` and
`CODEC_GOLDENS.json`.

## 14. Direct deletions

The implementation directly removes or replaces:

- name-only runtime-local storage authority;
- `RuntimeEnv::get_cloned`, `bindings_snapshot`, and ref-clone rebinding paths
  as successful ownership paths;
- closure capture of the complete visible environment;
- raw record ordinal/name-only path construction;
- any prepared Drop API accepting an arbitrary value;
- any prepared Move commit API accepting a separately supplied value;
- per-driver-only activation claims and direct active-session construction;
- restore-time affine-owner cursor guessing;
- live `RuntimeBinding` Serde as save authority;
- `Eq` on snapshots containing floating payloads;
- placeholder/reduced `RuntimeOwnedSlotId` variants;
- sidecar reservation/owner maps; and
- all raw public ID constructors and fake execution/token test constructors.

Deletion occurs in the same compile-clean cut as the replacement consumer.

## 15. Required invariants

An implementation is conforming only if all of the following hold:

1. One active execution exists per shared runtime domain.
2. An execution ID is minted or restored by exactly one domain reservation.
3. Every runtime local slot is globally unique within its execution and never
   reused.
4. Every storage mutation has a checked revision transition.
5. Every affine owner occurs at most once in the active value graph.
6. `RuntimeOwnedSlotId` is diagnostic evidence only.
7. Copy never duplicates an affine value.
8. Move and Drop cannot commit a different value than the prepared source slot.
9. No fallible branch exists after commit permit construction.
10. Path comparison and first-error choice are deterministic and platform
    independent.
11. All allocator cursors continue strictly after restore.
12. Failed prepare, commit mismatch, restore, replay, and replacement leave the
    previous active state unchanged.
13. No core-to-HIR/driver reverse dependency or source-text identity recovery is
    introduced.
14. No compatibility reader, dual writer, migration shim, endpoint DTO, source
    gate, extension-trait authority, or parallel value/environment model exists.
15. G1.3/G1.4, View expansion, AWBC wire publication, and Stream publication do
    not begin in this correction.

## 16. Completion result

All ten required decision groups are closed by exact owners, representations,
allocation rules, failure owners, ordering, codecs, persistence, implementation
cuts, and tests. `OPEN_QUESTIONS.md` is exactly `none`.
