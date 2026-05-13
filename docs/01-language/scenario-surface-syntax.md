# Flow-Integrated Scenario Syntax / Dialogue Sugar

Arcweft does not define a separate `script` item. Ordinary visual-novel writing is part of the `flow` grammar itself.

A `flow` body may mix:

- compact scenario statements such as `@bg`, `@show`, `alice:`, choices, and dialogue tags;
- canonical character method calls such as `alice.say()[ ... ]` and `alice.move(...)`;
- typed Arcweft statements such as `let`, `match`, `await ... with`, `Result`, contracts, and function calls.

The concise dialogue surface is parsed as normal `FlowItem` syntax. There is no `script` language and no script-lowering phase.

Related:

- [Dialogue Character Methods, TextBox Targets, Interpolation, and Preload](dialogue-character-methods-and-textbox.md)
- [Dialogue Control Tags, Ruby, Inline Formatting, and Hooks](dialogue-control-tags-and-ruby.md)
- [Dialogue Calls, Line Plans, Cancellation, and Scoped Content Blocks](dialogue-calls-scopes-cancellation.md)
- [Dialogue Content Calls, `with` Blocks, Line Output Values, and Scoped Handles](dialogue-line-handles-and-returns.md)
- [Localization for Dialogue](localization-dialogue.md)
- [Character Stage / Sprite / Voice Timeline](../03-presentation/character-stage.md)
- [ID と参照](ids-and-references.md)
- [Hooks and Memoization](hooks-and-memoization.md)

---

## Design goals

Arcweft should be close in writing cost to compact visual-novel formats while keeping type checking, stable IDs, voice synchronization, localization, contracts, hooks, and Agent observability.

```text
Simple conversation stays simple.
Complex staging expands only where needed.
There is one `flow` entry-point model.
```

---

## Canonical dialogue call form

The canonical dialogue form is a character method call with a content block:

```awft
alice.say()[
    おはよう。[p]
]
```

With common options:

```awft
alice.say(
    id = #say.opening.greeting,
    voice = auto,
    face = smile,
    window = #textbox.0,
)[
    おはよう。[p]
]
```

`alice` is a character alias in scope. If a character is referenced directly by entity ID, use a delimited reference or parentheses before method access:

```awft
#<character.alice>.say(voice=auto)[
    おはよう。[p]
]

(#character.alice).say(voice=auto)[
    おはよう。[p]
]
```

---

## `:` speaker syntax is sugar

The compact form:

```awft
alice: おはよう。[p]
```

is sugar for:

```awft
alice.say()[
    おはよう。[p]
]
```

Options are written in parentheses and are the same as `say()` options:

```awft
alice(id=#say.opening.greeting, face=smile, voice=auto):
    おはよう。[p]
```

is sugar for:

```awft
alice.say(
    id = #say.opening.greeting,
    face = smile,
    voice = auto,
)[
    おはよう。[p]
]
```

`#` remains an entity-reference marker, not an option marker. The older `alice #say... @smile voice auto:` style is not part of the stable grammar and may be formatter-migrated while early tooling still recognizes it.

The long method form is preferred when the line has a custom window, timed cues, cancellation, local variables, or custom hooks.

---

## Minimal dialogue flow

```awft
mod game::routes::opening

use game::prelude::*
use game::characters::{alice}

pub flow #flow.opening opening(state: GameState) -> Result<FlowExit, FlowError> {
    @bg #asset.bg.room fade=300ms
    @show alice normal at=center fade=200ms

    地の文: 扉の向こうから、雨の音がした。[p]
    alice: おはよう。[l]
    alice(voice=auto): 今日は少しだけ、｜変な夢《へんなゆめ》を見たんだ。[p]

    @choice #choice.opening.first {
        #choice.opening.listen "聞いてみる" -> #flow.alice_intro
        #choice.opening.silent "黙っている" -> #flow.quiet_intro
    }
}
```

This is a typed `flow`. The speaker lines and scenario commands are simply concise `FlowItem` forms.


---

## Dialogue call sugar

The canonical detailed dialogue shape is an explicit character call. Arcweft also accepts bracket shorthand when the callee is a speaker, character reference, or speaker preset:

```awft
alice[
    おはよう。[p]
]
```

This is sugar for `alice.say()[...]`. A line plan can be attached with canonical `with { ... }` or indentation sugar `with:`.

```awft
alice[
    おはよう。[p]
]
with:
    at(0.42s):
        alice.stage.face(smile)
```

The same line-plan attachment works with colon syntax:

```awft
alice:
    おはよう。[p]
with:
    at(0.42s):
        alice.stage.face(smile)
```

Here `alice:` lowers to the same dialogue call as `alice.say()[...]`, and `with:` lowers to `with { ... }`.

Colon lowering keeps speaker presets callable:

```text
alice: text
  -> alice.say()[text]

alice(voice=auto): text
  -> alice.say(voice=auto)[text]

alice2(voice=auto): text
  -> alice2(voice=auto)[text]
```

Use the canonical method form when the line exports handles or values:

```awft
let (actor, voice) = alice.say(voice=auto)[
    聞いて。[p]
]
with:
    let actor = alice.stage.acquire(scope=line)
    let voice = line.voice_handle()
    out (actor, voice)
```

