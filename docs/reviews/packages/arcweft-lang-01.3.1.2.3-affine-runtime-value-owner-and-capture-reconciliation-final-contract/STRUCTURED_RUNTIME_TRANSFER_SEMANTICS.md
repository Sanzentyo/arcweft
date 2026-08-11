# Structured runtime transfer semantics

This file is normative for the interpreter/structured engine. AWBC must produce the same outcomes and ownership transitions. “Copy” below always means `RuntimeValue::try_duplicate_unrestricted`; “Move” means taking a live typed slot and leaving it moved; “Borrow” is a non-escaping `RuntimeValueRef<'_>` valid only for the current operation.

## 1. State model

Every executable owner stores values in typed slots:

```text
Empty -> Live(value) -> Moved
                    \-> Dropped
```

`Moved` and `Dropped` are terminal until the owning lexical slot is reinitialized by an explicit assignment/binding operation. A use of `Empty`, `Moved`, or `Dropped` fails before evaluating any dependent expression or mutating another slot.

A value is `Unrestricted` only when every transitive child is unrestricted. One nested affine leaf makes the complete value `Affine`. Moving an aggregate moves every nested owner together; the runtime never extracts/copies a token separately from its leaf value.

## 2. Universal transaction phases

Every operation that can fan out, move more than one source, release an owner, publish a destination, enqueue a request, or spawn a child uses these phases:

1. **Shape preflight** — plan identity, source/destination cardinality, types/layouts, indices, source liveness, destination emptiness, static mode, limits, and generation facts.
2. **Ownership preflight** — recursively classify sources, reject illegal Copy, collect owner IDs, reject duplicate owner IDs and any conflicting in-flight execution transaction, and validate domain reciprocity such as Stream key/lease/table.
3. **Stage** — reserve vector/map capacity and construct every unrestricted duplicate; evaluate no source expression in this phase.
4. **Prepare moves/releases** — record source-slot revisions, owner-ID sets, and prevalidated domain actions without changing visible slot states, tokens, or tables.
5. **Commit** — take sources in plan order, install complete destinations, apply prevalidated table/release operations, and publish one revision/observation.
6. **Cleanup** — dispose transaction-local staged copies on pre-commit error; after commit, cleanup only follows the newly published owners.

An error from phases 1–4 leaves source slots, environment/frame revisions, Stream table, request queues, scheduler, observations, and destinations unchanged. Phase 5 is constructed to be non-fallible. Ordinary authored expression effects that occurred before this ownership transaction are not rolled back.

## 2.1 Structured control-flow ownership

`RuntimeFlow` is immutable block-arena plan data shared through `Arc<RuntimePlan>`. The structured engine borrows each `FlowOp` by `FlowCursor`; it never clones a body or queues an owned op. `FlowOp::Bind`, all `*Next` variants, and `ForNext` do not exist. The original `FlowControlStackEntryKind` owns loop continuation state and the sole live `RuntimeIterator`; await/host/child-join states retain owner/resume cursors. Successful pattern selection commits the adjacent binding plan directly into the original `RuntimeEnv`. This same owner split is required of the AWBC and compiled-region paths.

## 3. Exact operation matrix

