# seq06.13 CSS Motion and Advanced Effects Coverage Design

## Scope

This design is the broad seq06.13 cut. The narrower seq06.13a clip/mask closure
is specified in `seq-06.13a-clip-path-mask-render-closure-design.md` and shares
this package's renderer policy.

The first cut keeps Arcweft on Arcweft-owned retained UI and wgpu/native-web
shared paths. It does not introduce browser DOM rendering, CSS canvas fallback,
CPU-rasterized Takumi output, or screenshot-derived layout input.

## Ownership model

| Responsibility | Owner in this cut | Reason |
| --- | --- | --- |
| CSS syntax/cascade parsing | `arcweft-takumi-adapter` boundary | Takumi remains the CSS/layout source for adapter input. Unsupported CSS values must be lowered as typed unsupported diagnostics, not silently ignored. |
| Motion evaluation | `arcweft-ui::motion` plus player/runtime timeline input | Motion is style/value semantics and must be deterministic/Sans I/O. The player provides sampled timestamps. |
| Effect pass planning | `arcweft-render-wgpu` pure planning modules | Filter, clip, mask, and blend pass graphs are renderer data. |
| GPU execution | `arcweft-render-wgpu` compositor shader path | Native and web share wgpu/WGSL behavior. |
| Resource acquisition | native/web player adapter/resource tables | Data-format crates and renderer planning do not perform file or network I/O. |

## Motion model

### Timeline source

Motion never reads wall-clock time. The host/player samples a monotonic player
presentation timeline and passes `UiTimelineMillis` to transitions and keyframe
tracks. This lets native and web captures evaluate the same frame at the same
logical timestamp.

### Transitionable property set

The first transitionable set is deliberately paint-only:

- `Opacity`;
- `TranslateX`;
- `TranslateY`;
- `Scale`;
- `Rotate`;
- `Color`;
- `BackgroundColor`;
- `PlaceholderColor`;
- `SelectionColor`;
- `CaretColor`;
- `CompositionUnderlineColor`;
- `OutlineColor`;
- `OutlineWidth`;
- `BorderRadius`.

Layout-affecting properties (`Width`, `Height`, `Display`, `FontSize`) are not
transitionable in this cut because seq06.12 owns CSS layout/cascade coverage and
seq06.11/retained-frame integration owns interaction-state product wiring.

### Interpolation rules

Interpolation behavior is added to the Arcweft-owned boundary types:

- `Milli::lerp` clamps progress to `0..=1000` and uses saturating integer math;
- `Rgba8::lerp` interpolates each channel in 8-bit sRGB channel space;
- `UiPropertyKind::is_transitionable` owns the property-family decision;
- `UiPropertyKind::interpolate_value` rejects system colors, resources, booleans,
  incompatible value families, and non-transitionable kinds.

No extension trait or stringly helper is used for these rules.

### Easing functions

The implemented easing set is:

- `linear`;
- CSS keyword aliases `ease`, `ease-in`, `ease-out`, `ease-in-out`;
- explicit `cubic-bezier(x1, y1, x2, y2)`;
- `steps(n, jump-start | jump-end)`.

Cubic-bezier sampling uses fixed-iteration bisection over the x curve, producing
stable deterministic output without a new dependency.

### Keyframes

`UiKeyframeTrack` is per-property. A track contains ordered `UiKeyframe` values,
each with:

- normalized offset in `Milli` progress units;
- typed `UiPropertyValue`;
- easing used from this keyframe to the next keyframe.

The track sorts and clamps offsets on construction, requires at least two
keyframes, and rejects non-transitionable properties or incompatible value
families.

### Interruption and reversal

When a new target arrives while a transition is running, the runtime/player calls
`UiTransition::interrupt`. The old transition is sampled at the interruption
timestamp and the new transition starts from that sampled value. Reversal is not
a separate path; it is the same interruption rule with a previous value as the
new target.

### Reduced motion

The host/player chooses one of three policies:

- `Full`: use author duration and easing;
- `Shorten { max_duration_ms }`: clamp duration while preserving easing;
- `Disable`: duration becomes zero and the first sample jumps to the target.

The policy is applied at sampling time so the same transition spec can be reused
for normal, accessibility, and deterministic-capture runs.

### Motion evidence

Each sample returns `UiMotionSample`:

- property;
- timestamp;
- source value;
- target value;
- sampled value;
- linear progress;
- eased progress;
- finished flag.

Visual smoke fixtures use fixed timestamps such as `0 ms`, `125 ms`, `250 ms`,
`500 ms`, and `1000 ms` so renderer drift packets can cite exact expected state.

## Advanced effect support decisions

| Effect family | Decision |
| --- | --- |
| `box-shadow` | Designed but deferred as seq06.13b. The intended route is direct rounded-rect spread geometry for non-blurred shadows and compositor alpha/blur passes for blurred shadows. This package keeps unsupported box-shadow values as structured diagnostics rather than pretending `filter: drop-shadow` is parity. |
| `clip-path: inset/circle/ellipse/polygon` | Implemented through an analytic clip shader pass in the wgpu compositor. |
| `clip-path: path(...)` | Explicit unsupported diagnostic until a vector tessellator is selected. |
| `clip-path: url(...)` | Explicit unsupported diagnostic at the CSS/Takumi adapter boundary. No SVG/browser fallback. |
| `filter: url(...)` | Explicit unsupported diagnostic at the CSS/Takumi adapter boundary. No SVG/browser fallback. |
| Mask URL images | Supported as external mask textures through the renderer/player resource provider. File/network loading stays outside data crates. |
| Mask gradients | Explicit unsupported diagnostic for this cut. They need a gradient-mask resource/shader contract. |
| `mask: element(...)` | Explicit unsupported diagnostic for this cut. |
| `mask-size`, `mask-position`, `mask-repeat` | Implemented for `auto`, `cover`, `contain`, explicit lengths/percentages, `repeat`, `no-repeat`, `repeat-x`, and `repeat-y`. `space` and `round` remain diagnostics. |
| Alpha vs luminance mask mode | Implemented through `UiMaskChannel`; luminance uses Rec.709 luma multiplied by mask alpha. |
| HSL-family blend modes | Implemented for `hue`, `saturation`, `color`, and `luminosity` using a documented non-premultiplied sRGB HSL rule before source-over composition. |

## Diagnostics policy

Unsupported effects must remain typed and visible:

- `UiClipPathPlanError::PathUnsupported` for CSS `path()`;
- `UiClipPathPlanError::Unsupported` for URL or otherwise unsupported clip-path
  lowerings;
- `UiMaskPlanError` for unsupported mask images, sizes, positions, and repeats;
- `UiEffectPass::Unsupported` and `UiCompositorError::UnsupportedFilter` for
  unsupported filters such as `url(...)`;
- follow-up `UiBoxShadowPlanError` is recommended for seq06.13b.

A value that affects final pixels must either produce a pass/resource plan or a
structured diagnostic. It must not disappear while leaving the UI rendered as if
no effect was requested.

## Test strategy

Focused deterministic tests cover:

- transition interpolation for background color, opacity, scale, and outline
  width;
- reduced-motion shorten and disable behavior;
- interruption/reversal source values;
- keyframe interpolation;
- unsupported URL-like filters and clips as diagnostics;
- deterministic clip/mask/blend pass planning;
- existing seq06.9 compositor plan tests continuing to pass;
- ignored GPU smoke captures at multiple timestamps for native/web pinned
  adapters.
