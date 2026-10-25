# Typed View text preparation structural review

Audit checkout: Jujutsu change `xuxwrolxnzxx`, based on commit
`4f82638f267e` (`Execute per-mount View runtime`). The working commit ID is
content-dependent; the change ID is the stable audit identifier.

Canonical command:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/unified-text-view-prepared-sources-2026-07-12
```

Result: 2,632 files scanned, 1,258 Rust files, 633,404 physical Rust LOC,
92 package manifests, 0 errors, and 142 warnings. Generated, vendored,
`target/`, and VCS content are excluded by the audit script.

## Structured audit evidence

- [File measurements](file_metrics.csv)
- [Dependency edges](dependency_edges.csv)
- [Public type name screening](public_type_duplicates.csv)
- [Structural findings](violations.md)
