# Example: Object Hooks and Memoization

```awft
mod game::routes::opening

use game::prelude::*
use game::logic::affection::{has_affection_at_least}

memo fn alice_route_ready(state: GameState) -> Bool
scope = state
key = [state.affection[@character.alice]]
{
    state |> has_affection_at_least(@character.alice, 3)
}

pub hook @hook.opening.listen_enable
on @choice.opening.listen
phase InputHitTest
check on change state.affection[@character.alice]
memo condition scope = state key = [state.affection[@character.alice]]
when alice_route_ready(state)
effects { ui.enable, log.debug }
{
    emit UiCommand::EnableTarget { target = @choice.opening.listen }
    log debug "listen choice enabled" {}
}

pub hook @hook.opening.listen_hover
on @choice.opening.listen
phase InputTarget
check every frame
when input.pointer.hovered
effects { ui.style }
{
    emit UiCommand::SetClass {
        target = @choice.opening.listen,
        class = "hover",
        enabled = true,
    }
}

pub hook @hook.opening.listen_agent_note
on @choice.opening.listen
phase AgentObserved
check on change state.affection[@character.alice]
when !alice_route_ready(state)
effects { agent.annotate }
{
    agent.annotate(@choice.opening.listen) {
        reason = "Alice route requires affection >= 3"
        current = state.affection[@character.alice]
    }
}
```

Debug:

```bash
arcw hook explain hook.opening.listen_enable
arcw memo inspect --function alice_route_ready
arcw agent observe --target choice.opening.listen --json
```
