# Standard Types and Prelude

Arcweft's default prelude is deterministic and Sans I/O. Types exposed through
the language surface should be suitable for replay, source traces, manifests,
and tooling inspection.

## Core ADT

```awft
Unit
Never
Option<T>
Result<T, E>
Need<T, E>
Progress
Duration
TickId
```

`Never` is the canonical bottom type name. The advanced alias `!` is allowed in
type position.

## References and Handles

```awft
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

```awft
Vec<T>
VecDeque<T>
OrderedMap<K, V>
BTreeMap<K, V>
OrderedSet<T>
BTreeSet<T>
BitSet<E>
Array<T, N>
```

`Vec<T>` is the default growable ordered sequence. It preserves authored order
and is the normal target for list literals when no fixed-size context exists.

`Array<T, N>` is a fixed-length sequence. A literal such as `[a, b, c]` can be
typed as `Array<T, 3>` when the expected type requires exactly three elements.
Without an expected fixed-length type, the same literal defaults to `Vec<T>`.

```awft
let dynamic: Vec<i32> = [1, 2, 3]
let fixed: Array<i32, 3> = [1, 2, 3]
```

`[value; N]` is reserved for fixed-length array construction.

```awft
let zeros: Array<i32, 4> = [0; 4]
```

`OrderedMap` / `OrderedSet` preserve first insertion order. `BTreeMap` /
`BTreeSet` provide canonical sorted order for snapshots and replay-visible
serialization. `HashMap` and `HashSet` are not part of the default prelude.

## Capacity

Reservable containers provide explicit capacity APIs:

```awft
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

```awft
Bytes
Blob
BlobRef
SharedSliceDesc
MemoryLease
PodSlice<T>
```

Adapters may back these descriptors with mmap, shared memory, files, or GPU
buffers, but the core data format remains Sans I/O.

## Facade Prelude

The Rust facade crate `arcweft` re-exports the current minimal prelude:

```rust
use arcweft::prelude::*;
```

Low-level crates should depend on narrow crates directly instead of importing
the facade prelude.
