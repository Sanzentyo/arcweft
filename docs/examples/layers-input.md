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

pub layer @layer.view.game: NativeView {
    z = 1000
    input = block_below on_hit
    hit_test = view_tree
    focus = view_tree_order
}

pub layer @layer.view.modal: Modal {
    z = 3000
    input = modal
    hit_test = view_tree
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

        layer @layer.view.game {
            view(@view.MainDialogue)
            ChoiceList(opening_choices())
        }
    }

    choice @choice.opening.first {
        @choice.opening.listen "聞いてみる" -> @flow.alice_intro
        @choice.opening.silent "黙っている" -> @flow.quiet_intro
    }
}
```

Modal View:

```arcw
view SettingsPanel(config: Binding<Config>) {
    Column {
        Text("Settings")
        Button("閉じる")
            .agent_target(@view.settings.close)
            .on_click { action.invoke(@action.settings.close) }
    }
    .layer(@layer.view.modal)
}
```

Test:

```arcw
test @test.settings_modal_blocks_world scenario {
    goto @flow.opening
    invoke(@view.settings.open)

    input.click(@character.alice)

    expect.input(blocked_by=@layer.view.modal)
    expect.no_event(GameEvent.CharacterClicked)
}
```

