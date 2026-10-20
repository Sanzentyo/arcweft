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

State と Event の型は、型引数を適用した nominal 定義のグラフで検証する。
再帰的なフィールドも同じ型の定義へ接続し、型名だけの葉には置き換えない。
initializer の結果、入力 event、reducer が確定する state、保存から復元する
state は、実行中のプログラムが持つ型定義と layout に従う。entry binding に
別の schema のコピーを持たせない。

Root の所有状態は、state 値をまだ持たない初期化中と、committed state を
一つ保持する初期化済みに分ける。初期化済みの Root は reducer の待機／実行を
区別し、その切替のために state を複製しない。reducer frame への入力に必要な
state の Copy は、元 program の型・layout・資源上限を検証した後に行う。
event はその frame へ一度だけ移し、戻り値の state と command 列をすべて
検証してから一括で確定する。

初期化中の保存は、選択 entry、初期 Flow、state parameter の座標と実際の
initializer frame の継続位置を保持する。仮の state や constructor 内の再実行で
復元を補わない。通常保存は実行中 reducer、待機 event、未 dispatch command を
拒否し、命令単位の rollback image は実行中 phase と元の committed state を
保持する。保存契約は version `1` の形をその場で更新し、旧 reader は残さない。
initializer と reducer は、選択済み backend・formatter と同じ step allowance を
使う通常の executable frame で進める。budget による中断は言語上の `await`
とは区別する。

Root と、それを初期化・更新する実際の fiber は、一つの executor snapshot に
含めて保存・復元する。Root の状態だけを後から差し替えて対応を補わない。
復元対象全体を検証し、失敗時は元の Root・fiber・queue を保持する。
reducer の trap は committed state と transition cursor を変えず、Root に failure を
保持する。通常保存もその failure を保存・復元し、復元によって実行可能な Root に
戻さない。failure の phase・sequence・message digest は復元前に照合する。

実行中の reducer は、その entry の reducer role に対応する一つの executor
fiber が所有する。継続中は通常 Flow と後続 event より優先して進め、budget を
使い切った時点の frame と元 state を保持する。rollback は phase、entry、role、
関数本体と fiber の対応をすべて検証してから復元する。既に完了した外部呼出しを
復元時に再実行しない。式本体と純粋な executable 本体は同じ入力・戻り値契約を
満たす通常関数として扱う。

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

Flow の実行 contract は、受理済みの body invocation と complete formal layout を
同じ閉じた解析環境の証拠として保持する。未使用 parameter の型・passing 区分も
保持し、local の使用箇所や frame ingress の保証から再構成しない。runtime への
投影では、body の Flow owner と解析世代の一致を検査する。公開 effects は
未使用 permission や scoped coverage も保持し、body の実行 effects と区別する。別の Flow の
証拠や、mutation intent の証拠を Flow invocation として受理しない。

Core の Flow invocation schema は、whole formal の `Value`／`Shared`／`Affine`
passing 区分を必須で保持する。Entry や stateful Entry の schema も同じ受理済み
formal から区分を取得する。frame の `Owned`／`Shared`／`Mutable` ingress mode と
区分を混同せず、保存された schema に区分が欠けている場合は復元を拒否する。

`entry bench` の direct `goto` は、測定 adapter が complete formal inventory を
明示入力する Flow を指定できる。CLI は `--value` を同じ Flow invocation schema
へ一度だけ解決し、欠落・重複・未知名・型不一致を実行前に拒否する。反復測定は
全入力が unrestricted の場合だけ同じ immutable plan の invocation を再実行する。
通常の direct Entry startup は引数を補わず、引数を必要とする Flow の開始を拒否する。
CLI／Server／Activity／custom の direct `goto` は引き続き zero-parameter とし、
Server route の明示 binding や stateful Entry の typed startup と混同しない。

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

View は `await` 禁止。unary `Need<T>` はordinary `match`で時間状態を観測し、
View semaがretained subscription/branchへprojectする。domain failureは
`Ready(Result::Err(...))`としてpayload側で処理する。`AwaitView`専用surfaceや
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



