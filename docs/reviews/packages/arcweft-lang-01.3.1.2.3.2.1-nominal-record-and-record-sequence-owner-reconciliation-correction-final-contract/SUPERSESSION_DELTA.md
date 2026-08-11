# Narrow supersession delta

This file identifies every Lang-01.3.1.2.3.2 statement changed by this
correction. Unlisted parent decisions remain normative.

| Parent statement | Corrected decision |
|---|---|
| `RuntimeNominalRecordSchema` is an existing owner | False. Do not declare/alias it. Add `RuntimeNominalRecordLayout` with the exact core declaration in this package. |
| nominal ctor takes `Arc<RuntimeNominalRecordSchema>` | It takes `&RuntimeNominalRecordLayout`; the value does not retain Arc. Runtime expression/pattern retain the Arc. |
| `RecordSeqError` is an existing owner | False. Do not introduce it. Use and extend existing `RuntimeSeqError`. |
| `RecordSeq::try_from_accepted_fields -> RecordSeqError` | Return `RuntimeSeqError`. |
| nominal initializers are projected to layout order before runtime construction | Refined: child expressions stay authored order; evaluated results are scattered by accepted field ID, then passed layout order. This preserves effects and the parent's final value order. |
| nominal value constructor handles accepted layout | Expanded with exact count/field predicate checks and exact separation from name admission. |
| unchecked nominal constructor is replaced | Exact cut now deletes public `new` and `validate_shape` after all consumers migrate; no wrapper. |
| existing checked nominal identity is sufficient | Repository defect: current predicate checks nominal ID only. Add required `TypeLayoutHash` to `RuntimeCheckedType::Nominal`. |
| nominal pattern owner can be a checked type | Replace record-pattern owner with shared `RuntimeNominalRecordLayout`; delete positional zip. |
| record-column error behavior unspecified through absent error | Existing per-field length-before-duplicate behavior is retained and count/ID precedence is fixed. |
| live carrier final traits omit Clone/Serde | Interim derives remain while enclosing target types require them; final removal stays at accepted parent stages. |

## Explicitly preserved

- ownership classifier;
- all accepted runtime identity wrappers/cursor;
- one-based `RuntimeRecordFieldId` representation and codecs;
- all eight `RuntimeOwnedSlotId` variants/order/codecs;
- all ten `RuntimeValuePath` segments/order/Serde/fixed-LE;
- transaction, snapshot, activation, View, and Stream decisions outside this
  owner correction;
- AWBC ABI 1/codec 8; and
- parent G1.2-B onward sequencing, which remains blocked until this corrected
  G1.2-A cut is implemented.

Parent archive identity: `e95de2a9958000034a48f8c5228c8a4ff17f62226195cce4c0ef93e398c816e4`.
