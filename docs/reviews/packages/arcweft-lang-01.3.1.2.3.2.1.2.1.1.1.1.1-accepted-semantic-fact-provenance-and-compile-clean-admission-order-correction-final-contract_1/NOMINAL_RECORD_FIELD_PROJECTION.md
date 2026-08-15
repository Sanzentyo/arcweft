# Nominal record field projection

## Constructor and accessors

The exact API is specified in `RUST_API.md`. The constructor converts a
zero-based defining ordinal by `checked_add(1)`, `u32::try_from`, and
`NonZeroU32::new`, then invokes the existing private accepted
`RuntimeRecordFieldId` constructor. The error enum is:

```rust
pub enum RuntimeNominalRecordFieldProjectionError {
    FieldIdentityExhausted { zero_based_ordinal: usize },
}
```

The aggregate projection constructor has deterministic precedence:

1. nominal/semantic identity mismatch;
2. layout identity mismatch;
3. field count mismatch;
4. constructor conversion/identity exhaustion;
5. duplicate field ID;
6. gap/out-of-order field ID;
7. accepted field semantic type mismatch.

The compiler call site is the accepted nominal layout enumeration in defining
order:

```rust
let fields = layout
    .fields()
    .iter()
    .enumerate()
    .map(|(ordinal, field)| {
        RuntimeNominalRecordFieldProjection::try_from_accepted_ordinal(
            ordinal,
            field.semantic_type(),
        )
    })
    .collect::<Result<Box<[_]>, _>>()?;
```

No public field literal, `from_raw`, writable accessor, unchecked row list, or
source-name lookup is permitted.
