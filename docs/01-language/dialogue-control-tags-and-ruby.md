# Dialogue Control Tags, Ruby, Interpolation, and Line Marks

Arcweft supports KAG-like bracket tags inside dialogue text, but the feature is deliberately scoped. `[...]` tags are special only in dialogue text mode: speaker lines, narrator lines, indented dialogue bodies, and `Character.say(...)[ ... ]` content blocks.

Related:

- [Flow-Integrated Scenario Syntax](scenario-surface-syntax.md)
- [Dialogue Character Methods, TextBox Targets, Interpolation, and Preload](dialogue-character-methods-and-textbox.md)
- [Localization for Dialogue](localization-dialogue.md)
- [Dialogue Calls, Line Plans, Cancellation, and Scoped Content Blocks](dialogue-calls-scopes-cancellation.md)
- [Character Stage / Sprite / Voice Timeline](../03-presentation/character-stage.md)
- [Hooks and Memoization](hooks-and-memoization.md)
- [入力パース](parsing.md)

---

## Dialogue-text mode only

The following forms enable dialogue-text mode:

```arcw
alice: おはよう。[p]

alice:
    おはよう。[l]
    今日はいい天気だね。[p]

地の文: 扉の向こうから、雨の音がした。[p]

alice.say(voice=auto)[
    今日は少しだけ、|[変な夢](へんなゆめ)を見たんだ。[p]
]
```

Only in those text regions, and in typed `fn(args)[content]` content blocks whose declared content type is dialogue/rich text, are `[...]`, `[/...]`, and `#[...]` interpreted as dialogue markup. In normal typed code, brackets keep their normal meaning.

Historical flat-fence imports treat a physical text line beginning with `===`
as a fence. In stable source, use canonical dialogue calls such as
`alice.say()[...]`; tooling migrations escape literal text that begins with
three equals signs as `\===`.

---

## Tag families

Arcweft has four tag-like forms in dialogue text:

| Form | Purpose |
|---|---|
| `[p]`, `[l]`, `[r]` | short built-in control tags |
| `[ruby rt="..."]...[/ruby]`, `[rb rt=...]...[/rb]` | enclosing rich-text/control tags |
| `#[expr]`, `#[fmt(...)]`, `$(expr)` | pure content interpolation |
| `[call ...]`, `[! ...]` | dialogue-safe function dispatch |
| `[mark .name]`, `[.name]` | zero-width line-local marker for `with` handlers and waits |
| `[decorate .name ...]...[/decorate]` | a declared, reusable visual-decoration span |

Double brackets are not dialogue tags:

```arcw
/// [[flow.alice_intro]] is a documentation/RAG link, not a dialogue tag.
```

---

## Built-in reserved names

These names are reserved in tag position and scenario-command position. They cannot be used as unqualified custom tag names, unqualified scenario command names, character aliases, or local variables in dialogue tag scope.

A module may still define a qualified function such as `my_tags.p`, but it cannot be imported unqualified as `p`.

| Name | Meaning |
|---|---|
| `p` | user wait that closes the current logical page |
| `l` | user wait that keeps the current logical page open |
| `r` | hard line break |
| `br` | hard line break tag |
| `w` | automatic timed wait reached during reveal |
| `clear` | immediately reset displayed text when reached |
| `er` | alias of `clear` |
| `cm` | alias of `clear` |
| `ruby` | ruby annotation |
| `rt` | ruby text shorthand inside ruby-related tags |
| `em`, `strong` | emphasis spans |
| `color`, `font`, `size` | rich text styling spans |
| `speed` | reveal rate for subsequent text |
| `object` | typed text presentation object/proxy span |
| `reset` | reset text style/reveal modifiers |
| `voice` | voice cue inside a line |
| `face`, `pose` | expression/pose change |
| `show`, `hide` | stage visibility cue |
| `move`, `scale`, `rotate` | transform cue |
| `anim`, `shake` | animation cue |
| `mark` | zero-width line-local marker |
| `at` | timed cue shorthand inside dialogue text |
| `call` | call an allowed dialogue function/tag |
| `signal` | emit/set a public signal if capability allows it |
| `if`, `else`, `endif` | local text conditional |
| `raw` | literal no-parse span |
| `fmt` | explicit DisplayText/content formatting function |
| `decorate` | apply a declared reusable visual decoration |

