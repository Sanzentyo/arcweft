# Example: Object Hooks and Memoization

```awft
mod game::routes::opening

use game::prelude::*
use game::logic::affection::{has_affection_at_least}

@memo(scope = state, key = [state.affection[#character.alice]])
fn alice_route_ready(state: GameState) -> Bool {
    state |> has_affection_at_least(#character.alice, 3)
}

pub hook #hook.opening.listen_enable
on #choice.opening.listen
phase = input.hit_test
check = on_change(state.affection[#character.alice])
memo condition scope = state key = [state.affection[#character.alice]]
when alice_route_ready(state)
effects { ui.enable, log.debug }
{
    emit UiCommand::EnableTarget { target = #choice.opening.listen }
    log debug "listen choice enabled" {}
}

pub hook #hook.opening.listen_hover
on #choice.opening.listen
phase = input.target
check = every_frame
when input.pointer.hovered
effects { ui.style }
{
    emit UiCommand::SetClass {
        target = #choice.opening.listen,
        class = "hover",
        enabled = true,
    }
}

pub hook #hook.opening.listen_agent_note
on #choice.opening.listen
phase = agent.observed
check = on_change(state.affection[#character.alice])
when !alice_route_ready(state)
effects { agent.annotate }
{
    agent.annotate(#choice.opening.listen) {
        reason = "Alice route requires affection >= 3"
        current = state.affection[#character.alice]
    }
}
```

Debug:

```bash
arcw hook explain hook.opening.listen_enable
arcw memo inspect --function alice_route_ready
arcw agent observe --target choice.opening.listen --json
```