| Operation | Source use | Permission/result | Source after success | Failure atomicity |
|---|---|---|---|---|
| local by-value read | Copy or Move from typed evidence | Copy requires unrestricted | retained for Copy; moved for Move | no mutation on rejection |
| local condition/match tag/equality | Borrow | typed non-escaping borrow | live | no mutation |
| `let` whole binding | Move evaluated temporary | one destination | temporary consumed | binding unpublished on failure |
| duplicate pattern binding | first destination Move, later destinations Copy from staged value | all later copies require unrestricted | RHS consumed once | no partial bindings |
| tuple/record/sequence/variant construction | Move each operand left-to-right | every destination slot preallocated | operands moved | no partial aggregate |
| non-consuming projection | Borrow then Copy projected graph | projected graph unrestricted | aggregate live | no mutation |
| consuming projection/destructure | Move whole aggregate | selected child moved; remainder dropped | aggregate moved | no public hole; failure returns aggregate |
| call argument/return | Copy or Move from typed lowering | ordinary by-value is consuming; reuse requires prior Copy | consumed where Move | no partial callee frame/publication |
| closure capture | exact plan Copy/Move | affine always Move; unrestricted Copy | copied or moved | complete closure or unchanged env |
| partial application | consumes callee and evaluated group | result partial/open owns all accepted cells | inputs moved | owned error returns inputs |
| assignment | replacement Move; old value prepared Drop | replacement type valid and old drop valid | replacement live; old dropped | old remains live on failure |
| cross-fiber capture | exact Copy/Move packet | no ambient env | sender copied/moved | child absent on failure |
| iterator construction | Move collection | owner moves into iterator | source moved | no iterator on failure |
| iterator `next` | Move next internal element | each element once | iterator advances | no repeated item |
| repeat `0` | Move then Drop | exact-zero evidence or unrestricted | source dropped | source returned on prepare failure |
| repeat `1` | Move | exact-one evidence or unrestricted | source moved into result | source returned on prepare failure |
| repeat `>=2` | `n-1` Copy then final Move | unrestricted and within budget | source moved; copies in result | no partial result/source unchanged before commit |
| index/get | Borrow collection + Copy item | recursive collection unrestricted | source live | permission before bounds; no mutation |
| slice | Borrow collection + Copy selected items | recursive collection unrestricted, including empty slice | source live | no partial slice |
| push | consume sequence + Move element | storage shape/capacity/type valid | new sequence owns all cells | original sequence and element returned on failure |
| equality | Borrow both | typed Eq schema | both live | no mutation |
| explicit Drop/unwind | Move to prepared drop | every nested owner/table relation valid | dropped exactly once | original remains live on prepare error |

## 4. Local lookup and binding

### 4.1 By-value local use

RuntimePlan carries `RuntimeTransferMode` for every by-value local use. The structured evaluator does not infer from Rust clonability or inspect the value kind ad hoc.

```text
Copy:
  check Live + expected type
  call try_duplicate_unrestricted
  source stays Live
  return staged duplicate

Move:
  check Live + expected type
  record source revision + canonical nested owner-ID set
  take source at commit
  source becomes Moved
  return original value
```

A later use of a moved local yields the typed source-bound ownership diagnostic selected by sema/verifier and a runtime invariant error if invalid executable input reaches execution. No fallback duplicates the value.

### 4.2 Binding and shadowing

The RHS is evaluated in the pre-binding environment. New slots are allocated only after the complete binding plan validates. This preserves the accepted `let x = x` outer-resolution rule.

Pattern traversal is depth-first preorder: whole binding first when present, tuple/sequence left-to-right, record fields authored order, variant payload after tag, and rest after explicit members. `_` has no destination. Duplicate names remain a sema error; a recovered executable binding may not publish two destinations.

### 4.3 Mutable binding and capture reassignment

Assignment to a local replaces the value in that local slot. A captured mutable slot belongs to the closure value. Reassignment inside the closure changes that closure-owned capture; it does not retain a pointer or alias to the outer environment. This preserves value capture and avoids an escaping runtime borrow. Shared/by-reference capture is outside this contract.

## 5. Pattern and destructuring algorithms

### 5.1 Borrowed match/test

A refutable pattern first borrows the value to inspect tag, scalar, lengths, and field presence. `RuntimePattern::Literal(RuntimeConstantId)` borrows its checked plan constant and uses typed borrowed equality; no live literal value is embedded or instantiated for the test. Matching performs no move while selecting an arm. Once one arm is selected, its directly attached `RuntimePatternBindingPlan` runs transactionally; there is no global pattern side table.

### 5.2 Owned destructure

Owned destructure consumes the whole value. The implementation may internally decompose it, but no public partially-moved aggregate is returned or stored. The transaction:

1. validates selected field/index/rest paths and destination slots;
2. validates every copied binding is unrestricted;
3. stages copies;
4. takes the aggregate once;
5. moves selected components into destinations;
6. applies the prepared drop to unbound remainder;
7. publishes all bindings together.

If a remainder drop cannot be prepared, the aggregate is not taken. If the pattern is impossible or a destination is invalid, no binding is published.

### 5.3 Rest binding

