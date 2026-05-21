# Standard Types and Prelude

Arcweft's default prelude is deterministic and Sans I/O. Types exposed through
the language surface should be suitable for replay, source traces, manifests,
and tooling inspection.

## Core ADT

```arcw
Unit
Never
Option<T>
Result<T, E>
Need<T, E>
Progress
Duration
TickId
LogicalTime
```

`Never` is the canonical bottom type name. The advanced alias `!` is allowed in
type position.

## References and Handles

```arcw
Id<T>
Ref<T>
Handle<T>
WeakHandle<T>
Borrow<'a, T>
Slice<'a, T>
Lease<T>
```

`Ref<T>` is a symbolic entity reference. It is not a Rust-style memory borrow.
`Borrow<'a, T>` and `Slice<'a, T>` are lifetime-bound views.

## Collections

Arcweft uses Rust-like collection names for the default surface vocabulary:

```arcw
Vec<T>
VecDeque<T>
SmallList<T, N>
OrderedMap<K, V>
BTreeMap<K, V>
SortedMap<K, V>
OrderedSet<T>
BTreeSet<T>
SortedSet<T>
BitSet<E>
Array<T, N>
```

`Vec<T>` is the default growable ordered sequence. It preserves authored order
and is the normal target for bracket sequence literals when no fixed-size context exists.

`Array<T, N>` is a fixed-length sequence. A literal such as `[a, b, c]` can be
typed as `Array<T, 3>` when the expected type requires exactly three elements.
Without an expected fixed-length type, the same literal defaults to `Vec<T>`.

```arcw
let dynamic: Vec<i32> = [1, 2, 3]
let fixed: Array<i32, 3> = [1, 2, 3]
```

`[value; N]` is fixed-length array construction. `N` must be an integer
constant in the current Phase 2 surface.

```arcw
let zeros: Array<i32, 4> = [0; 4]
```

Length mismatch is a verifier/type-checking diagnostic:

```arcw
let bad: Array<i32, 2> = [1, 2, 3]  # error
```

`OrderedMap` / `OrderedSet` preserve first insertion order. `BTreeMap` /
`BTreeSet` provide canonical sorted order for snapshots and replay-visible
serialization. `SortedMap` / `SortedSet` are surface aliases for the sorted
contract. `HashMap` and `HashSet` are not part of the default prelude.

`SmallList<T, N>` is a deterministic small-list abstraction. The Rust MVP keeps
it Vec-backed so the public data contract stays Sans I/O and does not promise a
specific inline-storage implementation.

## Arena, Slot, Graph, and Tree Data

Compiler, runtime, presentation, and tooling code use explicit ID-bearing data
structures instead of naked numeric indexes:

```arcw
Arena<T>
StableArena<T>
ArenaId<T>
SlotMap<T>
GenerationalId<T>
SparseSet<T>
EntityStore<T>

Tree<T>
LayerTree<T>
SceneGraph<T>
UiTree<T>
StableGraph<N, E>
DependencyGraph<N>
RouteGraph
```

`Arena<T>` is append-only and preserves insertion order. `StableArena<T>` has
the same surface contract but signals that IDs are suitable for save, replay,
and debug manifests. `SlotMap<T>` uses `GenerationalId<T>` so stale runtime
handles can be rejected.

`Tree<T>` is the shared deterministic parent/child shape for layer, scene, and
UI trees. `StableGraph<N, E>` preserves node insertion order and explicit edge
order; `DependencyGraph<N>` is a unit-edge graph for memo, asset, and build
dependencies.

## State, Patch, and Replay Data

These types describe deterministic state deltas and replay-visible logs:

```arcw
StatePath
Patch<T>
PatchSet<T>
Diff<T>
Versioned<T>
Snapshot<T>
StableHash
EventLog<E>
TraceLog<E>
TaskQueue<T>
EventQueue<E>
NeedCacheState<T, E, P = Unit>
RingBuffer<T>
Signal<T>
SignalBus<T>
Source<T, E>
Stream<T, E>
```

`StatePath` is a structured dotted path. `PatchSet`, queues, and logs preserve
append order. `NeedCacheState` mirrors the state of cached asynchronous work
without depending on a host runtime or I/O adapter.

`RingBuffer<T>` is a bounded FIFO data structure for source/device queues. It is
still pure data: adapter crates decide whether the backing transport is shared
memory, browser events, native callbacks, or another host mechanism. `Signal<T>`
and `SignalBus<T>` model replay-visible last-value state and do not perform
notification I/O themselves.

`Source<T,E>` and `Stream<T,E>` are deterministic descriptors. Live callbacks,
permissions, and host polling are adapter concerns; the language and runtime
observe them through explicit events and replayable queue state.

## Source and Tooling Data

Compiler, verifier, formatter, and LSP-facing APIs share source identity and
diagnostic data:

```arcw
SourceRange
SourceSpan
SourceAnchor
LineIndex
Diagnostic
DiagnosticBag
SyntaxNode
AstNodeId
HirId
```

`SourceSpan` / `SourceRange` are byte-oriented source coordinates with optional
line/column positions for display. `SourceAnchor` is used in runtime traces and
error propagation. `SyntaxNode` is the lossless CST node type owned by
`arcweft-lang-syntax`; `AstNodeId` and `HirId` are stable IDs for compiler and
tooling manifests.

## Capacity

Reservable containers provide explicit capacity APIs:

```arcw
let items = Vec<String>.with_capacity(8)
items.reserve(16)
items.shrink()
items.shrink_to(4)

let bytes = Bytes.with_capacity(4096)
bytes.shrink()
```

`shrink` is available because Arcweft source often builds temporary lists during
tooling, parsing, localization, and trace preparation.

## Memory Data Plane

Shared-memory-facing models must not store `Vec<T>`, `String`, hash maps, raw
pointers, or host handles directly. Use descriptor types instead:

```arcw
Bytes
Blob
BlobRef
FrameArena<T>
BumpArena<T>
SharedSliceDesc
SharedSlice<T>
MemoryLease
PodSlice<T>
```

Adapters may back these descriptors with mmap, shared memory, files, or GPU
buffers, but the core data format remains Sans I/O.

## Text and Localization Data

Dialogue and presentation text use structured text data near the prelude:

```arcw
TextKey
Localized<T>
LocaleMap<T>
RichText
TextRun
InlineTag
RubyText
DialogueLine
LinePlan
```

`String` remains available for ordinary string values. User-facing narrative
text should prefer `TextKey`, `RichText`, and `Localized<RichText>` where
localization, ruby, reveal, or text effects matter.

## Rust Facade Namespaces

The Rust facade crate `arcweft` exposes crate-family namespaces instead of a
flat compatibility prelude:

```rust
use arcweft::core::frame::RuntimeStepInput;
use arcweft::dialogue::DialogueLine;
use arcweft::presentation::PresentationHandle;
```

Low-level crates should depend on narrow crates directly instead of importing
the facade crate.

