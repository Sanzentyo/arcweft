# seq06.9b wgpu UI Compositor Effects Implementation — 2026-06-29

## Upstream assumption

This implementation assumes seq06.9a has already introduced the Arcweft-owned
compositing graph in `arcweft-render-wgpu::view_scene`. The overlay consumes that
contract and does not replace its scene types.

## Changed files

- `crates/arcweft-render-wgpu/src/lib.rs`
- `crates/arcweft-render-wgpu/src/view_effects.rs`
- `crates/arcweft-render-wgpu/src/view_blend.rs`
- `crates/arcweft-render-wgpu/src/view_mask.rs`
- `crates/arcweft-render-wgpu/src/view_clip_path.rs`
- `crates/arcweft-render-wgpu/src/view_compositor.rs`
- `crates/arcweft-render-wgpu/src/view_shaders/compositor.wgsl`
- `crates/arcweft-render-wgpu/tests/view_compositor_plan.rs`
- `crates/arcweft-render-wgpu/tests/view_compositor_source_gates.rs`
- `crates/arcweft-render-wgpu/tests/view_compositor_gpu_smoke.rs`

## Acceptance criteria mapping

| Request item | Implementation evidence |
| --- | --- |
| Responsibility modules | Added `view_compositor`, `view_effects`, `view_mask`, `view_blend`, and `view_clip_path`. |
| Offscreen group rendering | `ViewCompositor::render_scene` renders the frame to a root offscreen target; groups render children into pooled offscreen targets. |
| Color-matrix filters | `ViewColorMatrix` implements brightness, contrast, grayscale, sepia, saturate, hue-rotate, invert, and opacity. |
| Blur | `ViewFilterPassPlan` emits horizontal/vertical blur passes with expanded extents. |
| Drop-shadow | `ViewDropShadowPassPlan` records alpha-derived shadow offset, blur, tint, and extent. WGSL includes a shadow pass. |
| Backdrop-filter | Parent target is copied to an intermediate texture before filtering; active target sampling is avoided. |
| Mask composition | `ViewMaskChainPlan` preserves ordered masks; WGSL supports alpha/luminance coverage. |
| Clip-path | Inset, ellipse/circle, and polygon plans are implemented; path remains explicit unsupported. |
| Mix-blend-mode | Shader modes cover the initial supported blend set; HSL/luminosity modes remain explicit unsupported. |
| Texture pools | `ViewRenderTargetPool` reuses same-format/same-extent targets and `ViewTextureExtent::bucketed` provides deterministic bounds. |

## Validation commands

Run from the repository root after applying seq06.9a and this overlay:

```bash
cargo fmt --all -- --check
cargo test -p arcweft-render-wgpu --all-features -- --nocapture
cargo test -p arcweft-render-wgpu --test view_compositor_plan --all-features -- --nocapture
cargo test -p arcweft-render-wgpu --test view_compositor_source_gates --all-features -- --nocapture
cargo check -p arcweft-render-wgpu --all-targets --all-features
cargo clippy -p arcweft-render-wgpu --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
```

Optional pinned-GPU fixture:

```bash
cargo test -p arcweft-render-wgpu --test view_compositor_gpu_smoke --all-features -- --ignored --nocapture
```

## Known platform/GPU limitations

- The exact image golden is included as an ignored fixture because this package
  was assembled outside a full Arcweft checkout with a pinned GPU readback
  harness.
- `mix-blend-mode: hue | saturation | color | luminosity` remains a deliberate
  follow-up because these modes need HSL/luminosity decomposition and exact
  visual goldens.
- `clip-path: path(...)` remains explicit unsupported pending a selected vector
  path tessellator.
- External mask image lookup is intentionally delegated to `ViewMaskTextureProvider`.

## Structural audit notes

The implementation is split before large-file thresholds are reached. The new
modules separate effect planning, mask planning, clip geometry, blend-mode
classification, and GPU orchestration instead of adding all responsibilities to
`renderer.rs` or `view_scene.rs`.

## Repository application notes

The package overlay was applied to the local repository and adjusted for the
current seq06.9a contract and workspace lint policy:

- `ViewCompositingEffects::clip_path` is `Option<Box<ViewClipPath>>` in the applied
  seq06.9a contract, so compositor planning uses `as_deref()`.
- `ViewCompositor::render_scene` now accepts `&mut ViewCompositorFrame` and internal
  rendering state is carried by `ViewCompositorRenderState`, avoiding wide
  argument lists while keeping device/queue/encoder/resource ownership outside
  the renderer crate.
- `ViewRenderTargetPool::acquire` returns the target directly because allocation
  failures are reported by wgpu rather than this typed planning layer.
- integer/float conversions for shader uniforms and texture extents use typed
  conversion helpers instead of unchecked casts.
- the source gate scans the seq06.9b compositor modules. It intentionally does
  not scan the pre-existing `offscreen.rs` readback harness, which is used for
  explicit capture fixtures and is not a Takumi full-surface fallback.
- the package's original multi-filter `cargo test ... view_effects view_blend ...`
  command is not valid Cargo syntax, so repository validation uses the full
  `arcweft-render-wgpu` test suite plus the named integration tests.

## Repository validation status

Executed from the local repository after applying the overlay:

```bash
cargo fmt --all -- --check
cargo test -p arcweft-render-wgpu --all-features -- --nocapture
cargo check -p arcweft-render-wgpu --all-targets --all-features
cargo clippy -p arcweft-render-wgpu --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
```

Results:

- all `arcweft-render-wgpu` unit, integration, source-gate, and doc tests
  passed;
- `view_compositor_gpu_smoke` remained ignored as designed because it requires a
  pinned adapter/device and exact image-readback fixture;
- `cargo check` passed for all targets and features in `arcweft-render-wgpu`;
- clippy passed with `-D warnings`;
- structural audit completed with `0 error(s), 117 warning(s)` across `1936`
  scanned files and `995` Rust files;
- `git diff --check` passed.
