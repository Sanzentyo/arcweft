# Concise Dialogue, Tags, Ruby, Views, and Localization Example

This example shows Arcweft's flow-integrated dialogue surface. There is no separate `script` item: concise dialogue and typed logic coexist inside `flow`. Ordinary lines use `character:` sugar, while complex lines use the canonical `character.say()[...]` form.

The paired project manifest owns the dialogue presentation used by this module:

```toml
[profiles.game]
kind = "game"
source = "src/opening.arcw"

[profiles.game.dialogue]
view = "view.main_dialogue"
style = "style.main_dialogue"

[profiles.game.dialogue.inline-failure]
kind = "fail_line"
```

```arcw
mod game.routes.opening

use game.prelude.*
use game.characters.{alice}
use tag game.fx.{flash}

preload next @flow.alice_intro {
    alice.prefetch(flow=@flow.alice_intro, lines=6)
    alice.sprite(smile).preload()
    alice.voice_for(@say.alice_intro.001).preload()
    asset.image(@asset:.bg.room_evening)
}

pub flow @flow.opening opening(state: GameState) -> Result<FlowExit, FlowError> {
    bg(@asset:.bg.room, fade = 300ms)
    show(@character.alice, .normal, at = .center, fade = 200ms)

    地の文: 扉の向こうから、雨の音がした。[p]

    alice(id=@say.opening.greeting, look=.smile, voice=auto):
        おはよう、#[player_name]。[l]

    alice.say(id=@say.opening.dream_hint, voice=auto, look=.normal)[
        今日は少しだけ、|[変な夢](へんなゆめ)を見たんだ。[flash time=90ms][p]
    ]
    with {
        at(0.45s) { alice.stage.look(worried, crossfade=120ms) }
        at(end-200ms) { alice.stage.move(to=left, time=260ms, ease=quad.out) }
    }

    let can_enter_alice = state |> has_affection_at_least(@character.alice, 3)

    choice @choice.opening.first {
        @choice.opening.listen "聞いてみる" if can_enter_alice -> @flow.alice_intro
        @choice.opening.silent "黙っている" -> @flow.quiet_intro
    }
}
```

A dialogue-safe custom content interpolation:

```arcw
pub dialogue tag @tag.flash flash(
    color: Color = rgb("#ffffff"),
    time: Duration = 120ms,
) -> Result<DialogueCue, TagError>
effects { stage.flash }
{
    Ok(DialogueCue.Flash { color, time })
}
```

Character style excerpt:

```arcw
pub character @character.alice alice {
    display_name ja-JP = "アリス"
    display_name en-US = "Alice"

    text_style {
        name_color = rgb("#ffb7d5")
        text_color = rgb("#f7e8ff")
        unread_color = rgb("#ffffff")
        read_color = rgb("#b8b8c8")
    }
}
```

Source line registry excerpt:

```toml
[lines."ent_01JABC_OPENING_DREAM"]
kind = "Dialogue"
public_id = "say.opening.dream_hint"
text_key = "text.opening.alice.002"
speaker = "character.alice"
source_locale = "ja-JP"
source_text = "今日は少しだけ、変な夢を見たんだ。"
source_rich_text = "今日は少しだけ、|[変な夢](へんなゆめ)を見たんだ。[flash time=90ms][p]"
source_hash = "b3:f8c0..."
voice_key = "voice.alice.opening.002"
flow = "flow.opening"
```

Long CSV export:

```csv
key,locale,speaker,source_text,target_text,status,source_hash,voice,context,notes
text.opening.alice.001,en-US,character.alice,"おはよう、{player_name}。","Good morning, {player_name}.",translated,b3:91a2...,voice.en-US.alice.opening.001,flow.opening,
text.opening.alice.002,en-US,character.alice,"今日は少しだけ、変な夢を見たんだ。",I had a strange dream today.,draft,b3:f8c0...,voice.en-US.alice.opening.002,flow.opening,"Keep ominous tone."
text.choice.opening.listen,en-US,,聞いてみる,Ask her about it,translated,b3:12cd...,,choice.opening.first,
```

`.arcwloc` equivalent:

```arcw
locale en-US from ja-JP {
    line text.opening.alice.001 {
        speaker = @character.alice
        source = "おはよう、{player_name}。"
        text = "Good morning, {player_name}."
        status = translated
        source_hash = "b3:91a2..."
        voice = @voice.en-US.alice.opening.001
    }

    line text.opening.alice.002 {
        speaker = @character.alice
        source = rich "今日は少しだけ、|[変な夢](へんなゆめ)を見たんだ。"
        text = "I had a strange dream today."
        status = draft
        source_hash = "b3:f8c0..."
        voice = @voice.en-US.alice.opening.002
    }

    line text.choice.opening.listen {
        source = "聞いてみる"
        text = "Ask her about it"
        status = translated
    }
}
```


---

## Bracket call, with-block, and handle return

The same line can be written with bracket speaker-call syntax:

```arcw
alice[
    おはよう、#[player_name]。[l]
]
with:
    at(0.30s):
        alice.stage.look(smile)
```

A complex line may return scoped handles. `_` explicitly discards and drops a returned handle.

```arcw
let (actor, (_, voice)) = alice.say(id=@say.opening.dream_hint, voice=auto)[
    今日は少しだけ、|[変な夢](へんなゆめ)を見たんだ。[flash time=90ms][p]
]
with:
    let actor = alice.stage.acquire(scope=line, memo=true)
    let face = actor.look(normal)
    let voice = line.voice_handle()

    at(0.45s):
        actor.look(worried, crossfade=120ms)

    out (actor, (face, voice))
```

Here the initial `face` handle is intentionally discarded with `_`; if the cue is still pending at destructuring time, Arcweft applies its drop policy.

Preload for a likely next flow can be explicit:

```arcw
preload next @flow.alice_intro:
    alice.stage.prefetch(pose=normal, faces=[smile, worried], view=@view.MainDialogue)
    alice.voice_for(@say.alice_intro.001).preload()
    bgm.prepare(@bgm.alice_theme)
```

