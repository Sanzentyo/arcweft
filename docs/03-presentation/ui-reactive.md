# Game Native UI

Game Native UI は SwiftUI 風の宣言的・リアクティブ UI。HTML/CSS とは別に、ゲーム画面、選択肢、HUD、text box、debug overlay、Agent 観測に使う。

## Component

```awft
pub component @ui.settings SettingsPanel(
    config: Binding<Config>,
    props: SettingsProps,
) -> View {
    local state tab: SettingsTab = .Audio

    VStack(spacing = 16) {
        Text("Settings").font(.title)

        Picker(value = bind tab) {
            option .Audio "Audio"
            option .Text  "Text"
            option .Video "Video"
        }

        match tab {
            .Audio => AudioSettings(config = config.audio)
            .Text  => TextSettings(config = config.text)
            .Video => VideoSettings(config = config.video)
        }

        Button("閉じる")
            .agent_target(@ui.settings.close)
            .on_click { event.emit(UiEvent.SettingsClosed) }
    }
    .padding(24)
    .background(.panel)
    .corner_radius(16)
}
```

## Binding

Binding は直接 state を破壊的に書き換えず、lens + event/command。

```awft
Slider(value = bind state.config.master_volume, range = 0.0..1.0)
```

展開:

```rust
Binding<f32> {
    get = .config.master_volume,
    set = |v| GameEvent.Ui(.SetMasterVolume { value = v }),
}
```

## Reactive dependencies

component 評価中に読んだ依存を記録する。

```rust
pub enum UiDependency {
    StatePath(StatePathId),
    Signal(EntityId),
    LocalState(ComponentLocalId),
    Environment(EnvironmentKey),
    Resource(AssetId),
    Font(FontId),
    Locale,
}
```

変更時は該当 component だけ invalidated。

## View / Modifier

```awft
Text("聞いてみる")
    .font(.body)
    .padding(x = 24, y = 12)
    .background(.button)
    .corner_radius(8)
    .transition(.fade(duration = 120ms))
    .animation(.spring, value = is_selected)
```

## AwaitView

UI でも `Need` の暗黙 force は禁止。

```awft
AwaitView(load_avatar(user)) {
    pending _ => SkeletonCircle()
    ready img => Image(img)
    error _ => Icon(@vector.avatar_fallback)
}
```

## Agent output

UI node は bbox / polygon / mask / action target を持つ。

```rust
pub struct UiNode {
    pub entity: Option<EntityId>,
    pub role: UiRole,
    pub label: Option<String>,
    pub bbox: BBox,
    pub polygon: Option<Polygon>,
    pub actions: Vec<ActionTarget>,
}
```



## Memoized component and hooks

UI component は dependency tracking によって必要部分だけ再評価される。高価な派生値には `memo` を使う。

```awft
component @ui.choice_list ChoiceList(state: GameState) -> View {
    memo choices key state.route, state.affection {
        opening_choices()
            .filter(choice_available(state))
            .map(choice_to_view(state))
            .collect<List<ChoiceView>>()
    }

    VStack {
        ForEach(choices, id = _.id) |choice| { ChoiceButton(choice) }
    }
}
```

UI node には hook を付けられる。

```awft
hook @hook.choice_button_has_action
on query UiNode where role == .Choice
phase AfterLayout
check every frame
{
    assert object.actions.contains("select")
}
```
