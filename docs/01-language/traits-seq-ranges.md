# Traits, Seq, Ranges, and Monad-like Abstractions

Arcweft は Rust 風の generics を採用するため、`trait` / `impl` / `where` を中核機能として持つ。

同時に、`Option` / `Result` / `Need` / `Parser` / `Seq` / `Source` を自然に扱うため、Monad 的な抽象を標準 prelude に入れる。ただし、通常のユーザーには `Monad` という名前を前面に出さず、`map`、`and_then`、`?`、`await`、`traverse`、`seq`、`collect` として見せる。

## trait / impl / where

```arcw
pub trait Format {
    fn format(self) -> String
}

pub trait Eq {}
pub trait Ord: Eq {}
pub trait Hash {}
pub trait StableHash {}
```

実装:

```arcw
pub impl Format for Route {
    fn format(self) -> String {
        match self {
            .Opening => "Opening",
            .AliceIntro { .. } => "AliceIntro",
            .BadEnd(reason) => "BadEnd"
        }
    }
}
```

Generic function:

```arcw
pub fn group_by<T, K>(key: T -> K)(xs: Vec<T>) -> OrderedMap<K, Vec<T>>
where
    K: Eq + Hash
{
    ...
}
```

## Associated type and GAT-like constructors

Arcweft は Rust の associated type 風の設計を使う。`map` の戻り wrapper を表すために、GAT 風の associated type constructor を許可する。

```arcw
pub trait Mappable {
    type Item
    type Mapped<B>

    fn map<B>(self, f: Self::Item -> B) -> Self::Mapped<B>
}
```

`Option<T>`:

```arcw
pub impl<T> Mappable for Option<T> {
    type Item = T
    type Mapped<B> = Option<B>

    fn map<B>(self, f: T -> B) -> Option<B> {
        match self {
            Some(x) => Some(f(x)),
            None => None,
        }
    }
}
```

`Result<T, E>`:

```arcw
pub impl<T, E> Mappable for Result<T, E> {
    type Item = T
    type Mapped<B> = Result<B, E>

    fn map<B>(self, f: T -> B) -> Result<B, E> {
        match self {
            Ok(x) => Ok(f(x)),
            Err(e) => Err(e),
        }
    }
}
```

## Bindable

`and_then` / `flat_map` 用の抽象。

```arcw
pub trait Bindable: Mappable {
    type Bound<B>

    fn and_then<B>(self, f: Self::Item -> Self::Bound<B>) -> Self::Bound<B>
}
```

Arcweft の標準 wrapper は `map` と `and_then` を持つ。

```text
Option<T>       map / and_then
Result<T, E>    map / map_err / and_then / or_else
Need<T, E>      map / and_then, but force requires await/poll/select
Parser<T, E>    map / and_then / alt / many / optional
Seq<T>          map / filter / flat_map / take / collect
Source<T, E>    map / filter / throttle / record, but live and permissioned
```

## TryLike and `?`

`?` は `TryLike` により支えられる。

```arcw
pub trait TryLike {
    type Output
    type Residual

    fn branch(self) -> ControlFlow<Self::Residual, Self::Output>
}
```

`Result<T, E>` と `Option<T>` が実装する。

```arcw
fn selected_route(state: GameState) -> Result<Ref<Flow>, GameError> {
    let route = state.route_override.ok_or(.MissingRoute)?
    Ok(route)
}
```

`Need<Result<T, E>, TaskError>` は `T` に暗黙変換できない。先に `await ... with { ... }` が必要。

```arcw
let bg =
    try await asset.image(@asset.bg.room) with {
        pending p => scene.show(@scene.loading); progress.set(p.ratio)
    }
```

## Seq and IntoSeq

Arcweft の表面 API は Rust の `Iterator` ではなく、pure lazy sequence の `Seq<T>` を使う。

```arcw
pub trait IntoSeq {
    type Item

    fn seq(self) -> Seq<Self::Item>
}
```

`Vec<T>` と整数 `Range<T>` は `IntoSeq` を実装する。

```arcw
let labels =
    choices
        .seq()
        .filter(_.enabled)
        .map(_.label)
        .collect<Vec<String>>()
```

`for` は `IntoSeq` を要求する。

```arcw
for c in choices {
    option c.id {
        label = c.label
    }
}
```

## Capacity and reservable collections

