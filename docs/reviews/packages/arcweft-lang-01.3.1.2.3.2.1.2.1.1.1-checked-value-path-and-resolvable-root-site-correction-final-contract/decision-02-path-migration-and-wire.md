# Decision 02 — path migration and exact Serde grammar

## No dual path

The following retry-only declarations are removed before implementation begins:

- `arcweft_core::pattern::RuntimeValuePath`
- `arcweft_core::pattern::RuntimeValuePathSegment`
- any conversion between a pattern path and the ownership path
- any diagnostic fallback that converts path segments to strings

Every consumer imports the existing `value::ownership` owner. Affine ownership, function capture, iterator remainder/witness, tuple/record column, and existing snapshot/save consumers retain their current variants unchanged. Nominal tree validation, CharacterDialogue validation/patch, restore, replay, View activation, save diagnostics, AWBC admission, and VM errors use the same owner. Dialogue-local ordinal-only `RuntimeFieldPath` is deleted after its callers use `NominalRecordField` or `OpaquePayload`; no compatibility alias remains.

## Human-readable Serde data model

A `RuntimeValuePath` serializes as a sequence of internally tagged maps. `kind` is lower snake case. Unknown fields are rejected. The exact maps are:

| kind | fields |
|---|---|
| `tuple_element` | `index: u32` |
| `sequence_element` | `index: String` containing canonical unsigned decimal `u64` |
| `tuple_column` | `index: u32` |
| `record_field` | `field: RuntimeRecordFieldId` |
| `record_column` | `field: RuntimeRecordFieldId` |
| `nominal_record_field` | `field: RuntimeRecordFieldId` |
| `function_capture` | `capture: RuntimeCaptureSlotId` |
| `variant_payload` | no fields other than `kind` |
| `iterator_remainder` | `index: String` containing canonical unsigned decimal `u64` |
| `iterator_witness_state` | no fields other than `kind` |
| `opaque_payload` | no fields other than `kind` |

Canonical decimal means: nonempty ASCII digits, no sign, no whitespace, and no leading zero unless the value is exactly `0`. Overflow is a deserialization error. Sequence length above 64 is a `RuntimeValuePathError::TooDeep` mapped through `serde::de::Error::custom`.

## Non-human Serde data model

The sequence and map/tag structure is identical. The only human/non-human difference is that `sequence_element.index` and `iterator_remainder.index` are native `u64`, not decimal strings. `u32`, newtype, and field payloads retain their typed Serde representations. This contract defines the Serde data model; a format such as bincode may choose its own byte framing and is not a second Arcweft canonical byte codec.

## Canonical ordering tags

`RUNTIME_VALUE_PATH_SEGMENTS.csv` is normative. Tags 0–9 are byte-for-byte/order-compatible with current production; `opaque_payload` is tag 10. Tags drive `Ord` and the plan/AWBC equality coordinate encoder. They are not inferred from Serde variant order.

## Migration cut

1. Extend the existing enum and its four Serde helper enums with `OpaquePayload`.
2. Add the same match arm to `canonical_tag`, `Ord`, serialize, deserialize, ownership traversal, snapshot traversal, and path resolution.
3. Move checked-validation call sites to imports from `value::ownership` and add `RuntimeCheckedTypePath` separately.
4. Delete the retry duplicate names and any conversion helper in the same compile-clean change.
5. Migrate dialogue, nominal tree, restore/replay/View/save/diagnostic consumers directly. No old tag reader, alias, lossy integer conversion, or string path fallback exists at any intermediate phase.
