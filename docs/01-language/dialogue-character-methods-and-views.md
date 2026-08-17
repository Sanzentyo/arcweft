# Dialogue Character Methods, Dialogue Views, Speaker Presets, Interpolation, and Preload

> **Superseded dialogue-call surface:** The canonical final model is
> [CharacterDialogue authoring](character-dialogue.md). In particular,
> `.say(...)`, `Speaker`, and `SpeakerPreset` are removed rather than retained
> as aliases. Examples below that use those spellings are migration inventory,
> not current authoring syntax; their non-dialogue Character, View,
> interpolation, and preload material remains applicable unless the new
> contract says otherwise.
>
> Dialogue content escapes, attached-body roles, Ruby, timeline calls, marks,
> and reactive View syntax are governed by
> [Converged Language, Content, and Presentation Surface](converged-language-surface.md).

Arcweft dialogue is written through character objects. The concise `alice:` form remains available, but it is syntax sugar over `alice.say()[ ... ]`. This keeps ordinary conversation compact while giving complex lines a typed, composable form.

Related:

- [Flow-Integrated Scenario Syntax](scenario-surface-syntax.md)
- [Dialogue Control Tags, Ruby, Inline Formatting, and Hooks](dialogue-control-tags-and-ruby.md)
- [Dialogue Calls, Line Plans, Cancellation, and Scoped Content Blocks](dialogue-calls-scopes-cancellation.md)
- [Dialogue Content Calls, `with` Blocks, Line Output Values, and Scoped Handles](dialogue-line-handles-and-returns.md)
- [Dialogue Views, Character Styles, and Read-State Hooks](dialogue-views-and-hooks.md)
- [Localization for Dialogue](localization-dialogue.md)
- [Character Stage / Sprite / Voice Timeline](../03-presentation/character-stage.md)
- [View Reactive](../03-presentation/view-reactive.md)

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
    view = @view.MainDialogue,
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
view      authored dialogue View target
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
let alice2 = alice(look=smile, voice=auto, view=@view.SideDialogue)

alice2: おはよう。[p]

alice2(id=@say.opening.side_001):
    こっちのViewで話すね。[p]
```

This is equivalent to:

```arcw
alice(look=smile, voice=auto, view=@view.SideDialogue)[
    おはよう。[p]
]

alice2(id=@say.opening.side_001)[
    こっちのViewで話すね。[p]
]
```

Presets can be refined by calling them again. Later options override earlier options.

```arcw
let alice_side = alice(view=@view.SideDialogue, voice=auto)
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
  -> authored dialogue View style
  -> selected profile dialogue Style
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
let phone_alice = alice(view=@view.PhoneMessage, voice=auto)
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

These handles can be scoped or preloaded. The stage subsystem owns resource
reuse rather than exposing a generic memo expression:

```arcw
let actor = alice.stage.acquire(scope=scene)
```

A `preload next` block explicitly prepares assets for a future flow.

```arcw
preload next @flow.alice_intro:
    alice.stage.prefetch(pose=normal, faces=[smile, worried], view=@view.MainDialogue)
    alice.voice_for(@say.alice_intro.001).preload()
    bgm.prepare(@bgm.alice_theme)
```

Preload is a hint, not a hidden blocking operation. If the resource is not ready when used, the normal unary `Need<T>` pending rules still apply; domain failure belongs to its payload Result.

---

## Dialogue View target

A dialogue line always targets an authored persistent View. If no line or
speaker-preset `view` override is specified, Arcweft uses the typed View selected
by the accepted launch or project profile, then the standard library's minimal
dialogue View resource.

```arcw
alice.say()[おはよう。[p]]
```

resolves to the reserved `std.view.dialogue` resource when no accepted profile
selects another View. The reserved identity is not written as a source
`EntityRef` and cannot be redeclared.

The standard resource has the same contract as a project View:

| ID | Meaning |
|---|---|
| `std.view.dialogue` | Reserved minimal standard speaker/content View resource |

Project profile ownership:

```toml
[profiles.game.dialogue]
view = "view.phone_message"
style = "style.phone_message"

[profiles.game.dialogue.inline-failure]
kind = "fail_line"
```

Projects normally declare a style and a typed View:

```arcw
pub style phone_message {
    .phone_message {
        color = rgba(216, 240, 255, 255)
    }
}

pub view PhoneMessage(dialogue: DialogueView) {
    Panel {
        Text(dialogue.speaker)
        RichText(dialogue.content)
            .style(@style.phone_message)
    }
}
```