- Owned tuple/sequence rest receives moved remaining members.
- Owned record rest receives moved remaining fields in canonical accepted field order.
- Borrowed rest is copy-producing and requires every included member unrestricted.
- A rest that binds no name owns nothing and does not copy.

## 6. Aggregate construction and projection

### 6.1 Construction

Tuple, record, sequence, variant, positional rest, and named rest constructors consume their operand temporaries. Evaluation order remains language order. Ownership commit occurs only after all operands have evaluated and the final aggregate has reserved capacity. If an expression fails, earlier ordinary expression effects remain, but every completed operand temporary is explicitly cleaned up once.

The aggregate ownership cache, where retained for performance, is private and computed by folding `join` in canonical child order. Decode, handoff, snapshot, and restore recompute and reject a mismatch.

### 6.2 Non-consuming projection

A language projection that leaves the aggregate usable must produce a distinct value. Therefore it borrows the child and checks/copies it. An affine child is rejected even when the selected expression is immediately dropped, because observable permission cannot depend on a later optimizer.

### 6.3 Consuming projection

Consuming destructure or internal `take_owned` consumes the complete aggregate, returns one child by move, and drops every other child through a prepared drop. There is no general `remove(index)` that leaves a hole in a live runtime aggregate. Mutation APIs that replace a member use a complete remove/replace transaction and recompute ownership.

## 7. Exact closure capture

### 7.1 Membership and order

The runtime receives one `RuntimeCapturePlan` derived from accepted HIR capture records:

```text
capture key     = (closure ExprId, outer LocalId)
order           = first source use / CaptureId ordinal
parameters      = excluded
nested lookup   = nearest visible local
source recovery = forbidden
```

`RuntimeEnv::bindings_snapshot()` is not used for executable closure construction. The complete visible environment is never copied as a fallback.

### 7.2 Mode selection

The compiler does not leave mode to runtime:

```text
computed source ownership == Affine       -> Move
computed source ownership == Unrestricted -> Copy
```

`CaptureAccess::Reassign` affects mutability of the closure-owned destination slot, not the transfer mode. If static ownership is unknown/poisoned, executable lowering is rejected. Runtime recomputes and traps before mutation if actual ownership contradicts the plan.

### 7.3 Transaction order

For captures `c0..cn` in ordinal order:

1. validate closure/plan identity and exact contiguous destination slots;
2. validate each source local is live and matches type;
3. reject duplicate source or owner token;
4. stage all Copy values in ordinal order;
5. record and recheck every Move source revision and canonical owner-ID set in ordinal order;
6. preallocate closure capture storage;
7. take Move sources in ordinal order;
8. insert each staged/moved value into its destination ordinal;
9. publish one closure value and one environment revision.

Any failure before step 7 drops only staged unrestricted copies and leaves all source locals unchanged. Steps 7–9 run as one non-fallible commit under exclusive environment access.

### 7.4 Nested closures

An inner closure captures the nearest slot, including an outer closure's capture slot. Copying an unrestricted capture leaves the outer closure slot live. Moving an affine capture leaves the outer closure slot moved; a later invocation/path that reads it fails as use-after-move. A closure containing such a moved-out capture cannot be invoked along a path requiring that slot; verifier/dataflow must establish valid use.

### 7.5 Duplication and partial application

Duplicating a closure recursively duplicates all captures. Any affine capture rejects the operation at its exact capture path. Ordinary closure partial application consumes supplied arguments and, where the callable must remain available, the lowerer emits explicit Copy first. Existing captures are moved into the resulting function value; they are never cloned internally.

## 8. Function call, return, and external partials

### 8.1 Ordinary calls

Every by-value callee/argument is consumed by the call frame unless the typed call ABI marks it borrowed. Reuse of an unrestricted value is represented by an explicit Copy before call. Call-frame construction validates all destinations and stages copies before moving any source.

Returning a value moves it from the callee frame into the caller destination. Frame cleanup then drops every remaining live local/capture in reverse registration order. A return never duplicates a value merely because the Rust carrier was cloneable.

### 8.2 External group application

The accepted authored/default evaluation order and canonical product order remain distinct. Evaluated values are stored in owned slots. Metadata/type/default/group checks that do not require evaluation happen before the first expression. After evaluation:

