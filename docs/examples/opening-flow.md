# Opening flow example

```arcw
mod crate.game.routes.opening

use crate.game.prelude.*
use super.logic.affection.{has_affection_at_least}

pub flow @flow.opening opening(state: GameState) -> Result<FlowExit, FlowError> {
    signal.set(@signal.current_flow, @flow.opening)
    log.info("enter flow {flow:?}", flow = @flow.opening)

    preload opening {
        asset.image(@asset:.bg.room)
        cue @cue.voice.alice.001
        shader @shader.transition.dissolve
    }

    let assets = try await load_opening_assets() with {
        pending p => scene.show(@scene.loading); progress.set(p.ratio)
    }

    scene { background image(assets.bg) }

    scope dream {
        alice(id=@.hint, voice=@cue.voice.alice.001):
            今日は少しだけ、|[変な夢](へんなゆめ)を見たんだ。[p]

        let can_enter_alice = state |> has_affection_at_least(@character.alice, 3)

        choice @.first {
            @.listen "聞いてみる" if can_enter_alice -> @flow.alice_intro
            @.truck "トラック勝負で聞き出す" -> @flow.truck_challenge
            @.silent "黙っている" -> @flow.quiet_intro
        }
    }
}
```

The relative IDs in `scope dream` normalize to stable registry IDs:

```text
alice(id=@.hint) -> @say.opening.alice.dream.hint
choice @.first   -> @choice.opening.dream.first
@.listen -> @choice.opening.dream.first.listen
```

