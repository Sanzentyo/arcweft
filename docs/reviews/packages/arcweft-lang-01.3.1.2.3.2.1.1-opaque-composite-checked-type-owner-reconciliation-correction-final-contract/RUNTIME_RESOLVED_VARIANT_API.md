# RuntimeResolvedVariant API and deletion contract

## 1. Final checked selection

Owner: `arcweft_runtime_plan::semantic_facts`.

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeCheckedVariantSelection {
    owner: RuntimeCheckedType,
    ordinal: u32,
    case: RuntimeCheckedVariantCase,
}

impl RuntimeCheckedVariantSelection {
    pub const fn owner(&self) -> &RuntimeCheckedType;
    pub const fn ordinal(&self) -> u32;
    pub const fn case(&self) -> &RuntimeCheckedVariantCase;
    pub fn name(&self) -> &str;
    pub const fn payload(&self) -> Option<&RuntimeCheckedType>;
}

impl RuntimeResolvedVariant {
    pub fn checked_selection(
        &self,
    ) -> Result<RuntimeCheckedVariantSelection, RuntimeResolvedVariantError>;

    // retained diagnostic/source accessors
    pub const fn owner(&self) -> &RuntimeVariantOwner;
    pub const fn ordinal(&self) -> u32;
    pub fn name(&self) -> &str;
}
```

Existing constructors `project`, `character`, `builtin_closed`,
`option_some`, `option_none`, `result_ok`, and `result_err` remain, but all
successful lowerers call `checked_selection` exactly once.

## 2. Projection algorithm

1. Project the complete `RuntimeVariantOwner` through its private inherent
   `project_checked_type` method.
2. Obtain the canonical case from `RuntimeCheckedType::variant_case(ordinal)`.
3. Compare the resolved source case name with the canonical case name.
4. Return owner, ordinal, and case together.
5. Expression/pattern lowering validates payload presence and payload checked
   type against `selection.case()`.

## 3. Errors

```rust
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeResolvedVariantError {
    #[error("variant owner checked-type projection failed")]
    CheckedTypeProjection(#[from] RuntimeCheckedTypeProjectionError),

    #[error("variant case ordinal {ordinal} is outside {case_count} cases")]
    CaseOrdinal {
        ordinal: u32,
        case_count: u32,
    },

    #[error("variant case {ordinal} resolved as `{actual}`, expected `{expected}`")]
    CaseName {
        ordinal: u32,
        expected: String,
        actual: String,
    },
}
```

Payload absence/mismatch remains in the expression/pattern lowering error enum
that owns the source expression/pattern range; it is not hidden in semantic
facts.

## 4. Compile-clean deletions

The A1.2 cut deletes:

- public `RuntimeVariantOwner::checked_type`;
- `RuntimeCheckedType::accepts_variant_case`;
- direct calls from final expression, final pattern, and AWBC lowerers to an
  owner checked type plus an independent ordinal/name check;
- any helper producing `Result<T,Never>`, `Result<Never,E>`, or
  `Option<Never>` solely from the selected constructor;
- any fallback to `Dynamic` or source spelling when projection fails;
- any separate pattern-only owner projection.

`RuntimeVariantOwner::project_checked_type` remains private and is used only by
`RuntimeResolvedVariant::checked_selection`. Behavior is implemented on the
original Arcweft-owned enums/structs; no extension trait is introduced.
