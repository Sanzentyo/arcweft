# Image and Animated Image Implementation Policy

Status: active implementation goal.

The source archive `arcweft-gif-webp-animation.zip`, supplied on 2026-06-19,
contained only empty directories after extraction. This policy therefore uses
the user requirement as the source of truth: ordinary images and animated image
containers such as GIF and WebP must become first-class Arcweft presentation
objects rather than renderer-specific special cases.

## Model

Images use a single decoded presentation model:

```text
source bytes
  -> decode adapter
  -> DecodedImage
       format
       dimensions
       repetition
       frames[]
            RGBA8 pixels
            frame duration
```

Static PNG/JPEG/WebP images are represented as one-frame `DecodedImage` values.
Animated GIF/WebP images are represented as multi-frame `DecodedImage` values
with deterministic frame timing.

The first implementation boundary is the Sans I/O `arcweft-image` crate. It
does not open paths or own renderer resources. It accepts bytes, decodes RGBA8
frames, normalizes zero or too-small animation delays, and exposes
`frame_at_time_millis` for deterministic native rendering, Agent capture, and
tests.

`arcweft-ui` owns the first UI image source table. `ImageId` values now resolve
to `UiImageSource` records containing decoded image data, fit/alignment policy,
and deterministic playback state. Static and animated images therefore cross
the retained-fragment/display-list boundary through the same `ImageId` path.

`arcweft-presentation` owns the semantic image object descriptor:
`ImagePresentationObject` binds an encoded asset reference to a stable image
object id, object layer, interaction target, layer-local bounds, fit,
alignment, opacity, depth, fixed-point transform, deterministic playback
policy, typed params, and semantic actions. It lowers to `SemanticRole::Image`
without renderer or filesystem dependencies, so hit-test, Agent observation,
and native submission can consume one shared object model.

`arcweft-runtime-host` exposes committed UI image display items as
`UiFrameImageItem` values carrying render layer, frame-local node id, `ImageId`,
and layout. This keeps Agent/renderer adapters from spelunking through generic
display-list payloads when they need image-specific capture or observation
metadata.

`arcweft-agent-protocol` now treats `object_layer` and `object_depth` as generic
observed-object metadata instead of deriving them only from `rich_text_ref`.
`AgentImageObjectRef` and the presentation tree preserve those fields for image
objects that have no rich-text child reference, while rich-text objects continue
to use their rich-text metadata as a fallback. Agent hit-test also accepts
generic observed object bounding boxes with `AgentHitRegionKind::Object`, so
image objects can be selected and can return their capture refs without
pretending to be rich text.

`arcweft-render-native` owns the first real native image rendering path:
`capture_image_quads_rgba` uploads RGBA8 image frames to wgpu textures and
renders them as textured quads into the same offscreen RGBA readback surface
used by native captures. This is intentionally not a debug raster fallback; it
is the native renderer's image submission primitive. `arcweft-render-native`
also resolves `arcweft-ui` image display items through `UiImageSourceTable` into
native quads, applying deterministic visual-time frame selection and
fit/alignment rectangle calculation before GPU submission.
`capture_image_debug_quads_rgba` uses the same textured-quad path after
recoloring non-transparent image pixels, giving object-id and mask capture the
same alpha-shaped geometry as color image capture.

## Presentation Rules

- An image is a semantic presentation object, like text, rich text, Activity, or
  a custom UI element.
- Animated images are not videos. They have no audio, no independent clock, and
  no host-side imperative callback.
- The active frame is selected from presentation visual time, not wall-clock
  time. Agent capture must be able to pin it with `capture_time`.
- Static and animated images share hit-test, object-id, mask, layer, depth, and
  metadata behavior.
- Decode is adapter work over bytes. Filesystem reads, asset lookup, cache
  eviction, and GPU upload are outside the pure data model.

## Required Follow-up Cuts

1. Wire Agent native observe to call the runtime-host image item API and UI
   image display-list bridge, rather than only using direct render-native unit
   submissions.
2. Make Agent observation, hit-test, MCP image metadata, and CLI capture
   selection treat image objects the same way rich-text objects are treated.
3. Add bundle/asset sidecar support so product-player `.awfb` execution can use
   decoded or encoded image payloads without source execution.
4. Add samples for PNG/JPEG/static WebP, GIF animation, animated WebP, clipped
   object capture, layer capture, and pinned-frame capture.
5. Add regression tests for frame selection, decode, native capture pixels,
   object metadata, hit-test routing, and no wall-clock dependence.

## Dependency Policy

`image 0.25.10` is used for the first pure-Rust decode path with PNG, JPEG, GIF,
and WebP features enabled and default heavy format support disabled. Animated
WebP is accessed through the same `image` animation decoder path. Native
libwebp bindings are not introduced unless a later validation cut proves the
pure-Rust path cannot cover required WebP animation behavior.
