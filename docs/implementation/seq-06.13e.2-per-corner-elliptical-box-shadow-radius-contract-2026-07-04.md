# seq06.13e.2 Per-Corner / Elliptical Box-Shadow Radius Contract Implementation Note — 2026-07-04

## Source assumptions

This package was authored against the GitHub-connector-inspected `Sanzentyo/arcweft`
`main` source observed at blob URLs under revision
`5cbba06bed6611e44a81d9b3b083988e08f57d23`.

The inspected repository already contained seq06.13d/seq06.13e substrate:

- `ViewBoxShadow`, `ViewBoxShadowKind`, and `ViewBoxShadowList` in
  `crates/arcweft-render-wgpu/src/view_scene/compositing.rs`;
- `ViewBoxShadowPassPlan` in `crates/arcweft-render-wgpu/src/view_box_shadow.rs`;
- `ViewCompositorUniform::box_shadow` and WGSL `PASS_BOX_SHADOW`;
- Takumi adapter lowering from `ComputedStyle::box_shadow`;
- scalar-radius outer/inset planner and smoke tests.

The package follows the current repository policy: Arcweft-owned behavior is
added to Arcweft-owned types, renderer work stays in `arcweft-render-wgpu`,
Takumi adaptation stays in `arcweft-takumi-adapter`, and transient completion
notes live under `docs/implementation/`.

## Changed files

### Renderer/resource boundary

- `crates/arcweft-render-wgpu/src/view_scene.rs`
- `crates/arcweft-render-wgpu/src/view_scene/compositing.rs`
- `crates/arcweft-render-wgpu/src/view_box_shadow.rs`
- `crates/arcweft-render-wgpu/src/view_compositor_uniform.rs`
- `crates/arcweft-render-wgpu/src/view_shaders/compositor.wgsl`

### Takumi adapter

- `crates/arcweft-takumi-adapter/src/lowering.rs`

### Focused tests

- `crates/arcweft-render-wgpu/tests/view_box_shadow_plan.rs`
- `crates/arcweft-render-wgpu/tests/view_box_shadow_gpu_smoke.rs`
- `crates/arcweft-takumi-adapter/tests/css_box_shadow_lowering.rs`

### Docs and fixtures

- `docs/design/seq-06.13e.2-per-corner-elliptical-box-shadow-radius-contract-design.md`
- `docs/implementation/seq-06.13e.2-per-corner-elliptical-box-shadow-radius-contract-2026-07-04.md`
- `docs/implementation/seq-06.13e.2-per-corner-elliptical-box-shadow-radius-support-matrix.md`
- `docs/implementation/seq-06.13-css-motion-effects-coverage-matrix.md`
- `docs/fixtures/css/seq06.13e.2-per-corner-elliptical-box-shadow-card.css`
- `docs/fixtures/native/seq06_13e2_per_corner_elliptical_box_shadow_smoke.json`
- `docs/fixtures/web/seq06_13e2_per_corner_elliptical_box_shadow_smoke.json`

## Implementation summary

### Radius data model

`ViewBoxShadow` now carries `border_radii: ViewBoxShadowRadii`, where each corner is
an independent `ViewBoxShadowCornerRadius { x_px, y_px }`. Existing scalar
constructors remain as explicit convenience constructors and map to uniform
circular radii. New `outer_with_radii` and `inset_with_radii` constructors accept
the typed per-corner contract.

The model is shared by outer and inset shadows. The kind-specific behavior is in
planning and shader coverage, not in separate radius types.

### Takumi lowering

`box_shadow_border_radii` lowers the four computed Takumi radius fields directly:

```rust
ViewBoxShadowRadii::from_corners(
    box_shadow_corner_radius_from_takumi(style.border_top_left_radius, sizing),
    box_shadow_corner_radius_from_takumi(style.border_top_right_radius, sizing),
    box_shadow_corner_radius_from_takumi(style.border_bottom_right_radius, sizing),
    box_shadow_corner_radius_from_takumi(style.border_bottom_left_radius, sizing),
)
```

