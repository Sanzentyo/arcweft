# Ordered compile-clean implementation plan

This is the sole accepted interleave. Each numbered publication boundary must compile and pass its focused tests. Internal temporary derives may survive only where explicitly stated and only while no affine leaf/Stream handle is constructible. No temporary compatibility API, dual reader, alternate value enum, or source gate is introduced.

## 0. Intake/re-pin gate

Before editing production:

1. fetch the then-current `main` into a clean checkout or isolated Git worktree and record the full Git SHA plus dirty state;
2. read the complete root and every applicable nested `AGENTS.md` plus the Rust Skill;
3. confirm the latest commit and whether baseline paths moved since `177ba1e61e43fb2da2149869ce35e165d1e93b66`;
4. confirm accepted P3 shared-sema external binding evidence is present/green and P4+C1 has not published a constructible handle on an ownerless value graph;
5. inventory the request-recorded 322 core errors plus all downstream `Clone` consumers using compiler diagnostics and typed API search/manual ownership review, not a source-text acceptance gate;
6. record exact pre-change workspace/metadata/structure state and current validation commands.

A path rename/split updates the inventory. A genuinely contradictory accepted production behavior may reopen only the exact affected result with evidence; it does not justify local ad hoc semantics.

## 1. G1 — generic owner types and non-behavioral APIs

### G1.1 classification/path/error owner

Add `arcweft-core::value::ownership` and inherent `RuntimeValue::ownership`, canonical paths, errors, boundary eligibility traversal scaffolding, and private cache checks. Add no Stream token constructor and change no source/runtime behavior.

Compile gate:

- core library/check/tests for existing unrestricted values;
- ownership traversal determinism and nested join tests;
- no new dependency/I/O edge.

### G1.2 checked duplication and slots

Add `try_duplicate_unrestricted`, `RuntimeValueSlot`, prepared Copy/Move/Drop transaction primitives, owned error wrappers, and typed borrowed equality. Route new tests through these APIs. The old `Clone` derives may temporarily remain solely to keep unmigrated callers compiling, but production behavior changed in this cut must use checked APIs. Marking/deprecating/aliasing is unnecessary and prohibited; the derive is deleted at G3 exit.

No affine token can yet be minted, so the temporary `Clone` surface cannot duplicate a live affine leaf. This is a compile-clean implementation interval, not an accepted final API.

Compile gate:

- unrestricted copy/slot/drop model tests;
- owned-error source preservation;
- no public token/handle constructor.

### G1.3 closed payload, typed capture, pattern binding, and constant schema

Directly replace the existing `RuntimePayload(pub RuntimeValue)` owner with the exact closed non-runnable enum and migrate every current payload constructor/codec/test in the same compile-clean cut; do not add a parallel payload type or dual reader. Extend the existing HIR/sema/compiler/runtime-plan capture projection with final `RuntimeCaptureMode`, exact source/destination/type/mutable-slot facts. Add `RuntimePatternBindingPlan` directly to the existing pattern/decision owner, plus the checked constant table/builder/ID and the new expression/pattern ID variants while callers migrate. No live affine leaf or Stream handle exists yet.

Compile gate:

- closed payload round-trip, ineligible runtime-kind rejection, budget, and no-`From<RuntimeValue>` API tests;
- accepted capture identity/order and directly attached pattern binding-plan tests;
- compiler boundary remains sema -> core/runtime-plan; runtime-plan gains no sema normal dependency;
- constant eligibility/digest/instantiate/borrowed-pattern tests.

### G1.4 snapshot evidence schema

Add schema-2 generic owner evidence/snapshot eligibility types and private restore typestate/validation skeleton, but do not activate a token or replace current save behavior yet. Snapshot DTO Clone is allowed; live owners/candidates are not.

Compile gate:

- strict codec/canonical/tamper unit tests over synthetic evidence rows;
- no public activation/mint API.

**G1 exit:** all final types/APIs are frozen; no Stream handle is constructible; current runtime may still contain temporary old Clone callers.

## 2. G2 — RuntimePlan and structured runtime migration

