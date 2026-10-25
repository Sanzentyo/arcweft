# Structure Audit Error Exception - Function Type Effect Rows

Audit command:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs\implementation\structure-audits\function-stack-function-type-effect-rows-2026-07-09
```

Result:

- 1 error / 152 warnings.
- Error: `crates/arcweft-lang-sema/src/checker/expr.rs` has 2510 physical LOC, exceeding the production-file error threshold by 10 LOC.

Resolution:

- The literal/path/short-variant expression checker was split into
  `checker/expr/path.rs` in the follow-up recorded by
  `docs/implementation/function-stack-expr-path-split-2026-07-09.md`.

## Structured audit evidence

- [File measurements](file_metrics.csv)
- [Dependency edges](dependency_edges.csv)
- [Public type name screening](public_type_duplicates.csv)
- [Structural findings](violations.md)
