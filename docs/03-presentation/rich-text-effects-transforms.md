# Rich Text Effects and Transforms

This document defines the presentation-side execution contract for rich-text
transforms, effects, and shader references. It complements the dialogue
authoring syntax in
[Dialogue Control Tags, Ruby, Interpolation, and Line Marks](../01-language/dialogue-control-tags-and-ruby.md)
and the object/proxy model in
[Text Presentation Objects](text-presentation-objects.md).

The renderer-agnostic data model is `RichTextPresentation`. Renderer adapters
resolve effect ids, shader ids, stateful classes, and host callbacks through
registries.

---

## Authoring and canonical form

Dot shorthand may infer a rich-text family when the selector is unambiguous:

```arcw
alice: [.shake amp=2px dir=0,1]揺れる文字[/][p]
alice: [.offset x=4px y=-2px]少しずらす[/][p]
alice: [.vertical_rl jlreq=strict]縦書き[/][p]
```

Canonical tooling expands those spans to explicit families:

```arcw
alice: [effect .shake amp=2px dir=0,1]揺れる文字[/effect][p]
alice: [transform .offset x=4px y=-2px]少しずらす[/transform][p]
alice: [layout .vertical_rl jlreq=strict]縦書き[/layout][p]
```

`[/]` closes the most recent inferred span. Zero-width markers canonicalize to
`[mark .name]` and do not retain a closing tag.

---

## Effective presentation

Each rich-text run and ruby annotation receives an effective
`RichTextPresentation` after defaults, textbox theme, character style, line
options, and inline spans are merged.

Scalar fields such as `italic`, `oblique`, `opacity`, and `z_index` use nearest
explicit value wins. Structured layout fields deep-merge by field. Transform is
one effective transform per run; the nearest transform span replaces earlier
transform fields for that run. Effects and shader refs append in source order.

Ruby base text and ruby annotation text may have different effective
presentations. Ruby layout fields first come from the run/default cascade, then
from ruby-specific overrides such as `ruby_size`, `ruby_gap`,
`ruby_overhang`, and `ruby_collision_gap`.

---

## Transform semantics

`RichTextTransform` contains:

| Field | Meaning |
|---|---|
| `translate` | post-layout offset in logical pixels |
| `rotate` | post-layout rotation angle |
| `scale` | post-layout scale |
| `skew` | post-layout skew |
| `origin` | pivot selection: baseline start, baseline center, center, or glyph center |
| `target` | document, line, sentence, run, glyph, textbox, or screen |

Transforms are visual operations unless their effect phase is explicitly
`before_layout` or `layout_transform`. Visual transforms must not alter source
ranges, dialogue control flow, or typewriter page boundaries.

Renderer conformance is explicit:

- all adapters must preserve and expose every transform descriptor
- adapters that claim transform visual conformance must render translate,
  rotate, scale, skew, origin, and target semantics for the advertised target
  set
- adapters with partial support must still preserve unsupported fields and
  report that limitation through observation diagnostics

Unsupported transform fields must be visible in Agent observation metadata and
should produce diagnostics when a capture claims exact visual conformance.
The native rich-text placement renderer claims visual conformance for
post-layout translate, rotate, scale, skew, origin, and target semantics on body
glyphs and ruby annotation glyphs.

---

## Effect descriptor

`RichTextEffectDescriptor` contains:

| Field | Meaning |
|---|---|
| `id` | selector id without the leading dot, such as `shake` |
| `params` | typed and raw authoring parameters |
| `target` | effect target |
| `phase` | execution phase |
| `state_scope` | state lifetime for deterministic animation classes |

Targets are `document`, `line`, `sentence`, `run`, `glyph`, `textbox`, and
`screen`. The default target is `run`.

Phases run in this order:

1. `before_layout`
2. `layout_transform`
3. normal shaping, line breaking, ruby planning, and glyph placement
4. `glyph_transform`
5. `glyph_color`
6. `glyph_mask`
7. `run_offscreen_pass`
8. `post_process`
9. `host_event`

Default phases are:

| Effect | Default phase |
|---|---|
| `.typewriter` | `glyph_mask` |
| `.shader` | `run_offscreen_pass` |
| other effects | `glyph_transform` |

Post-layout effects keep logical hit regions stable. If an effect needs layout
to change, it must use `before_layout` or `layout_transform` and accept that
line breaking, ruby placement, and capture hashes may change.

---

## Built-in effects

`.wave` displaces glyphs or runs along a deterministic periodic axis. Common
parameters are `amp`, `dir`, `period`, `speed`, and `phase`. `dir=0,1` is
interpreted by the wave builtin as a vector; the parser does not globally
convert comma-separated values into vectors.

`.shake` and `.jitter` apply deterministic pseudo-random offsets. Common
parameters are `amp`, `dir`, `seed`, `speed`, and `bucket`. The same source,
frame, capture time, and seed must produce the same capture.