Project-specific aliases may map to these names, but canonical names remain reserved:

```toml
[dialogue.tags.aliases]
"改ページ" = "p"
"待機" = "l"
"改行" = "r"
```

---

## Inferred rich-text selectors

Inline rich-text presentation selectors may be written with dot shorthand when
the selector is unambiguous:

```arcw
alice: [.shake amp=2px dir=0,1]揺れる文字[/][p]
alice: [.vertical_rl jlreq=strict]縦書き[/][p]
alice: [.offset x=4px y=-2px]少しずらす[/][p]
```

The canonical forms keep the family explicit:

```arcw
alice: [effect .shake amp=2px dir=0,1]揺れる文字[/effect][p]
alice: [layout .vertical_rl jlreq=strict]縦書き[/layout][p]
alice: [transform .offset x=4px y=-2px]少しずらす[/transform][p]
alice: [object .hotspot type=KeywordHit hit=true]当たり判定つき文字[/object][p]
```

`[/]` closes the most recent inferred rich-text span. Canonical tooling expands
it to the explicit family end tag, such as `[/effect]`.

Known selector families are style (`.italic`, `.oblique`), layout
(`.horizontal_tb`, `.vertical_rl`, `.vertical_lr`, `.dir`, ruby-position
selectors), transform (`.offset`, `.pos`, `.rotate`, `.scale`, `.skew`), and
effect (`.wave`, `.shake`, `.arc`, `.spin`, `.pulse`, `.motion`,
`.typewriter`, `.jitter`, `.shader`, `.host`). Unknown dot selectors without attributes are markers and canonicalize
to `[mark .name]`. If an unknown marker-like selector was accidentally written
with a following `[/]`, canonical tooling removes that inferred close because
markers are zero-width, not spans. Unknown dot selectors with attributes
canonicalize to custom effect spans, for example
`[.sparkle amp=2px]...[/]` becomes `[effect .sparkle amp=2px]...[/effect]`.

Text presentation object proxies are explicit object-family spans:
`[object .name ...]...[/object]`. They preserve custom proxy metadata for
hit-testing, depth ordering, object-id capture, and renderer/tooling registries
without reinterpreting the span as a visual effect. The declaration-time proxy
type may be marked with normal Arcweft attributes such as `#[text_proxy(...)]`;
inline dialogue text refers to it with `type=Name`, `struct=Name`, or
`proxy=Name` so it does not conflict with `#[expr]` interpolation. Canonical
tooling may infer the object family from `[.id type=Name]...[/]` or
`[.Name]...[/]` only when `Name` is a visible `#[text_proxy]` /
`#[rich_text_proxy]` struct, and rewrites that surface to explicit
`[object ...]...[/object]` form. Runtime-plan lowering uses the struct attribute
as proxy defaults: `kind` becomes the default role, `default_hit` becomes the
default hit-test policy, `depth` / `z` / `z_index` becomes default local depth,
and any remaining attribute arguments become default typed proxy params unless
the inline object span overrides them.

Effect and shader parameters preserve unknown values as raw authoring tokens.
The parser does not infer comma-separated values or expression-like strings as
structured values globally; renderer builtins interpret only the parameter names
they own, such as `dir=0,1` for a wave direction.
`.host` is the explicit host-dispatched effect selector. Its `id`, `effect`, or
`name` parameter selects the renderer registry id, so `[.host id=sparkle]...[/]`
canonicalizes as an effect span and lowers to the same registry effect id as
`[effect .sparkle]...[/effect]` while keeping the authoring surface explicit.

