# Example: Control Flow, Patterns, Await, and Loops

```awft
mod game::routes::control_flow_example

use game::prelude::*

pub flow @flow.control_flow_example example(state: GameState) -> Result<FlowExit, FlowError> {
    let target = if state.affection[@character.alice] >= 3 {
        @flow.alice_intro
    } else {
        @flow.alice_locked
    }

    let selected = try await wait_choice(@choice.opening.first) with:
        pending p:
            scene @scene.wait_choice:
                progress p.ratio

    let route = match selected.id {
        @choice.opening.listen when state.affection[@character.alice] >= 3 => @flow.alice_intro
        @choice.opening.listen => @flow.alice_locked
        @choice.opening.silent => @flow.quiet_intro
        _ => target
    }

    let .Some(save) = state.current_save else {
        goto @flow.new_game
    }

    while let .Some(event) = state.event_queue.pop_front() {
        match event {
            .ChoiceSelected { id } => log info "choice {id:?}" { id = id }
            .Ui { event: ui_event } => handle_ui(ui_event)
            _ => ()
        }
    }

    let next = loop {
        let event = await_input_event()

        match event {
            .ChoiceSelected { id } => break route_for_choice(id)
            .BackToTitle => break @flow.title
            _ => continue
        }
    }

    goto next
}
```

Function example with optional semicolon:

```awft
fn score_bonus(score: i32) -> i32 {
    if score >= 10 {
        3
    } else {
        0
    }
}

fn debug_score(score: i32) -> Unit {
    log info "score={score}" { score = score };
}
```
