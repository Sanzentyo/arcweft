# Dialogue Calls, Line Plans, Cancellation, and Scoped Content Blocks

Arcweft keeps ordinary dialogue concise, but the canonical form is explicit and composable:

```awft
character.say(args...)[dialogue_content]
with { line_plan }
```

The square bracket block is the player-facing dialogue content. The optional `with` block is a **line plan**: timed cues, cancellation, scoped variables, memoization, hooks, and parallel presentation work that belongs to that line.

Related:

- [Flow-Integrated Scenario Syntax](scenario-surface-syntax.md)
- [Dialogue Control Tags, Ruby, Inline Formatting, and Hooks](dialogue-control-tags-and-ruby.md)
- [Dialogue Windows, Character Styles, and Read-State Hooks](dialogue-windows-and-hooks.md)
- [Dialogue Character Methods, Dialogue Windows, Speaker Presets, Interpolation, and Preload](dialogue-character-methods-and-textbox.md)
- [Dialogue Content Calls, `with` Blocks, Line Output Values, and Scoped Handles](dialogue-line-handles-and-returns.md)
- [Character Stage / Sprite / Voice Timeline](../03-presentation/character-stage.md)
- [Hooks and Memoization](hooks-and-memoization.md)
- [Option / Result / Need / lifetime](types-and-effects.md)

---

## Design decision

Use these roles consistently:

```text
#foo.bar
  Entity reference only.
  Example: #flow.opening, #say.opening.001, #textbox.0.

speaker(args): text
  Compact dialogue sugar.
  `args` are the same as `say(args)`.

let speaker2 = speaker(args)
  Curried speaker preset.
  `speaker2:` uses the stored options.

speaker.say(args)[text] with { plan }
  Preferred detailed form.

with { plan } / with:
  Line-plan attachment for method, bracket, and colon forms.
  `with { plan }` is canonical. `with:` is indentation sugar used when `speaker:` form needs the same plan behavior.
```

`#` is not used for line options. `#` remains only an entity-reference marker. A line ID, window, voice, or hook reference is passed like any other option:

```awft
alice(id=#say.opening.greeting, window=#textbox.side, voice=auto, face=smile):
    おはよう。[p]
```

`with` is reserved for attaching a line plan to dialogue calls. Parentheses `(...)` are for options; brackets `[...]` are player-facing dialogue content; canonical `with { ... }` or indentation sugar `with:` attaches the line plan. A bare trailing `{ ... }` after `]` is a separate lexical scope, not a line plan attachment.

The older compact option style:

```awft
alice #say.opening.greeting @smile voice auto:
    おはよう。[p]
```

is not part of the stable grammar. The formatter may migrate it to the parenthesized form while this syntax is still recognized by early tooling.

---

## Speaker presets

A character can be called with line options to produce a reusable speaker preset. This is the preferred way to avoid repeating `voice`, `face`, `window`, and style options.

```awft
let alice2 = alice(face=smile, voice=auto, window=#textbox.side)

alice2: おはよう。[p]

alice2(id=#say.opening.side_001):
    こっちのウィンドウで話すね。[p]
```

`alice2:` is sugar for `alice2.say()[...]`. Per-line options override preset options. The preset is a pure lexical value, so it can be passed to helper functions or kept local to a block without mutating `#character.alice`.
---

## Canonical form

```awft
alice.say(
    id = #say.opening.dream_hint,
    window = #textbox.0,
    voice = auto,
    face = smile,
)[
    今日は少しだけ、｜変な夢《へんなゆめ》を見たんだ。[p]
]
with {
    at(0.42s) {
        alice.stage.face(worried, crossfade=120ms)
    }

    at(end-250ms) {
        alice.stage.animate(#anim.breath.once)
    }
}
```

This creates and executes a `DialogueLine`.

```awft
pub fn Character.say(
    self: Ref<Character>,
    id: Option<Ref<DialogueLine>> = None,
    window: Ref<Textbox> = #textbox.0,
    voice: VoicePolicy = auto,
    face: Option<Expression> = None,
    style: Option<TextStylePatch> = None,
    hooks: List<Ref<Hook>> = [],
) -> DialogueContentCall
```

A `DialogueContentCall` accepts a content block and optional line plan, then returns `LineOutcome`.

```awft
pub enum LineOutcome {
    Completed,
    Cancelled(LineCancel),
}
```

---

## `:` speaker syntax is sugar

The concise form:

```awft
alice: おはよう。[p]
```

is sugar for:

```awft
alice.say()[
    おはよう。[p]
]
```

Line options go inside parentheses:

