# Dialogue Content Actions, Ruby, Interpolation, and Line Marks

> **Converged surface authority:**
> [Converged Language, Content, and Presentation Surface](converged-language-surface.md)
> defines the canonical typed content calls in this chapter. The success surface
> uses explicit `#name(args)[content]` calls; bracket syntax is reserved for
> point controls, marks, and host actions.

Arcweft dialogue content has one typed body-bearing surface. `#name(args)[content]`
calls own rich-text bodies, while `[...]` is reserved for point controls, marks,
and host actions. These forms are recognized only in dialogue text mode: speaker
lines, narrator lines, indented dialogue bodies, and direct character calls.

The former paired or inline bracket bodies, bracket/compact Ruby alternatives,
bracket raw blocks, and dollar-parenthesis interpolation are not language
surfaces. There is no spelling-specific tombstone diagnostic, compatibility
reader, formatter rewrite, or source action for them; ordinary current grammar
and recovery apply.

Related:

- [Flow-Integrated Scenario Syntax](scenario-surface-syntax.md)
- [Dialogue Character Configuration, Views, Interpolation, and Preload](dialogue-character-methods-and-views.md)
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

alice(voice=auto)[
    今日は少しだけ、|[変な夢](へんなゆめ)を見たんだ。[p]
]
```

Only in those text regions, and in typed `fn(args)[content]` content blocks whose
declared content type is dialogue/rich text, are point controls, host actions,
typed `#name(args)[content]` calls, and `#[...]` interpreted as dialogue markup.
In normal typed code, brackets keep their normal meaning.

Historical flat-fence imports treat a physical text line beginning with `===`
as a fence. In stable source, use canonical dialogue calls such as
`alice()[...]`; tooling migrations escape literal text that begins with
three equals signs as `\===`.

---

## Content calls and point actions

Arcweft has several authored forms in dialogue text:

| Form | Purpose |
|---|---|
| `[p]`, `[l]`, `[r]` | short built-in control actions |
| `[voice ...]`, `[face ...]`, `[pose ...]`, `[show ...]`, `[hide ...]`, `[move ...]`, `[scale ...]`, `[rotate ...]`, `[anim ...]`, `[shake ...]`, `[signal ...]` | typed host actions |
| `[mark @.name]` | explicit zero-width line-local marker for `with` handlers and waits |
| `#[expr]`, `#[fmt(...)]` | pure content interpolation |
| `#strong()[...]`, `#em()[...]`, `#color(rgb("..."))[...]`, `#font("...")[...]`, `#size(36pt)[...]` | direct typed style calls |
| `#style(.selector, named=...)[...]`, `#layout(.selector, named=...)[...]`, `#transform(.selector, named=...)[...]` | typed selector calls |
| `#ruby("reading")[base]`, `#raw()[literal]`, `#object(id=@.id, type=Type)[body]`, `#fx(wave(...))[...]` | typed content and presentation calls |
| `[call ...]`, `[at ...]` | dialogue-safe dispatch and timed cue actions |

Double brackets are not dialogue content actions:

```arcw
/// [[flow.alice_intro]] is a documentation/RAG link, not dialogue markup.
```

---

## Built-in reserved names

These names are reserved in point-action or content-call position and scenario-command position. They cannot be used as unqualified custom action names, unqualified scenario command names, character aliases, or local variables in dialogue content scope.

| Name | Meaning |
|---|---|
| `p` | user wait that closes the current logical page |
| `l` | user wait that keeps the current logical page open |
| `r` | hard line break |
| `w` | automatic timed wait reached during reveal |
| `clear` | immediately reset displayed text when reached |
| `#ruby` | typed ruby content call |
| `#em`, `#strong` | typed emphasis content calls |
| `#color`, `#font`, `#size` | typed rich-text content calls |
| `speed` | reveal rate for subsequent text |
| `#object` | typed text presentation object/proxy content call |
| `reset` | reset text style/reveal modifiers |
| `voice` | voice cue inside a line |
| `face`, `pose` | expression/pose change |
| `show`, `hide` | stage visibility cue |
| `move`, `scale`, `rotate` | transform cue |
| `anim`, `shake` | animation cue |
| `mark` | zero-width line-local marker |
| `at` | timed cue shorthand inside dialogue text |
| `call` | call an allowed dialogue function |
| `signal` | emit/set a public signal if capability allows it |
| `#raw` | literal no-parse content call |
| `fmt` | explicit DisplayText/content formatting function |
| `#fx` | apply a typed Fx value produced by a standard or declared `#[fx]` callable |

