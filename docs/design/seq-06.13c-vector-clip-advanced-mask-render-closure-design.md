# seq06.13c Vector Clip and Advanced Mask Render Closure Design

## Goal

Close the remaining seq06.13a clip/mask gaps while preserving the retained `UiScene` / `UiCompositor` path and keeping native and web on Arcweft-owned wgpu rendering.

This design does not use browser DOM clipping, SVG filters, canvas fallback, CSS-rendered hidden elements, screenshot-derived masks, or CPU-rasterized Takumi output. Resource acquisition remains in player/renderer adapters; planning data remains deterministic and Sans I/O.

## Architecture fit

The existing seq06.13a boundaries remain the right ownership split:

- `UiClipPath` and `UiMaskImage` carry renderer-facing retained UI data.
- `UiClipGeometryPlan` owns clip-path normalization, path parsing, flattening, and typed diagnostics.
- `UiMaskPassPlan` owns mask image classification, sizing, positioning, repeat distribution, and gradient stop canonicalization.
- `UiCompositorUniform` owns GPU packing for the shared compositor WGSL contract.
- `UiMaskTextureProvider` remains the only path for external texture/capture resources.

No browser fallback or compatibility layer is introduced. Unsupported features continue to fail through structured diagnostics.

## Vector `clip-path: path(...)` substrate decision

Seq06.13c uses a renderer-owned analytic edge coverage substrate:

1. The renderer parses SVG path data from `UiClipPath::Path` into typed commands.
2. Lines are kept as line commands.
3. Quadratic and cubic Bézier curves are flattened deterministically into line edges using a fixed subdivision budget.
4. The compositor clip shader evaluates the flattened edge list with either even-odd crossing or non-zero winding.

This is not a generated CPU coverage texture and not Takumi raster output. CPU work is limited to deterministic vector command parsing and curve-to-edge planning. Pixel coverage is decided in the Arcweft compositor WGSL shader.

### Why not tessellated geometry for this cut

Tessellated geometry is a good later option for antialiasing and very large paths, but it would introduce a new vertex/index buffer lifecycle and a separate draw path. Seq06.13c keeps the existing one-fullscreen-triangle compositor pass boundary and extends the uniform contract instead. The fixed edge budget gives deterministic failures for oversized paths.

### Why not signed-distance textures

A generated coverage or signed-distance texture would be another resource path with resolution, cache lifetime, and native/web drift decisions. It also risks becoming a hidden CPU raster fallback. The selected edge coverage model keeps the path data typed and deterministic until a later geometry backend is explicitly designed.

## Typed path representation

`UiClipGeometryPlan::Path` contains:

- `fill_rule: UiFillRule` (`NonZero` or `EvenOdd`);
- `commands: Vec<UiClipPathCommandPlan>` preserving typed move, line, quadratic, cubic, and close-path commands;
- `edges: Vec<UiClipPathEdge>` consumed by the shader.

Supported commands:

- `M` / `m` move-to;
- `L` / `l` line-to;
- `H` / `h` horizontal line-to;
- `V` / `v` vertical line-to;
- `Q` / `q` quadratic Bézier;
- `C` / `c` cubic Bézier;
- `Z` / `z` close-path.

Unsupported commands, including arcs and smooth-curve shorthand in this cut, produce `UiClipPathPlanError::UnsupportedPathCommand { command }`.

### Fill rules and winding

The shader receives `fill_rule` as a uniform scalar:

- `NonZero` increments/decrements winding when an edge crosses the sample row.
- `EvenOdd` toggles inside state on each crossing.

Both rules operate on the same flattened edge list so native and web use identical planning and shader logic.

### Curves

Curves remain typed in `commands` and are flattened into `edges` with `PATH_CURVE_SUBDIVISIONS`. The fixed subdivision is deliberately deterministic. The design prefers deterministic first parity over adaptive per-device curve subdivision.

### Close-path behavior

`ClosePath` emits one edge from the current point to the subpath start when they differ. A repeated or zero-length close-path is treated as a typed degenerate segment diagnostic rather than silently disappearing.

### Degenerate segments

A drawable segment whose endpoints are non-finite or whose distance is below `PATH_EPSILON` returns `UiClipPathPlanError::DegeneratePathSegment { command, index }`. This applies to lines, curve subdivisions, and close-path edges. Move-to commands may repeat coordinates because they do not create coverage.

### Budgets

- Existing polygon budget remains `MAX_CLIP_POLYGON_VERTICES = 16`.
- Path command budget is `MAX_CLIP_PATH_COMMANDS = 48`.
- Flattened edge budget is `MAX_CLIP_PATH_EDGES = 96`.

Oversized plans return `TooManyPathCommands` or `TooManyPathEdges` with count and maximum.

## `clip-path: url(...)`

`clip-path: url(...)` remains unsupported in this cut. The design adds a typed retained-UI variant, `UiClipPath::Url(Box<str>)`, so renderer/player adapters can preserve the resource reference. Planning returns `UiClipPathPlanError::UrlClipResourceUnsupported { resource }`.

Reusable SVG/vector clip resources need a resource table, lifecycle, cycle prevention, and CSS reference-box rules. That is intentionally left as a later resource-clip package.

