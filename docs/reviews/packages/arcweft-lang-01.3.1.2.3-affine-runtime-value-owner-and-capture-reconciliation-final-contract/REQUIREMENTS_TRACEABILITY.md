# Requirements traceability

This table maps every required decision, inventory, test family, and constraint in the Lang-01.3.1.2.3 request to one selected result. `FINAL_CONTRACT.md` is normative; the other files make the result exact enough to implement and test without choosing a different behavior.

## 1. Numbered required decisions

| Request requirement | Selected result | Normative/detail owner | Direct evidence/tests |
|---|---|---|---|
| R1. Sole generic runtime ownership classification and token/evidence owner | `RuntimeValueOwnership::{Unrestricted, Affine}` with recursive join; one opaque `RuntimeAffineOwnerToken` per affine leaf; `StreamHandle` owns the token and the sole Stream table remains lifecycle/lease authority; aggregates/closures/partials own tokens only transitively | `FINAL_CONTRACT.md` §§2–3; `RUST_OWNERS_AND_APIS.md` §§2–4 | `OWN-001..030`, especially classification, nested paths, token opacity, token/table reciprocity |
| R2. Public Rust API: no unconditional Clone/Copy, checked duplication, move/transfer/drop, snapshot-candidate distinction | Executable value graph types have no `Clone`/`Copy`; only `RuntimeValue::try_duplicate_unrestricted`; movement is typed slot `take/put` and prepared transactions; language Drop is table-aware; snapshot image is dormant evidence and restore candidate is non-clone runnable typestate | `FINAL_CONTRACT.md` §3; `RUST_OWNERS_AND_APIS.md` §§5–7, 13–14; `SNAPSHOT_SAVE_RESTORE_CONTRACT.md` §§1–2, 7–11 | `DUP-001..028`, `XFER-001..012`, `DROP-001..010`, `SNAP-001..060`, compile-fail rows |
| R3. Structured closure capture from accepted typed evidence | Capture exactly accepted HIR free locals keyed by `(closure_expr_id, outer_local_id)`; first-use order; nearest binding; parameters excluded; `Copy` for unrestricted and `Move` for affine; no borrow escape/whole-env fallback/source reconstruction; preflight/stage/source-revision-and-owner-set recheck/non-fallible commit atomicity; nested closure and partial behavior fixed | `FINAL_CONTRACT.md` §§4–5; `RUST_OWNERS_AND_APIS.md` §§8–9; `STRUCTURED_RUNTIME_TRANSFER_SEMANTICS.md` §§3–4 | `CAP-001..030`; executable model tests for mixed copy/move, post-copy failure, affine-copy rejection |
| R4. Every generic operation that currently clones/fans out | Explicit borrow/copy/move/drop matrix for lookup, let/pattern, aggregates, projections, variants/rest, call/return, capture/partial, assignment, cross-fiber, iteration, sequence operations, equality, and unwind cleanup | `FINAL_CONTRACT.md` §6; `STRUCTURED_RUNTIME_TRANSFER_SEMANTICS.md` §§2–15 | `OPS-001..080`, `XFER-*`, `DROP-*`; structured/AWBC/compiled parity rows |
| R5. Exact borrow/move/unrestricted result for indexing, slicing, and repeat | Ordinary index/slice preserve source and require the entire sequence recursively `Unrestricted`, including an empty slice; affine extraction is only consuming destructure/iterator/internal whole-sequence take; repeat 0 consumes+drops, 1 moves, exact ≥2 requires unrestricted and makes `n-1` copies plus original; dynamic repeat requires unrestricted | `FINAL_CONTRACT.md` §6.2; `RUST_OWNERS_AND_APIS.md` §10; `STRUCTURED_RUNTIME_TRANSFER_SEMANTICS.md` §§7–8 | `OPS-039..060` and related boundary rows; executable model repeat/index/slice tests |
| R6. AWBC ABI-2 register/frame/verifier rules | One generic register state (`Uninitialized`, `Live`, `Moved`, `Dropped`); existing Move consumes; Drop is table-aware; new `CopyValue=0x2a` performs checked unrestricted copy; every operand has inherent Borrow/Copy/Consume/Destination use; exact joins, cleanup, safe-point, child transfer, compiled parity, and trap atomicity | `FINAL_CONTRACT.md` §7; `AWBC_ABI2_OWNERSHIP_CONTRACT.md` §§1–15 | `AWBC-001..053`; exact codec row and unknown-tag tests; parity/tamper rows |
| R7. Snapshot/save/restore ownership | Whole execution freezes; image contains strict dormant owner evidence only; exact traversal and generation-pin equality; restore only into empty target or atomic replacement of frozen session; old owners retire before candidate activation; fixed rejection order; failed restore leaves live state unchanged; no open/evaluation/replay/provider work during restore | `FINAL_CONTRACT.md` §8; `SNAPSHOT_SAVE_RESTORE_CONTRACT.md` §§2–17 | `SNAP-001..060`; executable model dormant-copy, duplicate-owner, exclusive-install, failed-replace tests |
| R8. Host/replay/persistent eligibility | The current `RuntimePayload(pub RuntimeValue)` wrapper is replaced in place by a closed recursively payload-safe enum that may remain `Clone`; no function/partial/handle/token/iterator/reference/continuation/generic RuntimeValue crosses general host/replay/canonical data; Stream-specific accepted typed host/replay/save boundaries remain; generation pins include every nested affine partial/handle | `FINAL_CONTRACT.md` §9; `PLAN_HOST_REPLAY_PERSISTENCE.md` §§3–9; `SNAPSHOT_SAVE_RESTORE_CONTRACT.md` §§6, 13–16 | `BOUND-001..036`, `STREAM-011..015`, parent replay/host/save rows |
| R9. `RuntimeExpr::Value`, plan clones, AOT/JIT caches, fixtures | Delete both live literal variants; use checked constant IDs/table; make `RuntimePlan` non-Clone/non-Serde and share only `Arc<RuntimePlan>`; normalize original `RuntimeFlow` to immutable block IDs; delete `FlowOp::{Bind, LoopNext, WhileNext, WhileLetNext, ForNext}` and pending cloned-op queue; keep live iterator/continuation on original control-frame enum; expression instantiation clones only closed data, pattern matching borrows, and affine fixtures use the real private test authority/Open path | `FINAL_CONTRACT.md` §10; `RUST_OWNERS_AND_APIS.md` §12; `PLAN_HOST_REPLAY_PERSISTENCE.md` §§1–3 | `PLAN-001..020`, especially Arc-only cache, removed-variant compile-fail, block/control-frame, and no-live-value tests |
| R10. Compile-clean interleave with Lang-01.3 P4+C1 through P8+C6 | Stage 0 re-pin; G1 API/evidence; G2 plan+structured; G3 AWBC/fiber/compiled/snapshot and atomic Clone removal; only then P4+C1 mints first handle; P5+C2; C3; protected P6+C4 (publishes `0x2a`); P7+C5; P8+C6; final parent/full matrix | `FINAL_CONTRACT.md` §11; `IMPLEMENTATION_ORDER.md` §§0–12 | `STREAM-001..020`, `FULL-001..016`; boundary tests prove no pre-G3/P4 constructor |

