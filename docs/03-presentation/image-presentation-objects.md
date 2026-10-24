# Image Presentation Objects

Static images and animated image containers are Arcweft presentation objects.
They are not renderer-private debug rectangles, and animated GIF/WebP assets are
not treated as video. A static image is a one-frame decoded image; an animated
image is the same decoded object with multiple RGBA frames and deterministic
frame timing.

## Source Surface

The source surface uses ordinary call syntax and authored
image-object declarations. Asset identities come from the selected asset
catalog rather than source declarations:

```arcw
pub image sample.pulse_sprite {
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

Background state is addressed by a typed target/slot pair. The defaults are
`@target.scene` and `@slot.background.default`; a later valid write replaces
only the same pair, while other pairs coexist. `bg.clear(...)` clears exactly
the selected pair. A malformed replacement or clear rejects the presentation
update atomically, leaving its previously committed state intact. The shared
target/slot ownership rules are described in [Presentation Layers](layers.md).
Stable object identity preserves distinct target/slot pairs. Slot names do not
imply depth or layering; explicit depth and semantic identity own paint order.

`image(asset = ..., ...)` creates a bounded image object. It carries authored
object identity, target, layer, bounds, fit/alignment, opacity, depth,
transform, playback policy, lifecycle flags, semantic actions, custom
`param.*` metadata, and optional `proxy.*` hit-test metadata.

`image @image... { ... }` declares the same bounded image object metadata at
module scope. `image(@image.id)` expands that declaration into the same
`ImagePresentationObject` model used by inline bounded calls, so Agent observe,
hit-test, capture, bundle validation, and native rendering all see the same
typed object. Call-site named arguments may override declaration fields.

Assets are catalog-only. The build adapter enumerates the manifest-selected
`assets/` root, normalizes each relative `/`-separated virtual path, removes its
extension, and derives an `AssetId`; for example, `bg/pulse.gif` becomes
`asset.bg.pulse`. The bundle records the encoded payload and decoded metadata in
`image_assets[]`. Authored source refers to catalog entries with
family-relative references such as `bg(@asset:.bg.room)` and
`image(asset = @asset:.bg.pulse)`. Fully qualified references such as
`@asset.bg.room` remain valid for generated surfaces, manifest/tooling output,
stored public-id roundtrips, and external interfaces that need the stored public
id verbatim.

The older fluent sketch form `image(@asset).fit(...)` is not the implemented
surface. Declared image objects are the canonical reusable object form and lower
into the same semantic image object model rather than adding a compatibility
adapter.

## Object Model

Image objects lower through the Sans I/O presentation model and then through View
frame commit as typed image display items. Adapters receive an image source id,
the decoded source table entry, object bounds, transform, opacity, layer, depth,
actions, params, proxies, and playback policy. Downstream tools must preserve
that metadata; they should not infer the object from coordinates, filenames, or
object-id string conventions.

The shared value authority is `arcweft-interaction-model::presentation`:
`ImageObjectFit`, `ImageObjectAlignment`, `ImageObjectPlayback`, and
`ImageObjectTransform`. Bundle objects, presentation objects, retained View image
sources, and render submissions carry those same types directly. There is no
separate bundle or View fit, alignment, playback, or transform representation.
The runtime Core can use these host-neutral values without depending on a
renderer or retained presentation state.

Alignment has two checked integer milli-unit axes, each in `0..=1000`.
Construction and decode reject values outside that interval; they do not clamp.
Playback stores an unsigned millisecond start, a non-negative `u32` milli-unit
rate, and optional unsigned millisecond pause and pinned-local times. Zero rate
stops unpinned playback. Transform stores six signed `i32` milli-unit components.
The four fit cases are `contain`, `cover`, `stretch`, and `intrinsic`.

Serialized bundle image objects require explicit `fit`, `alignment`, `playback`,
and `transform` records. Alignment requires both axes, playback requires its start
and rate, and transform requires all six components. Optional playback times may
be absent. Unknown value-record fields, duplicate fields, invalid types, and
unrepresentable integers are decode errors. Rust authoring constructors may select
centered alignment, normal playback, and identity transform; decoding never
supplies those authoring defaults for missing required fields. This evolves the
initial version `1` shape in place without a legacy reader.

Slot-removal results are structural options: `clear.bg(...)` returns
`Option<BackgroundSurface>` and `hide(...)` returns `Option<CharacterSurface>`.
The payload is the removed slot value, distinct from the scope-owned `Handle<T>`
returned by staging. The semantic graph retains the `Option` constructor and its
payload type, rather than a formatted generic type name. A match on either result
must cover both `.Some(_)` and `.None`.

Bundle image assets record stable asset id, virtual path, encoded format
(`png`, `jpeg`, `gif`, or `webp`), static/animated state, and intrinsic
dimensions. Source-level `asset.image(...)`, `bg(...)`, and `image(...)`
references are checked against bundle image assets when statically known, and
`run-bundle` validates the encoded payload records before execution.

## Placement resolution

The typed `StagePlacement` value carried by an image bundle object supports an
output-logical absolute rectangle or a responsive anchor resolved against the
design and output viewports. Player frame planning produces design, output, and
physical bounds that the image renderer and observation path consume together.
The current bounded `image(...)` call lowers its explicit `x`, `y`, `width`, and
`height` to absolute placement; background staging constructs its own top-left
design placement. This typed runtime model does not define a source-level anchor
spelling or connect character-stage placement to image objects.

## Animation Time

Animated images select their active frame from presentation visual time. Agent
observe, native capture, hit-test, direct `--read-uri`, and MCP resource reads
must all use the same frame selection rule for a given `capture_time`.

`playback.local_time` pins object-local image time and overrides observation
time for that image object. `playback.start`, `playback.paused_at`, and
`playback.rate` are deterministic presentation parameters, not wall-clock
state. Decode caches may reuse decoded bytes/frames, but cache access must not
advance animation.

The active native runtime and Agent observation state retain a decoded
`BundleImageCatalog` built from their bundle. Backgrounds and bounded image
objects reuse those decoded assets through shared frame preparation, so render,
hit-test, capture, and observation do not create object-local decode paths.

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
