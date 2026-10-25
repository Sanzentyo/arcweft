# Unified text: layer capture identity structural audit

Audit scope: Jujutsu change `vkwpxwkn`, which separates direct selected objects
from descendant pixel coverage for native Agent captures and makes layer
object-ID pixels and metadata use the same identity boundary.

Canonical command:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/unified-text-layer-capture-identity-2026-07-12
```

The audit scans 2,637 files and records 1,244 Rust files / 613,593 physical Rust LOC, 91 package
manifests, 0 errors, and 128 tracked warnings. No Cargo manifest, dependency
edge, serialized protocol shape, or public Rust boundary type changed.

## Structured audit evidence

- [File measurements](file_metrics.csv)
- [Dependency edges](dependency_edges.csv)
- [Public type name screening](public_type_duplicates.csv)
- [Structural findings](violations.md)