Layout selectors accept `jlreq=loose|normal|strict` to choose the vertical
Japanese punctuation-pair planning preset for that span. Omitting it keeps the
host textbox/default layout preset.

Ruby-position selectors also accept local typography overrides:

```arcw
alice: [.ruby_over ruby_size=11px ruby_gap=1px ruby_overhang=4px ruby_collision_gap=3px]|[夢](ゆめ)[/][p]
```

These attributes override the effective `rich_text { ruby { ... } }` defaults
only for the enclosed span. The inline names are prefixed with `ruby_` because
dialogue tag attributes are flat. Defaults use the structured form
`rich_text { ruby { size = ... } }`, normally written as a multiline block when
more than one field is set.

---

## Reusable visual decorations

A top-level `decoration` declaration gives a name to an ordered group of
visual rich-text layers. It is the canonical way to reuse combinations such as
strong text, a color, and an effect without copying the same tags into every
line:

```arcw
decoration warning(
    accent = "#ff4050",
    amplitude = 2px,
    seed = "warning",
    ...effect_args,
) {
    strong()
    color(value=accent)
    effect(.wave, amp=amplitude, seed=seed, effect_args...)
}

alice: [decorate .warning]既定値の警告[/decorate][p]
alice: [decorate .warning accent="#ffd060" amplitude=4px speed=2]強い警告[/decorate][p]
```

Decorations are module-local. The canonical declaration therefore has no
visibility modifier; `pub decoration` is rejected instead of implying a
cross-module selector/import contract that does not exist.

Parameters are named and compile-time only. `name` declares a required
parameter, while `name = value` declares a default. Invocation arguments are
named overrides; positional values after the leading decoration selector are
rejected. A final `...effect_args` parameter explicitly captures additional
custom named arguments. The body forwards that bag only where it writes
`effect_args...`. Without a rest parameter, an unknown invocation argument is
a diagnostic rather than an implicit renderer parameter.

Defaults and overrides must close to deterministic rich-text parameter values:
booleans, signed numeric/unit/duration tokens, strings, selectors, or raw
identifier tokens. Character literals such as `"x"c` are not decoration values;
use a string when a textual renderer parameter is intended. Invocation-only
renderer payloads may also use one safe token such as
`#ff4050`, `0,1`, or `source-shader`; quote values containing whitespace or
reserved punctuation. Runtime expressions and dialogue interpolation are not
accepted in a decoration declaration or invocation. Dynamic content remains
ordinary `#[expr]` interpolation outside the style/effect descriptor.
For effect and forwarded custom parameters, quoting is type-significant:
`"2"` and `"true"` remain text, while `2` and `true` infer integer and boolean
parameters.

Inside a builder call, a bare identifier denotes a declared decoration
parameter. Quote registry-owned raw words (for example `origin="center"`) or
use a selector where the builder requires one. This makes a misspelled
parameter a diagnostic instead of silently turning it into an unrelated raw
renderer token.

The declaration body is an outer-to-inner list of span builders. Supported
builders are `em`, `strong`, `color`, `font`, `size`, `style`, `layout`,
`transform`, `effect`, and `decorate` for composing another declared
decoration. A nested decoration graph must be acyclic. Closing `[/decorate]`
removes the expanded layers in reverse order as one authored span.

Builder calls have fixed shapes: `em()` and `strong()` take no arguments;
`color`, `font`, and `size` take exactly one positional value or `value=...`;
and the remaining builders take one literal `.Ident` selector followed only by
named values and at most one rest spread. A selector cannot be supplied through
a parameter. `style` accepts `.italic`, `.oblique`, `.opacity`, `.layer`,
`.meta`, and `.z_index`; `layout` accepts `.horizontal_tb`, `.vertical_rl`,
`.vertical_lr`, `.dir`, `.ruby_over`, `.ruby_under`, and
`.ruby_inter_character`; `transform` accepts `.offset`, `.pos`, `.rotate`,
`.scale`, and `.skew`. These are canonical closed compiler inventories, so
direct-tag aliases and misspellings are diagnostics instead of silently falling
back to an unknown style, horizontal layout, or identity transform. Effect ids
remain registry-extensible, while `decorate(.name, ...)` must select another
declaration in the same module. Named invocation/custom-argument keys use the
ordinary Unicode-aware `Ident` grammar even when a rest parameter is present.

