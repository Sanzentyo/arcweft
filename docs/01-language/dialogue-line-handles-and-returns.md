# Dialogue Content Calls, `with` Blocks, Line Output Values, and Scoped Handles

Arcweft supports concise dialogue while preserving typed control over voice, BGM, stage objects, hooks, and cancellation.

Related:

- [Dialogue Calls, Line Plans, Cancellation, and Scoped Content Blocks](dialogue-calls-scopes-cancellation.md)
- [Dialogue Character Methods, Dialogue Windows, Speaker Presets, Interpolation, and Preload](dialogue-character-methods-and-textbox.md)
- [Dialogue Control Tags, Ruby, Inline Formatting, and Hooks](dialogue-control-tags-and-ruby.md)
- [Flow-Integrated Scenario Syntax](scenario-surface-syntax.md)
- [Character Stage / Sprite / Voice Timeline](../03-presentation/character-stage.md)
- [Audio / Spatial / TTS / BGM](../03-presentation/audio.md)

---

## Accepted dialogue-call forms

The preferred detailed shape is:

```arcw
speaker.say(args)[dialogue]
with {
    line plan
}
```

For script-like sections, `with:` is indentation sugar for the same block:

```arcw
speaker.say(args)[dialogue]
with:
    line plan
```

For ordinary lines, the speaker itself can receive the content block:

```arcw
alice[
    おはよう。[p]
]
with:
    at(0.42s):
        alice.stage.look(smile)
```

This is equivalent to the canonical detailed form:

```arcw
alice.say()[
    おはよう。[p]
]
with:
    at(0.42s):
        alice.stage.look(smile)
```

The colon form is the shortest speaker-call sugar:

```arcw
alice:
    おはよう。[p]
with:
    at(0.42s):
        alice.stage.look(smile)
```

This is equivalent to the same canonical detailed form:

```arcw
alice.say()[
    おはよう。[p]
]
with:
    at(0.42s):
        alice.stage.look(smile)
```

All four source forms produce the same typed `DialogueLine`. The canonical
semantic shape uses an explicit speaker content call plus a `with { ... }` plan,
while formatters may preserve `with:` in hand-written scenario files. When the
callee is a speaker preset, the explicit content call remains the preset call
surface, for example `alice2(voice=auto)[...]`, instead of being rewritten as a
character `.say(...)` call.

---

## Why `alice[...] with {}` is reasonable

It is realistic and useful because `alice[...]` is only interpreted as a dialogue content call when `alice` has type `Speaker`, `Ref<Character>`, or `SpeakerPreset` and appears in flow-item position.

```arcw
alice[おはよう。[p]]
```

is a dialogue call.

```arcw
items[index]
```

is normal indexing, because `items` is not a speaker.

If a parser cannot decide during lossless parsing, it keeps a generic `PostfixBracket` CST node. HIR lowering resolves it by type.

```text
Speaker + [DialogueText] in flow item context
  -> DialogueContentCall

Collection + [Expr]
  -> IndexExpr
```

The formatter may expand ambiguous cases to `alice.say()[...]`.

---

## `with {}` and `with:` sugar

Both block forms are supported. `with { ... }` is canonical; `with:` is syntax sugar for an indented source block.

```arcw
alice[おはよう。[p]]
with {
    at(0.42s) { alice.stage.look(smile) }
}
```

```arcw
alice[おはよう。[p]]
with:
    at(0.42s):
        alice.stage.look(smile)
```

The two forms are equivalent after parsing. Project formatting controls the printed source style.

```toml
[fmt.dialogue]
line_plan_style = "indent"   # "indent" | "brace" | "preserve"
```

`with:` begins a line-plan block only when it is aligned with the speaker line or content call. Inside dialogue text, `with:` is ordinary text unless escaped or parsed as part of a dialogue tag. Lowering should normalize it to the same representation as `with { ... }`.

A bare block after dialogue content is not a line plan:

```arcw
alice.say()[おはよう。[p]] {
    debug_log()
}
```

The `{ ... }` above is a separate lexical scope. Use `with { ... }` when the block is intended to be a line plan.

---

## Colon speaker sugar with `with:`

The following is a concise complex line. The `alice:` head is syntax sugar for a character dialogue call, and `with:` is syntax sugar for `with { ... }`.

```arcw
alice(voice=auto, look=.smile):
    今日は少しだけ、|[変な夢](へんなゆめ)を見たんだ。[p]
with:
    at(0.42s):
        alice.stage.look(.worried, crossfade=120ms)

    cancel on input(.SkipLine):
        'line.voice |> drop(stop_now)
        text.flush(mode = .Instant)
        continue
```

It is equivalent to:

```arcw
alice.say(voice=auto, look=.smile)[
    今日は少しだけ、|[変な夢](へんなゆめ)を見たんだ。[p]
]
with:
    at(0.42s):
        alice.stage.look(.worried, crossfade=120ms)

    cancel on input(.SkipLine):
        'line.voice |> drop(stop_now)
        text.flush(mode = .Instant)
        continue
```

---

## Line result values

A line plan may export a value with `out`. Without an explicit `out`, the line result is `()`.

```arcw
let handles = alice.say(voice=auto)[
    聞いて。[p]
]
with:
    let voice = line.voice_handle()
    let look = alice.stage.look(.smile)
    out (voice, look)
```

The type is inferred from the `out` expression.

```text
alice.say(...)[...] with: out (voice, look)
  -> (VoiceHandle, StageCueHandle)
```

If a cancellation branch can complete the line differently, it must either:

```text
- out the same result type,
- perform non-returning flow control such as goto/return FlowExit,
- or make the whole expression return Result<R, LineCancel> with try-line syntax.
```

