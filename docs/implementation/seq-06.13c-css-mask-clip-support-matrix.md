# seq06.13c CSS Clip/Mask Support Matrix

| Area | seq06.13c status | Evidence | Notes |
| --- | --- | --- | --- |
| `clip-path: inset(...)` | Implemented, unchanged | `UiClipGeometryPlan::Inset`, WGSL | Seq06.13a retained. |
| `clip-path: circle(...)` | Implemented, unchanged | `UiClipGeometryPlan::Ellipse`, WGSL | Circle lowers through ellipse plan. |
| `clip-path: ellipse(...)` | Implemented, unchanged | `UiClipGeometryPlan::Ellipse`, WGSL | Seq06.13a retained. |
| `clip-path: polygon(...)` | Implemented, unchanged | `UiClipGeometryPlan::Polygon`, WGSL | Up to 16 vertices, nonzero/evenodd. |
| `clip-path: path(...)` | Implemented subset | `UiClipGeometryPlan::Path`, path tests | M/L/H/V/Q/C/Z, relative variants, deterministic curve flattening. |
| Path arcs/smooth shorthand | Structured diagnostic | `UnsupportedPathCommand` | Deferred to a later path parser expansion. |
| Degenerate path segments | Structured diagnostic | `DegeneratePathSegment` | No silent no-op for drawable zero-length segments. |
| Oversized path geometry | Structured diagnostic | `TooManyPathCommands`, `TooManyPathEdges` | Fixed shader uniform budget. |
| `clip-path: url(...)` | Typed unsupported diagnostic | `UiClipPath::Url`, `UrlClipResourceUnsupported` | Reusable vector clip resources are not in this cut. |
| External URL mask image | Implemented, unchanged | `UiMaskImage::Url`, `UiMaskTextureProvider` | Resource acquisition remains outside planning. |
| Gradient mask image, retained UI | Implemented subset | `UiMaskImage::Gradient`, WGSL generated coverage | Linear/radial/conic retained types. |
| Gradient mask image, CSS/Takumi | Linear implemented, radial/conic diagnostic | `lowering.rs` overlay | Non-repeating linear only in adapter cut. |
| Gradient color stops | Implemented subset | `UiMaskGradientPlan` | 2..=8 stops, deterministic coverage interpolation. |
| Alpha mask mode | Implemented | texture and gradient mask tests | `coverage = a`. |
| Luminance mask mode | Implemented | texture and gradient mask tests | Rec.709 luminance multiplied by alpha. |
| `mask-size: auto/cover/contain/explicit` | Implemented, extended to gradients | `UiMaskPassPlan::sampling_plan` | Gradients use source extent as intrinsic for auto. |
| `mask-position` | Implemented | `UiMaskSamplingPlan` | Anchor resolves against free space for no-repeat/repeat. |
| `mask-repeat: repeat/no-repeat/repeat-x/repeat-y` | Implemented, unchanged | `UiMaskAxisRepeat` | Boolean evidence retained. |
| `mask-repeat: space` | Implemented | `UiMaskAxisRepeat::Space` | Deterministic count/stride. |
| `mask-repeat: round` | Implemented | `UiMaskAxisRepeat::Round` | Deterministic tile resize/count. |
| `mask: element(...)` | Typed unsupported diagnostic | `UiMaskImage::Element`, `ElementMaskCaptureUnavailable` | Requires future typed element capture resource graph. |
| Browser DOM/SVG/canvas fallback | Not used | Design constraint | No hidden fallback route. |
| Native/web visual smoke | Optional ignored | fixture docs | Drift thresholds documented; not required until pinned readback. |
