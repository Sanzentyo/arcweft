# Arcweft lang-01.3.1.2.3.2.1.2.1.1 — design-only final contract

This standalone package closes the catalog-digest role-root and construction-authority correction against requested baseline `175a74da637ca5f455abdefda49c6b62897b00e2`.

## Binding result

- `OPEN_QUESTIONS=0`.
- The operational catalog digest is **derived by Arcweft core**, never conferred by a producer assertion.
- One closed `RuntimeCatalogDigestRole` vocabulary determines every role ordinal, domain tag, cardinality rule, and whether that role may issue a construction capability.
- One admitted `RuntimeCatalogDigestRoleRoot` closes the complete role set and is owned by the same `AdmittedRuntimeGeneration` that pair-admits `AdmittedRuntimePlan` and `AwbcProgram`.
- Runtime-value construction is possible only through a non-Serde, generation-bound, role-scoped, producer-scoped `RuntimeConstructionAuthority` or a narrower typed façade derived from it.
- Raw plan/AWBC/save/replay/producer declarations remain quarantined assertions until canonical derivation and admission complete.
- No production source, patch, branch, implementation overlay, compatibility layer, source gate, fallback reader, or dual reader is included.

## Reading order

1. `FINAL_CONTRACT.md`
2. `DECISIONS.md`
3. `CANONICAL_DIGEST_GRAMMAR.md`
4. `API_AND_TYPE_SHAPES.md`
5. `ADMISSION_AND_CONSTRUCTION_STATE_MACHINE.md`
6. `SERDE_AND_WIRE_BOUNDARY.md`
7. `ERROR_MODEL.md`
8. `MIGRATION_AND_DELETION_PLAN.md`
9. `IMPLEMENTATION_ORDER.md`
10. `ACCEPTANCE_COMMANDS.md`
11. CSV/JSON evidence and traceability artifacts
12. `VERIFICATION_BOUNDARY.md`

The original request is copied byte-for-byte under `request/` and covered by `MANIFEST.sha256`.
