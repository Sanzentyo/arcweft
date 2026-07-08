現行の arcweft 設計を見ると、Arcweft は単なるノベル DSL ではなく、DSL、Typed IR/Bytecode、wgpu、LayerTree、WASM/Rust plugin、Cranelift JIT、Memoization、device streams、LLM debug まで統合するエンジンとして設計されています。 したがって、標準データ構造は「便利な `Vec` / `HashMap` 集」ではなく、**決定性・Sans I/O・lifetime・replay・tooling 観測性**を前提にしたものがよいです。

## 結論：Arcweft のデフォルト prelude に置くべきもの

### 1. 基本 ADT

これは必須です。

```awft
Option<T>
Result<T, E>
Need<T, E>
```

Arcweft は `null` を置かず `Option<T>` を使い、失敗は `Result<T,E>`、時間がかかる処理は `Need<T,E>` として明示する設計になっています。`Need<T,E>` は暗黙に `T` へ変換されず、`await ... with { pending ... }` のように待機時挙動を明示するのが重要です。

追加で、prelude にはこれも欲しいです。

```awft
Unit
Never
Progress
Duration
LogicalTime
TickId
```

`Never` は `return`、`goto`、`panic`、無限 loop、失敗分岐などの型付けに便利です。`Progress`、`Duration`、`LogicalTime`、`TickId` は `Need`、scheduler、animation、replay に関わります。

---

### 2. ID・参照・所有 handle

Arcweft では物理的な stack/heap より、**ID 参照・borrow・owned handle・lease** の違いを明示するべきです。

```awft
Id<T>
Ref<T>
Handle<T>
WeakHandle<T>
Borrow<'a, T>
Slice<'a, T>
Lease<T>
SharedSlice<T>
```

`Ref<T>` は lifetime を持たない非 null の ID 参照、`&'a T` は lifetime を持つ borrow、`ImageHandle` のような handle は frame を跨げる owned handle、という整理がすでに仕様にあります。

おすすめの役割はこうです。

| 型                              | 用途                                         |
| ------------------------------ | ------------------------------------------ |
| `Id<T>`                        | 内部 runtime / compiler 用の typed ID          |
| `Ref<T>`                       | `.awft` 内の entity reference。例: `Ref<Flow>` |
| `Handle<T>`                    | asset、UI、audio、activity などの所有 handle       |
| `WeakHandle<T>`                | 失効しうる参照。debug UI や async callback 向け       |
| `Borrow<'a,T>` / `Slice<'a,T>` | lifetime 付き zero-copy view                 |
| `Lease<T>` / `SharedSlice<T>`  | shared memory / IPC / data plane 用         |

ここで大事なのは、`Ref<T>` と `&T` を混ぜないことです。`Ref<T>` は symbolic/entity reference、`&T` は実メモリ borrow です。

---

### 3. 決定的 collection

Arcweft の標準 collection は、**iteration order が replay や trace に影響しても壊れないもの**をデフォルトにするべきです。Core は deterministic state machine を目指し、Core / data-format crate は Sans I/O で、path、network、wall-clock、backend resource を直接触らないという境界を持っています。

prelude にはこう置くのがよいです。

```awft
List<T>
Deque<T>
OrderedMap<K, V>
SortedMap<K, V>
OrderedSet<T>
SortedSet<T>
BitSet<E>
SmallList<T, const N>
```

特に重要なのは、**`HashMap` をデフォルトにしない**ことです。使うとしても `EphemeralHashMap` のように、replay・save・serialization・observable iteration に使えないことを型名で示す方が安全です。

おすすめの意味付けはこうです。

| 型                 | 実装候補          | 用途                                      |
| ----------------- | ------------- | --------------------------------------- |
| `List<T>`         | `Vec<T>`      | authored order、choice、ops、assets        |
| `Deque<T>`        | `VecDeque<T>` | pending ops、event queue、scheduler queue |
| `OrderedMap<K,V>` | `IndexMap`    | authoring order を保つ table               |
| `SortedMap<K,V>`  | `BTreeMap`    | canonical order、snapshot、replay         |
| `OrderedSet<T>`   | `IndexSet`    | layer / hook / dependency の安定順          |
| `SortedSet<T>`    | `BTreeSet`    | deterministic canonical set             |
| `BitSet<E>`       | bitset        | flags、effects、capabilities、read-state   |
| `SmallList<T,N>`  | smallvec 系    | hot path、小さい固定上限 list                   |

