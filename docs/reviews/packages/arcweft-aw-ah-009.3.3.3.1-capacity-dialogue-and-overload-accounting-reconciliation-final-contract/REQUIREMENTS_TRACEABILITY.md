# Requirements traceability

## 1. Canonical request decisions

| Request decision | Resolution | Primary package section | Executable evidence |
| --- | --- | --- | --- |
| 1. Capacity status before/after switch | `PendingAuthority → IntentionallyUnchecked`; old dispatcher/drifted schema receive zero credit | `FINAL_CORRECTION.md` §2; `FAMILY_CLASSIFICATION.md` §5 | CAP-001..CAP-021 |
| 2. Dialogue status and exact pair | `PendingAuthority → RejectingSchema`; CharacterReconfigure accepted/spread-negative pair; ContentApplication activation smoke | `FINAL_CORRECTION.md` §3; `FAMILY_CLASSIFICATION.md` §6 | DIA-001..DIA-017 |
| 3. Exact Dialogue replacement event | one compiling Proof + .4.2 + .4.3 + sema/runtime switch with same-cut Speaker/frozen-reader deletion | `FINAL_CORRECTION.md` §3, §8 | DIA-005..DIA-015 |
| 4. Cardinalities and two axes | fixed `23/21/42`, `23/22/44`, `22/22/44` current; `23/20/40`, `23/21/42`, `22/22/44` final | `README.md`; `FAMILY_CLASSIFICATION.md` §§1-3 | CLASS-001..CLASS-009 |
| 5. Retained name and physical counter | `retained_argument_inference_facts` plus required `physical_candidate_argument_evaluations` typed events | `OVERLOAD_ACCOUNTING.md` §§2-4 | OA-001..OA-024 |
| 6. Parent accounting precedence | §23 physical algorithm retained; §36 item 4 and §19 corrected; .3.3.3 broad exact-once wording superseded | `FINAL_CORRECTION.md` §6; this file §2 | OA and DRIFT rows |
| 7. Direct multi-candidate tests | candidate-contextual enum, numeric, closure, partial, generic, nested, and spread cases; no source scan/cache | `TEST_MATRIX.md` §D | OA-001..OA-024 |
| 8. Preserve stable rows | Drop, Promotion, stable 18 preserved; Speaker current-only and PendingRemoval | `FAMILY_CLASSIFICATION.md` §§3-4, §7 | CLASS, DIA-002, DIA-013..017 |
| 9. Section-19 completion | staged classification text complete; final acceptance gate remains open until no pending/removal row | `FINAL_CORRECTION.md` §7; `FAMILY_CLASSIFICATION.md` §8 | CLASS-012 |

## 2. Parent and correction precedence

### AW-AH-009.3.3 shared resolver

Retained without redesign:

- one callable catalog and one resolver entry;
- candidate order and authority rank;
- contextual candidate transactions;
- checker-owned call-target facts;
- native semantic signature projection;
- resolver/work limits and cancellation;
- selected and rejected recovery replay.

Corrected interpretations:

- section 23 remains the physical execution authority;
- section 36 item 4 means one **retained** fact per retained logical slot;
- TEST_MATRIX section 19 uses the phase-aware family table and two axes.

### AW-AH-009.3.3.1 curried groups

No change. Candidate construction and replay continue to validate the current
call group at the accepted resolved-callable boundary. Overload physical events
do not create a second curried representation.

### AW-AH-009.3.3.2 typed external publication

No change. External project callables remain identified by typed segmented
paths. No display string participates in the new evidence.

### AW-AH-009.3.3.3 returned package

Retained:

- Drop and Promotion as intentionally unchecked;
- the stable 18 rejecting families;
- the accepted/rejected versus accepted/clean-recovery taxonomy;
- deterministic retained committed/recovery facts.

Superseded:

- CapacityMethod as rejecting;
- Capacity spread rejection;
- Dialogue based on `SpeakerLine` or the frozen carrier;
- Speaker as final completion evidence;
- 20 rejecting / 3 unchecked / 46-case cardinality;
- broad `argument_expression_checks == exactly_once` wording.

### AW-AH-009.3.3.4 Capacity

