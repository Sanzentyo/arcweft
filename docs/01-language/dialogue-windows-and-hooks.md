# Dialogue Windows, Character Styles, and Read-State Hooks

Arcweft treats the text display area as a first-class object. Ordinary dialogue uses a built-in global default window, but projects can define and address additional windows explicitly.

Related:

- [Flow-Integrated Scenario Syntax](scenario-surface-syntax.md)
- [Dialogue Character Methods, TextBox Targets, Interpolation, and Preload](dialogue-character-methods-and-textbox.md)
- [Dialogue Control Tags, Ruby, Inline Formatting, and Hooks](dialogue-control-tags-and-ruby.md)
- [Dialogue Calls, Line Plans, Cancellation, and Scoped Content Blocks](dialogue-calls-scopes-cancellation.md)
- [Character Stage / Sprite / Voice Timeline](../03-presentation/character-stage.md)
- [Hooks and Memoization](hooks-and-memoization.md)
- [Localization for Dialogue](localization-dialogue.md)

---

## Built-in default window

Arcweft prelude defines a global default dialogue textbox:

```arcw
pub textbox @textbox.main default_textbox {
    layer = @layer.view.dialogue
    position = bottom
    frame = rect(x=80, y=520, w=1120, h=160)
    page_policy = wait_then_clear
    text_reveal = typewriter(speed=normal)
}
```

When no window is specified, `@textbox.main` is used.

```arcw
alice.say()[おはよう。[p]]

alice: おはよう。[p]
```

Both update `@textbox.main` by default.

---

## Changing the project default

A project may replace the default window for all ordinary lines:

```arcw
pub textbox @textbox.main main_textbox {
    layer = @layer.view.dialogue
    position = bottom
    frame = rect(x=72, y=512, w=1136, h=174)
    page_policy = wait_then_scroll
}

dialogue defaults {
    window = @textbox.main
}
```

After this, speaker syntax uses `@textbox.main`:

```arcw
alice: ここは main_textbox に出る。[p]
```

---

## Addressing another window

Text windows can be addressed explicitly:

```arcw
pub textbox @textbox.side side_note {
    layer = @layer.view.overlay
    position = right
    frame = rect(x=900, y=80, w=360, h=240)
    page_policy = append
}
```

Use it from a line:

```arcw
narrator.say(window=@textbox.side)[
    右側の注釈ウィンドウに出る。[p]
]
```

Or use object style:

```arcw
@<textbox.side>.append()[
    注釈を追加する。
]

@<textbox.side>.clear()
```

Textbox methods are typed and effect-checked:

```arcw
pub trait TextboxObject {
    fn append(self: Ref<Textbox>) -> TextboxContentCall
    fn clear(self: Ref<Textbox>) -> Command
    fn set_theme(self: Ref<Textbox>, theme: Ref<TextboxTheme>) -> Command
    fn set_visible(self: Ref<Textbox>, visible: bool) -> Command
}
```

---

## Character styles

Character definitions may include text colors and nameplate styles. This keeps ordinary dialogue concise.

```arcw
pub character @character.alice alice {
    display_name ja-JP = "アリス"
    display_name en-US = "Alice"

    text_style {
        name_color = rgb("#ffb7d5")
        text_color = rgb("#f7e8ff")
        unread_color = rgb("#ffffff")
        read_color = rgb("#b8b8c8")
        emphasis_color = rgb("#ffd1e6")
    }
}
```

Then:

```arcw
alice: おはよう。[p]
```

uses Alice's default dialogue style.

Narration uses the built-in narrator style:

```arcw
地の文: 扉の向こうから、雨の音がした。[p]
```

---

## Global dialogue defaults

The project may declare default hooks, reveal behavior, voice behavior, and text-textbox behavior.

```arcw
dialogue defaults {
    window = @textbox.main
    reveal = typewriter(speed=normal)
    voice = auto_if_available

    hooks {
        before_text_resolve += @hook.dialogue.locale_text_substitution
        before_text_style += @hook.dialogue.read_state_color
        before_voice_resolve += @hook.dialogue.auto_voice_key
        after_line_complete += @hook.dialogue.mark_line_read
    }
}
```

Line-specific settings override defaults:

```arcw
alice.say(window=@textbox.side, reveal=instant)[
    ここだけ即時表示。[p]
]
```

---

## Read-state style hooks

A common pattern is changing line color depending on whether the line has been read.

Built-in hook:

```arcw
@hook.dialogue.read_state_color
```

Conceptual behavior:

```arcw
hook @hook.dialogue.read_state_color
on query DialogueLine
phase before_text_style
{
    match ctx.line.read_state {
        .Unread => DialoguePatch.Style { color = ctx.character.text_style.unread_color }
        .Read   => DialoguePatch.Style { color = ctx.character.text_style.read_color }
    }
}
```

Use globally:

```arcw
dialogue defaults {
    hooks {
        before_text_style += @hook.dialogue.read_state_color
    }
}
```

Use locally:

```arcw
alice.say(hooks=[@hook.dialogue.read_state_color])[
    この行だけ既読色フックを明示する。[p]
]
```

---

## Dialogue hook context

Dialogue hooks receive a typed context.

```arcw
pub struct DialogueHookCtx {
    line: DialogueLineInfo
    character: CharacterInfo
    textbox: Ref<Textbox>
    locale: Locale
    read_state: ReadState
    text: RichText
    style: TextStyle
    voice: Option<VoiceCue>
    state_hash: StateHash
}
```

A hook returns either no change or a patch:

```arcw
pub enum DialoguePatch {
    None,
    ReplaceText(RichText),
    Style(TextStylePatch),
    Voice(Option<VoiceCue>),
    Window(Ref<Textbox>),
    Cues(Vec<DialogueCue>),
}
```

Hooks are deterministic unless declared otherwise. Product builds may disable non-deterministic debug hooks.

---

## Textbox contracts

Text windows can have contracts.

```arcw
pub textbox @textbox.main default_textbox
ensures layout.width > 0
ensures layout.height > 0
ensures agent_observable == true
{
    ...
}
```

Line calls can assert that the active textbox is available:

```arcw
alice.say(window=@textbox.side)
requires textbox.visible == true
[
    side textbox text[p]
]
```

---

## Agent observation

Text windows expose:

```text
- textbox entity ID
- current speaker
- current line ID
- text key
- visible text range
- full rich text
- bbox / polygon / mask
- reveal cursor
- read state
- active hooks
- pending voice cue
```

This lets LLM debuggers inspect whether a line is visible, partially revealed, read/unread, or blocked by a wait tag.