---

## No separate script item

The `script` keyword is not part of Arcweft's scenario grammar.

```awft
pub flow #flow.opening opening(state: GameState) -> Result<FlowExit, FlowError> {
    alice: おはよう。[p]
}
```

Reusable scenario snippets use `fragment`:

```awft
pub fragment #frag.alice_enters: FlowFragment {
    @show alice normal at=right fade=220ms
    @move alice to=center time=300ms ease=cubic.out
}

pub flow #flow.opening opening(state: GameState) -> Result<FlowExit, FlowError> {
    include #frag.alice_enters
    alice: おはよう。[p]
}
```

`fragment` is typed flow content, not a second script language.

---

## Speaker line forms

```awft
alice: おはよう。

alice(face=smile): おはよう。

alice(voice=#voice.alice.001): おはよう。

alice(face=smile, voice=auto): おはよう。

alice(id=#say.opening.001, face=smile, voice=#voice.alice.001):
    おはよう。
```

Meaning:

| Form | Meaning |
|---|---|
| `alice:` | speaker is `#character.alice`; line ID, text key, voice, and window are inferred |
| `face=smile` | expression cue before text display |
| `voice=#voice...` | explicit voice binding |
| `voice=auto` | derive voice cue from line ID, locale, and speaker |
| `id=#say...` | explicit line entity ID |

The implicit window is `#textbox.0` unless the character, line, or project defaults override it.

Speaker presets are allowed in the same position:

```awft
let alice2 = alice(face=smile, voice=auto, window=#textbox.side)

alice2: おはよう。[p]

alice2(id=#say.opening.side_001):
    こっちのウィンドウで話すね。[p]
```

Here `alice2` is a lexical speaker preset. It is not a new character and it does not mutate `#character.alice`.

---

## Dialogue-text mode

Bracket control tags are special only in dialogue text mode.

Dialogue text mode begins only in these places:

```awft
alice: inline dialogue text here

alice:
    indented dialogue text here

地の文: narration text here

narrator:
    narration text here

alice.say()[
    dialogue text here
]
```

Inside dialogue text mode, Arcweft recognizes:

```text
[p]      page wait
[l]      line wait
[r]      hard line break
[ruby]   ruby annotation
#[...]   embedded Arcweft expression/content, requiring DisplayText or fmt(...)
[call]   dialogue-safe function call
[hook]   declared dialogue hook dispatch
```

Outside dialogue text mode, `[...]` is not treated as a dialogue control tag. It remains normal Arcweft syntax such as indexing, lists, attributes, or parser-specific syntax.

---

## Built-in narrator character

Arcweft prelude defines a built-in narrator-like character:

```awft
pub character #character.narrator narrator {
    role = narration
    nameplate = hidden
    localizable_name = false
    dialogue_style {
        textbox = #textbox.narrator
    }
}
```

Default aliases:

```text
narrator
地の文
地
```

All of these are equivalent by default:

```awft
narrator: 扉の向こうから、雨の音がした。[p]
地の文: 扉の向こうから、雨の音がした。[p]
地: 扉の向こうから、雨の音がした。[p]
```

The theme decides whether narration uses no nameplate, a separate narration box, or a special typographic style.

Project configuration can disable Japanese aliases or rename them:

```toml
[dialogue.narrator]
entity = "character.narrator"
aliases = ["narrator", "地の文", "地"]
nameplate = "hidden"
textbox = "textbox.narrator"
```

---

## Compact staging commands in flow bodies

Scenario commands are line-oriented `FlowItem`s. They are available inside `flow` and `fragment` bodies.

```awft
@bg #asset.bg.school_evening fade=600ms
@show alice normal at=center layer=characters fade=200ms
@face alice smile crossfade=120ms
@move alice to=left time=350ms ease=cubic.out
@anim alice #anim.breath loop
@hide alice fade=180ms
```

These are sugar over character/stage methods:

```awft
alice.show(expression=normal, at=center, layer=#layer.characters, fade=200ms)
alice.face(smile, crossfade=120ms)
alice.move(to=left, time=350ms, ease=cubic.out)
alice.animate(#anim.breath, mode=loop)
alice.hide(fade=180ms)
```

---

## Complex line with method form

```awft
alice.say(id=#say.opening.dream_hint, voice=auto, face=smile)[
    今日は少しだけ、#[fmt("変な夢", color=rgb("#a8b5ff"))]を見たんだ。[p]
]
with {
    reveal = voice
    cancel on input .SkipLine => continue
    cancel on input .BackToTitle => goto #flow.title

    at(0.42s) { alice.stage.face(worried, crossfade=120ms) }
    at(end-250ms) { alice.stage.animate(#anim.breath.once) }
}
```

Colon sugar can attach the same line plan with `with { ... }`:

```awft
alice(id=#say.opening.dream_hint, voice=auto, face=smile):
    今日は少しだけ、#[fmt("変な夢", color=rgb("#a8b5ff"))]を見たんだ。[p]
with {
    at(0.42s) { alice.stage.face(worried, crossfade=120ms) }
}
```

The line plan and `at(...) { ... }` blocks create scoped variables. See [Dialogue Calls, Line Plans, Cancellation, and Scoped Content Blocks](dialogue-calls-scopes-cancellation.md).
