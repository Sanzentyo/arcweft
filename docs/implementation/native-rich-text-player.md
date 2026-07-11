# Native Rich Text Player MVP

This note records the current implementation state for the first Arcweft rich text player path.
Native rendering and capture now live in `arcweft-render-native`; the native
player host delegates to that renderer instead of owning the renderer module
itself.
It is implementation-state documentation, not a language specification.

## Status

Arcweft now has a Sans I/O rich text display model in `crates/arcweft-render-text`.
Runtime-plan lowering emits a `LineDisplayCatalog` sidecar, and flow execution emits dialogue line
events with a snapshot of runtime bindings. Shared source compilation is owned
by `arcweft-compiler`; the native player crate uses that driver for developer
source execution. Product `.awfb` execution uses the `arcweft-runtime-host`
bundle runner, keeps typed flow events in the runner report, resolves
interpolation from the binding snapshot, and returns display frames as JSON.

The CLI also exposes this display path through `arcw agent observe`. That command runs the same
checked source and runtime executor path, keeps the display catalog, resolves emitted dialogue lines,
and returns `arcweft-agent-protocol` observation JSON with textbox objects, viewport bounds,
semantic actions, diagnostics, optional overlay SVG, and deterministic PNG/raw RGBA debug captures
for the full viewport, a selected layer bbox, or a selected object bbox. Native
offscreen capture is built from resolved display text plus `display_map` text
runs and ruby annotations, so interpolation output,
inline color/size/generic-and-named-font/strong/em styling, and inline ruby captures use the same ranges reported in observation JSON.
It uses system fonts when available so Unicode dialogue text is visible in captures, and falls back to
a built-in ASCII debug glyph set on minimal environments. Observed layers and objects include `capture_refs` so an
Agent can discover the stable color/object-id/mask PNG and raw RGBA crop URIs
for a layer or element before requesting a specific image resource. Resolved
rich-text frames include `display_map` metadata that
maps text runs and ruby annotations back to byte ranges in the displayed text; those text runs and
ruby annotations are also emitted as `dialogue.rich_text` child objects with `rich_text_ref`
metadata and their own crop URIs.
Typed text proxy spans are also native-addressable elements, not only JSON
metadata on their parent run. `NativeFrameElement::TextObjectProxy` uses the
source text-run index plus proxy index, resolves to the same post-transform
glyph geometry as the visible span, and drives proxy object color/mask/object-id
captures through the normal native element path. This keeps custom
`#[text_proxy]` / `[object ...]` spans aligned with image/model-like debug
objects that have their own object id, hit region, layer/depth metadata, and
capture refs.
Rich-text page and line child objects now aggregate the proxy metadata in their
covered text ranges as native-bounds-backed `text_object_proxy` hit regions.
The page/line primary bbox still describes the broad text object, but its
`rich_text_ref.object_layer`, `object_depth`, and proxy hit regions expose the
deepest nested proxy targets inside the range without substituting a fallback
rectangle.
Agent hit-test reports the resolved semantic layer on each hit's top-level
`layer` field, preferring proxy layer, then rich-text object layer, then the
render layer. This keeps headless input/debug routing aligned with semantic
text object layers while preserving the original render-layer capture path.
Hit-test hits also copy the observed object's `capture_refs`, so the same result
contains the object-id color and color/object-id/mask crop URIs for the selected
text object. Hit results also preserve the observed object's viewport polygon,
matching the geometry later returned by object-scoped image metadata. Hit
results include an `object` descriptor with the same `AgentImageObjectRef` shape
used by image metadata, keeping hit results and returned object crops aligned as
typed text object descriptors.
Native text shaping disables standard/contextual ligatures for submitted body
and ruby buffers. The current layout model maps styled/rich-text source ranges
to per-character layout glyphs before native shaping, so ligature clusters such
as `ffi` would otherwise overlap several source-local glyph slots and be drawn
more than once. The long-term target is shaping-aware cluster layout; until
then, disabling `liga`/`clig` keeps Latin words such as `offset`, `effect`, and
`serif` readable in Japanese rich-text/effect samples without adding a renderer
compatibility path.
For ordinary horizontal, non-ruby text runs, native submission now also feeds
glyphon shaped advances back into glyph origins before applying transforms.
This keeps Latin words such as `serif`, `style`, and `transform` from inheriting
the Sans I/O fallback width table in the actual framebuffer. Native visual-plan
glyph placements and measured text-run/glyph-cluster bounds use the same shaped
horizontal advances, so Agent object crops, mask/object-id captures, and LLM
debug geometry align with the submitted pixels. The adjustment is bounded to one
run and one line and resets around ruby bases, so long-ruby base allocation and
vertical layout continue to come from `arcweft-text-layout`.
For vertical mixed text, consecutive Latin/Greek graphemes that resolve to
sideways orientation are now one layout cluster rather than per-character
clusters. `arcweft-text-layout` gives that cluster a vertical inline extent
based on its horizontal shaping advance, and the glyphon adapter maps the
resolved horizontal glyph offsets into vertical progression before applying the
engine-side `Rotate90Cw` transform. This matches the long-term sideways-run
policy without introducing a separate compatibility renderer.
The native shader path resolves run-offscreen shader IDs through
`RichTextShaderRegistry` as deterministic additional glyph passes submitted
before the main text/ruby glyph areas. The default registry provides
`soft_glow` and `warm_glow`; `NativeOffscreenCaptureSession::shader_registry_mut`
allows tests and adapters to register additional shader IDs. Agent observe
coverage checks the full grammar sample by reading the `shader` run's raw RGBA
object crop and requiring visible blue-tinted glow pixels, while the effects
animation sample checks a separate `warm_glow` object crop for warm-tinted
pixels. These guards prove shader constructs affect rendered output rather than
only display-map metadata.
Step-pinned capture is part of the same native/debug surface: `--capture-step`
forces observe to run to the requested runtime step, and unless an explicit
`--capture-time` is supplied the renderer uses that step as deterministic effect
time. Full grammar coverage reads the `typewriter` run's raw mask crop at
`--capture-step 1` and compares it with the same step plus `--capture-time 0`,
proving animated glyph-mask effects can be inspected after a specified step. It
also reads object-scoped raw RGBA color crops for `.wave`, `.shake`,
`.sparkle`, and `.host id=sparkle`, proving native glyph-transform and
host-dispatched effects change observable pixels at requested debug samples
rather than only surviving as display-map metadata.
The dedicated `rich-text-effects-animation.arcw` regression also captures one
run carrying `typewriter + wave + shake + sparkle`: its object mask has zero
visible pixels at `--capture-time 0`, visible pixels at a later pinned sample,
and object color crops differ between two later samples. It also checks
horizontal and `vertical_rl` `spin + pulse` runs, where animated rotation and
scale alter the native object color crop between pinned samples while preserving
vertical layout metadata.

