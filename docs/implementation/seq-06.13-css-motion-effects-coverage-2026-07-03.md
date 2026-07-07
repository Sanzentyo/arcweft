# seq06.13 CSS Motion / Effects Coverage Implementation Note — 2026-07-03

## Source assumptions

This overlay assumes the current Arcweft main-line shape inspected through the
GitHub connector on 2026-07-03:

- seq06.9a introduced `ViewPaintNode`, `ViewCompositingGroup`, `ViewCompositingEffects`,
  `ViewClipPath`, `ViewMask`, and `ViewBlendMode` in `arcweft-render-wgpu::view_scene`;
- seq06.9b introduced `ViewCompositorPlan`, `ViewFilterPassPlan`, `ViewMaskChainPlan`,
  `ViewClipGeometryPlan`, `ViewBlendPassPlan`, and the compositor WGSL shader;
- current compositor execution already applies filters and mask passes but does
  not yet apply clip geometry as a final pixel constraint and does not resolve
  mask size/position/repeat in shader sampling;
- current `arcweft-ui::style` owns retained UI property kinds and values but has
  no motion model.

## Overlay changed files

### New source files

- `crates/arcweft-ui/src/motion.rs`
- `crates/arcweft-ui/tests/motion_transitions.rs`
- `crates/arcweft-render-wgpu/tests/ui_clip_mask_render_closure.rs`
- `crates/arcweft-render-wgpu/tests/view_blend_hsl_modes.rs`
- `crates/arcweft-render-wgpu/tests/view_compositor_gpu_smoke_timestamps.rs`

### Patched source files

- `crates/arcweft-ui/src/lib.rs`
- `crates/arcweft-ui/src/style.rs`
- `crates/arcweft-render-wgpu/src/view_clip_path.rs`
- `crates/arcweft-render-wgpu/src/view_mask.rs`
- `crates/arcweft-render-wgpu/src/view_compositor.rs`
- `crates/arcweft-render-wgpu/src/view_compositor_uniform.rs`
- `crates/arcweft-render-wgpu/src/view_direct_renderer.rs`
- `crates/arcweft-render-wgpu/src/view_blend.rs`
- `crates/arcweft-render-wgpu/src/view_shaders/compositor.wgsl`
- `crates/arcweft-render-wgpu/tests/view_compositor_plan.rs`

## Architecture alignment

### Owned-type behavior

The overlay follows Arcweft's owned-boundary rule:

- `UiPropertyKind` owns the transitionable-property decision and value
  interpolation dispatch.
- `Milli` and `Rgba8` own scalar/color interpolation.
- `ViewMaskPassPlan` owns mask sampling-plan resolution.
- `ViewClipGeometryPlan` owns the polygon shader-budget diagnostic.
- `ViewBlendShaderMode` owns the newly supported HSL blend mappings.

No extension trait, compatibility layer, or scattered local helper is introduced
for these boundary behaviors.

### Sans I/O boundaries

`arcweft-ui::motion` is pure data and deterministic sampling. It does not read
wall-clock time, GPU state, files, or network resources.

`arcweft-render-wgpu` receives prepared mask texture views and extents from the
resource provider. It does not perform URL fetches or filesystem loading.

### Renderer pass order

The compositor group order after this overlay is:

1. render group children to an offscreen target;
2. apply foreground filter plan;
3. apply clip geometry pass if needed;
4. apply ordered mask passes;
5. handle backdrop filters;
6. composite/blend to parent.

This keeps clip and mask constraints applied to the final group pixels before
blend composition.

## Implementation details

### Motion

`UiTransition` and `UiKeyframeTrack` sample explicit timestamps. The returned
`UiMotionSample` is designed to be copied into visual drift packets or golden
metadata.

Reduced motion is applied during sampling instead of mutating author specs. This
keeps original style data stable and lets captures compare multiple policies.

### Clip

The analytic clip pass uses uniform parameters rather than generating geometry or
CPU coverage textures:

- `params0.x` selects clip kind;
- `matrix[0]` stores rect/ellipse fields;
- `matrix[1]` stores inset radii;
- `clip_vertices[0..16]` stores polygon vertices.

Polygons above 16 vertices produce a typed planning error. That limit is chosen
so the first shader cut has a fixed uniform shape that works on native and web.

