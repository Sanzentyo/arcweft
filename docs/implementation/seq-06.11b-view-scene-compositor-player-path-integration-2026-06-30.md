# Seq06.11b UI Scene Compositor Player Path Integration

## Applied Scope

Applied package:

```text
D:/sanze/Downloads/arcweft-seq06.11b-view-scene-compositor-player-path-integration-2026-06-30.zip
```

The package patch wrapper did not apply directly with `git apply --check`
(`patch with only garbage at line 23`), so the implementation was applied
manually against current `main` after seq06.4k.2 and seq06.11a.

Implemented changes:

- Added `PreparedViewScene`, `PreparedViewSceneResources`,
  `PreparedViewImageResource`, and `PreparedViewMaskResource` to
  `arcweft-render-wgpu::geometry`. The provisional glyph-run sidecar described
  by the original cut was removed by the unified-text migration on 2026-07-12;
  `ViewPrimitive::Text` now refers to `PreparedTextId` directly.
- Added `PreparedFrame::with_view_scenes`, `push_view_scene`, and `view_scenes`.
- Added `arcweft-render-wgpu::view_direct_renderer` with a wgpu direct primitive
  renderer and prepared mask texture provider.
- Connected `SharedRenderer::render_to_view` to render attached View scenes
  through the existing `ViewCompositor`.
- Added typed compositor errors for invalid primitive ranges, missing image or
  prepared-text resources, and unsupported UI clips/primitives.
- Added focused render-wgpu and render-web tests for the frame attachment,
  compositor planning, direct prepared-text evidence, and the no-DOM-overlay web
  contract.

## Validation

Executed in `D:/git/arcweft` on 2026-06-30:

```bash
cargo fmt --all
cargo test -p arcweft-render-wgpu --test view_scene_player_path --all-features
cargo test -p arcweft-render-web --test no_dom_overlay_contract --all-features
cargo check -p arcweft-render-wgpu -p arcweft-render-web -p arcweft-player-native -p arcweft-player-web --all-targets --all-features
cargo clippy -p arcweft-render-wgpu -p arcweft-render-web -p arcweft-player-native -p arcweft-player-web --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
cargo build -p arcweft-player-web --target wasm32-unknown-unknown
just test-fast
```

Results:

- render-wgpu focused tests: 4 passed;
- render-web source contract test: 1 passed;
- native/web package check: passed;
- native/web package clippy with `-D warnings`: passed;
- structural audit: `files scanned: 2110`, `Rust files: 1041`,
  `Rust physical LOC: 492210`, `violations: 0 error(s), 125 warning(s)`;
- wasm player build: passed.
- `just test-fast`: 151 + 31 + 71 + 8 + 129 tests passed.

## Structural Notes

This cut changes a public frame boundary and adds a renderer subsystem module,
so structural audit was required.

Current changed file sizes after formatting:

```text
path                                                        bytes   LOC   role
crates/arcweft-render-wgpu/src/view_direct_renderer.rs        35898  1058  production renderer module
crates/arcweft-render-wgpu/src/geometry.rs                  38381  1118  production frame boundary
crates/arcweft-render-wgpu/src/renderer.rs                  38482  1082  production shared renderer
crates/arcweft-render-wgpu/src/view_compositor.rs             39236  1072  production compositor
crates/arcweft-render-wgpu/tests/view_scene_player_path.rs     4352   118  integration test
crates/arcweft-render-web/tests/no_dom_overlay_contract.rs    706    19  integration test
```

The new direct renderer remains below the 1,200 LOC production warning
threshold, but is intentionally close because it owns the first cohesive wgpu
primitive pipeline, geometry tessellation, image upload, and mask texture
provider. A later renderer-quality cut may split shader/pipeline helpers after
the retained View text and gradient paths settle.

## Remaining TODOs

- Full dialogue/text migration into `ViewScene` remains a separate migration.
- Exact per-pixel CSS gradient parity remains a renderer-quality follow-up.
- Advanced clip/path/mask closures remain future compositor work.
- Product View resource lowering must attach real retained `ViewScene` frames via
  `PreparedFrame::with_view_scenes`; this cut only provides the shared renderer
  attachment and execution path.

## Design Deviations

- The package source gate referenced `crates/arcweft-render-web/src/app.rs`,
  which does not exist in the current checkout. The test was adjusted to inspect
  `crates/arcweft-player-web/src/app.rs` and
  `crates/arcweft-render-web/src/web.rs`.
- The direct renderer overlay was updated for current wgpu pipeline-layout API
  and workspace clippy rules. No `allow` attributes were added.