1. verify callee is the exact partial/definition/generation/signature/next group;
2. assemble the canonical coordinates and product without copying cells;
3. verify owner uniqueness across old captured product and new group;
4. verify non-final/final destination and host payload eligibility where final;
5. prepare partial construction or the full Stream Open table/handle/request commit;
6. consume callee and evaluated group together;
7. publish one new partial or one Open result.

On pre-commit failure, `RuntimeOwnedFunctionApplicationFailure` returns callee and evaluated group. No sole affine owner is lost to Rust drop. On final Open, owner token, lease, handle, instance entry, request ID/request, and destination commit together.

## 9. Assignment and replacement

Assignment is not `mem::replace` followed by fallible drop. The exact order is:

1. evaluate replacement once;
2. validate destination binding mutability/type and replacement ownership;
3. prepare drop of the old live value;
4. preallocate replacement destination storage and record/recheck replacement source and destination revisions;
5. atomically take replacement temporary and old destination;
6. install replacement;
7. commit old-value drop;
8. publish one environment/frame revision.

If steps 2–4 fail, the old destination and replacement temporary remain owned. The user-visible error path owns/cleans the replacement according to ordinary expression cleanup. No assignment leaves the destination empty.

## 10. Cross-fiber transfer

### 10.1 Packet

A child receives a `RuntimeFiberCapturePlan` projected from accepted typed capture/effect/concurrency evidence. It contains exact source slots, child slots, types, and Copy/Move modes. It does not contain names or a parent environment snapshot.

### 10.2 Atomic spawn

The runtime prepares, then commits together:

- staged unrestricted copies;
- prepared affine move records containing source revisions and canonical owner-ID sets;
- child fiber ID and lexical task-scope membership;
- child frame/capture packet;
- parent source transitions;
- child mailbox/join destination;
- deterministic schedule observation.

If allocation, limit, scope, destination, type, or ownership validation fails, no child ID/fiber/scope member/observation exists and parent state is unchanged. Detached work may not retain a borrowed parent value.

### 10.3 Compiled-region exchange

Interpreter, JIT/AOT region, product-step facade, and scheduler exchange `RuntimeValueTransferPacket` by ownership. A packet is non-Clone and consumed by the receiver. Facades do not rebuild a second fiber/environment representation and synchronize it by cloning. Returning from compiled code yields an owned packet plus typed register/fiber facts; the core commit validates them before replacing state.

## 11. Iterator semantics

The existing `RuntimeSeq` and `RuntimeIterator` owners are changed in place. `RuntimeSeq::into_values(self)` consumes `Values`, dense storage, and tuple/record columns, transposing columnar cells into logical rows by move. Construction from sequence/tuple then owns `IntoIter<RuntimeValue>`. `next()` returns the next element once. There is no `RuntimeSequenceValue` wrapper and no index plus `.cloned()` implementation.

Required transitions:

```text
items [a,b], cursor start
next -> a, storage [b]
next -> b, storage []
next -> None
next -> None
```

Dropping/unwinding an iterator prepares and drops all remaining elements exactly once. Save traverses remaining elements in next-delivery order. A numeric range retains the non-materialized checked cursor and contains no value owner.

## 12. Repeat

### 12.1 Static permission

Sema/runtime-plan supplies one of:

- `ExactZero` when the count is statically exactly 0;
- `ExactOne` when exactly 1;
- `Unrestricted` for every other exact/domain/dynamic count.

An affine source under a dynamic count is rejected before runtime, even if a particular execution would produce 0 or 1. Runtime rechecks the permission/count pair.

### 12.2 Runtime algorithm

For `n == 0`, prepare source drop, consume it, commit drop, and return empty.

For `n == 1`, reserve result capacity, move source into the sole slot, and publish.

For `n >= 2`, require permission `Unrestricted`, check count/byte/work budgets, stage `n-1` duplicates, reserve result capacity, then move the original into the last slot. The result order is duplicate copies for indices `0..n-1` and the original at index `n-1`; values are semantically equal. Any staging failure destroys staged copies and returns the original source.

Overflow, one-over-limit, and allocation-budget failure occur before publication. Zero does not leak/drop by Rust destructor; it uses the language drop transaction.

## 13. Index, get, and slice

