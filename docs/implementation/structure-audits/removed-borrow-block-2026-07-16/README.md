# Removed borrow-block structural audit

- Jujutsu change: `knxxvmxlvktv`
- Parent revision: `38a5ce0534bc6819ee6b9de1155558e57343c46f`
- Command: `cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/removed-borrow-block-2026-07-16`
- Result: 2,747 files, 1,306 Rust files, 640,768 physical Rust LOC,
  90 package manifests, 0 errors, and 126 pre-existing warnings.

## Structured audit evidence

- [File measurements](file_metrics.csv)
- [Dependency edges](dependency_edges.csv)
- [Public type name screening](public_type_duplicates.csv)
- [Structural findings](violations.md)
