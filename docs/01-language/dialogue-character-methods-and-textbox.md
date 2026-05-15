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

```awft
alice.say()[
    おはよう。[p]
]
```

With options:

```awft
alice.say(
    id = @say.opening.greeting,
    voice = auto,
    look = smile,
    window = @textbox.0,
)[
    おはよう。[p]
]
```

Fully qualified entity references are allowed. Because `@character.alice.say` would be ambiguous as a single entity path, use `@<...>` or parentheses when calling a method on an entity reference:

```awft
@<character.alice>.say(voice=auto)[
    おはよう。[p]
]

(@character.alice).say(voice=auto)[
    おはよう。[p]
]
```

The short `alice` form is a speaker alias resolved from the module's character imports or prelude:

```awft
use game::characters::{alice, bob}
```

---

## `:` syntax sugar

For a character alias, the compact speaker-prefix form is sugar for
`Character.say(...)[...]`. For a `SpeakerPreset`, the same surface syntax calls
the preset and applies the dialogue content without forcing a character
`.say(...)` rewrite.

```awft
alice: おはよう。[p]
```

is equivalent to:

```awft
alice.say()[
    おはよう。[p]
]
```

Options in parentheses become `say()` options:

```awft
alice(id=@say.opening.greeting, look=smile, voice=auto):
    おはよう。[p]
```

is equivalent to:

```awft
alice.say(
    id = @say.opening.greeting,
    look = smile,
    voice = auto,
)[
    おはよう。[p]
]
```

`@` is the entity-reference marker. It is not used as a special option list marker.

The canonical expression/portrait option is `look`. The first positional option is shorthand for `look`, so `alice(smile, voice=auto):` and `alice(look=smile, voice=auto):` are equivalent. `face` is not a line option; stage APIs may still use methods such as `alice.stage.face(...)` for model parts.

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

```awft
地の文: 扉の向こうから、雨の音がした。[p]
```

is equivalent to:

```awft
@<character.narrator>.say()[
    扉の向こうから、雨の音がした。[p]
]
```

`:` is only sugar. The parser creates the same `DialogueLine` item as the canonical call form. There is no separate `script` language and no `script` lowering phase.


---

## Bracket content call

A character alias or speaker preset may be called directly with a dialogue
content block.

```awft
alice[
    おはよう。[p]
]
```

For a character alias, this is sugar for:

```awft
alice.say()[
    おはよう。[p]
]
```

Options still use the parenthesized speaker-preset call or the canonical
`.say(...)` call.

```awft
alice(look=smile, voice=auto)[
    おはよう。[p]
]

alice.say(look=smile, voice=auto)[
    おはよう。[p]
]
```

When a line plan is needed, attach canonical `with { ... }` or indentation sugar `with:`.

```awft
alice(look=smile, voice=auto)[
    おはよう。[p]
]
with:
    at(0.42s):
        alice.stage.face(worried)
```

This form is especially convenient for tool-generated dialogue because the content block and line plan are visually separated.

---

## Speaker presets and curried line options

A character alias is callable. Calling a character with line options does not
display text immediately; it returns a **speaker preset** that carries default
line options.

```awft
let alice2 = alice(look=smile, voice=auto, window=@textbox.side)

alice2: おはよう。[p]

alice2(id=@say.opening.side_001):
    こっちのウィンドウで話すね。[p]
```

This is equivalent to:

```awft
alice(look=smile, voice=auto, window=@textbox.side)[
    おはよう。[p]
]

alice2(id=@say.opening.side_001)[
    こっちのウィンドウで話すね。[p]
]
```

Presets can be refined by calling them again. Later options override earlier options.

```awft
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
per-line options
  -> speaker preset options
  -> character dialogue_style
  -> dialogue window theme
  -> global dialogue defaults
  -> engine defaults
```

Types:

```awft
pub type Speaker = Ref<Character> | SpeakerPreset

pub fn Character.call(self, options: SayOptions) -> SpeakerPreset
pub fn SpeakerPreset.call(self, options: SayOptions) -> SpeakerPreset
pub fn SpeakerPreset.say(self, options: SayOptions = {}) -> DialogueContentCall
```

The `:` sugar accepts both `Ref<Character>` and `SpeakerPreset`.

```awft
let phone_alice = alice(window=@textbox.phone_message, voice=auto)
phone_alice: スマホに通知が届いた。[p]
```

The preset is lexical. It can be local to a block, flow, or helper function. It does not mutate the character definition.

---

## Object-like stage handles

