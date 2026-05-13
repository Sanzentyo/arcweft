# Opening flow example

```awft
mod game::routes::opening

use game::prelude::*
use game::logic::affection::{has_affection_at_least}

pub flow #flow.opening opening(state: GameState) -> Result<FlowExit, FlowError> {
    signal #signal.current_flow <- #flow.opening
    log info "enter flow {flow:?}" { flow = #flow.opening }

    preload opening {
        asset.image(#asset.bg.room)
        cue #cue.voice.alice.001
        shader #shader.transition.dissolve
    }

    let assets = try await load_opening_assets() with {
        pending p => scene #scene.loading { progress p.ratio }
    }

    scene { background image(assets.bg) }

    alice.say(id=#say.opening.dream_hint, voice=#cue.voice.alice.001)[
        今日は少しだけ、｜変な夢《へんなゆめ》を見たんだ。[p]
    ]

    let can_enter_alice = state |> has_affection_at_least(#character.alice, 3)

    choice #choice.opening.first {
        #choice.opening.listen "聞いてみる" if can_enter_alice -> #flow.alice_intro
        #choice.opening.truck "トラック勝負で聞き出す" -> #flow.truck_challenge
        #choice.opening.silent "黙っている" -> #flow.quiet_intro
    }
}
```
