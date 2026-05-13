# Opening flow example

```awft
mod game::routes::opening

use game::prelude::*
use game::logic::affection::{has_affection_at_least}
lazy use mini_games::truck::{truck_game, TruckResult}

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

    say #say.opening.dream_hint alice rich """
    今日は少しだけ、{ruby "変な夢" "へんなゆめ"}を見たんだ。
    """ with voice #cue.voice.alice.001

    let choices = opening_choices()
        .filter(choice_available(state))
        .map(choice_to_view(state))
        .collect<List<ChoiceView>>()

    debug_assert choices.len() > 0

    let selected = choice #choice.opening.first {
        for c in choices { option c.id c.label }
    }

    match selected.id {
        #choice.opening.listen => {
            if state |> has_affection_at_least(#character.alice, 3) {
                Ok(FlowExit::Goto(#flow.alice_intro))
            } else {
                let result = try await #<activity.truck_game>.run({ seed = state.seed }) with {
                    pending .Realizing(p) => scene #scene.loading_plugin { progress p.ratio }
                    pending .Running(p) => scene #scene.truck_loading { progress p.ratio }
                }
                if result.rank == .S { Ok(FlowExit::Goto(#flow.secret_route)) }
                else { Ok(FlowExit::Goto(#flow.alice_locked)) }
            }
        }
        #choice.opening.silent => Ok(FlowExit::Goto(#flow.quiet_intro))
    }
}
```

