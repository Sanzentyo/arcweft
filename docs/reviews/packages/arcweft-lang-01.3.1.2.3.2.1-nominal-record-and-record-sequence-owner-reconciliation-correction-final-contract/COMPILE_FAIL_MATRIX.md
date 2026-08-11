# Compile-fail and deletion matrix

| ID | Old/raw surface | Required compiler result | Landing gate |
|---|---|---|---|
| CF-001 | external `RuntimeFieldValue { name, value }` | private/missing fields | A3 |
| CF-002 | external `RecordSeqField { name, values }` | private/missing fields | A3 |
| CF-003 | external/direct `RuntimeNominalRecordFieldExpr` literal | private fields | A2 |
| CF-004 | direct `RuntimeNominalRecordLayoutField` construction | private fields/no constructor | A1 |
| CF-005 | `RuntimeNominalRecordValue::new(type_id, layout, fields)` | no associated function | A4 |
| CF-006 | `RuntimeNominalRecordValue::validate_shape(...)` | no method | A4 |
| CF-007 | `RecordSeq::new(len, fields)` | no associated function | A3 |
| CF-008 | old `RuntimeSeq::record_columns(len, Vec<RecordSeqField>)` | argument type mismatch; no overload | A3 |
| CF-009 | old runtime-plan `push_nominal` for record expr | no method | A1 |
| CF-010 | old runtime-plan `pattern_nominal` accessor | no method | A1 |
| CF-011 | `RuntimeCheckedType::Nominal` without `layout` | missing field | A1 |

The trybuild fixtures must assert the stable semantic reason, not overfit full
rustc wording. The implementation may refresh `.stderr` only after confirming
that the error still proves the exact deleted/private boundary.
