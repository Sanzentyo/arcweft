# Agent Observe and Capture Contract

This document defines the stable observation and capture contract used by Agent
debugging tools. The typed JSON shape is owned by `arcweft-agent-protocol`.
CLI and MCP adapters expose that shape without reshaping it.

Related:

- [Agent Debug Bus / MCP / CLI](agent-debug-mcp-cli.md)
- [Text / RichText / Typst](../03-presentation/text-typesetting.md)
- [Text Presentation Objects](../03-presentation/text-presentation-objects.md)
- [Rich Text Effects and Transforms](../03-presentation/rich-text-effects-transforms.md)

---

## Contract boundary

`observe` returns semantic frame state and resource descriptors. `capture`
returns image bytes plus the same descriptor metadata. A debugger must be able
to answer these questions from one observation:

- what frame, viewport, layers, objects, and rich-text child ranges exist
- which full-frame, layer, object, and rich-text child captures are available
- which URI reads reproduce the exact bytes and metadata for a listed capture
- how image-local pixels map back to viewport coordinates and source ranges

The renderer that produced a capture is explicit metadata, not inferred from
the tool that returned it. The current native renderer value is `native`.

---

## Observation lifecycle

An observation represents a single rendered frame:

```json
{
  "session_id": "cli",
  "tick": 0,
  "frame_id": 0,
  "state_hash": "sha256:...",
  "render_hash": "sha256:...",
  "viewport": { "width": 1280, "height": 720 },
  "images": [],
  "layers": [],
  "objects": [],
  "diagnostics": []
}
```

`state_hash` identifies the runtime state used to prepare the frame.
`render_hash` identifies the effective render inputs. Tooling may compare
hashes before reading image bytes, but image comparisons must use capture
resources rather than synthetic replacement renderers.

`capture_time` is an observation/capture input in seconds. It affects
visibility-only effects such as typewriter reveal. It must not change source
ranges, object identity, or layout geometry unless the effect phase is
explicitly pre-layout. When a deterministic `capture_step` is supplied and
`capture_time` is omitted, native image capture derives the visual-effect time
from that step number in seconds. Image resources expose non-zero visual time as
`capture_time_millis` so debuggers can distinguish state step and effect time
without parsing command arguments. Observation reports also include optional
root-level `capture_time_millis` when the frame was observed with an explicit
`capture_time` or step-derived visual-effect time, so subsequent capture or URI
readback can reproduce the same animation state.

---

## Coordinate spaces

All observed layer and object geometry uses viewport coordinates in CSS-like
logical pixels:

- `bbox` is the layout or visual bounding rectangle in viewport coordinates.
- `polygon` is optional tighter geometry in viewport coordinates.
- `crop_origin` is the top-left viewport coordinate of an image crop.
- `content_bbox` is image-local pixel content inside a capture.
- `content_viewport_bbox` is the same content rectangle translated into
  viewport coordinates.
- `content_pixels` counts non-background or mask-selected pixels according to
  the capture composition.

Captures must include enough metadata to convert image-local pixels back to
viewport coordinates without parsing URI strings.

---

## Image resources

Every image resource has these semantic axes:

| Field | Values |
|---|---|
| `kind` | `color`, `overlay`, `overlay_svg`, `object_id`, `mask` |
| `renderer` | `native` |
| `scope` | `viewport`, `layer { id }`, `object { id }` |
| `composition` | `framebuffer`, `overlay_vector`, `framebuffer_crop`, `object_id_attachment`, `mask_attachment`, `masked_framebuffer_crop`, `isolated_regions`, `debug_geometry` |
| `mime_type` | `image/png`, `image/x-arcweft-rgba8`, `image/svg+xml` |

Required capture families:

- full viewport color image
- full viewport object-id and mask images when supported by the renderer
- layer color crop or isolated layer image
- layer object-id and mask image
- object color crop or isolated object image
- object object-id and mask image
- rich-text child object image, including ruby base, ruby annotation, and glyph
  cluster scopes when those children are listed

`raw-rgba` resources use `rgba8_unorm` with `row_stride_bytes = width * 4`.
PNG resources must be decoded as normal PNG images. Raw and PNG captures of the
same scope must describe the same viewport-space geometry.

Object-scoped image metadata carries an `object` reference in addition to
`scope.object.id`. The reference preserves the observed object's id, optional
entity, layer, role, optional display text, and optional `rich_text_ref`. For a
rich-text child crop, this means `--read-uri`, MCP `resources/read`, and direct
capture responses expose the same source range, presentation summary, proxy
metadata, hit regions, and object-layer/depth metadata that the observation
object exposed. Clients should use this metadata instead of reparsing the URI or
walking the whole object list after reading image bytes.
MCP `arcweft.capture` and `arcweft.resource.read` expose that metadata in the
JSON text block that accompanies image/blob content. Binary `resources/read`
contents still carry the MCP blob itself; clients that need image metadata
should use the Agent tool response or the session resource descriptors rather
than expecting metadata to be embedded inside the blob content.

---

## Scope semantics

`scope = viewport` captures the complete rendered frame at the requested
viewport size. This is the primary regression artifact for CI and human review.

`scope = layer { id }` captures the requested layer. A renderer may use a
framebuffer crop, a mask attachment, or an isolated-region render, but it must
report the actual `composition`. The layer descriptor lists its available
`capture_refs`.

