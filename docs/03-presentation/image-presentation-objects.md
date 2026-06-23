# Image Presentation Objects

Static images and animated image containers are Arcweft presentation objects.
They are not renderer-private debug rectangles, and animated GIF/WebP assets are
not treated as video. A static image is a one-frame decoded image; an animated
image is the same decoded object with multiple RGBA frames and deterministic
frame timing.

## Source Surface

The current implemented source surface is ordinary call syntax:

```arcw
pub asset bg_room {
  kind = image
  file = "bg/room.png"
}

pub image @image.sample.pulse_sprite {
  asset = @asset:.bg.pulse
  target = @target.sample.pulse_sprite
  layer = @layer.foreground
  x = 96px
  y = 72px
  width = 360px
  height = 180px
  fit = stretch
  opacity = 0.5
  playback.local_time = 50ms
  proxy.id = @proxy.pulse_sprite.hotspot
  proxy.hit_test = true
}

bg(@asset:.bg.room)
bg(@asset:.bg.poster, fit = "intrinsic", alignment.x = "right", alignment.y = "bottom")

image(@image.sample.pulse_sprite)

image(
  asset = @asset:.bg.pulse,
  id = "image.sample.pulse_sprite",
  target = "target.sample.pulse_sprite",
  layer = "layer.foreground",
  x = 96px,
  y = 72px,
  width = 360px,
  height = 180px,
  fit = "stretch",
  opacity = 0.5,
  playback.local_time = 50ms,
  transform.tx = 24px,
  transform.ty = 12px,
  depth = 2500,
  action = "action.inspect.pulse_sprite",
  param.role = "animated-hotspot",
  proxy.id = "proxy.pulse_sprite.hotspot",
  proxy.type = "PulseSpriteHotspot",
  proxy.role = "inspect",
  proxy.hit_test = true
)
```

`bg(...)` writes a background slot. By default it uses cover fit, centered
alignment, full opacity, and capture-time playback. It accepts the same
`fit`, `alignment.*`, `opacity`, and `playback.*` parameters as bounded image
objects so static and animated backgrounds can be observed through the same
debug surface.

`image(asset = ..., ...)` creates a bounded image object. It carries authored
object identity, target, layer, bounds, fit/alignment, opacity, depth,
transform, playback policy, lifecycle flags, semantic actions, custom
`param.*` metadata, and optional `proxy.*` hit-test metadata.

`image @image... { ... }` declares the same bounded image object metadata at
module scope. `image(@image.id)` expands that declaration into the same
`ImagePresentationObject` model used by inline bounded calls, so Agent observe,
hit-test, capture, bundle validation, and native rendering all see the same
typed object. Call-site named arguments may override declaration fields.

`asset name { ... }` is the recommended hand-written asset declaration surface:
the `asset` keyword already supplies the default declaration family, so the
family prefix is omitted there. It declares the stable asset id used by
`asset.image(...)`, `bg(...)`, and `image(...)`. Fully qualified declaration
headers such as `asset @asset.bg_room { ... }` remain valid for generated or
fully elaborated source, but authoring tools should lint them toward the
compact declaration form. Authored asset references should prefer
family-relative references such as `bg(@asset:.bg.room)` and
`image(asset = @asset:.bg.pulse)`; this is the compact authored form because
the `asset` anchor is explicit while the id path does not repeat the default
family. Fully qualified references such as `@asset.bg.room` remain valid for
generated surfaces, manifest/tooling output, stored public-id roundtrips, and
external interfaces that need the stored public id verbatim, but they are not
the recommended spelling for ordinary hand-authored asset references. Asset
bodies are preserved as source metadata;
the current bundle implementation still records encoded payloads from
`.arcweft/asset` into `image_assets[]` and validates statically known image
references against that table.

The older fluent sketch form `image(@asset).fit(...)` is not the implemented
surface. Declared image objects are the canonical reusable object form and lower
into the same semantic image object model rather than adding a compatibility
adapter.

## Object Model

Image objects lower through the Sans I/O presentation model and then through UI
frame commit as typed image display items. Adapters receive an image source id,
the decoded source table entry, object bounds, transform, opacity, layer, depth,
actions, params, proxies, and playback policy. Downstream tools must preserve
that metadata; they should not infer the object from coordinates, filenames, or
object-id string conventions.

Bundle image assets record stable asset id, virtual path, encoded format
(`png`, `jpeg`, `gif`, or `webp`), static/animated state, and intrinsic
dimensions. Source-level `asset.image(...)`, `bg(...)`, and `image(...)`
references are checked against bundle image assets when statically known, and
`run-bundle` validates the encoded payload records before execution.

## Animation Time

Animated images select their active frame from presentation visual time. Agent
observe, native capture, hit-test, direct `--read-uri`, and MCP resource reads
must all use the same frame selection rule for a given `capture_time`.

`playback.local_time` pins object-local image time and overrides observation
time for that image object. `playback.start`, `playback.paused_at`, and
`playback.rate` are deterministic presentation parameters, not wall-clock
state. Decode caches may reuse decoded bytes/frames, but cache access must not
advance animation.

## Capture and Hit-Test

Native image capture uses textured image quads over decoded RGBA frames. Color
capture uses the product image submission path; object-id and mask capture use
the same alpha-shaped geometry, opacity, transform, and viewport clipping after
debug recoloring. A missing decoded frame may still allow object-id/mask
capture from observed geometry, but color capture must not fabricate a filled
debug rectangle.

Agent observation emits `content.kind = "image"` objects with source id, asset
id, active frame index, local image time, opacity, intrinsic dimensions,
fit/alignment policy, authored actions, params, proxies, object layer, depth,
and capture refs. Hit-test returns the same object/proxy metadata, so an
animated image can be selected and then captured without re-resolving its frame
state.

The sample [image-animation.arcw](../../samples/image-animation.arcw) exercises
PNG, JPEG, static WebP, animated GIF, animated WebP, bounded animated foreground
objects, clipped object capture, alignment, opacity, transforms, actions,
params, proxies, and pinned playback time.
