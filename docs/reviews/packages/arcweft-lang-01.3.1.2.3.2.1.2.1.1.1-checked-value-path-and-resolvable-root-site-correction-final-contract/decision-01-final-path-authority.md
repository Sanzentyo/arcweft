# Decision 01 — one final checked-value path authority

## Selected owner

The legitimate canonical value-path owner remains the current production module:

- definition: `arcweft_core::value::ownership::path`
- public re-export: `arcweft_core::value::ownership::{RuntimeValuePath, RuntimeValuePathSegment, RuntimeValuePathError, MAX_RUNTIME_VALUE_PATH_SEGMENTS}`
- crate-root behavior: unchanged; no re-export from `pattern` under the same names

The retry declarations `arcweft_core::pattern::RuntimeValuePath` and `RuntimeValuePathSegment` are deleted from the design. They never land in production.

## Final value path declarations

```rust
pub const MAX_RUNTIME_VALUE_PATH_SEGMENTS: u32 = 64;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RuntimeValuePath(Box<[RuntimeValuePathSegment]>);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RuntimeValuePathSegment {
    TupleElement(u32),              // canonical tag 0
    SequenceElement(u64),           // tag 1
    TupleColumn(u32),               // tag 2
    RecordField(RuntimeRecordFieldId),       // tag 3
    RecordColumn(RuntimeRecordFieldId),      // tag 4
    NominalRecordField(RuntimeRecordFieldId),// tag 5
    FunctionCapture(RuntimeCaptureSlotId),   // tag 6
    VariantPayload,                 // tag 7
    IteratorRemainder(u64),         // tag 8
    IteratorWitnessState,           // tag 9
    OpaquePayload,                  // tag 10
}
```

`RuntimeValuePath` and `RuntimeValuePathSegment` retain manual `Ord`/`PartialOrd`. Ordering is lexicographic by segment; segment comparison is canonical tag first, then the payload's natural order. `OpaquePayload` has no payload and sorts after every existing segment. Existing tags and comparisons do not move.

Exact inherent API:

```rust
impl RuntimeValuePath {
    #[must_use] pub fn root() -> Self;
    pub fn try_from_segments(
        segments: impl IntoIterator<Item = RuntimeValuePathSegment>,
    ) -> Result<Self, RuntimeValuePathError>;
    #[must_use] pub fn segments(&self) -> &[RuntimeValuePathSegment];
    #[must_use] pub const fn is_root(&self) -> bool;
    pub fn child(
        &self,
        segment: RuntimeValuePathSegment,
    ) -> Result<Self, RuntimeValuePathError>;
}

impl RuntimeValuePathSegment {
    #[must_use] pub const fn canonical_tag(self) -> u8;
}
```

Construction remains public because this is already the public canonical evidence type. The private field, 64-segment limit, and checked `child` prevent an invalid path. There is no `Default`, mutable segment access, unchecked constructor, `Deref`, `From<Vec<_>>`, or truncating conversion.

## Checked-type diagnostic path

Expected-type graph edges are not all physical value edges. Therefore the design adds one distinctly named, non-Serde diagnostic type in `crates/arcweft-core/src/pattern/validation.rs`, re-exported by `arcweft_core::pattern`:

```rust
pub const MAX_RUNTIME_CHECKED_TYPE_PATH_SEGMENTS: u32 = 64;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeCheckedTypePath(Box<[RuntimeCheckedTypePathSegment]>);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeCheckedTypePathSegment {
    SequenceItem(u64),              // tag 0
    TupleItem(u32),                 // tag 1
    ChoiceAlternative(u32),         // tag 2
    ResultOk,                       // tag 3
    ResultError,                    // tag 4
    OptionSome,                     // tag 5
    OpaquePayload,                  // tag 6
    VariantPayload { ordinal: u32 },// tag 7
    NominalField(RuntimeRecordFieldId), // tag 8
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeCheckedTypePathError {
    #[error("runtime checked-type path has {actual} segments; maximum is {maximum}")]
    TooDeep { maximum: u32, actual: usize },
}

impl RuntimeCheckedTypePath {
    pub(crate) fn root() -> Self;
    pub(crate) fn child(
        &self,
        segment: RuntimeCheckedTypePathSegment,
    ) -> Result<Self, RuntimeCheckedTypePathError>;
    #[must_use] pub fn segments(&self) -> &[RuntimeCheckedTypePathSegment];
    #[must_use] pub const fn is_root(&self) -> bool;
}
```

It derives neither `Serialize` nor `Deserialize` and has no public constructor. It is evidence carried by structured errors, not a second persistence/wire authority. Choice alternatives, Result/Option branch names, and checked nominal field expectations belong here; physical values continue to use the sole `RuntimeValuePath`.
