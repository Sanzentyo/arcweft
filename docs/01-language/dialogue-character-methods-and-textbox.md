# Dialogue Character Methods, Dialogue Windows, Speaker Presets, Interpolation, and Preload

Arcweft dialogue is written through character objects. The concise `alice:` form remains available, but it is syntax sugar over `alice.say()[ ... ]`. This keeps ordinary conversation compact while giving complex lines a typed, composable form.

Related:

- [Flow-Integrated Scenario Syntax](scenario-surface-syntax.md)
- [Dialogue Control Tags, Ruby, Inline Formatting, and Hooks](dialogue-control-tags-and-ruby.md)
- [Dialogue Calls, Line Plans, Cancellation, and Scoped Content Blocks](dialogue-calls-scopes-cancellation.md)
- [Dialogue Content Calls, `with` Blocks, Line Output Values, and Scoped Handles](dialogue-line-handles-and-returns.md)
- [Dialogue Windows, Character Styles, and Read-State Hooks](dialogue-windows-and-hooks.md)
- [Localization for Dialogue](localization-dialogue.md)
- [Character Stage / Sprite / Voice Timeline](../03-presentation/character-stage.md)
- [UI Reactive](../03-presentation/ui-reactive.md)

---

## Canonical form

The canonical ordinary dialogue form is:

```arcw
alice.say()[
    おはよう。[p]
]
```

With options:

```arcw
alice.say(
    id = @say.opening.greeting,
    voice = auto,
    look = smile,
    window = @textbox.main,
)[
    おはよう。[p]
]
```

Fully qualified entity references are allowed. Because `@character.alice.say` would be ambiguous as a single entity path, use `@<...>` or parentheses when calling a method on an entity reference:

```arcw
@<character.alice>.say(voice=auto)[
    おはよう。[p]
]

(@character.alice).say(voice=auto)[
    おはよう。[p]
]
```

The short `alice` form is a speaker alias resolved from the module's character imports or prelude:

```arcw
use game.characters.{alice, bob}
```

---

## `:` syntax sugar

For a character alias, the compact speaker-prefix form is sugar for
`Character.say(...)[...]`. For a `SpeakerPreset`, the same surface syntax calls
the preset and applies the dialogue content without forcing a character
`.say(...)` rewrite.

```arcw
alice: おはよう。[p]
```

is equivalent to:

```arcw
alice.say()[
    おはよう。[p]
]
```

Options in parentheses become `say()` options:

```arcw
alice(id=@say.opening.greeting, look=smile, voice=auto):
    おはよう。[p]
```

is equivalent to:

```arcw
alice.say(
    id = @say.opening.greeting,
    look = smile,
    voice = auto,
)[
    おはよう。[p]
]
```

`@` is the entity-reference marker. It is not used as a special option list marker.

The canonical expression/portrait option is `look`. The first positional option
is shorthand for `look`, so `alice(.smile, voice=auto):` and
`alice(look=.smile, voice=auto):` are equivalent. `face` is not a line option;
stage APIs use `alice.stage.look(...)` for authored look changes and lower-level
model-part APIs only in adapter-specific code.

Built-in line options include:

```text
look      default expression/portrait look for this line
stage     stage surface or stage policy
portrait  portrait surface or portrait policy
focus     focus profile; `focus=.soft` statically guarantees `'line.focus`
cleanup   cleanup profile for line-scoped handles and child tasks
voice     voice policy
window    textbox target
```

The same rule applies to the built-in narrator aliases:

```arcw
地の文: 扉の向こうから、雨の音がした。[p]
```

is equivalent to:

```arcw
@<character.narrator>.say()[
    扉の向こうから、雨の音がした。[p]
]
```

`:` is only sugar. The parser creates the same `DialogueLine` item as the canonical call form. There is no separate `script` language and no `script` lowering phase.


---

## Bracket content call

A character alias or speaker preset may be called directly with a dialogue
content block.

```arcw
alice[
    おはよう。[p]
]
```

For a character alias, this is sugar for:

```arcw
alice.say()[
    おはよう。[p]
]
```

Options still use the parenthesized speaker-preset call or the canonical
`.say(...)` call.

```arcw
alice(look=smile, voice=auto)[
    おはよう。[p]
]