Source `#[fx] fn ... -> Fx` declarations now compile into renderer-independent
typed graphs, and RichText can expand their static text/color nodes. Dynamic
`Fx.transform` samplers are deliberately not claimed as native-executable yet:
the current renderer leaf has no typed closure program or `FxAbiHash` dispatch
boundary. Dynamic Fx leaves keep the complete `FxId` in their observable
selector so a function named `wave` cannot silently execute the unrelated
built-in `.wave` implementation by basename collision. The remaining bundle
graph transport and native/Web sampler ABI are specified in the linked Fx
implementation note and follow-up request. Existing capture evidence in this
document continues to cover the independent low-level rich-text effect
descriptors and registered host effects only.
Native window page changes reset the page-local effect clock and clear the
renderer-local rich-text effect state store before preparing the next page. A
cancelled or skipped line is therefore treated like a page/line replacement for
this native path: no old animation state is advanced after the page is replaced,
and subsequent pages start with fresh renderer-local effect state.
When native element bounds are unavailable, fallback child bboxes and ruby
placement advance through the same display-map run styles as native capture,
so size, weight, italic, font-family, and textbox-width wrapping influence
cursor positions instead of assuming a single fixed-width text stream.
This is the current LLM/debugger-facing way to inspect rich text rendering without starting the
native window.

The native window path uses `winit` and `glyphon` through `crates/arcweft-player-native`.
The window renderer consumes resolved `LineDisplayFrame` values and prefers the frame's
`display_map` as the authoritative mapping from visible text byte ranges to styles, ruby
annotations, and page/line-wait/clear controls. This keeps the native window aligned with the same
debug metadata exposed through `arcw agent observe` and MCP-style resources, instead of having each
adapter reinterpret authored rich-text nodes independently. Ruby base text remains in the main line
while ruby annotations are shaped with glyphon buffers and submitted as absolute `GlyphArea`
geometry positioned from `LaidOutText`, instead of being inserted as `base(ruby)` fallback text.
For main text, native submission resolves renderer cache keys for every Arcweft layout glyph:
the rich glyphon buffer supplies the normal shaped keys, vertical alternates use per-glyph
vertical-feature shaping, and any remaining glyph missing from the rich buffer is shaped from
its own text/style before `GlyphArea` construction. This keeps object/layer image capture stable
when a line mixes many inline spans, shaders, effects, and wrapped horizontal text.
Page, line-wait, and clear
controls split frames into native pages; Space, Enter, or `n` advances to the
next page, and Escape closes the window. The
renderer is deliberately outside `arcweft-core`; `arcweft-core` remains Sans I/O and only emits
typed runtime events.

