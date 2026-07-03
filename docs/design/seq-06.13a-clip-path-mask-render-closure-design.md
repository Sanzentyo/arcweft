# seq06.13a Clip-Path and Mask Render Closure Design

## Goal

Close the gap between the existing Arcweft-owned clip/mask contracts and actual
final-pixel constraints in the normal shared wgpu compositor path.

This design builds on the existing `UiClipPath`, `UiMask`, `UiMaskChainPlan`,
clip/mask metadata, and seq06.9 compositor contract. It does not use browser CSS
clipping, DOM masks, SVG filters, canvas fallback, or CPU-rasterized Takumi
output.

## Supported clip-path subset

Implemented in this cut:

- `clip-path: inset(...)`, including corner radii;
- `clip-path: circle(...)`;
- `clip-path: ellipse(...)`;
- `clip-path: polygon(...)` with `nonzero` and `evenodd` fill rules, up to
  `MAX_CLIP_POLYGON_VERTICES = 16`.

Diagnostics in this cut:

- `path(...)` returns `UiClipPathPlanError::PathUnsupported`;
- polygons above the fixed shader uniform budget return
  `UiClipPathPlanError::TooManyPolygonVertices`;
- `url(...)` and invalid values are represented as `UiClipPath::Unsupported` and
  return `UiClipPathPlanError::Unsupported`.

## Clip implementation strategy

The renderer uses a deterministic analytic shader pass:

1. Render children into the group offscreen target as seq06.9b already does.
2. Apply foreground filters.
3. Apply one clip pass when `UiClipGeometryPlan::requires_geometry_pass()` is
   true.
4. Apply mask passes.
5. Apply backdrop/filter/blend composition to the parent target.

The clip pass samples the group target and multiplies alpha by analytic coverage:

- inset: rectangle inclusion plus per-corner circular radius checks;
- circle/ellipse: normalized ellipse distance;
- polygon: fixed 16-vertex uniform buffer, with even-odd crossing or non-zero
  winding.

This route keeps all rendering in the shared compositor WGSL path. It avoids
stencil state complexity in the first cut and avoids generating CPU coverage
textures.

## Mask geometry behavior

### Resource provider contract

`UiMaskTextureProvider::texture_for` returns a `UiMaskTextureView` containing:

- texture view;
- mask channel (`Alpha` or `Luminance`);
- texture extent in device pixels.

The provider is owned by renderer/player resource preparation. It may be backed
by native or web resource tables, but the data-format and planning crates do not
open files, fetch URLs, or perform network I/O.

### Sampling plan

Each `UiMaskPassPlan` can resolve a `UiMaskSamplingPlan` from:

- source/group extent;
- external mask texture extent;
- CSS-normalized `mask-size`;
- CSS-normalized `mask-position`;
- CSS-normalized `mask-repeat`.

Supported size behavior:

- `Unspecified` / `Auto`: use intrinsic mask texture extent;
- `Cover`: uniform scale to cover the source extent;
- `Contain`: uniform scale to fit inside the source extent;
- `Explicit`: resolve width against source width and height against source
  height; px and percentages are supported.

Supported position behavior:

- `UiMaskPosition.anchor` resolves against the available free space after tile
  sizing, so `50% 50%` centers the tile.

Supported repeat behavior:

- `Repeat`: repeat both axes;
- `NoRepeat`: sample only the first tile;
- `RepeatX`: repeat x only;
- `RepeatY`: repeat y only;
- `Space` / `Round`: unsupported diagnostics.

### Channel behavior

- `Alpha`: coverage is `mask.a`.
- `Luminance`: coverage is `dot(mask.rgb, [0.2126, 0.7152, 0.0722]) * mask.a`.

### Multiple masks

Multiple masks are applied as ordered sequential mask passes. Because each pass
multiplies the current alpha by coverage, the first cut is equivalent to
intersecting the ordered mask coverage. CSS mask compositing operators beyond
this default remain a later request.

## Capture/evidence

The plan-level evidence expected for tests and capture packets:

- group local bounds;
- group visual bounds;
- clip plan kind and normalized geometry;
- mask tile origin and tile size;
- mask repeat booleans;
- mask channel;
- number of clip passes;
- number of shader passes;
- final pass graph counts.

Pixel-level smoke/golden expectations:

- run native and web through the same `UiCompositor::render_scene` path;
- capture the same motion timestamps used by `UiMotionSample` fixtures;
- compare exact hash only on pinned adapters;
- otherwise compare bounded drift with a documented per-channel tolerance.

## Deferred list

- CSS `path()` tessellation and fill-rule edge cases;
- `clip-path: url(...)`;
- gradient masks;
- `mask: element(...)`;
- CSS mask composite operators beyond sequential coverage multiplication;
- `mask-repeat: space | round` exact tile distribution;
- exact cross-GPU visual goldens for all clip/mask families.