## Gradient mask contract

Gradient masks are rendered by the compositor shader as generated coverage. They do not allocate hidden DOM, SVG, canvas, Takumi raster output, or CPU-generated mask textures.

### Supported retained-UI gradient forms

`UiMaskImage::Gradient(UiMaskGradient)` supports:

- `Linear { angle_degrees, stops }`;
- `Radial { center, radius_x, radius_y, stops }`;
- `Conic { center, from_degrees, stops }`.

The Takumi/CSS adapter in this package lowers non-repeating `linear-gradient(...)` mask images. Radial and conic gradient mask types are available to retained UI and shader planning, but CSS/Takumi lowering for radial/conic remains a structured unsupported diagnostic until the adapter exposes enough normalized shape data and fixture coverage.

### Color stop interpolation

Gradient stops are canonicalized by `UiMaskPassPlan`:

- at least two color stops are required;
- at most `MAX_MASK_GRADIENT_STOPS = 8` are accepted;
- offsets are clamped to `0.0..=1.0` and made non-decreasing in author order;
- interpolation is linear in stop coverage space.

The compositor does not need full RGBA interpolation for mask application. It pre-packs two coverage scalars for each stop:

- alpha coverage: `a`;
- luminance coverage: `dot(rgb, [0.2126, 0.7152, 0.0722]) * a`.

The shader selects the scalar based on `UiMaskChannel` and interpolates coverage.

### Alpha and luminance behavior

`Alpha` and `Luminance` remain the two supported mask channels. A red opaque gradient stop, for example, has alpha coverage `1.0` but luminance coverage around `0.2126`, so the two modes produce deterministic different coverage.

### Repeat and sizing interaction

Gradient masks use the same `UiMaskSamplingPlan` as texture masks.

- `mask-size: auto` / unspecified for gradients uses the source/group extent as the intrinsic mask extent.
- Explicit sizes create gradient tiles of the resolved size.
- `cover` / `contain` use the source/group extent as the intrinsic gradient extent.
- Repeat modes operate on the gradient tile, not a texture.

## `mask-repeat: space` and `round`

Seq06.13c upgrades repeat planning from booleans to per-axis modes while preserving old convenience booleans for callers that only need “does this axis repeat?”

Per-axis modes:

- `NoRepeat`: one tile at the resolved `mask-position` origin.
- `Repeat`: tiles at `origin + n * tile_size`.
- `Space`: compute `count = floor(source_size / tile_size)`. If `count <= 1`, center one tile. Otherwise place first and last tile flush with the source edges and distribute remaining free space evenly with `stride = (source_size - tile_size) / (count - 1)`.
- `Round`: compute `count = max(1, round(source_size / tile_size))`, resize the tile to `source_size / count`, set origin to `0`, and stride by the resized tile size.

This exact distribution is encoded in `UiMaskSamplingPlan` as tile origin, tile size, tile stride, tile count, and axis mode. The shader uses those fields for both texture and gradient masks.

## `mask: element(...)`

Element masks are represented explicitly as `UiMaskImage::Element(UiElementMaskSource)`. Seq06.13c does not claim element capture rendering because the inspected repository does not expose a typed element-capture resource graph equivalent to seq06.5/06.6 in the render-wgpu path.

Planning therefore emits `UiMaskPlanError::ElementMaskCaptureUnavailable { element_id }`.

Lifecycle and recursion requirements for the future implementation:

- element capture resources must be prepared by player/renderer adapters before `UiCompositor::render_scene`;
- a capture must include a stable element id and capture generation;
- capture lookup must reject the current element and all ancestor/descendant cycles before compositor work begins;
- missing captures must fail as structured diagnostics, never by falling back to DOM or screenshots;
- native and web evidence must record element id, capture generation, capture extent, mask channel, and drift thresholds.

## Error and diagnostic contract

New or refined typed failures:

- `UiClipPathPlanError::UnsupportedPathCommand { command }`;
- `UiClipPathPlanError::MalformedPath { reason }`;
- `UiClipPathPlanError::DegeneratePathSegment { command, index }`;
- `UiClipPathPlanError::TooManyPathCommands { count, maximum }`;
- `UiClipPathPlanError::TooManyPathEdges { count, maximum }`;
- `UiClipPathPlanError::UrlClipResourceUnsupported { resource }`;
- `UiMaskPlanError::UnsupportedGradient { reason }`;
- `UiMaskPlanError::TooManyGradientStops { count, maximum }`;
- `UiMaskPlanError::InvalidGradientStopCount { count }`;
- `UiMaskPlanError::ElementMaskCaptureUnavailable { element_id }`.

Existing unsupported image/size/position/repeat diagnostics remain in place.

## Visual evidence plan

Focused non-GPU tests assert deterministic plan output. Optional ignored smoke captures should compare native and web images using:

- same retained `UiScene` fixture;
- same source/mask resources;
- same timestamp metadata;
- per-channel absolute drift threshold of 2 for pinned adapters;
- mean absolute drift threshold of 0.75 for unpinned adapters.

Exact cross-GPU goldens remain ignored until a pinned adapter readback harness is promoted.
