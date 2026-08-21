# Decision traceability

## Required decisions

| Request item | Closed decision | Owning API / artifact | Principal tests |
|---|---|---|---|
| RD-1 | Exclude accepted but unreachable suspending ordinary functions from runtime publication; retain full semantic/tooling state. | `HirRuntimeSemanticReachability`, `RuntimeEmissionMode` | T001-T006, T050-T052 |
| RD-2 | Replace the current inventory with one generation-bound root/edge/closure owner; diagnose reached unsupported functions before type/layout projection. | HIR reachability owner, compiler preflight | T007-T020, T053-T060 |
| RD-3 | Conditional branch is not taken; no opaque project nominal layout is invented. | Explicit absence; schema/layout APIs unchanged | T021-T027, T061-T064 |
| RD-4 | Transient and persisted project nominals share schema-derived `TypeLayoutHash`; opaque-containing project nominals reject when reached. | `project_nominal_schema`, `RuntimeTypeSchema::try_layout_hash` | T028-T041 |
| RD-5 | Filtered facts/plans/products; full tooling; no emitted frame/save state for unreachable code. | Compiler, RuntimePlan, AWBC, native, session save | T042-T049, T065-T073 |
| RD-6 | Fixture 013 unchanged passes; direct reached suspension fails before nominal layout. Versions stay 1. | fixture gate, error precedence, version assertions | T001, T007-T009, T074-T078 |

## Required-test clauses

| Request clause | Rows |
|---|---|
| fixture 013 unchanged | T001-T003 |
| same function reached from Flow / selected Entry | T007-T010 |
| unreachable suspending function with primitive result | T004-T005 |
| unreachable / reachable project nominal with one opaque field | T006, T021-T024 |
| nested Option/Result/Vec/tuple/enum/project nominal composites | T028-T037 |
| persistence admission/rejection | T038-T041, T065-T069 |
| native/AWBC checked-type parity | T042-T047 |
| stale generation | T011-T013 |
| missing reachability edge | T014-T016 |
| forged producer / wrong semantic identity | T038, T043, T066-T067 |
| wrong nominal layout | T025, T039, T044, T068 |
| deterministic ordering | T017-T020, T048-T049 |

## Prohibited-answer closure

| Prohibition | Contract mechanism | Negative tests |
|---|---|---|
| fixture-name/source-text gate | root/edge graph contains only typed IDs; source text is never accepted by reachability APIs | T075 |
| `TypeKind::Named` runtime fallback | named fallbacks remain errors; no new projection arm | T062 |
| dummy `Bytes`, empty record, `Dynamic`, producer schema, name-derived layout | same schema-derived project nominal contract; opaque leaf typed rejection | T023-T024, T061, T063-T064 |
| parallel inventories/fallback readers | old inventory deleted; all consumers take one reachability owner/identity | T053-T056 |
| silently drop selected/reached function | matching-edge proof plus preflight admission matrix | T014-T016, T057-T060 |
| compatibility/V2/version bump | direct unreleased schema replacement only where needed; this design adds no wire change | T074, T076-T078 |
| production overlay in archive | package contains documentation/TSV/JSON/hash files only | archive validation |
