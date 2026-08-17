# State / Event / Reducer / Flow / View

## State

```arcw
pub struct GameState {
    pub route: Ref<Flow>
    pub config: Config
    pub flags: OrderedSet<Flag>
    pub affection: OrderedMap<Ref<Character>, i32>
    pub current_bg: Option<ImageHandle>
}

fn initial_game_state() -> GameState
effects {}
{
    GameState {
        route = @flow.opening,
        config = Config {},
        flags = {},
        affection = {},
        current_bg = None,
    }
}
```

グローバル状態は本物の mutable global ではなく、通常の nominal type を
root state として entry から選択し、typed lens で扱う。`state` 専用宣言は
使わない。

## Event

```arcw
pub enum GameEvent {
    StartGame,
    TextAdvanced,
    ChoiceSelected { id: Ref<ChoiceOption> },
    TruckFinished { result: TruckResult },
    Action(SemanticAction),
    Task(TaskEvent),
}
```

## Reducer

```arcw
pub fn update(state: &GameState, event: GameEvent)
    -> Result<Reduction<GameState>, ReducerError>
effects {}
{
    match event {
        .ChoiceSelected { id: @choice.opening.listen } => {
            Ok(Reduction.commit(
                state.add_affection(@character.alice, 1),
                [flow.goto(@flow.alice_intro)],
            ))
        }
        _ => Ok(Reduction.unchanged(state))
    }
}
```

Reducer は通常の `fn` で、`await` 禁止。必要なら `Reduction` の command
列として `Command` / `Task` を返す。root reducer であることは専用 keyword
ではなく、次の typed entry binding が決める。

```arcw
entry game @entry.game.main {
    state = GameState
    initializer = initial_game_state
    event = GameEvent
    reducer = update
    goto @flow.opening
}
```

## Flow

Flow は逐次進行で、suspend/resume 可能。

```arcw
pub flow opening(state: GameState) -> Result<FlowExit, FlowError> {
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

```arcw
pub view current_scene(state: GameState) -> Scene {
    scene {
        layer bg = image(asset = @asset:.bg.room, id = "image.scene.bg", x = 0px, y = 0px, width = 1280px, height = 720px, fit = "cover")
        layer text = view(@view.MainDialogue)
    }
}
```

View は `await` 禁止。`Need` はordinary `match`で観測し、View semaが
retained subscription/branchへprojectする。`AwaitView`専用surfaceや
untyped fallbackは使わない。

## Reduction

```arcw
pub struct Reduction<S> {
    pub state: S,
    pub commands: Vec<Command>,
}
```

Reducer は candidate state と順序付き command 列を `Reduction` として返す。
成功した値だけが一つの transaction として commit される。状態値そのものの
更新は persistent chain を使う。

```arcw
state
    .set(.config.text_speed, value.clamp(0.1, 3.0))
    .update(.affection[@character.alice], |v| v + 1)
```



