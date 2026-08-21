# Requirements traceability

Every primary/correction decision, inventory item, constraint, required test
family, and required artifact maps to a selected contract and verification row.
| Row | Requirement | Source | Requirement text | Selected decision | Artifacts | Tests |
|---|---|---|---|---|---|---|
| T001 | P01 | primary | sole checked owner/all retained facts | CheckedViewCatalog/CheckedViewNeedMatch under exact generation | FINAL_CONTRACT C2; RUST_SCHEMAS | TM-API-*; TM-MATCH-* |
| T002 | P02 | primary | subscription identity/producer mapping | HIR session key + local/semantic/contract IDs + AWBC/NeedId | FINAL_CONTRACT C3; OWNERS_AND_APIS | TM-ID-*; TM-TAMPER-* |
| T003 | P03 | primary | four-state deterministic publications/fanout/remount | generation journal, cursor table, one invalidation per observer | PUBLICATION_SEMANTICS | TM-PUB-*; TM-MOUNT-* |
| T004 | P04 | primary | ordinary patterns/bindings through generic Match | AWBC selector and RuntimeValue outputs | MATCH_EXECUTION; RUST_SCHEMAS | TM-MATCH-* |
| T005 | P05 | primary | nested Result/Option; no error/denied | only Ready nests carriers; admission outside Need | FINAL_CONTRACT C6 | TM-CARRIER-* |
| T006 | P06 | primary | start/dedup/cancel/failure | ObserveStartsNotStarted, JoinSameKey, ProducerOwned | PRODUCER_START_AND_CANCELLATION | TM-START-* |
| T007 | P07 | primary | v1 save/replay/replacement identity | canonical producer/publication/observer/arm/queue tables | WIRE_CODEC_SAVE_REPLAY_REPLACEMENT | TM-SAVE-*; TM-REPL-* |
| T008 | P08 | primary | strict Await deletion | 40 deletion rows; unknown await tag rejects | DELETION_MATRIX | TM-ABS-*; TM-TAMPER-OLD-* |
| T009 | P09 | primary | static contamination | LiveNeedSubscription at first scrutinee | STATIC_CERTIFICATION | TM-STATIC-* |
| T010 | P10 | primary | failure precedence/atomicity | ranked stages and seven transaction scopes | FAILURE_PRECEDENCE_AND_ATOMICITY | TM-ATOMIC-*; TM-TAMPER-* |
| T011 | P11 | primary | compile-clean interleave | parent substrate, facts/runtime, atomic switch/delete | IMPLEMENTATION_SEQUENCE | TM-GATE-* |
| T012 | P12 | primary | bounded accounting | 22 inclusive limits/exact/one-over | WORK_ACCOUNTING | TM-LIMIT-* |
| T013 | C01 | correction | complete independent redelivery | full package, not delta | README; all artifacts | validator required set |
| T014 | C02 | correction | current production precedence | full SHA and docs-only diff | SOURCE_EVIDENCE; repository-state | repository evidence |
| T015 | C03 | correction | retain failed return evidence | pass=false issues and supplied digest retained | inputs/FAILED_RETURN_VALIDATION.md | input presence |
| T016 | C04 | correction | README readiness/reading order | explicit design-ready and boundary | README.md | readiness check |
| T017 | C05 | correction | OPEN_QUESTIONS exact none | bytes b'none' without newline | OPEN_QUESTIONS.md | exact-byte validator |
| T018 | C06 | correction | Rust-shaped owners/APIs | schemas + inherent owner methods | OWNERS_AND_APIS; RUST_SCHEMAS | TM-API-* |
| T019 | C07 | correction | current SHA/line evidence | 90 rows, 77 Rust, exact ref/range | SOURCE_EVIDENCE.csv | evidence validator |
| T020 | C08 | correction | requirements traceability | all primary/correction/inventory/constraints/tests | REQUIREMENT_TRACEABILITY.csv | trace validator |
| T021 | C09 | correction | consumer/deletion matrices | 30 consumers, 40 deletions | CONSUMER_MATRIX; DELETION_MATRIX | matrix validator |
| T022 | C10 | correction | wire/save/replay/replacement/allocation | strict v1 fixed | WIRE_*; VERSION_1_* | TM-V1-*; TM-SAVE-* |
| T023 | C11 | correction | diagnostic/atomicity/work | ranked precedence and exact limits | FAILURE_*; WORK_* | TM-ATOMIC/LIMIT |
| T024 | C12 | correction | compile-clean sequence | five cuts plus freeze | IMPLEMENTATION_SEQUENCE | TM-GATE-* |
| T025 | C13 | correction | full positive/negative/tamper/Tier-2 tests | exact owner/input/expected/atomicity/gate rows | TEST_MATRIX | test validator |
| T026 | C14 | correction | verification/internal SHA manifest | read-only validator and all-other-file SHA rows | VERIFICATION; SHA256SUMS | manifest validator |
| T027 | C15 | correction | missing-artifact validator failure | hard-coded required files | tools/validate_package.py | validator mutation self-test |
| T028 | C16 | correction | unresolved-decision validator failure | decision IDs/model/open bytes checked | tools/validate_package.py | validator mutation self-test |
| T029 | C17 | correction | line-evidence validator failure | SHA/range/row minimums checked | tools/validate_package.py | validator mutation self-test |
| T030 | C18 | correction | matrix-size validator failure | trace/test/consumer/deletion thresholds checked | tools/validate_package.py | validator mutation self-test |
| T031 | A01 | required inventory | View/Need/Progress/Result/Option chapters | maintained doc evidence retained | SOURCE_EVIDENCE E001-E007 | TM-DOC-* |
| T032 | A02 | required inventory | syntax/HIR match/pattern/source/View context | exact paths/ranges and catalog schema | SOURCE_EVIDENCE; RUST_SCHEMAS | TM-MATCH/SEMA |
| T033 | A03 | required inventory | final sema Need/Progress/Result owners | TypeKind/final report evidence | SOURCE_EVIDENCE | TM-API/OWN |
| T034 | A04 | required inventory | compiler RuntimePlan/AWBC dynamic programs | ordinary lowerer and normalized types | SOURCE_EVIDENCE; OWNERS_AND_APIS | TM-COMP/API |
| T035 | A05 | required inventory | arcweft-view instruction/dependency/mount/local/static/old Await | all owners inventoried | SOURCE_EVIDENCE; DELETION_MATRIX | TM-MOUNT/ABS |
| T036 | A06 | required inventory | bundle model/codec/validation/merge/digest/source maps | strict v1/canonical/source exclusion | WIRE_*; SOURCE_EVIDENCE | TM-TAMPER-* |
| T037 | A07 | required inventory | runtime publication/evaluation/save/replacement/backends | one journal/catalog/frame | CONSUMER_MATRIX; PUBLICATION_* | TM-PUB/SAVE/REPL/PARITY |
| T038 | A08 | required inventory | current tests and superseded parent rows | replacement/absence/parent table | TEST_MATRIX; PARENT_ROW_SUPERSESSION | TM-ABS/GATE |
| T039 | A09 | required inventory | exact test owner/input/expected | CSV carries exact columns | TEST_MATRIX.csv | validator schema |
| T040 | A10 | required inventory | commands as design evidence only | verification denies production pass claim | VALIDATION_GATES; VERIFICATION | TM-GATE-* |
| T041 | N01 | constraint | design-only/no production overlay | package contains docs/data/validator only | NON_GOALS; VERIFICATION | structure validator |
| T042 | N02 | constraint | preserve parent catalog/Match/RuntimeValue/resources | only stale Await rows superseded | PARENT_ROW_SUPERSESSION | TM-PARENT/API |
| T043 | N03 | constraint | no direct Await/AwaitView/error/denied/View VM | strict deletion/forbidden list | DELETION_MATRIX; NON_GOALS | TM-ABS-* |
| T044 | N04 | constraint | no timeout/Stream/Watch/unrelated features | explicit non-goals | NON_GOALS | scope scan |
| T045 | N05 | constraint | lower crates Sans I/O | intent then host dispatch; dependency chain | OWNERS_AND_APIS; PRODUCER_* | TM-START-* |
| T046 | N06 | constraint | determinism | BTree/canonical sort/cursor rules | PUBLICATION_SEMANTICS | TM-PUB-* |
| T047 | N07 | constraint | all touched markers 1 | allocation/model/validator | VERSION_1_ALLOCATION_TABLE; CONTRACT_MODEL | TM-V1-* |
| T048 | R01 | required tests | all four Need states | fixed projection/pattern cases | MATCH_EXECUTION | TM-STATE-* |
| T049 | R02 | required tests | multiple Pending | greater cursor across frames; batch coalescing | PUBLICATION_SEMANTICS | TM-PUB-* |
| T050 | R03 | required tests | first/duplicate/stale/out-of-order/coalesced | selection table | PUBLICATION_SEMANTICS | TM-PUB-* |
| T051 | R04 | required tests | same-step progress-to-ready | Ready first terminal; one invalidation | PUBLICATION_SEMANTICS | TM-PUB-P2R-* |
| T052 | R05 | required tests | nested Result/Option patterns | ordinary nested variants | MATCH_EXECUTION | TM-CARRIER-* |
| T053 | R06 | required tests | bindings/source order/coverage/no-match | AWBC once; no fallback | MATCH_EXECUTION | TM-MATCH-* |
| T054 | R07 | required tests | two mounts/two observers/remount | shared journal, independent state | PUBLICATION_SEMANTICS | TM-MOUNT-* |
| T055 | R08 | required tests | dedup/start/cancel/stale generation | intent/JoinSameKey/ProducerOwned | PRODUCER_* | TM-START/PUB-GEN |
| T056 | R09 | required tests | save/restore/replay/replacement | same v1 tables/live API | WIRE_* | TM-SAVE/REPL |
| T057 | R10 | required tests | affine payload/capture | semantic persistence rejection | FINAL_CONTRACT C8 | TM-OWN-* |
| T058 | R11 | required tests | static contamination | first Need scrutinee | STATIC_CERTIFICATION | TM-STATIC-* |
| T059 | R12 | required tests | malformed identity/cursor/codec/no partial | strict failure/rollback | FAILURE_* | TM-TAMPER-* |
| T060 | R13 | required tests | exact/one-over limits | all 22 limits | WORK_ACCOUNTING | TM-LIMIT-* |
| T061 | R14 | required tests | API absence every old surface | 40 deletion/structural rows | DELETION_MATRIX | TM-ABS-* |
| T062 | R15 | required tests | focused/workspace/Clippy/docs/structure/save/diff/backends/Agent/generated | commands and expected gates | VALIDATION_GATES | TM-GATE/PARITY |
| T063 | AR01 | required artifact | README final readiness | DESIGN READY, implementation not claimed | README.md | validator |
| T064 | AR02 | required artifact | concrete contract | 18 normative clauses | FINAL_CONTRACT.md | validator |
| T065 | AR03 | required artifact | exact schemas | checked/product/runtime/snapshot owners | RUST_SCHEMAS.md | validator |
| T066 | AR04 | required artifact | source evidence | current SHA exact ranges | SOURCE_EVIDENCE.csv | validator |
| T067 | AR05 | required artifact | wire/allocation | v1 tables and identities | WIRE_CODEC_SAVE_REPLAY_REPLACEMENT.md | validator |
| T068 | AR06 | required artifact | diagnostics | ranked precedence and scope | FAILURE_PRECEDENCE_AND_ATOMICITY.md | validator |
| T069 | AR07 | required artifact | work limits | inclusive 22-row table | WORK_ACCOUNTING.md | validator |
| T070 | AR08 | required artifact | implementation order | fail-closed substrate then atomic delete | IMPLEMENTATION_SEQUENCE.md | validator |
| T071 | AR09 | required artifact | positive/negative/tamper/Tier-2 matrix | hundreds of exact rows | TEST_MATRIX.csv | validator |
| T072 | AR10 | required artifact | verification | staging/extracted/manifest scope | VERIFICATION.md | validator |
