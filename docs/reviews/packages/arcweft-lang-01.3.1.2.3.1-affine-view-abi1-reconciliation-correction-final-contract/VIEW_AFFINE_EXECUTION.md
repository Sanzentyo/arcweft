# Affine-aware final-HIR View execution

## Static admission rule

The existing runtime type/layout owner computes `RuntimeValueOwnership` for every closed `RuntimeCheckedType`. A type is admitted to retained View execution only when the complete closed layout is `Unrestricted`. Open, opaque, generic, or unknown layouts are rejected as may-be-affine.

No View-specific ownership registry is introduced. The checked View catalog retains the existing exact type plus the ownership result produced by the canonical runtime type/layout owner.

## Corrected semantic facts

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckedViewValueTransfer {
    Copy,
    Move,
}

pub struct CheckedViewValueInput {
    pub source: CheckedViewValueInputSource,
    pub value_type: RuntimeCheckedType,
    pub ownership: RuntimeValueOwnership,
    pub transfer: CheckedViewValueTransfer,
    pub source_role: CheckedViewSourceRole,
}
```

The transfer is derived, not authored:

- retained render/direct-await capture: `Copy`, ownership must be `Unrestricted`;
- handler input: `Move` from the exact event/request owner;
- handler capture from retained View state: `Copy`, ownership must be `Unrestricted`.

## Corrected product schema

```rust
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewValueTransferMode {
    Copy,
    Move,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ViewValueInputBinding {
    pub register: u16,
    pub source: ViewValueInputSource,
    pub value_type: RuntimeCheckedType,
    pub ownership: RuntimeValueOwnership,
    pub transfer: ViewValueTransferMode,
}
```

Canonical input ordering remains by register. Product validator recomputes ownership from the function/type/layout table; serialized ownership is consistency evidence, not authority.

## Role matrix

### Pure

- all inputs `Copy + Unrestricted`;
- result used by a retained binding/projection must be `Unrestricted`;
- no suspension or render effect;
- explicit AWBC `CopyValue` is emitted when a source must remain reusable.

### DirectAwait

- captures and environment inputs are `Copy + Unrestricted`;
- no borrow crosses suspension;
- the Need result is moved into the ready/error/denied retained slot;
- that retained result type must be statically `Unrestricted`;
- pending/ready/error/denied state is saved through dormant snapshots.

### Handler

- exact handler input is `Move` and consumed once;
- retained View captures are `Copy + Unrestricted`;
- handler-local generic values follow ordinary affine runtime rules;
- a handler may not commit an affine value into View parameter/state/local/repeat/export/render-cache storage;
- handler frame save, when eligible, uses the whole-execution snapshot owner.

## Corrected View boundaries

| Boundary | Final rule |
|---|---|
| parameter | Unrestricted; copied from mount slot to invocation |
| default | result must be exact type and Unrestricted |
| retained local/state | Unrestricted |
| repeat source/item/key | complete source and retained item/key Unrestricted |
| nested call argument | Unrestricted; caller result copied/moved into scratch then committed as reusable callee slot |
| environment render binding | Unrestricted and explicitly admitted |
| text/resource/property projection | result Unrestricted before projection |
| export carrying runtime value | Unrestricted |
| static fragment constant | closed unrestricted payload/resource evidence |
| handler input | moved exactly once |

Attempting to bind a `StreamHandle`, affine external partial, affine aggregate, or unknown may-be-affine generic to any retained View boundary is a semantic error before product generation.

## Save correction

`BundleViewRuntimeSnapshot` stores typed coordinate/state metadata only. Runtime values are referenced/projected through the existing whole-execution `RuntimeValueSnapshotV2` graph. No live `RuntimeBinding` derives or View-local value DTO remains.
