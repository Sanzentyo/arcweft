# Text Presentation Objects

Arcweft text is a typed presentation object tree, not only a string submitted to
a renderer. `character.say` and `narrator.say` are high-level facades that create
the same kind of presentation objects as image, sprite, model, and View layers.
The default dialogue facade chooses sensible text-box, line, run, glyph-cluster,
ruby, glyph, object-id, hit-test, depth, and capture behavior, while still
preserving the authored rich-text surface.

## Object Levels

Text adapters may expose these object levels:

```text
textbox
  page
    line
      run
        proxy_object
        glyph
        glyph_cluster
        ruby_object
          ruby_base
          ruby_annotation
```

Every observed object must have a stable object id, a `parent_id` when it is a
child of another presentation object, a viewport bbox, optional polygon,
object-id capture color, source range, effective presentation metadata, and
hit-test regions when hit testing is enabled. Renderers may also expose
layer-specific children such as shader passes, IME caret objects, or selection
handles, but those children must reference the text object they decorate.
Agent observation exposes the flat object table and a typed `presentation_tree`;
the tree is the canonical way to traverse layer, textbox, page, line, run,
ruby, glyph/cluster, and proxy relationships without re-parsing object ids.
Tree object nodes also expose lightweight effect, shader, transform,
motion-function, and proxy indexes. Proxy indexes include the authored proxy id,
resolved type name, role, layer, depth, declaration provenance, and typed proxy
params, so tools can find animated, custom-rendered, or `#[text_proxy]`-backed
text objects before loading the full object descriptor.

## Proxy Spans

Authors can attach typed proxy metadata to a text span with the explicit
`object` family:

```arcw
alice.say[
    [object .hotspot type=KeywordHit role=keyword depth=4 hit=true]夢[/object]を見た。[p]
]
```

The selector after `object` is the proxy id. `type`, `struct`, or `proxy`
selects the authored proxy type. Other attributes are preserved as typed
rich-text parameters. The proxy is not a visual effect by itself. It is metadata
that renderers, hit-test systems, depth sorting, Agent observation, and custom
effect/shader registries may consume.

Unknown dot selectors with attributes still infer to custom effects. This keeps
`[.sparkle amp=2px]...[/]` unambiguous. Text object proxy spans use
`[object .name ...]...[/object]` as their canonical form. Tooling may infer that
family from `[.id type=ProxyType]...[/]`, `[.id struct=ProxyType]...[/]`,
`[.id proxy=ProxyType]...[/]`, or `[.ProxyType]...[/]` only when `ProxyType` is
a visible `#[text_proxy]` / `#[rich_text_proxy]` struct; the canonical output
still writes the explicit `object` family.

## Attribute-Defined Proxy Types

Arcweft attributes are the declaration-time way to mark Rust/Arcweft structs as
text proxy payloads:

```arcw
#[text_proxy(kind="keyword", default_hit=true, depth=4, channel=choice)]
pub struct KeywordHit {
    channel: String
}
```

The declaration metadata is collected into the rich-text proxy registry during
runtime-plan lowering. Inline text does not overload `#[...]`, because dialogue
text already uses `#[expr]` for interpolation and source items use `#[...]` for
attributes. Inline spans refer to the proxy type by name through
`type=KeywordHit` or `proxy=KeywordHit`:

```arcw
alice.say[
    [object .hotspot type=KeywordHit]夢[/object]
]
```

Inline attributes override declaration defaults. Unspecified proxy metadata is
filled from the struct attribute: `kind` supplies the default proxy role,
`layer` / `object_layer` supplies object-layer metadata, `default_hit` supplies
hit-test policy, `depth` / `z` / `z_index` supplies local depth, and remaining
attribute arguments become default typed proxy params.
The resolved proxy also keeps typed declaration provenance: the source struct
name and the attribute family (`text_proxy` or `rich_text_proxy`) that supplied
the defaults. This provenance is separate from `type_name`, because an
attribute may choose a registry-facing proxy type name while the Arcweft struct
name remains the source declaration used by tooling.

Proxy spans may be nested or otherwise overlap. The effective text run keeps all
active proxies in source order instead of collapsing them into one object. Agent
observation emits one `rich_text_proxy` child object per effective proxy, each
with the same measured text range/bbox but its own proxy id, type, role, layer,
depth, hit-test flag, params, capture refs, and object-id color. This lets authors
attach separate semantic objects such as a choice hit target and a hover/depth
proxy to the same visible text without losing either object.
Page and line text objects also aggregate proxy metadata for the text range they
cover. Their own primary hit region remains `text_page` or `text_line`, while
additional `text_object_proxy` hit regions use the decorated proxy's measured
post-transform native bounds and local proxy depth/layer. This lets a debugger
inspect a broad page or line object and still recover the concrete clickable or
depth-aware text proxies inside it.

