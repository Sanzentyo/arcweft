# seq06.9b wgpu UI Compositor Effects Design

## Goal

This design implements the renderer substrate for the seq06.9a UI compositing
scene contract. The upstream contract owns `UiPaintNode`, `UiCompositingGroup`,
`UiCompositingEffects`, `UiFilterList`, `UiMask`, `UiClipPath`, `UiBlendMode`,
and `UiIsolation`; seq06.9b consumes those types and does not redefine them.

The renderer work is split into responsibility modules:

- `ui_effects.rs`: pure filter pass planning, color matrices, blur/drop-shadow
  bounds, and texture extent bucketing.
- `ui_blend.rs`: shader-supported `mix-blend-mode` classification.
- `ui_mask.rs`: mask alpha/luminance pass planning and resource requirements.
- `ui_clip_path.rs`: supported clip geometry planning and explicit unsupported
  path diagnostics.
- `ui_compositor.rs`: wgpu offscreen target pool, root/group render orchestration,
  backdrop copy path, shader pass execution, and direct primitive callback seam.
- `ui_shaders/compositor.wgsl`: full-screen triangle shader for color-matrix,
  blur, drop-shadow, mask, and blend/composite passes.

## Render model

Every composited frame is first rendered into a root offscreen texture. Direct
primitive ranges are drawn through `UiDirectPrimitiveRenderer`; compositing
groups allocate a group target from `UiRenderTargetPool`, render their children
into it, run filter/mask passes into scratch targets, then composite the result
back to the parent target.

This means the final swapchain or host target is never sampled directly. If a
group has `backdrop-filter`, the compositor copies from the already-rendered
parent offscreen texture into a separate backdrop texture and samples that copy
through the same filter pipeline.

## Filter implementation

Color filters are converted to deterministic 4x4 color matrices:

- `brightness()`
- `contrast()`
- `grayscale()`
- `sepia()`
- `saturate()`
- `hue-rotate()`
- `invert()`
- `opacity()`

`blur()` is planned as a separable horizontal + vertical shader pair. The target
extent expands using the seq06.9a filter-outset contract. `drop-shadow()` derives
coverage from source alpha, applies the requested offset/tint/blur plan, and
composites the shadow below the original source in the shader path.

## Mask and clip path

Mask planning preserves ordered masks and distinguishes alpha and luminance
sampling. External mask textures are supplied through `UiMaskTextureProvider`, so
the renderer crate does not read files or fetch resources.

Initial clip geometry supports inset, circle/ellipse, and polygon. CSS `path()`
remains an explicit `UiClipPathPlanError::PathUnsupported` until a vector path
tessellator is selected by a later request.

## Blend mode support

The first shader path supports:

`normal`, `multiply`, `screen`, `overlay`, `darken`, `lighten`, `color-dodge`,
`color-burn`, `hard-light`, `soft-light`, `difference`, `exclusion`,
`plus-lighter`, and `plus-darker`.

`hue`, `saturation`, `color`, and `luminosity` remain explicit unsupported
first-cut modes because they require HSL/luminosity decomposition that should be
specified and golden-tested separately.

## Allocation strategy

`UiTextureExtent` clamps to a deterministic maximum and buckets intermediate
textures to power-of-two dimensions. The pool reuses exact format/extent matches
within later frames to avoid resizing churn while keeping bounds predictable for
validation.

## Integration boundary

`SharedRenderer::create_ui_compositor()` constructs a compositor using the
renderer target format. The compositor remains platform-independent: surface
acquisition, event loops, file/resource loading, and exact GPU readback goldens
stay in host or test harness layers.
