# seq06.13e CSS Inset Box-Shadow Support Matrix

> **Superseded (2026-07-13):** The CSS/Takumi source path described below was
> removed by the [native-only typed Style path](native-only-style-path-2026-07-13.md).
> Native typed Style still uses the generic inset-shadow renderer, while this
> table remains only as historical implementation evidence.

| Area | Status | Evidence in package | Notes |
| --- | --- | --- | --- |
| Typed source | Implemented by seq06.13d | `ViewBoxShadowKind::Inset` from Takumi adapter | Not redesigned in seq06.13e. |
| Inset renderer plan | Implemented | `ViewBoxShadowPassPlan::from_shadows` | No `InsetUnsupported` path remains. |
| Unified outer/inset pass list | Implemented | `passes_for_kind(ViewBoxShadowKind)` | Keeps CSS list order and lets compositor draw two stages. |
| Inset pass order | Implemented | `ViewCompositor::render_group` | Outer before children; inset after children and before filters/clip/mask/blend. |
| Positive spread | Implemented | inset geometry tests | Deflates inner clear/caster rect. |
| Negative spread | Implemented | inset geometry tests | Expands inner clear/caster rect deterministically. |
| Rounded inset | Implemented | radius uniforms + ignored GPU smoke | Uses existing scalar radius contract. |
| Zero/transparent inset | Implemented identity | `ViewBoxShadow::is_identity`, tests | No pass is emitted. |
| Non-finite diagnostics | Implemented | `ViewBoxShadowPlanError::NonFinite`, tests | Non-finite data is preserved through canonicalization for planner diagnostics. |
| Degenerate receiver | Implemented diagnostic | `ViewBoxShadowPlanError::DegenerateGeometry`, tests | Non-empty zero-area inset receiver is rejected. |
| Shader contract | Implemented | `PASS_BOX_SHADOW`, `params0.w` kind flag | No new pipeline or bind group layout. |
| `filter: drop-shadow(...)` separation | Preserved | existing Takumi adapter test | Remains `ViewFilter::DropShadow`. |
| GPU smoke | Implemented/ignored and locally passed | `view_box_shadow_gpu_smoke.rs` | Requires local wgpu adapter; passed in the 2026-07-04 apply checkout. |
| Exact PNG promotion | Gated no-promotion until pinned evidence | `seq06.13e.1-inset-box-shadow-exact-png-policy.json` + seq06.13e.1 implementation note | Exact native/Web baselines must still be promoted only from pinned visual-golden jobs; unpinned packages claim no PNG promotion. |