A nested `decorate(.target, rest...)` may forward a rest bag only when
`.target` declares its own final rest parameter. Sema diagnoses the containing
declaration eagerly when the target has only fixed named parameters, regardless
of whether a particular call would supply any custom arguments. The bag denotes
an open set of keys, so its unknown keys cannot bind to undeclared target
parameters.

To keep malformed or generated composition graphs deterministic, one
decoration expansion is limited to 64 nested declarations, 16,384 visited
declaration nodes, and 4,096 concrete rich-text layers. Sema and runtime-plan
lowering use the same limits and report a structured compile error rather than
recursing or allocating without a bound.

Decoration bodies cannot contain page/wait controls, reveal-speed changes,
marks, object proxies, calls, signals, conditionals, host events, or an effect
whose phase is `host_event`. Those operations have runtime behavior or identity
that cannot be safely hidden inside a visual style group. The explicit
`[decorate .name]` family is canonical; bare `[name]` and `[.name]` keep their
existing unknown-control, marker, and custom-effect meanings and are never
silently reinterpreted as decoration calls.

---

## Wait and newline tags

```arcw
alice: おはよう。[l]今日はいい天気だね。[p]
```

Meaning:

```text
[l]  wait for user advance; then continue revealing on the same logical page.
[p]  wait for user advance; close the current logical page before later text.
```

Logical page boundaries are authored behavior, not a TextBox setting. If more
content follows a `[p]`, advancing starts that content on a new logical page.
If `[p]` is the terminal control, it does not manufacture an empty page: the
advance at that stage releases the line to its continuation. `[l]` never closes
the logical page, so text visible before the marker remains visible when reveal
continues.

Line break:

```arcw
alice: 1行目[r]2行目[p]
```

Timed wait:

```arcw
alice: えっと……[w time=500ms]なんでもない。[p]
```

Authoring shorthand:

```arcw
alice: えっと……[w 500ms]なんでもない。[page]
```

`[page]`, `[wait]`, and `[nl]` normalize to `[p]`, `[l]`, and `[r]`.

`[w]` begins only after reveal reaches its marker. It pauses automatically for
the authored duration and then resumes without user input. The duration must be
positive and use `ms` or `s`, for example `250ms`, `1s`, or `0.5s`. Missing,
zero, negative, unsupported-unit, sub-millisecond, and overflowing durations
are compile-time errors.

`[clear]` resets the currently displayed text immediately when reveal reaches
the marker. It neither waits for input nor closes the logical page; use an
adjacent `[l]` or `[p]` when a wait is also required. `[er]` and `[cm]`
normalize to `[clear]`. When `[l]` follows, the next stage retains the
post-clear display rather than reconstructing text removed before the marker.

```arcw
alice: 前の表示。[clear]ここから表示を作り直す。[p]
```

`[speed ...]` changes the reveal rate for subsequent text. The modifier
accepts `slow`, `normal`, `fast`, or a numeric rate from 1 through 240
characters per second with at most three decimal places. Missing, malformed,
out-of-range, and over-precise rates are compile-time errors. The modifier
remains active until a later speed/reset boundary or the end of the line.

```arcw
alice: 通常。[speed slow]ゆっくり。[speed 56]速く。[reset]通常。[p]
```

TextBox themes may animate or style a logical-page transition, but they do not
change whether `[p]` closes a page, `[l]` retains it, or a terminal `[p]`
releases the line.

---

## Ruby

Arcweft supports these ruby forms. The recommended authoring form is
`|[base](ruby)` because it is ASCII-friendly and works when the base contains
spaces or punctuation.

### ASCII explicit ruby

```arcw
alice: 今日は少しだけ、|[変な夢](へんなゆめ)を見たんだ。[p]
```

