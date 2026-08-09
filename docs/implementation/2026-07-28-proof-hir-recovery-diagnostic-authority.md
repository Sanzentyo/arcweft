# Proof final-HIR recovery-diagnostic authority

## Recovery context

- Recovered into the Proof public-switch worktree: 2026-08-07
- Inspected Git revision:
  `f587e75750d9c5d9b6d8c84e0f098a4cfa80f68b`
- Working tree: dirty Proof public-switch integration
- Validation authority:
  [`2026-08-06-proof-concurrency-v6-1-1-full-matrix-closure.md`](2026-08-06-proof-concurrency-v6-1-1-full-matrix-closure.md)

This note restores the accepted diagnostic-ownership decision only. No prior
PASS state, test count, structural result, or repository revision is inherited.

## Terminal recovery-event owner

Semantic poison and diagnostic ownership are related but not bijective. A
missing synthetic child may poison both itself and its retained parent while
representing one recovery event and therefore one diagnostic.

Module freeze derives the exact obligations from the immutable slot proposal:

- a poisoned synthetic child with role `RecoveryOperand` or
  `MissingRequiredTail` is the terminal owner;
- the matching poisoned parent is propagation state for that same event and
  must not create a duplicate diagnostic;
- unrelated parent-owned recovery is not erased by a later missing child;
- distinct recovery children remain distinct terminal events;
- every other poisoned source-backed owner owns its own terminal event; and
- clean, foreign, retired, duplicate, missing, or extra propagated-parent
  diagnostic owners reject publication.

Arena payloads remain the semantic-poison authority. Freeze first requires
payload poison to equal slot poison for every arena family; callers do not
supply an independent poison bit.

## Typed primary source

`HirRecoveryDiagnostic` retains a typed primary descriptor and the exact
`HirSourceSite` selected during lowering. Freeze resolves the descriptor
through the same frozen source index and requires exact site equality.

| Terminal owner | Required primary |
| --- | --- |
| synthetic Expr, Pattern, or Type recovery child | its typed `Whole` insertion query |
| generic source-backed Expr, Pattern, or Type Error | that family's `Recovery` query |
| recognized Expr, Pattern, Type, or Stmt family | the exact applicable component which caused poison; `Recovery` is not a fallback |
| Item or Local | typed `OwnerWhole` for its qualified owner |

`OwnerWhole` resolves immutable slot metadata; it is not a raw range or second
reader. Scope and Capture payloads are clean by construction. Resolution order
is exact owner, role/ordinal applicability, frozen requirement/presence, then
exact retained source site.

## Publication and limits

The immutable module constructor is the sole freeze authority for payload/slot
poison equality, parser-diagnostic order, deterministic recovery-diagnostic
order, exact terminal-owner coverage, and primary-source identity. The shared
diagnostic limit accepts 1,024 and rejects 1,025 before slot-ledger or module
publication. Identity, limit, transaction, and publication failures are fatal
and never become recoverable diagnostics.

## Deletion boundary

The public switch removes obsolete diagnostic readers and owner-kind source
guessing. It does not add message strings, copied ranges, reparsing, parallel
maps, compatibility wrappers, dual readers, source gates, or diagnostics for
removed syntax.
