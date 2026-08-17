# Dialogue Calls, Line Plans, Cancellation, and Scoped Content Blocks

> **Surface update:** Use the direct `CharacterDialogue` forms specified in
> [CharacterDialogue authoring](character-dialogue.md). Read every
> `character.say(options)[content]` example below as
> `character(options)[content]`. The line-plan, cancellation, and scoped-content
> rules remain the subject of this chapter; `.say` is not part of the final
> language.
> Content escapes and attached-body admission are governed by
> [Converged Language, Content, and Presentation Surface](converged-language-surface.md).

Arcweft keeps ordinary dialogue concise, but the canonical form is explicit and composable:

```arcw
character.say(args...)[dialogue_content]
with { line_plan }
```

The square bracket block is the player-facing dialogue content. The optional `with` block is a **line plan**: setup, timed cues, line-local handlers, cancellation, scoped variables, memoization, and parallel presentation work that belongs to that line.

Related:

- [Flow-Integrated Scenario Syntax](scenario-surface-syntax.md)
- [Dialogue Control Tags, Ruby, Inline Formatting, and Hooks](dialogue-control-tags-and-ruby.md)
- [Dialogue Views, Character Styles, and Read-State Hooks](dialogue-views-and-hooks.md)
- [Dialogue Character Methods, Dialogue Views, Speaker Presets, Interpolation, and Preload](dialogue-character-methods-and-views.md)
- [Dialogue Content Calls, `with` Blocks, Line Output Values, and Scoped Handles](dialogue-line-handles-and-returns.md)
- [Character Stage / Sprite / Voice Timeline](../03-presentation/character-stage.md)
- [Hooks and Memoization](hooks-and-memoization.md)
- [Option / Result / Need / lifetime](types-and-effects.md)

---

## Design decision

Use these roles consistently:

```text
@foo.bar
  Entity reference only.
  Example: @flow.opening, @say.opening.001, @view.MainDialogue.

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

`@` is the entity-reference marker. A line ID, View, voice, or hook reference
is passed like any other option:

```arcw
alice(id=@say.opening.greeting, view=@view.SideDialogue, voice=auto, look=smile):
    おはよう。[p]
```

`with` is reserved for attaching a line plan to dialogue calls. Parentheses `(...)` are for options; brackets `[...]` are player-facing dialogue content; canonical `with { ... }` or indentation sugar `with:` attaches the line plan. A bare trailing `{ ... }` after `]` is a separate lexical scope, not a line plan attachment.

---

## Speaker presets

A character can be called with line options to produce a reusable speaker preset. This is the preferred way to avoid repeating `voice`, `look`, `window`, and style options.

```arcw
let alice2 = alice(look=smile, voice=auto, view=@view.SideDialogue)

alice2: おはよう。[p]

alice2(id=@say.opening.side_001):
    こっちのウィンドウで話すね。[p]
```

`alice2:` is sugar for calling the speaker preset with an empty option set and
then applying dialogue content: `alice2()[...]`. Per-line options refine the
preset first, so `alice2(voice=auto): text` lowers to
`alice2(voice=auto)[text]`, not to a forced character `.say(...)` call. The
preset is a pure lexical value, so it can be passed to helper functions or kept
local to a block without mutating `@character.alice`.
---

## Canonical form

```arcw
alice.say(
    id = @say.opening.dream_hint,
    view = @view.MainDialogue,
    voice = auto,
    look = smile,
)[
    今日は少しだけ、|[変な夢](へんなゆめ)を見たんだ。[p]
]
with {
    at(0.42s) {
        alice.stage.look(worried, crossfade=120ms)
    }

    at(end-250ms) {
        alice.stage.animate(@anim.breath.once)
    }
}
```

This creates and executes a `DialogueLine`.

```arcw
pub fn Character.say(
    self: Ref<Character>,
    id: Option<Ref<DialogueLine>> = None,
    view: Ref<View> = @view.MainDialogue,
    voice: VoicePolicy = auto,
    look: Option<Expression> = None,
    style: Option<TextStylePatch> = None,
) -> DialogueContentCall
```

A `DialogueContentCall` accepts a content block and optional line plan, then returns `LineOutcome`.

```arcw
pub enum LineOutcome {
    Completed,
    Cancelled(LineCancel),
}
```

---

## `:` speaker syntax is sugar

The concise form:

```arcw
alice: おはよう。[p]
```

is sugar for:

```arcw
alice.say()[
    おはよう。[p]
]
```

Line options go inside parentheses:

```arcw
alice(id=@say.opening.greeting, look=smile, voice=auto):
    おはよう。[p]
