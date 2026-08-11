# Normative final contract

## 1. Scope and precedence

Lang-01.3.1.2.3 supplies the generic ABI-2 runtime owner that accepted Lang-01.3.1.2.1, .2, and .2.1 assumed but current production lacks. It does not redesign callable resolution, grouped coordinates, the canonical external argument product, Stream lifecycle/replay/policy, ordinary-function syntax, Proof identity, or Agent behavior.

Precedence is: this package for generic runtime ownership/capture/copy-move-drop/snapshot/payload/plan constants and its AWBC copy allocation; .2.1 for grouped names and Stream wire except that allocation; .2 for partial/effect semantics except owned failure and snapshot correction; .1 for table/lifecycle/replay/host/bundle/save except generic owner correction.

The final model has one `RuntimeValue` representation. Affinity is structural: a value is affine exactly when its transitive graph contains at least one live affine leaf token. Aggregates, closures, and external partials do not receive parallel owner side records merely for being affine.

## 2. Sole classification and token authority

The sole classification is the accepted `RuntimeValueOwnership::{Unrestricted, Affine}`. `join(Unrestricted, Unrestricted)` is `Unrestricted`; every other join is `Affine`. `RuntimeValue::ownership()` computes recursively in deterministic path order. The accepted external partial's cached `ownership` field becomes private and is checked against recursive computation at construction, decode, snapshot, and restore; it is never authority alone.

Every affine leaf owns one opaque `RuntimeAffineOwnerToken`. It is neither `Clone`, `Copy`, Serde, nor publicly constructible. Its typed ID is diagnostic/snapshot evidence only; copying the ID cannot mint authority. The sole production minting path in this sequence is final Stream Open, which allocates token, `StreamConsumerLease`, table entry, handle, and request in one commit.

`StreamHandle` retains accepted key/item-layout/error-layout/lease fields and adds the private token. The sole Stream instance table remains lifecycle/lease authority. The token travels in the value graph and is not a second table. Closures, partials, aggregates, iterators, frames, and fibers own tokens only by owning nested values; no extra aggregate token is minted.

## 3. Public Rust copy, move, and drop boundary

No `Clone`/`Copy` is implemented for `RuntimeValue`, `RuntimeBinding`, `RuntimeFunctionValue`, `RuntimeClosureValue`, `RuntimeExternalStreamPartialFunction`, `StreamHandle`, runtime aggregate/variant/sequence owners, `RuntimeIterator`, mutable environments, register files, frames, fibers, execution state, application/transfer transactions, or runnable restore candidates.

Typed IDs, immutable plans/facts, `RuntimeValueOwnership`, paths, and canonical payload data may remain clonable. The current `RuntimePayload(pub RuntimeValue)` wrapper is replaced in the same owner/name by a closed recursive data enum that mirrors retained safe Serde tags but cannot contain a function, handle, token, iterator, reference, continuation, table, or generic opaque runtime value. That closed enum may implement `Clone`; executable `RuntimeValue` may not.

The only language duplication API is:

```rust
RuntimeValue::try_duplicate_unrestricted(&self)
    -> Result<RuntimeValue, RuntimeDuplicateError>
```

It traverses deterministically, fails at the first affine leaf, and leaves the source unchanged. It never shares a live owner, rotates a lease, invokes a provider, or panics.

Movement is Rust ownership plus checked slots: `take` empties the source; `put` requires an empty destination. There is no affine-clone helper or Stream-specific transfer route.

Language drop is an explicit table-aware transaction. Preparation validates all nested owners and stages domain releases; commit consumes the value and applies releases in deterministic reverse acquisition order. Preparation failure returns or quarantines the still-owned value without table mutation. Rust destructors do not implement language Drop and never mint/rotate/release a Stream lease.

## 4. Exact typed closure capture

A closure captures exactly free locals named by accepted HIR capture evidence, keyed by `(closure_expr_id, outer_local_id)`. First source use fixes capture ordinal; later uses reuse it. Parameters are excluded. Nested closures resolve the nearest visible local. No runtime path may call `bindings_snapshot()`, capture the whole environment, scan source text, or infer captures from names/debug strings.

The compiler projects one `RuntimeCapturePlan`. Each entry carries exact source local/slot, destination capture slot, expected type, and `Copy | Move`. Borrow captures are functionalized or rejected before RuntimePlan; executable closures contain no escaping `&RuntimeValue`.

