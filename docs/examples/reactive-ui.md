# Reactive UI example

```arcw
pub view ChoiceButton(choice: ChoiceView)
requires choice.label.len() > 0
ensures result.has_action("select")
{
    Button {
        Row(spacing = 12) {
            Vector(@vector.icon.choice_arrow).size(18)
            RichText(choice.label).font(.body)
        }
    }
    .agent_target(choice.id)
    .padding(x = 24, y = 12)
    .background(if choice.enabled { .button } else { .button_disabled })
    .corner_radius(8)
    .on_click {
        if choice.enabled {
            event.emit(GameEvent.ChoiceSelected, id = choice.id)
        }
    }
}
```