---

## Typed rich-text calls

Body-bearing presentation is always a typed content call. Selectors and enum
values are dot-prefixed, vectors use `vec2(...)`, public IDs use `@`-form, and
every zero-argument call keeps its `()`:

```arcw
alice: #strong()[強調]、#em()[斜体]、#color(rgb("#a8b5ff"))[夜]。[p]
alice: #font("Yu Gothic")[游ゴシック]、#size(36pt)[大きな語]。[p]
alice: #style(.oblique, angle=12deg)[斜体角度][p]
alice: #layout(.vertical_rl, jlreq=.strict)[縦書き][p]
alice: #transform(.offset, x=4px, y=-2px)[少しずらす][p]
alice: #fx(wave(direction=vec2(0.0, 1.0), phase=.glyph_transform))[揺れる文字][p]
alice: #object(id=@.hotspot, type=KeywordHit, channel="choice")[当たり判定つき文字][p]
```

The selector families are closed typed registries. Unknown dot selectors,
inferred spans, and body-bearing bracket forms are rejected rather than
reclassified as a different family.

Text presentation object proxies are explicit generic calls:
`#object(id = @.name, type = Name, ...)[...]`. Both `id` and `type` are
required. They preserve custom proxy metadata for
hit-testing, depth ordering, object-id capture, and renderer/tooling registries
without reinterpreting the span as a visual effect. The declaration-time proxy
type may be marked with normal Arcweft attributes such as `#[text_proxy(...)]`;
inline dialogue text refers to it with `type = Name` inside the call, so it does
not conflict with `#[expr]` interpolation. The object call is not inferred from
a dot selector. Author proxy spans with the explicit
`#object(id = @.id, type = Name, ...)[...]` form. Final semantic analysis uses
declaration attributes as proxy defaults: `role` becomes the default role,
`hit_test` becomes the default hit-test policy, and `depth` becomes default
local pixel-milli depth and therefore requires an explicit `px` unit,
and any remaining attribute arguments become default typed proxy params unless
the inline object span overrides them.

Fx and shader parameters are checked by the owning callable schema. Use
`direction=vec2(0.0, 1.0)`, dot-prefixed phase/target enums, `@`-form shader resource IDs, and
numeric `u32` seeds where the selected Fx callable schema accepts them. Host and
timeline actions remain point actions in brackets; they are never converted to
visual effect spans.

Layout selectors accept `jlreq=.loose`, `jlreq=.normal`, or `jlreq=.strict` to choose the vertical
Japanese punctuation-pair planning preset for that span. Omitting it keeps the
host View/default layout preset.

Ruby typography belongs to the selected presentation style/default authority;
it is not a body-bearing inferred selector. Use the typed ruby call for content:

```arcw
alice: #layout(.ruby_over, ruby_size=11px, ruby_gap=1px, ruby_overhang=4px, ruby_collision_gap=3px)[|[夢](ゆめ)][p]
```

These named arguments override the effective style/default values only for the
enclosed span. Defaults are authored in the selected dialogue View/style or
profile rather than in a character declaration.

---

## Reusable presentation Fx

One ordinary `#[fx]` function defines a typed, reusable presentation treatment.
Static text style and animated transforms use the same immutable `Fx` graph:

```arcw
#[fx]
pub fn warning(
    accent: Color = rgb("#ff4050"),
    amplitude: Length = 2px,
) -> Fx {
    Fx.stack([
        Fx.text(weight = .strong, color = accent),
        wave(amplitude = amplitude),
    ])
}

alice: #fx(warning())[既定値の警告][p]
alice: #fx(warning(accent=rgb("#ffd060"), amplitude=4px))[強い警告][p]
```

The invocation after `fx` is a normal path-resolved call to a `#[fx]`
function, not a dot selector or a separately registered decoration id. The
function's original package and qualified name define its `FxId`; ordinary
`use` and `pub use` provide name resolution without manufacturing another id.
Builtin producers such as `wave(...)` use that same call path. `#wave(...)`
and `#effect(...)` are not alternate surfaces; `#fx(...)` is the sole
body-bearing Content adapter.

