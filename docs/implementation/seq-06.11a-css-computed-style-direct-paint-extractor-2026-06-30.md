# seq06.11a CSS computed style direct paint extractor

## Summary

Applied `arcweft-seq06.11a-css-computed-style-direct-paint-extractor-2026-06-29.zip` to the current checkout.

This cut adds a deterministic Takumi computed-style extractor in `arcweft-takumi-adapter`. The extractor converts supported CSS paint values into Arcweft-owned `DirectPaintCatalog` entries. It does not render, rasterize, fetch resources, read files, allocate GPU resources, or introduce a private renderer path.

The visual path remains:

```text
Takumi computed style
  -> ComputedDirectPaintExtractor
  -> DirectPaintCatalog
  -> TakumiSceneLowerer
  -> arcweft-render-wgpu::view_scene::ViewScene primitives
```

## Production changes

- Added `crates/arcweft-takumi-adapter/src/paint_extractor.rs`.
- Added focused tests in `crates/arcweft-takumi-adapter/tests/computed_direct_paint.rs`.
- Added fixture CSS at `crates/arcweft-takumi-adapter/tests/fixtures/seq06_11a_direct_paint.css`.
- Exported the extractor and evidence/resource types from `arcweft-takumi-adapter`.
- Replaced Takumi-owned direct background/border payloads with renderer-owned `ViewSurfacePaint`.
- Updated `TakumiSceneLowerer` and runtime surface lowering to share `ViewSurfacePaint::append_primitives` / `ViewScene::push_surface_primitives` for deterministic painter order before borders.

## Supported first cut

- `background-color` to solid direct paint.
- Uniform circular `border-radius` to rounded clip and rounded solid layer metadata.
- Uniform visible solid border.
- Non-repeating `linear-gradient(...)` when stops can be normalized.
- Image backgrounds only when the caller supplies a stable resource index.
- Opacity as direct paint metadata.

Unsupported paint values produce `TakumiDiagnosticCode::UnsupportedDirectCss`. Missing image resources produce typed `DirectPaintResourceRequirement` entries. The adapter crate does not perform filesystem or network I/O to satisfy them.

## Package drift

The package patch could not be applied with `git apply` because the patch had a nonstandard generated index and was reported as corrupt. The new overlay files were copied from the package, and the existing-file edits were integrated manually.

One local drift was required for the current Takumi API: `GradientStop` is non-exhaustive, so the extractor handles unknown gradient stop variants by producing an unsupported-CSS diagnostic instead of using an incomplete match.

## Non-goals

- No native/web player integration in this cut. That remains seq06.11b.
- No private renderer branch, DOM/CSS overlay, canvas 2D path, screenshot fallback, or Takumi CPU raster output.
- No full CSS paint support. Unsupported advanced layers remain diagnostics.

## Validation

Passed:

```bash
cargo fmt --package arcweft-takumi-adapter
cargo fmt --package arcweft-takumi-adapter -- --check
cargo fmt --all -- --check
cargo test -p arcweft-takumi-adapter computed_direct_paint -- --nocapture
cargo test -p arcweft-takumi-adapter adapter_preserves_metadata_sidecar_and_takumi_attributes -- --nocapture
cargo clippy -p arcweft-takumi-adapter --all-targets --all-features -- -D warnings
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
```

## Follow-up

Seq06.11b should consume the contract in `docs/design/seq-06.11b-frame-paint-contract.md` and the existing request `docs/reviews/requests/2026-06-30-seq-06.11b-ui-scene-compositor-player-path-integration-package.md`.
