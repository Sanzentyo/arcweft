# RuntimePlan, host, replay, and persistence boundaries

This file fixes every boundary that previously relied on cloning `RuntimeValue` or could accidentally expose an affine owner through a generic codec.

## 1. Boundary classification

A runtime value is evaluated independently for three different properties:

```text
Ownership:              Unrestricted | Affine
PlanConstantEligible:   yes | no
PayloadEligible:        yes | no
SnapshotEligible:       yes | no, under exact save context
```

These properties are not aliases. Examples:

| Value | Ownership | Plan constant | General payload | Save schema 2 |
|---|---|---:|---:|---:|
| integer/string/payload record | Unrestricted | yes when deterministic/canonical | yes | yes |
| unrestricted runtime closure | Unrestricted | no | no | yes when capture/body/generation rules allow |
| unrestricted external partial | Unrestricted | no | no | yes with exact generation |
| affine external partial | Affine | no | no | yes only when every capture has owning snapshot evidence and generation is pinned |
| `StreamHandle` | Affine | no | no | yes through handle/table evidence at a global checkpoint |
| consuming iterator | structural | no | no | yes only if iterator variant/state is snapshotable |
| runtime reference/borrow | structural | no | no | no |
| continuation/frame/table | structural | no | no | only through their dedicated whole-execution snapshot owner |

No caller infers one property from another except the explicit rule that plan constants and general payloads require `Unrestricted`.

## 2. RuntimePlan constant replacement

Delete both live-literal forms:

```rust
RuntimeExpr::Value(RuntimeValue)
RuntimePattern::Literal(RuntimeValue)
```

Replace them in their existing enums with:

```rust
RuntimeExpr::Constant(RuntimeConstantId)
RuntimePattern::Literal(RuntimeConstantId)
```

and one plan-owned immutable table:

```rust
#[derive(Debug)]
pub struct RuntimePlan {
    // parent-final immutable tables are private
    constants: RuntimeConstantTable,
    digest: RuntimePlanDigest,
}

impl RuntimePlan {
    pub fn constants(&self) -> &RuntimeConstantTable;
    pub fn digest(&self) -> RuntimePlanDigest;
}

impl Engine {
    pub fn new(plan: Arc<RuntimePlan>) -> Self;
}
```

`RuntimePlan` implements neither `Clone` nor direct Serde. The accepted bundle reader constructs the checked plan once and hands out `Arc<RuntimePlan>`; the engine and compiler caches never own separate plan copies.

`RuntimeConstantTableBuilder::try_push` consumes a value, checks recursive ownership first, then the closed plan-constant eligibility schema, then canonical type/layout/digest/budgets. An error returns the original value in `RuntimeOwnedPlanConstantError`; it never Rust-drops a potential owner. On success it stores `RuntimePlanConstant(RuntimePayload)`: a thin immutable checked-data wrapper, not a live `RuntimeValue`.

The final eligible set is the existing deterministic literal/data subset only: unit/bool/integer/finite numeric/text/bytes/entity/range and closed tuple/record/sequence/nominal-record/variant aggregates recursively composed from eligible values, subject to existing type schema and budgets. Functions, partials, handles, iterators, references, continuations, frames, runtime-only IDs, tables, provider objects, and opaque values are ineligible even if unrestricted. `RuntimePattern::Literal` additionally requires accepted equality evidence for its literal type.

## 3. Plan/AOT/JIT cache behavior

A plan cache, lowered-region cache, AOT artifact handle, or JIT cache may implement/derive `Clone` only by cloning immutable IDs/digests/artifact handles and `Arc<RuntimePlan>`. `RuntimePlan` and `RuntimeConstantTable` themselves do not implement `Clone`. A cache never owns or clones:

- an instantiated `RuntimeValue`;
- an environment/binding set;
- closure captures or partial arguments;
- a Stream handle/token/lease/table entry;
- registers/frames/fibers/cleanup state;
- a runnable restore candidate.

Execution instantiates a constant at the point it is needed:

```rust
let value = plan.constants.instantiate(id)?;
```

This clones only the closed `RuntimePlanConstant` data and consumes it into a fresh `RuntimeValue`; it never invokes `Clone` on an executable graph. Pattern matching borrows the same table entry and compares through typed borrowed equality without materializing. A compiled region may embed raw machine immediates for proven scalar constants, but all deoptimization/materialization paths produce a fresh value equivalent to `instantiate`; no live value pointer is cached.



### 3.1 No live continuations inside `FlowOp`

The original `RuntimeFlow`/`FlowOp` representation is normalized in place to a block arena. Body-bearing ops reference `RuntimeFlowBlockId`; `RuntimeMatchArm` references a block and carries its adjacent binding plan. The following runtime-only variants are deleted: `Bind`, `LoopNext`, `WhileNext`, `WhileLetNext`, and `ForNext`. Their old tags/shapes are rejected by the sole bundle plan decoder.