### G2.1 eliminate live values from plans/caches

Migrate every `RuntimeExpr::Value(RuntimeValue)` and `RuntimePattern::Literal(RuntimeValue)` producer/consumer, direct RuntimePlan Clone/Serde path, AOT/JIT plan cache, and fixture to `RuntimeConstantId` plus one non-Clone `RuntimePlan` shared as `Arc<RuntimePlan>`. Expression evaluation instantiates fresh executable values from `RuntimePlanConstant(RuntimePayload)` closed data; pattern selection borrows the same entry and runs its directly attached binding plan.

In the same compiling cut, normalize the **existing** `RuntimeFlow` owner into its block arena; replace nested `Vec<FlowOp>` bodies with `RuntimeFlowBlockId`; add adjacent binding plans; delete `FlowOp::Bind`, `LoopNext`, `WhileNext`, `WhileLetNext`, and `ForNext`; delete `FlowFiber.pending_ops: VecDeque<FlowOp>`; and move loop/while/while-let/for continuation state to the original `FlowControlStackEntryKind`. `Engine::new` takes `Arc<RuntimePlan>`. No runtime-only predecessor tag or recursive-body reader remains. Then delete both live literal variants and all old plan constructors/matches/codecs/tests atomically. The bundle codec becomes the sole plan reader/writer; no dual successful representation remains.

Gate:

- runtime-plan/compiler/AOT/JIT focused suites;
- bundle constant canonical round trip where applicable;
- compile-fail/no public old expression or pattern literal variant;
- direct RuntimePlan Clone/Serde is unreachable; caches/engines share one `Arc<RuntimePlan>` reconstructed by the bundle artifact;
- no plan/pending queue contains `RuntimeBinding`, `RuntimeIterator`, cloned bodies, or a runtime-only `*Next` op.

### G2.2 environment/local/pattern/aggregate operations

Migrate environment lookup/binding, `let`/parameters/pattern/rest, tuple/record/sequence/variant construction/projection, call argument/return, assignment, equality, cleanup, and sequence push/get/slice/repeat to exact borrow/copy/move/drop semantics. Add sequence behavior directly to the existing `RuntimeSeq`/`RuntimeIterator` owners and pattern behavior directly to `RuntimePattern`; introduce no `RuntimeSequenceValue`, pattern side table, or helper authority. Use owned errors and preflight/stage/commit. Delete successful clone helpers as their final callers disappear.

Gate:

- direct operation matrix, exact evaluation/transfer order;
- before/after state-digest equality for every pre-commit failure;
- repeat/index/slice exact and one-over cases.

### G2.3 closure capture/ordinary partial application

Replace executable `bindings_snapshot()`/whole-environment capture with the typed capture plan. Migrate nested closures and ordinary function partial application so existing captures/arguments are moved and reuse is explicit checked Copy. Delete the clone-based `partially_apply` path and executable ambient snapshot fallback.

Gate:

- exact free-set/first-use/nearest-shadow tests;
- unrestricted Copy vs affine-ready Move facts using synthetic no-token ownership fixtures where possible;
- nested/reassign/failure non-mutation/suspension tests;
- closure capture count proves unrelated locals absent.

### G2.4 iterator and structured suspension/cross-fiber

Replace `RuntimeIterator::Values` index+clone with consuming storage. Migrate structured application/suspension frames, flow/thread child capture, mailboxes, scheduler packets, and cleanup to non-clone owned slots/packets. Delete parent environment clone and facade synchronization paths in structured execution.

Gate:

- iterator exactly-once/drop remaining tests;
- child spawn atomicity and no ambient capture;
- suspension resumes from owned cursor without reevaluation;
- structured engine full focused suite.

**G2 exit:** structured/runtime-plan execution no longer requires value Clone. No Stream token/handle exists. AWBC/compiled/save legacy callers may still keep temporary derives compiling.

## 3. G3 — AWBC, fiber, compiled exchange, snapshot owner, final Clone removal

### G3.1 AWBC ownership dataflow (internal, before codec publication)

