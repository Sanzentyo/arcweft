# 構文概要

## 最小例

```arcw
mod crate.game.routes.opening

use crate.game.prelude.*
use super.logic.affection.{has_affection_at_least}

#[derive(StableHash)]
pub flow opening(state: GameState) -> Result<FlowExit, FlowError> {
    bg(@asset:.bg.room, fade = 300ms)
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

- 型パラメータは `<>`: `Vec<T>`, `Result<T, E>`。
- Entity 参照は family を明示する。手書きの通常形では `@asset:.foo`
  のような family-relative form を推奨し、`@asset.foo` は生成物や tooling
  出力向けの完全 public-id 形とする。複雑な参照やメンバアクセス前は `@<foo.bar>`。
- `@.suffix` / `@..suffix` / `@...suffix` / `@super.suffix` は dialogue line / choice / option / text key のような ID 文脈だけで使う相対 ID。
- module / import の相対指定は `self.` / `super.` / `crate.` を使う。
- コメントリンクは `[[foo.bar]]`。
- 属性は Rust 風の `#[derive(...)]`, `#[link(...)]`。
- `#` は `#[...]` attribute opener の一部であり、entity ref ではない。
- パイプは `|>`。
- パイプ左辺の placeholder は `^`。
- lambda / partial placeholder は `_`。
- 変数は immutable。`mut` は局所的にのみ許可。
- `null` はない。`Option` を使う。
- 数値 primitive は `i32`, `u64`, `f32` のように bit 幅を明示する。
- 数値 literal は期待型があればそれを使う。期待型がない整数 literal は `i32`、
  float literal は `f64` に fallback し、推論された closure body など契約が
  見えにくい場所では lint/warning で明示化を促す。

## Traits and impls

Arcweft DSL type abstraction uses Rust-like `trait` and `impl` syntax:

```arcw
pub trait SourceLike {
    type Item
    fn current(self) -> self.Item
}

impl SourceLike for ChapterSource {
    type Item = ChapterId
    fn current(self) -> ChapterId {
        self.current_id
    }
}
```

Generic bounds can be written inline or in a `where` clause:

```arcw
fn current_item<T: SourceLike>(source: T) -> T.Item {
    source.current()
}

fn exact<T>(source: T) -> ChapterId
where T: SourceLike<Item = ChapterId>
{
    source.current()
}
```

`self.Assoc` and `T.Assoc` are associated type projections. In seq08.1,
projections are compile-time type expressions; dynamic trait objects, fully
qualified method syntax, default associated types, and GAT-like associated type
constructors are deferred.

## `@` 参照

```arcw
goto @flow.alice_intro
bg(@asset:.bg.room)
#[link(Flow, @flow.alice_intro, level = .soft)]
```

境界明示が必要なとき:

```arcw
let result = try await @<activity.truck_game>.run(input) with:
    pending p:
        scene.show(@scene.loading)
        progress.set(p.ratio)

@<say.opening.dream_hint@sem:b3_9f2a1c>
@<asset:bg/room.ktx2>
```

Relative ID forms:

```arcw
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

```arcw
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

```arcw
/// この台詞は [[flow.alice_intro]] の伏線。
/// Aliceの好感度は [[state.GameState.affection]] で管理する。
```

## `Need` の待機強制

flow 内では、時間のかかる処理に待機時の表示が必須。

```arcw
let assets = try await load_opening_assets() with {
    pending p => {
        scene.show(@scene.loading)
        text.show("Openingを準備中")
        progress.set(p.ratio)
    }
    timeout 3s => {
        scene.show(@scene.loading_slow)
        text.show("少し時間がかかっています")
    }
}
```

## View の reactive `Need` match

View でも lazy 値の暗黙 force は禁止する。ordinary `match` grammarを使い、
View semaがretained subscriptionとbranchへprojectする。

```arcw
match typeset(@typeset.credits) {
    .pending(progress) => Text("組版中")
    .ready(document) => TypesetView(document)
    .error(error) => Text("表示できません")
    .denied(reason) => PermissionDenied(reason)
}
```

`AwaitView`専用syntaxは最終surfaceに残さない。
```



## Event ownership and derived values

Events are written next to their owner. For example, a choice owns its
availability and selection behavior:

```arcw
option @.listen {
    label = "聞いてみる"
    enabled = state.affection[@character.alice] >= 3

    select {
        goto @flow.alice_intro
    }
}
```

Derived values are ordinary pure functions:

```arcw
fn route_title(route: Ref<Flow>) -> String {
    registry.flow(route).title
}
```

There is no universal top-level event declaration or author-controlled memo
declaration/block. Internal dispatch and caches stay with the subsystems that
own their event ordering, keys, lifetimes, and invalidation. See
[Event Ownership and Caching](hooks-and-memoization.md).


## Flow-integrated scenario syntax

For ordinary visual novel writing, Arcweft provides compact line-oriented syntax directly inside `flow`. It is documented in [Flow-Integrated Scenario Syntax / Dialogue Sugar](scenario-surface-syntax.md). Dialogue tags and ruby are documented in [Dialogue Control Tags, Ruby, and Inline Hooks](dialogue-control-tags-and-ruby.md).

```arcw
pub flow opening(state: GameState) -> Result<FlowExit, FlowError> {
    bg(@asset:.bg.room, fade = 300ms)
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