```

which is sugar for:

```arcw
alice.say(id=@say.opening.greeting, look=smile, voice=auto)[
    おはよう。[p]
]
```

Narration is the same:

```arcw
地の文: 扉の向こうから、雨の音がした。[p]
```

is sugar for:

```arcw
narrator.say()[
    扉の向こうから、雨の音がした。[p]
]
```

---

## Colon sugar with a line plan

Because dialogue text may contain `{player_name}` localization placeholders, a raw `{ ... }` after `speaker:` is not used directly. Attach line-plan behavior with `with { ... }`.

```arcw
alice(voice=auto, look=smile):
    今日は少しだけ、|[変な夢](へんなゆめ)を見たんだ。[p]
with {
    at(0.42s) {
        alice.stage.look(worried, crossfade=120ms)
    }

    cancel on input(.SkipLine) {
        'line.voice |> drop(stop_now)
        text.flush(mode = .Instant)
        continue
    }
}
```

Equivalent canonical form:

```arcw
alice.say(voice=auto, look=smile)[
    今日は少しだけ、|[変な夢](へんなゆめ)を見たんだ。[p]
]
with {
    at(0.42s) {
        alice.stage.look(worried, crossfade=120ms)
    }

    cancel on input(.SkipLine) {
        'line.voice |> drop(stop_now)
        text.flush(mode = .Instant)
        continue
    }
}
```

For explicit bracketed content while still using `:`, this form is also allowed:

```arcw
alice(voice=auto):[
    今日は少しだけ、|[変な夢](へんなゆめ)を見たんだ。[p]
]
with {
    at(0.42s) { alice.stage.look(worried) }
}
```

This is useful when a line is generated or transformed by tools. Normalized output should prefer explicit `alice.say()[...] with { ... }`; `alice:` and `with:` remain source-level sugar for hand-written scripts.



---

## Bracket speaker call and `with:` indentation sugar

The following compact form is accepted and formatter-supported:

```arcw
alice[
    おはよう。[p]
]
with:
    at(0.42s):
        alice.stage.look(smile)
```

It is equivalent to:

```arcw
alice.say()[
    おはよう。[p]
]
with:
    at(0.42s):
        alice.stage.look(smile)
```

The colon form can use the same line plan attachment:

```arcw
alice:
    おはよう。[p]
with:
    at(0.42s):
        alice.stage.look(smile)
```

`with:` starts a line plan only at the same indentation level as the dialogue call. Inside dialogue text it is ordinary text unless introduced by a control tag. Lowering normalizes it to `with { ... }`.

The brace and indentation styles are equivalent:

```arcw
alice[おはよう。[p]]
with { at(0.42s) { alice.stage.look(smile) } }
```

```arcw
alice[おはよう。[p]]
with:
    at(0.42s): alice.stage.look(smile)
