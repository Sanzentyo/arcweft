# Requirement traceability

Every mandatory requirement is mapped to concrete normative artifacts and
implementation tests. No row is marked deferred or alternative.

| Requirement | Closed requirement | Normative artifacts | Tests/gates | Status |
|---|---|---|---|---|
| `A1` | Add NeedProducerInstanceKey/TaskLaunchOrdinal and exact instance transcript. | `RUST_SCHEMAS.md §1-2`, `IDENTITY_AND_DIGESTS.md §2,§6` | `ID-FAM-01A..ID-FAM-09B`, `ID-007` | `CLOSED` |
| `A2` | Keep fixed NeedId/TaskKey/TaskId with terminal/coalescing/launch roles. | `FINAL_CONTRACT.md §2`, `IDENTITY_AND_DIGESTS.md §7` | `ID-001..ID-006` | `CLOSED` |
| `A3` | Exact Need/Task transcripts, policy tags, Join zero, Always journal transaction. | `IDENTITY_AND_DIGESTS.md §7-8`, `TASK_LIFECYCLE_AND_PERSISTENCE.md §1` | `TASK-001..TASK-008`, `ID-012..ID-015` | `CLOSED` |
| `A4` | Reusable handle/MakeNeedHandle Join only; Always handle after accepted launch. | `RUST_SCHEMAS.md §4`, `TASK_LIFECYCLE_AND_PERSISTENCE.md §2` | `TASK-009..TASK-013` | `CLOSED` |
| `A5` | AwaitMany source-order base/child instances; duplicate indexes distinct; direct Await reads handle. | `RUST_SCHEMAS.md §5`, `TASK_LIFECYCLE_AND_PERSISTENCE.md §2,§6` | `AWAIT-001..AWAIT-014` | `CLOSED` |
| `A6` | Timeout derives distinct Join instance from source NeedId, contract/site/limit. | `RUST_SCHEMAS.md §6`, `TASK_LIFECYCLE_AND_PERSISTENCE.md §7` | `AWAIT-015..AWAIT-020` | `CLOSED` |
| `A7` | Terminal conflict exact generation/Need/contract/cursor and observer separation. | `TASK_LIFECYCLE_AND_PERSISTENCE.md §3-5`, `FAILURE_PRECEDENCE_AND_ATOMICITY.md §5-6` | `EVT-001..EVT-020`, `TASK-002`, `TASK-004` | `CLOSED` |
| `B1` | Reuse existing RuntimeValueDigest for all admitted values/items. | `FINAL_CONTRACT.md §3`, `IDENTITY_AND_DIGESTS.md §9` | `DIG-001..DIG-012` | `CLOSED` |
| `B2` | Tuple grammar for empty/source-order values; no ZERO/map/Serde grammar. | `IDENTITY_AND_DIGESTS.md §9` | `DIG-001`, `DIG-002`, `DIG-011`, `DIG-012`, `AWAIT-005..AWAIT-010` | `CLOSED` |
| `B3` | Sink-parametric existing canonical encoder and RuntimeValue::NeedHandle. | `RUST_SCHEMAS.md §4,§11`, `IDENTITY_AND_DIGESTS.md §9` | `DIG-003..DIG-010` | `CLOSED` |
| `B4` | Fixed ID zero rejection/no rehash; Generation/Join zero valid; Option absence. | `RUST_SCHEMAS.md §1`, `IDENTITY_AND_DIGESTS.md §6-8` | `ID-007..ID-016` | `CLOSED` |
| `B5` | Exact producer-contract/plan ordered inputs and nonduplication proof. | `IDENTITY_AND_DIGESTS.md §3-5,§17`, `DEPENDENCY_GRAPH.md` | `ID-FAM-01A..ID-FAM-09B`, `VIEW-007` | `CLOSED` |
| `C1` | Move one GenerationId to arcweft_core::task; delete driver duplicate. | `RUST_SCHEMAS.md §1`, `OWNER_API_MAP.md`, `COMPILE_CLEAN_SEQUENCE.md Cut 4-5` | `ID-011`, `STR-017` | `CLOSED` |
| `C2` | Exact final TaskSpec/correlation/handle/event/Need outcome plus Await/journal/save/host shapes. | `RUST_SCHEMAS.md §3-10`, `TASK_LIFECYCLE_AND_PERSISTENCE.md` | `TASK-001..TASK-020`, `EVT-001..EVT-020`, `AWAIT-001..AWAIT-020` | `CLOSED` |
| `C3` | TaskSpec has no IDs; ensure_task derives/allocates correlation. | `RUST_SCHEMAS.md §3,§7-8`, `TASK_LIFECYCLE_AND_PERSISTENCE.md §1` | `TASK-001..TASK-008`, `TASK-017` | `CLOSED` |
| `C4` | One event stream; validation/cursor/duplicate/stale/conflict/cancel/journal rollback. | `TASK_LIFECYCLE_AND_PERSISTENCE.md §3-5`, `FAILURE_PRECEDENCE_AND_ATOMICITY.md` | `EVT-001..EVT-020` | `CLOSED` |
| `C5` | Domain errors stay Ready(Result::Err); infrastructure/cancellation separate. | `FINAL_CONTRACT.md §4`, `TASK_LIFECYCLE_AND_PERSISTENCE.md §4` | `EVT-015..EVT-017` | `CLOSED` |
| `C6` | AwbcTaskPlan producer row, no self digest, recomputation at verifier/restore. | `RUST_SCHEMAS.md §10`, `IDENTITY_AND_DIGESTS.md §4`, `DELETION_MATRIX.md` | `STR-006`, `STR-007`, `ID-FAM-02A`, `ID-FAM-02B` | `CLOSED` |
| `D1` | Exact generic CheckedMatchSemanticDigest include/exclude set. | `IDENTITY_AND_DIGESTS.md §10`, `CHECKED_MATCH_AND_VIEW_ADMISSION.md §7` | `MATCH-001..MATCH-003` | `CLOSED` |
| `D2` | Separate CheckedViewMatchAdmissionDigest with exact retained/evidence/producer inputs. | `IDENTITY_AND_DIGESTS.md §11-13`, `CHECKED_MATCH_AND_VIEW_ADMISSION.md §8-9` | `VIEW-001..VIEW-003`, `VIEW-010..VIEW-016` | `CLOSED` |
| `D3` | Exact CheckedViewMatchCoordinate and stable ViewMatchSiteId derivation. | `RUST_SCHEMAS.md §13-14`, `IDENTITY_AND_DIGESTS.md §14` | `VIEW-004..VIEW-006` | `CLOSED` |
| `D4` | Use current ViewProgramId and AcceptedViewProgramRevision([u8;32]); no invented owner/u32. | `FINAL_CONTRACT.md §8`, `CHECKED_MATCH_AND_VIEW_ADMISSION.md §10-13` | `STR-001`, `STR-002` | `CLOSED` |
| `D5` | View plan/producer commits program/site/admission and excludes revision. | `IDENTITY_AND_DIGESTS.md §4,§15` | `VIEW-007`, `VIEW-008` | `CLOSED` |
| `D6` | Bundle/replacement carry revision and explicit full-evidence rebind; no NeedId translation. | `TASK_LIFECYCLE_AND_PERSISTENCE.md §11`, `CHECKED_MATCH_AND_VIEW_ADMISSION.md §11-13` | `VIEW-008..VIEW-018` | `CLOSED` |
| `E1` | Generic CheckedMatch validates only generic semantics/coverage/digest. | `CHECKED_MATCH_AND_VIEW_ADMISSION.md §1-2` | `MATCH-018..MATCH-020`, `OWN-021` | `CLOSED` |
| `E2` | Bounded Maranget retained with exact literal-only guard correction/FalseGuard precedence. | `CHECKED_MATCH_AND_VIEW_ADMISSION.md §3-6` | `MATCH-004..MATCH-017` | `CLOSED` |
| `E3` | Separate CheckedViewMatchAdmission blocks only View row. | `CHECKED_MATCH_AND_VIEW_ADMISSION.md §8-9` | `VIEW-001`, `MATCH-018`, `OWN-021` | `CLOSED` |
| `E4` | Separate producer argument/capture certificate does not construct contract identity. | `CHECKED_MATCH_AND_VIEW_ADMISSION.md §8`, `IDENTITY_AND_DIGESTS.md §12` | `OWN-022`, `VIEW-012` | `CLOSED` |
| `F1` | Mandatory opaque input value_class/persistence through catalog digest; constructors require both. | `RUST_SCHEMAS.md §15`, `OWNERSHIP_EVIDENCE.md §1-2,§11` | `OWN-001..OWN-005` | `CLOSED` |
| `F2` | AgentResource/Body use current Agent DTO and no unkeyed registry. | `OWNERSHIP_EVIDENCE.md §3,§9,§13` | `OWN-009`, `OWN-010`, `STR-004`, `STR-005` | `CLOSED` |
| `F3` | Ownership context exactly ProjectSymbolTable + RegisteredSemanticWorld. | `RUST_SCHEMAS.md §13`, `OWNERSHIP_EVIDENCE.md §3` | `STR-004`, `STR-005` | `CLOSED` |
| `F4` | Correct Need/Ref/ViewValue/Function/Shared/opaque/affine rows. | `OWNERSHIP_EVIDENCE.md §6-9`, `tables/ownership_matrix.csv` | `OWN-006..OWN-018`, `VIEW-001..VIEW-003` | `CLOSED` |
| `F5` | Classifier applied only to retained View and producer arguments, not generic Match. | `OWNERSHIP_EVIDENCE.md §10`, `CHECKED_MATCH_AND_VIEW_ADMISSION.md §1` | `MATCH-018`, `OWN-021` | `CLOSED` |
| `F6` | Every current TypeKind has exact owner/recursion/cycle/limit/first error. | `OWNERSHIP_EVIDENCE.md §4-9`, `machine/ownership_matrix.json` | `OWN-018`, `LIM-04A`, `LIM-04B`, `LIM-05A`, `LIM-05B` | `CLOSED` |
| `CUT1` | Required compile-clean dependency cut 1. | `COMPILE_CLEAN_SEQUENCE.md Cut 1`, `DELETION_MATRIX.md` | `STR-001`, `STR-002`, `STR-003`, `STR-004`, `STR-005`, `STR-006`, `STR-007`, `STR-008`, `STR-009`, `STR-010`, `STR-011`, `STR-012`, `STR-013`, `STR-014`, `STR-015`, `STR-016`, `STR-017`, `STR-018`, `STR-019`, `STR-020` | `CLOSED` |
| `CUT2` | Required compile-clean dependency cut 2. | `COMPILE_CLEAN_SEQUENCE.md Cut 2`, `DELETION_MATRIX.md` | `STR-001`, `STR-002`, `STR-003`, `STR-004`, `STR-005`, `STR-006`, `STR-007`, `STR-008`, `STR-009`, `STR-010`, `STR-011`, `STR-012`, `STR-013`, `STR-014`, `STR-015`, `STR-016`, `STR-017`, `STR-018`, `STR-019`, `STR-020` | `CLOSED` |
| `CUT3` | Required compile-clean dependency cut 3. | `COMPILE_CLEAN_SEQUENCE.md Cut 3`, `DELETION_MATRIX.md` | `STR-001`, `STR-002`, `STR-003`, `STR-004`, `STR-005`, `STR-006`, `STR-007`, `STR-008`, `STR-009`, `STR-010`, `STR-011`, `STR-012`, `STR-013`, `STR-014`, `STR-015`, `STR-016`, `STR-017`, `STR-018`, `STR-019`, `STR-020` | `CLOSED` |
| `CUT4` | Required compile-clean dependency cut 4. | `COMPILE_CLEAN_SEQUENCE.md Cut 4`, `DELETION_MATRIX.md` | `STR-001`, `STR-002`, `STR-003`, `STR-004`, `STR-005`, `STR-006`, `STR-007`, `STR-008`, `STR-009`, `STR-010`, `STR-011`, `STR-012`, `STR-013`, `STR-014`, `STR-015`, `STR-016`, `STR-017`, `STR-018`, `STR-019`, `STR-020` | `CLOSED` |
| `CUT5` | Required compile-clean dependency cut 5. | `COMPILE_CLEAN_SEQUENCE.md Cut 5`, `DELETION_MATRIX.md` | `STR-001`, `STR-002`, `STR-003`, `STR-004`, `STR-005`, `STR-006`, `STR-007`, `STR-008`, `STR-009`, `STR-010`, `STR-011`, `STR-012`, `STR-013`, `STR-014`, `STR-015`, `STR-016`, `STR-017`, `STR-018`, `STR-019`, `STR-020` | `CLOSED` |
| `ART1` | README, exact reading order, full SHA, final status | `README.md`, `FINAL_STATUS.md` | `PKG-VALIDATE` | `CLOSED` |
| `ART2` | OPEN_QUESTIONS exactly none | `OPEN_QUESTIONS.md` | `PKG-VALIDATE` | `CLOSED` |
| `ART3` | complete final contract and decision register | `FINAL_CONTRACT.md`, `DECISION_REGISTER.md` | `PKG-VALIDATE` | `CLOSED` |
| `ART4` | Rust schemas and owner/API map | `RUST_SCHEMAS.md`, `OWNER_API_MAP.md` | `PKG-VALIDATE` | `CLOSED` |
| `ART5` | dependency graph | `DEPENDENCY_GRAPH.md` | `PKG-VALIDATE` | `CLOSED` |
| `ART6` | identity domains and policy truth table | `IDENTITY_AND_DIGESTS.md`, `tables/task_policy_truth_table.csv` | `PKG-VALIDATE` | `CLOSED` |
| `ART7` | sink canonical owner and all digest grammars | `IDENTITY_AND_DIGESTS.md`, `RUST_SCHEMAS.md` | `PKG-VALIDATE` | `CLOSED` |
| `ART8` | final task/journal/snapshot/replay/replacement schemas | `TASK_LIFECYCLE_AND_PERSISTENCE.md`, `RUST_SCHEMAS.md` | `PKG-VALIDATE` | `CLOSED` |
| `ART9` | coverage and View admission matrices | `CHECKED_MATCH_AND_VIEW_ADMISSION.md` | `PKG-VALIDATE` | `CLOSED` |
| `ART10` | opaque evidence publication chain | `OWNERSHIP_EVIDENCE.md` | `PKG-VALIDATE` | `CLOSED` |
| `ART11` | deletion matrix and compile sequence | `DELETION_MATRIX.md`, `COMPILE_CLEAN_SEQUENCE.md` | `PKG-VALIDATE` | `CLOSED` |
| `ART12` | source evidence with line ranges | `SOURCE_EVIDENCE.md`, `machine/source_evidence.json` | `PKG-VALIDATE` | `CLOSED` |
| `ART13` | full test matrix | `TEST_MATRIX.md`, `machine/tests.json`, `tables/tests.csv` | `PKG-VALIDATE` | `CLOSED` |
| `ART14` | machine/human validation | `VALIDATION.md`, `VALIDATION_OUTPUT.txt`, `machine/validation.json` | `PKG-VALIDATE` | `CLOSED` |
| `ART15` | internal SHA-256 manifest and exact request hashes | `MANIFEST.json`, `MANIFEST.sha256`, `machine/request_hashes.json` | `PKG-VALIDATE` | `CLOSED` |
| `CON-01` | Design-only; no production patch/overlay. | `FINAL_CONTRACT.md`, `STRUCTURAL_ABSENCE.md`, `DELETION_MATRIX.md` | `STR-001..STR-020` | `CLOSED` |
| `CON-02` | No AWBC numeric table/reallocation. | `FINAL_CONTRACT.md`, `STRUCTURAL_ABSENCE.md`, `DELETION_MATRIX.md` | `STR-001..STR-020` | `CLOSED` |
| `CON-03` | No second producer identity/String/source/HIR identity. | `FINAL_CONTRACT.md`, `STRUCTURAL_ABSENCE.md`, `DELETION_MATRIX.md` | `STR-001..STR-020` | `CLOSED` |
| `CON-04` | No extension trait/copied registry/default opaque evidence. | `FINAL_CONTRACT.md`, `STRUCTURAL_ABSENCE.md`, `DELETION_MATRIX.md` | `STR-001..STR-020` | `CLOSED` |
| `CON-05` | No compatibility reader/dual carrier. | `FINAL_CONTRACT.md`, `STRUCTURAL_ABSENCE.md`, `DELETION_MATRIX.md` | `STR-001..STR-020` | `CLOSED` |
| `CON-06` | No redesign of selector ABI/guard Branch/View-core independence. | `FINAL_CONTRACT.md`, `STRUCTURAL_ABSENCE.md`, `DELETION_MATRIX.md` | `STR-001..STR-020` | `CLOSED` |
| `CON-07` | No redesign of parent lifecycle/timeout order/line/Stream. | `FINAL_CONTRACT.md`, `STRUCTURAL_ABSENCE.md`, `DELETION_MATRIX.md` | `STR-001..STR-020` | `CLOSED` |
| `CON-08` | No unrelated Dialogue/RichText/Stream/CSS/Takumi/outcome/map/receiver work. | `FINAL_CONTRACT.md`, `STRUCTURAL_ABSENCE.md`, `DELETION_MATRIX.md` | `STR-001..STR-020` | `CLOSED` |

## Completeness rule

The package validator requires every `A1..A7`, `B1..B5`, `C1..C6`,
`D1..D6`, `E1..E4`, `F1..F6`, `CUT1..CUT5`, and `ART1..ART15` row exactly
once with `CLOSED` status. Extra constraint rows strengthen but do not replace
those mandatory rows.