These operations preserve their source and therefore are copy-producing.

Failure precedence is:

1. typed sequence/index/range evidence;
2. recursive source ownership must be `Unrestricted`;
3. numeric conversion/overflow;
4. bounds/range normalization;
5. output count/byte/work budget;
6. checked duplicates in ascending index order;
7. publication.

Thus an affine sequence with an out-of-bounds index reports ownership illegality before bounds; an empty slice on an affine sequence is still illegal. This keeps permission independent of data/bounds and avoids probing affine containers through apparently empty copies.

Affine values are extracted only by a consuming pattern/destructure, consuming iterator, or crate-private whole-sequence `try_take_owned`. `try_take_owned` is not a general source-level indexing fallback.

## 14. Push and collection mutation

`RuntimeSeq::try_push_owned(self, value)` consumes both the sequence and the element. It validates element type/shape and pre-reserves every needed capacity before consuming internal storage. Matching dense/tuple-column/record-column shapes extend that representation; a mismatch consumes the old sequence through `into_values`, appends the element, and publishes `RuntimeSeq::Values`. On failure `RuntimeOwnedSequencePushError` returns both the original sequence and element. No ownership cache is trusted as authority; any private cache is recomputed/checked from the final graph.

A replace/remove operation, where existing language surface supports one, must follow the same whole-owner transaction: it returns/moves the removed item or explicitly drops it. It may not call `.clone()` to preserve a source.

## 15. Equality

Equality borrows both values. It receives accepted `RuntimeEqualityEvidence` and traverses only allowed schema fields. It transfers no owner and never calls duplication.

- Scalars and payload aggregates compare normally.
- An aggregate containing a non-Eq leaf is not Eq as a whole.
- `RuntimeFunctionValue`, `StreamHandle`, iterators, references, continuations, and runtime tables are not language-equality-comparable.
- Identity-specific APIs may compare typed IDs such as `StreamInstanceKey`; that is not generic value equality.

A forged/mismatched Eq schema fails before reading ineligible leaf details.

## 16. Branch/match joins

Verifier and structured lowering track, for each local/temporary/capture:

```text
(type, Live|Moved|Dropped|Empty, RuntimeValueOwnership)
```

At a join, facts must be identical. The only normalization is a value dead after the join: if one predecessor is live and another moved/dropped, lowering inserts an explicit drop on the live predecessor, after which both are terminal. It is invalid to silently resurrect, copy, or choose the live value.

Loop back-edges use the same rule. An affine loop-carried value must be live on every back-edge or be reinitialized on every path before the header. Match arm binding/cleanup completes before the join.

## 17. Cleanup, cancellation, and unwind

Each lexical scope/frame owns one ordered cleanup stack of live slots/resources. Registration order is deterministic. Normal exit, return, break, cancellation, trap, and unwind select a reason but execute the same ownership cleanup in reverse registration order.

Before cleanup begins, every drop is preflighted for the batch. If current runtime invariant data is corrupt, the execution traps/quarantines without pretending cleanup succeeded; restore/tamper validation should prevent this in accepted states. A cleanup entry transitions exactly once. Child-scope cancellation and value cleanup do not race: child terminalization/capture return (where any) completes before parent-owned slots are dropped according to the accepted structured-concurrency contract.

## 18. Suspension and safe points

A suspension is valid only when:

- no `RuntimeValueRef` borrow survives;
- no transfer/drop/application transaction is in phases 3–5;
- every evaluated temporary is in an owned frame/application slot;
- capture/application cursors point after the last committed evaluation;
- all affine owner IDs occur exactly once in the complete execution graph/table relation.

Resume uses those owned slots; it never reevaluates an expression or reconstructs a value from source/debug text. A local preemption point may retain active unsaveable work; a global snapshot point additionally satisfies snapshot eligibility and whole-execution closure.

## 19. Failure non-mutation checklist

For each structured operation, direct tests compare before/after typed snapshots or canonical state digests on failure. The invariant is not merely “no result”: it includes unchanged source slot states/values, environment/frame revisions, affine owner IDs, Stream leases/table rows, request/event/scheduler queues, child/fiber/scope inventories, cleanup stacks, and observations, except for ordinary expression effects explicitly committed before ownership preparation.
