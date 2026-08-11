\
# SURFACE INVENTORY

## Decision summary

The final surface is a direct correction of existing Arcweft-owned types. There are no new public product types and no compatibility surfaces.

| Surface | Current `main` evidence | Final contract | Compatibility action |
|---|---|---|---|
| `CurriedCallableId` fields | `base: Box<CallableCandidateId>`, `next_group: CallableGroupIndex` | Unchanged | None |
| `CurriedCallableId::try_new(base, next_group)` | Two arguments; no schema | Unchanged signature | None |
| `CurriedCallableId::base()` | Present | Unchanged | None |
| `CurriedCallableId::next_group()` | Present | Unchanged | None |
| recursive wrapper check | Rejects `Curried` and `DataLast` | Unchanged, remains first | None |
| group-zero identity error | `CallableIdentityError::MissingGroup` | Replace with `InvalidCurriedGroup` | Delete old variant directly |
| nonzero schema membership | Not provable in ID constructor | Checked in `ResolvedCallable::try_new` | No extra constructor |
| `CallableCandidateId::Curried` | Present | Unchanged | None |
| `CallableInstantiation::Curried { base, group }` | Present | Unchanged | None |
| `ResolvedCallable` fields | ID, origin, `Arc` schema, instantiation, equivalent sources, authority | Unchanged | None |
| `ResolvedCallable::try_new` signature | Present and schema-owning | Unchanged signature; corrected behavior | None |
| canonical curried pair | Curried ID pair accepted | Retained | None |
| base ID + Curried instantiation | Currently accepted when group exists | Rejected as `InvalidResolvedCallable` | Delete alternate success arm |
| absent curried schema group | Currently folded into `InvalidResolvedCallable` | `InvalidCallGroup { candidate: base, group }` | Use existing error |
| `ResolveCallError::InvalidCallGroup` | Present | Retained | None |
| diagnostic mapping | `InvalidCallGroup` -> `CallableDiagnosticCode::InvalidCallGroup` | Retained | None |
| shared resolver | Typed substrate exists; production migration not yet complete | All curried success must use existing resolved boundary | No second resolver |

## Exact changed enum fragment

```rust
pub enum CallableIdentityError {
    Scalar(#[from] CallableScalarError),
    InvalidCurriedGroup {
        base: Box<CallableCandidateId>,
        group: CallableGroupIndex,
    },
    InvalidCurriedBase {
        base: Box<CallableCandidateId>,
    },
    // existing data-last variants unchanged
}
```

Deleted without replacement or alias:

```rust
MissingGroup {
    base: Box<CallableCandidateId>,
    group: CallableGroupIndex,
}
```

## Exact retained resolver fragment

```rust
pub enum ResolveCallError {
    // ...
    InvalidCallGroup {
        candidate: CallableCandidateId,
        group: CallableGroupIndex,
    },
    // ...
}
```

No new resolver error variant is needed. `InvalidCallGroup` already has the correct public diagnostic code and fields.

## Visibility contract

- `CurriedCallableId`, its constructor, and accessors remain public through `arcweft_lang_sema::callable`.
- `CallableIdentityError`, `ResolveCallError`, and `CallableDiagnosticCode` remain public through the same module.
- `ResolvedCallable::try_new` remains public.
- Any test injection for corrupt-world evidence remains crate-private under `#[cfg(test)]` or uses existing public typed constructors.
- No unchecked public constructor is added.

## Ownership boundaries

| Rule | Owner |
|---|---|
| group index fits `u16` | `CallableGroupIndex` |
| `next_group != 0` | `CurriedCallableId::try_new` |
| no `Curried`/`DataLast` recursive wrapper | `CurriedCallableId::try_new` |
| group exists in complete schema | `ResolvedCallable::try_new` |
| project/standard/adapter lookup | shared resolver and catalog context |
| accepted request/world identity | AW-AH-009.3.2 context before resolver work |
| public diagnostic code | `ResolveCallError::code` |
| old-checker removal | family migration cut after direct evidence |

## Deliberately absent surfaces

There is no:

- `CurriedCallableId::try_new_with_schema`;
- `CurriedCallableId::new_unchecked`;
- `ValidatedCurriedCallable`;
- `CurriedResolvedCallable`;
- `LegacyCurriedCallableId`;
- `From<CallableIdentityError> for ResolveCallError` blanket conversion;
- catalog handle, schema handle, or world lease stored in the ID;
- compatibility module, alias, reader, or resolver.
