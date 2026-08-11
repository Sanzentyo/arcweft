# Arcweft Proof-concurrency v6.1.1.4.1.1 consistency correction

This archive is the standalone, design-only correction required after repository adjudication of the retained Proof v6.1.1.4.1 package. It contains no production Rust, Cargo edit, patch, overlay, branch, PR, or compatibility layer.

## Baseline and status

- Repository: `Sanzentyo/arcweft`
- Latest `main` inspected and ancestry-checked: `5018912852a45e96f48735767021bf858ffcd493`
- Retained parent ZIP SHA-256: `61e2ee166bff158fe83dcf1484b7b9380a81f60d865377503400d27d238cc708`
- Status: `READY_FOR_IMPLEMENTATION`
- `OPEN_QUESTIONS.md`: exactly `none`

## Closed contradictions

1. One typed source query now covers `ExprId`, `PatternId`, and `TypeId`; the old `expr_source_site` API is deleted in the same public switch.
2. Pathless variant patterns retain an explicit unqualified-head payload instead of fabricating an empty `HirPath`.
3. Duration structural identity includes the authored unit, while a separate semantic-value type defines unit-insensitive equality, order, cache, and checked fingerprint behavior.
4. Float width overflow and Duration runtime-range overflow are checker-owned rejection records; the conflicting HIR issue variants are deleted.
5. `SyntheticOwner::Type(TypeId)` is the exact owner for elided regions; `SyntheticKey` no longer stores or exposes an untyped raw owner.
6. Every source/leaf/path/registry hard limit has one numeric owner and accounting phase.
7. The full 82-row lowering matrix and complete test matrix are corrected, and all 164 formerly missing `T-Q-*`/`T-RB-*` family references are defined in `SUBTEST_REGISTRY.tsv`.

## Normative order

1. `FINAL_CORRECTION.md`
2. `RUST_SCHEMAS.md`
3. `SOURCE_ROLE_AND_QUERY_CONTRACT.md`
4. `PATH_LIFETIME_CALL_THREAD_CONTRACT.md`
5. `LITERAL_NUMERIC_CONTRACT.md`
6. `LIMITS_AND_ACCOUNTING.md`
7. `LOWERING_MATRIX.tsv`
8. `TEST_MATRIX.tsv`
9. `SUBTEST_REGISTRY.tsv`
10. `IMPLEMENTATION_AND_DELETION_ORDER.md`
11. `PREDECESSOR_PRECEDENCE.md`
12. `REQUIREMENTS_TRACEABILITY.tsv`

`REQUEST_COPY.md`, `PARENT_CORRECTION_REQUEST_COPY.md`, and `PRIMARY_REQUEST_COPY.md` are complete request copies. `PARENT_SUPERSEDED_SCHEMA_AND_ROWS.md` preserves the directly adjudicated parent material as historical, non-normative evidence; implementers use the corrected complete files above and do not compare archives manually.

`MANIFEST.txt` intentionally omits itself. Every other member has an exact byte length and SHA-256.
