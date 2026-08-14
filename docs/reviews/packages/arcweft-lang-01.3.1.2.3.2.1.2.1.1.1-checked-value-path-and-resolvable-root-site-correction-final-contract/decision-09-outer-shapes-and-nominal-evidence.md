# Decision 09 — complete outer-shape evidence and nominal semantic identity

## Physical outer shape

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeValueShape {
    Unit,
    Bool,
    Signed,
    Unsigned,
    F32,
    F64,
    MatrixF32,
    MatrixF64,
    TensorF32,
    TensorF64,
    String,
    Char,
    Duration,
    Range,
    Iterator,
    EntityReference,
    Tuple,
    Sequence,
    Record,
    NominalRecord,
    Opaque,
    Function,
    Variant,
}
```

`RuntimeValue::shape()` is added to the existing `RuntimeValue` inherent implementation; no helper trait is used. `RUNTIME_VALUE_OUTER_SHAPES.csv` is exhaustive over the current enum and is enforced by one compile-time exhaustive match test.

`RuntimeCheckedShape` describes the expected checked algebra and retains `Bytes` as a semantic expected shape. Physical bytes are still `RuntimeValue::Seq`, so actual shape is always `Sequence`. Bytes validation first accepts physical Sequence, then validates every element against `RuntimeCheckedType::Unsigned(RuntimeUnsignedIntWidth::U8)` with `SequenceItem/SequenceElement` paths. A non-sequence value yields `OuterShape { expected: Bytes, actual }`; a sequence with a non-U8 element yields the exact child width/shape failure.

Matrix, tensor, range, iterator, record, and function values are always reported with their exact actual shape even though the current `RuntimeCheckedType` algebra has no accepting case for several of them. They therefore produce deterministic `OuterShape` evidence rather than an incomplete/unknown shape.

## Nominal semantic identity

`RuntimeNominalRecordValue` legitimately stores nominal ID, layout, and fields; it does not store `RuntimeSemanticTypeId`. The retry error that claimed a raw `actual: RuntimeSemanticTypeId` is removed.

Validation order is:

1. expected `RuntimeCheckedType::Nominal` supplies `nominal`, `semantic_identity`, and `layout`;
2. compare raw outer shape, raw nominal ID, and raw layout;
3. ask `RuntimeCheckedValueContext` to resolve the exact authority domain + expected nominal + expected semantic ID + expected layout;
4. the admitted descriptor supplies its retained semantic ID, nominal ID, layout, defining-order fields, and checked field types;
5. descriptor disagreement returns `RuntimeNominalCatalogLookupError::{SemanticIdentity,NominalIdentity,Layout}` with the descriptor as the legitimate `actual` source;
6. validate count, one-based field IDs, and every field in defining order under the same work/depth/path budget.

No semantic ID is derived from a nominal name, display path, `TypeLayoutHash`, field bytes, or caller assertion. `RuntimeCheckedTypeError` retains only:

```rust
NominalLookup {
    type_path: RuntimeCheckedTypePath,
    value_path: RuntimeValuePath,
    #[source]
    source: RuntimeNominalCatalogLookupError,
}
```

The impossible `NominalSemanticIdentity { actual_from_raw, ... }` variant is deleted.