既存 runtime でも `RuntimePlan` は `Vec`、`FlowFiber` は `VecDeque` と `BTreeMap` を使う形になっており、`RuntimeStepInput` / `RuntimeStepOutput` もイベントや要求の `Vec` を中心に構造化されています。 また HookTable は phase ごとの `BTreeMap` と hook の `IndexMap` を持つ設計なので、**sorted order と insertion order の両方を明示的に使い分ける**方針が合っています。

---

### 4. Arena / Slot / Generational ID

compiler、runtime、presentation、activity にはこれが欲しいです。

```awft
Arena<T>
StableArena<T>
SlotMap<T>
GenerationalId<T>
SparseSet<T>
EntityStore<T>
```

用途はかなり広いです。

| 型                   | 用途                                         |
| ------------------- | ------------------------------------------ |
| `Arena<T>`          | AST/HIR/IR node、短命 compiler object         |
| `StableArena<T>`    | save/replay/debug で ID が安定する object        |
| `SlotMap<T>`        | runtime handle storage                     |
| `GenerationalId<T>` | stale handle 検出                            |
| `SparseSet<T>`      | Activity / mini-game / presentation object |
| `EntityStore<T>`    | entity component ほど重くない typed storage      |

Arcweft は `RuntimeFlowId`、`SourceId`、`StreamRuntimeId`、`TaskId`、`ActivityId` のような typed ID が増える設計なので、単なる `usize` や `u64` を裸で回すより、`GenerationalId<T>` / `Id<T>` を標準化した方が後で壊れにくいです。

---

### 5. Graph / Tree 系

Arcweft には graph と tree がかなり多いので、標準構造として分けた方がよいです。

```awft
Tree<T>
LayerTree<T>
StableGraph<N, E>
DependencyGraph<N>
RouteGraph
SceneGraph
ViewTree
```

必須級なのは `LayerTree` と `DependencyGraph` です。Arcweft は描画と入力の共通境界として `LayerTree` を使い、world、character、effect、Activity、native View、HTML View、modal、debug overlay を layer として扱う設計です。入力も同じ layer stack で hit-test されます。

`DependencyGraph` は memoization、asset、shader、typeset、hot reload、RAG、build graph に使えます。Memo runtime には dependency graph と scope mapping があるため、標準構造として早めに置く価値が高いです。

---

### 6. Source text / compiler tooling 用

Arcweft は lossless CST、HIR、semantic check、runtime-plan lowering、LSP を持つので、source 管理用の標準構造も必要です。crate-map でも `arcweft-lang-syntax` が rowan-compatible な lossless CST、source text、line index、syntax lint を所有する方針になっています。

```awft
Rope
SourceSpan
SourceRange
SourceAnchor
LineIndex
SpanMap<T>
DiagnosticBag
SyntaxNode
AstNode
HirId
```

特に `SourceAnchor` は hook、diagnostic、trace、agent debug、proof obligation のすべてに効きます。

---

### 7. State / Patch / Trace 系

Arcweft には save、replay、hot reload、agent debug があるので、状態変更を直接 mutable map で済ませない方がよいです。

```awft
StatePath
StateCell<T>
Patch<T>
PatchSet
EventLog<E>
TraceLog
Snapshot<T>
Diff<T>
Versioned<T>
StableHash
```

`RuntimeStepOutput` は実際の副作用ではなく、`EffectRequest`、`TaskSpec`、`Command` のような要求を返す設計です。 そのため、state も「即 mutation」より、`Patch`、`EventLog`、`TraceLog` として扱える方が debug / replay / verification に向いています。

---

### 8. Async / Task / Stream 系

Arcweft は `Need<T,E>`、scheduler、device streams、Activity、TaskEvent を持つので、async 周辺の構造は標準化した方がよいです。

```awft
TaskId
TaskKey
TaskQueue
CancelScope
EventQueue<T>
Signal<T>
SignalBus
Source<T, E>
Stream<T, E>
RingBuffer<T>
NeedCacheState<T, E>
```