The new matrix contains 395 cases: `OWN=30`, `DUP=28`, `XFER=12`, `DROP=10`, `CAP=30`, `OPS=80`, `AWBC=53`, `SNAP=60`, `BOUND=36`, `PLAN=20`, `STREAM=20`, and `FULL=16`. Machine-identical rows are in `TEST_MATRIX.json` and `TEST_MATRIX.csv`.

## 2. Required consumer inventory

| Required area | Coverage file/section | Required final action |
|---|---|---|
| `arcweft-core::value`, ranges, sequences, patterns | `CONSUMER_AND_DELETION_INVENTORY.md` §§1–2 | Extend the original `RuntimeValue`, `RuntimePayload`, `RuntimeSeq`, `RuntimeIterator`, `RuntimePattern`, and environment owners through inherent APIs; migrate every clone/fan-out path; delete executable `Clone`, live pattern literals, old sequence index+clone, and raw `RuntimeValue` payload/codec routes |
| Structured environment/evaluation/suspension and AOT plans | inventory §§3–4; `STRUCTURED_RUNTIME_TRANSFER_SEMANTICS.md`; `PLAN_HOST_REPLAY_PERSISTENCE.md` | Typed slots/capture/binding plans; original flow block arena and control-frame continuation; no ambient snapshot, pending cloned op, or runtime-only plan variant; expression/pattern constants by ID; Arc-only plan sharing; no live cache value or direct RuntimePlan codec |
| AWBC schema/codec/verifier/VM/fiber/product-step/snapshot/compiled exchange | inventory §§5–6; `AWBC_ABI2_OWNERSHIP_CONTRACT.md` | One generic register owner; consume/copy/drop facts; strict `0x2a`; one owned exchange; no facade clone/rebuild |
| `arcweft-runtime-plan`, compiler constants/lowering, accelerator/JIT | inventory §§7–8; plan document | Project accepted evidence to generic capture/constant/operand facts; immutable plan sharing only; compiled parity |
| runtime-driver save/restore/swap, runtime host, native/Web/Agent | inventory §§9–11; snapshot/host documents | Freeze/build/validate/activate/swap transaction; typed payload rejection; shared core host boundary; no adapter DTO owner |
| bundle and canonical value codecs | inventory §10; plan document §§4–6 | Closed payload/value-snapshot codecs only; strict ABI2/codec8/bundle6/save2 cuts; no generic runnable RuntimeValue codec |
| three Lang-01.3 parent package owners | `SUPERSESSION_DELTA.md`; `PARENT_TEST_MATRIX_INDEX.json` | Preserve all accepted grouped coordinate/product/handle/table/lifecycle/replay/host/wire/save results except explicit ownership/failure/snapshot/`0x2a` corrections |

