# Proof syntax current-lineage lookup decision

## Recovery context

- Recovered into the Proof public-switch worktree: 2026-08-07
- Inspected Git revision:
  `f587e75750d9c5d9b6d8c84e0f098a4cfa80f68b`
- Working tree: dirty Proof public-switch integration
- Validation authority:
  [`2026-08-06-proof-concurrency-v6-1-1-full-matrix-closure.md`](2026-08-06-proof-concurrency-v6-1-1-full-matrix-closure.md)

`SyntaxDatabase::current(lineage)` reports errors in this order:

1. `WrongDatabase` for a foreign database identity;
2. the exact current snapshot for a registered whole-source lineage; and
3. `UnknownLineage { lineage }` for an unregistered lineage.

`WrongLineage` remains for operations which have a concrete receiver lineage
and are given a different actual lineage. `current` does not fabricate an
expected lineage or report equal identities as wrong. This is the sole typed
lookup result, not a wrapper, fallback, or second reader. Its current behavior
must be credited only through the Proof acceptance matrix.