`FlowFiber.pending_ops: VecDeque<FlowOp>` is deleted. The existing `FlowCursor` gains a block ID and is the sole program coordinate. The existing `FlowControlStackEntryKind` owns loop/while/while-let/for continuation state; only its `For` variant owns a live `RuntimeIterator`. Bindings are committed directly into `RuntimeEnv` through the typed binding transaction. Body completion re-reads the immutable owner op through the plan `Arc`; no body, op, iterator, or environment is cloned. `Engine`, fibers, statuses, control frames, and compiled exchanges implement no `Clone`.

Cache identity includes the canonical constant-table digest. Source spans/debug labels are excluded according to existing parent identity rules. Cache eviction drops only immutable plan data, not language owners.

## 4. Runtime accelerator/compiled plans

The accelerator receives immutable RuntimePlan/AWBC and owned execution inputs. It returns a non-Clone `RuntimeCompiledRegionExchange`. It does not retain a hidden cloned baseline `FiberState` for rollback. Trap atomicity is implemented by validating/staging the returned exchange against the unchanged core state and committing once.

Speculation may duplicate only unboxed/proven unrestricted scalar machine values. A type/layout that may contain an affine owner remains in core-managed owned slots or a compiler-tracked single-owner machine location. Deoptimization transfers it once into core state.

## 5. General `RuntimePayload`

The current `RuntimePayload(pub RuntimeValue)` wrapper is replaced in place by the exact closed enum in `RUST_OWNERS_AND_APIS.md` §6. It may implement `Clone`/Serde because every recursive field is non-runnable data. Its variant/field spelling mirrors the existing safe `RuntimeValue` Serde shape, including payload sequence storage, so accepted payload bytes remain shared core bytes rather than an endpoint DTO. It cannot include `RuntimeValue`, `RuntimeFunctionValue`, `StreamHandle`, generic owner/token/evidence, iterator, reference, continuation, frame, or table.

Conversion is explicit:

```rust
RuntimeValue::payload_eligibility(&self)
RuntimeValue::try_into_payload(self)
```

Eligibility traverses in canonical value path order and requires:

1. a payload-supported runtime kind/type/schema;
2. recursive `Unrestricted` ownership;
3. canonical finite/size/depth/count limits;
4. no hidden runtime-only identity or execution reference.

Conversion is two-phase: the borrowed pass validates the complete graph and all limits before the private infallible consuming projection begins. On failure the error therefore owns and returns the byte/state-equivalent original runtime value. There is no `From<RuntimeValue>`, unchecked constructor, opaque variant, lossy/debug-string fallback, or adapter-local projection.

## 6. External Stream arguments and host boundary

Retain the .2.1 sole canonical product:

```rust
RuntimeExternalStreamArgumentProduct {
    definition,
    declaration,
    generation,
    signature,
    completed_groups,
    coordinates,
    values,
}
```

A non-final partial may locally own any runtime value admitted by the typed call/runtime rules, and therefore may become affine. Final external Open validates every transmitted cell/rest member through `try_into_payload` before reserving instance ID/token/lease/table/request. If one value is ineligible, the entire Open preparation fails and the owned application error returns callee/evaluated owners; no instance exists.

The core Open request retains the canonical product/typed payload projection selected by the parents. Native, Web, headless, and Agent adapters serialize the same core request directly. They do not define endpoint-specific ownership DTOs, flatten grouped coordinates, look up names, clone a handle, or receive a live partial/token.

Host response/event payloads are likewise closed `RuntimePayload` data. A provider cannot return a generic runtime value or Stream handle through the event codec.

## 7. Host adapter ownership

Adapters may own host-side requests/work/provider objects, but those are not Arcweft language `RuntimeValue` leaves unless a separate typed owning boundary exists. Host objects do not carry `RuntimeAffineOwnerToken` and cannot manufacture a language handle by serializing a key/lease.

A host event identifies the parent Stream request/instance through the accepted typed IDs. Core validates and commits it into the sole table. The unique consumer handle remains in the execution graph. Dropping the host request/provider does not drop the language consumer; dropping the language handle notifies the table/host according to the accepted parent policy.

## 8. Replay

Replay stores only accepted deterministic protocol evidence:

- typed request/instance/definition/generation identities;
- canonical payloads or parent-approved redacted/hash/summary records;
- event/commit sequences, lifecycle/outcome/accounting facts;
- artifact/fingerprint/digest values.

Replay never stores or recreates:

- `RuntimeAffineOwnerToken`;
- a runnable `StreamHandle` or external partial;
- a mutable environment/register/frame/fiber/table pointer;
- a provider object or runtime borrow;
- a generic `RuntimeValue` blob.

During ordinary replay of a restored/started execution, handle/token/table owners are created only by normal execution/restore authority. Recorded events are injected against an already valid typed instance according to parent replay rules. Replay bytes cannot be decoded as a handle or used to rotate/mint a lease.