Capture execution is transactional:

1. validate plan identity, source/destination uniqueness, source liveness/type, mode legality, and owner-token uniqueness;
2. stage all `Copy` entries through checked unrestricted duplication in capture ordinal order;
3. record and recheck every `Move` source-slot revision and canonical owner-ID set without mutating a token or visible slot;
4. atomically take move sources and publish the complete closure;
5. on preparation failure dispose staged copies and leave every source slot/environment revision unchanged.

Moving an affine capture into an inner closure consumes the outer capture slot. Later use is a typed use-after-move error. Duplicating an unrestricted closure recursively uses the same checked duplication; any affine capture makes the closure affine.

## 5. Calls and partial application

Callables and arguments live in ownership slots. Lowering emits exact Copy/Move uses. Application preflights callee, evaluated group, canonical product, generation, token uniqueness, destination, and Stream transaction before commit.

A non-final external group consumes its input callee/application values and publishes one new partial. A final group consumes them and atomically publishes opening instance, handle, table entry, and request. Any affine captured cell makes the partial affine. `OmittedOptional` owns nothing; rest aggregates structurally own members.

The .2 plain-error return is narrowly corrected: a preparation failure either leaves source slots untouched or returns `RuntimeOwnedFunctionApplicationFailure` containing callee and evaluated owners. No error path Rust-drops the sole affine owner before caller cleanup. Already-performed authored/default expression effects retain accepted ordinary semantics, while ownership/table/request/destination publication remains atomic.

## 6. Generic operation results

### 6.1 Locals, patterns, construction, projection

- By-value local use is explicit Copy or Move. Copy requires unrestricted and retains source; Move empties it. A `RuntimeValueRef<'_>` is ephemeral and cannot cross suspension, host, save, frame exchange, or closure escape.
- `let`, parameters, and patterns evaluate RHS once. Owned destructure moves; non-consuming tests borrow; duplicate bindings require unrestricted. Order is accepted HIR depth-first preorder, sequence/tuple left-to-right, authored record order.
- Tuple, record, sequence, variant, rest, call-argument, and return construction consumes operands left-to-right after preflight. Reuse requires an earlier explicit Copy.
- Non-consuming materialized projection duplicates an unrestricted projected graph. Consuming projection takes the whole aggregate, moves the selected component, and disposes the remainder. No public hole remains.
- Variant matching borrows the tag then executes the selected typed move/copy binding plan. Owned rest moves remaining members; borrowed rest requires all copied members unrestricted.
- Assignment evaluates/preflights replacement, atomically replaces the slot, then disposes old value. Failed replacement/drop preparation leaves old binding live and unchanged.

### 6.2 Cross-fiber and iterator

A child fiber receives only its typed capture packet. Sender-slot copies/moves, child creation, mailbox/frame insertion, lexical scope membership, and scheduler observation commit together. No parent environment is cloned. Compiled-region/facade exchange uses owned transfer packets, never cloned `FiberState`.

The existing `RuntimeSeq` and `RuntimeIterator` owners are changed in place; no `RuntimeSequenceValue` wrapper is introduced. `RuntimeSeq::into_values(self)` consumes `Values`, dense storage, and tuple/record columns and moves every logical cell exactly once. `RuntimeIterator::Values` owns the resulting `IntoIter<RuntimeValue>`. Iterator construction consumes the source collection; `next()` removes and returns each element exactly once, and exhaustion never revisits storage. Numeric range iteration retains the existing non-materialized cursor.

### 6.3 Repeat, indexing, slicing, push

For evaluated value `v` and exact count `n`:

- `n == 0`: consume and dispose `v`, return empty;
- `n == 1`: move `v` into singleton;
- `n >= 2`: require unrestricted, preflight count/budget, create `n-1` checked copies, then move original into the final slot.

Affine repeat is permitted only when sema proves exact count 0 or 1. A dynamic/non-singleton count domain requires unrestricted even if one run yields 0 or 1. Failure publishes no partial sequence and preserves a slot-backed source transaction.

