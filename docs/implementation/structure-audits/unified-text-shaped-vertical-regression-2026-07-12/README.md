# Unified text shaped-vertical regression structural audit

Audit scope: Jujutsu change `nyoynlov` over parent revision `37241449`
(`Unify player text in the prepared batch`). The canonical audit command was:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/unified-text-shaped-vertical-regression-2026-07-12
```

The audit scanned 2,636 repository files, including 1,255 Rust files and
621,347 physical Rust LOC. It reported 0 errors and 131 warnings. No Cargo
manifest, feature, workspace member, or dependency edge changed in this slice.

## Changed Rust files

The values below are exact current-checkout measurements, not diff additions.

| Path | Bytes | Physical LOC | Code LOC | Class | Embedded test LOC | Responsibilities |
|---|---:|---:|---:|---|---:|---|
| `crates/arcweft-cli/src/app/agent/native/player_observation/capture.rs` | 11,975 | 297 | 286 | production | — | Prepared-text capture-region visibility and painter ordering; aggregate cluster membership |
| `crates/arcweft-cli/src/app/agent/native/prepared_text_observation/view.rs` | 16,150 | 445 | 430 | production | — | View text line/run/ruby/glyph/cluster observation projection |
| `crates/arcweft-cli/src/app/agent/native/prepared_text_observation.rs` | 40,505 | 1,185 | 1,128 | production | — | Dialogue prepared-text observation and shared logical-cluster aggregation |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_samples_effects.rs` | 214,626 | 5,850 | 5,470 | integration test | — | Native Fx, reveal, mask, object-id, and raw-crop regression matrix |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_vertical.rs` | 238,245 | 6,613 | 6,152 | integration test | — | Native vertical, JLREQ, ruby, text-combine, geometry, and crop regression matrix |
| `crates/arcweft-glyphon/tests/shared_text_layout.rs` | 7,878 | 228 | 211 | integration test | — | Real project-font shaping and prepared vertical/ruby geometry |
| `crates/arcweft-render-text/src/resolved_document.rs` | 36,554 | 1,190 | 1,050 | production | — | Canonical resolved document, field-wise style cascade, source mapping |
| `crates/arcweft-render-text/tests/resolved_document.rs` | 13,602 | 434 | 401 | integration test | — | Resolved-document and nested layout-cascade behavior |
| `crates/arcweft-text-layout/src/config.rs` | 6,131 | 167 | 125 | production | — | Closed layout request, JLREQ resolution, structured layout errors |
| `crates/arcweft-text-layout/src/document_layout.rs` | 33,310 | 952 | 897 | production | — | Font-shaped document layout, run grouping, vertical placement, source maps |
| `crates/arcweft-text-layout/src/document_ruby.rs` | 20,769 | 585 | 555 | production | — | Ruby track reservation, shaping, placement, in-flow inter-character ruby |
| `crates/arcweft-text-layout/src/document_vertical.rs` | 8,113 | 254 | 224 | production | 64 | Shaped vertical-column DP, UAX/JLREQ break constraints, run-boundary policy |
| `crates/arcweft-text-layout/src/lib.rs` | 1,532 | 52 | 44 | facade | — | Intentional text-layout public surface and responsibility modules |
| `crates/arcweft-text-layout/src/vertical_clusters.rs` | 8,512 | 238 | 219 | production | — | Grapheme, UAX #50 orientation, text-combine, paragraph break offsets |
| `crates/arcweft-text-layout/tests/document_layout.rs` | 14,847 | 465 | 431 | integration test | — | Shaped layout, ruby reservation, run-boundary invariance, JLREQ behavior |

The two CLI integration matrices exceed the 2,500-LOC warning threshold but
remain below the 8,000-LOC error threshold. They are existing behavior-family
matrices split by vertical versus effects/capture responsibility. This slice
does not add another matrix or production responsibility to them. All changed
production modules remain below the 1,200-LOC warning threshold; the two
largest are already responsibility modules rather than crate facades.

## Dependency boundary

Unique workspace dependency fan-out/fan-in from the generated structured edge
inventory is:

| Crate | Fan-out | Fan-in |
|---|---:|---:|
| `arcweft-render-text` | 5 | 16 |
| `arcweft-text-layout` | 7 | 5 |
| `arcweft-glyphon` | 8 | 3 |
| `arcweft-render-wgpu` | 15 | 6 |
| `arcweft-player-scene` | 18 | 3 |
| `arcweft-cli` | 65 | 0 |

The dependency direction remains `render-text -> text-layout -> glyphon ->
render-wgpu/player-scene -> CLI` at the relevant boundary. The change adds no
reverse edge: paragraph/ruby semantics stay in the lower Sans-I/O layout
layer, raster preparation stays in glyphon, and observation/capture adaptation
stays in the CLI.

## Findings

- No error-level ownership, size, dependency, duplicate-public-type, or
  manifest finding was introduced.
- `document_layout.rs` groups contiguous vertical runs only by writing mode and
  resolved JLREQ strictness; paint/Fx boundaries no longer own line breaking.
- Logical cluster aggregation is implemented once for prepared observation and
  reused by dialogue and View projections. Capture uses the same cluster-index
  plus contained-source-range rule.
- Ruby track reservation and inter-character inline reservation remain in
  `document_ruby.rs`; the column planner consumes only the resulting typed
  advances and does not acquire presentation or renderer I/O responsibilities.
- The generated CSV and warning report in this directory are the canonical
  measurement evidence. The overall unified-text goal remains open for final
  project-font visual-golden promotion and deletion of the legacy layout API.
