# Seq06.11b UI Scene Compositor Player Path Integration

Seq06.11b connects retained `UiScene` paint data to the ordinary native/web
player frame path. It does not introduce a private native renderer, browser DOM
UI overlay, canvas-2D bridge, screenshot bridge, or Takumi CPU-raster fallback.

The renderer-facing contract is that a normal `PreparedFrame` can carry one or
more `PreparedUiScene` values:

```rust
PreparedFrame {
    rectangles,
    images,
    text,
    styled_paragraphs,
    choices,
    ui_scenes,
    ..
}

PreparedUiScene {
    scene: UiScene,
    resources: PreparedUiSceneResources,
}
```

`arcweft-render-wgpu` receives only Arcweft-owned renderer data: `UiScene`,
decoded image/mask frames, resource indices, mask channels, and explicit text
handoff records. It does not depend on CSS, Takumi computed style, browser DOM,
native windows, filesystem, network, or bundle loading. Player/runtime adapters
remain responsible for resolving external resources before a frame reaches the
renderer.

## Ownership

`PreparedFrame` owns the frame attachment point because native and web already
converge on `SharedRenderer::render_to_view`. `SharedFramePlanner::prepare`
initializes `ui_scenes` to an empty vector so existing dialogue/text scenes keep
their current behavior until their separate retained-UI migration is complete.

`SharedRenderer::render_to_view` is the only visual entrypoint. Its frame order
is:

1. prepare existing glyphon text;
2. render the background rectangle and non-UI images;
3. render each attached `PreparedUiScene` through `UiCompositor::render_scene`;
4. render existing overlay rectangles and glyphon text;
5. submit through the caller-owned native/web surface.

`WgpuUiDirectPrimitiveRenderer` implements the existing compositor callback for
direct primitive ranges. It supports solid rectangles, rounded rectangles,
borders, linear gradients, image primitives, selection, caret, and composition
underline geometry. `UiPrimitive::GlyphRun` requires a matching
`PreparedUiGlyphRunHandoff`; this cut does not fake text with rectangles or
route text through a separate DOM/canvas path.

## Deferred Items

Full dialogue/text migration into `UiScene`, exact per-pixel multi-stop CSS
gradient parity, advanced clip/path/mask closures, and product UI resource
lowering remain separate follow-up work. Those are not hidden fallbacks: missing
resources or unsupported primitive requirements surface through typed
`UiCompositorError` variants.
