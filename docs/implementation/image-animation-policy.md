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
`UiImagePresentationFrame` lowers decoded `ImagePresentationObject` inputs into
layered `UiLayerOutput` values and a single shared `UiImageSourceTable`, so
image ids remain unique across layers and pinned animated frame selection is
preserved.

`arcweft-presentation` owns the semantic image object descriptor:
`ImagePresentationObject` binds an encoded asset reference to a stable image
object id, object layer, interaction target, layer-local bounds, fit,
alignment, opacity, depth, fixed-point transform, deterministic playback
policy, typed params, enabled/visible lifecycle flags, and semantic actions. It
lowers to `SemanticRole::Image` without renderer or filesystem dependencies,
so hit-test, Agent observation, action dispatch metadata, and native submission
can consume one shared object model.

`arcweft-runtime-host` exposes committed UI image display items as
`UiFrameImageItem` values carrying render layer, frame-local node id, `ImageId`,
and layout. This keeps Agent/renderer adapters from spelunking through generic
display-list payloads when they need image-specific capture or observation
metadata.

`arcweft-bundle` now has a typed `image_assets` section. Each
`BundleImageAsset` maps a stable asset id to a bundle virtual file, records the
encoded format (`png`, `jpeg`, `gif`, or `webp`), records whether the asset is
static or animated, records intrinsic dimensions, and can resolve its encoded
bytes without filesystem I/O or source lowering. The CLI bundler decodes
`.arcweft/asset` PNG/JPEG/GIF/WebP files through `arcweft-image` while building
the bundle so metadata reflects the actual payload; static WebP remains static,
and multi-frame GIF/WebP is marked animated. Render/runtime adapters still
decode the encoded bundle payloads for frame upload and playback rather than
re-reading source files. The bundler preserves relative virtual paths and
avoids host path leakage.
`arcw bundle` also validates statically known `asset.image(@asset.id)` /
`asset.image("asset.id")` host-task references and presentation runtime calls
such as `bg(@asset.id)`, `image(@asset.id, ...)`, and
`image(asset = @asset.id, ...)` against `image_assets[]`. The validation walks
flow ops, await pending effects, and line-task effect graphs so source-level
presentation images cannot silently refer to assets that were omitted from the
bundle.
`arcw run-bundle` validates the encoded image asset records before
materializing the bundle workspace. The validation resolves the referenced
virtual file, decodes the encoded bytes with the declared image format, and
checks recorded static/animated state and dimensions when present, so broken or
contradictory image asset records fail before bytecode execution.

`arcweft-agent-protocol` now treats observed object payload as typed content.
Rich text objects carry `content.kind = "rich_text"` with a `LineDisplayFrame`;
image objects carry `content.kind = "image"` with a source id, optional bundle
asset id, optional active frame index, optional pinned local time, optional
object opacity, and optional intrinsic dimensions; and custom objects can use
`content.kind = "custom"`.
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
resolves the active decoded frame once, resolves the same native image quad
geometry used for rendering, converts its transformed corners into an Agent
polygon and viewport bbox, emits `content.kind = "image"` with source id, frame
index, local time, object opacity, object transform, and intrinsic dimensions,
and stores the same active RGBA frame plus native quad placement in an
object-id keyed frame store for native capture. This bridge is adapter-side and
does not add an Agent dependency to `arcweft-runtime-host`.

Agent native object capture now recognizes typed image objects without walking
through a parent rich-text textbox. When the same observation context carries a
decoded image frame store, color capture uses the native textured-quad renderer
with the stored transformed quad placement, applies image object opacity in the
same RGBA upload path, and object-id/mask capture can use the same image alpha,
opacity, and transformed quad geometry through native debug quads. Without
stored frame pixels, object-id and mask captures can still be produced from
observed image object geometry, while color capture intentionally fails;
returning a filled rectangle as color output would mix a debug geometry
fallback into the product image renderer path.

CLI and MCP observation state now carry the image frame store alongside the
observation report and native capture session. Direct `--image` capture,
`--read-uri`, MCP `resources/read`, MCP `arcweft.resource.read`, and MCP
`arcweft.capture` therefore share the same frame-store-aware capture path once
live UI commits populate the store.
Object-scoped image resources also preserve an `image_ref` summary on
`image.object`. The summary records the observed image source, authored object
id, target, asset id, active frame index, pinned local time, opacity, intrinsic
dimensions, fit/alignment policy, semantic actions, typed params, and object
proxy metadata. This keeps direct `--read-uri` and MCP resource/tool responses
useful for debugging animated images without re-inferring frame state from
pixels or object ids.
When an image object pins playback with `playback.local_time`, object and layer
readback report the resulting object-local `frame_index` and
`local_time_millis`, even if the observation itself used a different
`--capture-time`.