Capacity is an allocation hint, not semantic state. Standard containers may
reserve or shrink storage for performance, but programs must not branch on the
current capacity and stable data formats must not serialize capacity.

```arcw
pub trait WithCapacity {
    fn with_capacity(capacity: usize) -> Self
}

pub trait Reservable {
    fn reserve(&mut self, additional: usize) -> Unit
    fn shrink(&mut self) -> Unit
    fn shrink_to(&mut self, min_capacity: usize) -> Unit
}
```

MVP standard implementors:

```text
Vec<T>
String
Bytes
```

Non-owning or streaming views do not implement these traits:

```text
Slice<T>
Seq<T>
Source<T, E>
TextCluster
```

Examples:

```arcw
let names = Vec<String>.with_capacity(8)
names.reserve(4)
names.shrink_to(2)

let line = String.with_capacity(64)
line.shrink()
```

The runtime may treat these calls as no-ops in constrained targets such as
Wasm, embedded players, or deterministic replay modes. That is valid because
capacity is deliberately non-observable.

## Range notation

Range 記法を標準で持つ。

```arcw
0..10       // half-open: 0 <= x < 10
0..=10      // inclusive: 0 <= x <= 10
..10        // end-bounded
0..         // start-bounded
..          // full range
```

整数 range は `Step` により `Seq` 化できる。

```arcw
for i in 0..10 {
    log.debug("i={i}", i = i)
}
```

浮動小数 range は interval として扱う。反復したい場合は sampling を明示する。

```arcw
requires progress in 0.0..=1.0

render_shader @shader.transition.dissolve {
    params {
        progress = samples(0.0..=1.0, 32)
    }
}
```

## Step and Contains

```arcw
pub trait Step: Ord + Clone {
    fn next(self) -> Option<Self>
}

pub trait Contains<T> {
    fn contains(self, value: T) -> Bool
}
```

`Range<T>` と `RangeInclusive<T>` は `Contains<T>` を実装する。

```arcw
invariant @inv.affection_bounds(state) {
    forall c in CharacterId {
        state.affection[c] in 0..=100
    }
}
```

## Traversable

`map` は pure transformation。`Need` を返す関数を collection に適用する場合は `traverse` を使う。

```arcw
let images =
    await image_ids
        .traverse(asset.image)
        .parallel(limit = 4)? with {
            pending p => scene.show(@scene.loading); progress.set(p.ratio)
        }
```

## Source is not Seq

`Source<T, E>` は live external stream であり、`Seq<T>` とは分離する。

```arcw
Source<MicFrame, CaptureError>
Source<CameraFrame, CaptureError>
Source<UsbPacket, UsbError>
```

`Source` は permission、privacy、backpressure、cancel、record/replay を持つため、`Seq` へ暗黙変換しない。

```arcw
let frames =
    await camera_source
        .take(60)
        .record()
        .collect<Vec<CameraFrame>>() with {
            pending p => scene.show(@scene.capture_wait); progress.set(p.ratio)
        }
```

## Computation blocks

Monad 的な処理は block で読みやすく書ける。

```arcw
let route = result {
    let id = parse_choice_id(raw)?
    let route = route_for_choice(id)?
    Ok(route)
}
```

```arcw
let assets = task {
    let bg = await asset.image(@asset.bg.room)?
    let voice = await asset.audio(@asset.voice.alice.001)?
    Ok(OpeningAssets { bg, voice })
}
```

```arcw
let visible_choices = seq {
    for c in opening_choices() {
        if c.enabled {
            yield choice_to_view(state)(c)
        }
    }
}
```

`seq { yield ... }` は pure lazy sequence。device input の `Source` とは別。

## Memoization bounds

Memoization には stable key が必要。

```arcw
memo fn route_available(state: GameState, route: Ref<Flow>) -> Bool
scope = scene
where
    GameState: StableHash,
    Ref<Flow>: Hash + Eq
{
    ...
}
```

結果を cache に保持する場合は `Clone` も要求できる。

```arcw
memo fn expensive<T>(x: T) -> Computed
scope = session
where
    T: StableHash + Eq,
    Computed: Clone
{
    ...
}
```

## Coherence

Arcweft は曖昧な method resolution を避けるため、初期仕様では impl coherence を厳しくする。

```text
1. 同じ trait/type の impl は1つだけ
2. overlapping impl は禁止
3. orphan rule 相当を採用
4. blanket impl は core/prelude crate 中心
5. user blanket impl は将来拡張
```


