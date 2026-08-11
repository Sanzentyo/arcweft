# Final correction

## 1. Scope

This contract supersedes only the incomplete role-admission and fingerprint clauses in Proof v6.1.1.4.1.1. Every uncontradicted decision from that package remains normative, including the eight typed owner variants, database-qualified IDs, the sole typed source query, TypeId-owned elided regions, the 35-expression inventory, Dialogue/RichText ownership, and deletion-driven migration.

## 2. Closed owner policy

Every current role has a closed typed-owner set. Four former syntax owners are replaced directly:

- `ImplicitUnitTail` -> the requiring block-like `ExprId`;
- `PredicateBoolReturn` -> the predicate `ItemId`;
- `ProofUnitReturn` -> the proof `ItemId`; and
- `MissingRequiredTail` -> the requiring `ExprId`.

Exactly four roles admit more than one owner kind because the accepted language has both expression and statement forms: `RecoveryOperand`, `DesugaredTemporary`, `IfLetScrutinee`, and `MatchScrutinee` admit `Expr | Stmt`. Every other role admits exactly one owner kind. No current role admits `Local` or `Capture`.

The complete table is `ROLE_OWNER_ORDINAL_MATRIX.tsv`; no prose-only inheritance remains.

## 3. Closed ordinal policy

There are exactly two structural ordinal domains:

```text
ExactZero       ordinal == 0
SourceOrdered   0 <= ordinal <= 1_023
```

The six source-ordered roles are `RecoveryOperand`, `DesugaredTemporary`, `DestructuredBinding`, `ClosureCapture`, `PostfixIndexCandidateExpression`, and `DialogueContentCandidateExpression`. All other roles are exact-zero. `1_023` is the inclusive structural maximum, aligned with the accepted 1,024 descendants-per-owner transaction budget. `1_024` and `u32::MAX` are invalid; no sentinel or wrapping behavior exists.

Structural admission and transaction accounting are independent. A key with ordinal 1,023 can be structurally valid while a transaction still rejects it because the same owner already has 1,024 live/staged synthetic descendants across roles and child kinds.

## 4. Constructor result and precedence

`SyntheticRole::accepts_owner(kind, ordinal)` is exactly the conjunction of the role's owner-kind and ordinal predicates.

`SyntheticKey::try_new` uses deterministic precedence:

1. if the owner kind is not admitted, return `WrongOwnerKind`, even when the ordinal is also invalid;
2. otherwise, if the ordinal is invalid, return `InvalidOrdinal`;
3. otherwise construct the key.

The error variants and fields from v6.1.1.4.1.1 are retained. The constructor performs no module, snapshot, source, arena, or staged-owner lookup.

## 5. Liveness, staging, and rollback

The owning HIR transaction resolves a structurally valid typed owner. Resolution remains database/module qualified and preserves the accepted order: `WrongModule`, `NotYetLive`, `Retired`, then `KindMismatch` where an internal typed/raw invariant is being checked. A typed owner already reserved in the same transaction is usable only after its reservation is present.

The transaction counts fresh target-revision synthetic child slots for the exact `SyntheticOwner` across every role and produced `HirIdKind`. Reusing the same `(SyntheticKey, child HirIdKind)` retains the existing/reserved ID and counts once. The 1,025th fresh descendant returns `HirLowerError::Limit` for `HirLimit::SyntheticDescendantsPerOwner` before publication. Any failure rolls back the key, HIR slot, source component, scope/local/capture side effects, diagnostics, candidates, counters, and retained-result records.

## 6. Candidate-only ownership

Both candidate roles structurally accept only `Expr` owners. The AW-AH-009.4.2 candidate transaction additionally requires that owner to be the source-backed postfix parent expression.

- ordinal 0 is the interpretation root;
- the shared target is excluded and retains its source-backed `ExprId`;
- further candidate-only children use checked deterministic preorder;
- the two interpretations are separated by their distinct role tags; and
- a candidate key is never repurposed as the selected committed expression key.

A selected interpretation is lowered through the ordinary committed path. Candidate-only IDs are retained only in the accepted unresolved/ambiguous tooling product or are rolled back.

## 7. Fingerprint input

`arcweft_lang_hir::identity` owns `SyntheticKeyFingerprintInput`, an opaque 51-byte transcript returned by `SyntheticKey::fingerprint_input()`. It contains:

```text
"arcweft-hir-synthetic-key-v1\0"
owner tag
process-local database ID (u64 little-endian)
module slot (u32 little-endian)
HIR slot (u32 little-endian)
role tag
ordinal (u32 little-endian)
```

Every owner and role tag is explicitly allocated in `FINGERPRINT_TRANSCRIPT.md`. Tags are not Rust discriminants and `std::hash::Hash` output is never used as bytes. The identity layer emits the canonical session-qualified transcript and owns no digest algorithm. A higher accepted fingerprint owner may hash these bytes in its own domain-separated transcript; a persistent artifact must not treat this process-local transcript as a cross-session identity by itself.

## 8. Readiness

Every current role owner, arbitrary ordinal, error precedence, liveness boundary, candidate rule, fingerprint byte, and required test is closed. `OPEN_QUESTIONS.md` is exactly `none`; this focused correction is `READY_FOR_IMPLEMENTATION`.