alice.say(look=smile, voice=auto)[
    おはよう。[p]
]
```

When a line plan is needed, attach canonical `with { ... }` or indentation sugar `with:`.

```arcw
alice(look=smile, voice=auto)[
    おはよう。[p]
]
with:
    at(0.42s):
        alice.stage.look(worried)
```

This form is especially convenient for tool-generated dialogue because the content block and line plan are visually separated.

---

## Speaker presets and curried line options

A character alias is callable. Calling a character with line options does not
display text immediately; it returns a **speaker preset** that carries default
line options.

```arcw
let alice2 = alice(look=smile, voice=auto, window=@textbox.side)

alice2: おはよう。[p]

alice2(id=@say.opening.side_001):
    こっちのウィンドウで話すね。[p]
```

This is equivalent to:

```arcw
alice(look=smile, voice=auto, window=@textbox.side)[
    おはよう。[p]
]

alice2(id=@say.opening.side_001)[
    こっちのウィンドウで話すね。[p]
]
```

Presets can be refined by calling them again. Later options override earlier options.

```arcw
let alice_side = alice(window=@textbox.side, voice=auto)
let alice_worried = alice_side(look=worried)

alice_worried: ……本当に、大丈夫？[p]
```

The colon form on a speaker preset lowers through the preset call surface:

```text
alice2: text
  -> alice2()[text]

alice2(voice=auto): text
  -> alice2(voice=auto)[text]
```

If tooling chooses to display the fully expanded character call, it must preserve
the same effective option order and must not imply that the preset mutated the
underlying character.

The effective option order is:

```text
inline rich-text span
  -> per-line options
  -> speaker preset options
  -> character dialogue_style
  -> dialogue window theme
  -> selected dialogue defaults
  -> engine defaults
```

Types:

```arcw
pub type Speaker = Ref<Character> | SpeakerPreset

pub fn Character.call(self, options: SayOptions) -> SpeakerPreset
pub fn SpeakerPreset.call(self, options: SayOptions) -> SpeakerPreset
pub fn SpeakerPreset.say(self, options: SayOptions = {}) -> DialogueContentCall
```

The `:` sugar accepts both `Ref<Character>` and `SpeakerPreset`.

```arcw
let phone_alice = alice(window=@textbox.phone_message, voice=auto)
phone_alice: スマホに通知が届いた。[p]
```

The preset is lexical. It can be local to a block, flow, or helper function. It does not mutate the character definition.

---

## Object-like stage handles

Characters expose object-like stage APIs. The speaker alias remains a pure reference, while `stage` methods create handles for currently displayed presentation objects.

```arcw
let actor = alice.stage.acquire(scope=line)
let pose = actor.pose(normal)
let face = actor.look(smile)
```

These handles can be memoized or preloaded:

```arcw
let actor = memo(scope=scene, key=(@character.alice, pose=normal, theme=env.theme.hash)) {
    alice.stage.acquire(scope=line)
}
```

A `preload next` block explicitly prepares assets for a future flow.

```arcw
preload next @flow.alice_intro:
    alice.stage.prefetch(pose=normal, faces=[smile, worried], window=@textbox.main)
    alice.voice_for(@say.alice_intro.001).preload()
    bgm.prepare(@bgm.alice_theme)