Fully authoritative for typed associated receiver/callee ownership, collision
precedence, the existing Capacity candidate/ID, receiver result, and
`variadic_unchecked`. This package adds only the phase classification and
truthful evidence interpretation.

### AW-AH-009.4/.4.2/.4.3 Dialogue

Fully authoritative for first-class CharacterDialogue, ordinary
CharacterFactory/Reconfigure calls, bracket/colon ContentApplication, attached
HIR/project ownership, line identity, diagnostics, and same-switch Speaker
removal. This package adds only the pending/final evidence gate and exact
section-19 representative pair.

## 3. Required implementation order mapping

| Canonical order | Gate produced by this package | Exit condition |
| --- | --- | --- |
| 1. Staged correction | phase table and physical/retained vocabulary | design package accepted; no production behavior changed |
| 2. Capacity switch | Capacity row changes P→U | typed associated route active; old path deleted; CAP rows pass |
| 3. Other AW-AH-009.3 closure | all non-Dialogue rows and matrix evidence closed | only Dialogue remains pending; accepted-HIR/cache/limits/parity pass |
| 4. Proof prerequisite | attached source/fragments, typed HIR arena/source map, module-preserving project authority | no detached/dual reader used by Dialogue |
| 5. Atomic Dialogue switch | Dialogue P→R and Speaker U/PendingRemoval→Removed | .4.2/.4.3/sema/runtime active; all frozen readers/IDs removed |
| 6. Final matrix closure | final 22-family 19/3 and 44 cases | CLASS/DIA/OA/DRIFT gates pass; no pending disposition |

Remaining Proof runtime assertion, AWBC, codec, save/replay, and diagnostic work
is not a prerequisite for the narrow Dialogue source/HIR/project authority
except where the accepted Dialogue chain explicitly crosses a boundary.

## 4. Exact parent-row replacement map

| Parent claim | Final correction |
| --- | --- |
| `CapacityMethod = RejectingSchema` | `PendingAuthority` before .3.3.4; `IntentionallyUnchecked` after |
| Capacity negative = spread rejection | invalid; use accepted + clean-recovery unchecked pair |
| current homogeneous `_` schema earns credit | invalid implementation drift; must be deleted by switch |
| `Dialogue = RejectingSchema` via `SpeakerLine` | invalid pre-switch; Dialogue is pending until final authority |
| ContentApplication may be ordinary call | invalid; final bracket/colon typed HIR operation |
| Speaker contributes final row | invalid; current observation only, `PendingRemoval`, then deleted |
| 23 families = 20 R + 3 U | replaced by phase-dependent 18/3/2, 18/4/1, then final 19/3 over 22 |
| 46 total family cases | replaced by 42/44 current observations and 40/42/44 final cases by phase |
| every argument expression physically checked once | invalid under contextual probes/replay |
| one retained inference per committed/recovery slot | retained and clarified as normative semantic evidence |

## 5. Output requirement mapping

| Required member | Present | Content |
| --- | --- | --- |
| `README.md` | yes | sequence, hashes, scope, precedence, phase cardinalities |
| `FINAL_CORRECTION.md` | yes | exact replacement rules and switches |
| `FAMILY_CLASSIFICATION.md` | yes | exhaustive phase/final ledgers and Speaker transition |
| `OVERLOAD_ACCOUNTING.md` | yes | physical/retained semantics, types, observation points, rollback/work |
| `TEST_MATRIX.md` | yes | exact executable and drift cases |
| `REQUIREMENTS_TRACEABILITY.md` | yes | this mapping |
| `REPOSITORY_EVIDENCE.md` | yes | baseline, source owners, package verification, limitations |
| `OPEN_QUESTIONS.md` | yes | exact `none` |
| `FINAL_STATUS.md` | yes | ready status with implementation gates |
| `MANIFEST.txt` | yes | sorted SHA-256 and byte length for all other members |
| adjacent summary sidecar | yes | outside archive |
| adjacent status sidecar | yes | outside archive |
| adjacent `.zip.sha256` sidecar | yes | outside archive |

## 6. Completion claims

This package closes all design questions in the canonical request. It does not
claim that current `main` has implemented the Capacity or Dialogue authority
switches. `READY_FOR_IMPLEMENTATION` means the correction is decision-complete;
it does not override the explicit staged and final implementation gates.
