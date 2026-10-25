# Unified Text / View definition structural audit

Audit point: Jujutsu working change `tqkzspus`, parent `dc2cd458`.

Canonical command:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/unified-text-view-definitions-2026-07-12
```

The checkout contains 1,250 Rust files / 627,602 physical Rust LOC. The audit
reports 0 errors and 143 pre-existing or tracked warnings. No Cargo dependency,
feature, facade-crate, or crate-direction edge changed in this slice; the full
edge inventory is in `dependency_edges.csv`.

## Structured audit evidence

- [File measurements](file_metrics.csv)
- [Dependency edges](dependency_edges.csv)
- [Public type name screening](public_type_duplicates.csv)
- [Structural findings](violations.md)
