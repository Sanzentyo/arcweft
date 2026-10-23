# Layered rendering

## View scene paint order

The native View renderer submits an ordered `ViewScene` paint graph through `ViewCompositor`. A `ViewPaintNode::Direct` names a primitive range and renders it into the active target. A `ViewPaintNode::Group` owns bounds, isolation, compositing effects, and child nodes; its children retain their scene order inside the group target. Direct nodes and groups therefore share one ordered scene graph while using different render paths.

For each group, the compositor renders outer box shadows, visits child nodes in order, then renders inset shadows. It applies the group’s foreground filters, clip path, and mask in that order. A backdrop filter copies the already-rendered parent target and filters that copy; the group is then composited over the parent using its blend mode and opacity. Nested groups follow the same sequence.

The clip, mask, filter, blend, and shadow vocabularies and their typed failure cases are listed in the [WGPU renderer contract](wgpu-renderer.md). Unsupported geometry or effects remain explicit planning or compositor errors.

## Runtime control overlays

`PreparedControlPaint` associates a control’s rectangle and text ranges with its backdrop, shadow, and foreground-filter ranges. `SharedRenderer` processes those paint records in frame order between surrounding rectangle spans, after attached View scenes. For a control it draws outer shadows, applies backdrop filters, paints the control foreground, and draws inset shadows. A foreground filter renders the prepared control into an offscreen target before applying its filter passes.

Runtime-control foreground and backdrop plans use the shared `ViewFilterList` executor. Their typed filter vocabulary is brightness, contrast, grayscale, saturation, hue rotation, invert, sepia, opacity, and blur. Backdrop sampling is selected by `RuntimeControlBackdropSamplePolicy`: `PriorFrameContent` samples a snapshot of frame content captured before runtime controls, while `PriorFrameContentAndEarlierRuntimeControls` samples the active target after earlier controls. The current control-style projection assigns `PriorFrameContent` to prepared backdrop plans.

The scene graph and group passes are implemented by [`view_scene.rs`](../../crates/arcweft-render-wgpu/src/view_scene.rs) and [`view_compositor.rs`](../../crates/arcweft-render-wgpu/src/view_compositor.rs). Frame ordering and runtime-control projection are implemented by [`renderer.rs`](../../crates/arcweft-render-wgpu/src/renderer.rs) and [`geometry/control_style.rs`](../../crates/arcweft-render-wgpu/src/geometry/control_style.rs).
