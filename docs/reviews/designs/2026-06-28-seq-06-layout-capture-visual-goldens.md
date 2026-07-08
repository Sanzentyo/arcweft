# Seq-06 Layout, Capture, and Visual Goldens Contract

Date: 2026-06-28

## Current implementation evidence

The current repository already has a Sans I/O `arcweft-layout` crate with typed
`LayoutSize`, `LayoutPoint`, `LayoutRect`, `ScalePolicy`, `ContentRect`,
layout unit expressions, safe-area evaluation context, text overflow/fitting
contracts, and diagnostics. Native Agent observation consumes `arcweft-layout`
and currently publishes a raw-mode `layout.viewport_scale` scene-graph entry.
The earlier implementation note records that fit primitives moved out of
`arcweft-render-wgpu`, that raw/contain/cover/stretch and inverse mapping are
implemented, and that native Agent observe uses the shared geometry contract.

This design freezes the remaining presentation-space contract so the next cuts
can be small, deterministic, and testable.

## Coordinate-space contract

The public coordinate spaces are:

| Space | Meaning | Serialization rule |
| --- | --- | --- |
| `design` | Project-authored design logical pixels before fit transforms. | Stable authored layout, HIR/sema/runtime-plan quantities, and hit-test answers. |
| `content` | The fitted design viewport rectangle inside output space. | Signed rect is allowed; `cover` may have negative origin and size larger than output. |
| `output` | Output logical pixels after fit transforms and before device-pixel scaling. | Default Agent object bbox/polygon/crop coordinate basis in v1. |
| `physical` | Device pixels after device-pixel-ratio conversion. | Capture adapter/pixel-buffer metadata only. |
| `logical` | Host UI/window logical coordinates before Arcweft output mapping. | Host input adapters only; must be converted before runtime hit tests. |
| `object_local` | Object-local coordinates. | Only valid with an object id in the same capture/selection metadata. |
| `layer_local` | Layer-local coordinates. | Only valid with a layer id in the same capture/selection metadata. |

`arcweft-layout` owns the names and fit/inverse transform math. Agent clients
must not infer coordinate meaning from ad hoc strings; string values appearing
in JSON are serde output from typed Rust enums.

## Fit transform ownership

`arcweft-layout::ContentRect` remains the owning type for:

- `raw`, `contain`, `cover`, and `stretch` computation;
- signed content rects;
- letterbox/pillarbox bar reporting;
- cover crop reporting;
- design-to-output and output-to-design mapping;
- hit-test point conversion;
- deterministic fit-transform metadata.

Renderers, View layout, capture adapters, and Agent observation may cache or
serialize these values, but they must not reimplement the math.

## Scale policies in Agent observe and capture metadata

V1 keeps `arcw agent observe` defaulting to `raw` for compatibility with the
current diagnostic path. A later CLI/API cut may add an explicit `--scale-policy`
selector, but the current default must not silently change object coordinates.

Every Agent observation and selected capture reports the same fit block:

```json
{
  "policy": "raw|contain|cover|stretch",
  "coordinate_spaces": {
    "design": "design",
    "content": "content",
    "output": "output",
    "serialized_geometry": "output",
    "hit_test_input": "output"
  },
  "design_viewport": { "width": 1280, "height": 720 },
  "output_viewport": { "width": 1000, "height": 800, "device_pixel_ratio": 1.0 },
  "content_rect": { "x": 0.0, "y": 118.75, "width": 1000.0, "height": 562.5 },
  "visible_output_rect": { "x": 0.0, "y": 118.75, "width": 1000.0, "height": 562.5 },
  "visible_design_rect": { "x": 0.0, "y": 0.0, "width": 1280.0, "height": 720.0 },
  "bars": { "top": 118.75, "right": 0.0, "bottom": 118.75, "left": 0.0 },
  "crop": { "top": 0.0, "right": 0.0, "bottom": 0.0, "left": 0.0 },
  "scale": { "x": 0.78125, "y": 0.78125 },
  "raw_pixel_mode": false
}
```

