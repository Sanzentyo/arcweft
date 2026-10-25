# Unified text shaped-vertical regression structural audit

Audit scope: Jujutsu change `nyoynlov` over parent revision `37241449`
(`Unify player text in the prepared batch`). The canonical audit command was:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/unified-text-shaped-vertical-regression-2026-07-12
```

The audit scanned 2,636 repository files, including 1,255 Rust files and
621,347 physical Rust LOC. It reported 0 errors and 131 warnings. No Cargo
manifest, feature, workspace member, or dependency edge changed in this slice.

## Structured audit evidence

- [File measurements](file_metrics.csv)
- [Dependency edges](dependency_edges.csv)
- [Public type name screening](public_type_duplicates.csv)
- [Structural findings](violations.md)
