# Seq06.11b UI Scene Compositor Player Path Integration

## Applied Scope

Applied package:

```text
D:/sanze/Downloads/arcweft-seq06.11b-ui-scene-compositor-player-path-integration-2026-06-30.zip
```

The package patch wrapper did not apply directly with `git apply --check`
(`patch with only garbage at line 23`), so the implementation was applied
manually against current `main` after seq06.4k.2 and seq06.11a.

Implemented changes:

- Added `PreparedUiScene`, `PreparedUiSceneResources`,
  `PreparedUiImageResource`, `PreparedUiMaskResource`, and
  `PreparedUiGlyphRunHandoff` to `arcweft-render-wgpu::geometry`.
- Added `PreparedFrame::with_ui_scenes`, `push_ui_scene`, and `ui_scenes`.
- Added `arcweft-render-wgpu::ui_direct_renderer` with a wgpu direct primitive
  renderer and prepared mask texture provider.
- Connected `SharedRenderer::render_to_view` to render attached UI scenes
  through the existing `UiCompositor`.
- Added typed compositor errors for invalid primitive ranges, missing image
  resources, unhandled glyph runs, and unsupported UI clips/primitives.
- Added focused render-wgpu and render-web tests for the frame attachment,
  compositor planning, glyph handoff evidence, and the no-DOM-overlay web
  contract.

## Validation

Executed in `D:/git/arcweft` on 2026-06-30:

```bash
cargo fmt --all
cargo test -p arcweft-render-wgpu --test ui_scene_player_path --all-features
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
crates/arcweft-render-wgpu/src/ui_direct_renderer.rs        35898  1058  production renderer module
crates/arcweft-render-wgpu/src/geometry.rs                  38381  1118  production frame boundary
crates/arcweft-render-wgpu/src/renderer.rs                  38482  1082  production shared renderer
crates/arcweft-render-wgpu/src/ui_compositor.rs             39236  1072  production compositor
crates/arcweft-render-wgpu/tests/ui_scene_player_path.rs     4352   118  integration test
crates/arcweft-render-web/tests/no_dom_overlay_contract.rs    706    19  integration test
```

The new direct renderer remains below the 1,200 LOC production warning
threshold, but is intentionally close because it owns the first cohesive wgpu
primitive pipeline, geometry tessellation, image upload, and mask texture
provider. A later renderer-quality cut may split shader/pipeline helpers after
the retained UI text and gradient paths settle.

## Remaining TODOs

- Full dialogue/text migration into `UiScene` remains a separate migration.
- Exact per-pixel CSS gradient parity remains a renderer-quality follow-up.
- Advanced clip/path/mask closures remain future compositor work.
- Product UI resource lowering must attach real retained `UiScene` frames via
  `PreparedFrame::with_ui_scenes`; this cut only provides the shared renderer
  attachment and execution path.

## Design Deviations

- The package source gate referenced `crates/arcweft-render-web/src/app.rs`,
  which does not exist in the current checkout. The test was adjusted to inspect
  `crates/arcweft-player-web/src/app.rs` and
  `crates/arcweft-render-web/src/web.rs`.
- The direct renderer overlay was updated for current wgpu pipeline-layout API
  and workspace clippy rules. No `allow` attributes were added.
