# Dialogue View hardening and vertical Style audit — 2026-07-13

Audit command:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/dialogue-view-hardening-vertical-style-2026-07-13
```

The audited Jujutsu stack tip is `nprqlvxyowwurloqqryrwwkzknkzpyrw`, based on
commit `4f7338875f8c723712126f9639e283b2672f6df8`. The checkout contained 2,664
scanned files, 1,264 Rust files, 617,309 physical Rust LOC, and 91 package
manifests. The result was 0 errors and 125 warnings. The preceding product cut
had 128 warnings; moving `arcweft-view` tests out of its facade and production
text-field module removed three warnings without changing behavior.

## Validation note

Workspace check, Clippy with warnings denied, fast tests, workspace tests,
vertical Style capture, unified Native/Web text parity, and the four promoted
exact vertical golden references passed. The full Tier 2 aggregate still stops
in its pre-existing MCP stdio harness: ignored tests assume raw resource URIs,
unmoderated semantic IDs, and legacy dialogue geometry while the current MCP
content-policy contract publishes opaque moderated resources. The separate
slow Agent-observe test likewise retains legacy `frame/0` and 96/548/1088x124
geometry expectations. These failures are recorded as an independent test
harness migration, not waived as evidence for this cut.

## Structured audit evidence

- [File measurements](file_metrics.csv)
- [Dependency edges](dependency_edges.csv)
- [Public type name screening](public_type_duplicates.csv)
- [Structural findings](violations.md)
