# seq06.13e CSS Inset Box-Shadow Rendering Implementation Note — 2026-07-03

## Source assumptions

This overlay was authored against `Sanzentyo/arcweft` main as inspected through
the GitHub connector at revision `7811b2de8da312116b621348b581b05c316cb443`.
The inspected repository already contained:

- `UiBoxShadow`, `UiBoxShadowKind`, and `UiBoxShadowList` in
  `arcweft-render-wgpu::ui_scene`;
- `UiBoxShadowPassPlan` in `ui_box_shadow` with outer shadow planning and
  `InsetUnsupported` diagnostics;
- the direct wgpu `UiCompositor` path with box-shadow passes before children;
- WGSL `PASS_BOX_SHADOW` for outer shadow coverage;
- Takumi adapter tests proving typed `inset` lowering into
  `UiBoxShadowKind::Inset`.

The overlay follows the repository rules in `AGENTS.md`: behavior is added to
Arcweft-owned types, renderer work stays in `arcweft-render-wgpu`, implementation
notes stay under `docs/implementation/`, and no compatibility/fallback rendering
route is introduced.

## Changed files

### Renderer and shader

- `crates/arcweft-render-wgpu/src/ui_scene/compositing.rs`
- `crates/arcweft-render-wgpu/src/ui_box_shadow.rs`
- `crates/arcweft-render-wgpu/src/ui_compositor.rs`
- `crates/arcweft-render-wgpu/src/ui_compositor_uniform.rs`
- `crates/arcweft-render-wgpu/src/ui_shaders/compositor.wgsl`
- `crates/arcweft-render-wgpu/Cargo.toml`

### Tests

- `crates/arcweft-render-wgpu/tests/ui_box_shadow_plan.rs`
- `crates/arcweft-render-wgpu/tests/ui_box_shadow_gpu_smoke.rs`
- `crates/arcweft-takumi-adapter/tests/css_box_shadow_lowering.rs`

### Docs and fixtures

- `docs/design/seq-06.13e-css-inset-box-shadow-rendering-design.md`
- `docs/implementation/seq-06.13e-css-inset-box-shadow-rendering-2026-07-03.md`
- `docs/implementation/seq-06.13-css-motion-effects-coverage-matrix.md`
- `docs/implementation/seq-06.13e-css-inset-box-shadow-support-matrix.md`
- `docs/fixtures/css/seq06.13e-inset-box-shadow-card.css`
- `docs/fixtures/native/seq06_13e_inset_box_shadow_smoke.json`
- `docs/fixtures/web/seq06_13e_inset_box_shadow_smoke.json`

## Implementation summary

### Plan model

`UiBoxShadowPassPlan` remains the single plan type for a CSS box-shadow list. It
now accepts both `UiBoxShadowKind::Outer` and `UiBoxShadowKind::Inset`, exposes
`passes_for_kind(kind)`, and records `visual_inset_px()` metadata alongside
`visual_outset_px()`.

Outer planning is unchanged. Inset planning creates an inner clear/caster rect by
applying `outset_rect(bounds, -spread_radius_px)` and then applying the shadow
offset. Positive spread deflates the clear rect; negative spread expands it.

### Diagnostics

`InsetUnsupported` is removed. `NonFinite` is preserved. The new
`DegenerateGeometry` diagnostic rejects non-empty inset shadows when the receiver
bounds have no drawable area.

### Scene-owned behavior

`UiBoxShadowKind` gains inherent `is_outer()` / `is_inset()` methods. This keeps
kind behavior on the Arcweft-owned enum rather than scattering local helper
matches. `UiBoxShadow::is_identity()` now recognizes zero inset shadows as
canonical identity while preserving non-finite data for planner diagnostics.

### Compositor order

`UiCompositor::render_group` computes the box-shadow plan once, draws outer
passes before children, draws child nodes, then draws inset passes before filters,
clip-path, masks, backdrop filtering, opacity, and blend.

### WGSL / uniform contract

`PASS_BOX_SHADOW` remains the shader pass. `params0.w` carries the kind flag:
`0.0` for outer, `1.0` for inset. The shader chooses between:

- outer: `caster * (1.0 - body)`;
- inset: `body * (1.0 - caster)`.

No bind group layout, pipeline, texture binding, or new pass enum is added.

## Tests added or updated

Focused analytic coverage includes:

- inset no longer returns `InsetUnsupported`;
- deterministic inset body/caster geometry and `visual_inset_px()` metadata;
- multiple inset CSS-list ordering;
- negative spread geometry;
- transparent and zero inset canonical no-pass behavior;
- non-finite typed diagnostics;
- degenerate zero-area receiver diagnostics;
- mixed outer+inset ordering across compositor stages;
- preservation of existing outer shadow tests;
- Takumi adapter coverage proving typed CSS `inset` reaches the renderer plan;
- an ignored GPU smoke test for one rounded inset card and one mixed outer+inset
  card.