```

Preload is a hint, not a hidden blocking operation. If the resource is not ready when used, the normal `Need<T, E>` pending rules still apply.

---

## Dialogue window target

A dialogue line always targets a text window. If no window is specified, Arcweft uses the global default dialogue window.

```arcw
alice.say()[おはよう。[p]]
```

is equivalent to:

```arcw
alice.say(window=@textbox.main)[おはよう。[p]]
```

Built-in textboxes:

| ID | Meaning |
|---|---|
| `@textbox.main` | Default main dialogue textbox |
| `@textbox.narrator` | Optional narration textbox; defaults to `@textbox.main` unless configured |
| `@textbox.system` | System messages / debug messages |

Project default:

```toml
[dialogue.default_window]
main = "textbox.main"
narrator = "textbox.narrator"
missing = "textbox.main"
```

Custom dialogue windows can be declared as UI views or text surfaces:

```arcw
pub textbox @textbox.phone_message PhoneMessageBox {
    layer = @layer.ui.messages
    anchor = bottom_right
    width = 420

    rich_text {
        text {
            color = rgb("#d8f0ff")
        }
    }
}
```

The `rich_text` block on a textbox is the textbox theme contribution for
dialogue rendered into that window. It participates in the same effective
RichText cascade as dialogue defaults, character `dialogue_style`, speaker
presets, line options, and inline spans.

Use it from dialogue:

```arcw
alice.say(window=@textbox.phone_message, voice=auto)[
    スマホに通知が届いた。[p]
]
```

The canonical parameter name is `window`. `textbox` is not a line option name;
it remains the entity kind for dialogue window objects.

A dialogue window target is a stateful UI object. Updating a line in that textbox affects the selected window only. Agent observation exposes the target window, current line ID, visible text, reveal cursor, and actionable wait state.

---

## Character text style and color

Characters can define default dialogue style, nameplate style, and voice/text policies.

```arcw
pub character alice {
    display = "Alice"
    display_name ja-JP = "アリス"
    display_name en-US = "Alice"

    dialogue_style {
        text_color = rgb("#f7d7ff")
        name_color = rgb("#e070ff")
        unread_text_color = rgb("#ffffff")
        read_text_color = rgb("#c8c8d0")
        window = @textbox.main
    }

    voice {
        default_locale = ja-JP
        speaker = @speaker.alice
        tts_profile = @tts.alice
    }
}
```

When a line is displayed, the effective style is resolved in this order:

```text
inline rich-text span
  -> line options
  -> speaker preset options
  -> character dialogue_style
  -> dialogue window theme
  -> selected dialogue defaults
  -> engine defaults
```

Example:

```arcw
alice.say(color=rgb("#ff8080"))[
    この行だけ赤い。[p]
]
```

---

## Dialogue defaults resolution

`dialogue defaults` declares a named defaults profile for dialogue lines. It is
not an implicit source-order macro and `pub` does not make the profile apply by
itself. `pub` only makes the defaults profile visible to project manifests,
other modules, tooling, and build profiles.

The conventional project-wide profile is:

```arcw
pub dialogue defaults @dialogue.defaults {
    window = @textbox.main
    reveal = typewriter(speed=normal)
}
```

Resolution is:

1. A project or build profile may explicitly select a defaults profile by ID.
2. If none is selected, `@dialogue.defaults` is the canonical implicit profile
   when it is visible from the entry module.
3. If `@dialogue.defaults` is absent and exactly one visible `dialogue defaults`
   declaration exists for the entry module, that profile may be used by dev
   tooling.
4. If multiple visible profiles exist and none is selected, product/test
   lowering reports an ambiguity diagnostic instead of merging them by source
   order.

Additional defaults profiles are inert until selected:

```arcw
pub dialogue defaults @dialogue.defaults.debug {
    window = @textbox.system
    reveal = instant
}

pub dialogue defaults @dialogue.defaults.mobile {
    window = @textbox.phone_message
    rich_text {
        text {
            size = 24px
        }

        ruby {
            size = 11px
            gap = 1px
        }
    }
}
```

A selected defaults profile is the base of the dialogue cascade:

```text
inline rich-text span
  -> line options
  -> speaker preset options
  -> character dialogue_style
  -> dialogue window theme
  -> selected dialogue defaults
  -> engine defaults
