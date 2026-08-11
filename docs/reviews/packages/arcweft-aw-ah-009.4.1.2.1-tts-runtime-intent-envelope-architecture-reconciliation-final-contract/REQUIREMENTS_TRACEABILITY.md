# Requirements traceability

## 1. Request decisions

| Request requirement | Selected contract | Primary design section | Test coverage |
|---|---|---|---|
| 1 typed executable representation | nominal `std.audio.TtsSynthesisIntent` `RuntimePayload`; bridge-owned; generic core intent stage | Final C1–C4; Layout §§1–8 | SRC, AWBC, LAY, VIS |
| 2 dependency direction | narrow bridge; core/sema/scheduler audio-free; exact graph | Ownership §§1–2 | DEP-001–008, VIS |
| 3 preparation boundary | catalog inherent method orchestrated by driver local transaction before every publication | Final C6; Execution §§2–3 | PREP-001–028 |
| 4 task identity/joining | selected fingerprint; `tts.v1.<64hex>`; one scheduler; observer admission | Final C7–C8; Execution §4 | JOIN-001–032, CANCEL |
| 5 host visibility | typed registration; driver emits selected-only dispatch; runtime-host matches token and calls `submit_tts`; provider gets credential slot only | Final C9; Execution §5 | HOST-001–010, VIS-003–005 |
| 6 result/progress/error | exact nominal wrappers; typed generic errors; AwaitErr/Result semantics | Final C10; Execution §§6–8 | NEED-001–018, LAY |
| 7 AWBC/save/replay/reload | AWBC 8 direct replacement; existing blockers; schema 1 typed outcomes; exact reload tuple | Final C11; Layout §8; Save/Replay/Reload | AWBC, SAVE, REPLAY, RELOAD |
| 8 source integration | three accepted ordinary functions through shared callable registry | Final C5 | SRC-001–018 |

## 2. Required implementation order

| Request order | Handoff cut | Verification |
|---:|---|---|
| 1 freeze graph/type/API | Cuts 1–2 | DEP, VIS, LAY, AWBC |
| 2 direct codec/layout/visibility tests | Cuts 1–2 | LAY, AWBC, VIS |
| 3 connect callable lowering | Cut 3 | SRC |
| 4 atomic driver preparation | Cut 4 | PREP |
| 5 publish key/class/policy/pin | Cuts 4–5 | JOIN, PREP, DEP |
| 6 typed outcomes to Need/replay 1 | Cuts 6–7 | NEED, REPLAY |
| 7 cancellation/save/reload negatives | Cuts 5–7 | CANCEL, SAVE, RELOAD |
| 8 delete provisional bridge | Cut 8 | STR-006–007, T2-012 |

## 3. Required test bullets

| Request test bullet | Exact matrix rows |
|---|---|
| ordinary call to one typed intent; no string/JSON/TOML | SRC-001–018, AWBC-025, STR-004–006 |
| nominal/layout/missing/extra/nested/trunc/trailing/limits | LAY-001–082, AWBC-006–024 |
| preparation success/every catalog availability failure/atomicity | PREP-001–028 |
| join once and coordinate separation | JOIN-001–032 |
| core/host visibility and dependency proof | DEP-001–008, VIS-001–007 |
| typed progress/result/error through Need | NEED-001–018 |
| save/replay/reload matrix | SAVE-001–010, REPLAY-001–018, RELOAD-001–015 |
| Cargo metadata | DEP-001–008, T2-011 |
| Tier 2 native/Web/headless/Agent | T2-001–012 |

## 4. Affected accepted AW-AH-009.4.1.2 rows

The lower contract remains normative except where the following rows require
an ownership or carrier correction. No lower identity/catalog/provider/protocol
semantics are redesigned.

