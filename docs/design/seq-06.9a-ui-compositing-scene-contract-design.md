# seq-06.9a UI Compositing Scene Contract Design

## Purpose

seq06.2 established Takumi as the CSS cascade/layout/stacking source and kept
Arcweft on a direct wgpu primitive path. That is correct for solid rectangles,
rounded rectangles, borders, images, linear gradients, text glyph runs, opacity,
affine transforms, and rectangular clips. It is not sufficient for subtree
effects such as CSS `filter`, `backdrop-filter`, `mask`, `clip-path`, and
`mix-blend-mode`, because those effects apply to a paint subtree or to the
backdrop behind that subtree rather than to one primitive.

This design adds an Arcweft-owned compositing scene contract without changing the
direct primitive contract. The contract is renderer-ready data, not a shader
implementation and not a CPU-rasterized Takumi surface.

## Existing Surface Audited

The current Arcweft surface has these relevant properties:

- `arcweft-render-wgpu::view_scene` owns direct primitives and `ViewSceneContext`
  ranges.
- `arcweft-takumi-adapter::lowering` walks Takumi stacking contexts in paint
  order and emits `ViewSceneContext` entries.
- `arcweft-takumi-adapter::style` previously classified
  `filter`/`backdrop-filter`/`mask`/`clip-path`/`mix-blend-mode` as generic
  unsupported-direct values.
- `arcweft-takumi-adapter::cache` already keeps scene image revisions distinct
  from renderer-resource revisions.

The overlay preserves the first two properties and changes the third by adding
compositing-specific invalidation classes.

## Contract Shape

`arcweft-render-wgpu::view_scene` is split into a direct primitive core and a
compositing responsibility module:

- `view_scene.rs` remains the public module entry point and re-exports the old
  direct primitive API.
- `view_scene/core.rs` contains the existing primitive/context scene.
- `view_scene/compositing.rs` contains Arcweft-owned subtree-effect types.

`ViewScene` gains:

```rust
pub fn paint_nodes(&self) -> &[ViewPaintNode];
pub fn push_paint_node(&mut self, node: ViewPaintNode);
pub fn replace_paint_nodes(&mut self, paint_nodes: Vec<ViewPaintNode>);
```

`push_context` still appends a direct context and now also mirrors it into
`paint_nodes` for compatibility with direct-only producers. The Takumi lowerer
replaces `paint_nodes` with a tree after it has collected the full stacking
context order.

## Compositing Types

The overlay adds these owned types:

- `ViewPaintNode`
  - `Direct(ViewSceneContext)`
  - `Group(ViewCompositingGroup)`
- `ViewCompositingGroup`
  - local bounds
  - isolation
  - `ViewCompositingEffects`
  - ordered child paint nodes
- `ViewCompositingEffects`
  - opacity
  - foreground filter list
  - backdrop filter list
  - masks
  - clip path
  - blend mode
- `ViewFilter` and `ViewFilterList`
- `ViewMask`, `ViewMaskImage`, `ViewMaskSize`, `ViewMaskPosition`, `ViewMaskRepeat`
- `ViewClipPath`, `ViewLength`, `ViewShapeRadius`, `ViewPoint`, `ViewFillRule`
- `ViewBlendMode`
- `ViewIsolation`
- `ViewCompositingRequirements` and `ViewCompositingEffectClass`

The renderer can therefore decide whether a subtree needs a normal offscreen
surface, a backdrop read/composition step, mask sampling, clip tessellation, or
resource revision tracking.

## Canonical Inherent Behavior

The behavior lives on the owned types:

- `ViewFilter::visual_outset_px()`
- `ViewFilter::is_identity()`
- `ViewFilterList::canonicalized()`
- `ViewFilterList::visual_outset_px()`
- `ViewCompositingEffects::requirements()`
- `ViewCompositingEffects::visual_outset_px()`
- `ViewCompositingGroup::visual_bounds()`
- `ViewCompositingGroup::requirements()`
- `ViewMask::requires_resource_revision()`

This follows the Arcweft rule that missing behavior on owned boundary types
belongs on the original owned type rather than in helper traits or ad hoc
call-site utilities.

## Bounds and Outset Rules

The bounds contract is intentionally deterministic:

- color-matrix-like filters have zero visual outset;
- CSS `filter: blur(r)` uses `3r` outset;
- CSS `drop-shadow(x y blur color)` uses
  `max(abs(x), abs(y)) + 1.5 * blur`;
- masks and clip paths do not expand visual bounds;
- a group's visual bounds are its local bounds expanded by its own effect outset
  and any child group outset.

These are conservative enough for first renderer allocation. Future renderer work
can tighten allocations if it proves a more exact shader-specific bound.

## CSS Classification

`CssPropertyClass` now separates:

- `Compositing`: `filter`, `mix-blend-mode`, `isolation`
- `BackdropCompositing`: `backdrop-filter`
- `MaskCompositing`: `mask`, `mask-size`, `mask-position`, `mask-repeat`,
  `mask-mode`, `mask-origin`, `mask-clip`, `mask-composite`
- `ClipGeometry`: `clip-path`, `clip-rule`
- `Resource`: `mask-image`, normal image/background resources

The representable values for the five feature families are no longer generic
unsupported-direct diagnostics. Unsupported values remain explicit, for example
`filter: url(...)`, `backdrop-filter: url(...)`, `clip-path: url(...)`, and
`mask-image: element(...)`.

## Takumi Lowering

The lowerer now performs two passes over the Takumi render tree:

1. Build the Takumi `RenderNode` tree and collect computed compositing style by
   `TakumiPath`.
2. Build layout and stacking contexts, then lower paint buckets into direct
   primitives and ordered `ViewPaintNode` groups.

This is the key change from seq06.2: compositing style is recovered from the
Takumi render node path, not inferred from the direct-paint catalog.

Every Takumi stacking context becomes a `ViewCompositingGroup`. Direct primitive
contexts still exist in `ViewScene::contexts()` for the existing renderer path.
`ViewScene::paint_nodes()` gives the new subtree graph.

## Resource Revisions

`mask-image: url(...)` is a resource input. Its CSS classification is
`Resource`, and tests assert that changing the image revision changes the scene
cache key. Renderer-resource revision remains separate and is still used for
paint-only GPU resource changes.

## CPU Fallback Gate

The overlay adds a structural test that scans the Takumi adapter and renderer
source roots for known CPU full-surface raster fallback markers. The gate is
not intended to prove every future renderer implementation detail; it prevents
the specific disallowed path of routing the UI through Takumi's raster surface
and uploading that full surface as an image.
