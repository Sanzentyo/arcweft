# seq06.13c Vector Clip / Advanced Mask Render Closure Implementation Note — 2026-07-03

## Source assumptions

This overlay assumes Arcweft main as inspected through the GitHub connector on 2026-07-03. The relevant current code already has:

- retained `ViewScene` / `ViewPaintNode` / `ViewCompositingGroup` data;
- `ViewClipPath` variants for inset/circle/ellipse/polygon/path;
- `ViewClipGeometryPlan` for seq06.13a analytic clips;
- `ViewMaskPassPlan`, `ViewMaskTextureView`, and compositor mask passes;
- a shared `view_shaders/compositor.wgsl` pass for filter, mask, clip, blend, and box-shadow work.

The overlay follows the repository rules in `AGENTS.md`: behavior is added to owned Arcweft types, resource acquisition stays in renderer/player adapters, and implementation-state notes live under `docs/implementation/`.

## Changed files in overlay

### Renderer/compositor

- `crates/arcweft-render-wgpu/src/view_scene.rs`
- `crates/arcweft-render-wgpu/src/view_scene/compositing.rs`
- `crates/arcweft-render-wgpu/src/view_clip_path.rs`
- `crates/arcweft-render-wgpu/src/view_mask.rs`
- `crates/arcweft-render-wgpu/src/view_compositor.rs`
- `crates/arcweft-render-wgpu/src/view_compositor_uniform.rs`
- `crates/arcweft-render-wgpu/src/view_shaders/compositor.wgsl`
- `crates/arcweft-render-wgpu/tests/ui_clip_mask_render_closure.rs`

### Adapter

- `crates/arcweft-takumi-adapter/src/lowering.rs`

### Docs and fixtures

- `docs/design/seq-06.13c-vector-clip-advanced-mask-render-closure-design.md`
- `docs/implementation/seq-06.13c-vector-clip-advanced-mask-render-closure-2026-07-03.md`
- `docs/implementation/seq-06.13c-css-mask-clip-support-matrix.md`
- `docs/fixtures/css/seq06.13c-vector-clip-advanced-mask.css`
- `docs/fixtures/retained-view/seq06_13c_retained_view_fixture.rs`

## Implementation summary

### `clip-path: path(...)`

`ViewClipGeometryPlan::from_clip_path` now accepts `ViewClipPath::Path`. It parses path data, emits typed command records, flattens lines/quadratics/cubics into a fixed edge list, and returns `ViewClipGeometryPlan::Path`.

The compositor uniform adds a `clip_edges` array. The WGSL clip pass keeps existing inset/ellipse/polygon paths and adds path coverage with even-odd or non-zero winding over the edge array.

The path parser intentionally supports only `M/L/H/V/Q/C/Z` plus relative variants in this cut. Unsupported path commands are not ignored; they are typed diagnostics.

### `clip-path: url(...)`

The retained View contract gains `ViewClipPath::Url(Box<str>)`. The renderer returns `UrlClipResourceUnsupported`. No reusable vector clip resource table is claimed in this package.

### Gradient masks

The retained View contract gains `ViewMaskImage::Gradient(ViewMaskGradient)`.

`ViewMaskPassPlan` canonicalizes stop coverage and exposes `gradient_plan(tile_size_px)`. The compositor then runs a mask pass with generated gradient coverage instead of an external texture sample.

Supported retained forms:

- linear;
- radial;
- conic.

CSS/Takumi lowering implemented in this overlay:

- non-repeating `linear-gradient(...)` in `mask-image`.

CSS/Takumi lowering intentionally diagnostic:

- repeating linear gradients;
- radial/conic gradients until normalized Takumi adapter fixtures are added;
- color hints/unsupported stops that cannot be converted into deterministic `ViewGradientStop` values.

### `mask-repeat: space | round`

`ViewMaskSamplingPlan` now stores per-axis mode, stride, and tile count. `Space` and `Round` resolve deterministically and no longer return `UnsupportedRepeat`.