## Current Render Model

The display sidecar carries these value families:

- text nodes and ruby nodes for renderable text
- control nodes for page, wait, hard break, clear, reset, mark, and raw text
- typed style start/end nodes for inline styling, including font family requests
- effective base styles lowered from global dialogue defaults, character `dialogue_style`, line
  `style=font(...)` / `style=text_style(...)`, and direct line color/font/size options
- effective interpolation failure policy lowered from global dialogue defaults, character
  `dialogue_style`, and line `inline_fallback` or `inline_error`
- host events for voice, face, pose, show, hide, move, scale, rotate, anim,
  shake, call, signal, rich-text `phase=host_event` effects, and conditionals
- interpolation nodes that resolve against the runtime binding snapshot

Headless output separates renderable `text`, structured `nodes`, `host_events`, and unresolved
interpolations. Interpolation nodes carry an `InlineFailurePolicy`, and resolved frames separate
recorded inline failures from explicit fallback text or discard. `InlineFailure.fail` makes frame
resolution return a typed error instead of falling back. This keeps renderer policy and host-side
effects out of the parser and core runtime.
`display_map` is the adapter-facing render contract for resolved frames: it records visible text
runs, interpolation and fallback ranges, ruby base ranges, control-produced text such as hard
breaks/raw text, and host event node indices. Native rendering and Agent observation tests both use
this map to correlate captured pixels, object crops, and rich-text semantics.
The resolver now treats `[reset]` as a style/reveal reset for following runs by
clearing active inline styles after recording the reset control marker, so
Agent JSON and native display no longer leak prior inline styles across a reset.
Rich-text presentation spans also carry renderer-facing layout, transform,
effect, and shader descriptors. Runtime-plan lowering accepts inferred dot
selectors such as `[.shake]...[/]`, canonical tooling can expand them to
`[effect .shake]...[/effect]`, and the display map stores the resulting
presentation on text runs and ruby annotations. Effect parameters keep
non-trivial authoring values as raw tokens; native builtins interpret only
known parameters at the renderer boundary, such as `dir=0,1` for wave direction
or raw string seeds for deterministic shake jitter.
Transform lowering preserves authored `target` and `origin` fields. Rotation
spans accept named `angle=...` / `deg=...` values and positional angle tokens,
and `origin=baseline_start|baseline_center|center|glyph_center` is carried into
the native placement pivot calculation instead of being collapsed to the
selector default.
The native crate exposes a deterministic `NativeVisualPlan` that applies
resolved transforms, vertical layout hints, builtin glyph effects, and shader
filter references without creating a compatibility renderer. The same
presentation data is available to native debug/capture paths through
`LineDisplayFrame::display_map`.
The glyphon-backed native renderer now applies post-layout translate, rotate,
scale, skew, pivot-origin affine transforms, and builtin placement effects such
as wave/shake/arc to the actual submitted glyph instances, including broad
effect targets after they have been resolved onto a run's presentation. Glyph
affines resolve `target=glyph`, `target=run`, and broad line/textbox/screen
targets against the corresponding layout bounds before converting the pivot to
glyph-local coordinates, so center-origin rotation and scaling act on the
requested group instead of each glyph independently. Ruby
annotation GlyphAreas use the same submitted-glyph presentation path, so ruby
placement follows translate, rotate, scale, skew, origin pivots, and builtin
placement effects instead of only inheriting reveal alpha. Native ruby element
observation also applies the same presentation to its reported base and
annotation geometry before converting layout rects to viewport bboxes, keeping
Agent object crops aligned with transformed ruby captures.
Horizontal ruby annotations are also adapted through an absolute GlyphArea path:
their inline x positions still come from glyphon shaping, while y placement uses
the Arcweft ruby annotation track plus glyphon baseline offset and the layout
ruby line height. This keeps horizontal ruby pixels close to the body text and
aligned with observed ruby bboxes instead of drifting through an independent
TextArea baseline. The horizontal ruby track is calibrated against a Chromium
`<ruby><rb><rt>` reference: a 30px base with 13px ruby text produces about
4.67px of annotation/base bbox overlap, so Arcweft applies a 0.36em natural
overlap before adding any explicit ruby gap. The same model applies to
horizontal `ruby_under` and vertical side-track ruby: `ruby_gap` remains an
author-controlled extra gap, while the default zero-gap placement allows the
small natural annotation/base bbox overlap observed in Chromium ruby rendering
instead of forcing a hard separated rectangle.
Builtin wave effects use the descriptor target when choosing their phase index:
`target=glyph` evaluates per glyph, while `target=run` and broader targets move
the target as one placement group. Shake and jitter continue to use
`state_scope` for deterministic grouping.
The deterministic native visual plan exposes renderer diagnostics and can be
built with a `RichTextEffectRegistry`, `RichTextShaderRegistry`, and
`RichTextMotionRegistry`; builtin effect IDs are handled directly, registered
custom effect IDs run against `TextEffectGlyphContext`, registered shader IDs
emit deterministic glyph passes or post-process color readbacks, registered
motion function IDs drive `.motion`, and missing registries or unsupported
phases are reported instead of being silently reinterpreted as builtins.
`NativeOffscreenCaptureSession` also owns mutable effect/shader/motion
registries and a shared state store; custom registered placement and
`glyph_color` effects, source-local pure text effects, source-local pure text
shaders, run-offscreen shaders, and motion functions run when preparing
submitted glyphs for framebuffer, color, object-id, and mask captures, so
registry-backed custom behavior is visible in actual native image output rather
than only in plan snapshots. The same session
can measure native element bounds through
`measure_frame_elements_in`, and the standalone
`measure_frame_elements_with_effect_registry` API accepts an explicit
registry/state pair. This keeps glyph and ruby observe bboxes aligned with
registry-backed captures instead of reporting builtin-only debug geometry.
Native framebuffer, color-region, object-id, and mask captures preserve
renderer diagnostics from glyph submission. `arcw agent observe --json --image
...` appends those diagnostics to the Agent report, so missing custom native
effect implementations are visible to LLM/debugger tooling instead of becoming
silent no-op captures. Offscreen capture sessions and the native window renderer
install the native default host effect registry, which currently provides a
deterministic `sparkle` glyph effect for samples and smoke captures. The same
registry entry also supports `phase=glyph_color`, where it tints glyphs instead
of moving them, and `phase=post_process`, where it tints the native framebuffer
after glyph submission. Unknown custom IDs still report through diagnostics.
Surface `.host` selectors use their `id`, `effect`, or `name` metadata as the
registry id before native dispatch, so `[.host id=sparkle]...[/]` reaches the
same default registry entry as an explicit custom effect descriptor with id
`sparkle`.
Runtime-plan lowering now uses the same visible `#[text_proxy]` /
`#[rich_text_proxy]` struct registry as canonical tooling for inferred inline
selectors. `[.id type=KeywordHit]...[/]`, `[.id struct=KeywordHit]...[/]`,
`[.id proxy=KeywordHit]...[/]`, and `[.KeywordHit]...[/]` lower directly to
typed `RichTextStyle::Object` proxies when `KeywordHit` is a declared text proxy
type, while ordinary custom selectors such as `[.sparkle]` remain effect spans.
Those inferred proxies therefore produce the same run/page/line metadata,
`rich_text_proxy` observed objects, object-id/mask/color captures, and hit-test
regions as explicit `[object ...]` syntax.
Resolved object proxies now preserve typed declaration provenance from the
Arcweft struct that supplied defaults. The selected `object_proxies[]` entry and
each `text_object_proxy` hit region expose the struct name plus the attribute
family (`text_proxy` or `rich_text_proxy`), so Agent observe, image metadata,
and hit-test results remain self-describing even when the proxy type name is
registry-facing or inline metadata overrides defaults.
Ordinary text objects also expose presentation scalar metadata without becoming
proxies. Style-family selectors such as `[style .layer hud]...[/style]`,
`[style .z_index 3]...[/style]`, and `[style .opacity 0.8]...[/style]`,
including inferred forms like `[.layer hud]...[/]` and `[.z_index 3]...[/]`,
lower into `RichTextPresentation`. Agent observe reports the resolved
`presentation.layer`, `presentation.z_index`, and `presentation.opacity` on
runs, lines, and pages, maps presentation layer to `rich_text_ref.object_layer`,
and derives `rich_text_ref.object_depth = z_index * 1000` so ordinary text
participates in the same layer/depth-aware object ordering as images, models,
and custom proxy spans. Proxy objects still keep their own layer/depth metadata;
when a proxy layer is omitted it inherits the parent presentation layer for
Agent hit-test reporting.
Style-family `[style .meta ...]` and inferred `[.meta ...]` spans also lower to
ordinary `RichTextPresentation.params`. Agent observe serializes those typed
params inside `rich_text_ref.presentation.params` on runs, glyphs, clusters,
lines, pages, and ruby objects. This is separate from proxy `params`, which stay
inside the selected `object_proxies[]` entry and hit-test `proxy_params`.
Page and line objects build their `rich_text_ref.presentation` by merging the
overlapping text-run presentations in source order. They therefore preserve the
same metadata surface as run/glyph/cluster objects for object-scoped capture and
readback, while their top-level `object_layer` / `object_depth` still select the
deepest overlapping proxy when one is present.
Agent layer aggregation and `--layer` capture selection include both the render
layer stored on `AgentObservedObject.layer` and a rich-text child's semantic
`rich_text_ref.object_layer`. A text run can therefore remain in the dialogue
render layer while still being capturable as the `hud` or `ui` semantic object
layer used by input/debug tooling.
Typewriter `glyph_mask` effects use the same capture-time clock as other native
rich-text effects and honor `delay` before revealing glyphs. `delay` is
interpreted as seconds, with renderer-local raw token support for `s` and `ms`
suffixes such as `0.5s` or `500ms`, and it affects visual-plan glyph opacity,
framebuffer alpha, mask capture, and object-id visibility without changing text
layout geometry. `cursor=true` exposes the next unrevealed glyph as a low-alpha
ghost preview. The preview uses `cursor_alpha` / `cursor_opacity` when present,
is visible in framebuffer/mask/object-id captures, and still preserves the same
layout geometry as the fully revealed line.
For `before_layout` and `layout_transform` builtin placement effects,
`arcweft-text-layout` reserves the deterministic displacement envelope in
horizontal advances, vertical column planning, glyph bounds, and ruby base
allocation before native glyph submission. The native renderer still applies the
time-specific placement offset when drawing, while layout/ruby planning now
accounts for the space those layout-phase effects can occupy.
The native renderer maps registered `[effect .shader id=... phase=run_offscreen_pass]`
references to deterministic glyph-area passes submitted before the main glyph
pass, maps registered `phase=glyph_color` shader refs to main-glyph tint
overrides, and maps registered `phase=post_process` shader refs to deterministic
RGBA post-processing for native framebuffer and isolated color captures.
Object-id and mask captures remain pure identification attachments and are not
post-processed. The default shader registry provides `soft_glow`, `warm_glow`,
and `screen_tint`; custom native adapters can register additional post-process
shader IDs through `RichTextShaderRegistry::insert_post_process_lambda`. Unknown
shader IDs are not reinterpreted by the native renderer; they remain
host-resolved shader references until a concrete native/filter implementation
is added and are reported through renderer diagnostics.
Native builtin rich-text effects support `phase=post_process` as framebuffer
passes instead of reinterpreting that phase as glyph placement. Wave, shake, and
jitter displace the color framebuffer; arc, spin, pulse, and motion apply a
deterministic visual tint for post-process inspection. Custom native adapters
can register effect post-process passes through
`RichTextEffectRegistry::insert_post_process_lambda`, and object-id/mask
captures remain pure identification attachments. `host_event` phase effects
leave the visual pipeline during lowering and are exposed as typed
`DialogueHostEvent::Effect` markers instead of renderer diagnostics.

