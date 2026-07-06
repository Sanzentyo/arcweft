# Layer / Input example

関連: [Layer System](../03-presentation/layers.md)

```arcw
mod game.presentation.layers

pub layer @layer.world.background: World {
    z = -1000
    input = observe_only
    hit_test = none
}

pub layer @layer.world.characters: Character {
    z = 0
    input = pass_through
    hit_test = bbox
}

pub layer @layer.ui.game: NativeUi {
    z = 1000
    input = block_below on_hit
    hit_test = ui_tree
    focus = ui_tree_order
}

pub layer @layer.ui.modal: Modal {
    z = 3000
    input = modal
    hit_test = ui_tree
    focus = trap
}
```

Scene での利用:

```arcw
flow @flow.opening opening(state: GameState) -> Result<FlowExit, FlowError> {
    scene.show(@scene.opening)
    scope {
        layer @layer.world.background {
            image(@asset:.bg.room).fit(cover)
        }

        layer @layer.world.characters {
            sprite(@asset:.char.alice.default)
                .at(center)
                .agent_target(@character.alice)
        }

        layer @layer.ui.game {
            TextBox(current_text())
            ChoiceList(opening_choices())
        }
    }

    choice @choice.opening.first {
        @choice.opening.listen "聞いてみる" -> @flow.alice_intro
        @choice.opening.silent "黙っている" -> @flow.quiet_intro
    }
}
```

Modal UI:

```arcw
component @ui.settings SettingsPanel(config: Binding<Config>) -> View {
    VStack {
        Text("Settings")
        Button("閉じる")
            .agent_target(@ui.settings.close)
            .on_click { action.invoke(@action.settings.close) }
    }
    .layer(@layer.ui.modal)
}
```

Test:

```arcw
test @test.settings_modal_blocks_world scenario {
    start(@flow.opening)
    invoke(@ui.settings.open)

    input.click(@character.alice)

    expect.input(blocked_by=@layer.ui.modal)
    expect.no_event(GameEvent.CharacterClicked)
}
```