Fx parameters are typed and calls are named-only. A parameter without `=` is
required; a parameter default is const-evaluable and cannot read another
parameter or runtime state. Rest parameters and silently forwarded custom
argument bags are not supported. Unknown, duplicate, missing, and positional
arguments are diagnostics, which keeps completion, ABI checking, and renderer
bindings on a closed schema.

Dialogue-inline arguments must be closed values. Arbitrary state expressions
inside a line would destabilize localization, line caching, replay, and
tooling, so dynamic presentation belongs on a View-side `RichText(...)` value:

```arcw
RichText(message)
    .fx(warning(accent = state.warning_color))
```

Nested Fx calls compose in authored order, and the compiler rejects malformed
content-call boundaries, composition cycles, and expansion-budget overflow. An Fx function is implicitly
pure and deterministic: it cannot hide waits, pages, reveal-speed changes,
marks, object proxies, host events, state mutation, actions, I/O, tasks, or
View-child construction.

`#[fx] fn ... -> Fx` and the two application forms above are the complete
reusable presentation-effect grammar.

---

## Wait and newline actions

```arcw
alice: おはよう。[l]今日はいい天気だね。[p]
```

Meaning:

```text
[l]  wait for user advance; then continue revealing on the same logical page.
[p]  wait for user advance; close the current logical page before later text.
```

Logical page boundaries are authored behavior, not a View setting. If more
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

`[w]` begins only after reveal reaches its marker. It pauses automatically for
the authored duration and then resumes without user input. The duration must be
positive and use `ms` or `s`, for example `250ms`, `1s`, or `0.5s`. Missing,
zero, negative, unsupported-unit, sub-millisecond, and overflowing durations
are compile-time errors.

`[clear]` resets the currently displayed text immediately when reveal reaches
the marker. It neither waits for input nor closes the logical page; use an
adjacent `[l]` or `[p]` when a wait is also required. When `[l]` follows, the
next stage retains the post-clear display rather than reconstructing text
removed before the marker.

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

Dialogue Views may animate or style a logical-page transition, but they do not
change whether `[p]` closes a page, `[l]` retains it, or a terminal `[p]`
releases the line.

---

## Ruby

Arcweft supports two retained ruby spellings. The recommended authoring form is
`|[base](ruby)` because it is ASCII-friendly and works when the base contains
spaces or punctuation. The natural Japanese form is retained for Japanese
source.

### ASCII explicit ruby

```arcw
alice: 今日は少しだけ、|[変な夢](へんなゆめ)を見たんだ。[p]
```

### Natural Japanese ruby

```arcw
alice: 今日は少しだけ、｜変な夢《へんなゆめ》を見たんだ。[p]
```

### Function/content form

```arcw
alice()[
    今日は少しだけ、#ruby("へんなゆめ")[変な夢]を見たんだ。[p]
]
```

Both retained forms and the typed call normalize into the same
`Content.Ruby { base, ruby }` fragment.

Ruby typography is resolved from the active RichText cascade before layout:

```text
inline typed ruby call
  -> authored dialogue View style
  -> selected profile dialogue Style/default
  -> engine defaults
```

The ruby content syntax chooses the base and annotation text. Typography such
as position, annotation size, base gap, overhang, and collision separation comes
from the active `rich_text.ruby` style unless an inline ruby selector overrides
it.

Localization import validates ruby fragments:

```text
- natural ruby delimiters are balanced;
- typed ruby content has a non-empty base and reading;
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

`#[expr]` inserts the formatted representation of `expr`. The expression must
implement `DisplayText`; there is no alternate delimiter for this operation.

```arcw
narrator()[
    #[player_name]は鍵を手に入れた。[p]
]
```

If formatting needs options, use `fmt(...)` explicitly:

```arcw
narrator()[
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
 handled, unless the line, configured dialogue values, character state, or selected profile
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

Pure interpolation cannot emit commands, mutate state, play audio, or trigger stage effects. Use `[call]`, `[mark @.name]` plus `with: on mark(@.name):`, or line-plan `at(...) { ... }` for side-effecting dialogue behavior.

---

## Localization placeholders are separate

`#[expr]` is runtime expression interpolation. `{name}` is a localization placeholder.

```arcw
narrator(args={ player_name = state.player_name })[
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
alice: まぶしい……[call flash(color=rgb("#ffffff"), time=90ms)][p]
```