`DialogueView` is a standard-prelude nominal record and is visible to the type
checker and LSP. The View body chooses the exact `Text` or `RichText` consumer;
its style participates in the same effective RichText cascade as the selected
profile dialogue Style, character `dialogue_style`, speaker presets, line
options, and inline spans.

Use it from dialogue:

```arcw
alice.say(view=@view.PhoneMessage, voice=auto)[
    スマホに通知が届いた。[p]
]
```

The canonical parameter name is `view`. Each active target is a stateful,
persistent View mount. Updating one target affects only that mount. Agent
observation exposes the mount, current line ID, visible text, reveal cursor,
and actionable wait state.

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
        view = @view.MainDialogue
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
  -> authored dialogue View style
  -> selected profile dialogue Style
  -> engine defaults
```

Example:

```arcw
alice.say(color=rgb("#ff8080"))[
    この行だけ赤い。[p]
]
```

---

## Profile dialogue presentation

The accepted launch or project profile owns the base dialogue presentation. It
is neither inferred from source order nor selected by a stream-producing
callable.

```toml
[profiles.mobile.dialogue]
view = "view.phone_message"
style = "style.phone_message"

[profiles.mobile.dialogue.inline-failure]
kind = "fallback"

[profiles.mobile.dialogue.inline-failure.fallback]
kind = "text"
text = "?"

[profiles.mobile.dialogue.inline-failure.fallback.style]
kind = "inherit_surrounding"
```

`view` is a typed View identity. `style`, when present, is a typed base Style
identity. The inline-failure table is the profile-wide policy for interpolation
failures not handled by a more local line, preset, or character policy. A
profile without a View selection uses `std.view.dialogue`; it does not search
stream-producing callables for a default.

The selected profile Style is the base of the dialogue cascade:

```text
inline rich-text span
  -> line options
  -> speaker preset options
  -> character dialogue_style
  -> authored dialogue View style
  -> selected profile dialogue Style
  -> engine defaults
```

Scalar fields use nearest-wins semantics. Structured style records deep-merge by
field, so a character can override only ruby size while inheriting the selected
profile Style's ruby gap. Per-scene state, temporary line-plan variables, and
stage handles belong in flows, speaker presets, or line plans.

---

## RichText typography defaults

RichText typography is authored in a reusable Style and selected as the
profile's optional base Style. This keeps ruby, vertical writing, font choice,
wrapping, effects, and future typography parameters reusable by choices, View
text, logs, and HUD text.

```arcw
pub style dialogue_typography {
    .dialogue_content {
        font-family = text("Yu Gothic")
        font-size = 30px
        color = rgb("#f5f5f5")
        writing-mode = horizontal_tb
    }
}
```

```toml
[profiles.game.dialogue]
style = "style.dialogue_typography"
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
    view = @view.PhoneMessage,
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

## Built-in read/unread style policy

Common visual-novel read-state patterns are owned by the selected dialogue View
and its Style, with local character policy available when a speaker needs a
different treatment.

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

Custom behavior is expressed through character-local style policy or the
selected dialogue View. It is not installed as a global callback.

Built-in style policies:

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
 the surrounding line, speaker preset, character state, or selected profile must
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

## Local behavior inside dialogue text

`#[...]` is for safe expression/content insertion. Side-effecting local line behavior uses `[mark .name]` with `with: on mark(.name):` or a dialogue-safe `[call ...]`.

```arcw
alice.say()[
    まぶしい……[call flash(color=#ffffff, time=90ms)][p]
]
```

The mark handler may call an ordinary function:

```arcw
pub fn mark_keyword(
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

The scoped handler may read line context, speaker, dialogue View mount, read
state, locale, and reveal cursor. Durable state changes use the normal typed
event/command boundary.

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
4. A missing dialogue View target resolves to the standard authored View resource through normal linking.
5. `alice(options)` creates a lexical speaker preset; it does not display text until `:`, `[...]`, or `.say()[...]` is used.
6. Text interpolation uses `DisplayText` or explicit `fmt(...)`.
7. Runtime interpolation `#[expr]` is separate from localization placeholders `{name}`.
8. Dialogue text uses `[mark .name]` with line-plan `on mark(.name):` handlers or `[call ...]`; pure content insertion uses `#[...]`.
9. Characters define default text/name colors and may attach read/unread style hooks.
10. Character stage methods provide object-oriented sugar for show/face/move/animate/preload.
11. Character preload and memoization policies are explicit and observable by Agent Debug Bus.
```


