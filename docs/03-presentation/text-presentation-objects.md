# Text Presentation Objects

Arcweft text is a typed presentation object tree, not only a string submitted to
a renderer. `character.say` and `narrator.say` are high-level facades that create
the same kind of presentation objects as image, sprite, model, and UI layers.
The default dialogue facade chooses sensible text-box, line, run, glyph-cluster,
ruby, object-id, hit-test, depth, and capture behavior, while still preserving
the authored rich-text surface.

## Object Levels

Text adapters may expose these object levels:

```text
textbox
  page
    line
      run
        proxy_object
        glyph_cluster
        ruby_object
          ruby_base
          ruby_annotation
```

Every observed object must have a stable object id, a viewport bbox, optional
polygon, object-id capture color, source range, effective presentation metadata,
and hit-test regions when hit testing is enabled. Renderers may also expose
layer-specific children such as shader passes, IME caret objects, or selection
handles, but those children must reference the text object they decorate.

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
`default_hit` supplies hit-test policy, `depth` / `z` / `z_index` supplies local
depth, and remaining attribute arguments become default typed proxy params.

Proxy spans may be nested or otherwise overlap. The effective text run keeps all
active proxies in source order instead of collapsing them into one object. Agent
observation emits one `rich_text_proxy` child object per effective proxy, each
with the same measured text range/bbox but its own proxy id, type, role, depth,
hit-test flag, params, capture refs, and object-id color. This lets authors
attach separate semantic objects such as a choice hit target and a hover/depth
proxy to the same visible text without losing either object.

## Depth And Hit Testing

`z`, `z_index`, or `depth` on an object proxy is object metadata. It does not
replace renderer layer order, but it gives hit testing and debug capture a
stable local ordering key. When both `RichTextPresentation.z_index` and proxy
depth are present, the renderer sorts by layer, then presentation `z_index`,
then proxy depth, then source order.

`hit=true` enables hit-test regions for the span. `hit=false` keeps the proxy
observable but non-interactive. Hit regions are reported in Agent observation and
must use the same post-transform bounds as object-id and color captures.
Agent `rich_text_ref.hit_regions` reports these interactive spans with kind
`text_object_proxy`, the proxy id/type/role, and the resolved local depth. The
same resolved maximum proxy depth is exposed as `rich_text_ref.object_depth` so
debuggers can sort text objects with image/model-like presentation objects.

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

## Renderer Boundary

Renderer adapters may interpret registered proxy ids, effects, shaders, and
animation functions, but unknown proxy metadata must stay deterministic and
observable. Unknown proxies should not be silently reinterpreted as effects.
Unsupported proxy behavior is reported through renderer diagnostics while the
metadata remains visible in Agent JSON.

## Debug Contract

Agent observe/capture must be able to retrieve:

- the whole text layer as color, raw RGBA, mask, and object-id images
- individual textbox, run, glyph-cluster, ruby, and proxy-decorated object crops
- effective presentation metadata, including proxy ids and attributes
- hit regions and depth ordering metadata
- a pinned runtime step plus visual `capture_time` for animated text objects

This makes text debug behave like image, sprite, and 3D model debug: the Agent
can ask what object exists, where it is, what authored proxy metadata it carries,
and what exact pixels it produced at a deterministic step/time.
