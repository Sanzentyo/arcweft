# Decision 13 — typed checked-value and unique-Choice validation

## Owner and API

Owner: the existing `arcweft_core::pattern::RuntimeCheckedType` inherent implementation. No extension trait or ad hoc predicate is added.

```rust
pub const MAX_RUNTIME_CHECKED_VALUE_WORK: u32 = 65_536;

pub struct RuntimeCheckedValueValidator<'generation> {
    generation: &'generation AdmittedRuntimeGeneration,
    remaining_work: u32,
    depth: usize,
    type_path: RuntimeCheckedTypePath,
    value_path: RuntimeValuePath,
}

impl RuntimeCheckedType {
    pub fn validate_value(
        &self,
        value: &RuntimeValue,
        validator: &mut RuntimeCheckedValueValidator<'_>,
    ) -> Result<(), RuntimeCheckedTypeError>;

    pub(crate) fn matches_non_authoritative_pattern(&self, value: &RuntimeValue) -> bool;
}
```

Only an admitted plan/AWBC/dialogue/View/restore context can construct `RuntimeCheckedValueValidator`. Its constructor is crate-private and initializes `remaining_work=65_536`, `depth=0`, and root paths. The current public `accepts_value` is removed; the boolean convenience may remain crate-private only for already-admitted pattern dispatch where its failure is not authority evidence.

## Paths

```rust
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeCheckedTypePath(Box<[RuntimeCheckedTypePathSegment]>);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeCheckedTypePathSegment {
    SequenceItem(u32),
    TupleItem(u32),
    ChoiceAlternative(u32),
    ResultOk,
    ResultError,
    OptionSome,
    OpaquePayload,
    VariantPayload { ordinal: u32 },
    NominalField(RuntimeRecordFieldId),
}

#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeValuePath(Box<[RuntimeValuePathSegment]>);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeValuePathSegment {
    SequenceItem(u32),
    TupleItem(u32),
    ChoiceCandidate(u32),
    ResultOk,
    ResultError,
    OptionSome,
    OpaquePayload,
    VariantPayload { ordinal: u32 },
    NominalField(RuntimeRecordFieldId),
}
```

Paths expose `root`, `pushed`, and `segments` inherent methods. Indices are checked `u32`; overflow returns the work/shape error at the parent path rather than truncating.

