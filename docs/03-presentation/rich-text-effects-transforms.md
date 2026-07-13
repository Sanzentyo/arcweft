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

Reusable presentation is authored as a typed `#[fx] fn ... -> Fx`. The same Fx
value applies to a View with `.fx(value)` or to a rich-text span with
`[fx call(...)]...[/fx]`:

```arcw
#[fx]
pub fn notice(
    accent: Color = rgb("#ff4050"),
    amplitude: Length = 2px,
) -> Fx {
    Fx.stack([
        Fx.text(weight = .strong, color = accent),
        wave(amplitude = amplitude),
    ])
}

Text("WARNING").fx(notice(accent = state.warning_color))
alice: [fx notice(accent=rgb("#ff6b8a"))]WARNING[/fx][p]
```

This is the only reusable presentation declaration. There is no separate
`decoration`, presentation-effect implementation, or View-modifier language.
Static style and time-varying effects are ordered nodes in the same graph.

### Fx graph boundary

`Fx` is an immutable presentation-treatment graph. Its typed nodes include
style/text style, transform, color, mask, filter, shader, transition,
conditional selection, and ordered stack composition. These nodes reference
the existing typed style, transform, filter, and shader contracts rather than
reimplementing their semantics.

`#[fx]` is an argument-free marker on an ordinary function. It registers the
function as an Fx entry and implies pure, deterministic graph construction;
authors do not also write `#[pure]`. The body may call ordinary pure helpers
and other Fx functions, but may not mutate state, send signals, perform I/O or
capability calls, await tasks, use nondeterministic random or wall-clock time,
construct View children, or emit actions/events.

Parameters are typed and Fx entry calls are named-only. Defaults live in the
function signature, are const-evaluable, and cannot refer to other parameters
or runtime state. Rest parameters are not supported. View applications may
bind reactive values while preserving a compile-time graph shape; rich-text
tag applications accept only closed values so localization, replay, and line
caching remain deterministic.

An Fx definition is identified by a typed `FxId` derived from its package and
original qualified function name. Re-exports are aliases for resolution, not
new identities. Bundles additionally retain an ABI hash for parameter/default
and renderer-interface compatibility and a semantic hash for the body and
resource bindings. Each View application or rich-text span derives a separate
deterministic `FxInstanceId` from its retained location, authored ordinal, and
optional local key; state and default seeds therefore do not collide between
uses of the same definition. For a glyph-targeted application, `ctx.ordinal`
is the logical glyph index relative to the first glyph reached by that
application. It starts at zero independently for each application and is never
a UTF-8 byte offset or the containing document's global glyph index.

Extern Rust or WASM entries use the same declaration model:

```arcw
extern rust mod studio_fx from crate "studio_fx" {
    #[fx]
    pub fn bloom(
        amount: f32 = 1.0,
        radius: Length = 8px,
    ) -> Fx
}
```

Presentation extensions use the ordinary typed `#[fx] fn ... -> Fx` boundary.

---

## Effective presentation

Each rich-text run and ruby annotation receives an effective
`RichTextPresentation` after defaults, authored View style, character style, line
options, and inline spans are merged.

Scalar fields such as `italic`, `oblique`, `opacity`, `layer`, and `z_index` use nearest
explicit value wins. Structured layout fields deep-merge by field. Transform is
one effective transform per run; the nearest transform span replaces earlier
transform fields for that run. Effects and shader refs append in source order.
`opacity` / `alpha`, `layer` / `object_layer`, `meta` / `metadata` / `data`, and
`z_index` / `z` are authored as style-family presentation scalars, for example
`[style .opacity 0.8]...[/style]`, `[style .layer hud]...[/style]`,
`[style .meta role=caption hover=true]...[/style]`, `[style .z_index 3]...[/style]`,
or their inferred forms `[.opacity 0.8]...[/]`, `[.layer hud]...[/]`,
`[.meta role=caption]...[/]`, and `[.z_index 3]...[/]`. Native/Agent observation
exposes presentation `layer` as `rich_text_ref.object_layer`, metadata as
`rich_text_ref.presentation.params`, and `z_index` as
`rich_text_ref.object_depth = z_index * 1000` for ordinary run/glyph/line/page
text objects, while object proxy `layer` / `depth` / `params` continues to
override the local proxy object when present.

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
| `target` | node, content, background, line, glyph, or viewport |

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

## Typed Fx application

RichText retains a typed `FxApplication`: definition ID, stable instance ID,
validated parameter slots, target, phase, and source range. The referenced
`FxDefinition` owns the complete typed graph, sampler programs, renderer
interfaces, ABI hash, and semantic hash. Runtime does not interpret a second
renderer-local effect descriptor or retain raw parameter tokens.

Targets are `node`, `content`, `background`, `line`, `glyph`, and `viewport`.
The default target is `content`. Per-instance state and deterministic sampling
belong to `FxInstanceId`; no authored state-scope field selects an unrelated
renderer cache.

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