## Validation commands

Run from repository root after applying the patch:

```bash
cargo fmt --all -- --check
cargo test -p arcweft-render-wgpu --test ui_box_shadow_plan --all-features -- --nocapture
cargo test -p arcweft-takumi-adapter --test css_box_shadow_lowering --all-features -- --nocapture
cargo test -p arcweft-render-wgpu --test ui_box_shadow_gpu_smoke --all-features -- --ignored --nocapture
cargo check -p arcweft-render-wgpu -p arcweft-takumi-adapter --all-targets --all-features
cargo clippy -p arcweft-render-wgpu -p arcweft-takumi-adapter --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root . --write target/seq06_13e_structure_audit
git diff --check
```

## Apply-time adjustments in this checkout

The overlay applied cleanly, then the following checkout-local fixes were made:

- `arcweft-render-wgpu` now enables native `wgpu` backend features through a
  dev-dependency for tests. Without this, the ignored GPU smoke test panicked at
  `wgpu::Instance::new` before adapter probing because the crate-level test
  command did not enable any implemented platform backend.
- `UiCompositor::render_box_shadows` now returns `()` instead of
  `Result<(), UiCompositorError>` because the helper only enqueues shader passes;
  clippy correctly reported the wrapper as unnecessary.
- A new test assertion uses an epsilon check for a `f32` zero comparison.

These are validation fixes, not design deviations from the seq06.13e contract.

## Package-authoring validation result

The local container could not clone `https://github.com/Sanzentyo/arcweft.git`
because DNS resolution for `github.com` was unavailable. It also does not have a
repository checkout suitable for Rust compilation or GPU execution. Therefore,
this package does not claim local `cargo fmt`, `cargo test`, `cargo check`,
`cargo clippy`, structure-audit, or GPU smoke success.

The implementation has been statically assembled against connector-inspected
source and includes the validation commands that must be run in a real checkout.

## Applied checkout validation result

Validated in `D:\git\arcweft` on 2026-07-04 after applying the package:

- `cargo fmt --all -- --check`: pass.
- `cargo test -p arcweft-render-wgpu --test ui_box_shadow_plan --all-features -- --nocapture`: pass, 4 tests.
- `cargo test -p arcweft-takumi-adapter --test css_box_shadow_lowering --all-features -- --nocapture`: pass, 7 tests.
- `cargo test -p arcweft-render-wgpu --test ui_box_shadow_gpu_smoke --all-features -- --ignored --nocapture`: pass, 1 ignored GPU smoke test executed.
- `cargo check -p arcweft-render-wgpu -p arcweft-takumi-adapter --all-targets --all-features`: pass.
- `cargo clippy -p arcweft-render-wgpu -p arcweft-takumi-adapter --all-targets --all-features -- -D warnings`: pass.
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write target/seq06_13e_structure_audit`: pass; wrote reports under `target/seq06_13e_structure_audit`.
- `git diff --check`: pass.

The structural audit reported 4 existing error-level file-size violations and
125 warnings across the workspace. No audit violation matched the changed
seq06.13e files. Changed Rust file measurements from the audit:

| Path | Bytes | Physical LOC | Classification |
| --- | ---: | ---: | --- |
| `crates/arcweft-render-wgpu/src/ui_box_shadow.rs` | 12,905 | 394 | generated |
| `crates/arcweft-render-wgpu/src/ui_compositor.rs` | 39,020 | 1,103 | production |
| `crates/arcweft-render-wgpu/src/ui_compositor_uniform.rs` | 11,287 | 350 | production |
| `crates/arcweft-render-wgpu/src/ui_scene/compositing.rs` | 21,799 | 835 | production |
| `crates/arcweft-render-wgpu/tests/ui_box_shadow_gpu_smoke.rs` | 5,275 | 156 | test |
| `crates/arcweft-render-wgpu/tests/ui_box_shadow_plan.rs` | 3,985 | 129 | test |
| `crates/arcweft-takumi-adapter/tests/css_box_shadow_lowering.rs` | 7,380 | 275 | test |

## Structural audit expectation

This change modifies production renderer planning, compositor orchestration, and
WGSL. A structural audit is required after apply. Expected changed production
files remain cohesive responsibility modules; if local line counts exceed
AGENTS.md thresholds after merge with concurrent work, split by responsibility
rather than adding wrapper helpers or compatibility shims.

## Remaining TODOs

- Promote exact native/web PNG goldens only in the pinned visual-golden
  environment.
- Keep per-corner / elliptical shadow radii as a separate renderer contract
  extension; this package preserves the existing scalar radius contract.