```awft
alice(id=#say.opening.greeting, face=smile, voice=auto):
    おはよう。[p]
```

which is sugar for:

```awft
alice.say(id=#say.opening.greeting, face=smile, voice=auto)[
    おはよう。[p]
]
```

Narration is the same:

```awft
地の文: 扉の向こうから、雨の音がした。[p]
```

is sugar for:

```awft
narrator.say()[
    扉の向こうから、雨の音がした。[p]
]
```

---

## Colon sugar with a line plan

Because dialogue text may contain `{player_name}` localization placeholders, a raw `{ ... }` after `speaker:` is not used directly. Attach line-plan behavior with `with { ... }`.

```awft
alice(voice=auto, face=smile):
    今日は少しだけ、｜変な夢《へんなゆめ》を見たんだ。[p]
with {
    at(0.42s) {
        alice.stage.face(worried, crossfade=120ms)
    }

    cancel on input .SkipLine {
        stop voice fade=40ms
        flush text instant
        continue
    }
}
```

Equivalent canonical form:

```awft
alice.say(voice=auto, face=smile)[
    今日は少しだけ、｜変な夢《へんなゆめ》を見たんだ。[p]
]
with {
    at(0.42s) {
        alice.stage.face(worried, crossfade=120ms)
    }

    cancel on input .SkipLine {
        stop voice fade=40ms
        flush text instant
        continue
    }
}
```

For explicit bracketed content while still using `:`, this form is also allowed:

```awft
alice(voice=auto):[
    今日は少しだけ、｜変な夢《へんなゆめ》を見たんだ。[p]
]
with {
    at(0.42s) { alice.stage.face(worried) }
}
```

This is useful when a line is generated or transformed by tools. Normalized output should prefer explicit `alice.say()[...] with { ... }`; `alice:` and `with:` remain source-level sugar for hand-written scripts.



---

## Bracket speaker call and `with:` indentation sugar

The following compact form is accepted and formatter-supported:

```awft
alice[
    おはよう。[p]
]
with:
    at(0.42s):
        alice.stage.face(smile)
```

It is equivalent to:

```awft
alice.say()[
    おはよう。[p]
]
with:
    at(0.42s):
        alice.stage.face(smile)
```

The colon form can use the same line plan attachment:

```awft
alice:
    おはよう。[p]
with:
    at(0.42s):
        alice.stage.face(smile)
```

`with:` starts a line plan only at the same indentation level as the dialogue call. Inside dialogue text it is ordinary text unless introduced by a control tag. Lowering normalizes it to `with { ... }`.

The brace and indentation styles are equivalent:

```awft
alice[おはよう。[p]]
with { at(0.42s) { alice.stage.face(smile) } }
```

```awft
alice[おはよう。[p]]
with:
    at(0.42s): alice.stage.face(smile)
```

See [Dialogue Content Calls, `with` Blocks, Line Output Values, and Scoped Handles](dialogue-line-handles-and-returns.md) for output values and handle ownership.

---

## Line plan block

A line plan block contains behavior that runs with the line.

```awft
alice.say(voice=auto)[
    聞いて。[p]
]
with {
    memo rich_text key=(line.id, locale, theme.text_hash) cache=flow
    memo voice_cue key=(voice.key, locale) cache=session

    start {
        alice.stage.face(smile)
    }

    at(0.42s) {
        alice.stage.face(worried, crossfade=120ms)
    }

    at(marker("surprise")) {
        flash(color=rgb("#ffffff"), time=90ms)?
    }

    cancel on input .BackToTitle {
        stop voice fade=80ms
        stop cues policy=cancel_pending
        goto #flow.title
    }
}
```

Line plan statements are scoped to the line and cannot leak variables outward.

---

## Simultaneous execution

The dialogue content, voice, text reveal, stage cues, and line plan all run under the same line timeline. To start multiple actions at the same time, use `start { ... }`, `together { ... }`, or schedule cues at the same time.

```awft
alice.say(voice=auto)[
    走って！[p]
]
with {
    start {
        together {
            alice.stage.move(to=left, time=300ms, ease=quad.out)
            alice.stage.face(panic, crossfade=80ms)
            se.play(#se.footstep_fast)
        }
    }

    at(0.30s) {
        alice.stage.shake(strength=0.3, time=120ms)
    }
}
```

`together { ... }` is not a thread primitive. It groups effect requests on the same timeline tick. Long-running work must still use `Need<T, E>` and explicit pending handling.

---

## Timed cue syntax

`at(anchor) { ... }` schedules cues relative to the current line timeline.

