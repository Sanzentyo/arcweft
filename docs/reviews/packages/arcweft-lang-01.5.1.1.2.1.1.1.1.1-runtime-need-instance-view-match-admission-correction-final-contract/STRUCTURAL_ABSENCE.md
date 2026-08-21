# Structural absence and forbidden architecture

The implementation must prove absence across Rust API/typechecking, private
wire schemas, bundle/save/replay DTOs, generated artifacts, fixtures, and
runtime execution. Text search is supplemental and cannot be the sole gate.

## 1. Forbidden identities and carriers

| Forbidden structure | Required proof |
|---|---|
| `ViewProgramSemanticDigest` | public/private API model has no type; rustdoc/API snapshot and generated schemas absent |
| canonical `u32` View revision | only current `AcceptedViewProgramRevision([u8;32])` accepted by compiler/bundle/runtime |
| second public producer identity | only `NeedProducerInstanceKey` appears in core task model |
| duplicate `RuntimeValueDigest` | type resolves only to `arcweft_core::entry::RuntimeValueDigest` |
| String/hex/suffix NeedId/TaskKey/TaskId | compile-fail String construction; strict legacy decode rejection; final wire fixed bytes |
| NeedHandle-as-String/Dynamic | verifier and VM negative tests plus RuntimeValue shape test |
| direct Await String parser | API/type absence and negative AWBC/runtime test |
| caller-supplied IDs or ordinal in TaskSpec | struct construction compile-fail |
| identity translation table | replacement state schema and runtime journal have no alias/old-new NeedId table |

## 2. Forbidden semantic conflation

| Forbidden structure | Required proof |
|---|---|
| generic `CheckedMatch` calls ownership/persistence | dependency/API instrumentation test; ordinary affine Match accepted |
| View catalog copies Match arms/coverage | checked product schema retains one `CheckedMatchRef`; no duplicate fact type |
| producer admission constructs contract identity | API has no contract output/parameter; separate digest tests |
| whole unrelated catalog digest as View evidence | unrelated catalog-row mutation leaves admission digest unchanged |
| accepted View revision in Match/admission/plan/instance | revision-only property test for stable digests/NeedId |
| HIR IDs/source spans in persistent digests | independent HIR allocation/source-span differential tests |

## 3. Forbidden ownership shortcuts

| Forbidden structure | Required proof |
|---|---|
| `ResourceTypeRegistry` in current ownership context | API/dependency graph absence |
| Agent resource lookup without exact typed key | instrumented registry cannot be called |
| opaque evidence default | missing fields cannot construct/decode; all fixtures explicit |
| opaque behavior extension trait | original enum inherent impl owns tags/semantics |
| wildcard/default current TypeKind classifier | exhaustive match compile failure when enum grows |
| unconditional ViewValue snapshot row | negative typed error test |
| Function type-only capture inference | type classifier rejects; exact value certificate required |

## 4. Forbidden task/event/persistence partiality

| Forbidden structure | Required proof |
|---|---|
| `AwbcTaskPlan.need_id` | schema/API/generated absence |
| task-plan self-digest field | schema absence; recomputation tamper test |
| TaskEvent with only task id | complete correlation field construction required |
| RuntimeNeedState without correlation | final struct/API/snapshot model |
| observer-specific duplicate event stream | two-observer integration test sees one task publication |
| delayed old snapshot reader | old bytes strictly rejected at version-1 decoder |
| replay-only compatibility conversion | live/replay differential uses one event path |
| adapter-supplied task identity | adapter trait accepts derived launch envelope only |
| driver-local GenerationId | one type-definition/API dependency proof |

## 5. Frozen numeric authority

This return contains no AWBC opcode, function-kind, function-flag, varint, or
encoder allocation table. Implementation proof is:

1. the maintained numeric authority is changed only by its separately accepted
   prerequisite migration;
2. this feature cut adds semantic variants/consumers through inherent enum
   behavior without a feature-local numeric table;
3. no compatibility/tombstone reader is added; and
4. a diff gate confirms this correction does not independently renumber the
   frozen authority.

Digest-only tags in `IDENTITY_AND_DIGESTS.md` are domain-local semantic
transcript tags and must not be reused as AWBC numeric interpretation.

## 6. Parent View Await deletion

Where the retained parent old View Await product still exists, Cut 5 proves
absence of:

- `ViewProgramInstruction::Await`;
- `ViewAwait`;
- `ViewAwaitBranchSpan`;
- old four-way Await evaluator;
- `InvalidAwaitState`;
- bundle/runtime/snapshot rows for those types;
- parser/formatter/LSP/source-map `AwaitView` vocabulary; and
- generated API/schema rows.

Unary Need state is selected through ordinary generic Match and the retained
Variant/Tuple selector ABI.

## 7. Proof method

The structural test suite combines:

- compile-fail fixtures for removed constructors/fields/conversions;
- public/private API model snapshots;
- generated schema/source-map model checks;
- strict decode rejection of representative old bytes;
- dependency graph tests;
- exhaustive enum matches that fail to compile on missing rows;
- runtime differential tests; and
- a targeted repository search reported as supporting evidence only.

A dead branch, unused compatibility reader, private alias, or fixture-only old
carrier still violates this contract.
