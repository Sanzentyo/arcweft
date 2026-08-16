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

## Numeric Standard Modules

Numeric primitives are explicit-width types such as `i32`, `u64`, `f32`, and
`f64`. The float standard modules expose constants, math functions, predicates,
bit conversion, and explicit casts without implicit widening or narrowing:

```arcw
std.f32.nan
std.f32.infinity
std.f32.epsilon
std.f32.pi
std.f32.sqrt(x)
std.f32.powf(x, y)
std.f32.atan2(y, x)
std.f32.mul_add(a, b, c)
std.f32.is_nan(x)
std.f32.to_bits(x)
std.f32.from_bits(bits)
std.f32.to_f64(x)

std.f64.nan
std.f64.infinity
std.f64.epsilon
std.f64.pi
std.f64.sqrt(x)
std.f64.powf(x, y)
std.f64.atan2(y, x)
std.f64.mul_add(a, b, c)
std.f64.is_nan(x)
std.f64.to_bits(x)
std.f64.from_bits(bits)
std.f64.to_f32(x)
```

The VM reference backend defines these functions. JIT/AOT backends may lower a
subset directly and otherwise use the VM semantics as the correctness baseline.

## Dense Matrix And Tensor Types

Dense numeric matrix and tensor values are explicit about scalar width:

```arcw
MatrixF32
TensorF32
MatrixF64
TensorF64
```

`math.matmul_f32`, `math.matrix_add_f32`, and `math.tensor_add_f32` operate on
the `f32` types. `math.matmul_f64`, `math.matrix_add_f64`, and
`math.tensor_add_f64` operate on the `f64` types. The `f64` forms keep `f64`
storage through VM and CPU accelerator paths instead of widening or narrowing
through `f32`.

Runtime CLI values use path-free typed payload strings:

```bash
--value lhs=matrix/f32/4x4:1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1
--value rhs=matrix/f64/2x2:1.5,2,3.25,4.5
--value t=tensor/f64/2x2:1.5,2.25,3.75,4.5
```

The portable `wgpu` math backend is currently `f32`-only. `f64` matrix/tensor
kernels use scalar, glam 4x4, or ndarray CPU backends.

## Adapter-Contributed Tensor Ops

Forward inference uses the same dense `TensorF32` runtime value, but inference
namespaces are not part of Arcweft Core or the default prelude. A runner may
inject adapter-provided namespaces such as `conv2d` and `infer` into the
type-checking environment, and execution resolves those calls through a
runtime external-call adapter:

```arcw
let features = conv2d.valid_f32(image, kernel, 1usize, 1usize)
let hidden = infer.relu_f32(features)
let pooled = infer.max_pool2d_f32(hidden, 2usize, 2usize, 2usize, 2usize)
let flat = infer.flatten_outer_f32(pooled)
let logits = infer.matmul_bias_add_f32(flat, dense_weight, dense_bias)
let class = infer.argmax_last_dim_f32(logits)
```

The current optional native inference adapter contributes valid NCHW/OIHW
`conv2d.valid_f32`, `infer.matmul_f32`, `infer.add_f32`,
last-dimension `infer.bias_add_f32`, fused
`infer.matmul_bias_add_f32`, `infer.relu_f32`,
`infer.max_pool2d_f32`, `infer.softmax_last_dim_f32`,
`infer.argmax_last_dim_f32`, and outer-preserving
`infer.flatten_outer_f32`. Shape validation is performed by the adapter tensor
ops and by the Rust-side inference graph builder. The parser treats these as
ordinary dotted method calls; the language checker only accepts them when an
adapter profile injects the corresponding symbols and methods.

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
let bad: Array<i32, 2> = [1, 2, 3]  // error
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
ViewTree<T>
StableGraph<N, E>
DependencyGraph<N>
RouteGraph
```

`Arena<T>` is append-only and preserves insertion order. `StableArena<T>` has
the same surface contract but signals that IDs are suitable for save, replay,
and debug manifests. `SlotMap<T>` uses `GenerationalId<T>` so stale runtime
handles can be rejected.

`Tree<T>` is the shared deterministic parent/child shape for layer, scene, and
View trees. `StableGraph<N, E>` preserves node insertion order and explicit edge
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

`Stream<T,E>` is the sole asynchronous sequence descriptor. Live callbacks,
permissions, and host polling are external-capability concerns; an ordinary
capability operation returning `Stream<T,E>` exposes only the typed stream to
the language and runtime.

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