`scope = object { id }` captures a single observed object. Rich-text child
objects are normal object scopes with additional rich-text metadata, not a
separate URI grammar. The object descriptor lists its available `capture_refs`.

Missing scopes are reported as diagnostics or failed capture results. Adapters
must not return a visually similar synthetic image under the requested native
capture URI.

---

## URI families

URI syntax is a transport detail, but stable families are required so MCP,
CLI, and LSP clients can discover resources before reading bytes:

```text
arcweft://session/{session}/frame/{frame}/observation.json
arcweft://session/{session}/frame/{frame}/viewport.png
arcweft://session/{session}/frame/{frame}/viewport.rgba
arcweft://session/{session}/frame/{frame}/layer.{id}.png
arcweft://session/{session}/frame/{frame}/layer.{id}.mask.png
arcweft://session/{session}/frame/{frame}/object.{id}.png
arcweft://session/{session}/frame/{frame}/object.{id}.mask.rgba
```

Paged rich-text child captures append `?page=N` when the object is only visible
on a non-zero rendered page. The `page` value is also present in metadata.

Clients should treat URIs as opaque identifiers after discovery. The resource
metadata is authoritative for renderer, scope, composition, dimensions, and
coordinate mapping. For object resources, `image.object` is authoritative for
the object metadata associated with the captured pixels. Rich-text object
captures preserve both the render `layer` and the semantic `object_layer` /
`object_depth` resolved from `rich_text_ref`, so a saved crop remains
self-describing when inspected without the original observation object list.
`image.object.bbox` and `image.object.polygon` preserve the viewport geometry of
the captured object, matching the object descriptor used for hit-testing and
object-id capture.
`image.object.capture_refs` repeats the captured object's color/object-id/mask
resource descriptors, letting clients navigate from one returned crop to its
sibling debug images without re-reading `objects.json`.

---

## Rich-text observation

Rich-text objects expose display-map references so a debugger can connect a
pixel to the source span that produced it:

```json
{
  "id": "object.dialogue.0.0",
  "role": "rich_text",
  "bbox": { "x": 32, "y": 520, "width": 640, "height": 120 },
  "rich_text": {
    "kind": "ruby",
    "range": { "start": 12, "end": 13 },
    "ruby": "まつりごと",
    "orientation": "upright",
    "ruby_base_bbox": { "x": 48, "y": 552, "width": 38, "height": 48 },
    "ruby_annotation_bbox": { "x": 44, "y": 532, "width": 46, "height": 14 }
  },
  "capture_refs": []
}
```

Required rich-text child kinds are `text_page`, `text_line`, `text_run`,
`text_glyph`, `glyph_cluster`, `ruby`, and `text_object_proxy`. Hit regions
distinguish `text_page`, `text_line`, `text_run`, `text_glyph`,
`glyph_cluster`, `text_object_proxy`, `ruby_object`, `ruby_base`, and
`ruby_annotation`. Vertical writing observations also report glyph orientation
and vertical form when known.

The observation should include the effective presentation summary needed to
debug rich text: layout fields, ruby defaults/overrides, transforms, effects,
shader refs, object proxy metadata, hit-test regions, source anchors, and the
resolved `object_layer` / `object_depth` used by text objects.
For `text_object_proxy` hit regions, the region itself carries the proxy id,
type, role, layer, depth, and `proxy_params`. `proxy_params` is the typed
`RichTextParam` map after struct-attribute defaults and inline overrides have
been resolved, and the same region shape is returned by `arcw agent hit-test`
and MCP `arcweft.hit_test`. Hit-test hits also carry the observed object's
`capture_refs`, including object-id color and color/object-id/mask resource
URIs, so hit consumers can treat text objects as directly capturable debug
objects without separately resolving the object list. Hit entries preserve the
observed object's viewport `bbox` and `polygon`, so hit-test results and
object-scoped image metadata describe the same target geometry.

---

## CLI and MCP mapping

CLI examples:

```bash
arcw agent observe game/routes/opening.arcw --json
arcw agent observe game/routes/opening.arcw --image png --out viewport.png --json
arcw agent observe game/routes/opening.arcw --image png --layer dialogue --out dialogue.png --json
arcw agent observe game/routes/opening.arcw --image raw-rgba --object object.dialogue.0.0 --out object.rgba --json
arcw agent observe game/routes/opening.arcw --read-uri arcweft://session/cli/frame/0/object.object.dialogue.0.0.png
arcw agent hit-test game/routes/opening.arcw --x 520 --y 540 --json
```

MCP exposes the same contract through:

- `arcweft.observe`
- `arcweft.capture`
- `arcweft.hit_test`
- `arcweft.resource.read`
- `arcweft.session.info`
- `resources/list`
- `resources/templates/list`
- `resources/read`

Tool-result image blocks are convenience wrappers around the same resource
body and metadata. They are not a separate capture model.

---

## Conformance

A conforming adapter must:

- preserve `arcweft-agent-protocol` field names and enum values
- return full-frame captures and discoverable layer/object capture refs
- report raw RGBA stride and pixel format for raw captures
- report renderer, scope, composition, dimensions, crop origin, content bbox,
  and content viewport bbox for image captures
- keep capture resources readable for the current session
- fail or diagnose unsupported native captures instead of substituting a
  synthetic compatibility renderer

CI visual regression should compare native capture artifacts. Synthetic or
alternate renderers may be separate diagnostic tools, but they must not share
native capture URIs or conformance labels.
