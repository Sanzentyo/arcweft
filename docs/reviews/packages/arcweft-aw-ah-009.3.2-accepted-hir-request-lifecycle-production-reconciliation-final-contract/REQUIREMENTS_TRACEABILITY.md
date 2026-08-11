# Requirements traceability

## 1. Required decisions

| Request decision | Normative selection | Contract section | Primary tests | Implementation cut |
| --- | --- | --- | --- | --- |
| 1. Canonical carrier | One `AcceptedProjectSnapshot` retained by `AcceptedProfileEnvironment`; not in registered world | `FINAL_CONTRACT.md` 2 | AH32-001, 022, 065 | 1-2 |
| 2. Keys and uniqueness | `LspUriKey`, existing canonical module/source identity, private `AcceptedModuleKey`, strict duplicate rejection | 2-3 | AH32-002-015, 074 | 1, 3 |
| 3. Construction | Existing bounded profile parse/lower once; same `Arc<HirProject>` borrowed by registration and retained | 3 | AH32-001, 012-022, 068 | 1 |
| 4. Overlay acceptance | Changed bytes transactional/no query until publish; identical bytes/new version metadata-only generation; failed rebuild preserves old state | 4 | AH32-023-029, 063 | 2, 5 |
| 5. Request acquisition | Exact 14-step URI/profile/environment/source/module/HIR lease route and typed failures | 5 | AH32-001, 004-007, 023, 027-029 | 3 |
| 6. Final stamp validation | Exact Arc pointer and value comparisons before work, before hit return, and after compute; HIR identity is project Arc + module key | 6 | AH32-030, 030A, 031-045 | 4 |
| 7. Cancellation/deadline | One server-owned `RequestControl` atomic, weak typed binding, 250 ms, 4 workers, 32 active, publication gate, exact cleanup | 7 | AH32-046-058C | 4 |
| 8. Cache invalidation | Direct change/close/workspace/replace/failure/shutdown APIs; old/new stale insertion impossible | 8 | AH32-059-066 | 5 |
| 9. Limits/work | HIR build outside query; 4,096 docs, 8,388,608 bytes, 262,144 symbol work; bounded reads before parse | 10 | AH32-016-022, 050-055, 068 | 1, 4 |
| 10. Memory/ownership | One retained project Arc; exact footprint; old generations retained only by at most 32 admitted contexts | 10 | AH32-022, 053-055, 058, 065-066 | 1, 4-5 |

## 2. Required implementation order

| Required order | Handoff section | Completion evidence |
| --- | --- | --- |
| 1. registry and candidate validation | Cut 1 | AH32-001-022 |
| 2. atomic world/project/generation publication | Cut 2 | AH32-023-029, profile tests |
| 3. typed acquisition | Cut 3 | AH32-001, 004-007, 023, 027-029 |
| 4. cancellation/deadline/stamp | Cut 4 | AH32-030-058C |
| 5. lifecycle invalidation | Cut 5 | AH32-059-066 |
| 6. sema connection after 009.3.1 | Cut 6 | AH32-067-071 |
| 7. focused/workspace/structural validation | Cut 7 | AH32-072-076 |

## 3. Mandatory direct tests

| Request test | Matrix coverage |
| --- | --- |
| accepted URI -> exact module/document/HIR source | AH32-001 |
| module with no declarations and non-root module | AH32-002-003 |
| duplicate source identity/URI/conflicting module mappings reject atomically | AH32-008-011 |
| unchanged/open overlay, changed success, changed failure | AH32-023-025, including 023A-023B |
| identical bytes/new LSP version | AH32-026-027 |
| profile key/state pointer, generation, world, symbol revision, character inventory, document, module, HIR changes in flight | AH32-030, 030A, 031-045 |
| cancellation before, during, immediately before publication | AH32-046-049A |
| deadline, status mapping, close, workspace removal, replacement, shutdown with no stale insertion | AH32-050-052, 058B, 060-064 |
| old accepted Arc readers memory-safe but publication rejected | AH32-065-066 |
| typed API/Cargo metadata construction and dependency evidence, no source gate | AH32-072-075 |

## 4. Fixed constraints and non-goals

| Constraint | Enforcement |
| --- | --- |
| no `CharacterNominalType` redesign | No character nominal type is defined or changed in this contract. |
| no AW-AH-009.3 result/cache semantic redesign | Original key/result/order/limits remain authoritative; only validity and lifecycle APIs are added. AH32-070 verifies parity. |
| do not select AW-AH-009.3.1 authored model | Contract carries protocol position only and waits for the landed carrier in cut 6. AH32-071. |
| no second HIR/project/parser/type environment fallback | One project Arc; AH32-068-069 instrument no build/fallback. |
| no forged `SourceSnapshotId` | Stamp contains no snapshot ID and no derivation API. |
| no signature-specific syntax database | No such type/module is present. |
| no source gates | Tests and audits use runtime typed behavior, compile-time bounds, Cargo metadata, and canonical structure tooling. |
| no compatibility/deprecated accessors | Old internal modules/constructors/string maps/cancellation set/fallback are deleted in compiling cuts. |
| no stringly module key | Existing `CanonicalModulePath` plus private `AcceptedModuleKey`. |
| no CSS/Takumi/removed-syntax diagnostics | Outside all listed files/types/tests. |
| proof-concurrency node identity not prerequisite | Existing exact document/HIR ranges are retained; no dependency introduced. |

## 5. Output and readiness gate

| Output requirement | Satisfaction |
| --- | --- |
| exact archive name | Packaging uses the requested name. |
| ten required members | Manifest verification requires exactly the ten names. |
| `OPEN_QUESTIONS.md` exactly `none` | Packaging verifies exact bytes `none\n`. |
| sorted verified manifest with zero self-entry | Packaging script computes every digest, sorts names, and checks the self-entry. |
| summary/status/SHA sidecars | Produced next to the ZIP. |
| one carrier/acquisition/cancellation owner/complete invalidation | Sections 2, 5, 7, and 8. |
| zero result-changing open decisions | `FINAL_STATUS.md` and `OPEN_QUESTIONS.md`. |
| no per-feature reparse/stale mixing | Sections 3-8; AH32-023-069. |

Every request requirement maps to a selected behavior and an observable implementation test. No requirement remains deferred inside AW-AH-009.3.2.
