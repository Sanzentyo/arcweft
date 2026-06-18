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

`arcweft-bundle` now has a typed `image_assets` section. Each
`BundleImageAsset` maps a stable asset id to a bundle virtual file, records the
encoded format (`png`, `jpeg`, `gif`, or `webp`), records whether the asset is
static or animated, and can resolve its encoded bytes without filesystem I/O or
source lowering. Decode remains adapter work through `arcweft-image`. The CLI
bundler now populates this section from `.arcweft/asset` PNG/JPEG/GIF/WebP
files while preserving relative virtual paths and avoiding host path leakage.
`arcw bundle` also validates statically known `asset.image(@asset.id)` and
`asset.image("asset.id")` runtime-plan references against `image_assets[]`.
`arcw run-bundle` validates the encoded image asset records before
materializing the bundle workspace, so broken image asset records fail before
bytecode execution.

`arcweft-agent-protocol` now treats observed object payload as typed content.
Rich text objects carry `content.kind = "rich_text"` with a `LineDisplayFrame`;
image objects carry `content.kind = "image"` with a source id, optional bundle
asset id, optional active frame index, optional pinned local time, and optional
intrinsic dimensions; and custom objects can use `content.kind = "custom"`.
`object_layer` and
`object_depth` are generic observed-object metadata instead of deriving only
from `rich_text_ref`. `AgentImageObjectRef` and the presentation tree preserve
those fields for image objects that have no rich-text child reference, while
rich-text objects continue to use their rich-text metadata as a fallback. Agent
hit-test also accepts generic observed object bounding boxes with
`AgentHitRegionKind::Object`, so image objects can be selected and can return
their capture refs without pretending to be rich text.

`arcweft-cli` owns the first Agent adapter bridge from committed UI image
display items to observed image objects and their capture pixels. Given a
`UiFrameCommit`, `UiImageSourceTable`, and pinned visual time, the adapter
resolves the active decoded frame once, converts the UI layout box to a
viewport bbox, emits `content.kind = "image"` with source id, frame index,
local time, and intrinsic dimensions, and stores the same active RGBA frame in
an object-id keyed frame store for native capture. This bridge is adapter-side
and does not add an Agent dependency to `arcweft-runtime-host`.

Agent native object capture now recognizes typed image objects without walking
through a parent rich-text textbox. When the same observation context carries a
decoded image frame store, color capture uses the native textured-quad renderer,
and object-id/mask capture can use the same image alpha through native debug
quads. Without stored frame pixels, object-id and mask captures can still be
produced from observed image object geometry, while color capture intentionally
fails; returning a filled rectangle as color output would mix a debug geometry
fallback into the product image renderer path.

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

1. Add explicit source/DSL asset declaration syntax when the language design
   settles it; static `asset.image(...)` references are already checked against
   bundle image asset ids.
2. Wire the live Agent native observe loop to feed actual runtime UI commits
   through the UI image item bridge and carry the resulting image frame store
   into CLI read-uri/MCP image color capture.
3. Expose source-level image declarations in `.arcw` samples so real sample
   runs can populate the UI image source table without test-only setup.
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
