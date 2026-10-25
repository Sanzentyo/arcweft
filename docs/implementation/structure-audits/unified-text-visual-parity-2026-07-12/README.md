# Unified text visual parity structural audit — 2026-07-12

Revision measured: Jujutsu change `wyslyrwowkrnunwmlxytxuzlltskxzvm`.

Canonical command:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/unified-text-visual-parity-2026-07-12
```

The audit scanned 2,643 files, including 1,256 Rust files and 622,283 physical
Rust LOC. It reports 0 errors and 131 warnings. This cut changes no Cargo
manifest or dependency edge. The warnings are existing size, facade,
embedded-test, and tracked architecture hotspots; the changed files do not add
an error-level hotspot.

## Structured audit evidence

- [File measurements](file_metrics.csv)
- [Dependency edges](dependency_edges.csv)
- [Public type name screening](public_type_duplicates.csv)
- [Structural findings](violations.md)