`NeedCacheState<T,E>` はすでに runtime memoization 側で、`Missing`、`Running(TaskId)`、`Ready(T)`、`Failed(E)`、`Cancelled` の形が示されています。`TaskKey` は実行中 task の合流、`MemoKey` は完了結果の再利用、という役割分担も合っています。

device / capture / USB / HID 系には `RingBuffer<T>` と `Source<T,E>` が必須です。ただし DSL に raw callback を見せず、permissioned port / typed source として扱うべきです。crate-map でも device streams は明示的な backpressure、replay、privacy、cancellation policy を持つ `Source<T,E>` とされています。

---

### 9. Memory / zero-copy / IPC 系

ここは通常 collection と分けるべきです。

```awft
Bytes
Blob
BlobRef
PodSlice<T>
SharedSlice<T>
SharedSliceDesc
MemoryLease
FrameArena
BumpArena
RingBuffer<T>
```

shared memory / IPC では control plane と data plane を分け、大きい配列、mesh、telemetry、audio buffer、physics state は shared memory / mmap / ring buffer / frame arena 側に置く方針が明記されています。さらに shared memory 型では `Vec<T>`、`String`、`Box<T>`、`HashMap<K,V>`、trait object、raw pointer が禁止されています。

つまり、Arcweft の default collection は `List<T>` や `OrderedMap<K,V>` でよいですが、shared memory にはそれらを直接持ち込まず、`SharedSliceDesc`、`MemoryLease`、`PodSlice<T>` のような layout 明示型を使うべきです。

---

### 10. Text / localization / rich text 系

ノベルゲームエンジンとして、これも prelude に近い位置に欲しいです。

```awft
TextKey
Localized<T>
LocaleMap<T>
RichText
TextRun
InlineTag
RubyText
LinePlan
DialogueLine
```

Arcweft は dialogue、ruby、inline formatting、Typst 級 text、localization を扱う設計なので、`String` だけを標準にすると後で苦しくなります。ユーザー向けには `String` を残しつつ、シナリオ本文は `TextKey` / `RichText` / `Localized<RichText>` に寄せるのがよいです。

---

## 推奨する facade prelude

`arcweft` facade crate / DSL prelude に置くなら、最小セットはこれです。

```text
Option<T>
Result<T, E>
Need<T, E>

Id<T>
Ref<T>
Handle<T>
WeakHandle<T>

List<T>
Deque<T>
Map<K, V>          = OrderedMap<K, V>
Set<T>             = OrderedSet<T>
SortedMap<K, V>
SortedSet<T>
BitSet<E>

Source<T, E>
Stream<T, E>
Signal<T>
TaskId
TaskKey
CancelScope

StatePath
Patch
EventLog
TraceLog
Snapshot
StableHash

SourceSpan
SourceAnchor
Diagnostic

Bytes
Blob
SharedSlice<T>
MemoryLease
```

`Map<K,V>` のデフォルトは `OrderedMap` に寄せるのがよいと思います。理由は、Arcweft では authored order、choice order、layer order、hook order、debug 表示順が意味を持ちやすいからです。一方、serialization や replay canonicalization には `SortedMap` を明示的に使う、という分担が安全です。

## 入れない方がいいデフォルト

デフォルト prelude からは、これは外した方がよいです。

```text
HashMap
HashSet
Rc
Arc
Mutex
RawPtr
Box in shared-memory-facing models
Vec/String inside shared memory layout
```

完全禁止ではなく、**用途を限定して名前を変える**のがよいです。

```text
EphemeralHashMap<K, V>   // replay-visible にしてはいけない
HostArc<T>               // host adapter 側だけ
SharedPodSlice<T>        // shared memory 用
```

Arcweft は Core と data-format が Sans I/O で、host adapter が filesystem、network、clock、backend resource を扱う設計なので、標準データ構造もその境界を壊さない名前にした方がよいです。

一言でまとめると、Arcweft のデフォルトデータ構造は **`Option / Result / Need`、`Ref / Handle / Lease`、決定的 collection、typed arena、LayerTree、DependencyGraph、`Patch/Trace、Source/Stream、SharedSlice`** を中心にするのがよいです。特に `HashMap` をデフォルトにせず、`OrderedMap`と`SortedMap` を使い分ける方針が Arcweft らしいです。
