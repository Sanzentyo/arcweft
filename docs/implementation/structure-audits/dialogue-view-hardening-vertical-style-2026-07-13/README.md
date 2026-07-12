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

## Changed production hotspots

Exact measurements come from `file_metrics.csv` in this directory.

| Path | Owning crate | Bytes | Physical LOC | Responsibility |
| --- | --- | ---: | ---: | --- |
| `crates/arcweft-bundle/src/lib.rs` | `arcweft-bundle` | 77,326 | 2,162 | bundle assembly and cross-section validation |
| `crates/arcweft-bundle/src/resource_codec/view/codec.rs` | `arcweft-bundle` | 68,629 | 1,914 | deterministic View section codec and canonical-table validation |
| `crates/arcweft-bundle/src/resource_codec/view/model.rs` | `arcweft-bundle` | 77,205 | 2,449 | typed View resource records; remains below the 2,500 LOC error threshold |
| `crates/arcweft-bundle/src/resource_codec/view/dialogue_contract.rs` | `arcweft-bundle` | 6,270 | 154 | typed Dialogue projection cross-record validation split from `model.rs` |
| `crates/arcweft-runtime-driver/src/view_runtime/evaluator.rs` | `arcweft-runtime-driver` | 52,744 | 1,306 | deterministic View program evaluation |
| `crates/arcweft-player-scene/src/frame/view_text.rs` | `arcweft-player-scene` | 27,620 | 790 | canonical prepared-text lowering for mounted Views |
| `crates/arcweft-view/src/lib.rs` | `arcweft-view` | 5,953 | 152 | intentional public facade |
| `crates/arcweft-view/src/text_field.rs` | `arcweft-view` | 40,103 | 1,194 | text-field production behavior, now without embedded tests |

No Cargo manifest, dependency edge, feature, or crate boundary changed. The new
parameter role and text-surface contracts remain owned by `arcweft-bundle` and
flow through the existing `arcweft-cli -> arcweft-bundle` lowering and
`arcweft-runtime-driver -> arcweft-bundle` evaluation dependencies. The
complete workspace fan-in/fan-out inventory is in `dependency_edges.csv`.

## Structural result

The temporary 2,599 LOC growth in the View model was corrected before this
audit by extracting the closed Dialogue cross-record validator. No new
error-level hotspot, dependency cycle, broad root re-export, compatibility
module, source gate, or generated source was introduced. Remaining warnings
are pre-existing staged decomposition candidates and are listed in
`violations.md`.

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
