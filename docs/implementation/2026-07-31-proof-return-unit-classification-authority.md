# Proof return `Unit` classification authority

## Recovery context

- Recovered into the Proof public-switch worktree: 2026-08-07
- Inspected Git revision:
  `f587e75750d9c5d9b6d8c84e0f098a4cfa80f68b`
- Working tree: dirty Proof public-switch integration
- Current orchestration authority:
  [`2026-08-06-proof-public-switch-session-ownership-decision.md`](2026-08-06-proof-public-switch-session-ownership-decision.md)
- Validation authority:
  [`2026-08-06-proof-concurrency-v6-1-1-full-matrix-closure.md`](2026-08-06-proof-concurrency-v6-1-1-full-matrix-closure.md)

This note restores the semantic-classification decision only. Its former
pending/validated status and test output are not current evidence.

## Semantic authority

Proof omitted-tail behavior depends on the fully resolved return type:

- no authored return is implicit Unit and needs no semantic query;
- an authored return resolving to Unit, including through aliases, permits one
  clean `ImplicitUnitTail`;
- a resolved non-Unit return requires a tail and omission creates one poisoned
  `MissingRequiredTail`; and
- a poisoned/unresolved return also uses `MissingRequiredTail` and remains
  non-executable.

Structural `HirTypeKind::is_unit()` can recognize only the direct tuple shape
and is not semantic alias authority. HIR cannot depend on sema, while sema
cannot inspect a partially published module. The public switch therefore uses
one unpublished staged-header barrier:

```text
exact ParsedSource set
  -> staged module/item/signature identities
  -> immutable project header generation
  -> sole semantic symbol world classifies authored Proof returns
  -> same HIR transaction lowers bodies and source indexes
  -> all modules and the accepted project publish together
```

The classification is the closed
`Unit | NonUnit | Poisoned` value. Each fact is bound to the exact unpublished
project generation, item, return type, module/source lease set, and symbol-world
revision. Missing, duplicate, stale, foreign, wrong-item, or wrong-type facts
fail before tail allocation.

An authored tail remains source-backed and is checked against the same resolved
return type. Classification controls only the synthetic role selected when the
tail is omitted.

## Publication and deletion boundary

The staged transaction reserves all header identities, freezes one read-only
header snapshot, builds one symbol world, obtains the complete semantic fact
set, validates its generation stamp, lowers bodies/source rows, and publishes
all modules/project leases atomically. It supplies forward-reference and
module-order independence without a second HIR or symbol table.

The switch removes production Proof-tail use of structural `is_unit()`, source
freeze reclassification, clone/linked-HIR symbol construction, spelling-based
Unit guesses, and provisionally published modules completed later. The tuple
helper may remain only for structural inspection.

Current evidence must cover direct and aliased Unit, non-Unit aliases,
unknown/inaccessible/ambiguous/cyclic aliases, generation mismatch, input-order
determinism, tail-role substitution, exact/one-over rollback, and zero queries
for omitted return syntax. This note awards none of those rows PASS by itself.
