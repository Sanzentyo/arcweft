# Decision 03 — exact checked-type/value-path push rules

`CHECKED_PATH_PUSH_RULES.csv` is normative. The validator maintains both paths in one internal frame:

```rust
struct RuntimeCheckedValueValidator<'generation> {
    context: RuntimeCheckedValueContext<'generation>,
    remaining_work: u32,
    depth: u32,
    type_path: RuntimeCheckedTypePath,
    value_path: RuntimeValuePath,
}
```

## Index widths and overflow

- sequence/bytes physical indices are `u64`; enumeration converts `usize` with `u64::try_from`;
- tuple and Choice source-order indices are `u32`; enumeration converts with `u32::try_from`;
- variant ordinal is already `u32` and is copied losslessly;
- nominal field identity is the existing `RuntimeRecordFieldId`, derived by the existing one-based defining-order constructor rather than by casting an ordinal;
- every `i + 1`, `1 + i*2`, and `2 + i*2` plan-path calculation uses checked `u32` arithmetic.

Conversion failure returns:

```rust
RuntimeCheckedTypeError::PathIndexOverflow {
    edge: RuntimeCheckedPathEdge,
    index: u128,
    type_path: RuntimeCheckedTypePath,
    value_path: RuntimeValuePath,
}
```

No index is saturated, truncated, wrapped, or encoded with a different width. `RuntimeCheckedPathEdge` is a closed enum with `SequenceItem`, `TupleItem`, `ChoiceAlternative`, and `NominalFieldOrdinal`; behavior is implemented on its inherent `impl`.

## Deterministic first error

At every expected node:

1. charge one shared work unit;
2. reject depth above 64;
3. compare the physical outer shape;
4. compare owner/width/length/ordinal/name/payload-presence data local to the parent;
5. convert the child index and derive both child paths;
6. recurse.

Choice adds one work unit before each alternative and evaluates every alternative in source order under the same remaining budget. A work/depth/index failure is terminal and is never hidden by an earlier matching branch. Ordinary mismatches are retained in source order. Zero successes returns all ordered `RuntimeChoiceBranchMismatch` values; two or more returns the first two matching `u32` indices only after all alternatives have been evaluated; exactly one succeeds.

Result and Option have one physical `RuntimeValue::Variant` payload edge. Their semantic branch name changes only `RuntimeCheckedTypePath`; the value path pushes the existing `VariantPayload`. Choice has no physical edge and does not push the value path. Opaque payload pushes `OpaquePayload` on both paths. These rules prevent a second value-path grammar.
