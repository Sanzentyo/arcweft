# Unified text DialogueView final structural audit — 2026-07-13

Revision measured: Jujutsu change `oknwozsuyqqurprwpmuplplmmmxqprpt`, committed
as `29f0dded1039f0c7dc2fbbb041c449e400df2424` (`Finalize unified text and typed
dialogue Views`).

Canonical commands:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root .
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/unified-text-dialogue-view-final-2026-07-13
```

The audit scanned 2,651 files, including 1,255 Rust files and 616,763 physical
Rust LOC across 91 package manifests. It reports **0 errors and 128 warnings**.
The warnings comprise 93 size warnings, 25 embedded-test warnings, 7 facade
warnings, and 3 tracked architecture warnings. No architecture warning points
at a manifest changed by the DialogueView integration.

## Structured audit evidence

- [File measurements](file_metrics.csv)
- [Dependency edges](dependency_edges.csv)
- [Public type name screening](public_type_duplicates.csv)
- [Changed Rust files](changed-rust-files.csv)
- [Structural findings](violations.md)
