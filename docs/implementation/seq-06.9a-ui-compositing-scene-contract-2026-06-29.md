# seq-06.9a UI Compositing Scene Contract Implementation

Date: 2026-06-29
Package: `arcweft-seq-06.9a-ui-compositing-scene-contract-package`

## Files in the Overlay

```text
overlay/
  crates/
    arcweft-render-wgpu/
      src/
        view_scene.rs
        view_scene/
          core.rs
          compositing.rs
    arcweft-takumi-adapter/
      src/
        cache.rs
        lib.rs
        lowering.rs
        style.rs
      tests/
        no_cpu_raster_fallback.rs
```

## What Changed

### `arcweft-render-wgpu::view_scene`

The existing direct primitive scene was moved to `view_scene/core.rs`.
`view_scene.rs` now re-exports the existing direct types and introduces the
`view_scene/compositing.rs` responsibility module.

The direct scene still exposes:

- `ViewScene::contexts()`
- `ViewScene::primitives()`
- existing `ViewPrimitive` variants
- `ViewSceneContext`
- `ViewPrimitiveRange`

The new graph surface is:

- `ViewScene::paint_nodes()`
- `ViewPaintNode::Direct`
- `ViewPaintNode::Group`
- `ViewCompositingGroup`

### `arcweft-takumi-adapter::style`

The property classes were expanded so the five subtree-effect families are not
generic unsupported-direct values:

- `filter` and `mix-blend-mode`: `Compositing`
- `backdrop-filter`: `BackdropCompositing`
- `mask` and mask positioning/sizing/repeat fields: `MaskCompositing`
- `clip-path`: `ClipGeometry`
- `mask-image`: `Resource`

Representable values no longer produce unsupported-direct diagnostics. Values
that still cannot be represented remain explicit diagnostics.

### `arcweft-takumi-adapter::lowering`

The lowerer now builds a `TakumiCompositingStyleCatalog` from the Takumi
`RenderNode` tree before layout results are consumed. This preserves Takumi as
the cascade/layout/stacking source and recovers computed style by `TakumiPath`.

The stacking-context walk still emits the old direct primitive contexts. It now
also returns ordered `ViewPaintNode` children and wraps each Takumi stacking
context in `ViewCompositingGroup`.

### `arcweft-takumi-adapter::cache`

A focused test documents the resource-revision implication for
`mask-image: url(...)`: changing the scene image revision changes the scene cache
key.

### Source Gate

`tests/no_cpu_raster_fallback.rs` scans the Takumi adapter and renderer source
roots for known CPU full-surface raster fallback markers.

## Tests Added

- Filter list canonicalization.
- Filter visual outset.
- Deterministic compositing requirements.
- CSS property classification for `filter`, `backdrop-filter`, `mask`,
  `clip-path`, and `mix-blend-mode`.
- `mask-image` resource-revision classification and scene cache key behavior.
- Lowering build child-order preservation inside a compositing group.
- Structural no-CPU-raster-fallback source gate.

## Apply Instructions

From the repository root after seq06.2 and the current seq06.4-06.8 cuts:

```bash
cp -R overlay/crates/arcweft-render-wgpu/src/view_scene.rs crates/arcweft-render-wgpu/src/view_scene.rs
mkdir -p crates/arcweft-render-wgpu/src/view_scene
cp -R overlay/crates/arcweft-render-wgpu/src/view_scene/. crates/arcweft-render-wgpu/src/view_scene/

cp overlay/crates/arcweft-takumi-adapter/src/cache.rs crates/arcweft-takumi-adapter/src/cache.rs
cp overlay/crates/arcweft-takumi-adapter/src/lib.rs crates/arcweft-takumi-adapter/src/lib.rs
cp overlay/crates/arcweft-takumi-adapter/src/lowering.rs crates/arcweft-takumi-adapter/src/lowering.rs
cp overlay/crates/arcweft-takumi-adapter/src/style.rs crates/arcweft-takumi-adapter/src/style.rs
mkdir -p crates/arcweft-takumi-adapter/tests
cp overlay/crates/arcweft-takumi-adapter/tests/no_cpu_raster_fallback.rs crates/arcweft-takumi-adapter/tests/no_cpu_raster_fallback.rs

mkdir -p docs/design docs/implementation
cp docs/design/seq-06.9a-ui-compositing-scene-contract-design.md docs/design/
cp docs/implementation/seq-06.9a-ui-compositing-scene-contract-2026-06-29.md docs/implementation/
```

## Validation Commands

Run these from the repository root after applying the overlay:

```bash
cargo fmt --all -- --check
cargo check -p arcweft-render-wgpu -p arcweft-takumi-adapter --all-targets --all-features
cargo test -p arcweft-render-wgpu view_scene::compositing --all-features -- --nocapture
cargo test -p arcweft-takumi-adapter style::tests::compositing_properties_are_not_generic_unsupported_direct --all-features -- --nocapture
cargo test -p arcweft-takumi-adapter style::tests::mask_image_url_requires_resource_revision --all-features -- --nocapture
cargo test -p arcweft-takumi-adapter cache::tests::mask_image_url_resource_revision_changes_scene_cache_key --all-features -- --nocapture
cargo test -p arcweft-takumi-adapter lowering::tests::lowering_build_preserves_child_order_inside_compositing_group --all-features -- --nocapture
cargo test -p arcweft-takumi-adapter --test no_cpu_raster_fallback --all-features -- --nocapture
cargo clippy -p arcweft-render-wgpu -p arcweft-takumi-adapter --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
```

## Repository Application Notes

The package overlay was applied to the local repository and adjusted for the
current pinned Takumi API and workspace lint set:

- `TakumiFilter::HueRotate` reads the public `Angle` deref value instead of the
  private tuple field used by the package draft.
- the exhaustive `BackgroundImage` match no longer carries an unreachable
  fallback arm;
- `ViewCompositingEffects::clip_path` stores the large optional geometry behind
  `Option<Box<ViewClipPath>>` so `ViewPaintNode` stays compact without boxing the
  entire compositing group;
- Takumi lowering read-only dependencies are grouped in `TakumiLoweringRefs`
  instead of passing eight separate arguments through the recursion;
- the existing adapter contract test now checks `filter: url(...)` as the
  unsupported diagnostic case because `filter: blur(...)` is representable by
  this cut.

## Repository Validation Status

Executed from the local repository after applying the overlay:

```bash
cargo fmt --all -- --check
cargo check -p arcweft-render-wgpu -p arcweft-takumi-adapter --all-targets --all-features
cargo test -p arcweft-render-wgpu view_scene::compositing --all-features -- --nocapture
cargo test -p arcweft-takumi-adapter --all-features -- --nocapture
cargo clippy -p arcweft-render-wgpu -p arcweft-takumi-adapter --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
```

Results:

- all focused renderer compositing tests passed;
- all `arcweft-takumi-adapter` unit, integration, source-gate, and doc tests
  passed;
- clippy passed with `-D warnings`;
- structural audit completed with `0 error(s), 117 warning(s)` across `1925`
  scanned files and `987` Rust files;
- `git diff --check` passed.