### ASCII compact ruby

```arcw
alice: 今日は少しだけ、|変な夢{へんなゆめ}を見たんだ。[p]
```

Compact ruby is accepted only when the base is non-empty and contains no
whitespace or reserved markup characters: `[`, `]`, `{`, `}`, `#`, or `|`.
Use `|[base](ruby)` for longer or ambiguous base text.

### Natural Japanese ruby

```arcw
alice: 今日は少しだけ、｜変な夢《へんなゆめ》を見たんだ。[p]
```

### Bracket tag ruby

```arcw
alice: 今日は少しだけ、[ruby rt="へんなゆめ"]変な夢[/ruby]を見たんだ。[p]
```

The shorter `[rb rt=へんなゆめ]変な夢[/rb]` spelling is accepted and normalizes
to the same ruby fragment.

### Function/content form

```arcw
alice.say()[
    今日は少しだけ、#[ruby("変な夢", "へんなゆめ")]を見たんだ。[p]
]
```

All forms normalize into the same `Content.Ruby { base, ruby }` fragment.

Ruby typography is resolved from the active RichText cascade before layout:

```text
inline [.ruby_over ruby_size=... ruby_gap=...]
  -> line / speaker preset rich_text.ruby
  -> character dialogue_style.rich_text.ruby
  -> dialogue window theme rich_text.ruby
  -> selected dialogue defaults rich_text.ruby
  -> engine defaults
```

The ruby content syntax chooses the base and annotation text. Typography such
as position, annotation size, base gap, overhang, and collision separation comes
from the active `rich_text.ruby` style unless an inline ruby selector overrides
it.

Localization import validates ruby fragments:

```text
- natural ruby delimiters are balanced;
- bracket ruby has matching end tag;
- base text is not empty;
- ruby text is not empty;
- locale-specific ruby may be removed, preserved, or replaced depending on locale policy.
```

Example locale policy:

```toml
[locale.ruby]
ja-JP = "preserve"
en-US = "drop_or_emphasize"
zh-CN = "preserve_optional"
```

---

## Pure interpolation with `DisplayText`

`#[expr]` inserts the formatted representation of `expr`. `$(expr)` is accepted
as an authoring shorthand for the same pure interpolation. The expression must
implement `DisplayText`.

```arcw
narrator.say()[
    #[player_name]は鍵を手に入れた。[p]
]
```

If formatting needs options, use `fmt(...)` explicitly:

```arcw
narrator.say()[
    スコアは#[fmt(score, style="number", on_error=InlineFailure.fallback("?"))]点です。[p]
]
```

The display trait is:

```arcw
pub trait DisplayText {
    fn display_text(self, ctx: DisplayContext) -> Result<Content, DisplayError>
}
```

Built-in implementations include common scalar types, `String`, `LocalizedText`, `Ref<T>`, and selected wrappers. `Option<T>` must be explicitly handled or formatted with a fallback:

```arcw
#[fmt(state.nickname, none="名無し", on_error=InlineFailure.fallback("名無し"))]
```

Inline function calls inside `#[...]` must declare how interpolation failures are
handled, unless the line, speaker preset, character state, or dialogue defaults
supplies an inline failure policy. Canonical values use the `InlineFailure` enum
namespace. Contextual shorthand such as `.fail` and `.discard` is valid only
where an `InlineFailure` value is expected. For ordinary display text, prefer a
default policy on the line, preset, or character instead of repeating policy
arguments on every `fmt(...)` call:

```arcw
let alice_text = alice(inline_error=InlineFailure.fallback("?"))
alice_text: #[fmt(score, style="number")]点[p]
alice: #[fmt(score, style="number", on_error=InlineFailure.fallback("?"))]点[p]
alice(inline_error=.discard): #[fmt(debug_label)] [p]
alice: #[fmt(score, on_error=.fail)]点[p]
alice: #[fmt(score, style="number", on_error=InlineFailure.fallback(InlineFallback.expr_source))]点[p]
alice: #[fmt(score, style="number", on_error=InlineFailure.fallback(InlineFallback.call_source))]点[p]
```