Characters expose object-like stage APIs. The speaker alias remains a pure reference, while `stage` methods create handles for currently displayed presentation objects.

```awft
let actor = alice.stage.acquire(scope=line)
let pose = actor.pose(normal)
let face = actor.face(smile)
```

These handles can be memoized or preloaded:

```awft
let actor = memo(scope=scene, key=(@character.alice, pose=normal, theme=env.theme.hash)) {
    alice.stage.acquire(scope=line)
}
```

A `preload next` block explicitly prepares assets for a future flow.

```awft
preload next @flow.alice_intro:
    alice.stage.prefetch(pose=normal, faces=[smile, worried], window=@textbox.0)
    alice.voice_for(@say.alice_intro.001).preload()
    bgm.prepare(@bgm.alice_theme)
```

Preload is a hint, not a hidden blocking operation. If the resource is not ready when used, the normal `Need<T, E>` pending rules still apply.

---

## Dialogue window target

A dialogue line always targets a text window. If no window is specified, Arcweft uses the global default dialogue window.

```awft
alice.say()[おはよう。[p]]
```

is equivalent to:

```awft
alice.say(window=@textbox.0)[おはよう。[p]]
```

Built-in textboxes:

| ID | Meaning |
|---|---|
| `@textbox.0` | Default main dialogue textbox |
| `@textbox.main` | Alias of `@textbox.0` |
| `@textbox.narrator` | Optional narration textbox; defaults to `@textbox.0` unless configured |
| `@textbox.system` | System messages / debug messages |

Project default:

```toml
[dialogue.default_window]
main = "textbox.0"
narrator = "textbox.narrator"
missing = "textbox.0"
```

Custom dialogue windows can be declared as UI components or text surfaces:

```awft
pub textbox @textbox.phone_message PhoneMessageBox {
    layer = @layer.ui.messages
    anchor = bottom_right
    width = 420
    style = @style.phone_message
}
```

Use it from dialogue:

```awft
alice.say(window=@textbox.phone_message, voice=auto)[
    スマホに通知が届いた。[p]
]
```

The canonical parameter name is `window`. The legacy `textbox` option name is not valid Arcweft source; `textbox` remains the entity kind for dialogue window objects.

A dialogue window target is a stateful UI object. Updating a line in that textbox affects the selected window only. Agent observation exposes the target window, current line ID, visible text, reveal cursor, and actionable wait state.

---

## Character text style and color

Characters can define default dialogue style, nameplate style, and voice/text policies.

```awft
pub character @character.alice Alice {
    display_name ja-JP = "アリス"
    display_name en-US = "Alice"

    dialogue_style {
        text_color = rgb("#f7d7ff")
        name_color = rgb("#e070ff")
        unread_text_color = rgb("#ffffff")
        read_text_color = rgb("#c8c8d0")
        window = @textbox.0
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
line options
  -> character dialogue_style
  -> dialogue window theme
  -> global dialogue defaults
  -> engine defaults
```

Example:

```awft
alice.say(color=rgb("#ff8080"))[
    この行だけ赤い。[p]
]
```

---

## Built-in read/unread style hooks

Common visual-novel patterns are built in.

```awft
pub dialogue defaults @dialogue.defaults {
    window = @textbox.0
    read_state_style = builtin.read_state_color(
        unread = rgb("#ffffff"),
        read = rgb("#b8b8c0"),
    )
    auto_mark_read = on_page_advance
}
```

Character-level override:

```awft
pub character @character.alice Alice {
    dialogue_style {
        read_state_style = builtin.read_state_color(
            unread = rgb("#ffeaff"),
            read = rgb("#d0b8d8"),
        )
    }
}
```

A custom hook can override or extend this behavior:

