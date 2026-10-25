# Typed RichText built-in Fx structural audit — 2026-07-12

Audit subject: Jujutsu working change `pzllnvpo`, based on `31972dcf`
(`Align layer capture pixels and object identity`). The canonical audit command
scanned the current checkout after the built-in RichText Fx compiler was split
by responsibility.

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/unified-text-builtin-fx-compiler-2026-07-12
```

The checkout contains 1,249 Rust files and 615,551 physical Rust LOC. The audit
reports 0 errors and 128 pre-existing warnings.

## Structured audit evidence

- [File measurements](file_metrics.csv)
- [Dependency edges](dependency_edges.csv)
- [Public type name screening](public_type_duplicates.csv)
- [Structural findings](violations.md)
