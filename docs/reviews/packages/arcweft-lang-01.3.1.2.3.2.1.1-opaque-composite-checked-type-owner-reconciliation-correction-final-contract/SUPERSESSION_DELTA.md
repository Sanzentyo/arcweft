# Narrow supersession delta against Lang-01.3.1.2.3.2.1

Parent retained-byte ZIP SHA-256:
`4b15a5eaea31663a9323f41f75345b2acb6faa0ea3a61784eeeabd482a13966a`.

## Retained without redesign

- ownership lattice and G1.2-A identity/slot/path foundation;
- `RuntimeRecordFieldId`, `RuntimeOwnedSlotId`, `RuntimeValuePath`;
- `RuntimeNominalRecordLayout` and defining-layout-order record admission;
- `RuntimeSeqError` for record-sequence admission;
- project nominal checked types with exact `TypeLayoutHash`;
- `RuntimeTypeSchema::try_layout_hash` as sole schema-layout hash authority;
- parent nominal `RuntimeCheckedType::Nominal { nominal, semantic_identity,
  layout }` and inherent `accepts_value` move;
- activation, View, Stream, and ABI-1 decisions outside this gap.

## Replaced statements/requirements

| Parent statement | Corrected statement |
|---|---|
| Non-nominal checked-type variants remain unchanged | Add one truthful `Opaque { owner }` variant and matching value carrier |
| `RuntimeTypeShape::Named` gains layout | Only schema-owning project nominal types gain layout; runtime-facing `Named` fails and accepted opaque rows carry producer evidence |
| bare opaque/unrepresentable shape errors | producer-bearing opaque shape projects successfully; producerless shapes fail typed |
| selected `RuntimeVariantOwner::checked_type` plus boolean case check | one `RuntimeResolvedVariant::checked_selection` owns complete owner/case validation |
| A1 is one atomic gate | A1 is four named compile-clean subgates; parent continuation resumes after A1.4 |
| AWBC ABI 1 / codec 8 | exact required commit is ABI 1 / codec 10; final is ABI 1 / codec 11 |
| no wire allocation | opaque values/types are serialized, so allocate canonical value tag 16, AWBC tags 23/18, codec 11, save schema 3 |

## Explicitly not superseded

This correction does not alter parent field-ID derivation, record initializer
evaluation/scatter order, layout validation precedence, anonymous/column record
policy, record sequence admission, source span ownership, or downstream A2+
order other than delaying it until corrected A1.4 passes.