Add final register liveness/ownership/cleanup facts, consuming Move/Drop semantics, explicit copy IR/instruction model internally, aggregate/call/capture/return operand-use metadata, branch/loop joins, safe points, and child transfer. Keep external codec version unchanged until P6+C4; do not publish provisional `0x2a` bytes.

Gate:

- verifier unit/CFG matrix;
- VM copy/move/drop/constructor/call/capture atomicity;
- no Stream-specific register model.

### G3.2 fiber/product-step/compiled-region owner migration

Make register files, frames, fibers, execution trees, transfer/exchange packets, and product-step facade state non-clone in behavior. Replace clone/rebuild/synchronization with one owned state plus validated exchange. Accelerator/JIT shares immutable plans only.

Gate:

- interpreter/compiled parity across result/error/owners/cleanup/state digest;
- stale/malformed exchange leaves core unchanged;
- child transfer/cancel/unwind tests.

### G3.3 whole-execution snapshot/restore generic owner

Switch driver/core snapshot traversal to strict dormant `RuntimeValueSnapshotV2`/owner evidence, snapshot guard, restore candidate, exact validation order, isolated activation plan, and empty/replace atomic swap. At this point no production Stream handle exists, so validate generic snapshot structure with unrestricted values and closed synthetic evidence tests; the parent Stream integration lands later.

Gate:

- snapshot image copy vs runtime copy distinction;
- old runnable clone candidate path deleted;
- failure at each validation/allocation/recheck point leaves old state identical;
- no alongside install API.

### G3.4 atomic executable Clone/trait deletion

In one compile-clean change, remove `Clone`/`Copy` (and generic Serde/blanket equality authority where listed) from `RuntimeValue`, bindings, function/closure/partial shape, `RuntimeSeq` and its affine-capable aggregate/column owners, `RuntimeIterator`, `RuntimePattern` executable carriers, env/register/frame/fiber/execution, transactions/candidates/live table shapes. Fix the last compiler errors only by routing to the already selected APIs or moving owners—never by adding an ad hoc clone/helper/Arc/token exception.

Delete remaining:

- the `bindings_snapshot()` method and every full-environment clone caller;
- clone-based partial/call/pattern/sequence/iterator/AWBC/fiber/facade/snapshot paths;
- the old `RuntimePayload(pub RuntimeValue)` wrapper and raw generic RuntimeValue payload/codec paths (already migrated in G1.3);
- both old live literal carriers, all five runtime-only FlowOp variants, pending cloned-op queues, direct RuntimePlan Clone/Serde, and any live cache value/iterator/binding;
- fake affine fixture constructors if any.

Gate:

```text
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
focused generic ownership + structured + AWBC + compiled + snapshot suites
public API/trybuild compile-fail suite
cargo metadata --no-deps --format-version 1
affected structure audit
```

**Critical boundary:** unconditional executable Clone disappears at G3.4 exit. `StreamHandle` is still not constructible. Main/release may publish this owner foundation only when all G3 gates are green.

## 4. P4 + C1 — first constructible Stream handle/partial

Now apply the accepted .1/.2/.2.1 P4+C1 cut using the final generic owner:

- parent identities/lifecycle/sole table;
- grouped callable boundary and canonical product;
- sole `RuntimeFunctionValue` two-variant enum in place;
- `StreamHandle` with private generic token;
- partial private checked ownership cache;
- all core matches/traversals/accessors/ownership/drop integration.

Open's private atomic commit becomes the sole production token mint. Update direct tests/fixtures through `RuntimeStreamTestAuthority`/real Open path.

**First constructible point:** only the successful atomic P4+C1 commit may produce `StreamHandle`. At that point executable Clone is already unreachable.

Gate:

- key/lease/token/table one-to-one invariants;
- handle move/drop/use-after-move;
- partial recursive ownership/unrestricted duplication/affine rejection;
- no public token/handle literal constructor.

## 5. P5 + C2 — RuntimePlan Stream projection

Land the parent definition table and compiler's one accepted-sema-to-core projection, including grouped boundary, authored evaluation plan, canonical slots/product, fingerprints, capture/ownership facts, and final constant owner usage. No source/name/debug reconstruction or sema dependency in runtime-plan.