| Accepted row(s) | Reconciliation | Design | Tests |
|---|---|---|---|
| SRC-006 | effect remains `tts.synthesize`; operation string is not runtime dispatch | Final C5/C9 | SRC-006, HOST-004 |
| SRC-010 | no compatibility spelling survives | Final C5 | SRC-009–011 |
| SRC-012–014 | three ordinary functions bind to shared typed callable IDs and one nominal intent | Final C5 | SRC-001–005, SRC-013 |
| ID-016–018 | fingerprint unchanged; core-owned `TaskKey::for_tts` replaced by selected-request inherent key text + generic `TaskKey::try_new` | Final C6–C7 | JOIN-001–032, PREP-027 |
| ADP-005 | progress remains exact domain progress; bridge carries it nominally | Final C10; Execution §6 | NEED-001–005 |
| ADP-007–008 | cancellation remains typed TTS cancellation; joined observer semantics are now exact | Final C8 | CANCEL-001–010 |
| ADP-020 | deterministic epoch/sequence ordering assigned only after preparation | Execution §§2,10 | PREP-024–025, JOIN, REPLAY |
| ADP-021 | one typed registration path replaces string operation ownership | Final C9 | HOST-001–005, STR-004 |
| RUN-001 | exact key retained after selected request; owner corrected out of core | Final C6–C7 | JOIN, PREP-027 |
| RUN-001A | preparation is before registry/dispatch/pin/replay/I/O | Execution §2 | PREP-001–028 |
| RUN-001B | typed failure to same observer, no host dispatch/partial registry | Execution §3 | PREP-005–026 |
| RUN-002 | one selected-key scheduler join | Final C7 | JOIN-001 |
| RUN-003 | every selected coordinate separates | Execution §4 | JOIN-002–026 |
| RUN-004 | observer-only coordinates excluded from key | Execution §4 | JOIN-027–032 |
| RUN-005 | task/host/replay failure carrier is typed RuntimePayload | Final C10; Save §3 | NEED-007–018, REPLAY-002 |
| RUN-006 | recorded complete success injects exact asset | Save §5 | REPLAY-001, T2-006 |
| RUN-007 | omitted bytes fail; no provider fallback | Save §§3,5 | REPLAY-005–006 |
| RUN-008 | failure replay injects exact typed TtsError | Save §5 | REPLAY-002, T2-007 |
| RUN-009–010 | existing HostTasks/TaskGenerationPins blockers only | Save §2 | SAVE-001–004, STR-003 |
| RUN-011–012 | deterministic ordering and replay matching use selected task identity | Execution §10; Save §§3–5 | REPLAY-007–011 |
| RUN-013 | active work completes under old generation pin | Save §8 | RELOAD-014 |
| RUN-014 | queued migration uses exact compatibility tuple | Save §7 | RELOAD-001–011 |
| RUN-015 | incompatible queued work produces CatalogChanged/no dispatch | Save §7 | RELOAD-002–012 |
| RUN-016 | candidate preparation/pin swap is atomic | Save §7 | RELOAD-015 |
| RUN-017–018 | save/result limits and typed durability remain accepted | Layout §7; Save §2 | SAVE-005–010, LAY-076–077 |
| RUN-019 | exact redacted debug label retained | Execution §11 | HOST-010, DIAG-009 |
| STR-001 | accepted lower crate remains Sans-I/O; bridge is separate | Ownership | DEP-004/006 |
| STR-004 | no source-specific scheduler path | Final C7 | STR-001 |
| STR-004A | stage visibility: intent cannot reach host; selected request only | Final C9; Ownership §5 | VIS-002–005, HOST-003 |
| STR-007 | behavior is inherent on owners; no extension trait | Final C3/C6; Handoff §2 | STR-007 |
| STR-008 | no old string branch/alias/legacy reader | Handoff §2 | SRC-009–011, HOST-004–005, T2-012 |
| STR-009–012 | payload/AWBC/host/replay structural validation | Layout; Execution; Save | AWBC, LAY, HOST, REPLAY |
| STR-013–015 | workspace/Tier 2/dependency closure | Handoff §5; Matrix | DEP, T2 |

All other accepted AW-AH-009.4.1.2 rows are inherited unchanged and remain
required in addition to this correction matrix.

## 5. Constraints and non-goals

| Constraint | Enforcement |
|---|---|
| no lower identity/catalog/fingerprint/protocol redesign | Final §1; affected-row table |
| no core audio dependency | Ownership; DEP-001 |
| no string/JSON envelope | Layout §§6/8; AWBC-025; STR-005 |
| no parallel scheduler/log | Final C7; STR-001–003 |
| no unprepared host/replay state | Execution §§2/5; REPLAY transaction |
| no compatibility/V2 | AWBC-002; VIS-007; T2-012 |
| no source-text gate | Matrix execution rules; structural evidence owners |
