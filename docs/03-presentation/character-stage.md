# Character Stage, Sprites, Looks, and Voice-Synchronized Direction

Arcweft treats character presentation as a typed stage graph, not as ad-hoc image commands.

## Character definition

```arcw
pub surface character @character.alice Alice as alice {
    display_name ja-JP = "アリス"
    display_name en-US = "Alice"

    voice {
        default_locale = ja-JP
        speaker = @speaker.alice
        tts_profile = @tts.alice
    }

    sprite_sheet @sprite.alice.standing {
        base = @asset.char.alice.body

        part body {
            normal = @asset.char.alice.body.normal
        }

        part look {
            normal = @asset.char.alice.face.normal
            smile = @asset.char.alice.face.smile
            worried = @asset.char.alice.face.worried
            surprised = @asset.char.alice.face.surprised
        }

        part mouth {
            closed = @asset.char.alice.mouth.closed
            a = @asset.char.alice.mouth.a
            i = @asset.char.alice.mouth.i
            u = @asset.char.alice.mouth.u
            e = @asset.char.alice.mouth.e
            o = @asset.char.alice.mouth.o
        }
    }

    look .normal { part.look = normal; mouth = closed }
    look .smile { part.look = smile; mouth = closed }
    look .worried { part.look = worried; mouth = closed }
}
```

## Stage object

When a character is shown, the runtime creates a stage object:

```text
StageObject<CharacterSprite>
  entity: @character.alice
  instance: @stage.alice.main
  layer: @layer.characters
  transform: position / scale / rotation / skew
  look: smile
  pose: standing
  active animations
  voice/lip-sync binding
```


## Character object API

Character aliases are typed objects in Arcweft. They expose presentation, voice, memoized asset, and preload methods.

```arcw
alice.say()[おはよう。[p]]
alice.stage.show(.smile, at=.center, fade=200ms)
alice.stage.look(.worried, crossfade=120ms)
alice.stage.move(to=left, time=300ms, ease=quad.out)
alice.stage.scale(1.08, time=240ms)
alice.stage.animate(@anim.breath.loop)
alice.voice(@voice.alice.001).play()
alice.sprite(.smile).preload()
alice.prefetch(flow=@flow.alice_intro, lines=6)
```

Character staging uses ordinary calls and object methods. There is no
separate `@show`, `@face`, `@move`, `@scale`, `@rotate`, or `@anim` command
family.

```arcw
show(@character.alice, .smile, at = .center, fade = 200ms)
```

means:

```arcw
alice.stage.show(.smile, at = .center, fade = 200ms)
```

`look` is the canonical line and stage option. Older `face = ...` examples are
not canonical; authoring tools should migrate them to `look = ...` or
look-patch operands.

```arcw
alice(look=.smile & .casual & .motion.nod, voice=auto):
    おはよう。[p]
```

Object methods are effect-checked. `stage` methods affect the character stage object, `sprite` and `voice` methods return handles or `Need<...>` values, and `prefetch` schedules preload tasks.

## Show / hide

```arcw
show(@character.alice, .smile, at = .center, fade = 200ms)
show(@character.alice, .worried, at = (0.35, 0.92), scale = 1.05, z = 10)
hide(@character.alice, fade = 180ms)
```

`show(...)` returns a `StageHandle<CharacterSurface>`. The visible stage object
and the handle have the same scope lifetime by default. When the handle is
dropped at scope exit, the runtime clears the registered slot unless the handle
was detached or moved to a longer scope.

Object equivalent:

```arcw
alice.stage.show(smile, at=center, fade=200ms)
```

The default target and slot are:

```text
target = @target.scene
slot   = @slot.character.{character}.default
```

Multiple instances of a character require explicit slots:

```arcw
let alice_main = show(@character.alice, .smile, slot = @slot.character.alice.main)
let alice_mirror = show(@character.alice, .worried, slot = @slot.character.alice.mirror)

let current_main = ref show(@character.alice, slot = @slot.character.alice.main)
let removed_main = hide(@character.alice, slot = @slot.character.alice.main)
```

`ref show(...)` only reads the slot. It does not create or retain a stage
object. `hide(...)` is the clear operation paired with `show(...)`; it returns
the removed handle/value when the slot was occupied.

## Face and pose change

```arcw
face(@character.alice, .worried, crossfade = 120ms)
pose(@character.alice, .hands_front, time = 160ms)
mouth(@character.alice, .closed)
```

look changes are patch operations on sprite parts. They do not recreate the entire character object.

## Movement and transform

```arcw
move(@character.alice, to = .left, time = 350ms, ease = cubic.out)
move(@character.alice, by = (-0.08, 0.0), time = 200ms)
scale(@character.alice, 1.08, time = 240ms)
rotate(@character.alice, -2deg, time = 180ms)
transform(@character.alice) {
    position = (0.30, 0.92)
    scale = (1.04, 1.04)
    skew = (0deg, 0deg)
    time = 300ms
    ease = quad.out
}
```