`.spin` applies deterministic time-varying rotation. Common parameters are
`angle`, `speed`, `phase`, `origin`, and `target`.

`.pulse` applies deterministic time-varying scale. Common parameters are
`amp`, `amount`, `speed`, `phase`, `origin`, and `target`.

`.motion` applies a renderer-resolved deterministic animation function to
translation, rotation, and scale together. Common parameters are `fn` or
`curve`, `amp`, `angle`, `scale`, `speed`, `phase`, `seed`, `origin`, and
`target`. The function name is authored in Arcweft source and preserved in the
effect descriptor; renderers may only execute functions they explicitly expose
through their animation-function registry. Unknown names must remain
observable through renderer diagnostics and must not be silently reinterpreted
as another fallback animation or nondeterministic host code.

`.typewriter` controls glyph visibility. It changes alpha or mask coverage at
`glyph_mask` phase and must not change layout geometry. Common parameters are
`cps`, `delay`, `cursor`, and `capture_time` supplied by the observe/capture
request.

`.shader` references a renderer shader registry entry. It does not embed shader
source in dialogue text. Common parameters are `id`, `amount`, `dir`, `phase`,
and registry-specific raw tokens.

`.host` and unknown custom effect ids are registry-dispatched. `.host` is the
explicit host-dispatched form; its `id`, `effect`, or `name` parameter names the
renderer registry entry and is descriptor metadata rather than a custom
parameter. For example, `[.host id=sparkle]...[/]` lowers to an effect descriptor
with id `sparkle`. If no registry entry exists, the descriptor is preserved and
observation reports it; rendering may no-op with a diagnostic, but must not
silently reinterpret it as a different builtin.

---

## Parameter model

Parser and lowering produce `RichTextParam` values. The global parser recognizes
only syntax that is unambiguous across all custom effects:

- booleans
- integers
- milli numeric values, including unit-like authoring tokens such as `px`,
  `deg`, and `ch`
- selectors such as `.shake`
- raw tokens for everything else

Custom renderer builtins own higher-level interpretation by parameter name.
For example, wave may interpret `dir=0,1` as `Vec2`, while another effect may
preserve the same token as raw text. This avoids hard-coding custom parameter
grammars into dialogue parsing.

Expression-looking values are not inferred as expressions globally. Explicit
expression parameters must use the documented expression form when the language
surface adds one.

---

## Shaders

`RichTextShaderRef` is an effect-like reference with an id, params, and phase.
The default phase is `run_offscreen_pass`.

Shader execution receives the rendered run/layer image as input according to
the renderer registry. Shader refs should be reported in Agent observation so
visual debuggers can distinguish shader output from text layout output.

---

## Observation and capture

Agent observation must expose the effective presentation needed to debug text:

- layout mode, direction, vertical latin mode, JLREQ strictness, and ruby
  typography fields
- full transform descriptor
- effect descriptors with id, params, target, phase, and state scope
- shader refs with id, params, and phase
- text object proxies with id, type, role, depth, hit-test policy, and params
- source ranges and ruby base/annotation bboxes

Image capture follows
[Agent Observe and Capture Contract](../04-tooling/agent-observe-capture-contract.md).
Visual review should compare native full-frame, layer, object, and rich-text
child captures instead of a synthetic compatibility raster.

---

## Conformance expectations

An adapter may advertise a narrower renderer profile during development, but
the data contract does not change by profile. These fields must round-trip
through parsing, lowering, observation, and resource metadata even when a visual
renderer has not implemented every operation:

- transform `skew`, `origin`, and `target`
- effect `target`, `phase`, and `state_scope`
- unknown custom effect parameters as raw tokens
- shader refs and registry-owned parameters
- ruby base and annotation presentations

Unsupported rendering behavior is reported as diagnostics or profile metadata.
It must not introduce alternate syntax, compatibility aliases, or hidden
fallback semantics.
The native rich-text placement renderer currently supports builtin placement
effects for `before_layout`, `layout_transform`, and `glyph_transform`, builtin
typewriter masking for `glyph_mask`, registry-dispatched custom placement
effects, and registry-dispatched custom `glyph_color` effects for observe,
visual-plan, and framebuffer capture paths. The native renderer also supports
registered `run_offscreen_pass` shaders for text and ruby glyph submissions;
the default registry provides `soft_glow` and `warm_glow`, and native adapters
may register additional shader IDs through the shader registry. Unsupported
shader ids, supported shader ids used at the wrong phase, unregistered motion
function ids, unregistered custom effects, post-process effects, and host-event
phases must be diagnosed instead of being silently reinterpreted as placement
effects. For builtin wave placement, `target=glyph` evaluates the phase per
glyph, while `target=run` and broader targets evaluate the placement as one
group; shake and jitter grouping is controlled by `state_scope`.