Example with explicit cancel result:

```arcw
let result = try alice.say(voice=auto)[
    聞いて。[p]
]
with:
    cancel on input(.SkipLine):
        out Err(LineCancel::Skipped)

    out Ok(())
```

For most visual-novel lines, cancel handlers use `continue`, `goto`, or `return Ok(FlowExit::...)`, so ordinary bindings remain ergonomic.

---

## Tuple destructuring and `_` discard

Line results can be destructured.

```arcw
let (line_alice, (face0, face1, voice)) = alice.say(
    id=@say.opening.dream_hint,
    voice=auto,
    look=.smile,
)[
    今日は少しだけ、|[変な夢](へんなゆめ)を見たんだ。[p]
]
with:
    let line_alice = alice.stage.acquire(scope=line)
    let voice = line.voice_handle()
    let face0 = line_alice.look(.smile)
    let face1 = at(0.42s):
        line_alice.look(.worried, crossfade=120ms)

    out (line_alice, (face0, face1, voice))
```

`_` explicitly discards a returned value.

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

For plain values, `_` just discards. For scoped handles, `_` runs the handle's drop policy immediately after destructuring. This is intentional: if you do not keep the handle, you explicitly give up ownership and Arcweft cancels or releases it according to its type.

---

## Scoped handles

Line plans can create handles for voice, BGM, animation, stage leases, subscriptions, and scheduled cues.

```arcw
pub trait ScopedHandle {
    fn drop_policy(self) -> DropPolicy
}

pub enum DropPolicy {
    Cancel,
    Stop { fade: Duration },
    Finish,
    Release,
    Detach,
}
```

Default policies:

| Handle | Default when dropped in line scope |
|---|---|
| `VoiceHandle` | stop or release line voice; skip/cancel uses line cancel policy |
| `BgmHandle(scope=line)` | stop/fade according to BGM policy |
| `BgmHandle(scope=global)` | detach; not stopped by line drop |
| `AnimationHandle` | cancel pending animation unless marked `complete_on_drop` |
| `ScheduledCueHandle<T>` | cancel cue if it has not fired |
| `StageLease` | release lease; does not hide sprite unless `hide_on_drop=true` |
| `SignalSubscription` | unsubscribe |
| `HookHandle` | unregister hook |

A returned handle lives in the receiving scope. When that scope ends, the handle is dropped. Binding it to `_` drops it immediately.

```arcw
let _ = bgm.play(@bgm.tension, scope=line, drop=fade(300ms))
// BGM is started and then immediately dropped, so it is faded/stopped immediately.
// LSP warns because this is probably a mistake.
```

To persist BGM beyond the line, detach it or use a global scope explicitly.

```arcw
let bgm_handle = alice.say()[始まるよ。[p]]
with:
    let bgm = bgm.play(@bgm.tension, scope=line, drop=fade(300ms))
    out bgm.detach()
```

---

## Line-plan scope

`with:` creates a lexical scope.

```arcw
alice:
    聞いて。[p]
with:
    let local_color = rgb("#ffffff")
    at(0.2s):
        flash(color=local_color)

// local_color is not visible here.
```

Only values exported with `out` from the line plan can escape the line. Borrowed values such as `&'frame T` and `&'lease T` cannot be exported or captured across `at`, `await`, `yield`, or cancellation boundaries.

---

## Stage object handles and preload

Characters expose object-like stage APIs. These APIs return handles that can be scoped, memoized, or preloaded.

```arcw
preload next @flow.alice_intro:
    alice.stage.prefetch(pose=normal, faces=[smile, worried], window=@textbox.0)
    alice.voice_for(@say.alice_intro.001).preload()
    bgm.prepare(@bgm.alice_theme)
```

Within a line:

```arcw
let actor = alice.stage.acquire(scope=line, memo=true)
let pose = actor.pose(normal)
let face = actor.look(smile)
```

The `memo=true` flag allows the stage proxy to reuse loaded sprite atlases, expression meshes, and text-layout assets when the same key is requested again.

```arcw
let actor = memo(scope=scene, key=(@character.alice, pose=normal, theme=env.theme.hash)) {
    alice.stage.acquire(scope=line)
}
```

Preload declarations are hints, not hidden blocking operations. If an asset is not ready at use time, the normal `Need<T, E>` / pending-display rules still apply.

---

## Shadowing speaker names

A line result binding may technically shadow a speaker alias:

```arcw
let (alice, _) = alice.say()[おはよう。[p]] with:
    let actor = alice.stage.acquire(scope=line)
    out (actor, ())
```

This is allowed but discouraged, because after the binding `alice` refers to the stage handle, not the speaker alias. The LSP warns by default. Prefer a distinct name:

```arcw
let (alice_actor, _) = alice.say()[おはよう。[p]] with:
    let actor = alice.stage.acquire(scope=line)
    out (actor, ())
```

---

## Desugaring summary

```arcw
alice[
    おはよう。[p]
]
with:
    at(0.42s): alice.stage.look(smile)
```

becomes conceptually:

```arcw
alice.say()[
    おはよう。[p]
]
with:
    at(0.42s): alice.stage.look(smile)
```

```arcw
alice:
    おはよう。[p]
with:
    at(0.42s): alice.stage.look(smile)
```

also becomes the same call.

```arcw
let (_, cue) = alice.say()[おはよう。[p]] with:
    let cue = at(0.42s): alice.stage.look(smile)
    out (line.voice_handle(), cue)
```

runs `drop_now` on the discarded voice handle immediately after destructuring, while `cue` remains owned by the surrounding scope.