Inline dialogue function calls must declare per-call handling through `on_error`, `fallback`, or
`discard_error`, unless the line or speaker preset supplies `inline_fallback` or `inline_error`.
Canonical values are `InlineFailure.fail`, `InlineFailure.discard`, and
`InlineFailure.fallback(...)`; `.fail` and `.discard` are context-sensitive shorthand where an
`InlineFailure` is expected. `on_error`, `fallback`, and `discard_error` are mutually exclusive.
Runtime-plan lowering applies global defaults first, then character-level `dialogue_style`, then
line-local options, so later style entries override earlier entries in renderer adapters.

## Usage

Run an `.awfb` bytecode bundle without source compilation:

```bash
cargo run -p arcweft-cli --quiet -- bundle path/to/file.arcw --output target/game.awfb
cargo run -p arcweft-player-native --quiet -- --headless --json target/game.awfb
```

Run a source file in explicit developer mode:

```bash
cargo run -p arcweft-player-native --features dev-source -- --headless --json --source path/to/file.arcw
```

Bundles carry the line display catalog needed for native window presentation.
Product/native player entry uses `.awfb` input through the runtime-host bundle
runner; source input is a development convenience only. Native capture is a
development/debug harness and is exposed by the `dev-capture` feature rather
than the default product player argv. The `native_capture` JSON report field and
`NativePlayerCaptureMetadata` API are also present only in `dev-capture` builds.
Default product-player headless JSON still exposes product runtime metadata:
the `runtime` report section records the runtime-host source label, bytecode
instruction count, adapter manifest count, executor choice, executor stats, and
native I/O scheduler stats. This keeps product lifecycle/scheduler readback
available without exposing framebuffer debug capture data.

