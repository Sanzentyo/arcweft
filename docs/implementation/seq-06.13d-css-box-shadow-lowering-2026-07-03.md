# seq06.13d CSS Box-Shadow Lowering Implementation Note — 2026-07-03

## Current implementation evidence inspected

The current Arcweft line already has the seq06.13b renderer substrate:

- `ViewBoxShadow`, `ViewBoxShadowKind`, and `ViewBoxShadowList` are owned by
  `arcweft-render-wgpu::view_scene::compositing`.
- `ViewBoxShadowPassPlan` plans shadows back-to-front, supports negative spread,
  canonicalizes transparent/identity shadows through `ViewBoxShadowList`, and emits
  `ViewBoxShadowPlanError::InsetUnsupported` for inset shadows.
- `ViewCompositor::render_group` draws box-shadow passes before rendering group
  children and before filter/clip/mask/blend passes.

The current gap is in `arcweft-takumi-adapter::lowering`:

```rust
box_shadows: ViewBoxShadowList::default(),
```

Takumi's pinned source already exposes `ComputedStyle::box_shadow:
Option<BoxShadows>`, so no production CSS source scanning or local CSS parser is
needed.

## Overlay changed files

### Patched source files

- `crates/arcweft-takumi-adapter/src/lowering.rs`
- `crates/arcweft-takumi-adapter/src/style.rs`
- `crates/arcweft-takumi-adapter/src/coverage.rs`
- `docs/implementation/seq-06.13-css-motion-effects-coverage-matrix.md`

### New test / fixture files

- `crates/arcweft-takumi-adapter/tests/css_box_shadow_lowering.rs`
- `docs/fixtures/css/seq06.13d-box-shadow-card.css`
- `docs/fixtures/native/seq06_13d_box_shadow_smoke.json`
- `docs/fixtures/web/seq06_13d_box_shadow_smoke.json`

## Implementation details

### Lowering

The adapter imports Takumi's typed box-shadow model:

```rust
BoxShadow as TakumiBoxShadow,
```

and lowers it through:

```rust
fn box_shadow_list_from_takumi(
    shadows: Option<&[TakumiBoxShadow]>,
    style: &ComputedStyle,
    sizing: &SizingContext,
    current_color: TakumiColor,
) -> ViewBoxShadowList
```

The conversion preserves CSS list order in `ViewBoxShadowList`. The existing
renderer planner reverses the list for back-to-front paint order.

### Radius scalar

The adapter derives one radius by taking `max(min(rx, ry))` over the four corners.
This keeps mixed and elliptical CSS radii deterministic without changing
`ViewBoxShadow`'s public shape.

### Coverage

`DirectCssFeature::BoxShadow` is added to
`DirectCssSupport::implementation_ready_features()` only after this package adds
end-to-end lowering and tests. `CssCoverageFeature::BoxShadow` is also added to
the coverage matrix as `SupportedNow` with the explicit caveat that inset shadows
are typed diagnostics, not rendered visual support.

### Diagnostics

Seq06.13d intentionally routes unsupported typed values to existing structured
systems:

- malformed or unsupported syntax remains Takumi CSS parse/cascade diagnostics;
- inset remains `ViewBoxShadowPlanError::InsetUnsupported`;
- non-finite fields remain `ViewBoxShadowPlanError::NonFinite`;
- transparent shadows are not diagnostics because they are identity paint.

## Tests implemented in package

`crates/arcweft-takumi-adapter/tests/css_box_shadow_lowering.rs` covers:

1. one outer shadow lowers to `ViewBoxShadowList` with offset, blur, spread, color,
   and radius;
2. multiple shadows preserve CSS list order and compositor plan paints
   back-to-front;
3. negative spread lowers and plans deterministically;
4. transparent shadows canonicalize to an empty list;
5. inset shadows lower to typed data and then produce
   `ViewBoxShadowPlanError::InsetUnsupported`;
6. `filter: drop-shadow(...)` remains `ViewFilter::DropShadow` and leaves
   `box_shadows` empty;
7. direct CSS ready features now include `BoxShadow`.

## Validation

```bash
cargo fmt --all
cargo test -p arcweft-takumi-adapter --test css_box_shadow_lowering --all-features -- --nocapture
cargo test -p arcweft-render-wgpu --all-targets --all-features view_box_shadow -- --nocapture
cargo check -p arcweft-takumi-adapter -p arcweft-render-wgpu --all-targets --all-features
cargo clippy -p arcweft-takumi-adapter -p arcweft-render-wgpu --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
```

All commands above were run in the repository checkout after applying the
package. Structure audit completed successfully and reported the current
workspace hotspot total as `4 error(s), 125 warning(s)`.

## Application adjustments

- The package patch file itself was malformed and could not be applied directly,
  so the changes were applied against the current source by ownership area.
- The source already used `ViewAffine2D` after the UI affine rename; imports were
  kept on that current type name rather than reintroducing `ViewAffine2`.
- The focused test now uses Takumi's typed builders for non-exhaustive
  `BoxShadow` and `TextShadow` values.

## Known platform limits

- No exact cross-GPU visual golden is promoted in this package. The native/web
  fixture records expected pass counts and the shared wgpu route; exact pixels
  still depend on the existing pinned-adapter golden harness policy.
- Inset box-shadow remains a structured diagnostic until an inset renderer pass
  is explicitly designed and implemented.
- Per-corner box-shadow radii remain deterministic scalar lowering until
  `ViewBoxShadow` grows a per-corner radius contract.

## Design deviations

None from the seq06.13d request. The renderer substrate is not redesigned.

## Remaining TODOs

- Promote visual smoke to pinned native/web golden once the repo's visual harness
  has stable adapter/device readback for this path.
- Add a future request for inset shadow rendering if product needs it.
