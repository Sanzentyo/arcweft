# Agent Debug Bus / MCP / CLI

## Agent Debug Bus

Verifier, CLI, and LSP diagnostics use the same stable JSON shape that future
Agent tools will embed into `Observation.diagnostics`. This means an Agent can
see an obligation id, source span, related proof/audit ids, and available
actions before the renderer/MCP runtime exists.

Runtime observation now has two Phase 2.0 headless slices:

- `arcw run --json` reports deterministic runtime step output, observations, and
  diagnostics.
- `arcw agent observe ... --json` reports the first Agent-shaped frame
  observation for dialogue/rich-text debugging. It resolves runtime dialogue
  events against the rich-text display catalog and returns textbox objects,
  viewport-space bounds, semantic advance actions, logs/signals/metrics/events,
  diagnostics, and optional image resources.

The shared diagnostic/action schema produced by `arcweft-verify` and consumed by
CLI/LSP remains the connection point for future Agent tools.
`arcweft-agent-protocol` owns the current observation JSON data types used by
the CLI slice and intended for future MCP resources/tools.
`arcweft-agent-mcp` maps those resources to MCP `resources/read` and tool-result
JSON shapes without owning stdio, HTTP, auth, or renderer sessions.
`arcw agent mcp` is the current minimal stdio transport. It accepts line-delimited
JSON-RPC requests for `initialize`, `tools/list`, `tools/call`, `resources/list`,
`resources/templates/list`, and `resources/read`. `resources/templates/list`
advertises the stable `arcweft://` URI families for viewport, layer, object, and
rich-text child captures before a source has been observed. `tools/call`
supports `arcweft.observe`, `arcweft.capture`, `arcweft.resource.read`, and
`arcweft.session.info`; the server keeps the latest one-shot observation in
memory so clients can observe once, inspect the current session/frame/resource
state, list resources, then either call `arcweft.capture` for a
viewport/layer/object PNG or raw RGBA image content, or read a specific
object/layer/rich-text child image URI. `arcweft.capture` can also take a
listed image `uri` directly, using that URI to infer format, capture kind, and
viewport/layer/object scope. Passing `source` to `arcweft.capture` runs a
bounded observation first and then returns the requested capture, which gives
LLM debuggers a single-call path for "show me this layer/object" workflows while
still updating `resources/list`. Capture resources are retained in the current
MCP session by URI, so later captures do not evict earlier PNG/raw resources;
`resources/read` and `arcweft.resource.read` return the same native renderer
composition, metadata, and image bytes that each capture tool call produced.
`arcweft.session.info` returns the latest frame identifiers, resources, images,
observed layers, observed objects, resource templates, and latest capture
metadata so a debugger can recover current capture options, object ids, and
rich-text child capture refs without rereading the whole observation. It also
includes `latest_capture_uri` and a `latest_capture_resource` descriptor, so a
client can immediately read or recapture the most recent PNG/raw image without
matching it against `resources/list`. `capture_resource_count` records how many
tool-produced capture resources are currently cached in the session.
When the latest observation generated a selected image, reading that image URI
through `resources/read` or `arcweft.resource.read` returns the cached image
bytes and metadata from the same observation. This preserves native framebuffer,
native crop, masked framebuffer crop, isolated-region rich-text layer/object
captures, and native object-id/mask attachments.
`resources/list` includes the standard observation/log/signal/audio resources,
the selected frame image when present, layer color/object-id/mask PNG/raw
capture refs, and object-local color/object-id/mask PNG/raw capture refs for
textbox and rich-text child objects. Image descriptors also summarize the
image kind, renderer, scope, composition, and dimensions in their MCP
description field, so debuggers can choose a viewport, layer, or object capture
from the list before fetching the full resource body and metadata.

```arcw
pub trait AgentDebugBus {
    fn observe(&mut self, req: ObserveRequest) -> Result<Observation, AgentError>;
    fn act(&mut self, action: AgentAction) -> Result<ActionResult, AgentError>;
    fn resource(&mut self, id: AgentResourceId) -> Result<AgentResource, AgentError>;
    fn subscribe(&mut self, filter: EventFilter) -> AgentEventStream;
}
```

## Observation

```arcw
pub struct Observation {
    pub session_id: SessionId,
    pub tick: TickId,
    pub frame_id: FrameId,
    pub state_hash: StateHash,
    pub render_hash: RenderHash,
    pub viewport: ViewportInfo,
    pub images: Vec<ImageResource>,
    pub layers: Vec<ObservedLayer>,
    pub objects: Vec<ObservedObject>,
    pub actions: Vec<ActionTarget>,
    pub ui_tree: Option<UiTree>,
    pub scene_graph: Option<SceneGraphSlice>,
    pub audio_state: Option<AudioObservation>,
    pub logs: Vec<DecodedLog>,
    pub signals: Vec<SignalSnapshot>,
    pub diagnostics: Vec<Diagnostic>,
}

pub struct ObservedLayer {
    pub id: LayerId,
    pub bbox: BBox,
    pub object_count: u32,
    pub capture_refs: Vec<ImageResource>,
}
```

