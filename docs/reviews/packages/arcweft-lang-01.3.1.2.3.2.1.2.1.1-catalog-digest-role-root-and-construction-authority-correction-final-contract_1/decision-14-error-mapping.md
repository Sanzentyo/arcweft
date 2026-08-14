# Decision 14 — typed checked-value error propagation

`ERROR_MAPPING.csv` is normative. Each outer error owns the concrete table/site/slot/domain coordinate and embeds the next lower structured error with `#[source]`; it never stores only `to_string()` output.

## Required owner changes

```rust
pub enum RuntimeNominalRecordTreeError {
    CheckedValue {
        field: RuntimeRecordFieldId,
        type_path: RuntimeCheckedTypePath,
        value_path: RuntimeValuePath,
        #[source]
        source: RuntimeCheckedTypeError,
    },
    // retained identity/layout/count/field-id variants
}

pub enum CharacterDialogueValueError {
    CheckedValue {
        path: CharacterDialogueValuePath,
        #[source]
        source: RuntimeCheckedTypeError,
    },
    // retained domain/canonical variants
}
```

Restore/replay/View/plan/AWBC/VM types take the exact variants shown in the CSV. Error conversions are direct `#[from]` only where the outer coordinate is already present; otherwise the owner maps inline while attaching its coordinate. No generic boxed error, diagnostic string, or lossy enum code is accepted.

## Precedence

Descriptor/domain lookup remains before nominal identity, then layout, count, one-based field-ID derivation, first defining-order field checked failure, dialogue/View/restore domain validation, and publication. Plan/AWBC structural reference errors precede domain issuance. Once an earlier failure occurs, no later error is observable.

## Boolean convenience boundary

The public authority-bearing `RuntimeCheckedType::accepts_value` is removed. A crate-private `matches_non_authoritative_pattern` may remain only after the enclosing value has already passed typed admission and only to select a pattern branch where `false` is not an externally observable authority error. Nominal construction, dialogue, restore, replay, View, plan/AWBC admission, constants, VM, and persistence may never call the boolean method.
