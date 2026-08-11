# Decision register

| ID | Question | Selected decision | Status |
| --- | --- | --- | --- |
| D-01 | Opaque checked representation | RuntimeCheckedType::Opaque { owner } plus exact RuntimeValue::Opaque | closed |
| D-02 | Composite recursion | complete recursive checked-type tree; opaque generic atomic cut | closed |
| D-03 | Variant ownership | complete owner; selected case descriptor only; no Never sentinel | closed |
| D-04 | Native acceptance | one inherent accepts_value; fail closed without wrapper/producer | closed |
| D-05 | AWBC | ABI1/codec11, type tag23, constant tag18, same relation | closed |
| D-06 | Producers | accepted opaque rows, CharacterDialogue exact/wide, standard errors opaque | closed |
| D-07 | Type reconciliation | Named runtime failure; producer-bearing Opaque shape; no schema | closed |
| D-08 | RuntimeResolvedVariant | checked_selection only; old helpers deleted | closed |
| D-09 | Persistence | Serde changes, value tag16, save3, no dual reader | closed |
| D-10 | A1 order | four compile-clean subgates; resume parent after A1.4 | closed |

```text
REQUIRED_DECISIONS=10
CLOSED_DECISIONS=10
OPEN_RESULT_CHANGING_DECISIONS=0
OPEN_QUESTIONS=0
```