This removes the scalar `max(min(rx, ry))` collapse from the box-shadow path.
No CSS source string parsing is added.

### Planner changes

`ViewBoxShadowPass` now records:

- `body_radii`: validated and CSS-overlap-normalized radii for the body rect;
- `shadow_radii`: spread-adjusted and normalized radii for the caster rect.

Outer spread adds to each corner axis. Inset spread subtracts from each axis,
matching the existing scalar seq06.13e behavior where positive inset spread
shrinks the caster and negative inset spread expands it.

### Diagnostics

The planner keeps existing scalar non-finite diagnostics for offset/blur/spread
fields and adds:

- `ViewBoxShadowPlanError::NonFiniteRadius` for non-finite corner axes;
- `ViewBoxShadowPlanError::DegenerateRadius` for negative direct renderer radius
  inputs.

Non-empty invalid shadows are therefore not silently dropped. Transparent shadows
and real zero-effect shadows still canonicalize to no-op entries.

### WGSL / uniform packing

The existing `PASS_BOX_SHADOW` shader path is retained. The uniform packs body
and caster radii into currently unused box-shadow slots of `matrix[2]`,
`matrix[3]`, `clip_vertices[0]`, and `clip_vertices[1]`; no bind-group layout or
pipeline is changed.

WGSL coverage now tests the selected corner ellipse instead of a single scalar
rounded-rect SDF. The existing deterministic 9-tap blur approximation samples
that per-corner/elliptical coverage.

## Tests added or updated

- Scalar outer and inset constructor tests remain valid and assert uniform typed
  radii in planner output.
- Takumi lowering tests cover four different corners and elliptical radii without
  scalar collapse.
- Planner tests cover outer per-corner preservation, inset elliptical
  preservation, spread/blur determinism for mixed corners, non-finite radius
  diagnostics, and negative/degenerated radius diagnostics.
- The ignored GPU smoke fixture now includes one mixed-corner outer shadow and
  one elliptical inset shadow.

## Validation commands

Run from repository root after applying the source edits:

```bash
cargo fmt --all -- --check
cargo test -p arcweft-render-wgpu --test view_box_shadow_plan --all-features -- --nocapture
cargo test -p arcweft-takumi-adapter --test css_box_shadow_lowering --all-features -- --nocapture
cargo test -p arcweft-render-wgpu --test view_box_shadow_gpu_smoke --all-features -- --ignored --nocapture
cargo check -p arcweft-render-wgpu -p arcweft-takumi-adapter --all-targets --all-features
cargo clippy -p arcweft-render-wgpu -p arcweft-takumi-adapter --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root . --write target/seq06_13e2_structure_audit
git diff --check
```

## Applied checkout validation result

Performed in the Arcweft checkout on 2026-07-04 after applying the package:

- `cargo fmt --all -- --check` passed.
- `cargo test -p arcweft-render-wgpu --test view_box_shadow_plan --all-features -- --nocapture`
  passed: 9 tests.
- `cargo test -p arcweft-takumi-adapter --test css_box_shadow_lowering --all-features -- --nocapture`
  passed: 9 tests.
- `cargo test -p arcweft-render-wgpu --test view_box_shadow_gpu_smoke --all-features -- --ignored --nocapture`
  passed: 1 ignored GPU smoke test.
- `cargo test -p arcweft-render-wgpu --lib --all-features view_box_shadow -- --nocapture`
  passed: 14 focused unit tests.
- `cargo check -p arcweft-render-wgpu -p arcweft-takumi-adapter --all-targets --all-features`
  passed.
