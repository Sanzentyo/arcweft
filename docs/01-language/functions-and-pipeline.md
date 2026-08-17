# 関数、パイプ、カリー化

## 関数は data-last が標準

```arcw
fn map<A, B>(f: A -> B)(xs: Vec<A>) -> Vec<B>
fn filter<A>(pred: A -> bool)(xs: Vec<A>) -> Vec<A>
fn fold<A, B>(init: B, step: (B, A) -> B)(xs: Vec<A>) -> B
```

Function type は closed effect row を suffix として持てる。これは関数値の
生成ではなく、その関数値を apply した時に発生しうる effect を表す。

```arcw
let load_text: String -> String effects { fs.read } = read_text
let projector: (String -> String) effects { fs.read } = read_text
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

`rhs` に `^` がない場合は、`rhs` をまず関数値として評価し、その関数値へ
左辺を 1 引数の次 call group として適用する。

```arcw
x |> f(a)       // f(a)(x)
x |> f(a, b)    // f(a, b)(x)
```

この規則は既存の call group へ左辺を append する書き換えではない。
したがって `f(a)(b)` と `f(a, b)` の区別はパイプでも保たれる。左辺は
`rhs` より先に一度だけ評価され、その値を次の apply が読む。

## プレースホルダ付きパイプ

```arcw
raw_score |> clamp(0, ^, 100)
```

概念上の展開:

```arcw
let <pipe-left> = raw_score
clamp(0, <pipe-left>, 100)
```

`<pipe-left>` はソースから記述できない内部 binding である。RHS 内に `^` が
複数あっても、closure や `if` / `match` の内側にあっても、すべて同じ値を
読む。左辺式を `^` の個数だけ複製してはならない。入れ子のパイプでは、内側
パイプの RHS にある `^` は内側の値を参照し、内側パイプの LHS にある `^` は
外側の RHS scope を参照する。

`_` と `^` は役割が違う。

```arcw
threshold |> choices.filter(_.score >= ^)
```

- `_`: `choices` の各要素。
- `^`: pipe 左辺の `threshold`。

概念上の展開:

```arcw
let <pipe-left> = threshold
choices.filter(|choice| choice.score >= <pipe-left>)
```

## Method calls do not fall back to free callables

`receiver.method(args...)` resolves only an inherent method or a visible trait
method. A same-named free callable is never tried as a fallback. Use the pipe
for explicit data-last application:

```arcw
receiver |> transform(options...)
```

This keeps method resolution stable when an API later adds a real method.

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

Closure は返り値型を明示できる。返り値型を明示する場合、body は block
必須である。

```arcw
let is_high =
    |score: i64| -> bool {
        score >= 80i64
    }

let now_text =
    || -> String {
        clock.now().to_string()
    }
```

返り値型なしの軽い closure はそのまま使える。

```arcw
choices.filter(|choice| choice.enabled)
```

Curried closure では call group を flatten しない。`|a, b| -> C { ... }`
と `|a| |b| -> C { ... }` は別の関数型である。

```arcw
let ge =
    |min: i64| |value: i64| -> bool {
        value >= min
    }
```

`_` と `^` に直接 return type annotation は付けない。型を明示したい場合は
binding 側の `let f: A -> B = ...` か、明示 closure を使う。

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

Call group は flatten しない。`f(a, b)(c)` と `f(a)(b, c)` は別の
関数型として扱う。この curried call group は ordinary `fn` / trait member /
impl member の関数的な宣言に属する構文であり、`flow` parameter は 0 個または
1 group に限る。
`flow main(a)(b)` のような curried flow parameter は構文診断になる。

```arcw
fn tuple_tail(a: i64, b: i64)(c: i64) -> (i64, i64, i64) {
    (a, b, c)
}

fn chain(a: i64)(b: i64)(c: i64, d: i64) -> i64 {
    a + b + c + d
}

let tupled = tuple_tail(1i64, 2i64)(3i64)
let sum = chain(1i64)(2i64)(3i64, 4i64)
```

## 部分適用

```arcw
let is_high = (_ >= 80)
let add_alice = add_affection(@character.alice, 1)
```

## Spread arguments in partials and staged application

Spread call arguments use `expr...`.

Ordinary exact calls may use spread only where the callee signature gives a
deterministic target. A variable-length spread may feed a rest parameter after
the required fixed parameters have already been supplied. It is not used to
infer how many fixed parameters should be filled.

Partial-call construction accepts spread only when the spread source has a
statically known inline literal length:

```arcw
let add_one = add([1i64]...)
let exact = add([1i64]..., 2i64)
```

The same rule applies to function-value calls:

```arcw
let add_one = add(1i64)
let three = add_one([2i64]...)
```

Variable-length spread remains rejected in partial-call construction and
data-last fallback:

```arcw
let later = add(values...)          // error: variable-length partial spread
let later = add(values..., 1i64)    // error: spread followed by fixed arg
let ok = score.above(thresholds...) // error: variable-length data-last spread
```

This is a language contract, not a temporary lowering limitation. The runtime
can expand `RuntimeExpr::SpreadArg`, but source-level partial construction
needs deterministic arity, argument order, and typed lowering evidence. Use an
inline fixed-length literal spread or write an explicit closure when the spread
length is not statically known.

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