`on_error`, `fallback`, and `discard_error` are mutually exclusive. Use exactly
one per inline call. `fallback="..."` is shorthand for
`on_error=InlineFailure.fallback("...")`.

`InlineFallback.expr_source` renders the primary input expression without the
formatting call. For `fmt(score, style="number")`, it renders `score`.
`InlineFallback.call_source` renders the full failed call source.
`InlineFallback.value_plain` is reserved for formatter failures where the runtime
value was available but formatter/style application failed.

Pure interpolation cannot emit commands, mutate state, play audio, or trigger stage effects. Use `[call]`, `[mark .name]` plus `with: on mark(.name):`, or line-plan `at(...) { ... }` for side-effecting dialogue behavior.

---

## Localization placeholders are separate

`#[expr]` is runtime expression interpolation. `{name}` is a localization placeholder.

```arcw
narrator.say(args={ player_name = state.player_name })[
    {player_name}は鍵を手に入れた。[p]
]
```

The extracted text key records the placeholder:

```toml
placeholders = [
  { name = "player_name", type = "String" }
]
```

Translation import checks that required placeholders are present and well-typed.

---

## Dialogue-safe calls and line marks

Use `[call]` for a dialogue-safe function:

```arcw
alice: まぶしい……[call flash(color=#ffffff, time=90ms)][p]
```

The short form `[! flash(color=#ffffff, time=90ms)]` is accepted for the same
dialogue-safe call.

Use `[mark .name]` to place a zero-width local marker in the line. Handlers live in the line plan; this keeps text markup separate from effectful behavior.

```arcw
alice: 変な夢[.keyword][p]
with:
    on mark(.keyword):
        mark_keyword(word="夢", color=@color.dream)
```

Dot-prefixed authoring tags are inferred only when the family is unambiguous.
Known rich-text selectors such as `[.vertical_rl]...[/]` and
`[.shake amp=1px]...[/]` lower as `layout` / `effect` spans. Unknown selectors
without attributes, such as `[.keyword]`, lower as zero-width marks. Unknown
selectors with attributes, such as `[.sparkle amp=2px]...[/]`, lower as custom
rich-text effects because markers do not carry parameters. Tooling can
canonicalize these inferred forms back to `[layout .vertical_rl]...[/layout]`,
`[mark .keyword]`, and `[effect .sparkle amp=2px]...[/effect]`.

A dialogue-safe function must declare its effects:

```arcw
pub dialogue fn flash(
    color: Color = rgb("#ffffff"),
    time: Duration = 120ms,
) -> Result<DialogueCue, TagError>
effects { stage.flash }
{
    Ok(DialogueCue.Flash { color, time })
}
```

A dialogue-safe function used by the handler:

```arcw
pub dialogue fn mark_keyword(
    word: String,
    color: Color,
) -> Result<DialogueCue, TagError>
{
    Ok(DialogueCue.StyleRange { word, color })
}
```

`[hook ...]`, `#[hook ...]`, `#[mark ...]`, and local `hook name:` blocks are not valid line-local syntax. Top-level `hook @hook...` declarations still exist for engine phase hooks, but dialogue text uses `[mark .name]` and line-plan `on mark(.name):` handlers.

---

## Escaping special characters

Inside dialogue text mode, special characters can be escaped:

| Escape | Output |
|---|---|
| `\\` | backslash |
| `\[` | literal `[` |
| `\]` | literal `]` |
| `\#` | literal `#` |
| `\$` | literal `$` |
| `\(` | literal `(` |
| `\)` | literal `)` |
| `\|` | literal ASCII ruby bar `|` |
| `\{` | literal `{` |
| `\}` | literal `}` |
| `\:` | literal `:` in contexts where it may be parsed specially |
| `\｜` | literal ruby bar `｜` |
| `\《` | literal `《` |
| `\》` | literal `》` |

