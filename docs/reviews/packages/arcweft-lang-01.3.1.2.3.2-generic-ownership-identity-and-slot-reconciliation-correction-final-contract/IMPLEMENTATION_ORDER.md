# Corrected compile-clean implementation order

## 1. Global rule

Every cut below is independently compile-clean and reviewable. No cut may
publish a fake constructor, placeholder enum variant, dual reader, source gate,
or parallel environment/value model merely to keep compilation green.

Repository intake starts from the then-current `main`; this package was designed
against `d8fbeaa5757fe5836fba17fca35fa104eeb72a1d`. The implemented classifier cut
`b76465c128322be2d5e66398bc6c30794ca0276f` is preserved as the G1.1 baseline. Rebase drift is
resolved by owner/type evidence, never by changing the decisions in this
contract silently.

G1.3/G1.4, View expansion, AWBC wire publication, and Stream handle/token
publication remain blocked until all G1.2 cuts pass.

## 2. G1.1 — preserved classifier baseline

### Work

- retain `RuntimeValueOwnership` and current exhaustive classification;
- add no constructible affine leaf;
- add no execution/slot/transaction serialization;
- record the exact baseline commit in implementation evidence.

### Exit gate

```text
cargo test -p arcweft-core value::ownership
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Existing tests must remain green. No changed classification golden is accepted.

### First constructible/serialized state

- no new identity constructible;
- no new identity serialized.

## 3. G1.2-A — lower identity, record ID, owner enum, and path owner

### Owners

- `arcweft_core::runtime_id`;
- `arcweft_core::value`;
- `arcweft_core::value::ownership`;
- existing record/nominal/sequence/iterator owners.

### Work

1. Add private-field scalar wrappers and manual codec modules.
2. Add `RuntimeRecordFieldId` to anonymous/column record carriers.
3. Replace unchecked nominal-record construction with accepted-layout
   construction and inherent `field_id`.
4. Add the complete `RuntimeOwnedSlotId` and inherent ordering/rendering.
5. Extract the one internal path-aware visitor while preserving classifier
   results.
6. Add exact path depth/node accounting and canonical record/iterator paths.
7. Add compile-fail tests for raw constructors and reduced owner variants.

### Exit gate

- focused identity/record/path tests;
- golden scalar/owner/path bytes;
- classifier parity against G1.1;
- core check/clippy;
- no reverse dependency.

### First constructible/serialized state

| Identity | First constructible | First serialized |
|---|---|---|
| `RuntimeRecordFieldId` | G1.2-A record admission | G1.2-A record snapshot/golden |
| static/owner-local wrappers | G1.2-A crate-private validators | not product-published yet |
| `RuntimeOwnedSlotId` | G1.2-A typed evidence constructors | G1.2-A diagnostic/golden only |
| `RuntimeValuePath` | G1.2-A visitor | G1.2-A diagnostic/golden only |
| `ExecutionInstanceId` | representation only; no mint | not yet |

## 4. G1.2-B — HIR projection, local slots, captures, and storage revisions

### Owners

- `arcweft-lang-hir` retained IDs;
- `arcweft-lang-sema` checked resolution;
- `arcweft-runtime-plan` transient projection;
- existing `RuntimeBinding`, `RuntimeEnv`, closure, pattern, and engine owners.

### Work

1. Produce deterministic `RuntimeLocalDeclarationId` and
   `RuntimeCaptureSlotId` plans from typed HIR facts.
2. Add projection coverage/uniqueness errors; discard transient HIR maps.
3. Replace `RuntimeBinding` with typed slot/declaration/mutability/cell.
4. Allocate execution-local dynamic slot/scope/closure occurrence IDs through
   a test-only internal identity state seeded with an injected execution ID.
5. Preserve names only for lookup/diagnostics.
6. Replace clone-based capture/partial-apply/binding paths under the parent
   Copy/Move rules.
7. Add revisions and typed Vacant/Live/Moved/Dropped storage.
8. Replace scope-exit cleanup with `RuntimeScopeExitView` -> canonical Drop
   transaction -> `recycle_committed_scope`; transaction implementation lands
   next, and capacity recycling occurs only after committed Drops.
9. Delete successful name-only/ref-clone APIs in the same cut.

### Exit gate

- HIR→plan mapping tests;
- local shadowing/reuse/revision tests;
- capture order and exact-set tests;
- structured engine compile/tests;
- dependency metadata proves core has no HIR dependency.

### First constructible/serialized state

| Identity | First constructible | First serialized |
|---|---|---|
| declaration/capture IDs | G1.2-B runtime-plan freeze | G1.2-D |
| scope/closure/local slot IDs | G1.2-B internal executor allocator | G1.2-D |
| slot revision | G1.2-B binding/cell construction | G1.2-D |
| execution ID | crate-private injected test fixture only, not public/domain mint | G1.2-D |

The injected fixture constructor is test-module private, accepts only a typed ID
constructed by an even narrower test helper, and is not exported from the
crate. It cannot create a token/handle.

## 5. G1.2-C — ownership transaction, prepared owners, evidence, and commit permit

### Owners

- `arcweft_core::value::ownership`;
- one sealed executor storage protocol;
- existing environment/closure/AWBC/mailbox/child/transfer/cleanup storage.

### Work

1. Add transaction/affine-owner allocators to execution identity state.
2. Add exact limits, plan, endpoints, observations, errors, and owner return.
3. Implement one canonical preflight/value traversal.
4. Implement checked Copy staging.
5. Implement slot-integrated reservations.
6. Implement prepared Copy/Move/Drop; no arbitrary commit value.
7. Implement commit revalidation, aborted owner, and permit.
8. Make `commit_permit` infallible after permit construction.
9. Connect structured storage first, then AWBC/mailbox/child/cleanup through the
   same sealed protocol.
10. Delete independent transfer algorithms and side reservations.

### Exit gate

- exact-limit/one-over tests for all six limits;
- stale/source/destination/type/duplicate-owner/affine-copy/allocation tests;
- byte-identical state on every failure;
- source preservation until permit;
- no semantic failure branch after first take;
- structured/AWBC/compiled-region parity tests where substrate exists;
- core check/clippy.

### First constructible/serialized state

| Identity/evidence | First constructible | First serialized |
|---|---|---|
| transaction ID | G1.2-C begin transaction | G1.2-D |
| moved/dropped evidence | G1.2-C commit | G1.2-D |
| affine owner ID | representable/restorable; allocator crate-private | G1.2-D cursor/evidence only |
| affine token/Stream handle | **not constructible** | **not serialized as live handle** |

## 6. G1.2-D — snapshot, strict codec, digest, and golden bytes

### Owners

- parent closed snapshot carrier;
- current save-schema-2 codec owner;
- existing canonical digest owner;
- bundle/runtime-plan serializer where static IDs are carried.

### Work

1. Add bitwise f32/f64 snapshot wrappers; remove `Eq`/`Hash` from value snapshot.
2. Add identity/domain snapshots and strict manual codecs.
3. Serialize all local/capture/occurrence/slot/revision/evidence IDs.
4. Persist all four execution-local cursors and domain next-execution cursor.
5. Add the domain-separated digest section through the existing owner.
6. Block save on active ownership transactions.
7. Implement stages 1–11 of restore validation without activation.
8. Delete live binding/value Serde as save authority and any cursor inference.
9. Land canonical JSON/binary goldens and tamper corpus.

### Exit gate

- encode/decode/re-encode byte parity;
- digest change/isolation tests;
- float bit tests;
- all missing/extra/duplicate/cursor tamper tests;
- old/noncanonical identity forms rejected;
- no dual reader/writer;
- bundle/save focused check/clippy.

### First constructible/serialized state

All G1.2 identities become serialized here. None becomes publicly raw
constructible.

## 7. G1.2-E — shared execution domain and activation

### Owners

- new `arcweft_runtime_driver::execution`;
- existing runtime host and driver/session construction/persistence/replay
  owners.

### Work

1. Create one host-shared `RuntimeExecutionDomain`.
2. Make independent runnable session construction/private activation paths
   unreachable.
3. Implement new execution monotonic mint and reservation.
4. Implement validated empty restore/replay reservation.
5. Implement exact `RuntimeFreshExecution`.
6. Implement empty activation and non-Clone active owner.
7. Implement restart preserving identity/cursors.
8. Implement same-ID/epoch replacement returning both owners on failure.
9. Connect stages 12/reservation/activation to existing restore/replay.
10. Delete per-driver-only activation maps/claims and install-alongside APIs.

### Exit gate

- new ID creation/collision/exhaustion;
- two-driver/concurrent reservation and activation tests;
- empty/replace/restart/replay semantics;
- failed activation/replacement exact owner return;
- first post-restore transaction/local/owner allocation;
- driver/save/replay/hot replacement tests;
- native/Web/Agent façade parity where applicable.

### First constructible/serialized state

| Identity | First constructible | First serialized |
|---|---|---|
| production `ExecutionInstanceId` | G1.2-E domain reservation | already G1.2-D format |
| `RuntimeFreshExecution` | G1.2-E only | never serialized |
| reservation ID | G1.2-E only | never serialized |
| active owner | G1.2-E only | snapshot projects identity, not owner object |

## 8. G1.2-F — direct deletion, full matrix, and merge gate

### Work

- remove temporary test-only fixture construction outside test modules;
- delete all obsolete clone/name-only/raw-active/dual-save paths listed in the
  inventory;
- run public API/compile-fail/dependency tests;
- run every G1.2 matrix row;
- re-run all retained parent matrices applicable before handle publication;
- run formatting, workspace check, strict Clippy, tests, metadata, and structure
  audit.

### Required commands

```text
cargo fmt --all --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
cargo metadata --format-version 1 --no-deps
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

Run repository-owned Tier-2/native/Web/headless/Agent commands named by the
latest `AGENTS.md` and affected crate docs. Record exact commit, toolchain,
target, environment, exit status, and real test counts.

### Merge gate

G1.2 is merge-ready only when:

- every matrix row has implementation evidence;
- all deletes are complete;
- no affine token or Stream handle constructor exists;
- one shared execution domain and one transaction owner remain;
- no current classification result changed;
- all gates pass at one commit; and
- no unreviewed result-changing drift from this contract exists.

## 9. G1.3 and G1.4

Do not start in this correction.

After G1.2-F is accepted, the parent interleave may proceed:

- G1.3 may add later affine leaf/token/handle construction only through
  `RuntimeAffineOwnerAllocator` and the existing visitor/transaction owner.
- G1.4 may publish later wire/save/View/Stream integration only through the
  final identities and ownership-aware slots.

Any later wire allocation must use the then-current owning enum/constants, not a
number reserved by this package.