- `cargo clippy -p arcweft-render-wgpu -p arcweft-takumi-adapter --all-targets --all-features -- -D warnings`
  passed.
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write target/seq06_13e2_structure_audit`
  passed with 0 errors and 129 warnings; reports were written under
  `target/seq06_13e2_structure_audit`.
- `git diff --check` passed.

## Package-authoring validation result

Performed in this environment:

- Read the attached seq06.13e.2 request.
- Read the provided Rust Skill completely.
- Read current `AGENTS.md` through the GitHub connector.
- Inspected the current renderer, WGSL, Takumi adapter, tests, support matrices,
  and smoke fixtures through the GitHub connector.
- Attempted direct `git clone https://github.com/Sanzentyo/arcweft.git`; it failed
  because `github.com` DNS resolution is unavailable in the sandbox.
- Created this zip package with an apply script, docs, fixtures, and validation
  notes.
- Performed a static package check: 34 replacement entries, balanced raw string
  delimiters, indentation-preserving replacement helpers, and no single-quoted Rust path/label literals in the apply script.
- Attempted to compile-check the apply script with `rustc`; the sandbox has no
  Rust compiler (`rustc: command not found`).

Not performed in this sandbox:

- `cargo fmt`, `cargo test`, `cargo check`, `cargo clippy`, structure audit, GPU
  smoke execution, exact PNG validation, or Rust-level apply-script compilation,
  because no checkout/dependency graph and no local Rust compiler were available.

## Structural audit result

This change materially updates a renderer boundary type, planner data, uniform
packing, WGSL, and adapter lowering, so `cargo +nightly -Zscript tools/structure-audit.rs --root .`
was run after apply. The changed files remain existing responsibility modules,
and the audit reported no error-threshold violations. If concurrent changes
later push any file beyond AGENTS thresholds, split by real responsibility
rather than adding wrapper helpers.

Audit checkout: Jujutsu change `olslpzlz`, parent `mtrzvkou`
(`2bb9818b`). The file metrics below are from
`target/seq06_13e2_structure_audit/file_metrics.csv`.

| Path | Owning crate | Kind | Bytes | Physical LOC | Embedded test LOC | Major responsibilities |
| --- | --- | --- | ---: | ---: | ---: | --- |
| `crates/arcweft-render-wgpu/src/view_box_shadow.rs` | `arcweft-render-wgpu` | production planner with embedded unit tests | 18,392 | 565 | 312 | box-shadow pass ordering, geometry/radius validation, visual outset/inset planning |
| `crates/arcweft-render-wgpu/src/view_compositor_uniform.rs` | `arcweft-render-wgpu` | production uniform packing | 11,869 | 366 | 0 | compositor uniform construction and box-shadow radii packing |
| `crates/arcweft-render-wgpu/src/view_scene.rs` | `arcweft-render-wgpu` | production facade | 1,671 | 31 | 0 | intentional presentation type re-exports |
| `crates/arcweft-render-wgpu/src/view_scene/compositing.rs` | `arcweft-render-wgpu` | production compositing model with embedded unit tests | 28,031 | 1,059 | 60 | compositing effect data model, typed box-shadow radii, canonicalization helpers |
| `crates/arcweft-render-wgpu/tests/view_box_shadow_gpu_smoke.rs` | `arcweft-render-wgpu` | integration test | 6,087 | 178 | 0 | ignored GPU compositor smoke for direct box-shadow rendering |
| `crates/arcweft-render-wgpu/tests/view_box_shadow_plan.rs` | `arcweft-render-wgpu` | integration test | 8,543 | 278 | 0 | pass planner contract tests for direct CSS box-shadow rendering |
| `crates/arcweft-takumi-adapter/src/lowering.rs` | `arcweft-takumi-adapter` | production adapter with embedded unit tests | 38,016 | 1,159 | 122 | Takumi computed-style lowering into Arcweft presentation/compositing types |
| `crates/arcweft-takumi-adapter/tests/css_box_shadow_lowering.rs` | `arcweft-takumi-adapter` | integration test | 9,870 | 362 | 0 | CSS box-shadow lowering contract tests |

## Remaining TODOs

- Promote exact native/web PNG goldens only in the pinned visual-golden
  environment; this package only updates the smoke contract.

## Design deviations

None from the seq06.13e.2 request. Exact PNG promotion remains an explicit
non-goal for this package.
