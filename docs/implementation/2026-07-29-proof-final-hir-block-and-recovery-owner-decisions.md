# Proof final-HIR block and recovery owner decisions

## Recovery context

- Recovered into the Proof public-switch worktree: 2026-08-07
- Inspected Git revision:
  `f587e75750d9c5d9b6d8c84e0f098a4cfa80f68b`
- Working tree: dirty Proof public-switch integration
- Validation authority:
  [`2026-08-06-proof-concurrency-v6-1-1-full-matrix-closure.md`](2026-08-06-proof-concurrency-v6-1-1-full-matrix-closure.md)

This note restores owner decisions only. Historical implementation progress,
test outcomes, and repository identifiers are not acceptance evidence here.

## Value blocks and ordinary functions

Ordinary Function bodies are block-only:

```text
HirFunctionBody::Block { scope, statements, tail }
HirFunctionBody::Error(recovery_expr)
```

There is no expression-bodied Function carrier. Predicate and Proof keep their
separately accepted body alternatives.

An authored value-block tail owns its source-backed `ExprId`. An omitted
ordinary Function tail allocates one clean Unit at
`SyntheticKey(Scope(body_scope), ImplicitUnitTail, 0)` after the body scope is
allocated. Return-type compatibility is semantic analysis, not a reason to
change that structural payload. Only a missing Function body uses the Error
alternative and its typed missing-required-tail recovery child.

Proof omitted-tail selection is governed separately by the semantic Unit
classification in
[`2026-07-31-proof-return-unit-classification-authority.md`](2026-07-31-proof-return-unit-classification-authority.md).

## Statement blocks and Thread

Statement-owned blocks contain one scope and source-ordered `StmtId`s. They
never own a value tail, implicit Unit, missing-required-tail child, or omitted
tail syntax. A final expression in statement context is an
ExpressionStatement. This applies to LetElse, DeferBlock, On, UnsafeLifetime,
statement If/IfLet branches, loops, and statement Match block arms.

`HirThreadExpr` and `HirThreadBody` solely own Thread name, mode, body scope,
and ordered Flow items. A retained Thread statement is only the typed
execution-context reference `HirStmtKind::Thread { thread: ExprId }`; the
referenced expression is same-module and owns its body scope. A second
statement-owned Thread body is deleted.

## Callable scopes and Match arms

View, Action, TraitFunction, and CapabilityFunction each own one Callable
scope, not one scope per parameter. View/Action scopes are item-owned; member
callable scopes retain the enclosing item owner plus their distinct attached
member allocation identity. Parameters, defaults, bounds, return types,
values/bodies, and effects all refer to that one scope.

Match does not introduce one container-wide Block scope. It remains the
semantic and transaction owner of one distinct lexical scope boundary per
arm. An expression Match creates one MatchArm scope per arm; an authored
BlockExpr value creates its own nested Block scope below that arm. In ordinary
statement context, one MatchArm scope is shared by the arm pattern, guard, and
typed Expression or statement-Block body. Block arms have no tail, and
expression arms are not wrapped in a synthetic statement. A missing arm result
uses one typed missing-required-tail expression under the arm scope.

Every ordinary MatchArm scope has the inherited outer scope as its lexical
parent and the source-backed Match `ExprId` or `StmtId` as its typed owner. It
is not a child of a Match statement scope, because no Match-container ScopeId
exists. Sibling arms therefore share a semantic Match owner without sharing a
lexical scope.

This does not make Match ownerless. The source-backed Match `ExprId` or
`StmtId` remains the semantic and transaction owner of the scrutinee and every
arm, while the scrutinee is evaluated in the inherited outer lexical scope.
Each arm boundary owns only its pattern bindings, guard, and selected value or
body. If a later lowering stage must materialize a HIR scrutinee temporary, the
dedicated exact-zero `MatchScrutinee` role belongs to that Match ID; an ordinary
runtime-local value used to evaluate the retained scrutinee once does not by
itself require a fabricated HIR node or a Match-level Block scope.

Thread context uses the accepted nested-Flow body owner instead: each braced
statement Match arm has one Block scope which is also that arm's
`HirThreadBodyOwner::NestedScope`. It does not create a parallel MatchArm scope
above or below the Block. That single Block is the arm scope. In every context,
sibling arm scopes remain distinct and the Match delimiter itself is not a
lexical Block owner.

## Known-family recovery and deletion

Recognized items and statements retain their typed family with Clean or typed
Poisoned state. Required malformed children remain typed recovery IDs, body
error alternatives, or state-consistent absent fields. Generic Error is only
for unclassified syntax or transactional child failure.

`recovered_family` side data, routes which demote a recognized family to Error,
and recoverable `LimitExceeded` states remain deleted. Hard limits roll back.
`UnsafeAuditInsertion` exists only for a clean, fully delimited unsafe
statement.

Current acceptance must cover source-backed and synthetic owner validation,
source order, known-family poison, stale/foreign rejection, exact/one-over
rollback, and compile-fallout deletion through the full matrix. Source spelling
scans and this historical decision note do not provide PASS evidence.