## Image / bbox / polygon / mask

- color screenshot
- overlay screenshot
- object-id image
- layer/object crop image
- bbox
- polygon
- segmentation mask: RLE / PNG alpha / raw bitmap

## Action

Physical:

```arcw
PointerClick { x, y, space, button }
PointerDrag { from, to, duration_ms }
KeyDown / KeyUp
TypeText
```

Semantic:

```arcw
Invoke { target, action, args }
SelectChoice { choice }
AdvanceText
OpenMenu { menu }
SetSlider { target, value }
AudioSetBus { bus, gain }
```

Semantic action を優先し、座標 click は fallback。

## CLI

Current implemented source/profile observation slice:

```bash
arcw agent mcp
arcw agent observe game/routes/opening.arcw --json
arcw agent observe game/routes/opening.arcw --json --image overlay
arcw agent observe game/routes/opening.arcw --image overlay --out overlay.svg
arcw agent observe game/routes/opening.arcw --image png --out native.png --json
arcw agent observe game/routes/opening.arcw --image png --layer dialogue --out dialogue.png --json
arcw agent observe game/routes/opening.arcw --image png --capture object-id --layer dialogue --out object-id.png --json
arcw agent observe game/routes/opening.arcw --image raw-rgba --capture mask --object object.dialogue.0.0 --out object-mask.rgba --json
arcw agent observe game/routes/opening.arcw --image png --layer dialogue --resource image
arcw agent observe game/routes/opening.arcw --image png --layer dialogue --resource image --mcp
arcw agent observe game/routes/opening.arcw --image png --capture object-id --layer dialogue --resource all --mcp --mcp-format list
arcw agent observe game/routes/opening.arcw --image png --layer dialogue --resource image --mcp --mcp-format tool-result
arcw agent observe game/routes/opening.arcw --read-uri arcweft://session/cli/frame/0/object.object.dialogue.0.0.mask.rgba
arcw agent observe game/routes/opening.arcw --read-uri arcweft://session/cli/frame/0/object.object.dialogue.0.0.png --mcp --mcp-format tool-result
arcw agent observe game/routes/opening.arcw --image raw-rgba --object object.dialogue.0.0 --out object.rgba --json
arcw agent observe --manifest arcw.toml --profile game.dev --json
```