For `raw`, `content_rect` is the design viewport at output origin with scale
`1.0`; `visible_output_rect` may clip if the output viewport is smaller. For
`contain`, `bars` carries letterbox/pillarbox in output logical pixels. For
`cover`, `crop` carries positive overflow amounts in output logical pixels and
`content_rect` may have a negative origin. For `stretch`, both bars and crop are
zero and `scale.x != scale.y` is allowed.

## Layout unit resolution model

Parsing owns token recognition only. The typed representation crossing parser,
HIR, sema, runtime-plan, View layout, renderer, and Agent observe boundaries is
`LayoutLengthExpr` plus `LayoutUnit`.

Resolution phases are:

1. `HIR`: preserve typed AST expression; do not resolve.
2. `sema`: validate unit support, expression shape, and whether context-dependent
   units are allowed in the property being checked; do not guess viewport or font
   values.
3. `runtime-plan`: may fold pure numeric `px` constants and preserve typed
   expressions for context-dependent units.
4. `View layout`: provides project/profile design viewport and containing boxes.
5. `renderer`: provides content rect, safe-area, font size, and glyph metrics;
   this is the first phase where all v1 units can be fully evaluated.
6. `Agent observe`: must report the evaluated context and resulting basis when
   it serializes bboxes, hit regions, text fitting results, or captures.

V1 supported units are:

| Unit | Meaning | First safe full resolution |
| --- | --- | --- |
| `px` | design-space logical pixel | runtime-plan for constants; renderer for mixed expressions |
| `sp` | text scale unit equal to current font size multiplier | renderer |
| `%` | containing-box axis percentage | renderer |
| `vw`, `vh` | design viewport width/height percentage | View layout |
| `cw`, `ch` | fitted content rect width/height percentage in output space | renderer |
| `safe_area_*` | output safe-area inset multiplier | renderer |
| `em` | current font-size multiplier | renderer |
| glyph `ch` | measured glyph advance multiplier | renderer |

`cw`/`ch` intentionally mean content-rect width/height, not CSS character `ch`.
The font-relative glyph unit is represented by `glyph_ch`.

## Text fitting and overflow model

V1 policies are:

- `clip`: preserve box and font; report `overflow_clipped` and/or
  `text_truncated` diagnostics when shaped text overflows.
- `page`: preserve box and font; split by shaped cluster indices into stable
  `TextPage` ranges. Typewriter reveal operates inside the active page range and
  never reflows across pages.
- `fit_text`: preserve box; reduce font size using renderer measurement until
  it fits or a configured minimum is reached. Report the final font size and
  `fit_text_reached_minimum`/`fit_text_failed` diagnostics when relevant.
- `expand_box`: preserve font; expand bounds within constraints. Report the
  expanded bounds or `expand_box_constrained`.
- `diagnostic`: fail the presentation contract for this object without silently
  changing geometry.

The shared Sans I/O result is `TextFitResult`. Renderers compute it after
measurement and before capture/Agent serialization. `TextFitResult::report()`
produces a stable `TextFitReport` with `outcome`, compact behavior flags, page
count, font size, expanded bounds, and structured diagnostics.

## Selected object/layer capture metadata

Selected captures use output logical coordinates by default and must include:

- capture `scope`: `viewport`, `layer { id }`, or `object { id }`;
- capture `composition`: framebuffer crop, object-id attachment, mask attachment,
  masked framebuffer crop, isolated regions, overlay vector, or debug geometry;
- `coordinate_basis`: normally `output` in v1;
- `crop.unclipped` and `crop.clipped` rects in the declared basis;
- optional `mask` metadata with object ids, layer ids, object-id attachment flag,
  and alpha-mask flag;
- the same `fit_transform` metadata used by Agent observe.

Object/layer ids are metadata, not filename conventions. Crop origins may be
signed before clipping. Resource dimensions remain unsigned image dimensions.

## Native/WebGPU shared capture behavior

Shared behavior:

- use `arcweft-layout` for fit transform, inverse mapping, bars, crop, and hit
  conversion;
- report identical metadata for viewport, selected layer, and selected object
  captures;
- use typed renderer labels: `native_rich_text_observer`, `shared_web_gpu_scene`,
  or `native_wgpu_adapter`;
