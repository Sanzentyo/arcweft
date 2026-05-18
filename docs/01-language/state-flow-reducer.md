# State / Event / Reducer / Flow / View

## State

```awft
pub state GameState {
    pub route: Ref<Flow> = @flow.opening
    pub config: Config = Config {}
    pub flags: Set<Flag> = {}
    pub affection: Map<Ref<Character>, i32> = {}
    pub current_bg: Option<ImageHandle> = None
}
```

グローバル状態は本物の mutable global ではなく、root state と typed lens で扱う。

## Event

```awft
pub enum GameEvent {
    StartGame,
    TextAdvanced,
    ChoiceSelected { id: Ref<ChoiceOption> },
    TruckFinished { result: TruckResult },
    Ui(UiEvent),
    Task(TaskEvent),
}
```

## Reducer

```awft
pub reducer update(state: GameState, event: GameEvent) -> Result<Update<GameState>, GameError> {
    match event {
        .ChoiceSelected { id: @choice.opening.listen } => {
            Ok(
                state
                    .add_affection(@character.alice, 1)
                    .to_update()
                    .cmd(flow.goto(@flow.alice_intro))
            )
        }
        _ => Ok(state.to_update())
    }
}
```

Reducer は `await` 禁止。必要なら `Command` / `Task` を返す。

## Flow

Flow は逐次進行で、suspend/resume 可能。

```awft
pub flow @flow.opening opening(state: GameState) -> Result<FlowExit, FlowError> {
    alice(id=@say.opening.greeting): おはよう。[p]

    choice @choice.opening.first {
        @choice.opening.listen "聞いてみる" -> @flow.alice_intro
        @choice.opening.silent "黙っている" -> @flow.quiet_intro
    }
}
```

Flow 内の `await` は `pending` branch 必須。

## View

View は状態から描画仕様を作る純粋関数。

```awft
pub view current_scene(state: GameState) -> Scene {
    scene {
        layer bg = image(@asset.bg.room)
        layer text = TextBox(current_text())
    }
}
```

View は `await` 禁止。`Need` は `AwaitView` や fallback を使う。

## Update

```awft
pub struct Update<S> {
    pub state: S,
    pub commands: Vec<Command>,
}
```

状態更新は persistent chain。

```awft
state
    .set(.config.text_speed, value.clamp(0.1, 3.0))
    .update(.affection[@character.alice], |v| v + 1)
```


