# Arcweft Proof-concurrency v6.1.1.4.1 corrected final contract

This archive is the complete standalone replacement for the rejected Proof-concurrency v6.1.1.4 return. It is a design-only package: it contains no production Rust edit, patch, overlay, branch, or compatibility layer.

## Baseline and status

- Repository: `Sanzentyo/arcweft`
- GitHub `main` inspected at: `ac9ce44fe9423efd85280e26832dd30c725b3b34`
- Package status: `READY_FOR_IMPLEMENTATION`
- Open questions: `none`
- Required public switch: one deletion-driven compiling switch; no aliases, wrappers, dual readers, reparsing, source gates, CSS/Takumi path, or removed-syntax-specific diagnostics.

## Normative order inside this archive

1. `FINAL_CONTRACT.md`
2. `RUST_SCHEMAS.md`
3. `SOURCE_ROLE_AND_QUERY_CONTRACT.md`
4. `LITERAL_NUMERIC_CONTRACT.md`
5. `PATH_LIFETIME_CALL_THREAD_CONTRACT.md`
6. `DIALOGUE_RICHTEXT_CONTRACT.md`
7. `LOWERING_MATRIX.tsv`
8. `TEST_MATRIX.tsv`
9. `IMPLEMENTATION_AND_DELETION_ORDER.md`
10. `PREDECESSOR_PRECEDENCE.md`
11. `REQUIREMENTS_TRACEABILITY.tsv`

`PRIMARY_REQUEST_COPY.md` and `CORRECTION_REQUEST_COPY.md` are complete request copies. `REPOSITORY_EVIDENCE.md` records the exact evidence and verification depth. `MANIFEST.json` covers every other member; it intentionally omits itself because a self-hash cannot be simultaneously embedded and verified.

## Completion rule

Every result-changing choice called out by the correction request is fixed here. The implementation is not permitted to substitute a different schema, default-on-error behavior, fallback reader, textual reconstruction, or alternate resolver. Any contradiction discovered during implementation must stop the affected public switch and be adjudicated as a new correction; it is not an implementation option left by this contract.
