# Requirement traceability

| ID | Requirement | Closed decision | Owners | Evidence | Test rows |
|---|---|---|---|---|---|
| D01 | Single-value AWBC selector representation supporting heterogeneous arms and deterministic v1 identity | Synthetic nominal Variant; one case per source arm; tuple payload | runtime-plan/core | E015,E016,E018,E019,E024,E025,E026 | T001-T012,T041,T042,T096 |
| D02 | Validate or replace preferred synthetic nominal variant and reject alternatives | Preferred candidate selected without alternatives | runtime-plan/core | E015,E018,E026 | T001-T010,T043-T050 |
| D03 | Exact construction/signature/assembly/verified decode/extraction/install APIs | Builder, core verifier, driver decoder, LocalInstallTransaction | runtime-plan/core/runtime-driver | E016,E018,E019,E024,E025 | T001-T020,T051-T058 |
| D04 | Keep arcweft-view core-independent and define core/bundle/driver join | Lightweight View coordinates; sole bundle cross-section; driver active use | view/bundle/runtime-driver | E006,E042,E043,E044,E045 | T081,T082,T111-T115 |
| D05 | Resolve ViewMatchSelection visibility | Delete public type; private driver scratch only | runtime-driver | E010,E012,E044 | T083,T116 |
| D06 | Exact dense/order/type/ownership/malformed/no-match/rollback validation | Read-only decode then all-or-nothing local install | bundle/runtime-driver | E024,E025,E026 | T013-T040,T051-T058,T101-T105 |
| D07 | Sole typed Need carrier and separation from String/TaskHandle | NeedHandle{payload}; RuntimeValue::NeedHandle; fixed-byte NeedId | core | E015,E021,E024,E026,E028,E030 | T059-T071,T084-T087 |
| D08 | Exact producer result/task-plan relationship and verified extraction/generation checks | Flagged synthetic producer, MakeNeedHandle, one plan, private VerifiedNeedHandle | core/bundle/runtime-driver | E016,E017,E018,E020,E030,E045 | T059-T080,T106-T110 |
| D09 | Construction/verifier/VM/value/digest/wire/bundle/snapshot/replay/replacement behavior | Dedicated end-to-end typed carrier lifecycle | core/runtime-plan/bundle/runtime-driver | E021,E027,E028,E029,E031,E043,E044 | T059-T080,T088-T095,T121-T132 |
| D10 | Delete NeedHandle-as-String and await_target conversion while explicit TaskHandle/String behavior remains | Atomic strict v1 deletion; TaskHandle unchanged in this scope | core/runtime-plan/bundle/runtime-driver | E020,E024,E026,E031,E046,E047 | T084-T090,T116-T120 |
| D11 | Reconcile TypeId/runtime type/ownership fields and retain single projection | TypeKind authority; RuntimeNormalizedType and AwbcInventory only | sema/runtime-plan/core | E027,E031,E032,E035,E037 | T033-T040,T097-T100 |
| D12 | Replace nonexistent arm identity with exact current HIR fields/order/lookup | CheckedMatchArmId(owner ExprId, ordinal) plus exact scope/pattern/guard/value/locals | hir/sema | E034,E036,E037 | T021-T032,T097-T100 |
| D13 | Define all checked/runtime ownership dispositions | CheckedOwnershipDisposition + closed rejection reasons; bundle only SnapshotClone | sema/bundle | E032,E035,E037 | T033-T040,T053,T054,T102 |
| D14 | Make generic Match one authority and checked View catalog reference it | CheckedExpressionResolution::Match; CheckedMatchRef only | sema/compiler | E034,E036,E038,E040 | T021-T032,T097-T100,T112 |
| D15 | Exact ResourceTypeRegistryDigest input/constructor/lifetime/equality/error | Sema borrows verified registry; compiler passes context registry; all digests equal | resource-model/sema/compiler/bundle | E038,E039,E040,E041 | T091-T095,T103,T104 |
| D16 | Compile-clean implementation sequence without dual/empty/scaffolding states | Five final-owner cuts and atomic switch | workspace | E001-E006,E014 | T111-T140 |
| D17 | Close current verifier/VM guard semantic split | Explicit pattern/bind/ordinary guard/Branch selector; Match guard forbidden | runtime-plan/core | E022,E023,E033,E034 | T009-T012,T041-T050,T096,T125-T128 |

All request decisions 1–16 and the current-source guard defect are mapped. No row is closed by a generic placeholder.
