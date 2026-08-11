# Requirements traceability

## 1. Required decisions

| Request decision | Closed result | Normative files | Test coverage |
|---:|---|---|---|
| 1. `ExecutionInstanceId` owner/representation/visibility/traits/codec | core runtime-ID private `NonZeroU64`; domain-only monotonic mint; exact traits/codecs | `FINAL_CONTRACT.md` §3; `RUST_OWNERS_AND_APIS.md` §§2,14; `IDENTITY_AND_CODEC_CONTRACT.md` | `EXE-*`, `COD-*`, `API-*` |
| 2. fresh execution end to end | shared domain, exact fresh/reservation/active owners, collision/exhaustion, restart/replay/restore, empty/replace | `FINAL_CONTRACT.md` §3; `RUST_OWNERS_AND_APIS.md` §14; `SNAPSHOT_ACTIVATION_AND_RESTORE.md` | `EXE-*`, `SNP-*` |
| 3. `RuntimeRecordFieldId` | one-based accepted ordinal; anonymous authored order; nominal layout order; duplicate rejection; strict codec | `FINAL_CONTRACT.md` §4; `RUST_OWNERS_AND_APIS.md` §§2.4,3; `IDENTITY_AND_CODEC_CONTRACT.md` §3 | `REC-*`, `PTH-*`, `SNP-*` |
| 4. `RuntimeLocalSlotId` | execution-wide nonreused slot; declaration/capture plan projection; revisions/shadowing/suspension/restore | `FINAL_CONTRACT.md` §5; `RUST_OWNERS_AND_APIS.md` §§2,4,13; inventory | `LOC-*`, `SNP-*`, `API-*` |
| 5. complete `RuntimeOwnedSlotId` | exact eight variants/tags/order/render/codec; evidence only | `FINAL_CONTRACT.md` §6; `RUST_OWNERS_AND_APIS.md` §6; codec contract | `SLT-*`, `PAR-*`, `SNP-*` |
| 6. G1.2 symbols/APIs/traits | exact transaction ID, revision, evidence, prepared owners, errors, limits, transaction, support symbols | `RUST_OWNERS_AND_APIS.md` §§7–13 | `TXN-*`, `LIM-*`, `ALC-*`, `CMT-*`, `API-*` |
| 7. preflight/stage/commit and owner return | ten-rank prepare, eight-rank commit, exact transaction/aborted/fresh owners, permit boundary | `TRANSACTION_AND_COMMIT_CONTRACT.md`; final §§8–9 | `TXN-*`, `CMT-*`, `ALC-*` |
| 8. canonical `RuntimeValuePath` | ten segment kinds, shipped graph traversal, iterator remainder, manual comparison/first error | `VALUE_PATH_AND_PRECEDENCE.md`; Rust §5 | `PTH-*`, nested `TXN-*`, `REC-*` |
| 9. snapshot/restore/digest | persisted/rebuilt list, bit floats, four cursors/domain cursor, 12 stages, tamper-before-activation | `SNAPSHOT_ACTIVATION_AND_RESTORE.md`; codec §8 | `SNP-*`, `COD-*` |
| 10. corrected G1.1/G1.2/G1.3/G1.4 order | G1.1 preserved; G1.2-A through F; first constructible/serialized tables; G1.3/G1.4 blocked | `IMPLEMENTATION_ORDER.md` | `FUL-*` and cut column of all rows |

## 2. Required producer/consumer inventory

| Requested area | Inventory closure |
|---|---|
| core ownership/value/record/sequence/iterator/binding/env | inventory §§2,8; exact owners in Rust §§3–6 |
| HIR local/capture and layer-correct projection | inventory §3; final §5; local tests |
| structured scopes/pattern/capture/suspension/mailbox/child/cleanup | inventory §4; transaction §§13–15 |
| AWBC registers/frames/verifier/fibers/snapshot | inventory §5; complete owner enum and parity rows |
| driver creation/activation/save/restore/replay/replacement/cursor | inventory §6; snapshot/activation contract |
| bundle/save codecs/digest | inventory §7; codec/snapshot contract |

## 3. Required tests

| Request test family | Matrix rows |
|---|---|
| execution ID creation/collision/exhaustion/codecs/restart/replay/domain exclusivity | `EXE-*`, `COD-*`, selected `SNP-*` |
| record path anonymous/nominal/order/duplicates/limits/first error | `REC-*`, `PTH-*` |
| local slots/shadowing/exit/reuse/revision/suspension/restore/HIR mapping | `LOC-*`, selected `SNP-*` |
| every owned-slot variant order/codec/render | `SLT-*`, `PAR-*` |
| Copy/Move preparation/commit/race/budget/allocation/source preservation/no branch | `TXN-*`, `LIM-*`, `ALC-*`, `CMT-*` |
| moved/dropped diagnostics | Move/Drop `TXN-*`, local use-after rows |
| value paths/precedence across all shapes/domains | `PTH-*`, nested Copy rows, `PAR-*` |
| snapshot missing/extra/duplicate IDs/revisions/cursors/evidence | `SNP-*` |
| compile-fail raw IDs/rebinding/dependencies/fake token/reduced enum | `API-*` |
| full gates | `FUL-*` |

## 4. Constraint closure

| Constraint/non-goal | Contract enforcement |
|---|---|
| do not redesign classifier/lattice | G1.1 preserved, one visitor refactor with parity |
| no parallel value/path/slot/env model | sole-owner architecture and inventory |
| no names/spans/debug/pointers/iteration accidents as identity | identity rules and API negatives |
| no core dependency upward | dependency direction + metadata/compile-fail row |
| no fake token/handle/execution constructor | private raw construction and API matrix |
| no source gate/compat alias/dual reader/migration shim/side table | final invariants, deletions, full audit |
| do not start G1.3/G1.4/View/AWBC wire/Stream publication | implementation order §9 |
| preserve parent ABI/View/save/activation results except narrow rows | supersession delta |
| no production overlay | package member/validator rule |

## 5. Expected output closure

| Expected output item | Location |
|---|---|
| one standalone archive | outer artifact |
| `OPEN_QUESTIONS=0` | `OPEN_QUESTIONS.md`, `FINAL_STATUS.md` |
| exact Rust-shaped G1.2 symbol closure | `RUST_OWNERS_AND_APIS.md` |
| execution/local/record/transaction identity rules | final/Rust/codec docs |
| canonical codec and ordering | `IDENTITY_AND_CODEC_CONTRACT.md`, goldens |
| narrow supersession delta | `SUPERSESSION_DELTA.md` |
| complete producer/consumer/deletion inventory | inventory |
| corrected compile-clean order | `IMPLEMENTATION_ORDER.md` |
| positive/negative/tamper/full matrix | `TEST_MATRIX.csv`, negative matrix |
| no production code overlay | manifest/package validation |
| explicit verification boundary | `VALIDATION.md`, `FINAL_STATUS.md` |

## 6. Decision completeness

There is no external authority required to implement this package. Every
result-changing choice identified by the request has one selected outcome,
owner, representation, visibility, ordering, failure owner, persistence rule,
implementation cut, and test oracle. `OPEN_QUESTIONS.md` is exactly `none`.
