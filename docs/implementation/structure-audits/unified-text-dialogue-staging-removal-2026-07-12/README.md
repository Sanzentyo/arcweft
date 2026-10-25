# Unified text: legacy dialogue staging removal structural audit

Audit target: Jujutsu change `xnrroynx` over parent revision `c3cbba0a`.
Canonical command:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/unified-text-dialogue-staging-removal-2026-07-12
```

The audit scanned 1,256 Rust files / 621,678 physical Rust LOC and found 0
errors / 133 warning-level findings. The generated `dependency_edges.csv`,
`file_metrics.csv`, `public_type_duplicates.csv`, and `violations.md` are the
exact checkout evidence.

## Structured audit evidence

- [File measurements](file_metrics.csv)
- [Dependency edges](dependency_edges.csv)
- [Public type name screening](public_type_duplicates.csv)
- [Structural findings](violations.md)
