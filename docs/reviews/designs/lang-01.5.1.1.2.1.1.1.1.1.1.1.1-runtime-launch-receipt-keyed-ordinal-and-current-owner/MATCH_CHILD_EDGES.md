# HIR-only child edges and sema checked roles

## Ownership

`arcweft-lang-hir` owns `HirExpressionChildEdge`,
`HirExpressionChildRole`, `HirNestedExpressionPath`, and its segment enum.
These types use only HIR IDs, ordinals, and HIR-owned path segments.

`HirExprKind::child_edges()` is the one child-order implementation.
`direct_expression_children()` becomes a projection of it. Recovery operand
lookup also indexes the same edge slice rather than maintaining a third switch.

`arcweft-lang-sema::FinalSemanticAnalysis::checked_child_edges(owner)` reads
those edges and enriches them into `CheckedExpressionChildRole`. It may attach
accepted record-field identities and validated nested coordinates, but it does
not reorder, drop, or add a child. HIR therefore gains no dependency on core or
sema.

## Complete 38-family HIR edge inventory

Tags are the retained version-1 semantic tags. Optional edges are absent when
the child is absent. Repeated edges carry source ordinals.

| Tag | `HirExprKind` | Ordered HIR-only edge roles |
|---:|---|---|
| `0x0100` | `Unit` | none |
| `0x0101` | `Literal` | none |
| `0x0102` | `EntityReference` | none |
| `0x0103` | `LifetimePath` | none |
| `0x0104` | `Path` | none |
| `0x0105` | `ShortVariant` | none |
| `0x0106` | `Placeholder` | none |
| `0x0107` | `Tuple` | `Element { ordinal }` |
| `0x0108` | `BracketSequence` | `Element { ordinal }` |
| `0x0109` | `NumericBracketSequence` | none; compact numeric values stay in their typed owner |
| `0x010A` | `ArrayRepeat` | `RepeatedValue`, `RepeatLength` |
| `0x010B` | `Call` | optional `Callee`, then `Argument { ordinal }` |
| `0x010C` | `Select` | `Target` |
| `0x010D` | `Index` | `Target`, `Index` |
| `0x010E` | `Pipe` | `PipeLeft`, `PipeRight` |
| `0x010F` | `Try` | `Operand` |
| `0x0110` | `Await` | `Operand` |
| `0x0111` | `Thread` | none; FlowItem roots remain in their typed inventory |
| `0x0112` | `Choice` | exact nested Choice walk below, then plan items |
| `0x0113` | `Range` | optional `RangeStart`, optional `RangeEnd` |
| `0x0114` | `Record` | explicit-value `RecordField { source_ordinal }` in source order |
| `0x0115` | `RecordLiteral` | explicit-value `RecordField { source_ordinal }` in source order |
| `0x0116` | `Binary` | `BinaryLeft`, `BinaryRight` |
| `0x0117` | `Borrow` | `Operand` |
| `0x0118` | `Dereference` | `Operand` |
| `0x0119` | `Closure` | `ClosureBody` |
| `0x011A` | `Unary` | `Operand` |
| `0x011B` | `Block` | `BlockTail`; statements remain typed roots |
| `0x011C` | `ComputationBlock` | `BlockTail`; statements remain typed roots |
| `0x011D` | `NamedBlock` | `BlockTail`; statements remain typed roots |
| `0x011E` | `Loop` | `LoopTail`; statements remain typed roots |
| `0x011F` | `If` | `Condition`, `ThenBranch`, `ElseBranch` |
| `0x0120` | `IfLet` | `Scrutinee`, optional `IfLetGuard`, `ThenBranch`, `ElseBranch` |
| `0x0121` | `Match` | `Scrutinee`, then optional `Guard { arm }`, `ArmValue { arm }` per arm in source order |
| `0x0122` | `DialogueContentApplication` | `DialogueTarget`, coordinates, interpolations, expression tag payloads, then exact line-plan walk |
| `0x0123` | `PostfixBracket` | `Target`; ambiguous form then `PostfixIndexCandidate`, `PostfixDialogueCandidate` |
| `0x0124` | `Error` | none; semantic transcript rejects this owner |
| `0x0125` | `ForSynthetic` | `ForInput` |

The edge vector's child IDs must equal the current production
`direct_expression_children()` result for all 38 constructors. That equality is
a migration test before the old switch is deleted.

## Nested Choice walk

