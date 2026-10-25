# Unified text/Fx contract structural audit — 2026-07-12

Audit target: Jujutsu change `rrnvupruqzsu` over committed baseline
`3d3167ba` (`Finalize unified text, TextBox, View, and Fx design`). The Jujutsu
change ID is stable across final description/bookmark updates.

Canonical command:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/unified-text-fx-contracts-2026-07-12
```

Result: 1,231 Rust files / 615,563 physical Rust LOC, 0 errors, 144 warnings.
The CSV files in this directory are the exact current-checkout measurements;
they exclude target output and VCS internals.

## Structured audit evidence

- [File measurements](file_metrics.csv)
- [Dependency edges](dependency_edges.csv)
- [Public type name screening](public_type_duplicates.csv)
- [Structural findings](violations.md)
