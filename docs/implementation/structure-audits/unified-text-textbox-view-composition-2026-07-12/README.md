# Unified text: standard TextBox View composition structural audit

Audit target: Jujutsu change `uormqyrm` over parent revision `94821c0b`.
Canonical command:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/unified-text-textbox-view-composition-2026-07-12
```

The audit scanned 1,258 Rust files / 623,665 physical Rust LOC and found 0
errors / 133 warning-level findings. No Cargo manifest changed in this slice,
so workspace dependency fan-in and fan-out are unchanged. The generated
`dependency_edges.csv`, `file_metrics.csv`, `public_type_duplicates.csv`, and
`violations.md` are the exact checkout evidence.

## Structured audit evidence

- [File measurements](file_metrics.csv)
- [Dependency edges](dependency_edges.csv)
- [Public type name screening](public_type_duplicates.csv)
- [Structural findings](violations.md)