`CONSUMER_AND_DELETION_INVENTORY.md` names production paths by responsibility and provides the deletion/retention/replacement outcome for each current clone surface. Because no current-main Git checkout/full head SHA was available, implementation Stage 0 must update moved path names against a pinned checkout without changing the selected ownership result unless a concrete result-changing conflict is found.

## 3. Required tests

| Request test family | Contract rows |
|---|---|
| unrestricted scalar/aggregate/closure/external-partial duplication | `DUP-001..012`, `STREAM-006` |
| direct/recursive affine duplication rejection | `DUP-013..028`, `STREAM-007` |
| affine capture/nested capture/partial/call/return/move/cross-fiber/drop/use-after-move | `CAP-*`, `XFER-*`, `DROP-*`, `OPS-*`, `STREAM-003..010` |
| exact evaluation/transfer order and failure non-mutation | `CAP-020..030`, `OPS` atomicity rows, `AWBC-041..053`, `SNAP` failure rows |
| iterator-next exactly once; repeat/get/slice exact/boundary rules | `OPS-039..060` and executable model tests |
| branch/match joins and unwind cleanup | `OPS` branch/match/cleanup rows, `AWBC` join/cleanup rows |
| structured/AWBC/compiled parity | `AWBC` parity rows, `STREAM-016..017`, `FULL-010` |
| snapshot exclusivity, duplicate lease/token tamper, failed restore, generation pins, no open/evaluation replay | `SNAP-001..060`, `STREAM-012..015`, parent save rows |
| general payload/replay/host rejection | `BOUND-001..036`, `STREAM-011` |
| compile-fail removed Clone/Copy surfaces | compile-fail rows across `OWN`, `DUP`, `XFER`, `AWBC`, `PLAN`, plus `FULL-008` |
| full Lang-01.3 parent matrix | `PARENT_TEST_MATRIX_INDEX.json`: `.1=530`, `.2=168`, `.2.1=105`, total `803`; `FULL-012..014` |
| workspace check/strict Clippy/Tier 2/metadata/structure audit | `FULL-001..007`, `FULL-015..016` |