Source-level runtime calls can now feed that same presentation-image path for
the first background slot. During Agent observe, `bg(@asset.bg.room)` and the
quoted equivalent resolve to `samples/.arcweft/asset/bg/room.{png,jpg,jpeg,gif,webp}`
beside the observed `.arcw` source, decode through `arcweft-image`, lower into
an `ImagePresentationObject`, lower again through `UiImagePresentationFrame`,
and populate typed Agent image objects plus object-id keyed native frame-store
pixels. Multiple background calls use slot semantics: the last valid background
call wins, avoiding duplicate background object ids. Missing image assets
produce structured `image_asset_unavailable` diagnostics rather than a debug
rectangle or a panic.
`bg(...)` defaults to cover-fit, centered alignment, full opacity, and normal
capture-time playback, but it accepts the same `fit`, `alignment.*`, `opacity`,
and `playback.*` named arguments as bounded `image(...)` so static and animated
backgrounds can be debugged through the same image-object policy.

Agent observe uses an observation-local source image decode cache keyed by
public asset id. The cache stores successful decoded `DecodedImage` values for
the duration of one observation build, so repeated `bg(...)` / `image(...)`
uses of the same static or animated asset reuse decoded frames while preserving
deterministic `capture_time` frame selection. Failed filesystem lookups or
decode errors are not cached, and long-lived eviction policy remains an adapter
concern.

`samples/image-animation.arcw` is the first source-level image sample. It has
separate flows for static PNG, static JPEG, static WebP, animated GIF, and
animated WebP backgrounds. The animated flows are intended to be observed with
different `--capture-time` values so native object PNG/raw captures can prove
that visual-time frame selection, not wall-clock time, drives the active image
frame. The same sample also includes `image_sprite_overlay`, which combines a
background image with a bounded foreground `image(...)` object using authored
id, target, layer, bounds, fit, alignment, opacity, depth, semantic action,
custom `playback.*` timing, `transform.*` matrix/translation metadata,
enabled/visible lifecycle flags, `param.*` metadata, and animated frame
timing. The `image_clipped_object` flow
places a bounded animated image partially outside the viewport so object color
and object-id capture fixtures prove that native capture uses the same
viewport-visible geometry as Agent observation.

The source-level `image(...)` surface now accepts image object metadata in the
same call that defines the visual object:

```arcw
image(
  asset = @asset.bg.pulse,
  id = "image.sample.pulse_sprite",
  target = "target.sample.pulse_sprite",
  layer = "layer.foreground",
  x = 96px,
  y = 72px,
  width = 360px,
  height = 180px,
  fit = "stretch",
  alignment.x = "center",
  alignment.y = "center",
  opacity = 0.5,
  playback.local_time = 50ms,
  transform.tx = 24px,
  transform.ty = 12px,
  depth = 2500,
  enabled = true,
  visible = true,
  action = "action.inspect.pulse_sprite",
  param.role = "animated-hotspot",
  proxy.id = "proxy.pulse_sprite.hotspot",
  proxy.type = "PulseSpriteHotspot",
  proxy.role = "inspect",
  proxy.layer = "layer.hit",
  proxy.depth = 2600,
  proxy.hit_test = true,
  proxy.param.channel = "preview"
)
```

`param.*` is parsed as a dotted named argument, not as an ad hoc string parse.
Checked Arcweft source can express `opacity` as a ratio such as `0.5` or a
milli value such as `500`; presentation call checking supplies the expected
numeric type and the runtime lowering stores the result in the presentation
model's `opacity_milli` field.
`alignment.x` / `alignment.y` (or the short `align.x` / `align.y`) accept
keywords (`left`, `center`, `right`, `top`, `bottom`, `start`, `end`), ratio
values from `0` through `1`, or milli values from `0` through `1000`; they lower
to `ImageObjectAlignment` before native fit and transform resolution.
`playback.start`, `playback.paused_at`, and `playback.local_time` accept
non-negative durations (`150ms`, `0.15s`, or a bare seconds number).
`playback.rate` accepts either a ratio (`0.5`) or milli value (`500`), and all
four fields lower to `ImageObjectPlayback` before UI frame resolution. A pinned
`playback.local_time` therefore overrides the observation `--capture-time` for
that object while still using the same deterministic frame selection path as
un-pinned animated images.
`transform.tx` and `transform.ty` accept pixel lengths. `transform.m11`,
`transform.m12`, `transform.m21`, and `transform.m22` accept fixed-point matrix
components and default to the identity matrix.
`visible = false` omits the image object from UI lowering and Agent
observation. `enabled = false` keeps the observed object and hit-test geometry
available, but emitted semantic actions for that object are marked disabled.
`proxy.*` defines image-object proxy metadata on the same presentation object:
`proxy.id` creates the proxy, `proxy.type` and `proxy.role` classify it,
`proxy.layer` / `proxy.depth` override hit-test ordering metadata, and
`proxy.hit_test = true` emits an `object_proxy` hit region over the image's
actual transformed polygon. `proxy.param.*` is separate from image-level
`param.*`; it is preserved under `content.proxies[]`, presentation-tree
`object_proxies[]`, and hit-test `region.proxy_params`.
The UI display list preserves the semantic spec id for image nodes, and
`UiImageSource` preserves presentation metadata for source-table based frame
resolution. Agent observation therefore emits the authored image object id,
target, asset, action list, custom typed params, object layer, object opacity,
object transform, object depth, object proxies, active frame index, local image
time, and intrinsic dimensions from the same presentation object that native
capture and hit-test use. Authored image actions are also exposed in the Agent
top-level `actions[]` list as semantic `invoke` targets, using the authored
interaction target when present.

