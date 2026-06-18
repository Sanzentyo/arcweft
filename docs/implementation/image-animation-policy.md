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

1. Wire `arcweft-image` into UI display items with an image source table instead
   of bare integer-only `ImageId` metadata.
2. Add presentation image descriptors for object layer, depth, fit, alignment,
   opacity, transform, and semantic params.
3. Add native renderer upload and textured quad submission for decoded RGBA8
   image frames.
4. Make native capture, object-id, mask, Agent observation, hit-test, and MCP
   image metadata treat image objects the same way rich-text objects are treated.
5. Add bundle/asset sidecar support so product-player `.awfb` execution can use
   decoded or encoded image payloads without source execution.
6. Add samples for PNG/JPEG/static WebP, GIF animation, animated WebP, clipped
   object capture, layer capture, and pinned-frame capture.
7. Add regression tests for frame selection, decode, native capture pixels,
   object metadata, hit-test routing, and no wall-clock dependence.

## Dependency Policy

`image 0.25.10` is used for the first pure-Rust decode path with PNG, JPEG, GIF,
and WebP features enabled and default heavy format support disabled. Animated
WebP is accessed through the same `image` animation decoder path. Native
libwebp bindings are not introduced unless a later validation cut proves the
pure-Rust path cannot cover required WebP animation behavior.