Capture the first resolved frame through the native `wgpu`/`glyphon` offscreen
renderer and include readback metadata in the JSON report:

```bash
cargo run -p arcweft-player-native --features dev-capture -- --headless --json --capture png --capture-out native.png target/game.awfb
cargo run -p arcweft-player-native --features dev-capture -- --headless --json --capture raw-rgba --capture-out native.rgba --capture-width 960 --capture-height 540 target/game.awfb
```

The native capture path renders to an offscreen texture, copies the texture to a
readback buffer, strips WebGPU row padding, and reports `pixel_format =
"rgba8_unorm"`, `row_stride_bytes`, image-local `content_bbox`,
viewport-space `content_viewport_bbox`, and `content_pixels`. This is the first
real native framebuffer readback path. `arcw agent observe --image png` and MCP `image: "png"` use that path for full-viewport, layer-bbox, and object-bbox color PNG/raw RGBA captures. The native Agent
capture path accepts `--page N` and MCP `page: N` for 0-based rendered rich-text
pages, so LLM/debugger tools can capture text after `[p]`, line waits, or
`[clear]` without opening the native window. Non-zero page selection is handled by the native renderer.
It also accepts `--capture-time SECONDS` and MCP `capture_time` as the native
presentation animation sample time for glyph effects, shaders, registry-backed
motion functions, typewriter visibility, animated proxy bounds, hit-testing,
animated image frame selection, and image capture. Source ranges and object ids
remain stable, while the native measurement and render paths sample visual
bboxes, hit regions, GlyphArea alpha/color, active image frames,
object-id/mask attachments, and object crops at the requested time. Native
Agent tests cover ordinary vertical clusters,
text-combine-upright digit clusters, animated proxy objects, ruby objects, and
function-backed motion/effect/shader runs, so combined cells and source-local
registry paths are checked against the same readback rule. Ruby annotation
GlyphAreas use the annotation presentation as well, so ruby object masks can be
captured before and after reveal while keeping stable ruby source identity and
page-local object ids.
For animation/debugging workflows that need a deterministic runtime state, the
same Agent path accepts `--capture-step N` and MCP `capture_step`. This overrides
the normal `--steps` loop, advances observation through exactly `N` runtime
steps even if the flow has already reached `Done`, and records that value in
`images[].capture_step`. If `--capture-time` / MCP `capture_time` is omitted,
the native capture uses `capture_step` seconds as the deterministic visual
effect time; an explicit capture time still overrides that default. Image
metadata records non-zero visual time as `images[].capture_time_millis`.
Observation reports also retain optional root-level `capture_time_millis` for
explicit or step-derived visual time, so later MCP capture calls and Agent URI
readback use the same animation state instead of recomputing a time from the
completed step count.
Use `capture_step` to choose the dialogue/runtime state and `capture_time` to
choose the visual-effect time inside that state when those should differ.
Native measurement now also has a time-aware page API, and Agent observe/crop
paths pass `capture_time` through it, so animated glyph-transform bboxes used
for rich-text child objects, textbox capture refs, and scoped native crops track
the same effect time as the rendered framebuffer.
Agent hit-testing consumes those same time-aware observed hit regions: an
animated proxy span can be hit at its sampled `capture_time` position, while the
same viewport coordinate does not hit that proxy at a different sampled time if
the glyph-transform bbox has moved away.
Object-scoped mask and object-id captures for animated rich-text proxies use
that same sampled bbox, and their `content_viewport_bbox` is checked against the
sampled proxy object rather than the time-zero proxy geometry.
Object-scoped native captures can also resolve rich-text child object IDs that
are absent from the current observation object list because a visibility effect
has hidden their pixels. The CLI derives the parent textbox and native
`TextRun`/`Ruby`/`GlyphCluster` element from the object ID, measures that element
for the requested page/time, and uses the resulting geometry for color,
object-id, and mask crops. This keeps typewriter-hidden glyph clusters
addressable for deterministic before/after animation debugging without
accepting removed or mismatched object IDs.
The interactive window path shares the same layout-backed body/ruby GlyphArea
model: window page construction is covered for a `vertical_lr` line containing
side-track ruby plus a 4-digit text-combine-upright cluster, and the test adapts
that page-local layout source into the same body and ruby GlyphAreas used by the
window renderer.
The window renderer also keeps a page-local animation clock. When the current
page contains rich-text effect descriptors, the event loop continuously requests
redraws and passes elapsed page time into glyph color, transform, ruby, and
custom-effect execution. This keeps live window playback aligned with
`--capture-time` debug captures instead of showing a fixed sampled frame.
Image resources include non-zero `page` metadata. Rich-text child objects also
record their rendered page in `rich_text_ref.page`, and their `capture_refs`
append `?page=N` for non-zero pages while also exposing `capture_refs[].page`
metadata. `--read-uri` and MCP resource reads parse that query and reconstruct
the native page capture when needed, so an Agent can observe a child object
after `[clear]`, copy its capture URI, and read that URI directly without
parsing text ranges. MCP `arcweft.capture` calls also keep every generated
capture resource in the current session by URI, allowing a later capture to be
made without evicting earlier PNG/raw capture bytes from `resources/read`.
The native player exposes `NativeOffscreenCaptureSession` for repeated
framebuffer, layer, and object captures. CLI native capture requests reuse one
session across full-frame, isolated-color, object-id, and mask reads, so a
single debug request no longer recreates the GPU device and glyph renderer for
each derived image.
MCP stdio sessions also keep a native capture session in `AgentMcpState` after
the first native `arcweft.capture` call. Later native captures in the same MCP
session reuse that renderer/device state while cached PNG/raw resources remain
available through `resources/read`.
MCP `observe` and `resources/list` enumerate layer/object capture refs lazily:
they return addressable image descriptors without rendering every advertised
PNG/raw variant up front. The actual native image bytes are generated by
`resources/read` or `arcweft.capture` for the requested URI/scope and are then
cached in the session.
Native object-id and mask captures for text/ruby-backed scopes now render
selected glyphs through the offscreen text framebuffer on a transparent
background, and textbox-parent regions expand through the rich-text display map
before rendering. These captures report `object_id_attachment` or
`mask_attachment` composition, making them distinct from ordinary framebuffer
crops. Native regions without a rich-text element mapping use their observed
bbox as an attachment primitive, so non-text object-id and mask captures still
report `object_id_attachment` or `mask_attachment` instead of falling back to
generic debug geometry. The selected-region render combines debug style alpha
with capture-time effect alpha, preserving transparent unselected spans while
still letting typewriter reveal control selected glyph clusters, text-combine
cells, and ruby annotations.
Native color scopes without a rich-text element mapping now mask the original
framebuffer to the selected object rectangles before cropping and report
`masked_framebuffer_crop`, so object color image resources avoid carrying
unrelated pixels outside the selected scope. Parent layer color captures such as
`--layer dialogue` crops the native framebuffer to the observed
layer bounds and report `framebuffer_crop`.
Native color
captures for rich-text child objects, rich-text-only layers, and textbox-parent
objects now use an isolated selected-region render with original text
styling before cropping, so unrelated glyphs are not carried in from the full
framebuffer. Rich-text child observed bboxes and object crops use a native
layout measurement API that returns glyphon-derived bounds for display-map text
runs and ruby annotations across all rendered rich-text pages. Vertical ruby
annotations reserve their side track in Sans I/O layout before native
measurement, so short `vertical_rl` ruby at the viewport edge remains visible as
a ruby child object and keeps its annotation bbox inside the captured viewport.
Page-selected
rich-text layer captures filter out child elements that are not visible on the
requested rendered page before computing the crop rectangle, so `[clear]` or
page-wait captures do not carry stale bboxes from earlier pages. Layer/object crops report
`crop_origin` in viewport coordinates and `composition` metadata such as
`framebuffer`, `framebuffer_crop`, `object_id_attachment`, `mask_attachment`,
`masked_framebuffer_crop`, `isolated_regions`, or `debug_geometry`,
keeping image-local pixels traceable back to observed bboxes, rich-text display
ranges, and the capture path that produced them. `content_viewport_bbox` records
the translated non-background pixel bounds directly, which gives Agent/LLM
debuggers a stable way to compare a capture against observed rich-text child
bboxes without decoding the image first.
Object-scoped image resources also carry `image.object` metadata, including the
captured object's `rich_text_ref`; CLI `--read-uri` and MCP capture tool-result
metadata therefore preserve proxy params, source ranges, hit regions, and
object-layer/depth alongside the returned PNG/raw bytes. The same metadata also
copies semantic `object_layer` / `object_depth` onto `image.object` itself while
leaving `image.object.layer` as the render layer, so a saved object crop remains
self-describing as a text presentation object. It also preserves the captured
object's viewport `bbox`/`polygon`. `image.object.capture_refs` preserves the
captured object's sibling color/object-id/mask resource URIs, so debuggers can
switch capture kind from a returned image artifact without walking the
observation object list again.