## Depth And Hit Testing

`layer` / `object_layer` authored in the `style` family is presentation metadata
for the ordinary text object itself. It is exposed as
`rich_text_ref.object_layer` on runs, glyphs, clusters, lines, pages, and ruby
objects without creating a proxy. `z` / `z_index` works the same way for ordinary
object depth, exposed as `rich_text_ref.object_depth = z_index * 1000`.
`meta` / `metadata` / `data` authored in the `style` family attaches typed
debug/input metadata to the ordinary text object without making it a proxy:
`[style .meta role=caption hover=true]...[/style]` and
`[.meta role=caption]...[/]` appear under `rich_text_ref.presentation.params`.

`layer` / `object_layer`, `z`, `z_index`, or `depth` authored on an `object`
proxy is proxy object metadata. Proxy layer does not replace the parent
render-layer group, but it is exposed as first-class object metadata so hit
testing, object-id capture, and headless debuggers can distinguish semantic
layers such as View, dialogue, hotspot, or depth proxy. When both
`RichTextPresentation.z_index` and proxy depth are present, renderers sort by
the resolved render layer, then presentation `z_index`, then proxy depth, then
source order. Proxy params remain local to the proxy object and are reported in
hit-test results as `proxy_params`; ordinary presentation params stay on
`rich_text_ref.presentation.params`. A proxy with no explicit layer inherits the
parent presentation layer for Agent hit-test reporting; an explicit proxy layer
overrides it for that proxy object.

`hit=true` enables hit-test regions for the span. `hit=false` keeps the proxy
observable but non-interactive. Hit regions are reported in Agent observation and
must use the same post-transform bounds as object-id and color captures.
Agent `rich_text_ref.hit_regions` reports these interactive spans with kind
`text_object_proxy`, the proxy id/type/role/layer, the resolved local depth, and
the typed declaration provenance and proxy params that came from inline
attributes or `#[text_proxy]` / `#[rich_text_proxy]` defaults. Hit-test reports
return the same `proxy_declaration` and `proxy_params`, so input handlers and
Agent tools do not need to recover custom metadata by separately walking the
parent presentation object.
The selected proxy layer is exposed as `rich_text_ref.object_layer`, and the
same resolved maximum proxy depth is exposed as `rich_text_ref.object_depth` so
debuggers can sort text objects with image/model-like presentation objects.
`arcw agent hit-test` and MCP `arcweft.hit_test` consume those same observed
regions. They rank hits by resolved depth descending, then by semantic
specificity (`text_object_proxy` before glyph/cluster/run/line/page regions),
then by the smaller hit bbox and stable object id. A custom proxy therefore
behaves like an image/model hit target: it can overlap the same glyph pixels as
another proxy, carry a higher local depth, and still be returned as the top hit
without losing the lower-ranked proxy.

Agent observation also emits each authored proxy span as its own
`rich_text_proxy` observed object. Its object id is rooted at the parent textbox
and includes the native run index plus proxy index:

```text
object.dialogue.<step>.<textbox>.proxy.<run>.<proxy>
```

The proxy object uses the same measured post-transform bbox and native capture
target as the decorated run, but its `rich_text_ref.kind` is
`text_object_proxy` and its presentation contains only the selected proxy. This
lets LLM/debug tools capture or inspect the proxy span directly without having
to infer it from a broader `rich_text_run`.
Image resources produced for the proxy object keep an `image.object` reference
with the same selected `rich_text_ref` and `parent_id`. A raw/PNG mask,
object-id, or color crop therefore carries the proxy id/type/role/layer/depth,
typed params, hit-test regions, source range, and containing text-object
identity alongside the pixels. This is the text-object equivalent of a sprite or
3D model crop retaining its object identity and depth/hit metadata.

## Renderer Boundary

Renderer adapters may interpret registered proxy ids, effects, shaders, and
animation functions, but unknown proxy metadata must stay deterministic and
observable. Unknown proxies should not be silently reinterpreted as effects.
Unsupported proxy behavior is reported through renderer diagnostics while the
metadata remains visible in Agent JSON.

## Debug Contract

Agent observe/capture must be able to retrieve:

- the whole text layer as color, raw RGBA, mask, and object-id images
- individual textbox, page, line, run, glyph, glyph-cluster, ruby, and
  proxy-decorated object crops
- effective presentation metadata, including proxy ids and attributes
- hit regions and depth ordering metadata
- a pinned runtime step plus visual `capture_time` for animated text objects
- object-scoped image resource metadata that preserves the captured text
  object's `rich_text_ref`

This makes text debug behave like image, sprite, and 3D model debug: the Agent
can ask what object exists, where it is, what authored proxy metadata it carries,
and what exact pixels it produced at a deterministic step/time.