The current CLI image resources use the same observation `images` slots planned
for MCP. `--image overlay` emits `kind = "overlay_svg"` and embeds the SVG body
when requested. `--image png` and `--image raw-rgba` emit `kind = "color"` by
default, or `kind = "object_id"` / `kind = "mask"` with `--capture object-id`
or `--capture mask`. `--resource image` returns the selected image as an
`AgentResource` whose body is base64-encoded binary data, and `--resource image
--mcp` returns MCP `resources/read` compatible contents with a base64 `blob`.
The MCP server also supports `resources/templates/list`, which returns the
current URI template families for observation JSON, object JSON, viewport
captures, layer color/object-id/mask captures, and object color/object-id/mask
captures.
`--mcp-format list` returns MCP `resources/list` compatible descriptors for the
selected resources and object-local capture refs when `--resource all` is used,
including image kind, renderer, scope, composition, and dimensions in each image
descriptor description. `--mcp-format tool-result` returns MCP content blocks;
single image resources become a compact JSON metadata text block followed by an
image content block for multimodal clients, and multi-resource observations
become resource links. Image metadata includes the producing `renderer`
(`native`), structured `scope` (`viewport`, `layer`, or `object`), and
`composition` (`overlay_vector`, `framebuffer`, `framebuffer_crop`,
`object_id_attachment`, `mask_attachment`, `masked_framebuffer_crop`,
`isolated_regions`, or `debug_geometry`), so clients do not need to parse the
URI to know what was captured, which rendering path produced it, or whether the
image is a crop, isolated selected-region render, object-id/mask attachment,
masked framebuffer crop, debug-geometry pass, or diagnostic attachment. Raw
RGBA metadata includes `pixel_format = "rgba8_unorm"` and `row_stride_bytes =
width * 4` so tools can decode the blob without guessing. Raster metadata also
includes image-local `content_bbox`, viewport-space `content_viewport_bbox`,
and `content_pixels`, measured against the capture background, to expose empty
crops and bbox drift without requiring an image decoder. Cropped layer/object
images include `crop_origin` in viewport coordinates, so an Agent can map
image-local pixels back to observed object bboxes and rich-text display ranges
or read the already-translated `content_viewport_bbox` directly.
For MCP sessions, repeated `arcweft.capture` calls reuse a native capture
session after the first native capture; `arcweft.session.info` reports this via
`native_capture_session_active`. If the same command also generated an image
with `--image`, `--read-uri` returns the cached selected image when the URI
matches `images[0].uri`, so native framebuffer, layer/object crop,
isolated-rich-text, and object-id/mask attachment captures keep their original
bytes and metadata on readback. If `--read-uri` targets a capture URI and no
cached image matches it, the resource is reconstructed through the native
capture path.
Rich-text child capture refs for non-zero rendered pages append `?page=N`; URI
readback parses that query and uses the native renderer automatically. The refs
also include `page = N` metadata for non-zero pages, which lets MCP/CLI clients
follow an observed capture ref for text after `[p]`, line-wait, or `[clear]`
without separately tracking page state. Native page-selected rich-text layer
captures filter out rich-text child objects that are not visible on the
requested rendered page before computing the crop, so layer crops after
`[clear]` do not include stale child bboxes from earlier pages.
`--layer` crops to the selected layer's object bounds; `--object` crops to one
observed object's bbox. PNG/raw output is produced by the native
`wgpu`/`glyphon` offscreen framebuffer readback. Full viewport, layer bbox
crops, and object bbox crops are supported. Rich-text child object crops use
native text layout bounds for text runs, ruby annotations, and glyph clusters.
The implemented object slice is focused on rich-text/textbox debugging: each
observed layer includes a viewport bbox, object count, and stable
`capture_refs` for color/object-id/mask PNG and raw RGBA images. Each
observed textbox includes resolved display text, structured rich-text nodes,
host events, inline interpolation failures, base styles, viewport bbox, polygon,
object-local `capture_refs` for color/object-id/mask PNG and raw RGBA crops, the
object-id debug color used in object-id images, rich-text `display_map` ranges
for text/interpolation/ruby/control output, and a semantic `advance_text` action.
Visible rich-text text runs, ruby annotations, and glyph clusters are also
exposed as `dialogue.rich_text` child objects with their own bbox,
`capture_refs`, and a structured `rich_text_ref` pointing back to the parent
display-map element, so an Agent can request a crop such as
`arcweft://session/cli/frame/0/object.object.dialogue.0.0.ruby.0.png` when a
specific inline element needs visual inspection. For child objects on later
rendered pages, `rich_text_ref.page` records the page and capture refs include a
matching `?page=N` query plus `capture_refs[].page` metadata. Child object
bboxes come from the native text layout metrics used by native image capture,
so image crop origins and observed child bboxes describe the same glyph, ruby,
and cluster geometry. If native metrics are unavailable, the child object is omitted
instead of emitting an approximate bbox.
The same `display_map` is also consumed by the native rich-text window path, so
Agents can compare an object or layer capture against the exact resolved text
ranges and style/ruby metadata that the player adapter uses.

Planned session-control commands remain:

```bash
arcw agent audio state --json
arcw agent tts preview voice.alice.tts "おはよう" --out voice.wav
arcweft://session/{sid}/observation/latest.json
arcweft://session/{sid}/frame/{tick}/color.png
arcweft://session/{sid}/frame/{tick}/color.rgba
arcweft://session/{sid}/frame/{tick}/layer.{layer}.png
arcweft://session/{sid}/frame/{tick}/object.{object_id}.png
arcweft://session/{sid}/frame/{tick}/overlay.png
arcweft://session/{sid}/frame/{tick}/objects.json
arcweft://session/{sid}/state/current.json
arcweft://session/{sid}/logs.ndjson
arcweft://session/{sid}/signals.json
arcweft://session/{sid}/audio.json
arcweft://session/{sid}/observation/latest.json
arcweft://session/{sid}/frame/{tick}/color.png
arcweft://session/{sid}/frame/{tick}/color.rgba
arcweft://session/{sid}/frame/{tick}/layer.{layer}.png
arcweft://session/{sid}/frame/{tick}/object.{object_id}.png
arcweft://session/{sid}/frame/{tick}/overlay.png
arcweft://session/{sid}/frame/{tick}/objects.json
arcweft://session/{sid}/state/current.json
arcweft://session/{sid}/logs.ndjson
arcweft://session/{sid}/signals.json
arcweft://session/{sid}/audio.json
```

Tools:

```text
arcweft.observe
arcweft.capture
arcweft.resource.read
arcweft.session.info
arcweft.click
arcweft.invoke
arcweft.choose
arcweft.advance_text
arcweft.wait_until
arcweft.step_frames
arcweft.get_state
arcweft.log_query
arcweft.signal_get
arcweft.audio_state
arcweft.tts_preview
arcweft.shader_preview
```

## Product flags

```text
--agent=off
--agent=observe
--agent=control
--agent=debug
```

Capabilities:

```arcw
pub struct AgentPermissions {
    pub observe_image: bool,
    pub observe_state: bool,
    pub observe_audio: bool,
    pub control_input: bool,
    pub semantic_actions: bool,
    pub mutate_state: bool,
    pub hot_reload: bool,
}
```

Product mode は token、audit log、debug indicator、redaction 必須。


