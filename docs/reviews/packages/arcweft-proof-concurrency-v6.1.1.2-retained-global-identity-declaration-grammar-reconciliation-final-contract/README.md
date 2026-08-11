# Retained global-identity declaration grammar reconciliation

Status: **READY_FOR_IMPLEMENTATION**  
Repository baseline: `3acc9cfec034d00cee173e41cbfb37cd46115c50` (`main`)  
Sequence: proof-concurrency v6.1.1.2  
Production changes in this design task: **none**

## Result in one sentence

Arcweft has eight retained global-identity families but only seven authored top-level declaration grammars: `character`, `view`, `action`, `activity`, `signal`, `metric`, and `layer`; `asset` is a build/catalog-derived identity family and deliberately has no source declaration, public AST item, or HIR item.

## Authority

This archive is a self-contained, latest-`main` restatement of the request at:

`docs/reviews/requests/2026-07-20-seq-proof-01.1.1.2-retained-global-identity-declaration-grammar-reconciliation.md`

It reconciles that request with the implemented private grammar recorded by:

- `docs/implementation/2026-07-20-proof-concurrency-v6-1-1-2-retained-global-identity-implementation.md`;
- `docs/implementation/2026-07-21-proof-public-switch-readiness.md`; and
- the current private parser, attachment, ID, HIR-identity, project-symbol, and bundle code at the pinned commit.

The repository records an earlier accepted archive with the same requested filename and SHA-256 `7be398ebe2cefa2daefa963c7c8c6efb0b2389bb015edf36e585fb8b770242b1`. This returned archive is a newly generated latest-`main` reconciliation, so it is not claimed to be byte-identical to that historical archive. Its own SHA-256 is published in the external sidecar.

## Reading order

1. `FINAL_CONTRACT.md` — binding decisions and invariants.
2. `FAMILY_GRAMMARS.md` — exact authored grammar, including the absence of source `asset`.
3. `IDENTITY_VISIBILITY_REFERENCES.md` — names, IDs, visibility, references, collisions, and aliases.
4. `PRIVATE_GRAMMAR_NODES.md` and `BODY_OWNERSHIP.md` — lossless nodes and typed child ownership.
5. `RECOVERY_AMBIGUITY_LIMITS.md` — deterministic recovery, poison, synchronization, and budgets.
6. `PUBLIC_AST_HIR_MIGRATION.md` — final attached AST and arena-HIR ownership.
7. `IMPLEMENTATION_PLAN.md` and `MIGRATION_AND_DELETION.md` — exact compiling order and deletion inventory.
8. `TEST_MATRIX.md`, `VERIFICATION_PLAN.md`, and `STRUCTURE_PLAN.md` — direct acceptance evidence required from implementation.
9. `REPOSITORY_EVIDENCE.md` and `REQUIREMENTS_TRACEABILITY.md` — production reconciliation and request coverage.

## Normative language

`MUST`, `MUST NOT`, `SHALL`, and `SHALL NOT` are binding. Examples do not add syntax beyond the productions and invariants stated in this archive. A lower layer may reject a syntactically typed construct for a semantic reason, but it must consume the typed syntax node rather than reparse text.

## Package integrity

`MANIFEST.txt` lists all eighteen archive members in lexical order. Every ordinary entry carries the lowercase SHA-256 of its exact bytes. The manifest self-entry uses sixty-four zeroes; this avoids a recursive hash while keeping membership explicit.
