# 構文概要

## 最小例

```awft
mod game::routes::opening

use game::prelude::*
use game::logic::affection::{has_affection_at_least}

pub flow #flow.opening opening(state: GameState) -> Result<FlowExit, FlowError> {
    alice(id=#say.opening.greeting): おはよう。[p]

    let can_enter_alice = state |> has_affection_at_least(#character.alice, 3)

    choice #choice.opening.first {
        #choice.opening.listen "聞いてみる" if can_enter_alice -> #flow.alice_intro
        #choice.opening.listen_locked "聞いてみる" -> #flow.alice_locked
        #choice.opening.silent "黙っている" -> #flow.quiet_intro
    }
}
```

## 重要な構文ルール

- 型パラメータは `<>`: `List<T>`, `Result<T, E>`。
- Entity 参照は `#foo.bar`、複雑な参照やメンバアクセス前は `#<foo.bar>`。
- コメントリンクは `[[foo.bar]]`。
- 属性は `@id(...)`, `@link<T>(...)`, `@derive(...)`。
- パイプは `|>`。
- パイプ左辺の placeholder は `^`。
- lambda / partial placeholder は `_`。
- 変数は immutable。`mut` は局所的にのみ許可。
- `null` はない。`Option` を使う。

## `#` 参照

```awft
goto #flow.alice_intro
image(#asset.bg.room)
@link<Flow>(#flow.alice_intro, level = soft)
```

境界明示が必要なとき:

```awft
let result = try await #<activity.truck_game>.run(input) with:
    pending p:
        scene #scene.loading:
            progress p.ratio

#<say.opening.dream_hint@sem:b3_9f2a1c>
#<asset:bg/room.ktx2>
```

## `[[...]]` コメントリンク

```awft
/// この台詞は [[flow.alice_intro]] の伏線。
/// Aliceの好感度は [[state.GameState.affection]] で管理する。
```

## `Need` の待機強制

flow 内では、時間のかかる処理に待機時の表示が必須。

```awft
let assets = try await load_opening_assets() with {
    pending p => scene #scene.loading {
        text "Openingを準備中"
        progress p.ratio
    }
    timeout 3s => scene #scene.loading_slow {
        text "少し時間がかかっています"
    }
}
```

## UI の AwaitView

UI でも lazy 値の暗黙 force は禁止。

```awft
AwaitView(typeset(#typeset.credits)) {
    pending p => Text("組版中")
    ready doc => TypesetView(doc)
    error e => Text("表示できません")
}
```



## Object hooks / memoization

```awft
hook #hook.opening.choice_visible
on #choice.opening.listen
phase AfterLayout
check every frame
when object.visible && object.enabled
effects { signal_write, assert }
{
    signal #signal.choice_visible <- true
    debug_assert object.bbox.area > 0
}

memo fn route_title(route: Ref<Flow>) -> String
scope = session
{
    registry.flow(route).title
}
```

詳細は [Object Hooks / Memoization](hooks-and-memoization.md)。

## Object hook

```awft
hook #hook.choice.listen_visible
on #choice.opening.listen
phase VisibilityChanged
check on change
when object.visible
once
{
    signal #signal.choice_visible <- true
}
```

## memo

```awft
memo fn visible_opening_choices(state: GameState) -> List<ChoiceView>
key = (state.route, state.flags, state.affection)
scope = frame
{
    opening_choices()
        .filter(choice_available(state))
        .map(choice_to_view(state))
        .collect<List<ChoiceView>>()
}
```

`memo` は pure な式だけに使える。`Need` の暗黙 force、log/signal 更新、Command 発行、wall-clock 参照は memo 対象では禁止。

## Hook / memo

Object hook と memoization の詳細は [Object Hook / 条件チェック / Memoization](hooks-and-memoization.md) を参照。


## Flow-integrated scenario syntax

For ordinary visual novel writing, Arcweft provides compact line-oriented syntax directly inside `flow`. It is documented in [Flow-Integrated Scenario Syntax / Dialogue Sugar](scenario-surface-syntax.md). Dialogue tags and ruby are documented in [Dialogue Control Tags, Ruby, and Inline Hooks](dialogue-control-tags-and-ruby.md).

```awft
pub flow #flow.opening opening(state: GameState) -> Result<FlowExit, FlowError> {
    @bg #asset.bg.room fade=300ms
    @show alice smile at=center

    地の文: 扉の向こうから、雨の音がした。[p]
    alice: おはよう。[l]
    alice(voice=auto): 今日は少しだけ、｜変な夢《へんなゆめ》を見たんだ。[p]

    choice #choice.opening.first {
        #choice.opening.listen "聞いてみる" -> #flow.alice_intro
        #choice.opening.silent "黙っている" -> #flow.quiet_intro
    }
}
```

There is no separate `script` item. Concise dialogue statements and typed Arcweft statements are both `FlowItem`s.
