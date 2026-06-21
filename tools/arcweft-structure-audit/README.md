# arcweft-structure-audit

Std-only structural scanner for an Arcweft checkout. It does not modify source files.
The default is dry-run: it scans and prints a summary without writing reports.

```bash
cargo +nightly -Zscript tools/arcweft-structure-audit.rs --root .
cargo +nightly -Zscript tools/arcweft-structure-audit.rs \
  --root . --write target/structure-audit --fail-on-violations
```

Generated reports:

- `file_metrics.csv`
- `dependency_edges.csv`
- `public_type_duplicates.csv`
- `violations.md`

The Cargo parser intentionally supports the dependency forms used by Arcweft and is not a full TOML
implementation. Treat unknown/multiline dependency syntax as a reason to inspect the manifest manually.
