# Flow-Integrated Scenario Syntax / Dialogue Sugar

Arcweft does not define a separate `script` item. Ordinary visual-novel writing is part of the `flow` grammar itself.

A `flow` body may mix:

- ordinary effectful function calls such as `bg(...)` and `show(...)`;
- compact dialogue statements such as `alice:`, choices, and dialogue tags;
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
    id = @say.opening.greeting,
    voice = auto,
    look = .smile,
    window = @textbox.side,
)[
    おはよう。[p]
]
```

`alice` is a character alias in scope. If a character is referenced directly by entity ID, use a delimited reference or parentheses before method access:

```awft
@<character.alice>.say(voice=auto)[
    おはよう。[p]
]

(@character.alice).say(voice=auto)[
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
alice(id=@say.opening.greeting, look=.smile, voice=auto):
    おはよう。[p]
```

is sugar for:

```awft
alice.say(
    id = @say.opening.greeting,
    look = .smile,
    voice = auto,
)[
    おはよう。[p]
]
```

`@` is the entity-reference marker. `#` is reserved for Rust-like attributes
such as `#[derive(...)]`. Older compact option styles without parentheses are
not part of the stable grammar.

Line IDs may also be relative in the `id` option. Relative line IDs are resolved
using the current flow, speaker, and named-scope path.

```awft
scope rain {
    alice(id=@.comment, voice=auto):
        雨、強くなってきたね。[p]
}
```

```text
alice(id=@.comment)
  -> @say.opening.alice.rain.comment
  -> @text.opening.alice.rain.comment
  -> @voice.ja-JP.alice.opening.rain.comment
```

The long method form is preferred when the line has a custom window, timed cues, cancellation, local variables, or custom hooks.

---

## Minimal dialogue flow

```awft
mod crate::game::routes::opening

use crate::game::prelude::*
use self::characters::{alice}

pub flow @flow.opening opening(state: GameState) -> Result<FlowExit, FlowError> {
    bg(@asset.bg.room, fade = 300ms)
    show(@character.alice, .normal, at = .center, fade = 200ms)

    scope rain {
        地の文(id=@.sound):
            扉の向こうから、雨の音がした。[p]

        alice(id=@.comment, voice=auto):
            雨、強くなってきたね。[p]
    }

    scope dream {
        choice @.first {
            @.listen "聞いてみる" -> @flow.alice_intro
            @.silent "黙っている" -> @flow.quiet_intro
        }
    }
}
```

This is a typed `flow`. Speaker lines are concise `FlowItem` forms, while
stage operations are normal effectful calls. The `scope` names are lexical
scopes and ID namespaces, so `@.sound`, `@.comment`, `@.first`, `@.listen`, and
`@.silent` normalize to stable fully qualified IDs.

Named scopes make compact source IDs practical while keeping registry IDs
stable and fully qualified.

```awft
scope dream {
    let can_enter = {
        let affection_ok = state.affection[@character.alice] >= 3
        affection_ok
    }

    choice @.first {
        @.listen "聞いてみる" if can_enter -> @flow.alice_intro
        @.silent "黙っている" -> @flow.quiet_intro
    }
}
```

The relative IDs normalize as follows:

```text
@say.opening.narrator.rain.sound
@text.opening.narrator.rain.sound
@say.opening.alice.rain.comment
@text.opening.alice.rain.comment
@choice.opening.dream.first
@choice.opening.dream.first.listen
@text.choice.opening.dream.first.listen
```

If a relative line or choice ID appears outside a named scope, the scope segment
is omitted:

```text
alice(id=@.greeting) -> @say.opening.alice.greeting
choice @.first       -> @choice.opening.first
```


---

## Choice option state and value form

The compact arm form is sugar. `->` advances the flow with `goto`; `=>` outputs a value from the choice expression.

```awft
let next_flow = choice @choice.opening.first {
    @choice.opening.listen "聞いてみる" => @flow.alice_intro
    @choice.opening.silent "黙っている" => @flow.quiet_intro
}

goto next_flow
```

Use full `option` blocks when the UI needs visible/enabled state, disabled reasons, badges, hotkeys, or a multi-statement selected action.

```awft
let can_enter_alice = state.affection[@character.alice] >= 3

choice @choice.opening.first {
    option @choice.opening.listen {
        label = "聞いてみる"
        enabled = can_enter_alice
        visible = true
        order = 10

        ui {
            disabled_reason = if can_enter_alice { None } else { Some("アリスの好感度が足りません") }
            badge = if can_enter_alice { None } else { Some("LOCKED") }
            style = if can_enter_alice { @style.choice.normal } else { @style.choice.locked }
        }

        select {
            event.emit(GameEvent::ChoiceSelected, id = @choice.opening.listen)
            goto @flow.alice_intro
        }
    }

    @choice.opening.silent "黙っている" -> @flow.quiet_intro
}
```

Inline arm `if` is enabled-state sugar. A block `if` controls whether an option exists at all.

```awft
choice @choice.opening.first {
    if state.flags.contains(.alice_route_discovered) {
        @choice.opening.listen "聞いてみる" -> @flow.alice_intro
    }

    @choice.opening.silent "黙っている" -> @flow.quiet_intro
}
```

Dynamic options use ordinary `for` over lists, sequences, or sorted map entries.

```awft
choice @choice.opening.routes {
    for route in opening_routes(state) {
        option route.choice_id {
            label = route.label
            enabled = route.enabled

            ui {
                disabled_reason = route.disabled_reason
                badge = route.badge
            }

            select {
                goto route.target
            }
        }
    }
}
```

`choice ... with { ... }` attaches a choice lifecycle plan. `with:` is indentation sugar, as with dialogue line plans.

```awft
choice @choice.opening.first {
    @choice.opening.listen "聞いてみる" -> @flow.alice_intro
    @choice.opening.silent "黙っている" -> @flow.quiet_intro
}
with {
    window = @choice_window.main
    layout = vertical
    default_focus = @choice.opening.listen

    timeout 10s {
        select @choice.opening.silent
    }

    cancel on input .BackToTitle {
        return Ok(FlowExit::Goto(@flow.title))
    }

    on select selected {
        log.info("selected choice {id:?}", id = selected.id)
    }
}
```

Choice execution is defined in terms of a candidate-option plan, not as a
one-shot list of strings:

1. Enter the choice body's lexical scope.
2. Evaluate local `let`, block `if`, `match`, and `for` items to build option candidates.
3. Evaluate each candidate's `visible`, `enabled`, `order`, `hotkey`, and `ui { ... }` state.
4. Send visible options to the choice UI, accessibility layer, tests, and Agent observation.
5. Suspend the flow while waiting for player, Agent, test, timeout, or cancel input.
6. Re-evaluate dependent option state when tracked state/signals change.
7. Run choice-level `on select selected` handlers.
8. Run the selected option's `select { ... }` block.
9. Continue according to `goto`, `return`, `out`, or normal completion.

Dynamic labels are not automatically localization extraction targets. If a
dynamic option should be localizable, its `label` expression should evaluate to
localized text, a text key, or rich text carrying localization identity. A plain
runtime `String` is displayable, but tools should warn when it is used where a
localizable choice label is expected.

```awft
choice @choice.opening.routes {
    option route in opening_routes(state) {
        id = route.choice_id
        label(id=@text.choice.opening.route) = route.label
        value = route.target
        enabled = route.enabled

        ui {
            disabled_reason = route.disabled_reason
            badge = route.badge
        }

        select {
            out route.target
        }
    }
}
```

Relative choice IDs are resolved from the current flow and named scope path.
Relative option IDs are resolved under the current choice ID.

```awft
scope dream {
    choice @.first {
        @.listen "聞いてみる" -> @flow.alice_intro
        @.silent "黙っている" -> @flow.quiet_intro
    }
}
```

```text
choice @.first -> @choice.opening.dream.first
@.listen       -> @choice.opening.dream.first.listen
@.silent       -> @choice.opening.dream.first.silent
```

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
        alice.stage.look(smile)
```

The same line-plan attachment works with colon syntax:

```awft
alice:
    おはよう。[p]
with:
    at(0.42s):
        alice.stage.look(smile)
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
pub flow @flow.opening opening(state: GameState) -> Result<FlowExit, FlowError> {
    alice: おはよう。[p]
}
```

Reusable scenario snippets use `fragment`:

```awft
pub fragment @frag.alice_enters: FlowFragment {
    show(@character.alice, .normal, at = .right, fade = 220ms)
    move(@character.alice, to = .center, time = 300ms, ease = cubic.out)
}

pub flow @flow.opening opening(state: GameState) -> Result<FlowExit, FlowError> {
    include @frag.alice_enters
    alice: おはよう。[p]
}
```

`fragment` is typed flow content, not a second script language.

---

## Speaker line forms

```awft
alice: おはよう。

alice(look=.smile): おはよう。

alice(voice=@voice.alice.001): おはよう。

alice(look=.smile, voice=auto): おはよう。

alice(id=@say.opening.001, look=.smile, voice=@voice.alice.001):
    おはよう。
```

Meaning:

| Form | Meaning |
|---|---|
| `alice:` | speaker is `@character.alice`; line ID, text key, voice, and window are inferred |
| `look=.smile` | expression cue before text display |
| `voice=@voice...` | explicit voice binding |
| `voice=auto` | derive voice cue from line ID, locale, and speaker |
| `id=@say...` | explicit line entity ID |

The implicit window is `@textbox.0` unless the character, line, or project defaults override it.

Speaker presets are allowed in the same position:

```awft
let alice2 = alice(look=.smile, voice=auto, window=@textbox.side)

alice2: おはよう。[p]

alice2(id=@say.opening.side_001):
    こっちのウィンドウで話すね。[p]
```

Here `alice2` is a lexical speaker preset. It is not a new character and it does not mutate `@character.alice`.

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
[mark .name]
         line-local marker consumed by `with: on .name:`
```

Outside dialogue text mode, `[...]` is not treated as a dialogue control tag. It remains normal Arcweft syntax such as indexing, lists, attributes, or parser-specific syntax.

---

## Built-in narrator character

Arcweft prelude defines a built-in narrator-like character:

```awft
pub character @character.narrator narrator {
    role = narration
    nameplate = hidden
    localizable_name = false
    dialogue_style {
        window = @textbox.narrator
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
window = "textbox.narrator"
```

---

## Staging calls in flow bodies

Scenario staging is expressed as ordinary effectful calls inside `flow` and
`fragment` bodies. The older `@bg` / `@show` command family is not part of the
stable grammar; `@` is reserved for entity references.

```awft
bg(@asset.bg.school_evening, fade = 600ms)
show(@character.alice, .normal, at = .center, layer = @layer.characters, fade = 200ms)
face(@character.alice, .smile, crossfade = 120ms)
move(@character.alice, to = .left, time = 350ms, ease = cubic.out)
anim(@character.alice, @anim.breath, mode = .loop)
hide(@character.alice, fade = 180ms)
```

`bg(...)` and `show(...)` return scope-bound handles. The lifetime of the
visible presentation value is the lifetime of the returned handle unless the
handle is explicitly detached, moved into another scope, or cleared through the
matching clear API.

```awft
scope opening_view {
    let room = bg(@asset.bg.school_evening, fade = 600ms)
    let alice_on_stage = show(@character.alice, .normal, at = .center)

    alice: おはよう。[p]
}
# room and alice_on_stage leave scope here; their registered values are cleared.
```

Every presentation value is registered in a typed target/slot pair. A slot is a
static-option-like cell: setting it replaces and returns the previous value if
one existed, reading it does not change it, and clearing it returns the current
value if present.

```awft
let previous_bg = bg(@asset.bg.room, target = @target.scene, slot = @slot.background.main)
let current_bg = ref bg(target = @target.scene, slot = @slot.background.main)
let cleared_bg = clear bg(target = @target.scene, slot = @slot.background.main)
```

Defaults:

```text
bg(asset)
  target = @target.scene
  slot   = @slot.background.default

show(character, expression)
  target = @target.scene
  slot   = @slot.character.{character}.default
```

If more than one background or one instance of the same character must coexist,
the author must specify a different `target` or `slot`:

```awft
let far = bg(@asset.bg.city_far, slot = @slot.background.far)
let near = bg(@asset.bg.city_near, slot = @slot.background.near)

let alice_main = show(@character.alice, .smile, slot = @slot.character.alice.main)
let alice_reflection = show(@character.alice, .sad, slot = @slot.character.alice.reflection)
```

The `ref` forms are read-only. They do not create a new visible object and they
do not extend the current handle lifetime.

The syntax checker treats these as presentation-aware calls, not legacy
scenario commands. It validates target and slot families and warns when multiple
live handles use the same default slot in one lexical scope; use explicit slots
for simultaneous values.

---

## Complex line with method form

```awft
alice.say(id=@say.opening.dream_hint, voice=auto, look=.smile)[
    今日は少しだけ、#[fmt("変な夢", color=rgb("#a8b5ff"))]を見たんだ。[p]
]
with {
    reveal = voice
    cancel on input .SkipLine => continue
    cancel on input .BackToTitle => goto @flow.title

    at(0.42s) { alice.stage.look(worried, crossfade=120ms) }
    at(end-250ms) { alice.stage.animate(@anim.breath.once) }
}
```

Colon sugar can attach the same line plan with `with { ... }`:

```awft
alice(id=@say.opening.dream_hint, voice=auto, look=.smile):
    今日は少しだけ、#[fmt("変な夢", color=rgb("#a8b5ff"))]を見たんだ。[p]
with {
    at(0.42s) { alice.stage.look(worried, crossfade=120ms) }
}
```

The line plan and `at(...) { ... }` blocks create scoped variables. See [Dialogue Calls, Line Plans, Cancellation, and Scoped Content Blocks](dialogue-calls-scopes-cancellation.md).