```

Scalar fields use nearest-wins semantics. Structured style records such as
`rich_text`, `rich_text.text`, `rich_text.layout`, and `rich_text.ruby` deep
merge by field, so a character can override only ruby size while inheriting the
global ruby gap. Lists and hook collections must use explicit operators:
`=` replaces the collection, `+=` appends, and future removal operators must be
spelled explicitly rather than inferred.

`dialogue defaults` should carry dialogue policy, not arbitrary renderer state.
Window choice, reveal behavior, voice policy, hooks, localization policy, and
RichText typography are appropriate. Per-scene state, temporary line-plan
variables, and stage handles belong in flows, speaker presets, or line plans.

---

## RichText typography defaults

RichText defaults are grouped under `rich_text` instead of flattening every
text parameter into `dialogue defaults`. This keeps ruby, vertical writing,
font choice, wrapping, effects, and future typography parameters in one
namespace that can also be reused by choices, UI text, logs, and HUD text.

```arcw
pub dialogue defaults @dialogue.defaults {
    window = @textbox.main

    rich_text {
        text {
            font = "Yu Gothic"
            size = 30px
            color = rgb("#f5f5f5")
        }

        layout {
            writing_mode = horizontal_tb
            jlreq = normal
            vertical_latin = mixed
            wrap = textbox
            overflow = page
        }

        ruby {
            position = over
            size = 14px
            gap = 2px
            overhang = 7px
            collision_gap = 2px
        }
    }
}
```

Character defaults use the same structure and override only the fields they
mention:

```arcw
pub character alice {
    display = "Alice"

    dialogue_style {
        rich_text {
            text {
                color = rgb("#f7d7ff")
            }

            ruby {
                size = 13px
                gap = 1px
            }
        }
    }
}
```

Line and speaker preset options may pass the same data as a typed value:

```arcw
let phone_alice = alice(
    window = @textbox.phone_message,
    rich_text = rich_text_style(
        text = text_style(size=24px),
        ruby = ruby_style(size=11px, gap=1px),
    ),
)
```

Inline rich-text selectors remain the most local override:

```arcw
alice: [.ruby_over ruby_size=11px ruby_gap=1px]|[夢](ゆめ)[/][p]
```

The inline spelling uses `ruby_size` and `ruby_gap` because tag attributes share
one flat namespace. Defaults use `rich_text { ruby { size = ... } }` because the
record path already disambiguates the field.

---

## Built-in read/unread style hooks

Common visual-novel patterns are built in.

```arcw
pub dialogue defaults @dialogue.defaults {
    window = @textbox.main
    read_state_style = builtin.read_state_color(
        unread = rgb("#ffffff"),
        read = rgb("#b8b8c0"),
    )
    auto_mark_read = on_page_advance
}
```

Character-level override:

```arcw
pub character alice {
    display = "Alice"

    dialogue_style {
        read_state_style = builtin.read_state_color(
            unread = rgb("#ffeaff"),
            read = rgb("#d0b8d8"),
        )
    }
}
```

A custom hook can override or extend this behavior:

```arcw
pub hook @hook.dialogue.alice_read_color
on query DialogueLine where line.speaker == @character.alice
phase BeforeTextStyle
when line.read_state == .Read
{
    line.style.text_color = rgb("#c5b6cc")
}
```

Built-in hook patterns:

| Built-in | Purpose |
|---|---|
| `builtin.read_state_color` | Change line color depending on read/unread state |
| `builtin.choice_seen_color` | Color visited choices differently |
| `builtin.voice_missing_marker` | Add a subtle mark when localized voice is missing |
| `builtin.locale_stale_marker` | Add debug styling when translation is stale |
| `builtin.auto_mark_read` | Mark line read on page advance or voice end |

---

## Interpolation and `DisplayText`

Dialogue text may insert values. Insertion is only allowed for values that implement `DisplayText`, or values explicitly wrapped by `fmt(...)`.

```arcw
narrator.say()[
    #[player_name]は鍵を手に入れた。[p]
]
```

This requires:

```arcw
player_name: DisplayText
```

Explicit formatting:

```arcw
narrator.say()[
    スコアは#[fmt(score, style="number", on_error=InlineFailure.fallback("?"))]点です。[p]
]
```

Formatting with locale:

```arcw
narrator.say()[
    所持金: #[fmt(money, currency="JPY", locale=state.locale, on_error=InlineFailure.fallback("--"))][p]
]
```

Trait:

```arcw
pub trait DisplayText {
    fn display_text(self, ctx: DisplayContext) -> Result<Content, DisplayError>
}
```

Built-in implementations:

```text
String, LocalizedText, i32, u32, f32, bool, Duration, DateTime, Ref<T>, Option<T>, Result<T,E>
```

`Option<T>` is displayed only if explicitly formatted or matched. This avoids accidentally showing `None` in player-facing text.

```arcw
match state.player_nickname {
    Some(nick) => alice.say()[#[nick]、おはよう。[p]]
    None => alice.say()[おはよう。[p]]
}
```

`fmt(...)` can also wrap values into `Content` for hooks and custom tags:

```arcw
#[fmt(route_title(state.route, on_error=InlineFailure.fallback("")), color=@color.accent, on_error=InlineFailure.fallback(""))]
```

Inline function calls in `#[...]` must declare failure handling for that call, or
the surrounding line, speaker preset, character state, or dialogue defaults must
provide an inline failure policy. Canonical values use the `InlineFailure` enum
namespace; contextual `.fail` and `.discard` shorthands are accepted where
`InlineFailure` is expected. For ordinary display text, prefer a default
fallback on the preset or character instead of repeating `on_error` on every
`fmt(...)` call:

```arcw
let alice_text = alice(inline_error=InlineFailure.fallback("?"))
alice_text: #[fmt(score, style="number")]点[p]
alice: #[fmt(score, style="number", on_error=InlineFailure.fallback("?"))]点[p]
alice: #[fmt(score, style="number", on_error=.fail)]点[p]
alice: #[fmt(score, style="number", on_error=InlineFailure.fallback(InlineFallback.expr_source))]点[p]
```

Use `on_error=InlineFailure.discard`, `on_error=.discard`, or
`inline_error=.discard` only when omitting the text is intentional. `on_error`,
`fallback`, and `discard_error` are mutually exclusive.

### Interpolation vs localization placeholders

`#[expr]` is runtime interpolation. `{name}` inside rich text is a localization placeholder.

```arcw
narrator.say()[
    {player_name}は鍵を手に入れた。[p]
]
```

The above is extracted as a localizable string with placeholder `player_name`. At runtime the placeholder must be supplied by the line options or surrounding context:

```arcw
narrator.say(args={ player_name = state.player_name })[
    {player_name}は鍵を手に入れた。[p]
]
```

Use `#[expr]` for computed text that should not be translated as a placeholder.

---

## Escaping special characters

Inside dialogue-text mode, the following characters can be escaped:

| Escape | Output |
|---|---|
| `\\` | backslash |
| `\[` | literal `[` |
| `\]` | literal `]` |
| `\#` | literal `#` |
| `\{` | literal `{` |
| `\}` | literal `}` |
| `\:` | literal `:` in contexts where it may be parsed specially |
| `\｜` | literal ruby bar `｜` |
| `\《` | literal `《` |
| `\》` | literal `》` |

Raw span:

```arcw
alice.say()[
    [raw]ここでは[p]も#[expr]も解釈されない。[/raw]
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

## Hooks inside dialogue text

`#[...]` is for safe expression/content insertion. Side-effecting local line behavior uses `[mark .name]` with `with: on mark(.name):` or a dialogue-safe `[call ...]`.

```arcw
alice.say()[
    まぶしい……[call flash(color=#ffffff, time=90ms)][p]
]
```

Custom hook:

```arcw
pub dialogue hook @hook.dialogue.mark_keyword mark_keyword(
    word: String,
    color: Color = rgb("#ffcc00"),
) -> Result<DialogueCue, TagError>
{
    Ok(DialogueCue.StyleRange { word, color })
}
```

Use:

```arcw
alice.say()[
    変な夢[mark .keyword][p]
with:
    on mark(.keyword):
        mark_keyword(word="夢", color=@color.dream)
]
```

Hooks may read line context, speaker, dialogue window, read state, locale, and reveal cursor. They may not mutate global state unless explicitly declared with capability.

---

## Scoped line blocks

`Character.say(...)[...]`, `with { ... }`, `with:`, and `at(...) { ... }` create lexical scopes. `with:` normalizes to `with { ... }`.

```arcw
alice.say(voice=auto)[

    今日は少しだけ、変な夢を見たんだ。[p]

]
with {

    let line_theme = @theme.dream

    at(0.4s) {

        let blink = @anim.blink.once

        alice.stage.animate(blink)

    }

}
```

Scope rules:

```text
- variables in the canonical line-plan block are visible to timed cues;
- variables in `at(...) { ... }` are local to that cue;
- variables in content interpolation `#[...]` cannot leak outward;
- borrowed references cannot cross wait/reveal/cancel boundaries;
- line-scoped memo entries are discarded when the line completes.
```

---

## Character stage as an object

A character alias also exposes stage-oriented methods. These methods operate on a stage instance and return typed commands or `Need` values when loading is required.

```arcw
alice.stage.show(smile, at=center, fade=200ms)
alice.stage.look(worried, crossfade=120ms)
alice.stage.move(to=left, time=300ms, ease=cubic.out)
alice.stage.scale(1.05, time=200ms)
alice.stage.hide(fade=180ms)
```

When the instance matters:

```arcw
alice.stage(@stage.alice.main).move(to=left, time=300ms)
```

Fully qualified:

```arcw
@<character.alice>.stage(@stage.alice.main).look(worried)
```

Character staging uses ordinary effectful calls and methods. There is no
separate `@show`, `@face`, or `@move` command family.

```arcw
show(@character.alice, .smile, at = .center, fade = 200ms)
```

is sugar for:

```arcw
alice.stage.show(smile, at=center, fade=200ms)
```

---

## Character preload and memoization

Characters own preload and memoization policies for sprites, expressions, mouth parts, voice metadata, and composed render layers.

```arcw
pub character alice {
    display = "Alice"

    preload_policy {
        sprites = on_flow_anticipate
        expressions = [normal, smile, worried]
        voices = locale_current
        lipsync = metadata_only
    }

    memo_policy {
        compose_sprite key=(pose, expression, mouth, scale_bucket, locale) cache=session
        text_style key=(read_state, locale, theme) cache=flow
    }
}
```

Explicit preload:

```arcw
preload character alice {
    expressions [normal, smile, worried]
    voices for flow @flow.alice_intro locale current
    sprites scale_buckets [1.0, 1.25]
}
```

Preload for likely next flow:

```arcw
anticipate @flow.alice_intro {
    alice.preload(expressions=[smile, worried], voices=auto, sprites=true)
    asset.preload(@asset:.bg.room_evening)
    shader.preload(@shader.transition.dissolve)
}
```

`anticipate` is an explicit scheduling hint. It starts tasks early but never changes story state. If a preloaded value is still pending when used, the normal `Need`/`await with` rules still apply.

---

## Design decisions

```text
1. `Character.say(...)[...]` is canonical for character-alias dialogue.
2. `speaker:` is only sugar for applying dialogue content to a speaker value. For `Ref<Character>` this is `speaker.say()[...]`; for `SpeakerPreset` this is `speaker()[...]`.
3. There is no `script` item and no script-lowering phase.
4. A missing dialogue window target resolves to `@textbox.main`.
5. `alice(options)` creates a lexical speaker preset; it does not display text until `:`, `[...]`, or `.say()[...]` is used.
6. Text interpolation uses `DisplayText` or explicit `fmt(...)`.
7. Runtime interpolation `#[expr]` is separate from localization placeholders `{name}`.
8. Dialogue text uses `[mark .name]` with line-plan `on mark(.name):` handlers or `[call ...]`; pure content insertion uses `#[...]`.
9. Characters define default text/name colors and may attach read/unread style hooks.
10. Character stage methods provide object-oriented sugar for show/face/move/animate/preload.
11. Character preload and memoization policies are explicit and observable by Agent Debug Bus.
```