`.sparkle` applies deterministic shimmer as a glyph transform, glyph color, or
post-process. It is an Arcweft-owned builtin even when authored without
attributes; `amp`, `amount`, `speed`, `seed`, `phase`, and `target` customize
its typed program.

`.motion` applies a renderer-resolved deterministic animation function to
translation, rotation, and scale together. Common parameters are `fn` or
`curve`, `amp`, `angle`, `scale`, `speed`, `phase`, `seed`, `origin`, and
`target`. The function name is authored in Arcweft source and preserved in the
effect descriptor; renderers may only execute functions they explicitly expose
through their animation-function registry. Unknown names must remain
observable through renderer diagnostics and must not be silently reinterpreted
as another fallback animation or nondeterministic host code.

Reusable motion is an Fx function whose body returns `Fx.transform` with a
typed sampling closure. `ctx.time`, target-local ordinal/phase helpers, and the
function's named parameters are explicit inputs to deterministic sampling:

```arcw
#[fx]
pub fn wave(amplitude: Length = 2px, speed: f32 = 1.0) -> Fx {
    Fx.transform(
        target = .glyph,
        sample = |ctx| Transform2D {
            translate_y:
                sin(ctx.time * speed + ctx.ordinal_phase()) * amplitude,
        },
    )
}
```

Renderer adapters validate the Fx renderer interface and ABI hash instead of
registering a separate source-language motion-function attribute.

`.typewriter` controls glyph visibility. It changes alpha or mask coverage at
`glyph_mask` phase and must not change layout geometry. Common parameters are
`cps`, `delay`, `cursor`, and `capture_time` supplied by the observe/capture
request. Time literals such as `0.5s` or `500ms` are converted once during
typed compilation; renderer adapters do not parse raw duration tokens or keep
a native-only effect registry. `cursor=true` asks the typed built-in graph to
expose the next unrevealed glyph as a low-alpha ghost preview without changing
layout, with `cursor_alpha` / `cursor_opacity` as an optional typed override.

`.shader` references a resource in the bundle's typed renderer-resource table;
it does not embed shader source in dialogue text. Common parameters are `id`,
`amount`, `dir`, `phase`, and `color`. Shared evaluation resolves glyph,
offscreen, and post-process operations before a backend adapter receives them.
Unknown resources and invalid uniform schemas are typed diagnostics rather
than backend-local no-ops.

Unknown custom effect shorthand retains its exact missing definition identity
and produces a diagnostic; it is not reinterpreted as a built-in basename.
`.host` is not a visual-registry escape hatch. Host dispatch is represented by
the explicit typed `phase=host_event` path described below.

If an effect selector uses `phase=host_event`, it is not a visual presentation
style. Lowering emits a typed `DialogueHostEvent::Effect` marker with the
resolved effect id and raw attrs, while the span text remains ordinary display
text. This lets authored rich-text cues address host systems without forcing a
native renderer to reinterpret `host_event` as a glyph effect.

---

## Parameter model

Parser and lowering produce `RichTextParam` values. The global parser recognizes
only syntax that is unambiguous across all custom effects:

- booleans
- integers
- milli numeric values, including unit-like authoring tokens such as `px`,
  `deg`, and `ch`
- selectors such as `.shake`
- quoted strings as `Text`, preserving the distinction between `"2"` /
  `"true"` and their unquoted integer / boolean forms
- raw tokens for other unquoted values

Legacy low-level rich-text descriptors and renderer builtins own higher-level
interpretation by parameter name.
For example, wave may interpret `dir=0,1` as `Vec2`, while another effect may
preserve the same token as raw text. This avoids hard-coding custom parameter
grammars into dialogue parsing. Reusable author-facing Fx parameters do not use
this open raw-token model: they are a closed typed function schema.

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
- text object proxies with id, type, role, layer, depth, hit-test policy, and
  params
- source ranges and ruby base/annotation bboxes

Image capture follows
[Agent Observe and Capture Contract](../04-tooling/agent-observe-capture-contract.md).
Visual review should compare native full-frame, layer, object, and rich-text
child captures instead of a synthetic compatibility raster.

---

## Conformance expectations

An adapter may advertise a narrower renderer profile during development, but
the data contract does not change by profile. Typed transforms, targets,
phases, shader resource IDs, sampler programs, and ruby presentations must
round-trip through parsing, lowering, bundle codecs, evaluation, observation,
and capture.

Unknown parameters fail closed-schema compilation. Missing resources, ABI
mismatch, unsupported target/interface pairs, non-finite arithmetic, and
budget exhaustion produce typed diagnostics carrying the definition and
instance identities. They do not introduce alternate syntax, hidden fallback
semantics, or silent no-op rendering. Native, Web, WASM, and headless hosts use
the same reference evaluator and submit only its resolved plan.