The HIR path is a nonempty boxed sequence of typed segments from
`HirNestedExpressionPathSegment`. It is structural; it contains no `ExprId`,
scope ID, source range, spelling, or flattened global ordinal.

The walk retains current `append_choice_expression_children` behavior:

1. process each current body item in order; nested bodies are pushed onto the
   current LIFO pending-body stack;
2. `If`: emit branch conditions in branch order, then queue branch bodies and
   optional else body;
3. `For`: emit source, then queue body;
4. `Match`: emit scrutinee, optional guards in arm order, then queue arm bodies;
5. `Option`: emit ID, then its fields in field order;
6. `OptionFor`: emit source, then fields;
7. `CompactArm`: emit label, optional condition, optional Out value;
8. `View` field: key then value for each entry in entry order; and
9. after the body walk, plan items in source order: Assignment value, Timeout
   duration, and expression-bearing Signal/Timeout/Expr cancel triggers.

Every edge uses the specific role from `HirExpressionChildRole` and carries the
typed nested path to its owner. Plan roles retain the source plan-item ordinal.

## Nested dialogue/line-plan walk

The order is target; coordinate values; interpolation expressions;
expression-bearing tag payloads; then line-plan values. A line-plan slice emits
Option, Let, Out, TimelineAssert, and Expression values; TimedCue emits anchor
then body. Start/Together children use the current LIFO pending-group stack.
Statement-bearing items remain statement roots and are not fabricated as
expression edges.

`LinePlanItem`, `LinePlanStartGroupItem`, and
`LinePlanTogetherGroupItem` path segments retain each source ordinal at each
nesting level, so two equal expressions at different positions remain distinct
roles without using arena allocation identity.

## HIR → sema role mapping

Mapping is exhaustive and first-error:

- roles with no semantic payload map one-to-one;
- ordinal payloads are checked for `u32` fit and copied;
- `RecordField { source_ordinal }` resolves the current checked record field and
  becomes `{ source_ordinal, accepted_field: RuntimeRecordFieldId }`;
- every `HirNestedExpressionPath` is validated against the current checked
  Choice/dialogue/line-plan fact and projected segment-for-segment into
  `CheckedNestedPathV1`;
- call `Callee` and `Argument` roles must also be present in
  `FinalSemanticAnalysis::call(owner)` at the same source coordinate;
- Match guards must agree with checked guard presence and boolean type; and
- a missing owner, stale path, wrong child ID, field mismatch, or call-slot
  mismatch returns `CheckedChildEdgeError` before any role/digest is returned.

The complete `HirExpressionChildRole` and `CheckedExpressionChildRole` enums
are in `schemas/final_contract.rs`. Their declaration order is not the wire
tag authority; retained explicit semantic tags remain the parent contract's
`0x1000..=0x103D` table.

## Current callable join

There is no new callable catalog. For a selected ordinary call:

```text
FinalSemanticAnalysis::call(expr)
→ CallTargetFact::Selected
→ ResolvedCallable::checked()
→ FinalSemanticAnalysis::checked_callables()
→ CheckedCallableCatalog::callable(id)
→ id.semantic_digest()
```

The catalog row must agree with the call's signature, receiver mode, effects,
generic instantiation, and result. A selected Method converts the current HIR
lookup name to the current typed `CallableName`, forms `ReceiverMethodKey` from
the accepted receiver `TypeKind`, requires one `CheckedMethodLookup::Unique`
ID, then performs the same catalog join. HIR/source spelling is used only as a
lookup key and is never emitted.

For an intrinsic selected by current typed intrinsic facts, the selected
`ResolvedCallable::id()` (`CallableCandidateId`) must be one of its existing
typed intrinsic variants. Its closed candidate/family tag, accepted intrinsic
signature digest, and instantiation transcript are emitted. Missing, ambiguous,
rejected, or noncallable facts reject. No intrinsic is assigned a fabricated
`CheckedCallableId`.

## Differential acceptance

- `child_edges().map(child)` equals pre-migration
  `direct_expression_children()` for every constructor and nested fixture;
- changing ExprId allocation, HirSnapshotId, spans, or display spelling while
  retaining accepted facts leaves the semantic digest unchanged;
- changing accepted field identity, checked callable ID/digest, receiver mode,
  optional-role presence, nested path segment, or arm ordinal changes it; and
- removing a checked call/field/path join produces a typed first error and no
  partial transcript.