## Error shape

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeChoiceBranchMismatch {
    alternative: u32,
    type_path: RuntimeCheckedTypePath,
    value_path: RuntimeValuePath,
    source: Box<RuntimeCheckedTypeError>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeValueShape {
    Unit, Bool, Signed, Unsigned, F32, F64, String, Char, Bytes, Duration,
    EntityReference, Sequence, Tuple, Record, NominalRecord, Opaque, Function,
    Iterator, Variant,
}

#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
pub enum RuntimeCheckedTypeError {
    #[error("runtime checked-value nesting exceeds {limit}")]
    NestingLimit { limit: usize, type_path: RuntimeCheckedTypePath, value_path: RuntimeValuePath },
    #[error("runtime checked-value work exceeds {limit}")]
    WorkBudget { limit: u32, consumed: u32, type_path: RuntimeCheckedTypePath, value_path: RuntimeValuePath },
    #[error("runtime value has wrong outer shape")]
    OuterShape { expected: RuntimeCheckedShape, actual: RuntimeValueShape, type_path: RuntimeCheckedTypePath, value_path: RuntimeValuePath },
    #[error("signed integer is outside the accepted width")]
    SignedWidth { width: RuntimeSignedIntWidth, value: i128, type_path: RuntimeCheckedTypePath, value_path: RuntimeValuePath },
    #[error("unsigned integer is outside the accepted width")]
    UnsignedWidth { width: RuntimeUnsignedIntWidth, value: u128, type_path: RuntimeCheckedTypePath, value_path: RuntimeValuePath },
    #[error("tuple length differs")]
    TupleLength { expected: usize, actual: usize, type_path: RuntimeCheckedTypePath, value_path: RuntimeValuePath },
    #[error("opaque owner differs")]
    OpaqueOwner { expected: RuntimeOpaqueTypeOwner, actual: RuntimeOpaqueTypeOwner, type_path: RuntimeCheckedTypePath, value_path: RuntimeValuePath },
    #[error("nominal identity differs")]
    NominalIdentity { expected: RuntimeNominalTypeId, actual: RuntimeNominalTypeId, type_path: RuntimeCheckedTypePath, value_path: RuntimeValuePath },
    #[error("nominal semantic identity differs")]
    NominalSemanticIdentity { expected: RuntimeSemanticTypeId, actual: RuntimeSemanticTypeId, type_path: RuntimeCheckedTypePath, value_path: RuntimeValuePath },
    #[error("nominal layout differs")]
    NominalLayout { expected: TypeLayoutHash, actual: TypeLayoutHash, type_path: RuntimeCheckedTypePath, value_path: RuntimeValuePath },
    #[error("variant owner differs")]
    VariantOwner { expected: RuntimeVariantIdentity, actual: RuntimeVariantIdentity, type_path: RuntimeCheckedTypePath, value_path: RuntimeValuePath },
    #[error("variant ordinal differs")]
    VariantOrdinal { expected: u32, actual: u32, type_path: RuntimeCheckedTypePath, value_path: RuntimeValuePath },
    #[error("variant name differs")]
    VariantName { expected: String, actual: String, type_path: RuntimeCheckedTypePath, value_path: RuntimeValuePath },
    #[error("variant payload presence differs")]
    VariantPayloadPresence { expected: bool, actual: bool, type_path: RuntimeCheckedTypePath, value_path: RuntimeValuePath },
    #[error("Choice has no matching alternative")]
    ChoiceNoMatch { branches: Box<[RuntimeChoiceBranchMismatch]>, type_path: RuntimeCheckedTypePath, value_path: RuntimeValuePath },
    #[error("Choice has more than one matching alternative")]
    ChoiceAmbiguous { first: u32, second: u32, type_path: RuntimeCheckedTypePath, value_path: RuntimeValuePath },
    #[error("nominal admitted-shape lookup failed")]
    NominalLookup { source: RuntimeNominalCatalogLookupError, type_path: RuntimeCheckedTypePath, value_path: RuntimeValuePath },
    #[error("nominal tree validation failed")]
    NominalTree { source: Box<RuntimeNominalRecordTreeError>, type_path: RuntimeCheckedTypePath, value_path: RuntimeValuePath },
}
```

`RuntimeCheckedShape` is an exact enum mirroring the existing checked algebra: Never, Unit, Bool, Signed(width), Unsigned(width), F32, F64, String, Char, Duration, EntityReference, Bytes, Sequence, Tuple, Choice, Nominal, Opaque, Variant, Result, Option.

## Work, depth, and Choice algorithm

- Charge one work unit before validating every expected type node.
- For a Choice, additionally charge one unit immediately before each alternative. Work consumed by a failed or successful branch is never rolled back.
- Increment depth when descending into sequence item, tuple item, Choice alternative, Result/Option payload, opaque payload, Variant payload, or nominal field. The existing `MAX_RUNTIME_VALUE_NESTING_DEPTH=64` applies; depth is checked before outer shape.
- Evaluate every Choice alternative in source order even after one matches. A nesting/work failure in any later branch is returned immediately and cannot be hidden by an earlier match.
- Store each ordinary mismatch in source order. Zero successes returns `ChoiceNoMatch` with all ordered branch mismatches. Two or more successes returns `ChoiceAmbiguous` with the first two successful indices after all branches have been evaluated. Exactly one success returns success.
- Nominal identity, semantic identity, layout, admitted-shape lookup, and defining-order tree validation use the same validator and remaining budget. No new budget is allocated at nominal boundaries.

## Deterministic first-error order

1. nesting and shared work;
2. outer runtime shape;
3. checked owner/width;
4. Variant owner;
5. Variant ordinal and canonical name;
6. payload presence;
7. recursive payload;
8. every Choice alternative in source order;
9. zero-match branch evidence or first two matching indices;
10. nominal admitted-shape lookup/tree validation;
11. authority-domain publication.
