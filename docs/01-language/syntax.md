# 構文概要

## 最小例

```awft
mod crate::game::routes::opening

use crate::game::prelude::*
use super::logic::affection::{has_affection_at_least}

#[derive(StableHash)]
pub flow @flow.opening opening(state: GameState) -> Result<FlowExit, FlowError> {
    bg(@asset.bg.room, fade = 300ms)
    show(@character.alice, .smile, at = .center, fade = 220ms)

    scope greeting {
        alice(id=@.opening): おはよう。[p]
    }

    let threshold: i32 = 3
    let can_enter_alice: bool =
        state |> has_affection_at_least(@character.alice, threshold)

    scope dream {
        choice @.first {
            @.listen "聞いてみる" if can_enter_alice -> @flow.alice_intro
            @.listen_locked "聞いてみる" -> @flow.alice_locked
            @.silent "黙っている" -> @flow.quiet_intro
        }
    }
}
```

## 重要な構文ルール

- 型パラメータは `<>`: `List<T>`, `Result<T, E>`。
- Entity 参照は `@foo.bar`、複雑な参照やメンバアクセス前は `@<foo.bar>`。
- `@.suffix` / `@..suffix` / `@...suffix` / `@super.suffix` は dialogue line / choice / option / text key のような ID 文脈だけで使う相対 ID。
- module / import の相対指定は `self::` / `super::` / `crate::` を使う。
- コメントリンクは `[[foo.bar]]`。
- 属性は Rust 風の `#[derive(...)]`, `#[link(...)]`。
- `#` は `#[...]` attribute opener の一部であり、entity ref ではない。
- パイプは `|>`。
- パイプ左辺の placeholder は `^`。
- lambda / partial placeholder は `_`。
- 変数は immutable。`mut` は局所的にのみ許可。
- `null` はない。`Option` を使う。
- 数値 primitive は `i32`, `u64`, `f32` のように bit 幅を明示する。
- 型推論の期待型がない数値 literal は `10i32`, `2.0f32` のような suffix が必要。

## `@` 参照

```awft
goto @flow.alice_intro
image(@asset.bg.room)
#[link(Flow, @flow.alice_intro, level = .soft)]
```

境界明示が必要なとき:

```awft
let result = try await @<activity.truck_game>.run(input) with:
    pending p:
        scene @scene.loading:
            progress p.ratio

@<say.opening.dream_hint@sem:b3_9f2a1c>
@<asset:bg/room.ktx2>
```

Relative ID forms:

```awft
alice(id=@.opening): おはよう。[p]
alice(id=@..shared): 共有スコープの台詞です。[p]
alice(id=@...outer_shared): さらに外側のスコープの台詞です。[p]
alice(id=@super.super.outer_shared): 明示形でも同じです。[p]
```

`@.suffix` resolves in the current ID scope. Each extra dot walks one named ID
scope outward, so `@...suffix` means two levels up. `@super.suffix` and
`@super.super.suffix` are the explicit readable spellings. Bare `.suffix` and
`..suffix` are not syntax.

## Literals and primitive values

```awft
let count: i32 = 10
let explicit = 10i32
let alpha: f32 = 2.0
let exact = 2.0f32
let font_size: Length = 100pt
let fade: Duration = 300ms
let angle: Angle = 1.57079632679rad
let opacity: Ratio = 85%
let color: Color = "#1e1e2ecc"
```

`100pt`, `24px`, `50%`, `90deg`, `1rad`, `0.25turn`, `-6db`, and `92bpm`
are unit-number literals. `"#fff"` is a string literal and becomes `Color`
only when the expected type is `Color`.

## `[[...]]` コメントリンク

```awft
/// この台詞は [[flow.alice_intro]] の伏線。
/// Aliceの好感度は [[state.GameState.affection]] で管理する。
```

## `Need` の待機強制

flow 内では、時間のかかる処理に待機時の表示が必須。

```awft
let assets = try await load_opening_assets() with {
    pending p => scene @scene.loading {
        text "Openingを準備中"
        progress p.ratio
    }
    timeout 3s => scene @scene.loading_slow {
        text "少し時間がかかっています"
    }
}
```

## UI の AwaitView

UI でも lazy 値の暗黙 force は禁止。

```awft
AwaitView(typeset(@typeset.credits)) {
    pending p => Text("組版中")
    ready doc => TypesetView(doc)
    error e => Text("表示できません")
}
```



## Object hooks / memoization

```awft
hook @hook.opening.choice_visible
on @choice.opening.listen
phase AfterLayout
check every frame
when object.visible && object.enabled
effects { signal_write, assert }
{
    signal.set(@signal.choice_visible, true)
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
hook @hook.choice.listen_visible
on @choice.opening.listen
phase VisibilityChanged
check on change
when object.visible
once
{
    signal.set(@signal.choice_visible, true)
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
pub flow @flow.opening opening(state: GameState) -> Result<FlowExit, FlowError> {
    bg(@asset.bg.room, fade = 300ms)
    show(@character.alice, .smile, at = .center)

    scope rain {
        地の文(id=@.sound): 扉の向こうから、雨の音がした。[p]
        alice(id=@.comment, voice=auto): 雨、強くなってきたね。[p]
    }

    scope dream {
        choice @.first {
            @.listen "聞いてみる" -> @flow.alice_intro
            @.silent "黙っている" -> @flow.quiet_intro
        }
    }
}
```

There is no separate `script` item. Concise dialogue statements and typed Arcweft statements are both `FlowItem`s.
