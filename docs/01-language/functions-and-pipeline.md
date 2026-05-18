# 関数、パイプ、カリー化

## 関数は data-last が標準

```awft
fn map<A, B>(f: A -> B)(xs: Vec<A>) -> Vec<B>
fn filter<A>(pred: A -> Bool)(xs: Vec<A>) -> Vec<A>
fn fold<A, B>(init: B, step: (B, A) -> B)(xs: Vec<A>) -> B
```

これにより、以下が自然になる。

```awft
choices
    .filter(_.enabled)
    .map(_.label)
```

desugar:

```awft
map(_.label)(filter(_.enabled)(choices))
```

## パイプ

```awft
choices
    |> filter(_.enabled)
    |> map(_.label)
```

`rhs` に `^` がない場合は、左辺を最後の引数に渡す。

## プレースホルダ付きパイプ

```awft
raw_score |> clamp(0, ^, 100)
```

展開:

```awft
clamp(0, raw_score, 100)
```

`_` と `^` は役割が違う。

```awft
threshold |> choices.filter(_.score >= ^)
```

- `_`: `choices` の各要素。
- `^`: pipe 左辺の `threshold`。

展開:

```awft
choices.filter(|choice| choice.score >= threshold)
```

## カリー化

```awft
fn has_affection_at_least(character: Ref<Character>, min: i32)(state: GameState) -> Bool {
    state.affection.get(character).unwrap_or(0) >= min
}

let alice_ready = has_affection_at_least(@character.alice, 3)
if state |> alice_ready { ... }
```

## 部分適用

```awft
let is_high = (_ >= 80)
let add_alice = add_affection(@character.alice, 1)
```

## Seq と lazy pipeline

```awft
let choices =
    opening_choices()
        .seq()
        .filter(_.enabled)
        .map(choice_to_view(state))
        .take(5)
```

`Seq<T>` は lazy。必要時に `collect<Vec<T>>()` で materialize。

## effectful map は `traverse`

```awft
let images = await image_paths.traverse(asset.image).parallel(limit = 4)
```

- `map`: pure / synchronous。
- `traverse`: `Task` / `Need` を返す。