An affine partial/handle influences replay through exact generation/instance/lifecycle identities and canonical effects, not by being embedded in the trace. Replay does not call Open while merely validating a snapshot/candidate.

## 9. Persistent boundaries

The persistent eligibility matrix is closed:

| Boundary | Accepted owner/data | Rejected |
|---|---|---|
| RuntimePlan/bundle constant table | `RuntimePlanConstant(RuntimePayload)` + layout/digest, proven unrestricted + plan-eligible | live `RuntimeValue`, function/partial/handle/iterator/runtime state |
| canonical general value codec | `RuntimePayload` only | generic `RuntimeValue`, owner evidence, function/handle |
| host request/event JSON/binary | accepted core request/event + payload values | endpoint DTOs, live token/handle/partial |
| replay trace | typed IDs/digests/payload records/lifecycle facts | runnable values/owners/provider objects |
| save schema 2 | whole-execution snapshot DTO, function/partial/handle evidence, sole table snapshot | live tokens/references/opaque value escape |
| bundle executable | RuntimePlan/AWBC/constant tables/fingerprints | live execution values/state |

No boundary accepts a `Serialize` implementation on live `RuntimeValue` as a shortcut.

## 10. Bundle and canonical codecs

Bundle schema 6/codec 8 remain parent-owned. The bundle contains immutable programs, type/signature/Stream definition tables, the strict `RuntimePlanConstant` table artifact, and fingerprints. It does not contain a live owner token/lease occurrence. Direct derived Serde on `RuntimePlan` is removed; bundle decode validates the artifact and rebuilds one `RuntimeConstantTable` inside the checked `Arc<RuntimePlan>`. Expression and pattern literals use that table, never the save-value snapshot codec.

Save schema 2 uses `RuntimeValueSnapshotV2` and parent table/function snapshots. The general canonical payload codec never gains a tag for snapshot evidence or a generic opaque runtime value. Unknown tag/version remains a hard error; no schema-1/codec-7 migration or dual reader is added.

## 11. Runtime-driver save/restore/swap

The driver owns exclusivity, generation pin retention, active artifact set, and one atomic execution swap. It invokes core snapshot/restore APIs; adapters and bundle crates do not activate owner evidence.

A session swap/hot reload cannot clone the current execution to stage a candidate. It freezes/observes the current state, builds dormant evidence or retains the old execution until the new immutable artifact validates, then performs one accepted replacement according to `SNAPSHOT_SAVE_RESTORE_CONTRACT.md`. Failed validation leaves the current session and presentation/facade state unchanged.

## 12. Native/Web/Agent parity

All targets share:

- identical ownership classification and payload eligibility;
- identical grouped product coordinates/order;
- identical host request/event bytes for the same core value;
- identical rejection of partial/handle/token/general runtime value leakage;
- identical save/replay generation/instance identity;
- no target-specific clone/lease workaround.

Web JS cloning/structured-clone, Agent JSON, or native Rust `Clone` cannot be used as language duplication. Adapters receive only the already projected closed data owners.

## 13. Tests and fixtures

### 13.1 Fixture construction

Unrestricted runtime test values use ordinary constructors/builders. Plan fixtures use `RuntimeConstantTableBuilder` and instantiate.

Affine fixtures use a crate-private `RuntimeStreamTestAuthority` that:

1. creates a minimal accepted definition/profile/table;
2. executes the same private reserve/commit path as Open;
3. returns one handle in one runtime value plus its table/execution context.

It cannot mint a detached raw token or construct two handles for one lease. Compile-fail tests prove no public raw token/handle struct literal/Clone path exists.

### 13.2 Golden/canonical tests

- constant-table canonical digest/bytes and clone-sharing identity;
- general payload round trips for allowed values;
- exact rejection path/kind for function, partial, handle, iterator, reference, continuation, affine aggregate;
- host bytes equal across native/Web/Agent;
- replay trace contains no live owner carrier/tag;
- save schema 2 round trip retains dormant evidence and exact pins;
- old/provisional generic `RuntimeValue` codec bytes are unknown/rejected.

## 14. Deletion end state

Delete every successful path that:

- stores a live `RuntimeValue` directly in `RuntimeExpr`/plan/AOT/JIT cache;
- embeds `RuntimeValue` in an expression or pattern literal, or clones a runtime value to instantiate/match one;
- serializes/deserializes a live generic `RuntimeValue` through payload/bundle/host/replay codecs;
- exposes a handle/partial through `RuntimePayload`;
- flattens the canonical grouped product to endpoint arguments;
- constructs host-specific ownership DTOs;
- snapshots/swaps by cloning runnable env/frame/fiber/table/facade state;
- creates affine test values with fake/debug-string/raw token constructors.

The final state has one explicit owner for each boundary and no compatibility alias or hidden fallback.