Ordinary `sequence[index]` and `sequence[slice]` preserve the source and are copy-producing inherent operations on the existing `RuntimeSeq`. They require recursive sequence ownership `Unrestricted`; empty slice is not an exception. Runtime checks permission/type, then bounds/range, then duplicates selected values. Affine extraction exists only through consuming destructure, iterator conversion/next, or internal typed `take_owned` that consumes the whole sequence. `push` consumes both the original sequence and its element, returning both on preparation failure; matching dense/columnar storage is extended directly and other shapes are converted by consuming moves into `RuntimeSeq::Values`.

### 6.4 Equality, joins, cleanup

Equality borrows both operands and transfers nothing. It is available only when accepted type evidence makes the complete type equality-comparable. Function values, `StreamHandle`, and other non-Eq runtime leaves are rejected; affinity alone does not force copying.

At branch/match joins every live local/register must have identical type/liveness/ownership facts. Live on one path and moved on another is accepted only when dead after the join and lowering inserts explicit Drop on the live path; otherwise verification fails. Return/break/cancellation/trap/unwind cleanup uses one explicit reverse-registration stack exactly once.

## 7. AWBC ABI-2 ownership

ABI 2 uses one register model: `Uninitialized | Live { type_id, ownership } | Moved | Dropped` (terminal states may be compacted while retaining diagnostics). Existing `Move` changes from clone-like behavior to a consuming source-to-empty-destination transition. Existing `Drop = 0x1f` consumes and routes through table-aware drop.

This later contract allocates the next generic codec-8 instruction:

```text
0x2a CopyValue { dst, src }
wire = 2a <dst:canonical-varu32> <src:canonical-varu32>
```

This narrowly supersedes .2.1's `0x2a` unknown statement; `0x2b..=0x7f` remain unknown. `CopyValue` requires live statically unrestricted source and empty destination; source stays live and destination receives checked duplicate. A runtime ownership mismatch traps before mutation.

All aggregate constructors, calls, captures, returns, child exchange, Stream Apply/Open operands, and compiled boundaries classify each operand as borrowed/copied/consumed. Reuse goes through `CopyValue`; constructors never clone internally. Joins require identical facts. Safe points require no in-flight ownership transaction, no borrow, and every live value in an owned slot.

Instruction execution is prepare/stage/commit/cleanup. A preparation/staging trap leaves registers, frames, fibers, table, request batch, mailbox, and cleanup stack unchanged. Interpreter and compiled paths call the same core slot/transfer APIs. Accepted Stream opcodes remain exactly `0x27 OpenStream`, `0x28 FinishStream`, `0x29 ApplyExternalStreamGroup`; `CopyValue` lands only in the protected P6+C4 ABI2/codec8 cut.

## 8. Snapshot, save, restore

Snapshot is not runtime cloning. `begin_snapshot` exclusively freezes the complete execution at a global checkpoint; while frozen it cannot run. Traversal emits canonical `RuntimeValueSnapshotV2` data. Unrestricted values become data; affine leaves become dormant `RuntimeAffineOwnerSnapshotV2` evidence containing owner ID and typed domain evidence (Stream key/lease/layout/table relation). Candidate contains no runnable token/provider handle.

After candidate construction the original may resume; snapshot remains dormant. Copying bytes copies evidence, not authority. Activation is private to runtime driver and only installs into an empty driver or atomically replaces one frozen session. An isolated `RuntimeActivationArena` materializes dormant records without registering them runnable. Final commit retires/revokes replaced owners and activates the complete candidate in one non-fallible swap. No public alongside-install API exists.

Restore order is fixed:

1. envelope/schema/checksum/canonical framing;
2. artifact/content identity, ABI2, codec8, host ABI, bundle/save versions;
3. size/count/tag budgets;
4. recursive ownership recomputation and snapshot/persistent eligibility;
5. duplicate owner IDs and duplicate Stream key/lease occurrences;
6. constants/values/environments/captures/iterators/registers/frames/mailboxes/children/cleanup;
7. Stream table/tombstones/cursors/lifecycle/accounting;
8. exact token-handle-table reciprocity and payload exclusion;
9. exact required-generation set;
10. isolated dormant execution construction;
11. active freeze/epoch recheck/old-owner retirement/one atomic swap.

Any failure destroys dormant arena and staged copies; active session remains runnable and equivalent. Snapshot/restore performs no argument/default evaluation, Open, provider work, replay injection, host dispatch, scheduler progress, or lease rotation.

