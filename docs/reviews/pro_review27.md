According to a document from 2026-05-16, arcweft については理念・構造を踏まえて考える前提なので、ここでは「今すぐ構文を増やすべきか」ではなく、**将来どこで自然に欲しくなりそうか**で整理します。
現状は呼び出し AST が `args: Vec<Expr>` を持つので呼び出し側は任意個数を受けられますが、宣言側の `FnSignature` は固定の `FnParamGroup` / `FnParam` 構造なので、話題は「一般関数に rest parameter を入れるか」というレイヤーです。

欲しくなりそうなのは、優先度順にこのあたりです。

| パターン                                   | 例                                                                            | variadic が欲しくなる理由            | ただし                                                      |
| -------------------------------------- | ---------------------------------------------------------------------------- | ---------------------------- | -------------------------------------------------------- |
| **ログ・診断・観測**                           | `log.info("loaded", asset, ms, source)` / `ensure(cond, "msg", key = value)` | メッセージ本体 + 任意個の補足情報、という形が自然   | 最有力。ただし named fields / record に畳む方が arcweft らしい          |
| **イベント発火**                             | `event.emit(@event.hit, actor, target, damage)`                              | event ごとに payload が違う        | 既に `event.emit` は特別扱いしやすい。一般 variadic より intrinsic 扱いがよい |
| **format / i18n / text interpolation** | `format("{} {}", a, b)` / `tr(@text.greet, name = alice)`                    | 置換値の数が文字列側で決まる               | i18n は named arg の方が安全。位置 variadic は診断が難しい               |
| **テスト・検証ヘルパ**                          | `expect_events(e1, e2, e3)` / `case(name, input, expected, tags...)`         | fixture では「列挙するだけ」の API が多い  | `[e1, e2, e3]` で十分なことが多い                                 |
| **条件合成・トリガ合成**                         | `any(sig1, sig2, sig3)` / `all(flag1, flag2)`                                | 可読性が高く、ネストを減らせる              | `any([sig1, sig2])` でもよい。糖衣としてならあり                       |
| **並列・レース・タスク合成**                       | `together(a, b, c)` / `race(a, b, c)`                                        | line/task graph の DSL と相性がよい | runtime-plan 側で境界を明確化する必要あり                              |
| **presentation / staging の合成**         | `show(alice, .normal, fade, slot, z)`                                        | 演出指定が増えやすい                   | これは可変長より named options / record がよい                      |
| **asset preload / dependency 宣言**      | `preload(bg1, bg2, voice1)`                                                  | 単純な同型リスト                     | `preload([bg1, bg2])` の方が型が明快                            |
| **parser/combinator 系**                | `one_of(p1, p2, p3)`                                                         | DSL ライブラリでは定番                | 内部 DSL としてならあり                                           |
| **modder 向け軽量 API**                    | `print(a, b, c)` / `debug(a, b, c)`                                          | 使い勝手重視の場面で欲しくなる              | core language ではなく標準ライブラリ側で限定するのが安全                      |

一番現実的に「欲しい」と感じるのは、**ログ・イベント・assert/ensure/fail の診断 payload** です。arcweft はすでに runtime observation を通常 call syntax として扱う方向で、`log.info(...)`、`signal.set(...)`、`metric.set(...)`、`event.emit(...)` などを typed runtime IR に落とす設計になっています。 ここは「任意個の追加情報」が自然に出ます。

ただ、ここでも一般 variadic ではなく、まずはこういう形がよさそうです。

```arcw
log.info("asset loaded", asset = @asset.bg.room, elapsed = 120ms)
event.emit(@event.asset_loaded, {
    asset: @asset.bg.room,
    elapsed: 120ms,
})
ensure(ready, "scene is not ready", scene = @scene.opening)
```

つまり **可変個の positional args** より、**任意個の named fields / record payload** の方が arcweft の構造化・診断・runtime-plan に合います。

逆に、一般関数でこういうのを早く入れる必要は低いです。

```arcw
fn foo(first: T, rest: ...T) -> U
```

これを入れると、`default parameter`、`named arg`、`curried parameter groups`、型推論、arity 診断、HIR lowering の全部に影響します。現状の関数 signature は generics・curried parameter group・where clause・default parameter までは持っていますが、rest parameter の表現はありません。

判断基準としては、次の兆候が出たら検討でよいと思います。

複数の API が `foo(a)`, `foo2(a, b)`, `foo3(a, b, c)` みたいに増え始めた。呼び出し側で毎回 `foo([a, b, c])` の `[]` がノイズになっている。`log` / `event` / `format` / `assert` のように「先頭は固定、残りは補足情報」という形が大量に出る。型検査後に結局 `Vec` や `Record` へすぐ畳んでいる。

その段階で入れるなら、設計は小さく始めるのがよいです。
**最後の 1 パラメータだけ rest 可**、**同型 rest か named-only rest に限定**、**HIR では `Vec<T>` または `Record` に正規化**、**core language ではなく標準 intrinsic から先に試す**、くらいが安全です。

なので、将来欲しくなる本命は **観測・診断・イベント payload・format/i18n・タスク/条件 combinator**。
でも実装順としては、**まず named args / record / Vec で回し、どうしても呼び出しノイズが溜まった箇所だけ限定 variadic 化**が一番よさそうです。
