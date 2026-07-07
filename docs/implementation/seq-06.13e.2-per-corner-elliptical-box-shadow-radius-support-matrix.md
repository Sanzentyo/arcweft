# seq06.13e.2 Per-Corner / Elliptical Box-Shadow Radius Support Matrix

| Area | Status | Evidence in package | Notes |
| --- | --- | --- | --- |
| Public renderer radius model | Implemented | `ViewBoxShadowRadii`, `ViewBoxShadowCornerRadius` | Owned by `arcweft-render-wgpu::view_scene`. |
| Retained rounded fill radius model | Implemented | `ViewCornerRadii`, `ViewCornerRadius`, `ViewRoundedRect` | Surface fill preserves per-corner/elliptical radii instead of collapsing to one circular radius. |
| Scalar constructor migration | Implemented | `ViewBoxShadow::outer` / `inset` | Constructors map to uniform circular radii; no duplicate scalar pass fields. |
| Per-corner CSS lowering | Implemented | `box_shadow_border_radii` | Lowers four Takumi computed `SpacePair<Length>` fields. |
| Elliptical CSS lowering | Implemented | `box_shadow_corner_radius_from_takumi` | Preserves independent `x_px` and `y_px`. |
| Outer planner radii | Implemented | `ViewBoxShadowPass::from_outer_shadow` | Body and caster radii are typed. |
| Inset planner radii | Implemented | `ViewBoxShadowPass::from_inset_shadow` | Same contract as outer; spread direction differs. |
| Oversized normalization | Implemented | `ViewBoxShadowRadii::clamped_to_rect` | CSS corner-overlap scaling after validation. |
| Negative direct radius | Typed diagnostic | `ViewBoxShadowPlanError::DegenerateRadius` | Direct renderer inputs are not silently clamped. |
| Non-finite radius | Typed diagnostic | `ViewBoxShadowPlanError::NonFiniteRadius` | Non-empty invalid shadows survive canonicalization for diagnostics. |
| Mixed-corner spread/blur | Implemented | planner tests + WGSL 9-tap caster sampling | Blur samples typed caster coverage; it does not collapse radii. |
| WGSL per-corner circular/elliptical math | Implemented | `rounded_rect_coverage_at(... radii0, radii1)` | Circular is `rx == ry`; elliptical uses normalized ellipse equation. |
| Unified outer/inset shader path | Preserved | `PASS_BOX_SHADOW`, `params0.w` kind flag | No new pipeline or bind group layout. |
| GPU smoke fixture | Updated | `view_box_shadow_gpu_smoke.rs`, native/web JSON fixtures | Covers mixed-corner outer + elliptical inset; ignored unless local adapter exists. |
| Exact PNG promotion | Deferred | implementation note | Must run only in pinned visual-golden environment. |
| DOM/canvas/SVG/CPU fallback | Not added | source edits | Explicit non-goal. |
