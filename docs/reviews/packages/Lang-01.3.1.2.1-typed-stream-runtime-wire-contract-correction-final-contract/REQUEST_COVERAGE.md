# Sole-request coverage map

The sole normative input is `SOURCE_REQUEST.md` (SHA-256 `b8ec6b3efcbe31739b2d2b7bad119c2a9dcc394b95777941c4210fc485315f64`). This map is an
index; it does not add requirements.

## Required decisions

| ID | Request decision | Normative location | Test coverage |
| --- | --- | --- | --- |
| RD-01 | Lossless shared callable parameter projection and Optional decision | FINAL_CONTRACT §4; RUST_SCHEMAS §4/§18; WIRE §6/§7 | STR-CALL-* |
| RD-02 | Exactly one live/tombstone owner; lookup, fibers, moves, producer, snapshot/restore | FINAL_CONTRACT §6; RUST_SCHEMAS §10-15/§19 | STR-OWN-*; STR-SAVE ownership cases |
| RD-03 | Replay variants/store/order/identity/limits/redaction/hash/summary/erasure/tombstone | FINAL_CONTRACT §9; RUST_SCHEMAS §14-15; WIRE §9 | STR-RPL-* |
| RD-04 | RuntimeEffectSetId canonical owner/table/AWBC/fingerprint/tamper | FINAL_CONTRACT §10; RUST_SCHEMAS §5; WIRE §3-4 | STR-EFF-* |
| RD-05 | Complete RuntimePlan support types and debug-only source maps | FINAL_CONTRACT §11; RUST_SCHEMAS §2/§7/§18 | STR-PLAN-* |
| RD-06 | Typed profile input, maxima/flags/floors/restart/replacement/first-error order | FINAL_CONTRACT §12; POLICY_PROFILE §§1-9; RUST_SCHEMAS §21 | STR-PROF-* |
| RD-07 | Exact exhaustion split, staging, counters, terminal/close/result/observation/repeat | FINAL_CONTRACT §8/§12; RUST_SCHEMAS §13/§16-17 | STR-EXH-* |
| RD-08 | Dropped-consumer queue lifecycle, bounds, release/tombstone condition | FINAL_CONTRACT §7/§12; RUST_SCHEMAS §12-15 | STR-DROP-* |
| RD-09 | Lang-01.1.1 reconciliation and one ABI2/codec8/save2 ownership | FINAL_CONTRACT §13; IMPLEMENTATION_ORDER §§1-3 | STR-ABI-*; STR-SAVE-* |
| RD-10 | Numeric/codec allocation rebased to current main and canonical order | WIRE_AND_VERSION_ALLOCATIONS §§1-7/§13 | STR-ABI allocation/table tests |
| RD-11 | Every Stream-owned host/save integer canonical decimal string and strict JSON | FINAL_CONTRACT §14; WIRE §11; RUST_SCHEMAS §3/§16/§19 | STR-JSON-* |
| RD-12 | Deletion inventory and amended test matrix | DELETION_INVENTORY; TEST_MATRIX | STR-DEL-* plus all invariant rows |

## Required implementation order

| Request step | Requirement | Contract location |
| --- | --- | --- |
| 1 | Freeze corrected contracts | IMPLEMENTATION_ORDER Cut 1 |
| 2 | Consume codec-stable Lang-01.1.1 substrate without provisional Stream wire | Cut 2 |
| 3 | Consume shared-resolver external callable evidence | Cut 3 |
| 4 | Corrected core identities/policy/lifecycle/instance/replay/requests/events/tests | Cut 4 |
| 5 | Replace RuntimePlan Source/old Stream ownership | Cut 5 |
| 6 | One atomic ABI2/codec8 generator+Stream migration | Protected group Cut 6 |
| 7 | One shared RuntimeStep/native/web/Agent boundary | Protected group Cut 7 |
| 8 | One save2/bundle/restore/fingerprint/hot-reload/pin migration | Protected group Cut 8 |
| 9 | Delete Source/provisional paths and run workspace/audit | Cut 9 |

## Required test classes

Every test bullet in the request is represented by stable prefixes:

- shared callable projection/bypass/source-recovery: `STR-CALL-*`;
- single owner/moves/producer/drop/tombstone/snapshot/tamper: `STR-OWN-*`;
- replay/privacy/retention/erasure: `STR-RPL-*`;
- effect canonicalization/fingerprint/AWBC/tamper: `STR-EFF-*`;
- every profile maximum/flag/tightening/native-web-Agent rejection: `STR-PROF-*`;
- MAX-1/MAX/exhausted host/local/delivery/result/observation/close atomicity: `STR-EXH-*`;
- empty/nonempty/pending/terminal/drop-retention/eventual release: `STR-DROP-*`;
- support IDs/source-map nonidentity/branch/match/for-await: `STR-PLAN-*`;
- ABI2/codec8 direct suspension/generator/external/derived/safe-point/producer/old rejection:
  `STR-ABI-*`;
- save2 queues/generator/replay/tombstones/affine uniqueness/tamper/old schema/atomic restore:
  `STR-SAVE-*`;
- decimal strings/duplicates/unknown/BOM/UTF-8/native-web-Agent bytes: `STR-JSON-*`;
- sole table/authority/reader-writer, deletion, dependency/audit: `STR-DEL-*`.

## Expected output artifacts

| Artifact | Coverage |
| --- | --- |
| FINAL_CONTRACT.md | All decisions closed; OPEN_QUESTIONS=0 |
| NORMATIVE_DELTA.md | Field/owner/variant/invariant/tag/superseded-shape ledger |
| RUST_SCHEMAS.md | Exact Rust-shaped owners and every support type/reuse link |
| WIRE_AND_VERSION_ALLOCATIONS.md | Versions, full table order, tags, transcripts, strict wire |
| POLICY_PROFILE.md | Typed target/project profile and first-error order |
| IMPLEMENTATION_ORDER.md | Nine cuts and named ABI/codec/bundle/save owners |
| TEST_MATRIX.md/.json/.csv | Stable exhaustive positive/negative/tamper/parity matrix |
| DELETION_INVENTORY.md | Amended Source/provisional Stream deletion/preservation inventory |
| WORKED_EXAMPLES.md | Host JSON, AWBC, fingerprint, save/restore, replay, exhaustion/drop |
| REPOSITORY_EVIDENCE.md | Implemented substrate versus proposed changes at exact main SHA |
| VERIFICATION_REPORT.md | Actual/deferred verification and package integrity |
| MANIFEST.json + MANIFEST.sha256 | Byte counts and SHA-256 integrity envelope |

## Constraint/non-goal enforcement

`FINAL_CONTRACT` §§1/13/16, `IMPLEMENTATION_ORDER` §1, and
`DELETION_INVENTORY` explicitly preserve verified callable/direct-suspension/FiberState
substrate; keep core/data crates Sans I/O; prohibit compatibility shims, dual formats,
source gates, endpoint DTOs, extension-trait/sidecar detours, source-name recovery, and
CSS/Takumi paths; and leave unrelated proof/concurrency/style/view/Need/task domains out
of scope.