`arcweft-render-native` owns the first real native image rendering path:
`capture_image_quads_rgba` uploads RGBA8 image frames to wgpu textures and
renders them as textured quads into the same offscreen RGBA readback surface
used by native captures. This is intentionally not a debug raster fallback; it
is the native renderer's image submission primitive. `arcweft-render-native`
also resolves `arcweft-ui` image display items through `UiImageSourceTable` into
native quads, applying deterministic visual-time frame selection and
fit/alignment rectangle calculation plus affine image-object transform before
GPU submission.
`capture_image_debug_quads_rgba` uses the same textured-quad path after
recoloring non-transparent image pixels, giving object-id and mask capture the
same alpha-shaped geometry, object opacity, and transformed placement as color
image capture.

## Presentation Rules

- An image is a semantic presentation object, like text, rich text, Activity, or
  a custom UI element.
- Animated images are not videos. They have no audio, no independent clock, and
  no host-side imperative callback.
- The active frame is selected from presentation visual time, not wall-clock
  time. Agent capture must be able to pin it with `capture_time`.
- Static and animated images share hit-test, object-id, mask, layer, depth, and
  metadata behavior.
- Authored image object metadata must survive lowering through UI frame commit
  and Agent observation; downstream debug tools should not infer it from object
  ids or coordinates.
- Decode is adapter work over bytes. Filesystem reads, asset lookup, cache
  eviction, and GPU upload are outside the pure data model.
- Short-lived adapter decode caches may reuse decoded `DecodedImage` values,
  but animation playback state remains presentation-time based and is not
  advanced by cache access.

## Required Follow-up Cuts

1. Generalize source asset declarations from the current entity-id declaration
   surface into a payload-driving declaration if the language chooses to let
   source declarations override or supplement `.arcweft/asset` discovery.
   `asset @asset... { ... }` now parses, lowers, resolves, and typechecks as
   an Asset entity declaration, and `samples/image-animation.arcw` declares its
   static and animated image asset ids. The bundle image asset table still owns
   encoded file records and decoded static/animated metadata, and static
   `asset.image(...)`, `bg(...)`, and bounded `image(...)` references are
   checked against that table when they are statically known.
2. Generalize the source-level image surface from the current `bg(...)` and
   bounded `image(...)` calls into declared image objects with hit-test
   proxies. Depth, transforms, lifecycle flags, semantic actions, and custom
   `param.*` metadata are now present on the bounded source-level call path and
   Agent observation path.
3. Add more regression tests for native capture pixels, object metadata, and no
   wall-clock dependence. Bounded animated image pinned-frame readback is
   covered for direct object and layer `--read-uri`; image-object proxy
   hit-test metadata is covered against the same sample; unpinned animated
   background image hit-test metadata is covered across different
   `capture_time` values so `image_ref.frame_index` changes while object
   identity remains stable; clipped animated image object color and object-id
   captures are covered; MCP tool-content preservation is covered with serialized
   metadata fixtures and the CLI `--mcp --mcp-format tool-result` path is
   covered for animated image object raw readback metadata plus blob bytes; live MCP
   `arcweft.resource.read` plus protocol `resources/read` are covered for the
   bounded animated image layer raw resource; and the checked-in
   `image-animation.arcw` sample is bundled as a CLI regression to prove
   PNG/JPEG/static WebP/GIF/animated WebP metadata is recorded in
   `image_assets[]` and validated again by `run-bundle`.

## Dependency Policy

`image 0.25.10` is used for the first pure-Rust decode path with PNG, JPEG, GIF,
and WebP features enabled and default heavy format support disabled. Animated
WebP is accessed through the same `image` animation decoder path. Native
libwebp bindings are not introduced unless a later validation cut proves the
pure-Rust path cannot cover required WebP animation behavior.

## Milestone Gate

Run `just test-image-animation-goal` before claiming the image-animation object
goal complete. The recipe is indexed in
`docs/implementation/image-animation-goal-audit.md` and covers decode,
presentation model, UI lowering, native image quads, declared asset ids,
bundle image metadata, Agent readback, MCP tool-result readback, hit-test,
alignment, clipped object capture, and the checked-in sample.
