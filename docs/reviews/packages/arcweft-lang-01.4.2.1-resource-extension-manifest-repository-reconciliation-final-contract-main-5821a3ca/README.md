# Lang-01.4.2.1 final contract

This archive is the independently throwable, repository-reconciled final contract for the Arcweft resource extension-manifest wire boundary.

**Pinned repository:** `Sanzentyo/arcweft@5821a3ca479b5b89ca6ede997b9cf4f42f6280a6` (`main`, equal to the connector-visible `origin/main` at final pin)

**Result:** `OPEN_QUESTIONS=0`, `IMPLEMENTATION_READY=true`, `repository_contract_validation_succeeded=true`, `FINAL_CONTRACT=true`, `FALLBACK=false`.

Production code was not changed. The archive contains the contract, current repository inventory, exact wire schema and canonical examples, ownership/dependency plan, source-range and budget rules, package/bundle integration, test matrix, executed validation logs, and a standalone validator.

## Validate after extraction

```sh
python3 tools/validate_contract.py .
```

The validator uses the Python standard library. When `jsonschema` is installed it also checks the bundled Draft 2020-12 schema, but that optional check is not required for success because equivalent structural and semantic checks are built in.

## Reading order

1. `FINAL_CONTRACT.md`
2. `REPOSITORY_INVENTORY.md`
3. `WIRE_SCHEMA.md`
4. `DTO_AND_CONVERSION.md`
5. `DIAGNOSTICS_AND_LIMITS.md`
6. `OWNERSHIP_AND_DEPENDENCIES.md`
7. `PACKAGE_AND_ARTIFACT_PUBLICATION.md`
8. `TEST_MATRIX.md`
9. `VALIDATION.md`

`STATUS.json`, `PINNED_REVISION.json`, `REPOSITORY_EVIDENCE.json`, and `MANIFEST.sha256` are the machine-readable authority.
