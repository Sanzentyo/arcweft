# Decision 02 — `ViewRegistry` runtime digest

## Complete owner and transcript

Owner: `crates/arcweft-view/src/view/registry.rs`, module `arcweft_view::view::registry`.

The current Arcweft implementation descriptor is not sufficient because it stores only `ViewProgramId`. Directly replace that unreleased shape in place:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ViewImplementation {
    Rust(RustViewId),
    Arcweft {
        program: ViewProgramId,
        revision: AcceptedViewProgramRevision,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProjectedRuntimeViewId([u8; 32]);

impl ProjectedRuntimeViewId {
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32];
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewRegistryRuntimeDigest([u8; 32]);

impl ViewRegistryRuntimeDigest {
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32];
}

#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
pub enum ViewRegistryRuntimeDigestError {
    #[error(transparent)]
    Registry(#[from] ViewRegistryError),
    #[error(transparent)]
    Identity(#[from] ViewIdentityError),
    #[error("View registry contains {observed} live public rows; maximum is {maximum}")]
    EntryLimit { observed: usize, maximum: usize },
    #[error("public View {view} points to vacant registry slot {slot:?}")]
    DanglingPublicIndex { view: ViewId, slot: ViewRegistryId },
    #[error("public View map key {key} differs from descriptor identity {descriptor}")]
    PublicIdMismatch { key: ViewId, descriptor: ViewId },
    #[error("public View map key {view} points to an anonymous descriptor")]
    AnonymousPublicEntry { view: ViewId },
    #[error("View identity field {field:?} has {bytes} UTF-8 bytes; maximum is {maximum}")]
    StringLength {
        view: ViewId,
        field: ViewRegistryStringField,
        bytes: usize,
        maximum: u32,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ViewRegistryStringField {
    ViewId,
    ViewProgramId,
}

impl ViewId {
    pub fn projected_runtime_id_v1(&self) -> ProjectedRuntimeViewId;
}

impl ViewRegistry {
    pub const MAX_RUNTIME_DIGEST_PUBLIC_ROWS: usize = 65_536;

    pub fn runtime_digest_v1(
        &self,
    ) -> Result<ViewRegistryRuntimeDigest, ViewRegistryRuntimeDigestError>;
}
```

The digest and projected-ID fields are private and have no Serde, Default, raw-byte constructor, or caller-supplied digest path. Methods are inherent on the current owners; no extension trait, side table, or display-name resolver is introduced.

## Stable public View identity projection

Domain:

```text
arcweft.runtime-view-id.v1\0
```

Transcript:

```text
DOMAIN
u32_le(1)
str32(ViewId::as_str())
```

The BLAKE3 output is `ProjectedRuntimeViewId`. The upper bridge losslessly copies its bytes into current core `RuntimeViewId::from_bytes`. `ViewRegistryId`, `ViewMountId`, dense indices, insertion order, Debug, and Display are not inputs.

## Registry digest transcript

Domain:

```text
arcweft.view-registry.runtime.v1\0
```

Complete transcript:

```text
DOMAIN
u32_le(1)
u32_le(live_public_row_count)
repeat self.public rows in ViewId Ord order:
    str32(view_id)
    [u8; 32] projected_runtime_view_id_v1
    u64_le(ViewSchemaId.0)
    implementation:
        Rust:
            u8(0x00)
            u32_le(RustViewId.0)
        Arcweft:
            u8(0x01)
            str32(ViewProgramId::as_str())
            [u8; 32] AcceptedViewProgramRevision
```

All integers are little-endian; strings use `u32_le` UTF-8 byte length. `AcceptedViewProgramRevision` contributes its raw 32 canonical bytes, never its hex Serde spelling. Its existing source-owned semantic transcript and nonzero rule remain authoritative.

## Authored, generated, anonymous, retirement, and insertion order

- Every live public descriptor (`Some(ViewId)`) participates, whether the ID is authored `view.*`, generated but published under a typed `ViewId`, or engine-owned `std.view.*`.
- Anonymous Rust descriptors (`id: None`) never participate: they cannot be referred to by a stable `RuntimeViewId` and are process-local capabilities.
- An anonymous Arcweft descriptor is unrepresentable because the only Arcweft constructor requires a public `ViewId`.
- `ViewRegistryId` and slot insertion order never participate.
- A retired Arcweft slot is `None` and absent from `self.public`; the tombstone is excluded. Retirement therefore removes the public row and changes row count/digest. Tombstone count and old dense slot are not semantic.
- Re-registering a removed `ViewId` creates a new process-local slot but the same digest only when schema and exact implementation identity are byte-identical.
- Public duplicates fail candidate-first in existing `register`; digest recomputation additionally rejects a dangling public index, an anonymous descriptor reachable from the public map, or a key/descriptor ID mismatch before hashing.
- `RustViewId` is included because it is the current stable host implementation identity. Arcweft implementation identity includes both program ID and accepted semantic revision. Product artifact IDs, source maps, mount IDs, renderer caches, and display metadata are excluded.

## Limits and order

1. Require live public count at most 65,536.
2. For each `BTreeMap` row in key order, resolve slot and verify key/descriptor identity.
3. Validate all `str32` lengths and `AcceptedViewProgramRevision`.
4. Build the complete transcript.
5. Hash once and issue `ViewRegistryRuntimeDigest`.

No partial digest is observable.