Generation traversal covers all roots/env bindings, closure captures, external partial products/rest, aggregates/variants/sequences, iterator remainder, registers/frames/suspended applications, mailboxes/children/transfer packets/cleanup stacks, handles and sole table entries. Partial contributes definition/generation and nested values; handle contributes instance generation/table relation. Encoded and recomputed pin sets must be exactly equal; missing and extra pins both fail.

## 9. Payload, host, replay, persistence

`RuntimePayload` is the sole closed cloneable non-runnable host/replay algebra. Conversion from `RuntimeValue` is two-phase: a complete borrowed eligibility/type/layout/budget pass runs first, then an infallible consuming conversion. It accepts only recursively payload-eligible unrestricted values and rejects functions (including unrestricted closures/partials), handles/tokens, iterators, references, continuations, and runtime tables with exact value path. There is no `From<RuntimeValue>`, opaque variant, or unchecked public constructor.

External Stream host arguments remain in accepted typed `RuntimeExternalStreamArgumentProduct`; each transmitted cell converts to `RuntimePayload`. An affine value may be locally captured by a partial, but final Open fails before instance allocation if that value would cross the general payload boundary. This contract invents no capability-specific owning host parameter.

Replay stores typed identities, canonical payload outcomes, digests, and lifecycle facts—never runnable handle/token/partial. Save schema 2 is the only owning persistence boundary for such values. Native/Web/headless/Agent adapters forward the same core bytes and define no endpoint ownership DTO.

## 10. RuntimePlan, AOT/JIT, fixtures

Delete both `RuntimeExpr::Value(RuntimeValue)` and `RuntimePattern::Literal(RuntimeValue)`. Expressions and pattern literals reference `RuntimeConstantId` in one immutable `RuntimeConstantTable`. Each private entry owns a checked `RuntimePlanConstant`, which is a thin immutable wrapper around the closed `RuntimePayload` data algebra plus exact layout/digest evidence—not a live `RuntimeValue`. `instantiate(id)` clones only that closed data and consumes it into a fresh executable value; borrowed pattern matching reads the same constant without materializing it.

The table itself is not `Clone` or general-Serde. `RuntimePlan` is also non-`Clone` and has no direct Serde; the parent bundle codec constructs one checked plan and consumers share `Arc<RuntimePlan>`. Plan/AOT/JIT cache clone therefore shares only the plan Arc, IDs/digests, and immutable compiled artifacts, never a live runtime value/env/frame/iterator/partial/handle.

The original `RuntimeFlow`/`FlowOp` owner is normalized in place to immutable block IDs. `FlowOp::Bind`, `LoopNext`, `WhileNext`, `WhileLetNext`, and `ForNext` are deleted. `FlowFiber.pending_ops: VecDeque<FlowOp>` is deleted; the existing `FlowCursor` addresses `(flow, block, op)`, while the existing `FlowControlStackEntryKind` owns loop/while/while-let/for continuation state and the sole live `RuntimeIterator`. Pattern bindings commit directly to `RuntimeEnv` through adjacent binding plans. No parallel op enum, cloned body, or live plan field exists. `Engine`, fibers/statuses/control frames, and compiled exchanges are non-`Clone`.

Affine and runtime-only values cannot be plan constants. Affine tests obtain leaves only through the sole Stream table test authority, not a raw/fake token constructor.

## 11. Required implementation sequence

The exact compile-clean order in `IMPLEMENTATION_ORDER.md` is normative:

1. G1 generic classification/token/errors/capture/constant APIs;
2. G2 plan constants and structured runtime migration while no handle is constructible;
3. G3 AWBC/fiber/compiled/snapshot migration and removal of unconditional executable Clone;
4. only then P4+C1 grouped partial/handle/table publication;
5. P5+C2, C3, protected P6+C4 including `CopyValue=0x2a`, P7+C5, P8+C6;
6. final deletion and combined matrix.

Unconditional executable Clone disappears at G3 exit. Stream handles become constructible only at atomic P4+C1 after G3. No intermediate main/release state exposes a handle while executable values remain unconditionally clonable.

## 12. Prohibited solutions

No panic-on-Clone, Arc sharing of live affine owner, lease rotation, debug-string side table, Stream-only runtime enum, second environment/register file, copied capture registry, source free-variable reconstruction, local extension trait, compatibility alias, dual reader, migration shim, endpoint DTO, source gate, removed-syntax diagnostic, CSS, or Takumi path. Core/data stay Sans I/O and layer direction is preserved.