Gate: parent focused projection matrix plus ownership/capture/constant tests.

## 6. C3 — structured external application over final owner

Implement non-final grouped application and atomic final Open through inherent behavior on the sole function/partial/product/table types. All inputs are owned slots. Failures return owners or preserve slots. Keep public codec 7 until protected P6+C4 exactly as parent order requires.

Gate:

- effect/evaluation/default order;
- non-final partial ownership and nested affine captures;
- final payload rejection before instance/token allocation;
- final Open atomicity and failure non-mutation;
- suspension frame no reevaluation/no Clone.

## 7. P6 + C4 — protected ABI2/codec8 + ownership wire publication

In one complete protected cut:

- set ABI 2/codec 8;
- land all parent tables/tags/types;
- exact `0x27/0x28/0x29`;
- publish `0x2a CopyValue { dst, src }` and keep `0x2b..=0x7f` unknown;
- update codec, verifier, VM, runtime lowerer, compiler lowering, AOT/JIT codegen, debug maps, safe points, snapshots;
- reject removed/old/provisional bytes with no dual reader.

Gate:

- exact golden bytes and malformed/noncanonical/unknown matrix;
- ownership verifier/VM/interpreter/compiled parity;
- Stream Apply/Open consuming operands and trap atomicity;
- parent P6+C4 full focused matrix.

## 8. P7 + C5 — strict host boundary

Publish the shared core host request/event owner carrying exact `StreamInstanceKey` and canonical product/payload projection. Native, Web, headless, and Agent serialize the same bytes and add no endpoint DTO/flattening/owner carrier.

Gate:

- cross-target byte parity;
- every ineligible/affine cell rejects before Open publication;
- no handle/partial/token/general RuntimeValue through host codecs.

## 9. P8 + C6 — bundle/save2/restore/hot reload

Complete parent bundle schema 6/save schema 2/fingerprints/generation pins/blockers with final generic owner evidence:

- exact partial/closure/canonical product snapshots;
- handle token/lease/table dormant evidence;
- whole-graph occurrence/pin traversal;
- strict tamper order;
- failed restore cleanup;
- atomic empty/replace activation;
- no owner translation/rebinding.

Gate:

- parent P8+C6 save/hot reload matrix;
- duplicate owner/lease/table/orphan/pin tamper matrix;
- original/image/candidate exclusivity and no Open/eval/replay/host activity;
- native/Web/Agent save/restore parity where parent requires.

## 10. Final deletion and combined closure

After P8+C6, remove any final temporary/private migration path that is no longer needed. Run the new matrix plus all 803 retained predecessor rows. Confirm no compatibility aliases/readers/shims/sidecars/source gates/endpoint DTOs/CSS/Takumi additions.

Final commands (use newer exact AGENTS commands if changed and record them):

```text
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
just test-workspace
just test-tier2
cargo metadata --no-deps --format-version 1
just structure-audit
just structure-audit-gate
```

The implementation note records the exact full Git SHA and dirty state, commands, exits, retries, test counts, public API checks, codec golden hashes, structural measurements, and any scope changes. No pass is claimed without raw results.

## 11. Publication invariant table

| Boundary | Executable unconditional Clone | Constructible Stream handle | Public codec 8/0x2a | Save owner evidence |
|---|---:|---:|---:|---:|
| initial/P3 | yes (current defect) | no | no | no final generic owner |
| G1 | temporary yes | no | no | schema only |
| G2 | remaining AWBC/save callers may keep temporary derives | no | no | not active |
| G3.4 exit | **no** | no | no | generic final owner active |
| P4+C1 | no | **yes** | no | in-memory traversal wired |
| C3 | no | yes | no | structured frame wired |
| P6+C4 | no | yes | **yes** | AWBC wired |
| P7+C5 | no | yes | yes | host excludes owner |
| P8+C6 | no | yes | yes | **full save2/restore** |

No allowed boundary has both a constructible handle and executable unconditional Clone.
