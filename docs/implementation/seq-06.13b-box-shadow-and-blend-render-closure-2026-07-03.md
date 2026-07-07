# seq06.13b Box Shadow and Blend Render Closure Implementation

## Acceptance Mapping

| Request item | Current implementation |
| --- | --- |
| Decide box-shadow route | Arcweft-owned compositor group effect with pure planning in `view_box_shadow.rs`. |
| Outer shadow | `ViewBoxShadowKind::Outer`, `ViewBoxShadowPassPlan`, and `PASS_BOX_SHADOW`. |
| Inset shadow | `ViewBoxShadowPlanError::InsetUnsupported`. |
| Spread / negative spread | `ViewBoxShadowPass::from_outer_shadow` expands or shrinks the caster rect. |
| Multiple shadows | Planner reverses CSS list order while preserving `shadow_index`. |
| Rounded corners | Body and shadow radii are carried into compositor uniforms. |
| Transparent colors | `ViewBoxShadow::is_identity` removes no-op shadows. |
| Opacity/filter/clip/mask/blend interaction | `ViewCompositor::render_group` draws shadows before children, then runs the existing group pass chain. |
| HSL/luminosity blend modes | Existing `ViewBlendShaderMode` and WGSL HSL branch are preserved by tests. |
| No DOM/CSS/canvas fallback | No fallback path was added; the implementation stays in typed scene data, pass planning, and WGSL. |

## Changed Files

- `crates/arcweft-render-wgpu/src/lib.rs`
- `crates/arcweft-render-wgpu/src/view_scene.rs`
- `crates/arcweft-render-wgpu/src/view_scene/compositing.rs`
- `crates/arcweft-render-wgpu/src/view_box_shadow.rs`
- `crates/arcweft-render-wgpu/src/view_compositor.rs`
- `crates/arcweft-render-wgpu/src/view_compositor_uniform.rs`
- `crates/arcweft-render-wgpu/src/view_shaders/compositor.wgsl`
- `crates/arcweft-render-wgpu/tests/view_box_shadow_plan.rs`
- `crates/arcweft-takumi-adapter/src/style.rs`
- `crates/arcweft-takumi-adapter/src/lowering.rs`
- `docs/design/seq-06.13b-box-shadow-and-blend-render-closure-design.md`
- `docs/implementation/seq-06.13b-box-shadow-and-blend-render-closure-2026-07-03.md`
- `docs/implementation/seq-06.13b-support-matrix.md`

## Integration Notes

CSS/Takumi lowering still needs to attach parsed `box-shadow` values to
`ViewCompositingEffects::box_shadows`. This cut intentionally stops at the
renderer/compositor closure because broad CSS cascade/lowering is owned by the
seq06.11/06.12/06.13 style pipeline.

The Takumi adapter now classifies `box-shadow` as a compositing invalidation
instead of paint-only. Actual value extraction is still absent, so
`compositing_effects_from_takumi` emits an empty `ViewBoxShadowList` until that
follow-up is designed.

Follow-up request:

- `docs/reviews/requests/2026-07-03-seq-06.13d-css-box-shadow-lowering-package.md`

The package included a source-gate test for forbidden fallback strings. That
test was not added because it would be a brittle textual scan rather than a
typed renderer contract. The no-fallback rule is instead recorded here and
enforced by keeping the implementation entirely in the `ViewScene` /
`ViewCompositor` / WGSL path.

## Validation Commands

```bash
cargo fmt --all -- --check
cargo test -p arcweft-render-wgpu view_box_shadow --all-features -- --nocapture
cargo test -p arcweft-render-wgpu --test view_box_shadow_plan --all-features -- --nocapture
cargo check -p arcweft-render-wgpu --all-targets --all-features
cargo clippy -p arcweft-render-wgpu --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
```

## Local Validation

- `cargo fmt --all -- --check`
- `cargo check -p arcweft-render-wgpu -p arcweft-takumi-adapter --all-targets --all-features`
- `cargo test -p arcweft-render-wgpu view_box_shadow --all-features -- --nocapture`
- `cargo test -p arcweft-render-wgpu --test view_box_shadow_plan --all-features -- --nocapture`
- `cargo test -p arcweft-takumi-adapter compositing_properties_are_not_generic_unsupported_direct --all-features -- --nocapture`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write target\structure-audit-seq06-13b`
- `git diff --check`

Structural audit completed with existing workspace findings:
`4 error(s), 125 warning(s)`. The new `view_box_shadow.rs` is 222 physical LOC,
`view_scene/compositing.rs` is 746 physical LOC, and `view_compositor.rs` is 1080
physical LOC, so this cut does not introduce a new threshold violation.

## Known Limitations

- Inset `box-shadow` is a typed unsupported diagnostic.
- Exact browser Gaussian parity is not claimed.
- CSS/Takumi lowering into `ViewBoxShadowList` is still follow-up work.
