# seq06.13d CSS Box-Shadow Support Matrix

| Area | Status | Evidence in package | Notes |
| --- | --- | --- | --- |
| Takumi typed source | Implemented | `ComputedStyle::box_shadow: Option<BoxShadows>` | No new Takumi extension required. |
| One outer shadow | Implemented | `css_box_shadow_lowering.rs` | Offset, blur, spread, color, scalar radius. |
| Multiple shadows | Implemented | `css_box_shadow_lowering.rs` | CSS list order preserved; compositor paints reversed. |
| Negative spread | Implemented | `css_box_shadow_lowering.rs` | Preserved through lowering and planner. |
| Transparent shadow | Implemented identity | `ViewBoxShadowList::new` + test | Empty list / no pass. |
| Rounded border radius | Implemented deterministic scalar | design note + tests | `max(min(rx, ry))` over computed corners. |
| Per-corner exact radius | Deferred | implementation note | Requires renderer contract change. |
| Inset shadow | Structured diagnostic | `ViewBoxShadowPlanError::InsetUnsupported` test | Not visually rendered in seq06.13d. |
| Malformed shadow list | Typed parse diagnostic | Takumi parse path | Not parsed by Arcweft renderer. |
| Unsupported color function | Typed parse/cascade diagnostic | Takumi parse/cascade path | No string scanning. |
| `filter: drop-shadow(...)` | Preserved separate path | test | Lowers to `ViewFilter::DropShadow`. |
| CSS direct-ready feature list | Implemented | `DirectCssFeature::BoxShadow` patch | Outer subset is ready after tests. |
| Coverage property whitelist | Implemented | `CssCoverageFeature::BoxShadow` and `is_supported_property("box-shadow")` patch | `DirectCssSupport::diagnose_css` no longer reports authored outer shadows as a gap. |
| Native/web visual smoke | Specified fixture | JSON smoke manifests | Ignored until pinned golden harness. |
| WGSL renderer changes | Not changed | non-goal | Seq06.13b substrate reused. |