Raw span:

```arcw
alice.say()[
    [raw]これは[p]をタグとして解釈しない。[/raw]
]
```

Short raw span:

```arcw
alice.say()[
    [raw: [p]をタグとして扱わない]
]
```

Raw block:

```arcw
alice.say()[
    [raw]
    ここでは複数行にわたりタグを解釈しない。
    [p] も文字として表示する。
    [/raw]
]
```

---

## Short style spans

Single-span style forms are accepted for common emphasis:

```arcw
alice: [em:夢]を見た。[strong:本当に]変だった。[p]
alice: [color #a8b5ff:夜]だけが光っていた。[p]
```

They normalize to the same rich-text span model as `[em]...[/em]`,
`[strong]...[/strong]`, and `[color value="..."]...[/color]`.

Font spans are typed rich-text style spans. Canonical generic families are
`serif`, `sans-serif`, `monospace`, `cursive`, and `fantasy`; any other quoted
or unquoted value is a requested named family that renderers may resolve through
their font system.

```arcw
alice: [font serif]Serif text[/font][p]
alice: [font "Noto Sans JP"]日本語フォント指定[/font][p]
```

Line-level dialogue `style` can set the base font for the whole line. Inline
`[font ...]` spans override that base font only for their span:

```arcw
alice(style=font(monospace)): 全体は monospace。[font serif]ここだけ serif[/font][p]

let alice_serif = alice(style=text_style(font=serif, color="#f7e8ff"))
alice_serif: この preset の通常表示は serif です。[p]
```

---

## Color and style hooks

Character default colors are defined in the character declaration. Dialogue lines inherit them automatically.

```arcw
pub character alice {
    display = "Alice"

    dialogue_style {
        text_color = rgb("#f7d7ff")
        name_color = rgb("#e070ff")
        unread_text_color = rgb("#ffffff")
        read_text_color = rgb("#c8c8d0")
    }
}
```

Built-in read/unread hook:

```arcw
pub dialogue defaults @dialogue.defaults {
    read_state_style = builtin.read_state_color(
        unread = rgb("#ffffff"),
        read = rgb("#b8b8c0"),
    )
}
```

Custom hook:

```arcw
pub hook @hook.dialogue.read_color
on query DialogueLine
phase BeforeTextStyle
when line.read_state == .Read
{
    line.style.text_color = rgb("#b8b8c0")
}
```

---

## Tag parsing and scope

`[]` tags are parsed only in dialogue text mode. Tag arguments are retained as
ordered positional or named values with source ranges. Quoted values may
contain whitespace. Unterminated quotes and duplicate or otherwise invalid
arguments are reported by the owning tag family instead of being collapsed by
a whitespace-to-map conversion.

`DialogueContent` removes authored indentation and normalizes physical line
endings to `\n`. Token, argument, expression, and end-tag ranges are therefore
byte ranges relative to the normalized `DialogueContent::raw()`, not absolute
document ranges. Compiler and tooling consumers project them to the authored
document with `DialogueContent.source_range(...)`; the reverse mapping for an
editor cursor uses `DialogueContent.content_offset(...)`. Removed indentation
and the interior byte of an authored CRLF pair intentionally have no reverse
content offset.

Values used by dialogue interpolation should be defined in the surrounding flow scope or inside a pure `#[...]` expression. Line-plan variables are for cues and cancellation, not for text interpolation that has already been parsed as content.

```arcw
let emphasis_color = rgb("#a8b5ff")

alice.say()[
    #[fmt("夢", color=emphasis_color, on_error=InlineFailure.fallback("夢"))]を見た。[p]
]
```

For cue-local values, use the line plan:

```arcw
alice.say()[
    夢を見た。[p]
]
with {
    let flash_color = rgb("#a8b5ff")
    at(0.2s) { flash(color=flash_color)? }
}
```

Line-plan variables are not visible after the line finishes. `at(...) { ... }` creates an even smaller cue-local scope.


