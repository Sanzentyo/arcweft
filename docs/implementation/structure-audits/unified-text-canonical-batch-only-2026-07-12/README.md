# Unified text canonical prepared-batch structural audit

Audit checkout: Jujutsu working change `ruvumkxk`, based on revision
`309e5aeba99d` (`Remove legacy dialogue renderer staging`). The working commit
ID is content-dependent; the change ID is the stable audit identifier.

Canonical command:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/unified-text-canonical-batch-only-2026-07-12
```

Result: 2,635 files scanned, 1,254 Rust files, 620,421 physical Rust LOC,
91 package manifests, 0 errors, and 131 warnings. Generated, vendored,
`target/`, and VCS content are excluded. The CSV files and `violations.md` are
the exact checkout evidence.

Deleted paths recorded as absent from current-size measurements: `crates/arcweft-player-native/src/font_family.rs`, `crates/arcweft-player-native/src/font_system.rs`, and `crates/arcweft-render-wgpu/src/renderer/tests.rs`.

## Structured audit evidence

- [File measurements](file_metrics.csv)
- [Dependency edges](dependency_edges.csv)
- [Public type name screening](public_type_duplicates.csv)
- [Structural findings](violations.md)