```

See [Dialogue Content Calls, `with` Blocks, Line Output Values, and Scoped Handles](dialogue-line-handles-and-returns.md) for output values and handle ownership.

---

## Line plan block

A line plan block contains behavior that runs with the line.

```arcw
alice.say(voice=auto)[
    聞いて。[p]
]
with {
    memo(.rich_text, key=(line.id, locale, theme.text_hash), cache=.flow)
    memo(.voice_cue, key=(voice.key, locale), cache=.session)

    start {
        alice.stage.look(smile)
    }

    at(0.42s) {
        alice.stage.look(worried, crossfade=120ms)
    }

    at(marker("surprise")) {
        try flash(color=rgb("#ffffff"), time=90ms)
    }

    cancel on input(.BackToTitle) {
        'line.voice |> drop(stop_now)
        cues.stop(policy = .CancelPending)
        goto @flow.title
    }
}
```

Line plan statements are scoped to the line and cannot leak variables outward.

Line-plan block heads may use the canonical brace form, indentation sugar, or
flat fence sugar. For example, `start { ... }`, `start:`, and
`=== start === ... === /start ===` all lower through the same line-plan item
model. The same rule applies to `init`, `thread`, `on mark(...)`,
`cancel on ...`, `defer on ...`, `start`, `together`, and `scope` block items.
Malformed flat fences are not migration syntax: unknown fence kinds, close
mismatches, and missing close fences are parser diagnostics before HIR lowering.

`init` runs before reveal begins. `thread name` creates a line-scoped child task
owned by the line task group; it is a Sans I/O runtime task, not an OS thread.
Scoped cleanup uses `defer { ... }` on the current runtime scope. Line-wide
cleanup is also modeled as line-scope `defer`. Use `defer on completed`,
`defer on cancelled`, or `defer on failed` when cleanup should run only for a
specific scope-exit outcome.
Line-local events use `[mark .name]` in text and `on mark(.name)` in the line plan.
Registered `defer` blocks run when their owning scope exits, including normal
completion, early control transfer, line cancellation, and child-task
cancellation. A cancelled child task must unwind its defer stack before it is
considered joined.

```arcw
alice.say(look=smile, focus=.soft)[
    今日は少しだけ、変な夢を見たんだ。[mark .release_focus][p]
]
with {
    init {
        'line.focus.main <- acquire_focus()
    }

    on mark(.release_focus) {
        'line.focus |> drop
        out .Released
    }

    thread motion {
        wait(mark(.release_focus))
        wait(0.35s)
        alice.stage.look(worried)

        defer {
            cleanup_motion()
        }
    }

    defer {
        cleanup_line()
    }

    defer on completed {
        metric.increment(@metric.line_completed)
    }

    defer on cancelled {
        metric.increment(@metric.line_cancelled)
    }
}
```

Line lifetime registry paths use the same lifetime sigil as scope labels:
`'line.focus` is a guaranteed value only when the checker can prove the key was
registered, while `'line.focus?` is optional and returns an `Option`-like value.
`focus=.soft` guarantees the default `'line.focus`; explicit keys such as
`'line.focus.main` become guaranteed after an assignment with `<-`.

`cleanup` is a line option/profile. `cleanup on ...:` is not part of the line
plan grammar; use `defer { ... }` in the relevant runtime scope, line-level
`defer { ... }`, cancellation rules, or explicit drop operations.

---

## Simultaneous execution

The dialogue content, voice, text reveal, stage cues, and line plan all run under the same line timeline. To start multiple actions at the same time, use `start { ... }`, `together { ... }`, or schedule cues at the same time.

```arcw
alice.say(voice=auto)[
    走って！[p]
]
with {
    start {
        together {
            alice.stage.move(to=left, time=300ms, ease=quad.out)
            alice.stage.look(panic, crossfade=80ms)
            se.play(@se.footstep_fast)
        }
    }

    at(0.30s) {
        alice.stage.shake(strength=0.3, time=120ms)
    }
}
```

`together { ... }` is not a thread primitive. It groups effect requests on the same timeline tick. Long-running work must still use unary `Need<T>` and explicit pending handling; fallible work uses a Result payload.

---

## Timed cue syntax

`at(anchor) { ... }` schedules cues relative to the current line timeline.

```arcw
at(0.35s) { alice.stage.look(blink) }
at(+120ms) { alice.stage.mouth(open) }
at(end-200ms) { alice.stage.move(to=left, time=260ms, ease=quad.out) }
at(marker("soft_smile")) { alice.stage.look(smile, crossfade=100ms) }
at(phoneme("a")) { alice.stage.mouth(a) }
at(char(12)) { signal.set(@signal.text_reveal_hit, true) }
```

Supported anchors:

