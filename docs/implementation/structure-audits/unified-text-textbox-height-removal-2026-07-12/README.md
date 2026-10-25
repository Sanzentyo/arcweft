# Unified text: Agent TextBox-height removal structural audit

Audit scope: Jujutsu change `xxwwmruw`, which removes the unconsumed Agent
`textbox_height` request field from the CLI and MCP tool schemas and removes its
stale tests and command plumbing.

Canonical command:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/unified-text-textbox-height-removal-2026-07-12
```

The audit scans 2,635 files, including 1,244 Rust files / 613,514 physical Rust
LOC and 91 package manifests. It reports 0 errors and 128 pre-existing tracked
warnings. No Cargo manifest, workspace dependency edge, public Rust boundary
type, renderer contract, or persistence format changed.

## Structured audit evidence

- [File measurements](file_metrics.csv)
- [Dependency edges](dependency_edges.csv)
- [Public type name screening](public_type_duplicates.csv)
- [Structural findings](violations.md)