- attach pixel format, row stride, content bbox, and content pixel count when a
  concrete image resource exists;
- preserve full-frame captures separately from selected object/layer crops.

Adapter-specific behavior:

- GPU readback, staging buffers, texture formats, MSAA resolves, and filesystem
  writes remain outside `arcweft-layout`;
- native rich-text observer may provide text-specific element bboxes without
  becoming the canonical full-scene WebGPU capture path;
- WebGPU captures must label backend/adapter and may have looser image tolerance
  than metadata tests.

The canonical full player frame capture path for product visual verification is
`shared_web_gpu_scene`. The native rich-text observer remains a diagnostic route
with the explicit renderer label `native_rich_text_observer`.

## Visual golden policy

Visual validation has three tiers:

1. **Metadata exact**: JSON metadata, fit transforms, bars/crops, unit values,
   text fit reports, selected capture scopes, and resource descriptors. This tier
   is deterministic and required in normal CI.
2. **Visual smoke**: render commands must produce an image of the expected size,
   format, renderer label, and non-empty content bbox. This tier is normal CI
   safe across fonts/GPU backends.
3. **Exact visual golden**: only for pinned fixtures with bundled fonts, fixed
   renderer backend label, explicit device scale, and explicit tolerances. This
   tier may be ignored or platform-gated when platform fonts or GPU backends are
   unstable.

Suggested v1 fixture matrix:

- design viewport 1280x720;
- output viewports 1280x720, 960x540, 640x360, and 1000x800;
- `raw`, `contain`, `cover`, and `stretch` metadata fixtures;
- Zundamon dialogue/rich-text smoke fixture;
- selected dialogue textbox object capture fixture;
- selected dialogue layer capture fixture.

Tolerance policy for exact images: require same dimensions and metadata; allow a
small per-channel delta and pixel-count ratio only when the backend/font label
matches the fixture's recorded policy. Cross-platform CI should assert metadata
and smoke, not exact pixels.

## Implementation cuts

1. Freeze coordinate-space naming and transform ownership in `arcweft-layout`.
2. Add `FitTransformMetadata`, bars/crop reporting, and hit-test conversion
   methods to `ContentRect`.
3. Wire fit-transform metadata into Agent observe `layout.viewport_scale` output.
4. Add layout unit resolution phase/dependency methods on `LayoutUnit`.
5. Add `TextFitReport` and deterministic outcome classification for
   `TextFitResult`.
6. Add selected object/layer capture metadata types and constructors in
   `arcweft-layout`.
7. Wire renderer/View layout to these contracts without moving renderer/GPU/I/O
   into `arcweft-layout`.
8. Add deterministic metadata tests first; then add visual smoke/golden fixtures.

The overlay implements cuts 1, 2, 4, 5, and 6 in `arcweft-layout`, and supplies a
small patch for cut 3 in native Agent observe. Cuts 7 and 8 require a full local
checkout and renderer fixture assets.

## Focused tests

Required deterministic tests:

- raw/contain/cover/stretch transform values;
- inverse output-to-design hit-test mapping;
- 1000x800 non-16:9 bar/crop values;
- unit resolution with viewport/content/safe-area/font inputs;
- text fitting outcome and diagnostics;
- Agent observe coordinate metadata JSON;
- selected layer/object crop and mask metadata.

Required smoke/golden commands after applying to a checkout:

```bash
cargo test -p arcweft-layout
cargo test -p arcweft-cli --features native-capture --lib agent_observe_
just test-rich-text
just test-cli-native
# exact image tier only when pinned fonts/backend are available:
just test-tier2
```

## Non-goals

- No renderer, GPU, filesystem, staging-buffer, or capture I/O implementation is
  moved into `arcweft-layout`.
- No change is made to the default Agent observe coordinate behavior; it remains
  raw until an explicit CLI/API scale-policy selector is implemented.
- No exact cross-platform pixel golden is required without bundled fonts and a
  pinned backend label.
- No parser syntax redesign is included beyond preserving typed unit expression
  contracts.
- No Servo, Wasmtime, MCP stdio, or release artifact behavior is changed.