| Form | Meaning |
|---|---|
| `at(0.42s)` | absolute offset from line start or voice start |
| `at(+120ms)` | relative offset after previous cue |
| `at(end-250ms)` | relative to voice/text reveal end |
| `at(marker("name"))` | voice marker event |
| `at(phoneme("a"))` | lip-sync phoneme event |
| `at(char(12))` | character reveal index |
| `at(word(3))` | word/token reveal index |

The line-plan cue form is `at(...) { ... }` or the indentation sugar
`at(...):`.

---

## Inline timed cue tags

For simple one-shot events inside dialogue text, inline tags are allowed:

```arcw
alice(voice=auto):
    ねえ。[at 0.42s call=flash(color=#ffffff, time=90ms)]聞いて。[p]
```

This is sugar for a timeline event in the surrounding line. Prefer line-plan `at(...) { ... }` blocks when there are multiple cues or when source readability matters.

---

## Audio playback and nested content

Audio can also be the outer call when a voice region controls nested text or cues:

```arcw
alice.voice(@voice.alice.opening.002).play()[
    alice.say()[今日は少しだけ、変な夢を見たんだ。[p]]
]
with {
    cancel on input(.SkipLine) {
        stop fade=40ms
        continue
    }

    at(marker("surprise")) {
        alice.stage.look(surprised)
    }
}
```

For ordinary dialogue, prefer:

```arcw
alice.say(voice=auto)[今日は少しだけ、変な夢を見たんだ。[p]]
```

---

## Cancellation policies

Dialogue lines, voice playback, animations, and timed hooks may be cancelled by input, branch, timeout, or external signals.

```arcw
alice.say(voice=auto)[
    今日は少しだけ、変な夢を見たんだ。[p]
]
with {
    cancel on input(.SkipLine) {
        'line.voice |> drop(stop_now)
        text.flush(mode = .Instant)
        continue
    }

    cancel on input(.BackToTitle) {
        'line.voice |> drop(stop_now)
        cues.stop(policy = .CancelPending)
        goto @flow.title
    }

    cancel on signal(@signal.route_forced) {
        'line.voice |> drop(stop_now)
        goto signal_value(@signal.route_forced)
    }
}
```

Cancellation can return an outcome:

```arcw
let outcome = alice.say(voice=auto)[
    今日は少しだけ、変な夢を見たんだ。[p]
]
with {
    cancel on input(.SkipLine) => LineCancel.Skipped
    cancel on input(.BackToTitle) => LineCancel.Goto(@flow.title)
}

match outcome {
    .Completed => continue
    .Cancelled(.Skipped) => continue
    .Cancelled(.Goto(flow)) => return Ok(FlowExit.Goto(flow))
}
```

If the result is ignored, the default line policy is used. A `goto` cancellation terminates the current flow segment and produces a `FlowExit.Goto`.

---

## Scoped variables

Content calls, line plan blocks, `with` blocks, and `at` blocks create explicit lexical scopes.

```arcw
alice.say()[
    #[let local_word = "まぶしい"]
    #[local_word]……[p]
]
with {
    let local_flash_color = rgb("#ffffff")

    at(0.25s) {
        try flash(color=local_flash_color)
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
- `[mark .name]` zero-width line-local markers
```

Allowed in line plan blocks:

```text
- init blocks
- scoped `defer` cleanup on line, init, thread, and handler scopes
- line-scope `defer` cleanup
- line-local `on mark(.mark):` handlers
- `wait(mark(.name))` and duration waits
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
- character object methods such as alice.stage.look(...)
- debug log/signal if capability permits
```

Use normal typed `if`, `match`, `await`, and `return` in the surrounding `flow` body, not inside raw dialogue text.

---

## Dialogue-safe functions

A function called from dialogue text or cue mode must be declared dialogue-safe.

```arcw
pub fn flash(
    color: Color = rgb("#ffffff"),
    time: Duration = 120ms,
) -> Result<DialogueCue, TagError>
effects { stage.flash }
{
    Ok(DialogueCue.Flash { color, time })
}
```

Use from line plan:

