# Errors and deterministic precedence

All scans are left-to-right in the order named below. No consumer may reorder
checks for diagnostic convenience.

## 1. Layout descriptor construction

Owner: `RuntimeNominalRecordLayoutError`.

| Rank | Condition | Variant |
|---:|---|---|
| 1 | field count exceeds `u32::MAX` | `TooManyFields` |
| 2 | first repeated defining-layout name | `DuplicateFieldName` |
| 3 | first ordinal that cannot map to one-based ID | `InvalidFieldIdentity` |

The count preflight makes rank 3 unreachable for ordinary vectors, but the
mapping remains typed and is tested through synthetic ordinal arithmetic.

## 2. Nominal initializer admission

Owner: `RuntimeNominalRecordInitializerError`.

| Rank | Condition | Variant |
|---:|---|---|
| 1 | authored initializer count exceeds identity space | `TooManyFields` |
| 2a | at current authored entry, name was already seen | `DuplicateName` |
| 2b | otherwise, name is absent from descriptor | `UnknownField` |
| 2c | otherwise, accepted descriptor ordinal cannot map to ID | `InvalidFieldIdentity` |
| 3 | after scan, first missing descriptor field in layout order | `MissingField` |

At one authored entry, duplicate wins over unknown. Across entries, the earlier
authored entry wins. Missing is checked only after every authored entry is
valid.

For validation of a deserialized expression, `FieldIdentityMismatch` is checked
after authoritative name lookup and before the final missing scan. Stored IDs
are rejected, never repaired.

## 3. Nominal runtime-value construction

Owner: `RuntimeNominalRecordError`.

`try_from_accepted_layout`:

| Rank | Condition | Variant |
|---:|---|---|
| 1 | value count differs from descriptor field count | `FieldCount` |
| 2 | first layout ordinal cannot map to ID | `InvalidFieldIdentity` |
| 3 | first value failing its field predicate in layout order | `FieldType` |

The constructor cannot return `Type` or `Layout`: it copies both from the sole
accepted descriptor.

`validate_against_layout`:

| Rank | Condition | Variant |
|---:|---|---|
| 1 | runtime nominal ID differs | `Type` |
| 2 | layout hash differs | `Layout` |
| 3 | field count differs | `FieldCount` |
| 4 | first layout ordinal cannot map to ID | `InvalidFieldIdentity` |
| 5 | first field predicate mismatch in layout order | `FieldType` |

This precedence is used by root/replay/snapshot/bundle/save validation before
ownership traversal or activation.

## 4. Anonymous record admission

Owner: accepted parent `RuntimeRecordAdmissionError`.

| Rank | Condition | Variant |
|---:|---|---|
| 1 | count exceeds one-based field identity space | `TooManyFields` |
| 2 | first repeated name in authored order | `DuplicateName` |
| 3 | first defensive ID conversion failure while publishing | `InvalidFieldIdentity` |

No `RuntimeValue::Record` is published on failure.

## 5. Record-column admission

Owner: existing `RuntimeSeqError`; there is no record-only error enum.

| Rank | Condition | Variant |
|---:|---|---|
| 1 | field count exceeds one-based identity space | `TooManyRecordFields` |
| 2a | current stored ordinal cannot map to ID | `InvalidRecordFieldIdentity` |
| 2b | current column length differs from declared rows | `ColumnLength` |
| 2c | current diagnostic name duplicates an earlier stored field | `DuplicateRecordField` |

The earliest stored ordinal wins. At the same ordinal, identity precedes length,
and length precedes duplicate. This preserves the target implementation's
observable length-before-duplicate behavior.

Examples:

- rows=2, fields `[a(len=2), a(len=1)]` -> `ColumnLength` at ordinal 1;
- rows=2, fields `[a(len=2), a(len=2)]` -> `DuplicateRecordField("a")`;
- count=`u32::MAX + 1` -> `TooManyRecordFields` before any column access;
- synthetic conversion failure at ordinal `k` ->
  `InvalidRecordFieldIdentity` before length/duplicate for `k`.

Tuple-column admission keeps the existing `ColumnLength` behavior and does not
use record-only variants.

## 6. Fact publication

Fact publication first performs existing unique owner collection, then HIR
family/lease validation, then nominal/layout coherence, then declaration-field
coherence, then interning conflict detection. It returns typed
`RuntimeSemanticFactsError`/`RuntimeNominalRecordFactError`; it never returns a
stringly fallback for the new record-specific failures.

## 7. Evaluation

Checked nominal initializer expressions preserve authored evaluation order.
Ordinary expression failures therefore follow existing authored evaluation
precedence. After all fields evaluate, layout-order `FieldType` validation runs.
No partially built nominal value is published.