Existing `repeat_x` / `repeat_y` fields are retained as compatibility-neutral convenience evidence, but shader behavior uses `repeat_mode_x`, `repeat_mode_y`, `tile_stride_px`, and `tile_count`.

### `mask: element(...)`

`ViewMaskImage::Element(ViewElementMaskSource)` is added so adapters can represent the feature without stringly unsupported values. Since no typed compositor capture resource graph was available in the inspected render path, the current cut returns `ViewMaskPlanError::ElementMaskCaptureUnavailable { element_id }`.

## Tests added/updated

Focused tests cover:

- `path(...)` lines and curves for both fill rules;
- degenerate path commands as typed diagnostics;
- gradient alpha versus luminance coverage;
- `space` and `round` tile distribution;
- element mask structured diagnostics;
- compositor plan counts with a path clip and gradient mask;
- existing seq06.13a inset/circle/ellipse/polygon and texture-mask tests.

Optional ignored native/web smoke fixtures are documented but not promoted to required CI.

## Validation commands

Run from repository root after applying the overlay:

```bash
cargo fmt --all -- --check
cargo test -p arcweft-render-wgpu --test ui_clip_mask_render_closure --all-features -- --nocapture
cargo test -p arcweft-takumi-adapter --lib --all-features -- --nocapture
cargo check -p arcweft-render-wgpu -p arcweft-takumi-adapter --all-targets --all-features
cargo clippy -p arcweft-render-wgpu -p arcweft-takumi-adapter --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root . --write target/seq06_13c_structure_audit
just test-fast
git diff --check
```

Optional visual smoke:

```bash
cargo test -p arcweft-render-wgpu --test view_compositor_gpu_smoke_timestamps --all-features -- --ignored --nocapture
```

## Local package generation note

The zip was generated from GitHub-connector-inspected source in a container without GitHub network access and without a Rust toolchain. Therefore, no local `cargo` or `rustfmt` result is claimed by this package. Repository-side validation must be run after apply, and any failures must be fixed or documented before commit.

## Repository application validation

Applied to the local repository on 2026-07-03 and validated with:

```bash
cargo fmt --all -- --check
cargo test -p arcweft-render-wgpu --test ui_clip_mask_render_closure --all-features -- --nocapture
cargo test -p arcweft-takumi-adapter --lib --all-features -- --nocapture
cargo check -p arcweft-render-wgpu -p arcweft-takumi-adapter --all-targets --all-features
cargo clippy -p arcweft-render-wgpu -p arcweft-takumi-adapter --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root . --write target\seq06_13c_structure_audit
just test-fast
git diff --check
```

Results:

- focused render-wgpu closure test passed: 8 passed;
- focused Takumi adapter lib test passed: 18 passed;
- targeted check passed for `arcweft-render-wgpu` and `arcweft-takumi-adapter`;
- targeted clippy passed with `-D warnings`;
- `just test-fast` passed;
- `git diff --check` passed;
- structural audit wrote `target\seq06_13c_structure_audit` and reported existing workspace hotspots: 4 error(s), 125 warning(s). The error-level files are pre-existing large files outside this seq06.13c change set.

The ignored GPU visual smoke was not promoted or executed as required evidence because pinned native/web readback is still a remaining TODO for exact visual promotion.

## Structural audit expectation

The change materially modifies renderer planning and shader contracts, so a structural audit is required. Expected changed production files remain below the AGENTS.md decomposition error threshold. If local line counts exceed thresholds after merge with concurrent work, split by responsibility rather than adding helper shims.

## Remaining TODOs

- Promote native/web visual goldens after pinned adapter capture is available.
- Design reusable `clip-path: url(...)` vector resources separately.
- Add real `mask: element(...)` rendering only after a typed element-capture resource graph exists.
- Add Takumi/CSS radial and conic mask lowering after normalized gradient shape fields and fixtures are confirmed.
