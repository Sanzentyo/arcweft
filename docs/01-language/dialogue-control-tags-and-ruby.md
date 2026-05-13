# Dialogue Control Tags, Ruby, Interpolation, and Inline Hooks

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

```awft
alice: おはよう。[p]

alice:
    おはよう。[l]
    今日はいい天気だね。[p]

地の文: 扉の向こうから、雨の音がした。[p]

alice.say(voice=auto)[
    今日は少しだけ、｜変な夢《へんなゆめ》を見たんだ。[p]
]
```

Only in those text regions, and in typed `fn(args)[content]` content blocks whose declared content type is dialogue/rich text, are `[...]`, `[/...]`, and `#[...]` interpreted as dialogue markup. In normal typed code, brackets keep their normal meaning.

---

## Tag families

Arcweft has four tag-like forms in dialogue text:

| Form | Purpose |
|---|---|
| `[p]`, `[l]`, `[r]` | short built-in control tags |
| `[ruby rt="..."]...[/ruby]` | enclosing rich-text/control tags |
| `#[expr]`, `#[fmt(...)]` | pure content interpolation |
| `[call ...]`, `[hook ...]` | dialogue-safe function or hook dispatch |

Double brackets are not dialogue tags:

```awft
/// [[flow.alice_intro]] is a documentation/RAG link, not a dialogue tag.
```

---

## Built-in reserved names

These names are reserved in tag position and scenario-command position. They cannot be used as unqualified custom tag names, unqualified scenario command names, character aliases, or local variables in dialogue tag scope.

A module may still define a qualified function such as `my_tags::p`, but it cannot be imported unqualified as `p`.

| Name | Meaning |
|---|---|
| `p` | page wait / advance and page-break request |
| `l` | line wait / advance without page clear |
| `r` | hard line break |
| `br` | alias of `r` |
| `w` | timed wait |
| `clear` | clear current message text |
| `er` | erase current message text, compatibility alias |
| `cm` | clear message layer, stronger compatibility alias |
| `ruby` | ruby annotation |
| `rt` | ruby text shorthand inside ruby-related tags |
| `em`, `strong` | emphasis spans |
| `color`, `font`, `size` | rich text styling spans |
| `speed` | text reveal speed control |
| `reset` | reset text style/reveal modifiers |
| `voice` | voice cue inside a line |
| `face`, `pose` | expression/pose change |
| `show`, `hide` | stage visibility cue |
| `move`, `scale`, `rotate` | transform cue |
| `anim`, `shake` | animation cue |
| `hook` | dispatch a declared hook |
| `at` | timed cue shorthand inside dialogue text |
| `call` | call an allowed dialogue function/tag |
| `signal` | emit/set a public signal if capability allows it |
| `if`, `else`, `endif` | local text conditional |
| `raw` | literal no-parse span |
| `fmt` | explicit DisplayText/content formatting function |

Project-specific aliases may map to these names, but canonical names remain reserved:

```toml
[dialogue.tags.aliases]
"改ページ" = "p"
"待機" = "l"
"改行" = "r"
```

---

## Wait and newline tags

```awft
alice: おはよう。[l]今日はいい天気だね。[p]
```

Meaning:

```text
[l]  wait for user advance; keep current page.
[p]  wait for user advance; request page break according to textbox policy.
```

Line break:

```awft
alice: 1行目[r]2行目[p]
```

Timed wait:

```awft
alice: えっと……[w time=500ms]なんでもない。[p]
```

`[p]` is not hard-coded to clear the box. It requests a page wait; the active TextBox theme decides whether to clear, scroll, animate, or continue.

```toml
[textbox.page]
default_policy = "wait_then_clear"
```

---

## Ruby

Arcweft supports three ruby forms.

### Natural Japanese ruby

```awft
alice: 今日は少しだけ、｜変な夢《へんなゆめ》を見たんだ。[p]
```

### Bracket tag ruby

```awft
alice: 今日は少しだけ、[ruby rt="へんなゆめ"]変な夢[/ruby]を見たんだ。[p]
```

### Function/content form

