# Final tail-owner and generator-evidence correction

## 1. Authority and scope

This archive supersedes only the affected owner/test/liveness rows of the rejected
Proof v6.1.1.4.1.1.1 return. It retains the final eight typed owners,
database-qualified IDs, all 21 role names and tags, the sole typed source query,
accepted body/arm payloads, AW-AH-009.4.2 candidate semantics, the 51-byte
fingerprint transcript, and deletion-driven migration.

## 2. Final tail-owner rule

Both tail roles have the same closed structural owner set and exact-zero ordinal:

```text
ImplicitUnitTail    owner kind in { Expr, Scope } and ordinal == 0
MissingRequiredTail owner kind in { Expr, Scope } and ordinal == 0
```

The producer, not the role constructor, selects the typed variant:

- a source-backed expression container owns its tail through
  `SyntheticOwner::Expr(container_expr_id)`;
- a predicate/proof block without a source-backed container expression owns its
  tail through `SyntheticOwner::Scope(body_scope_id)`;
- a missing match-arm value is owned through
  `SyntheticOwner::Scope(match_arm.scope)`.

No role admits `Syntax`, a raw ID, `Item` as a body surrogate, or the tail `ExprId`
being allocated. No match-arm ID is invented. The complete producer mapping and
anchors are normative in `TAIL_PRODUCER_OWNER_MATRIX.tsv`.

## 3. Uniqueness and non-circular allocation

The lowering transaction reserves the source-backed root `ExprId` or existing body/
arm `ScopeId` before lowering or synthesizing the tail. It then constructs the
exact-zero key and requests child kind `HirIdKind::Expr`.

Consequences:

- a synthetic tail never owns itself;
- two ordinary containers differ by root `ExprId`;
- predicate/proof bodies differ by body `ScopeId`;
- two match arms differ by arm `ScopeId`, even under one match expression;
- `(SyntheticKey, HirIdKind::Expr)` is unique for one producer and is reused on
  repeated requests rather than double-allocated; and
- failed parent or child lowering rolls back the owner reservation, child slot,
  insertion source row, poison, diagnostic, and descendant count together.

## 4. Source insertion ownership

The synthetic child is an `ExprId` and its insertion is stored in the final Expr
source index. `SyntheticOwner::Scope` changes allocation identity only; it does not
create a second scope-source reader.

- block-like omitted tails use the opening byte of the authored close brace;
- a recovered missing close brace uses that delimiter's checked insertion anchor;
- a missing match-arm value uses the zero-width insertion immediately after the
  arm's fat arrow, or the parser-owned missing-value insertion when the arrow is
  recovered;
- omitted `else` uses the end of the then branch;
- closure/if-let required children use their attached typed `Body`/branch component
  insertion; and
- no source slice, vector index, display spelling, or reparsing reconstructs an
  anchor.

## 5. Production ordinal evidence

Identity truth-table tests remain necessary but are not production-order evidence.
`GENERATOR_EVIDENCE_CONTRACT.md` and the `T-GEN-*` rows require the real attached
lowerer and mutable HIR transaction to emit and commit the inspected keys.

The direct matrix proves:

- declared semantic child-role order and optional absence for `RecoveryOperand`;
- source-token order followed by immutable recipe-step order for
  `DesugaredTemporary`;
- depth-first authored pattern preorder and first-alternative sharing for
  `DestructuredBinding`;
- first source-use capture allocation and later-use reuse for `ClosureCapture`;
- root zero, shared-target exclusion, per-child-kind preorder, interpretation
  separation, and selected-key non-reuse for both candidate roles; and
- exact 1,024 / one-over rollback without wrapping or partial publication.

## 6. Exact liveness vocabulary

The transaction uses the retained exact records:

```rust
IdResolveError::NotYetLive {
    id: RawHirIdView,
    snapshot: HirSnapshotId,
    born: HirRevision,
}
IdResolveError::Retired {
    id: RawHirIdView,
    snapshot: HirSnapshotId,
    retired_at: HirRevision,
}
```

`last_live` is not a field or alias. Structural construction performs no liveness
lookup. The transaction resolves `WrongModule`, `NotYetLive`, `Retired`, and the
private `KindMismatch` invariant before staging a fresh child or source row.

## 7. Readiness

Every current role has a total arbitrary-`u32` predicate, every tail producer has
an existing typed owner allocated before the child, exact-zero keys are unique,
every variable generator has direct lowerer/transaction tests, liveness payloads are
exact, and the retained fingerprint vectors recompute. `OPEN_QUESTIONS.md` is
exactly `none`; this correction is `READY_FOR_IMPLEMENTATION`.