Use `[mark @.name]` to place a zero-width local marker in the line. Handlers live in the line plan; this keeps text markup separate from effectful behavior.

```arcw
alice: 変な夢[mark @.keyword][p]
with:
    on mark(@.keyword):
        mark_keyword(word="夢", color=@color.dream)
```

Body-bearing selectors and Fx do not use bracket actions. Use typed calls such as
`#layout(.vertical_rl)[縦書き]`, `#fx(shake(amplitude=1px))[揺れる]`, and
`#object(id=@.id, type=Type)[対象]`; unknown selectors and Fx callables are rejected. Host and
timeline actions remain point actions in brackets.

Zero-width dialogue operations are ordinary typed callables and are reached
through `[call ...]` point actions. A registered body-bearing callable is
reached through the same path-owned surface, `#path(args)[body]`; neither form
needs a second dialogue-language declaration.

The mark handler above may call a registered ordinary `mark_keyword` callable;
its effects are checked by the normal callable contract.

Dialogue text declares local synchronization points with `[mark @.name]` and
handles them with line-plan `on mark(@.name):` clauses. Top-level
`hook @hook...` declarations are a separate engine-phase construct.

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
alice()[
    #raw()[これは[p]をタグとして解釈しない。]
]
```

Raw block:

```arcw
alice()[
    #raw()[
    ここでは複数行にわたりタグを解釈しない。
    [p] も文字として表示する。
    ]
]
```

---

## Typed style spans

Style spans are typed content calls. Canonical generic font families are
`serif`, `sans-serif`, `monospace`, `cursive`, and `fantasy`; a quoted value is
a requested named family that renderers may resolve through their font system.

```arcw
alice: #font("serif")[Serif text][p]
alice: #font("Noto Sans JP")[日本語フォント指定][p]
alice: #color(rgb("#a8b5ff"))[夜]、#strong()[本当に]変だった。[p]
```

Line-level presentation style can set the base font for the whole line. Inline
`#font(...)` calls override that base font only for their span:

```arcw
alice(style=font(monospace)): 全体は monospace。#font("serif")[ここだけ serif][p]

let alice_serif = alice(style=text_style(font=serif, color=rgb("#f7e8ff")))
alice_serif: この preset の通常表示は serif です。[p]
```

---

## Color and style hooks

Character declarations own identity/display only. Default colors are defined in
the selected dialogue View/style or profile, and dialogue lines inherit them
through that presentation authority.

```arcw
pub character alice {
    display = "Alice"
}

pub style alice_dialogue {
    .dialogue_content {
        color = rgba(247, 215, 255, 255)
    }
}
```

Custom read-state presentation belongs in the selected dialogue View and its
Style, or in character-local style policy, rather than a global callback.

---

## Content-action parsing and scope

Point-action arguments and content-call arguments are parsed only in dialogue
text mode. They are retained as ordered positional or named values with source
ranges. Quoted values may contain whitespace. Unterminated quotes and duplicate
or otherwise invalid arguments are reported by the owning typed schema instead
of being collapsed by a whitespace-to-map conversion.

`DialogueContent` removes authored indentation and normalizes physical line
endings to `\n`. Token, argument, expression, and body ranges are therefore
byte ranges relative to the normalized `DialogueContent::raw()`, not absolute
document ranges. Compiler and tooling consumers project them to the authored
document with `DialogueContent.source_range(...)`; the reverse mapping for an
editor cursor uses `DialogueContent.content_offset(...)`. Removed indentation
and the interior byte of an authored CRLF pair intentionally have no reverse
content offset.

Values used by dialogue interpolation should be defined in the surrounding flow scope or inside a pure `#[...]` expression. Line-plan variables are for cues and cancellation, not for text interpolation that has already been parsed as content.

```arcw
let emphasis_color = rgb("#a8b5ff")

alice()[
    #[fmt("夢", color=emphasis_color, on_error=InlineFailure.fallback("夢"))]を見た。[p]
]
```

For cue-local values, use the line plan:

```arcw
alice()[
    夢を見た。[p]
]
with {
    let flash_color = rgb("#a8b5ff")
    at(0.2s) { try flash(color=flash_color) }
}
```

Line-plan variables are not visible after the line finishes. `at(...) { ... }` creates an even smaller cue-local scope.


