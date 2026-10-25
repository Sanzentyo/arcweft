# Proof raw-source ID materializer deletion structural audit

Date: 2026-07-26

Repository state: Jujutsu change `wkotzwtz` / working-copy commit `c387b9da`,
based on `e7656f71` (`Derive typed Proof runtime assertion guards`).

The canonical structural audit scanned 3,700 files, including 1,939 Rust files
and 906,506 physical Rust lines. Cargo metadata covered 95 workspace package
manifests. It reported 0 errors and 146 pre-existing warnings. This deletion
does not add a crate, dependency edge, public compatibility surface, duplicate
identity payload, or source gate.

## Deleted-file measurements

- `crates/arcweft-lang-hir/src/id_context.rs`: 8,040 bytes, 233 physical LOC, 21 embedded test LOC.
- `crates/arcweft-tooling/src/id_context.rs`: 1,462 bytes, 46 physical LOC.

## Structured audit evidence

- [File measurements](file_metrics.csv)
- [Dependency edges](dependency_edges.csv)
- [Public type name screening](public_type_duplicates.csv)
- [Structural findings](violations.md)
