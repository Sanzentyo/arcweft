# Production generator evidence contract

## 1. What counts as evidence

A role-table unit test proves only structural admission. A production generator test
must call the real attached-syntax HIR lowering entry point (or the exact candidate/
closure/pattern lowering context used by it), commit or roll back through the real
HIR transaction, and inspect emitted `SyntheticKey`, child typed ID, source site,
transaction counters, and retained payload. Test-only direct key construction cannot
satisfy a `T-GEN-*` row.

Every test runs a perturbation that changes map insertion, diagnostic collection, or
work-queue order without changing attached source order. The resulting keys and typed
IDs must remain identical.

## 2. RecoveryOperand

Representative attached source fixtures:

```arcw
-
lhs +
[item; ]
start..
```

The direct lowerer assertions are:

- missing unary operand -> semantic operand role ordinal `0`;
- missing binary right operand -> declared right-child ordinal `1`;
- missing array-repeat count -> declared count ordinal `1`;
- attached statement `let value =` missing initializer -> statement owner and initializer ordinal `0`;
- omitted optional range end -> no `RecoveryOperand` key;
- reversing diagnostic storage or unrelated sibling-lowering order does not change
  any key; and
- a generated attached child-role sequence with ordinals `0..=1_023` commits through
  the production cursor/transaction, while the next checked increment rolls back the
  entire fixture before publication.

The expected allocation tuples are `(Expr(unary_root), RecoveryOperand, 0, Expr)`,
`(Expr(binary_root), RecoveryOperand, 1, Expr)`,
`(Expr(repeat_root), RecoveryOperand, 1, Expr)`, and
`(Stmt(let_stmt), RecoveryOperand, 0, Expr)`. The lowerer uses its closed child-role
enum/match. It does not derive ordinals from a `Vec` index after recovery.

## 3. DesugaredTemporary

The direct expression-owned attached HIR plan is bound to source
`input |> normalize |> finish`; its source-causing ranges are exactly `6..8` and
`19..21`. The plan uses the real production recipe descriptor type with this exact
temporary-producing step inventory:

```text
event 0 @ 6..8:  [step[0], step[1]]
event 1 @ 19..21: [step[0], step[1], step[2]]
```

The bracketed values are indices into the owning production recipe descriptor's
immutable temporary-producing step slice; they introduce no new public type or codec.
Expected emitted keys for the Expr owner are ordinals `0, 1, 2, 3, 4`, in the exact
(event range, listed step) order above. Each key's insertion anchor is the end of its
event token.

The same production cursor is invoked through a statement-owned attached HIR plan
with source-causing ranges and steps:

```text
event 0 @ 4..6:   [step[0], step[1]]
event 1 @ 12..14: [step[0]]
```

Expected Stmt-owned ordinals are `0, 1, 2`. The tests record exact event ranges, step
identities, owner variant, emitted keys, child kinds, and source anchors. They rerun
after reversing recipe lookup-map insertion and the work queue; the outputs remain
identical because events are sorted by attached source range and steps are consumed
in the descriptor's immutable slice order.

A production cursor sequence of exactly 1,024 temporary-producing steps emits
ordinals `0..=1_023`; the 1,025th checked request fails before key construction or
transaction publication and rolls back every staged temporary, source row, and
count. The immutable recipe behavior stays on the original recipe enum/descriptor or
its lowering context; no free-standing string-name helper or extension trait is
introduced.

## 4. DestructuredBinding

The attached pattern fixture has this typed shape and authored order:

```text
Tuple(
  Binding(a),
  Record(
    left: Binding(b),
    right: Tuple(Binding(c), Binding(d)),
    rest: PatternRest,
  ),
  Whole(binding=e, nested=Tuple(Binding(f), Binding(g))),
)
```

Expected `DestructuredBinding` ordinals are `a=0, b=1, c=2, d=3, e=4, f=5,
g=6`; `PatternRest` uses its separate exact-zero role. A paired or-pattern fixture
has two alternatives with the same binding positions: the first accepted alternative
creates the map and the second reuses those ordinals. Field lookup-map insertion is
permuted while attached authored order remains fixed; keys do not change. A mismatch
that would append a new binding in a later alternative poisons/rejects before any
partial local/key publication.

A production preorder fixture with 1,024 bindings commits; the next binding fails
checked conversion/increment and rolls back pattern children, locals, source rows,
and counts.

## 5. ClosureCapture

The attached closure fixture has outer locals whose declaration/map order differs
from use order, with source-ordered uses:

```text
beta, alpha, beta, gamma
```

Expected allocations are `beta -> ordinal 0`, `alpha -> 1`, repeated `beta` reuses
the first `CaptureId` without a charge, and `gamma -> 2`. Reversing the capture-map
insertion or lookup-map implementation does not change IDs or order. Exactly 1,024
distinct first uses commit; the 1,025th fails before staging and rolls back the
closure environment, captures, source rows, and descendant counter.

## 6. Postfix candidate roles

The direct ambiguous postfix fixture is one source-backed `target[payload]` node with
a shared target and independently viable index and Dialogue interpretations. Each
interpretation runs its production candidate builder:

- root candidate Expr -> ordinal `0` under its own role;
- shared target -> no candidate key and original source-backed ExprId retained;
- further Expr children -> `1, 2, ...` preorder;
- Pattern and Stmt children -> independent `0, 1, ...` preorder per child kind;
- index and Dialogue trees differ by role even with equal ordinals;
- selected committed lowering does not reuse a candidate key; and
- reversing candidate work queues or maps does not change canonical preorder.

Exactly 1,024 fresh `(key, child kind)` pairs for one candidate owner commit in the
bounded transaction fixture. The 1,025th fails before candidate/result publication
and rolls back both interpretation products when they share one enclosing ambiguous
postfix transaction.

## 7. Test-row closure

`TEST_MATRIX.tsv` names separate structural `T-ROLE-*` tests and direct production
`T-GEN-*` tests. Mechanical validation requires at least one direct lowerer test,
one perturbation test, and one exact/one-over transaction test for each of the six
source-ordered roles. Identity tests are never counted toward that requirement.
