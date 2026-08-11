# Superseded rejected-return material

**Non-normative provenance.** The normative replacement is
`RUST_SCHEMAS.md`, `ROLE_OWNER_ORDINAL_MATRIX.tsv`,
`TAIL_PRODUCER_OWNER_MATRIX.tsv`, and `TEST_MATRIX.tsv`.

## Rejected tail rows

The rejected return stated:

```text
ImplicitUnitTail    accepted owner = Expr only; ordinal == 0
MissingRequiredTail accepted owner = Expr only; ordinal == 0
```

That wording is superseded by:

```text
ImplicitUnitTail    accepted owner = Expr | Scope; ordinal == 0
MissingRequiredTail accepted owner = Expr | Scope; ordinal == 0
```

Producer selection is closed by the producer matrix, not by accepting any arbitrary
scope. Only retained predicate/proof body scopes and individual match-arm scopes use
the Scope variant for these roles.

## Rejected evidence claim

The rejected `T-ROLE-*` identity tests proved boolean predicates but were described as
if they proved semantic child order, recipe order, pattern preorder, and first-use
capture order. They remain structural tests only. The new `T-GEN-*` rows call the
production lowerers and transaction.

## Rejected liveness wording

The phrase `Retired with exact last-live/snapshot fields` is superseded. The exact
record is:

```rust
Retired {
    id: RawHirIdView,
    snapshot: HirSnapshotId,
    retired_at: HirRevision,
}
```

No alias is introduced.