The included Python model executes 20 focused ownership-law tests. It is supporting behavioral evidence, not a substitute for the production Cargo/Tier-2 matrix.

## 4. Constraints and non-goals

| Constraint | Enforcement |
|---|---|
| No panic-on-Clone, silent sharing, lease rotation, or deferred uniqueness | No executable `Clone`; checked copy fails before mutation; one private token/lease/table commit; AWBC verifier and runtime both check |
| No debug-string side table, Stream-only value enum/register model, second environment, copied capture registry, or source reconstruction | One `RuntimeValue`; token nested in value; one generic register state; accepted HIR capture plan; owner/path identities are typed |
| No compatibility aliases, dual readers, migration shims, endpoint DTOs, source gates, removed-syntax diagnostics, CSS, or Takumi | Direct deletion/interleave in `IMPLEMENTATION_ORDER.md`; strict codec/version cuts; `FULL-015` architecture gate |
| Core/data Sans I/O and layer direction | Owners remain in core/data/plan/bundle/save layers; adapters own I/O; Cargo metadata and structure audit rows prove final graph |
| Do not redesign callable selection/accounting, grouped coordinates, Stream lifecycle/replay/policy, Proof identity, ordinary-function syntax | Scope/precedence in `FINAL_CONTRACT.md` §1 and exact parent delta in `SUPERSESSION_DELTA.md` |
| Add missing behavior to original Arcweft owner, not ad hoc helpers/extension traits | APIs are inherent on `RuntimeValue`, `RuntimeFunctionValue`, `RuntimeSeq`, `RuntimeIterator`, `RuntimePattern`, `RuntimePayload`, `StreamHandle`, `AwbcInstruction`, and existing table/environment owners; policy recorded in `AGENTS_AND_RUST_POLICY.md` |

## 5. Output and readiness gate

| Output requirement | Delivered member |
|---|---|
| `OPEN_QUESTIONS=0` | `OPEN_QUESTIONS.md` is exactly `none\n`; `FINAL_STATUS.md` records zero result-changing decisions |
| Exact Rust-shaped owners/APIs | `RUST_OWNERS_AND_APIS.md` |
| Structured/AWBC transfer semantics | `STRUCTURED_RUNTIME_TRANSFER_SEMANTICS.md`; `AWBC_ABI2_OWNERSHIP_CONTRACT.md` |
| Snapshot/save rules | `SNAPSHOT_SAVE_RESTORE_CONTRACT.md` |
| Supersession delta | `SUPERSESSION_DELTA.md` |
| Complete consumer/deletion inventory | `CONSUMER_AND_DELETION_INVENTORY.md` |
| Ordered compile-clean plan | `IMPLEMENTATION_ORDER.md` |
| Positive/negative/tamper/full matrix | `TEST_MATRIX.md/.json/.csv`; `PARENT_TEST_MATRIX_INDEX.json` |
| Machine-readable decision summary | `contract.json` |
| Verification honesty | `REPOSITORY_EVIDENCE_AND_VERIFICATION_SCOPE.md`; `validation/VALIDATION_REPORT.md` |
| Integrity and deterministic packaging | `MANIFEST.txt`; `validation/verify_contract.py`; `validation/build_zip.py` |

Readiness means the result-changing choices are closed at baseline `177ba1e61e43fb2da2149869ce35e165d1e93b66`. It does **not** mean the proposed Rust APIs have already compiled against a pinned current `main` checkout; that implementation evidence is deliberately deferred and must be recorded at the G/P/C gates.