```awft
alice.say()[
    今日は少しだけ、#[ruby("変な夢", "へんなゆめ")]を見たんだ。[p]
]
```

All three forms normalize into the same `Content::Ruby { base, ruby }` fragment.

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

`#[expr]` inserts the formatted representation of `expr`. The expression must implement `DisplayText`.

```awft
narrator.say()[
    #[player_name]は鍵を手に入れた。[p]
]
```

If formatting needs options, use `fmt(...)` explicitly:

```awft
narrator.say()[
    スコアは#[fmt(score, style="number")]点です。[p]
]
```

The display trait is:

```awft
pub trait DisplayText {
    fn display_text(self, ctx: DisplayContext) -> Result<Content, DisplayError>
}
```

Built-in implementations include common scalar types, `String`, `LocalizedText`, `Ref<T>`, and selected wrappers. `Option<T>` must be explicitly handled or formatted with a fallback:

```awft
#[fmt(state.nickname, none="名無し")]
```

Pure interpolation cannot emit commands, mutate state, play audio, or trigger stage effects. Use `[call]`, `[hook]`, or line-plan `at(...) { ... }` for side-effecting dialogue behavior.

---

## Localization placeholders are separate

`#[expr]` is runtime expression interpolation. `{name}` is a localization placeholder.

```awft
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

## Dialogue-safe calls and hooks

Use `[call]` for a dialogue-safe function:

```awft
alice: まぶしい……[call flash(color=#ffffff, time=90ms)][p]
```

Use `[hook]` for a declared hook:

```awft
alice: [hook mark_keyword word="夢" color=#color.dream]変な夢[p]
```

A dialogue-safe function must declare its effects:

```awft
pub dialogue fn flash(
    color: Color = rgb("#ffffff"),
    time: Duration = 120ms,
) -> Result<DialogueCue, TagError>
effects { stage.flash }
{
    Ok(DialogueCue::Flash { color, time })
}
```

A custom hook:

```awft
pub dialogue hook #hook.dialogue.mark_keyword mark_keyword(
    word: String,
    color: Color,
) -> Result<DialogueCue, TagError>
{
    Ok(DialogueCue::StyleRange { word, color })
}
```

---

## Escaping special characters

Inside dialogue text mode, special characters can be escaped:

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
    [raw]これは[p]をタグとして解釈しない。[/raw]
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

## Color and style hooks

Character default colors are defined in the character declaration. Dialogue lines inherit them automatically.

```awft
pub character #character.alice Alice {
    dialogue_style {
        text_color = rgb("#f7d7ff")
        name_color = rgb("#e070ff")
        unread_text_color = rgb("#ffffff")
        read_text_color = rgb("#c8c8d0")
    }
}
```

Built-in read/unread hook:

```awft
pub dialogue defaults #dialogue.defaults {
    read_state_style = builtin.read_state_color(
        unread = rgb("#ffffff"),
        read = rgb("#b8b8c0"),
    )
}
```

Custom hook:

```awft
pub hook #hook.dialogue.read_color
on query DialogueLine
phase BeforeTextStyle
check on change line.read_state
{
    if line.read_state == .Read {
        line.style.text_color = rgb("#b8b8c0")
    }
}
```

---

## Tag parsing and scope

`[]` tags are parsed only in dialogue text mode. Tag arguments are parsed with the input-parser subsystem and return `Result<TagArgs, ParseError>`.

Values used by dialogue interpolation should be defined in the surrounding flow scope or inside a pure `#[...]` expression. Line-plan variables are for cues and cancellation, not for text interpolation that has already been parsed as content.

```awft
let emphasis_color = rgb("#a8b5ff")

alice.say()[
    #[fmt("夢", color=emphasis_color)]を見た。[p]
]
```

For cue-local values, use the line plan:

```awft
alice.say()[
    夢を見た。[p]
]
with {
    let flash_color = rgb("#a8b5ff")
    at(0.2s) { flash(color=flash_color)? }
}
```

Line-plan variables are not visible after the line finishes. `at(...) { ... }` creates an even smaller cue-local scope.
