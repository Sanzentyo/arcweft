# seq06.13 / seq06.13a Implemented Coverage Matrix

| Area | Status | Evidence in package | Notes |
| --- | --- | --- | --- |
| Transitionable property set | Implemented | `arcweft-view/src/motion.rs`, `style.rs` patch | Paint-only properties. Layout properties are excluded. |
| Background color interpolation | Implemented | `motion_transitions.rs` | Channel interpolation through `Rgba8::lerp`. |
| Opacity interpolation | Implemented | `motion_transitions.rs` | `Milli` interpolation. |
| Transform scale interpolation | Implemented | `motion_transitions.rs` | `ViewPropertyKind::Scale`. Translate/rotate use same `Milli` path. |
| Outline width interpolation | Implemented | `motion_transitions.rs` | Included because it is paint invalidation in current style model. |
| Easing functions | Implemented | `ViewEasingFunction` | Linear, CSS ease aliases, cubic-bezier, steps. |
| Keyframes | Implemented | `ViewKeyframeTrack` | Per-property track with ordered offsets. |
| Interruption/reversal | Implemented | `ViewTransition::interrupt`, tests | New transition starts from sampled current value. |
| Reduced motion | Implemented | `ViewReducedMotionPolicy`, tests | Full, Shorten, Disable. |
| Animation evidence | Implemented | `ViewMotionSample` | Timestamp, source/target/sampled value, progress fields. |
| Renderer ownership separation | Implemented by design | design docs | Motion is in `arcweft-view`; GPU effects are in `arcweft-render-wgpu`. |
| `clip-path: inset(...)` pixels | Implemented | `apply_clip_plan`, WGSL, tests | Analytic shader pass. |
| Circle/ellipse clip pixels | Implemented | `apply_clip_plan`, WGSL, tests | Circle lowered through ellipse plan. |
| Polygon clip pixels | Implemented | `apply_clip_plan`, WGSL, tests | Up to 16 vertices; nonzero/evenodd fill. |
| `clip-path: path(...)` | Explicit diagnostic | `ViewClipPathPlanError::PathUnsupported` | Deferred until tessellator selection. |
| `clip-path: url(...)` | Explicit diagnostic | `ViewClipPath::Unsupported` path | CSS/Takumi adapter should lower to unsupported. |
| `filter: url(...)` | Explicit diagnostic | existing `ViewEffectPass::Unsupported` behavior | No SVG/browser fallback. |
| External mask resource provider | Implemented | `ViewMaskTextureView { extent }` patch | Resource loading remains in renderer/player resource tables. |
| `mask-size` | Implemented subset | `ViewMaskPassPlan::sampling_plan`, tests | Auto, cover, contain, explicit px/percent. |
| `mask-position` | Implemented subset | `ViewMaskPassPlan::sampling_plan`, tests | Position anchor resolves against free space. |
| `mask-repeat` | Implemented subset | `ViewMaskPassPlan::sampling_plan`, tests | repeat, no-repeat, repeat-x, repeat-y. |
| `mask-repeat: space/round` | Explicit diagnostic | `ViewMaskPlanError::UnsupportedRepeat` | Deferred exact distribution. |
| Alpha mask mode | Implemented | WGSL `mask_coverage`, tests | Uses alpha. |
| Luminance mask mode | Implemented | WGSL `mask_coverage`, tests | Rec.709 luma multiplied by alpha. |
| Gradient masks | Explicit diagnostic/deferred | design docs | Needs shader/resource contract. |
| `mask: element(...)` | Explicit diagnostic/deferred | design docs | Needs element capture contract. |
| HSL blend modes | Implemented first cut | `view_blend.rs` patch, WGSL, tests | Non-premultiplied sRGB HSL rule. |
| `box-shadow` renderer parity | Implemented seq06.13b + seq06.13e validation gate | `view_box_shadow.rs`, compositor pass, WGSL, `view_box_shadow_plan.rs`, ignored GPU smoke | Renderer substrate supports outer/multiple/negative spread and typed inset rendering after seq06.13e focused tests and GPU smoke pass. |
| CSS `box-shadow` lowering | Implemented seq06.13d | `arcweft-takumi-adapter::lowering`, `css_box_shadow_lowering.rs` | Takumi computed `BoxShadows` lower to `ViewBoxShadowList`; `filter: drop-shadow(...)` remains separate. |
| CSS `box-shadow: inset ...` direct visual rendering | Verification-gated seq06.13e | `ViewBoxShadowKind::Inset`, unified pass plan, `PASS_BOX_SHADOW` kind flag, `view_box_shadow_gpu_smoke.rs` | Treat as supported only after focused renderer/Takumi tests and ignored GPU smoke pass in a real checkout; exact PNG promotion remains manual. |
| Per-corner / elliptical `box-shadow` radii | Implemented seq06.13e.2 | `ViewBoxShadowRadii`, Takumi corner lowering, planner diagnostics, WGSL per-corner coverage | Scalar constructors remain convenience APIs; exact PNG promotion remains pinned-environment only. |
| Native/web visual smoke at timestamps | Specified | `view_compositor_gpu_smoke_timestamps.rs`, fixture docs | Ignored until pinned GPU golden harness is available. |
| Existing seq06.9 compositor tests | Preserved by intent | validation commands | Run existing `view_compositor_plan` and package tests after apply. |
