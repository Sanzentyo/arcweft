# Predecessor precedence

| Authority | Verified identity | Status | Effect in this correction |
|---|---|---|---|
| Proof v6.1.1 typed-AST/proof/HIR/runtime identity package | SHA-256 `1b7de5f2c10a5b29d67c72011e4272df9a76af8907fd21fe162de54809fc69ef` | retained | predicate/proof block shape, omitted-tail anchors, typed arenas, transactions, scopes, and the 1,024 descendant limit remain authority |
| AW-AH-009.4.2 dialogue-content application package | SHA-256 `05e825dde033f308f24fc1f6e504b4c26bba2d61fd33852ce880dc666ba8f2a8` | retained higher authority for postfix candidates | source-backed postfix owner, shared-target exclusion, interpretation-specific role, root zero, per-kind preorder, and selected-key non-reuse remain unchanged |
| Proof v6.1.1.4.1 leaf/expression package | SHA-256 `61e2ee166bff158fe83dcf1484b7b9380a81f60d865377503400d27d238cc708` | retained where uncontradicted | final expression/child records, including source-backed block/closure/if roots and `HirMatchArm { scope, ... value }`, remain authority |
| Proof v6.1.1.4.1.1 source-owner correction | SHA-256 `2bcd3f78efb76442c2698a24251c4d874f7a941c5a8985649ea157100908a72e` | retained | eight typed owners, Type-owned elision, source query, liveness separation, limits, and final payloads remain authority |
| Rejected Proof v6.1.1.4.1.1.1 return | SHA-256 `a9603b3cc758d95dada69310f87a2dc26b7a2ce0ea8b6e0de39de4aa51e75024` | rejected as a whole | fingerprint transcript, tags, ordinal domains, constructor precedence, and representable non-tail role rows are retained inputs; Expr-only tail rows, generator-evidence claims, and `last-live` wording are superseded |
| This v6.1.1.4.1.1.1.1 correction | this archive | newest focused authority | `Expr | Scope` tail admission, exact producer mapping/allocation order, direct generator tests, and exact liveness payload rows |
| GitHub `main` | `5214a4836d5aa13a934ea8cb7037cc3a2a3c8e31` | implementation evidence | typed owner projection is landed; final key remains unimplemented pending this correction |

## Exact supersession

Only these result-changing statements are replaced:

1. `ImplicitUnitTail = Expr-only` becomes `Expr | Scope`, with producer selection
   fixed by `TAIL_PRODUCER_OWNER_MATRIX.tsv`.
2. `MissingRequiredTail = Expr-only` becomes `Expr | Scope`, with the same exact
   producer selection rule.
3. Identity-table rows are no longer claimed as production generator evidence;
   named `T-GEN-*` lowerer/transaction rows are required.
4. The obsolete `last-live` test wording is replaced by the exact `retired_at`
   payload.

No fingerprint byte, owner/role tag, source-ordered ordinal maximum, candidate role,
body/arm payload field, source-query API, or migration prohibition is changed.