Inspect the same rich-text display frame through the Agent Debug Bus CLI slice:

```bash
cargo run -p arcweft-cli -- agent observe path/to/file.arcw --json --image overlay
cargo run -p arcweft-cli -- agent observe path/to/file.arcw --image png --out native.png --json
cargo run -p arcweft-cli -- agent observe path/to/file.arcw --image png --capture-step 12 --capture-time 0.35 --out native-step-12.png --json
cargo run -p arcweft-cli -- agent observe path/to/file.arcw --image png --page 1 --object object.dialogue.0.0.run.1 --out page-1-run.png --json
cargo run -p arcweft-cli -- agent observe path/to/file.arcw --image png --layer dialogue --out dialogue.png --json
cargo run -p arcweft-cli -- agent observe path/to/file.arcw --image png --capture object-id --layer dialogue --out object-id.png --json
cargo run -p arcweft-cli -- agent observe path/to/file.arcw --image raw-rgba --capture mask --object object.dialogue.0.0 --out object-mask.rgba --json
cargo run -p arcweft-cli -- agent observe path/to/file.arcw --image png --layer dialogue --resource image
cargo run -p arcweft-cli -- agent observe path/to/file.arcw --image png --layer dialogue --resource image --mcp
cargo run -p arcweft-cli -- agent observe path/to/file.arcw --image png --capture object-id --layer dialogue --resource all --mcp --mcp-format list
cargo run -p arcweft-cli -- agent observe path/to/file.arcw --image png --layer dialogue --resource image --mcp --mcp-format tool-result
cargo run -p arcweft-cli -- agent observe path/to/file.arcw --read-uri arcweft://session/cli/frame/0/object.object.dialogue.0.0.mask.rgba
cargo run -p arcweft-cli -- agent observe path/to/file.arcw --read-uri arcweft://session/cli/frame/0/object.object.dialogue.0.0.png --mcp --mcp-format tool-result
cargo run -p arcweft-cli -- agent observe path/to/file.arcw --image raw-rgba --object object.dialogue.0.0 --out object.rgba --json
```

