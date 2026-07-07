# 関数、パイプ、カリー化

## 関数は data-last が標準

```arcw
fn map<A, B>(f: A -> B)(xs: Vec<A>) -> Vec<B>
fn filter<A>(pred: A -> bool)(xs: Vec<A>) -> Vec<A>
fn fold<A, B>(init: B, step: (B, A) -> B)(xs: Vec<A>) -> B
```

これにより、以下が自然になる。

```arcw
choices
    .filter(_.enabled)
    .map(_.label)
```

desugar:

```arcw
map(_.label)(filter(_.enabled)(choices))
```

## パイプ

```arcw
choices
    |> filter(_.enabled)
    |> map(_.label)
```

`rhs` に `^` がない場合は、左辺を最後の引数に渡す。

## プレースホルダ付きパイプ

```arcw
raw_score |> clamp(0, ^, 100)
```

展開:

```arcw
clamp(0, raw_score, 100)
```

`_` と `^` は役割が違う。

```arcw
threshold |> choices.filter(_.score >= ^)
```

- `_`: `choices` の各要素。
- `^`: pipe 左辺の `threshold`。

展開:

```arcw
choices.filter(|choice| choice.score >= threshold)
```

## カリー化

```arcw
fn has_affection_at_least(character: Ref<Character>, min: i32)(state: GameState) -> bool {
    state.affection.get(character).unwrap_or(0) >= min
}

let alice_ready = has_affection_at_least(@character.alice, 3)
if state |> alice_ready { ... }
```

## 関数値と適用

クロージャーと bare function name は関数値である。評価時にその lexical
environment を決定的な capture binding として保持し、後続の call / apply で
引数 binding より先に復元する。

```arcw
let add_with_bonus = |score: i64| score + bonus
let next = add_with_bonus(3i64)
```

```arcw
fn add(a: i64)(b: i64) -> i64 { a + b }

let f = add
let add_two = add(2i64)
let seven = add_two(5i64)
```

実装は exact arity の既知 pure function call を最適化された helper call に
落としてよい。ただし、関数が値位置に現れる場合や必要数より少ない引数で
呼ばれる場合の言語意味は、関数値の apply と同じでなければならない。

関数値に必要数より少ない引数を渡した場合は、渡した引数を capture した残り
引数の関数値になる。これにより curried call は通常の apply の連鎖として扱える。

```arcw
fn add(a: i64)(b: i64) -> i64 { a + b }

let add_two = add(2i64)
let seven = add_two(5i64)
let also_seven = add(2i64)(5i64)
```

## 部分適用

```arcw
let is_high = (_ >= 80)
let add_alice = add_affection(@character.alice, 1)
```

## Seq と lazy pipeline

```arcw
let choices =
    opening_choices()
        .seq()
        .filter(_.enabled)
        .map(choice_to_view(state))
        .take(5)
```

`Seq<T>` は lazy。必要時に `collect<Vec<T>>()` で materialize。

## effectful map は `traverse`

```arcw
let images = try await image_paths.traverse(asset.image).parallel(limit = 4) with {
    pending p => progress.set(p.ratio)
}
```

- `map`: pure / synchronous。
- `traverse`: `Task` / `Need` を返す。
- `.parallel(limit = N)`: bounded fanout。VM は一度に最大 `N` 件の
  `TaskSpec` を出し、結果を入力順の `Vec` として返す。




