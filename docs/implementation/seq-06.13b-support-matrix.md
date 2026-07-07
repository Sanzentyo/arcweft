# seq06.13b Support Matrix

| Feature | Status | Evidence |
| --- | --- | --- |
| Outer `box-shadow` | Implemented | `ViewBoxShadowKind::Outer`, `ViewBoxShadowPassPlan`, `PASS_BOX_SHADOW`. |
| Inset `box-shadow` | Deferred with typed diagnostic | `ViewBoxShadowPlanError::InsetUnsupported`. |
| Multiple shadows | Implemented | Pass plan paints reverse CSS list while preserving `shadow_index`. |
| Spread radius | Implemented | Shadow caster rect expands or shrinks by `spread_radius_px`. |
| Negative spread | Implemented | Collapsed caster becomes no pass; otherwise rect and radius are clamped. |
| Rounded corners | Implemented | `border_radius_px`, body radius, and shadow radius uniforms. |
| Transparent color | Implemented as no-op | `ViewBoxShadow::is_identity`. |
| Non-finite shadow values | Diagnostic | `ViewBoxShadowPlanError::NonFinite`. |
| `filter: drop-shadow(...)` | Preserved | Existing `ViewEffectPass::DropShadow` remains unchanged. |
| HSL blend family | Preserved | `ViewBlendShaderMode::{Hue,Saturation,Color,Luminosity}` tests. |
| Native/web visual parity | Fixture still needed | Shared WGSL path is implemented; pinned capture/golden evidence remains follow-up. |
| Browser DOM/CSS/canvas fallback | Not introduced | No fallback path added; rendering stays in typed scene/compositor/WGSL. |