```awft
at(0.35s) { alice.stage.face(blink) }
at(+120ms) { alice.stage.mouth(open) }
at(end-200ms) { alice.stage.move(to=left, time=260ms, ease=quad.out) }
at(marker("soft_smile")) { alice.stage.face(smile, crossfade=100ms) }
at(phoneme "a") { alice.stage.mouth(a) }
at(char 12) { signal #signal.text_reveal_hit <- true }
```

Supported anchors:

| Form | Meaning |
|---|---|
| `at(0.42s)` | absolute offset from line start or voice start |
| `at(+120ms)` | relative offset after previous cue |
| `at(end-250ms)` | relative to voice/text reveal end |
| `at(marker("name"))` | voice marker event |
| `at(phoneme "a")` | lip-sync phoneme event |
| `at(char 12)` | character reveal index |
| `at(word 3)` | word/token reveal index |

Older `at(...)[...]` cue blocks are accepted by the parser for compatibility, but the canonical line-plan form is `at(...) { ... }` or the indentation form `at(...):`.

---

## Inline timed cue tags

For simple one-shot events inside dialogue text, inline tags are allowed:

```awft
alice(voice=auto):
    ねえ。[at 0.42s call=flash(color=#ffffff, time=90ms)]聞いて。[p]
```

This is sugar for a timeline event in the surrounding line. Prefer line-plan `at(...) { ... }` blocks when there are multiple cues or when source readability matters.

---

## Audio playback and nested content

Audio can also be the outer call when a voice region controls nested text or cues:

```awft
alice.voice(#voice.alice.opening.002).play()[
    alice.say()[今日は少しだけ、変な夢を見たんだ。[p]]
]
with {
    cancel on input .SkipLine {
        stop fade=40ms
        continue
    }

    at(marker("surprise")) {
        alice.stage.face(surprised)
    }
}
```

For ordinary dialogue, prefer:

```awft
alice.say(voice=auto)[今日は少しだけ、変な夢を見たんだ。[p]]
```

---

## Cancellation policies

Dialogue lines, voice playback, animations, and timed hooks may be cancelled by input, branch, timeout, or external signals.

```awft
alice.say(voice=auto)[
    今日は少しだけ、変な夢を見たんだ。[p]
]
with {
    cancel on input .SkipLine {
        stop voice fade=40ms
        flush text instant
        continue
    }

    cancel on input .BackToTitle {
        stop voice fade=80ms
        stop cues policy=cancel_pending
        goto #flow.title
    }

    cancel on signal #signal.route_forced {
        stop voice fade=120ms
        goto signal_value(#signal.route_forced)
    }
}
```

Cancellation can return an outcome:

```awft
let outcome = alice.say(voice=auto)[
    今日は少しだけ、変な夢を見たんだ。[p]
]
with {
    cancel on input .SkipLine => LineCancel::Skipped
    cancel on input .BackToTitle => LineCancel::Goto(#flow.title)
}

match outcome {
    .Completed => continue
    .Cancelled(.Skipped) => continue
    .Cancelled(.Goto(flow)) => return Ok(FlowExit::Goto(flow))
}
```

If the result is ignored, the default line policy is used. A `goto` cancellation terminates the current flow segment and produces a `FlowExit::Goto`.

---

## Scoped variables

Content calls, line plan blocks, `with` blocks, and `at` blocks create explicit lexical scopes.

```awft
alice.say()[
    #[let local_word = "まぶしい"]
    #[local_word]……[p]
]
with {
    let local_flash_color = rgb("#ffffff")

    at(0.25s) {
        flash(color=local_flash_color)?
    }
}

// local_flash_color and local_word are not visible here.
```

Scope rules:

