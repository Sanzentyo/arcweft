# Constructor and transaction contract

## 1. Structural admission

`SyntheticRole::accepts_owner(kind, ordinal)` is pure, const, allocation-free, and total for every `HirIdKind` and every `u32`. Its complete truth table is the role matrix. It does not inspect an arena, infer an owner kind from a slot, consult a source node, or treat any integer as a sentinel.

`SyntheticKey::try_new` first evaluates the conjunction through `accepts_owner`. On failure it checks owner kind first, then ordinal. Therefore:

```text
wrong kind + wrong ordinal -> WrongOwnerKind
wrong kind + valid ordinal -> WrongOwnerKind
right kind + wrong ordinal -> InvalidOrdinal
right kind + valid ordinal -> Ok(SyntheticKey)
```

The retained error payloads are exact; no erased string message or generic invariant error replaces them.

## 2. Canonical source-ordered generation

The six variable roles accept only `0..=1_023` structurally. Their lowerers generate ordinals as follows:

- `RecoveryOperand`: use the semantic child-role ordinal declared by the exhaustive lowering schema. Optional absence does not allocate. This is not the index of an error vector.
- `DesugaredTemporary`: use a checked counter per `(owner, child HirIdKind)` after sorting source-causing tokens by source order and applying the fixed lowering-recipe step order. No map iteration participates.
- `DestructuredBinding`: use depth-first preorder, left-to-right/authored field order, with the whole binding before its nested pattern. The first valid or-pattern alternative fixes the shared ordinal map.
- `ClosureCapture`: allocate on first source-ordered use of a distinct outer `LocalId`; later uses reuse the capture.
- candidate roles: the candidate root expression is 0. Within each additional child `HirIdKind`, preorder starts at 0; within the Expr kind it starts at 1 because the root occupies 0. The shared target is excluded.

Every conversion from `usize` uses `u32::try_from`; every increment uses checked arithmetic; exceeding 1,023 fails before `SyntheticKey` construction and before transaction publication.

## 3. Liveness separation

A valid key proves only structural owner-kind and ordinal admission. The transaction then resolves `key.owner()`:

- a different database/module returns `IdResolveError::WrongModule`;
- an owner born after the target snapshot returns `NotYetLive`;
- an owner retired before the target snapshot returns `Retired`;
- an impossible private raw-kind disagreement returns `KindMismatch`; and
- an owner reserved earlier in the same transaction is accepted as staged.

This phase does not call `SyntheticRole::accepts_owner` again to infer liveness and does not probe an arbitrary slot to discover the owner kind.

## 4. Allocation key and reuse

The allocation ledger uses `(SyntheticKey, child HirIdKind)`. This preserves the accepted global typed arenas and candidate per-kind preorder without adding child kind to the public key schema.

- same live/reserved pair: return the same typed child ID;
- same key, different child kind: distinct typed child slot;
- changed owner, role, ordinal, database, module, or owner slot: distinct key;
- retired owner: no child survives into the new snapshot;
- rolled-back owner: no key can be observed.

## 5. Descendant limit and atomicity

`HirLimit::SyntheticDescendantsPerOwner.maximum()` remains 1,024. It counts live and newly staged child slots under the exact owner across all roles and child kinds in the target revision. Reused pairs count once.

The transaction preflights `current + fresh` with checked `usize` arithmetic. Exactly 1,024 commits. The 1,025th fresh child returns a typed limit error before reserving a slot or source row. The failure rolls back all changes in the enclosing lowering transaction, not merely the one child.

The structural ordinal maximum and aggregate transaction maximum intentionally coexist:

- ordinal 1,023 can be structurally valid;
- a structurally valid role/ordinal may still exceed the aggregate owner budget;
- ordinal 1,024 is never structurally valid, even when the owner has no other descendants.

## 6. Candidate-only admission

The generic key constructor accepts candidate roles for an `Expr` owner because it cannot and must not query expression payload/origin. The accepted postfix candidate transaction performs the additional semantic checks:

- owner is the source-backed postfix parent `ExprId`;
- role matches the interpretation being built;
- root is ordinal 0;
- shared target is not reallocated;
- selected committed lowering never reuses candidate-only keys.

Failure is an internal HIR invariant/transaction failure, not a removed-syntax or author-facing diagnostic. It commits no candidate, key, source row, or result.
