# WGPU renderer

## Frame ownership

`PreparedFrame` is the shared renderer input. It carries the frame’s rectangle and image paint data, controls, text, and attached `PreparedViewScene`s. Each prepared View pairs a renderer-facing `ViewScene` with `PreparedViewSceneResources`; the player/runtime resolves image and mask assets before submission, so the resource payload itself performs no I/O. Prepared text IDs refer to the frame’s canonical prepared text batch.

`SharedRenderer` draws into its scene target in this order: background and frame images, attached View scenes in attachment order, then the ordered frame content including runtime controls. It composites the completed target to the caller’s texture last. Each attached View scene is rendered by `ViewCompositor`.

## Direct primitives and compositing

`ViewScene` preserves paint order as direct context ranges and compositing groups. The direct renderer draws solid and rounded rectangles, borders, linear gradients, images, and prepared text into the active target. A group is rendered through an offscreen target so its children and effects can be composed as one subtree. The group pass order is specified in [Layered rendering](layered-rendering.md).

The current compositor supports color-matrix filters (brightness, contrast, grayscale, saturation, hue rotation, invert, sepia, and opacity), blur, and drop-shadow filters; outer and inset box shadows; all declared `ViewBlendMode` values; and inset, circle, ellipse, polygon, and vector-path clips. Vector paths accept move, line, horizontal/vertical line, quadratic/cubic curve, and close commands under non-zero or even-odd fill, with curves flattened into bounded edge plans. URL clip resources and unsupported path commands return typed planning errors.

Masks accept prepared external textures and linear, radial, or conic gradients. External mask textures carry alpha or luminance sampling; sizing, positioning, and repeat modes are resolved by a typed mask plan. Element masks need a prepared capture resource, which the current compositor does not receive, and return a typed error. Missing mask/image resources and unsupported filters, clips, blend modes, or shadow plans also propagate through `ViewCompositorError` rather than being silently substituted.

The frame and scene owners are [`geometry.rs`](../../crates/arcweft-render-wgpu/src/geometry.rs), [`view_scene.rs`](../../crates/arcweft-render-wgpu/src/view_scene.rs), [`view_compositor.rs`](../../crates/arcweft-render-wgpu/src/view_compositor.rs), and [`renderer.rs`](../../crates/arcweft-render-wgpu/src/renderer.rs). Effect, clip, mask, blend, and shadow plans are owned by their corresponding `view_*` modules in the same crate.
