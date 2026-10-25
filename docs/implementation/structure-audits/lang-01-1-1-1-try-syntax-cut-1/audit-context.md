# Lang-01.1.1.1 Try syntax Cut 1 structural audit

## Scope

- Jujutsu change: `uulltkts` on parent `579c2fd5`.
- Command: `cargo +nightly -Zscript tools/structure-audit.rs --root .`.
- Result: 3,492 files, 1,818 Rust files, 845,201 Rust physical LOC,
  94 package manifests, 0 errors, and 137 warnings.
- The generated full-workspace evidence is in `file_metrics.csv`,
  `dependency_edges.csv`, `public_type_duplicates.csv`, and `violations.md`.

## Structured audit evidence

- [File measurements](file_metrics.csv)
- [Dependency edges](dependency_edges.csv)
- [Public type name screening](public_type_duplicates.csv)
- [Structural findings](violations.md)
