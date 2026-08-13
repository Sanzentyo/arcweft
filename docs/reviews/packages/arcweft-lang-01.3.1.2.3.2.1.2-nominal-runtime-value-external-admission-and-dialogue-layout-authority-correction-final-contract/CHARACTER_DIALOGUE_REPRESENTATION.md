# CharacterDialogue final runtime representation

## 1. Top-level opaque owner

The final value is an exact `RuntimeValue::Opaque` with producer
`std.character_dialogue`. The producer is fixed by
`CharacterDialogueRuntimeSchema::opaque_type_producer()`. The semantic identity
is exact for the decoded/encoded character and is derived by the accepted
`CharacterDialogueType::Exact` projection.

`CharacterDialogueType::Any` remains a producer-wide checked type for static
admission only. A producer-wide concrete value is impossible under the retained
opaque contract.

The payload is a producer-owned tuple. Core never publishes a schema/layout for
it and never descends into it as part of generic opaque validation.

## 2. Exact 18-element payload

Tuple indices and `RuntimeValuePath::TupleElement` coordinates are zero-based.
The order is fixed:

| Index | Logical field | Physical value | Closed validation |
|---:|---|---|---|
| 0 | character | `EntityRef` | valid `CharacterId`, present in accepted character catalog |
| 1 | character manifest digest | dense `u8[32]` sequence | exact accepted manifest digest |
| 2 | defaults digest | dense `u8[32]` sequence | exact contract digest |
| 3 | custom schema digest | dense `u8[32]` sequence | exact active custom catalog digest |
| 4 | View contracts digest | dense `u8[32]` sequence | exact active View contract digest |
| 5 | voice | `Option` variant | `None`, `Auto`, or exact `Id(EntityRef)` under the retained voice variant owner |
| 6 | look | `Option<String>` | accepted character look when present |
| 7 | stage | `Option<RuntimeValue>` | active stage `RuntimeCheckedType` and producer catalog tree validation |
| 8 | portrait | `Option<RuntimeValue>` | active portrait type and producer catalog tree validation |
| 9 | focus | `Option<RuntimeValue>` | active focus type and producer catalog tree validation |
| 10 | cleanup | `Option<RuntimeValue>` | active cleanup type and producer catalog tree validation |
| 11 | View | `EntityRef` | accepted `ViewId` |
| 12 | source locale | `Option<String>` | valid `DialogueLocaleId` |
| 13 | hooks | `RuntimeSeq::values` | every element satisfies active hook type/tree; aggregate limits |
| 14 | style | `RuntimeValue` | active style type/tree; structured limits |
| 15 | rich text | `RuntimeValue` | active rich-text type/tree; structured limits |
| 16 | inline failure | direct closed `RuntimeValue::Variant` | exact owner/case/payload contract below |
| 17 | custom | `RuntimeSeq::values` of two-element tuples | strict field-ID order, exact descriptor per ID, full tree validation |

The tuple must contain exactly 18 elements. There is no optional tail, unknown
field rule, fallback decoder, or layout-hash-only acceptance.

## 3. Decode/encode correlation

### Decode

1. require opaque producer `std.character_dialogue`;
2. require tuple shape and exactly 18 elements;
3. parse only tuple element 0 as a canonical accepted `CharacterId`;
4. derive the exact expected semantic identity and compare it with the opaque
   value;
5. preflight/obtain all active role/custom descriptor admissions;
6. decode every fixed field in index order;
7. at each nested nominal node perform descriptor lookup, identity, layout,
   count, ID, and field-predicate validation;
8. validate catalogs, contract digests, accepted View/look/custom rules,
   limits, and canonical custom order;
9. canonical re-encode and require byte equality when decoding persisted
   external bytes; and
10. publish `CharacterDialogueValue`.

### Encode

1. validate local `CharacterDialogue` invariants;
2. validate every role/custom value against the active checked type and
   producer capability;
3. validate domain catalogs/digests/limits;
4. build the exact 18-element tuple;
5. derive the exact opaque owner from the character;
6. wrap through `RuntimeOpaqueTypeOwner::try_wrap`;
7. canonical encode within the configured budget; and
8. publish.

A caller does not supply root nominal identity, root layout, or opaque owner.

## 4. Custom entry representation

Element 17 is a values sequence. Each element is exactly:

```text
TupleElement(0): String(CharacterDialogueCustomFieldId)
TupleElement(1): RuntimeValue
```

The sequence is strictly increasing by the parsed field ID. Duplicate,
out-of-order, unknown, or view-incompatible IDs fail before publication. The
active `CharacterDialogueRuntimeCustomFieldCatalog` supplies:

- the field ID;
- one closed `RuntimeCheckedType`;
- `clearable` policy; and
- accepted Views.

The value does not repeat nominal ID or layout. If the checked type contains a
nominal node, its semantic/layout evidence comes from that checked type and the
active producer catalog. The old four fields
`field_id/declared_nominal_type/declared_layout/value` and
`RuntimeTypeSchema::Named("Dynamic")` are deleted.

## 5. Inline failure representation

Element 16 is the existing direct variant owned by
`arcweft.dialogue.InlineFailurePolicy`:

| Ordinal | Name | Payload |
|---:|---|---|
| 0 | `FailLine` | none |
| 1 | `Discard` | none |
| 2 | `Fallback` | exact `InlineFallback` variant |

`InlineFallback` retains exact cases `Text`, `ExprSource`, `CallSource`, and
`ValuePlain`; `FallbackStylePolicy` retains `Plain`, `InheritSurrounding`, and
`Apply`. Style values in `Apply` are validated using the active style checked
type and producer capability. Owner, ordinal, name, payload presence, tuple
arity, and child value type are all exact.

The former one-field `std.inline_failure_policy` nominal record is deleted.

## 6. Canonical identity and bytes

This is a direct unreleased representation replacement. The old root/custom/
inline nominal encodings have no reader after the cut. The exact opaque value's
existing canonical encoding is used; no new codec/version is allocated.

Ordinary project nominal records elsewhere retain their existing nominal
canonical bytes. Anonymous and nominal records remain distinct exactly as
accepted by A1–A3. CharacterDialogue no longer claims ordinary project-nominal
bytes for producer-owned payload structure.

## 7. Deleted root layout surface

The implementation deletes:

- `CharacterDialogue.layout` and `layout()`;
- the layout parameter of `CharacterDialogue::try_new`;
- `CharacterDialogueRuntimeSchema.expected_layout`;
- root nominal `type_id()`/`expected_layout()` APIs;
- `character_dialogue_type_id()`;
- `CharacterDialogueValue.record`;
- `encode_record`/`decode_record` nominal signatures;
- root calls to `RuntimeNominalRecordValue::new` and `validate_shape`; and
- digest/equality logic that requires descriptorless nominal encoding.

Digest/canonical encoding becomes schema-owned. `CharacterDialogue` equality
and hashing are structural over its already admitted domain fields; they do not
reconstruct a nominal carrier.
