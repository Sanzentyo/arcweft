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

## Changed production responsibilities

| Path | Bytes | Physical LOC | Role |
| --- | ---: | ---: | --- |
| `crates/arcweft-render-wgpu/src/geometry.rs` | 72,595 | 2,196 | generic shared frame contract after deleting dialogue/styled-paragraph staging |
| `crates/arcweft-render-wgpu/src/renderer.rs` | 60,637 | 1,742 | ordered rectangle/image/ordinary/prepared submission orchestration after deleting the styled renderer |
| `crates/arcweft-render-wgpu/src/renderer/prepared_text.rs` | 9,411 | 281 | canonical prepared glyph submission and effect isolation with per-submission GPU buffer lifetime |
| `crates/arcweft-render-wgpu/src/renderer/view_text.rs` | 8,174 | 250 | exact View painter-position callback into the canonical prepared submission pool |
| `crates/arcweft-player-web/src/report.rs` | 22,451 | 644 | host-neutral projection of canonical runs, lines, glyph paint, ruby, ownership, and transforms |
| `crates/arcweft-player-web/src/parity.rs` | 10,743 | 299 | native-equivalent player-frame checkpoints through the normal stateful planner/input path |
| `crates/arcweft-player-web/src/app.rs` | 32,347 | 840 | Web event-loop/GPU readiness, frame preparation, rendering, and observation dispatch |
| `crates/arcweft-player-scene/src/frame.rs` | 17,313 | 495 | product frame orchestration and direct persistent TextBox state |
| `tools/verify-text-raster-parity.rs` | 58,105 | 1,752 | standalone canonical prepared-glyph evidence and raster comparison tool |

Deleting `geometry/dialogue.rs` and the 537-LOC styled-paragraph integration
test removes rather than relocates the duplicated paragraph model. The shared
geometry facade fell from 2,367 to 2,196 LOC and the renderer from 2,370 to
1,742 LOC. Both remain warning-level files, but their dialogue layout,
styled-evidence relayout, and platform-specific paragraph arithmetic are gone.
Prepared submission and View callbacks remain separate focused responsibility
modules.

The largest non-vendored Rust files remain pre-existing integration suites:
`cli_runtime_bench.rs` (7,948 LOC), `native_vertical.rs` (6,564 LOC), and
`published_jlreq_class_mix.rs` (6,161 LOC). The largest production files are
the unchanged `arcweft-core/src/value.rs` (2,500 LOC) and
`arcweft-core/src/engine/eval/calls.rs` (2,481 LOC). This slice creates no new
error-level file.

## Dependency and boundary review

`arcweft-player-web` adds direct normal edges to `arcweft-glyphon` and
`arcweft-text-layout`, increasing its recorded fan-out to 28 while fan-in
remains 0. Both edges point from the high-level Web adapter to lower-level
Sans-I/O layout/prepared-text contracts. They let the observation adapter
project exact owned boundary types without asking WGPU to relayout or adding a
Web-only evidence model. No lower-level crate depends on the Web adapter.

- runtime/persistence TextBox identity remains in `arcweft-runtime-driver`;
  the frame contains only typed scalar observation state and prepared owners;
- Web, native offscreen, and normal player capture all use
  `PlayerFramePlannerState` and the same ordered project font resources;
- each prepared glyph submission owns a distinct `TextRenderer` until the
  command buffer is submitted, preventing later prepares from destroying or
  overwriting an earlier pass's vertex buffer;
- the Web report serializes completed canonical layout/paint evidence and no
  longer performs renderer-owned shaping or layout;
- the CSS parity fixture's Noto subset is a deterministic test asset derived
  from the already licensed product font; it does not replace the stock player
  font inventory; and
- no source gate, compatibility alias, migration reader, unsafe block, or
  platform-specific text evaluator was added.