```text
- Variables declared in a line plan are visible to that line's timed cues.
- Variables declared in `#[...]` expression blocks are local to that expression unless the expression returns Content.
- Variables declared in `at(...) { ... }` are local to that timed cue.
- Borrowed references cannot cross timeline, await, yield, or cancellation boundaries.
- Captures for timed cues must be owned, Copy, Ref<T>, Handle<T>, or serializable values.
```

This intentionally mirrors Arcweft's `Need`, lifetime, and cancellation model.

---

## Flow-control restrictions inside content blocks

Dialogue content blocks are not arbitrary flow blocks.

Allowed in dialogue content:

```text
- text and rich text
- `[p]`, `[l]`, `[r]`, ruby, style tags
- `#[expr]` expressions returning Content/String/Option/Result or Display-compatible values
- `fmt(expr, ...)`
- dialogue-safe function calls
- hook dispatches
```

Allowed in line plan blocks:

```text
- line options
- cancellation rules
- at(...) cue blocks
- start/together cue groups
- let bindings scoped to the line
- memo declarations scoped to the line
- contracts/debug assertions for the line
```

Allowed in `at` blocks:

```text
- DialogueCue-producing functions
- stage cue sugar: face/move/anim/shake/signal
- character object methods such as alice.stage.face(...)
- debug log/signal if capability permits
```

Use normal typed `if`, `match`, `await`, and `return` in the surrounding `flow` body, not inside raw dialogue text.

---

## Dialogue-safe functions

A function called from dialogue text or cue mode must be declared dialogue-safe.

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

Use from line plan:

```awft
alice.say()[まぶしい……[p]]
with {
    at(0.25s) {
        flash(color=rgb("#ffffff"), time=90ms)?
    }
}
```

Use from inline text with the reserved `[call]` tag:

```awft
alice: まぶしい……[call flash(color=#ffffff, time=90ms)][p]
```

The function must be in scope through `use dialogue` or `use tag`.

```awft
use dialogue game::fx::{flash}
use tag game::fx::{flash as flash_tag}
```

---

## Custom tags and hooks

A custom tag maps bracket syntax to a typed function.

```awft
pub dialogue tag #tag.shake shake(
    target: Ref<Character>,
    strength: f32 = 0.25,
    time: Duration = 180ms,
) -> Result<DialogueCue, TagError>
requires strength >= 0.0 && strength <= 1.0
effects { stage.animate }
{
    Ok(DialogueCue::Shake { target, strength, time })
}
```

Usage:

```awft
alice: きゃっ。[shake target=alice strength=0.4 time=160ms][p]
```

Hook dispatch:

```awft
alice: #[player_name]、聞いて。[hook #hook.dialogue.mark_important][p]
```

Custom tag names cannot collide with reserved built-ins such as `p`, `l`, `ruby`, `call`, `hook`, `voice`, or `at`.

---

## Desugaring summary

```awft
alice(face=smile, voice=auto):
    おはよう。[p]
```

becomes conceptually:

```awft
alice.say(face=smile, voice=auto)[
    おはよう。[p]
]
```

```awft
alice(id=#say.opening.003, face=smile, voice=#voice.alice.003):
    ほら、ここ。覚えてる？[p]
```

becomes conceptually:

```awft
alice.say(id=#say.opening.003, face=smile, voice=#voice.alice.003)[
    ほら、ここ。覚えてる？[p]
]
```

```awft
@face alice worried crossfade=120ms
```

becomes:

```awft
alice.stage.face(worried, crossfade=120ms)
```

```awft
alice(voice=auto):
    聞いて。[p]
with {
    at(0.42s) { alice.stage.face(worried) }
}
```

becomes:

```awft
alice.say(voice=auto)[
    聞いて。[p]
]
with {
    at(0.42s) { alice.stage.face(worried) }
}
```


---

## Line plan return values and scoped handles

A line plan may `return` a value. This is how short-lived line-local handles can be exported deliberately.

```awft
let (actor, (face0, face1, voice)) = alice.say(
    id=#say.opening.dream_hint,
    voice=auto,
    face=smile,
)[
    今日は少しだけ、｜変な夢《へんなゆめ》を見たんだ。[p]
]
with:
    let actor = alice.stage.acquire(scope=line)
    let face0 = actor.face(smile)
    let voice = line.voice_handle()
    let face1 = at(0.42s):
        actor.face(worried, crossfade=120ms)

    return (actor, (face0, face1, voice))
```

Returned values are ordinary typed values, but many presentation operations return scoped handles. Handles have drop policies. Binding a returned handle to `_` explicitly discards it; for cancellable handles this cancels or releases the operation immediately after destructuring.

```awft
let (_, (face0, _, voice)) = alice.say(voice=auto)[
    聞いて。[p]
]
with:
    let actor = alice.stage.acquire(scope=line)
    let face0 = actor.face(smile)
    let face1 = at(0.42s): actor.face(worried)
    let voice = line.voice_handle()
    return (actor, (face0, face1, voice))
```

`face1` is discarded and its scheduled cue is cancelled if it has not fired. The discarded `actor` release policy runs immediately. The `voice` handle is kept by the surrounding scope.

BGM, subscriptions, hooks, and stage leases follow the same rule. To keep BGM beyond a line, detach or promote the handle explicitly:

```awft
let bgm_handle = alice[始まるよ。[p]]
with:
    let scoped_bgm = bgm.play(#bgm.tension, scope=line, drop=fade(300ms))
    return scoped_bgm.detach()
```