Transforms are deterministic animation tracks sampled from logical time.

## Animation commands

```arcw
anim(@character.alice, @anim.breath, mode=loop)
anim(@character.alice, @anim.step_forward, mode=once)
stop_anim(@character.alice, @anim.breath)
```

Animations can target transform, opacity, sprite part selection, shader params, or vector/text effects.

## Voice-synchronized staging

Dialogue line timeline:

```arcw
alice.say(id=@say.opening.003, look=smile, voice=auto)[
    ほら、ここ。覚えてる？[p]
] with {
    at(0.15s) { alice.stage.look(normal, crossfade=80ms) }
    at(0.45s) { alice.stage.move(by=(0.04, 0.0), time=180ms) }
    at(marker("soft_smile")) { alice.stage.look(smile, crossfade=100ms) }
    at(end-120ms) { alice.stage.animate(@anim.breath.once) }
}
```

The scheduler drives the timeline from the voice cue if present. If the voice is missing, the same timeline may fall back to text reveal timing depending on policy.

## Lip sync

```arcw
alice.say(id=@say.opening.004, voice=@voice.alice.004, lipsync=auto)[
    夢の中では、君もそこにいた。[p]
]
```

Lip-sync modes:

| Mode | Behavior |
|---|---|
| `none` | no mouth animation |
| `auto` | derive mouth motion from amplitude/phoneme estimator |
| `markers` | use phoneme markers from sidecar data |
| `manual` | timeline controls mouth part |

Manual mouth timeline:

```arcw
alice.say(id=@say.opening.005, voice=auto, lipsync=manual)[
    あのね。[p]
] with {
    at(phoneme "a") { alice.stage.mouth(a) }
    at(phoneme "o") { alice.stage.mouth(o) }
    at(end) { alice.stage.mouth(closed) }
}
```

## Agent and test observation

Each stage object exposes:

```text
- entity id
- instance id
- current look
- current pose
- bbox / polygon / mask
- active animation tracks
- current voice cue
- timeline cursor
```

Agent observation example:

```json
{
  "entity": "character.alice",
  "instance": "stage.alice.main",
  "look": "worried",
  "voice": "voice.ja.alice.003",
  "timeline_time_ms": 450,
  "bbox": [330, 80, 420, 640]
}
```

## Contracts

```arcw
component CharacterStageObject(character: Ref<Character>) -> View
ensures result.has_bbox()
ensures result.agent_observable == true
{
    ...
}
```

Dialogue timelines can also be validated:

```arcw
verify @proof.dialogue_timeline_bounds {
    for_all line in DialogueLine {
        line.timeline.events.all(_.time >= 0ms)
    }
}
```

---

## Dialogue `at(...) { ... }` integration

Character stage cues can be scheduled from dialogue line plan blocks, or from colon sugar with `with { ... }`.

```arcw
alice.say(voice=auto)[
    今日は少しだけ、|[変な夢](へんなゆめ)を見たんだ。[p]
] {
    at(0.42s) { alice.stage.look(worried, crossfade=120ms) }
    at(end-200ms) { alice.stage.move(to=left, time=260ms, ease=quad.out) }
    at(marker("surprise")) { alice.stage.shake(strength=0.35, time=180ms) }
}
```

These cues are compiled to `StageTimelineEvent` records owned by the dialogue line. They are replayable, cancellable, and visible to Agent Debug Bus.

Cancellation policy can stop or complete pending stage cues:

```arcw
alice.say(voice=auto)[
    まだ話している途中……[p]
] {
    cancel on input .SkipLine {
        'line.voice |> drop(stop_now)
        cues.stop(policy = .CompleteCurrent)
        text.flush(mode = .Instant)
        continue
    }
}
```

Cue cancellation policies:

| Policy | Meaning |
|---|---|
| `.CancelPending` | cancel all cues not yet started |
| `.CompleteCurrent` | finish currently active transition, cancel future cues |
| `.SnapToFinal` | apply final transform/look immediately |
| `keep_running` | do not cancel stage cues |


---

## Object-style character API

Character aliases act like object handles in flow code. This is syntax over typed stage commands, not mutable global object state.

```arcw
alice.show(look=smile, at=center, fade=200ms)
alice.look(worried, crossfade=120ms)
alice.pose(hands_front, time=160ms)
alice.mouth(closed)
alice.move(to=left, time=350ms, ease=cubic.out)
alice.scale(1.08, time=240ms)
alice.rotate(-2deg, time=180ms)
alice.animate(@anim.breath, mode=loop)
alice.hide(fade=180ms)
```

When a stage instance must be specified explicitly:

```arcw
alice.stage(@stage.alice.main).move(to=left, time=300ms)
@<character.alice>.stage(@stage.alice.sub).look(smile)
```

Ordinary calls are sugar over this API:

```arcw
show(@character.alice, .smile, at = .center, fade = 200ms)
face(@character.alice, .worried, crossfade = 120ms)
move(@character.alice, to = .left, time = 350ms, ease = cubic.out)
```

