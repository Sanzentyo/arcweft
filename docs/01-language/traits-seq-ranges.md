# Traits, Sequences, Ranges, and Wrapper Operations

Arcweft は Rust 風の generics を採用するため、`trait` / `impl` / `where` を中核機能として持つ。

This chapter defines trait declarations, typed iteration, and wrapper operations.
The public callable catalog owns the available operations; a generic `Monad`
trait is not required.

## Trait declarations and boundaries

`trait` is the canonical keyword for language traits; `protocol` remains reserved for host, wire, and Agent protocol concepts. Trait declarations support supertraits, required associated types and method signatures, trait and inherent impls, associated type assignments and projections, generic bounds, `where` predicates, associated type equality constraints, coherence checks, and typed semantic evidence.

Associated type defaults, associated type constructors such as `type Mapped<B>`, and default method bodies are parsed but rejected by semantic analysis. These are current language boundaries; see the [trait diagnostics](../../crates/arcweft-lang-sema/src/diagnostics/trait_diagnostic.rs#L303).

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

Trait associated types support required declarations and impl assignments. Generic associated type constructors such as `type Mapped<B>` are parsed but rejected by semantic analysis. The current public `map` surface uses callable overloads and does not depend on GAT support.

The public `map` surface is a family of ordinary functions with an explicit extension receiver. The published receiver families are `Vec<A> -> Vec<B>`, `Seq<A> -> Seq<B>`, `Array<A, N> -> Array<B, N>`, `Slice<A> -> Vec<B>`, `Option<A> -> Option<B>`, and `Result<A, E> -> Result<B, E>`. The catalog does not publish `map` overloads for `Need`, `Parser`, or `Stream`. See the [published map families](../../crates/arcweft-lang-sema/src/types.rs#L872).

The name remains `map`: names such as `map_with`, `map_last`, or `map_values` would expose calling convention rather than operation meaning. Each overload supports `map(f)(value)`, `value |> map(f)`, and `value.map(f)` through the same callable declaration. `Slice<A>` materializes a `Vec<B>` because a borrowed slice cannot own a new mapped backing store. Mapping `Array<A, N>` preserves its checked length without introducing user-written const-generic parameter syntax. `String` is not a collection-map receiver; text transformation uses text-specific operations. The mapping function remains pure and synchronous; effectful concurrent transformation is `traverse`.

## Wrapper combinators

`Option` and `Result` composition, including `and_then`, `map_err`, and `or_else`, is specified in [Result / Option, `try`, and Context](result-option-context.md). `Need` and `await` behavior is specified in [Await, unary Need, carrier blocks, and `try`](await-need-result.md).

## `try` and residuals

`try expr` handles the `Result` and `Option` residuals described in [Result / Option, `try`, and Context](result-option-context.md). This is an expression-level contract.

```arcw
fn selected_route(state: GameState) -> Result<Ref<Flow>, GameError> {
    let route = try state.route_override.ok_or(.MissingRoute)
    Ok(route)
}
```

`Result` の error parameter は anonymous sum にできる。`try` は
`FsError` や `ParseError` を期待される `FsError | ParseError` へ一意に注入する。

```arcw
fn load_config(path: VirtualPath) -> Result<Config, FsError | ParseError> {
    let text = try read_text(path)
    try parse_config(text)
}
```

`Need<Result<T, E>>` は `T` に暗黙変換できない。先に
`await ... with { ... }`で時間層を外し、そのResultをmatch/tryする。

```arcw
let bg =
    try await asset.image(@asset:.bg.room) with {
        pending p => scene.show(@scene.loading); progress.set(p.ratio)
    }
```

## Iterator, IntoIterator, and Seq

Arcweft の `for` は typed iteration facts を使う。`Seq<T>` は pure lazy sequence を表す concrete type であり、built-in iterator families の1つである。

```arcw
pub trait Iterator {
    type Item

    fn next(&mut self) -> Option<self.Item>
}

pub trait IntoIterator {
    type Item
    type IntoIter

    fn into_iter(self) -> self.IntoIter
}
```

Built-in iteration facts cover `Range<T>`, `Seq<T>`, `Stream<T, E>`, `Vec<T>`, `Array<T, N>`, and `Slice<T>`. `Range` advances through runtime iterator state without materializing its elements. See [built-in iteration selection](../../crates/arcweft-lang-sema/src/final_analysis/analyzer/expression_types.rs#L91).

For other source types, the checker resolves an `IntoIterator` conformance and then an `Iterator` conformance for its associated `IntoIter` type. If no `IntoIterator` conformance exists, an `Iterator` conformance on the source itself serves as identity `IntoIterator`, with `IntoIter = Self`. Typed witness evidence is carried into lowering and runtime execution. See [trait-witness selection](../../crates/arcweft-lang-sema/src/final_analysis/analyzer/statements.rs#L82).

```arcw
let labels =
    choices
        .into_iter()
        .filter(_.enabled)
        .map(_.label)
        .collect<Vec<String>>()
```

```arcw
for c in choices {
    option c.id {
        label = c.label
    }
}
```

## Capacity and reservable collections

Sequence length is semantic state and is available without forcing a dynamic
element materialization boundary.

```arcw
pub trait Len {
    fn len(self) -> usize
}
```

MVP standard implementors:

```text
Vec<T>
Seq<T>
Slice<T>
Array<T, N>
```

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

The `&mut self` receiver requires a writable place: a local value or a direct
nominal field. The `mut` marker is a distinct binding annotation and is not
required just to call these methods.

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
Stream<T, E>
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

Integer ranges are built-in iteration sources and advance through non-materialized runtime iterator state.

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
    fn contains(self, value: T) -> bool
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
    try await image_ids.traverse(asset.image).parallel(limit = 4) with {
        pending p => scene.show(@scene.loading); progress.set(p.ratio)
    }
```

The bounded traversal contract is `Vec<T>.traverse(capability.fn).parallel(limit = N)`, where the capability function returns `Need<Result<U, E>>`. The limit bounds in-flight child requests, and the aggregate preserves source order. Awaiting produces `Result<Vec<U>, E>`; `try await` applies the `Need` and `Result` rules described in [Result / Option, `try`, and Context](result-option-context.md). Runtime fanout details are in [Async scheduler and Need](../02-runtime/async-scheduler.md#need-handling).

`traverse` captures its collection and unary callback in `Traversal<S, F>`;
it does not invoke the callback. `S` retains the exact collection type and `F`
retains the callback's parameter, `Need<Result<U, E>>` result, and effect row.
Vec, Array, Slice, and Seq use the same checked element/callback relation.
The intermediate value owns its captured values; type-level function identity
alone does not grant permission to copy captures.

`parallel` consumes this intermediate value and constructs the ordinary
`Need<Result<Vec<U>, E>>`. Its required named `limit` is a positive `u32` value
and may be evaluated at runtime. Invalid configuration is rejected before any
child starts. Starting follows the producer's checked start policy; `await`
removes only the temporal layer. The callback's effect row is preserved at the
scheduled operation. The Need can be bound or passed before it is awaited:

```arcw
let work = paths.traverse(fs.read_text).parallel(limit = limit)
let texts = try await work
```

Empty traversal produces `.Ok([])`. Domain results are collected at their source
positions; after collection, all `.Ok` payloads form the ordered vector, or the
lowest source position containing `.Err` supplies the aggregate error. Host
completion order does not select the error. Infrastructure failure and
cancellation retain their separate runtime lifecycle rules.

## Stream is not Seq

`Stream<T, E>` は外部 capability または別の Stream 変換が返す ordered
stream であり、`Seq<T>` とは分離する。外部入力は通常の capability
operation として宣言し、Source 型や `source` declaration は持たない。

```arcw
extern capability camera {
    fn frames() -> Stream<CameraFrame, CaptureError>
}
```

`Stream` は `Seq` へ暗黙変換しない。permission、privacy、backpressure、cancel、
record/replay の方針は capability/host 境界が所有し、DSL は typed Stream
events を消費する。

```arcw
let frames = camera.frames()
let captured = frames.take(60).collect<Vec<CameraFrame>>()
```

## Computation blocks

Monad 的な処理は block で読みやすく書ける。

```arcw
let route = result {
    let id = try parse_choice_id(raw)
    let route = try route_for_choice(id)
    Ok(route)
}
```

```arcw
let assets = task {
    let bg = try await asset.image(@asset:.bg.room)
    let voice = try await asset.audio(@asset:.voice.alice.001)
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

## Subsystem cache bounds

Cache policy is not part of a function declaration. A subsystem that retains a
value may require typed key traits such as `StableHash + Eq`, and may require an
owned or `Clone` result, at that subsystem API boundary. Ordinary functions do
not gain cache-specific bounds merely because the compiler or runtime can reuse
their pure result.

## Coherence

Arcweft は曖昧な method resolution を避けるため、初期仕様では impl coherence を厳しくする。

```text
1. 同じ trait/type の impl は1つだけ
2. overlapping impl は禁止
3. orphan rule 相当を採用
4. blanket impl は core/prelude crate 中心
5. user blanket impl は将来拡張
```