### Mask

`ViewMaskTextureView` now carries `ViewTextureExtent`. This lets `ViewMaskPassPlan`
resolve `mask-size` and `mask-position` before a mask pass is run.

WGSL mask sampling converts source UV to source pixels, subtracts the tile
origin, divides by tile size, applies repeat behavior per axis, and samples the
mask texture. Out-of-tile no-repeat coordinates produce zero coverage.

### Blend

The HSL-family modes use non-premultiplied sRGB input colors:

- `hue`: source hue + backdrop saturation/lightness;
- `saturation`: backdrop hue + source saturation + backdrop lightness;
- `color`: source hue/saturation + backdrop lightness;
- `luminosity`: backdrop hue/saturation + source lightness.

The result is then source-over composited using the existing compositor path.
Exact cross-GPU visual goldens should lock this rule before exposing it as a
public compatibility claim.

## Validation commands

Applied in a repository checkout on 2026-07-03. The package tests originally
expected floor rounding for midpoint fixed-point samples; the production
implementation uses nearest rounding consistently for `Milli` and `Rgba8`
interpolation, so the package test expectations were adjusted by one unit at
the exact half boundaries.

During application, the compositor shader uniform packing was split into
`crates/arcweft-render-wgpu/src/view_compositor_uniform.rs` so
`view_compositor.rs` stays below the repository's 1,200 physical LOC ownership
review threshold.

```bash
cargo fmt --all -- --check
cargo test -p arcweft-ui --test motion_transitions --all-features -- --nocapture
cargo test -p arcweft-render-wgpu --test ui_clip_mask_render_closure --all-features -- --nocapture
cargo test -p arcweft-render-wgpu --test view_blend_hsl_modes --all-features -- --nocapture
cargo test -p arcweft-render-wgpu --test view_compositor_plan --all-features -- --nocapture
cargo check -p arcweft-ui -p arcweft-render-wgpu --all-targets --all-features
cargo clippy -p arcweft-ui -p arcweft-render-wgpu --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
```

Validated during application:

```bash
cargo fmt --all
cargo test -p arcweft-ui --test motion_transitions --all-features -- --nocapture
cargo test -p arcweft-render-wgpu --test ui_clip_mask_render_closure --all-features -- --nocapture
cargo test -p arcweft-render-wgpu --test view_blend_hsl_modes --all-features -- --nocapture
cargo test -p arcweft-render-wgpu --test view_compositor_plan --all-features -- --nocapture
cargo check -p arcweft-ui -p arcweft-render-wgpu --all-targets --all-features
cargo clippy -p arcweft-ui -p arcweft-render-wgpu --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root . --write target/seq06_13_structure_audit_final
git diff --check
```

Final structural audit summary:

```text
files scanned: 2205
Rust files: 1071
Rust physical LOC: 504328
package manifests: 91
violations: 3 error(s), 126 warning(s)
```

The three audit errors are existing workspace size violations outside this
seq06.13 cut. The seq06.13 compositor split removed the new
`view_compositor.rs` warning that appeared during initial application.

Optional pinned-adapter smoke:

```bash
cargo test -p arcweft-render-wgpu --test view_compositor_gpu_smoke_timestamps --all-features -- --ignored --nocapture
```

## Known limitations and follow-ups

- `box-shadow` parity is not implemented in this zip; use seq06.13b for direct
  spread geometry and compositor blur parity.
- CSS `path()` clip-path remains unsupported until a tessellator is selected.
- `clip-path: url(...)`, `filter: url(...)`, gradient masks, and
  `mask: element(...)` remain structured diagnostics.
- Exact visual goldens are specified but not promoted to required CI until a
  pinned adapter/device readback harness is available.

## Design deviations

The broad seq06.13 request included box-shadow parity and CSS `path()` clip-path
as problem areas. This package intentionally implements diagnostics and follow-up
boundaries for those items rather than claiming incomplete parity.

## Remaining TODOs

- Promote the ignored GPU smoke fixture after native/web pinned golden capture is
  available.
- Use the existing follow-up seq06.13b request for box-shadow parity.
- Use
  `docs/reviews/requests/2026-07-03-seq-06.13c-vector-clip-and-advanced-mask-render-closure-package.md`
  for CSS path tessellation and gradient/element masks.