The object-style API is allowed because it preserves Arcweft's deterministic state model:

```text
alice.look(worried)
  -> StageCommand::Setlook(character.alice, worried)
  -> applied at frame boundary
  -> observable in replay and Agent Debug Bus
```

---

## Character preload policy

Characters can declare how their sprite parts, looks, lip-sync data, and voices should be prepared.

```arcw
pub character @character.alice Alice {
    preload_policy {
        sprites = on_flow_anticipate
        looks = [normal, smile, worried, surprised]
        mouth_parts = on_voice_line
        voices = locale_current
        lipsync = metadata_only
    }
}
```

Explicit character preload:

```arcw
preload character alice {
    looks [normal, smile, worried]
    voices for flow @flow.alice_intro locale current
    sprites scale_buckets [1.0, 1.25]
}
```

Read-ahead for a likely next flow:

```arcw
anticipate @flow.alice_intro {
    alice.preload(looks=[smile, worried], voices=auto, sprites=true)
    asset.preload(@asset.bg.room_evening)
    shader.preload(@shader.transition.dissolve)
}
```

`anticipate` starts tasks early and may use the scheduler's lower priority queues. It never changes story state. If the anticipated branch is not taken, pending preload tasks may be cancelled according to policy.

---

## Character memoization

Expensive character presentation work is memoized by stable keys.

```arcw
pub character @character.alice Alice {
    memo_policy {
        compose_sprite key=(pose, look, mouth, scale_bucket, theme_hash) cache=session
        lipsync_plan key=(voice_key, locale) cache=session
        look_patch key=(from_look, to_look) cache=flow
    }
}
```

Runtime memo entries are typed:

```text
Memo<SpriteComposite>
Memo<LipSyncPlan>
Memo<lookPatch>
```

They are invalidated by:

```text
- asset hash change
- locale change for voice/lip-sync dependent entries
- theme or color policy change
- renderer scale bucket change
- hot reload of sprite definitions
```

Agent observation exposes memo status for stage objects:

```json
{
  "entity": "character.alice",
  "memo": {
    "compose_sprite": "hit",
    "lipsync_plan": "pending"
  }
}
```

---

## Character methods inside dialogue lines

Dialogue line plan and `at(...) { ... }` blocks can call character stage methods.

```arcw
alice.say(voice=auto)[
    ほら、ここ。覚えてる？[p]
] {
    at(0.15s) { alice.stage.look(normal, crossfade=80ms) }
    at(0.45s) { alice.stage.move(by=(0.04, 0.0), time=180ms) }
    at(marker("soft_smile")) { alice.stage.look(smile, crossfade=100ms) }
    at(end-120ms) { alice.stage.animate(@anim.breath.once) }
}
```

These calls create timeline cues attached to the line. They are cancelled or completed according to the line's cancellation policy.

---

## Memoized sprite and voice handles

Character presentation resources are memoized by character, pose, look, locale, scale policy, and render target.

```arcw
memo fn Character.sprite(
    self: Ref<Character>,
    look: look,
    pose: Pose = standing,
) -> Need<Result<SpriteHandle, SpriteError>, TaskError>
cache character_sprite_cache
key (self, look, pose, env.locale, env.render_profile)
{
    ...
}
```

The object API hides the memo function behind concise calls:

```arcw
let smile = try await alice.sprite(smile) with {
    pending p => scene.show(@scene.loading_sprite); progress.set(p.ratio)
}

alice.stage.show(smile, at=center)
```

`alice.stage.show(smile, ...)` may request the same memoized sprite handle internally. If the handle is not ready and the line/flow is visible to the player, `Need<T, E>` pending rules apply.

---

## Explicit next-flow preloading

A flow can declare what to preload for likely next flows.

```arcw
preload next @flow.alice_intro {
    alice.prefetch(flow=@flow.alice_intro, lines=6)
    alice.sprite(smile).preload()
    alice.voice_for(@say.alice_intro.001).preload()
    asset.image(@asset.bg.room_evening)
}
```

A character may also declare policy-level hints:

```arcw
pub character @character.alice alice {
    preload_policy {
        next_flow_lookahead = 6.lines
        looks = [smile, worried, normal]
        voice = auto_locale
        memoize_sprites = true
    }
}
```

Preload is a hint. If the player reaches a line before preload completes, the normal `Need` pending/cancel behavior is used.

---

## Stage hooks

Stage objects support hooks. These can be used for read-state visual changes, automatic lip sync, debug overlays, or look defaults.

```arcw
hook @hook.character.unread_glow
on query StageObject where entity == @character.alice
phase before_render
check on change ctx.dialogue.read_state
{
    if ctx.dialogue.read_state == .Unread {
        return StagePatch::ShaderParam { name = "glow", value = 0.18 }
    }

    StagePatch::None
}
```

Common built-ins:

```text
@hook.character.auto_lipsync
@hook.character.look_from_dialogue
@hook.character.prefetch_next_look
@hook.character.agent_bbox_debug
```