```awft
pub hook @hook.dialogue.alice_read_color
on query DialogueLine where line.speaker == @character.alice
phase BeforeTextStyle
check on change line.read_state
{
    if line.read_state == .Read {
        line.style.text_color = rgb("#c5b6cc")
    }
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

```awft
narrator.say()[
    #[player_name]は鍵を手に入れた。[p]
]
```

This requires:

```awft
player_name: DisplayText
```

Explicit formatting:

```awft
narrator.say()[
    スコアは#[fmt(score, style="number")]点です。[p]
]
```

Formatting with locale:

```awft
narrator.say()[
    所持金: #[fmt(money, currency="JPY", locale=state.locale)][p]
]
```

Trait:

```awft
pub trait DisplayText {
    fn display_text(self, ctx: DisplayContext) -> Result<Content, DisplayError>
}
```

Built-in implementations:

```text
String, LocalizedText, i32, u32, f32, bool, Duration, DateTime, Ref<T>, Option<T>, Result<T,E>
```

`Option<T>` is displayed only if explicitly formatted or matched. This avoids accidentally showing `None` in player-facing text.

```awft
match state.player_nickname {
    Some(nick) => alice.say()[#[nick]、おはよう。[p]]
    None => alice.say()[おはよう。[p]]
}
```

`fmt(...)` can also wrap values into `Content` for hooks and custom tags:

```awft
#[fmt(route_title(state.route), color=@color.accent)]
```

### Interpolation vs localization placeholders

`#[expr]` is runtime interpolation. `{name}` inside rich text is a localization placeholder.

```awft
narrator.say()[
    {player_name}は鍵を手に入れた。[p]
]
```

The above is extracted as a localizable string with placeholder `player_name`. At runtime the placeholder must be supplied by the line options or surrounding context:

```awft
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

```awft
alice.say()[
    [raw]ここでは[p]も#[expr]も解釈されない。[/raw]
]
```

Raw block:

```awft
alice.say()[
    [raw]
    ここでは複数行にわたりタグを解釈しない。
    [p] も文字として表示する。
    [/raw]
]
```

---

## Hooks inside dialogue text

`#[...]` is for safe expression/content insertion. Side-effecting local line behavior uses `[mark .name]` with `with: on .name:` or a dialogue-safe `[call ...]`.

```awft
alice.say()[
    まぶしい……[call flash(color=#ffffff, time=90ms)][p]
]
```

Custom hook:

```awft
pub dialogue hook @hook.dialogue.mark_keyword mark_keyword(
    word: String,
    color: Color = rgb("#ffcc00"),
) -> Result<DialogueCue, TagError>
{
    Ok(DialogueCue::StyleRange { word, color })
}
```

Use:

```awft
alice.say()[
    変な夢[mark .keyword][p]
with:
    on .keyword:
        mark_keyword(word="夢", color=@color.dream)
]
```

Hooks may read line context, speaker, dialogue window, read state, locale, and reveal cursor. They may not mutate global state unless explicitly declared with capability.

---

## Scoped line blocks

`Character.say(...)[...]`, `with { ... }`, `with:`, and `at(...) { ... }` create lexical scopes. `with:` normalizes to `with { ... }`.

```awft
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

```awft
alice.stage.show(smile, at=center, fade=200ms)
alice.stage.face(worried, crossfade=120ms)
alice.stage.move(to=left, time=300ms, ease=cubic.out)
alice.stage.scale(1.05, time=200ms)
alice.stage.hide(fade=180ms)
```

When the instance matters:

```awft
alice.stage(@stage.alice.main).move(to=left, time=300ms)
```

Fully qualified:

```awft
@<character.alice>.stage(@stage.alice.main).face(worried)
```

Character staging uses ordinary effectful calls and methods. There is no
separate `@show`, `@face`, or `@move` command family.

```awft
show(@character.alice, .smile, at = .center, fade = 200ms)
```

is sugar for:

```awft
alice.stage.show(smile, at=center, fade=200ms)
```

---

## Character preload and memoization

Characters own preload and memoization policies for sprites, expressions, mouth parts, voice metadata, and composed render layers.

```awft
pub character @character.alice Alice {
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

```awft
preload character alice {
    expressions [normal, smile, worried]
    voices for flow @flow.alice_intro locale current
    sprites scale_buckets [1.0, 1.25]
}
```

Preload for likely next flow:

```awft
anticipate @flow.alice_intro {
    alice.preload(expressions=[smile, worried], voices=auto, sprites=true)
    asset.preload(@asset.bg.room_evening)
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
4. A missing dialogue window target resolves to `@textbox.0`.
5. `alice(options)` creates a lexical speaker preset; it does not display text until `:`, `[...]`, or `.say()[...]` is used.
6. Text interpolation uses `DisplayText` or explicit `fmt(...)`.
7. Runtime interpolation `#[expr]` is separate from localization placeholders `{name}`.
8. Dialogue text uses `[mark .name]` with line-plan `on .name:` handlers or `[call ...]`; pure content insertion uses `#[...]`.
9. Characters define default text/name colors and may attach read/unread style hooks.
10. Character stage methods provide object-oriented sugar for show/face/move/animate/preload.
11. Character preload and memoization policies are explicit and observable by Agent Debug Bus.
```