```arcw
alice.say()[まぶしい……[p]]
with {
    at(0.25s) {
        try flash(color=rgb("#ffffff"), time=90ms)
    }
}
```

Use from inline text with the reserved `[call]` tag:

```arcw
alice: まぶしい……[call flash(color=#ffffff, time=90ms)][p]
```

The function must be in scope through `use dialogue` or `use tag`.

```arcw
use dialogue game.fx.{flash}
use tag game.fx.{flash as flash_tag}
```

---

## Custom tags and hooks

A custom tag maps bracket syntax to a typed function.

```arcw
pub dialogue tag @tag.shake shake(
    target: Ref<Character>,
    strength: f32 = 0.25,
    time: Duration = 180ms,
) -> Result<DialogueCue, TagError>
requires strength >= 0.0 && strength <= 1.0
effects { stage.animate }
{
    Ok(DialogueCue.Shake { target, strength, time })
}
```

Usage:

```arcw
alice: きゃっ。[shake target=alice strength=0.4 time=160ms][p]
```

Hook dispatch:

```arcw
alice: #[player_name]、聞いて。[mark .important][p]
with:
    on mark(.important):
        mark_important()
```

Custom tag names cannot collide with reserved built-ins such as `p`, `l`, `ruby`, `call`, `mark`, `voice`, or `at`.

---

## Desugaring summary

```arcw
alice(look=smile, voice=auto):
    おはよう。[p]
```

becomes conceptually:

```arcw
alice.say(look=smile, voice=auto)[
    おはよう。[p]
]
```

```arcw
alice(id=@say.opening.003, look=smile, voice=@voice.alice.003):
    ほら、ここ。覚えてる？[p]
```

becomes conceptually:

```arcw
alice.say(id=@say.opening.003, look=smile, voice=@voice.alice.003)[
    ほら、ここ。覚えてる？[p]
]
```

```arcw
face(@character.alice, .worried, crossfade = 120ms)
```

becomes:

```arcw
alice.stage.look(.worried, crossfade = 120ms)
```

```arcw
alice(voice=auto):
    聞いて。[p]
with {
    at(0.42s) { alice.stage.look(worried) }
}
```

becomes:

```arcw
alice.say(voice=auto)[
    聞いて。[p]
]
with {
    at(0.42s) { alice.stage.look(worried) }
}
```


---

## Line plan output values and scoped handles

A line plan exports a value with `out`. Do not use `return` for line-plan
values; `return` exits the nearest `fn`, `parser`, or `flow`.
`out` is how short-lived line-local handles are exported deliberately.

```arcw
let (actor, (face0, face1, voice)) = alice.say(
    id=@say.opening.dream_hint,
    voice=auto,
    look=smile,
)[
    今日は少しだけ、|[変な夢](へんなゆめ)を見たんだ。[p]
]
with:
    let actor = alice.stage.acquire(scope=line)
    let face0 = actor.look(smile)
    let voice = line.voice_handle()
    let face1 = at(0.42s):
        actor.look(worried, crossfade=120ms)

    out (actor, (face0, face1, voice))
```

Returned values are ordinary typed values, but many presentation operations return scoped handles. Handles have drop policies. Binding a returned handle to `_` explicitly discards it; for cancellable handles this cancels or releases the operation immediately after destructuring.

```arcw
let (_, (face0, _, voice)) = alice.say(voice=auto)[
    聞いて。[p]
]
with:
    let actor = alice.stage.acquire(scope=line)
    let face0 = actor.look(smile)
    let face1 = at(0.42s): actor.look(worried)
    let voice = line.voice_handle()
    out (actor, (face0, face1, voice))
```

`face1` is discarded and its scheduled cue is cancelled if it has not fired. The discarded `actor` release policy runs immediately. The `voice` handle is kept by the surrounding scope.

BGM, subscriptions, hooks, and stage leases follow the same rule. To keep BGM beyond a line, detach or promote the handle explicitly:

```arcw
let bgm_handle = alice[始まるよ。[p]]
with:
    let scoped_bgm = bgm.play(@bgm.tension, scope=line, drop=fade(300ms))
    out scoped_bgm.detach()
```


