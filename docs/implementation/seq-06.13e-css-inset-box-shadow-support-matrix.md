# seq06.13e CSS Inset Box-Shadow Support Matrix

| Area | Status | Evidence in package | Notes |
| --- | --- | --- | --- |
| Typed source | Implemented by seq06.13d | `UiBoxShadowKind::Inset` from Takumi adapter | Not redesigned in seq06.13e. |
| Inset renderer plan | Implemented | `UiBoxShadowPassPlan::from_shadows` | No `InsetUnsupported` path remains. |
| Unified outer/inset pass list | Implemented | `passes_for_kind(UiBoxShadowKind)` | Keeps CSS list order and lets compositor draw two stages. |
| Inset pass order | Implemented | `UiCompositor::render_group` | Outer before children; inset after children and before filters/clip/mask/blend. |
| Positive spread | Implemented | inset geometry tests | Deflates inner clear/caster rect. |
| Negative spread | Implemented | inset geometry tests | Expands inner clear/caster rect deterministically. |
| Rounded inset | Implemented | radius uniforms + ignored GPU smoke | Uses existing scalar radius contract. |
| Zero/transparent inset | Implemented identity | `UiBoxShadow::is_identity`, tests | No pass is emitted. |
| Non-finite diagnostics | Implemented | `UiBoxShadowPlanError::NonFinite`, tests | Non-finite data is preserved through canonicalization for planner diagnostics. |
| Degenerate receiver | Implemented diagnostic | `UiBoxShadowPlanError::DegenerateGeometry`, tests | Non-empty zero-area inset receiver is rejected. |
| Shader contract | Implemented | `PASS_BOX_SHADOW`, `params0.w` kind flag | No new pipeline or bind group layout. |
| `filter: drop-shadow(...)` separation | Preserved | existing Takumi adapter test | Remains `UiFilter::DropShadow`. |
| GPU smoke | Implemented/ignored and locally passed | `ui_box_shadow_gpu_smoke.rs` | Requires local wgpu adapter; passed in the 2026-07-04 apply checkout. |
| Exact PNG promotion | Manual remaining work | fixture JSON + implementation note | No exact visual-golden promotion is claimed here. |
