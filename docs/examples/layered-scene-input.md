# Example: layered scene and input

```awft
mod game::routes::opening

use game::prelude::*

layer @layer.world: World {
    z = 0
    input = passthrough
}

layer @layer.dialogue: Group {
    z = 100
    input = hit_test
}

layer @layer.choices: Choice {
    z = 120
    input = hit_test
    hit_test = ui_layout
}

layer @layer.settings_modal: Modal {
    z = 1000
    input = modal
    backdrop = consume
}

pub flow @flow.opening opening(state: GameState) -> Result<FlowExit, FlowError> {
    scene @scene.opening {
        layer @layer.world {
            image @asset.bg.room fit cover
            sprite @asset.char.alice.default at center
        }

        layer @layer.dialogue {
            TextBox(current_text())
        }

        layer @layer.choices {
            ChoiceList(opening_choices())
        }
    }

    choice @choice.opening.first {
        @choice.opening.listen "聞いてみる" -> @flow.alice_intro
        @choice.opening.silent "黙っている" -> @flow.quiet_intro
    }
}
```

## Modal example

```awft
component @ui.settings SettingsPanel(config: Binding<Config>) -> View {
    VStack {
        Text("Settings")

        Slider(value = bind state.config.master_volume, range = 0.0..1.0)
            .agent_target(@ui.settings.volume)

        Button("閉じる")
            .agent_target(@ui.settings.close)
            .on_click { event.emit(UiEvent.SettingsClosed) }
    }
    .layer(@layer.settings_modal)
}
```

## Test

```awft
test @test.layered_input_blocks_lower scenario {
    start @flow.opening

    open_ui @ui.settings

    choose @choice.opening.listen
    expect no_event GameEvent::ChoiceSelected

    invoke @ui.settings.close "click"
    choose @choice.opening.listen
    expect event GameEvent::ChoiceSelected { id: @choice.opening.listen }
}
```