Open the first resolved frame in a native text window:

```bash
cargo run -p arcweft-player-native -- target/game.awfb
```

## Verified

The current MVP was verified with:

```bash
cargo check -p arcweft-player-native -p arcweft-runtime-plan -p arcweft-render-text
cargo test -p arcweft-agent-protocol -p arcweft-render-text -p arcweft-runtime-plan -p arcweft-player-native
cargo test -p arcweft-render-text reset_control_clears_active_inline_styles_for_following_runs --lib
cargo test -p arcweft-render-native native_layout_reports_text_run_and_ruby_element_bounds --lib
cargo test -p arcweft-render-native native_debug_capture_uses_layout_bounds_for_text_elements --lib
cargo test -p arcweft-render-native native_color_region_capture_preserves_selected_text_style --lib
cargo test -p arcweft-cli agent_observe_json_reports_rich_text_display_objects
cargo test -p arcweft-cli agent_observe_json_reports_rich_text_reset_controls_and_host_markers --test check -- --exact
cargo test -p arcweft-cli agent_observe_writes_layer_png_and_object_raw_images -- --exact
cargo test -p arcweft-cli agent_mcp_stdio_observes_and_reads_rich_text_child_image --test check -- --exact
cargo test -p arcweft-cli --features native-capture --test check agent_observe_native::visual_smoke_viewport_layer_and_object_captures_expose_selected_metadata -- --exact
cargo test -p arcweft-cli agent_observe_native_renderer_writes_rich_text_layer_png_crop --test check -- --exact
cargo test -p arcweft-cli agent_observe_native_renderer_captures_clear_after_page_layer --test check -- --exact
cargo test -p arcweft-cli agent_observe_native_renderer_captures_clear_after_page_object --test check -- --exact
cargo test -p arcweft-cli agent_mcp_stdio_captures_source_with_native_renderer --test check -- --exact
cargo test -p arcweft-cli agent_mcp_stdio_captures_clear_after_page_object_with_native_renderer --test check -- --exact
cargo test -p arcweft-cli agent_mcp_stdio_reads_page_query_capture_ref_with_native_renderer --test check -- --exact
cargo test -p arcweft-cli agent_mcp_stdio_captures_source_layer_with_native_renderer --test check -- --exact
cargo test -p arcweft-cli agent_mcp_stdio_captures_source_ruby_element_with_native_renderer --test check -- --exact
cargo test -p arcweft-cli agent_observe_native_renderer_writes_ruby_mask_raw_crop --test check -- --exact
cargo test -p arcweft-cli agent_mcp_stdio_captures_source_ruby_object_id_with_native_renderer --test check -- --exact
cargo test -p arcweft-cli agent_mcp_stdio_observes_and_reads_rich_text_child_image -- --exact
cargo clippy -p arcweft-player-native --all-targets --all-features
cargo run -p arcweft-player-native -- --headless --json target/game.awfb
cargo run -p arcweft-player-native --features dev-source -- --headless --json --source path/to/file.arcw
cargo run -p arcweft-player-native --features dev-capture -- --headless --json --capture png --capture-out native.png --capture-width 480 --capture-height 270 target/game.awfb
```

No absolute host paths are required in tracked docs or generated player reports.

## Remaining Work

- Add timed wait scheduling and automatic advance policy.
- Add renderer-specific automated screenshot checks once a stable local UI harness is available.
- Decide how host events such as voice, face, and signal are dispatched by real adapters.
- Add golden-image screenshot checks for native/offscreen rich-text rendering once a stable local UI harness is available.


