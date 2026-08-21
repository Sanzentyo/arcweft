# Complete positive, negative, tamper, parity, persistence, and API matrix

Every row is mandatory.  “Both” means the same scenario is run through the
structured executor and AWBC VM against the same scripted typed host, with
normalized traces compared byte-for-byte.  Tests must not use source-name
recognizers, manually constructed string handles/results, fixture allowlists,
or unverified RuntimePlan/AWBC builders.

## A. Source, check, and maintained fixtures

| ID | Path/layer | Scenario | Required assertion |
|---|---|---|---|
| SRC-001 | `tests/fixtures/arcw/spec_should_pass/run/011_dialogue_line_value_and_handle_discard.arcw` | unchanged source, ordinary `check` pipeline | succeeds; inferred line result is `(VoiceHandle, CueHandle)`; outer pattern is exact tuple discard/bind |
| SRC-002 | same | inspect accepted final HIR facts | source item order is acquire → schedule → voice → out; callback captures actor exactly once |
| SRC-003 | same | inspect accepted RuntimePlan | one `LineTaskGroup`, four handle sites including child look cue, one dialogue target, no detached line-plan tree |
| SRC-004 | `tests/fixtures/arcw/current_pass/check/011_dialogue_with_plan.arcw` | mark-triggered plan through check | succeeds unchanged; mark child remains existing reducer topology; sema fixes the absent authored output to Unit and lowering synthesizes one exact Unit commit |
| SRC-005 | sema callable tests | `alice.stage.acquire(scope=line)` | resolves `StageMethodId::Acquire`; result direct exact StageActor handle type |
| SRC-006 | sema callable tests | `actor.look(.worried, crossfade=120ms)` | resolves `StageMethodId::Look`; actor/look Character precision agrees; result direct CueHandle |
| SRC-007 | sema callable tests | `at(0.42s): callback` | resolves dedicated scheduling callable identity, never pure intrinsic/ordinary wait |
| SRC-008 | sema callable tests | `line.voice_handle()` | resolves `LineContextMethodId::VoiceHandle`, not `CapacityMethodId` |
| SRC-009 | runtime semantic facts | final handle/capability types | capabilities are non-values; handles project to accepted opaque owners; look projects to exact entity ref |
| SRC-010 | negative check | `try` to capture/return StageApi or LineContext as value | structured non-value capability diagnostic; no dynamic projection |

## B. Positive structured/AWBC/CLI execution

| ID | Mode | Scenario | Required assertion |
|---|---|---|---|
| POS-001 | structured | RUN-037 primary fixture, advance after 0.42s | returns `"done"`; exact actor/cue/voice values; look command occurs; result cue binds; voice `_` drops |
| POS-002 | AWBC | same scripted host/input | same return, trace, final env, handle states, diagnostics, command sequence as POS-001 |
| POS-003 | CLI structured | same fixture through normal CLI run | success exit; expected stdout; empty/unmodified error class; no edge skip |
| POS-004 | CLI AWBC | same fixture through product/AWBC run | same exit/stdout/stderr classification as POS-003 |
| POS-005 | Both | exact StageActor value | `RuntimeValue::Opaque`; producer `std.line.stage_actor_handle`; affine; snapshot-only; exact Character owner |
| POS-006 | Both | exact scheduled CueHandle | producer `std.line.cue_handle`; token site/issuance stable; pending then completed/cancelled state correct |
| POS-007 | Both | exact look CueHandle from unbound expression statement | remains child/line-owned until cleanup; command is not immediately cancelled merely because expression result is unbound |
| POS-008 | Both | exact VoiceHandle | producer `std.line.voice_handle`; lease references active session; outer `_` releases at publication |
| POS-009 | Both | `out (voice, cue)` | hidden committed RuntimeValue tuple has exact type and affine paths before ready |
| POS-010 | Both | outer `let (_, cue)` | pattern validates atomically; voice discard and cue transfer occur only after joined close |
| POS-011 | Both | explicit `let _ = at(...)` | cue is issued, then immediate typed drop cancels pending callback; no callback execution |
| POS-012 | Both | explicit `drop(cue)` | existing drop operation consumes owner slot and ledger authority; no string effect |
| POS-013 | Both | schedule capture order with side-effect-observable pure values | delay first, captures left-to-right, issuance last |
| POS-014 | Both | zero-delay cue | result commits first; zero cue runs before `DialogueReady`; joined failure can abort activation |
| POS-015 | Both | two scheduled cues at distinct sites/same deadline | order by deadline then site then issuance then child node |
| POS-016 | Both | loop executes one schedule site three times | tokens issuance 0/1/2; three exact capture sets; stable execution order |
| POS-017 | Both | two `voice_handle()` calls at one site | distinct affine lease tokens, same voice session, final stop/release follows last-lease policy |
| POS-018 | Both | lazy voice start | activation suspends on typed start request, resumes, commits result, then ready |
| POS-019 | Both | dialogue with no line plan | exact Unit result path; no dynamic slot; parent continuation resumes normally |
| POS-020 | Both | simple mark fixture mark fires | existing mark reducer child executes once; cleanup/result Unit parity |

## C. Cue timing, cancellation, cleanup, and nonlocal control

| ID | Mode | Scenario | Required assertion |
|---|---|---|---|
| TIME-001 | Both | advance at 0.419999999s | 0.42s cue is future and cancelled during close; no look command |
| TIME-002 | Both | advance exactly at 0.42s | cue callback and look command complete/fail before advance is applied |
| TIME-003 | Both | advance after 0.42s | cue runs once before advance; no duplicate fire during close |
| TIME-004 | Both | drop pending schedule handle before deadline | child transitions Pending→Cancelling→Cancelled; joined cancellation completes before drop returns |
| TIME-005 | Both | drop running scheduled handle | node's `CancelAndJoin` policy is honored; cleanup order stable |
| TIME-006 | Both | drop completed schedule handle | token releases only; callback not repeated and no cancellation command |
| TIME-007 | Both | callback returns normally with unbound look cue | look cue remains scope-owned and is cleaned in canonical order |
| TIME-008 | Both | callback structured failure | dialogue primary failure; siblings cancel/join; failed cleanup; committed result abandoned; parent unbound |
| TIME-009 | Both | host rejects actor acquire during activation | no ready; activation abort cleanup; no result publish |
| TIME-010 | Both | host rejects look command after ready | callback failure wins; cleanup errors secondary; parent unbound |
| TIME-011 | Both | host rejects lazy voice start | activation fails with exact voice diagnostic; no fake VoiceHandle |
| TIME-012 | Both | ordinary cancellation before result commit | no result; activation/host resources roll back; parent unbound |
| TIME-013 | Both | non-completing cancellation after commit | result affine leaves dropped; no parent pattern; cancelled cleanup once |
| TIME-014 | Both | admitted completing cancellation result path | exactly one R commit on selected path; joined close then one publish |
| TIME-015 | Both | parent return while dialogue suspended | close/join/cleanup/abandon before return continuation; no leaked handles |
| TIME-016 | Both | parent goto while dialogue suspended | same unwind guarantee before target transfer |
| TIME-017 | Both | normal completion with future cues | future cues cancel first; joined work terminal; cleanup; result publish |
| TIME-018 | Both | `Finish` child on cancellation | child allowed to finish but dialogue does not publish until terminal |
| TIME-019 | admission | detached child captures StageActor/Cue/Voice or line context | rejected before execution |
| TIME-020 | Both | detached child with unrestricted captures only | detach succeeds; no result/ledger authority; active line can close |
| TIME-021 | Both | cleanup itself fails after callback failure | callback failure remains primary; cleanup failure ordered secondary |
| TIME-022 | Both | repeated close/cancel request | idempotent state transition; no second cleanup, drop, or publication |

## D. Identity, save, replay, and hot replacement

| ID | Mode | Scenario | Required assertion |
|---|---|---|---|
| ID-001 | Both | revisit same dialogue site twice | distinct activation occurrence ids; site issuance restarts at zero |
| ID-002 | Both | same-site repeated handle creation | distinct issuance ordinals and inequality despite same host resource |
| ID-003 | replay | record and replay primary fixture | exact activation ids, handle tokens, deadlines, commands, result digest, trace bytes |
| ID-004 | save/restore | save with pending 0.42s cue | token/deadline/captures/counter restored; cue fires once at same logical time |
| ID-005 | save/restore | save while lazy voice start pending | exact activation frame/request id restored; outcome correlated once |
| ID-006 | save/restore | save after committed result but before ready | result/affine owner paths restored; zero-cue phase then ready |
| ID-007 | save/restore | save active with committed result | parent remains unbound until restored line closes |
| ID-008 | save/restore | save during closing/joined cancellation | reducer outstanding work and cleanup-started bit restore without duplication |
| ID-009 | save/restore | save immediately before publication safe point | one publication after restore; no partial pattern state |
| ID-010 | hot replace | install new generation while old dialogue active | old activation/continuation remains pinned; new activations use new fingerprint |
| ID-011 | hot replace | try explicit in-place replacement of active dialogue | transaction rejected `ActiveDialogueGenerationPinned`; no mutation |
| ID-012 | hot replace | old committed result after new generation install | old pattern/type publishes under old continuation; new plan does not reinterpret |
| ID-013 | restore | old generation retained | old activation restores successfully and remains pinned |
| ID-014 | restore | old generation unavailable | deterministic generation error before activation/host mutation |
| ID-015 | host | delayed stale outcome from previous occurrence | rejected by activation/command correlation; cannot affect current line |
| ID-016 | replay | host rejection recorded | same token is issued and enters Failed in replay; ordinal not reused |

## E. Structured runtime negative/type/authority/limit tests

| ID | Scenario | Required diagnostic and non-mutation proof |
|---|---|---|
| NEG-001 | active dialogue has `Absent` voice and calls `voice_handle` | `MissingActiveVoice`; no handle issued; no result/ready |
| NEG-002 | look value belongs to another Character | `WrongLookOwner`; no host command/cue issuance |
| NEG-003 | actor handle exact Character differs from operation Character | `WrongActorCharacter`; no host command |
| NEG-004 | actor token belongs to another dialogue activation | `WrongActivation`; both ledgers unchanged |
| NEG-005 | actor token carries stale artifact generation | `StaleGeneration`; no host dispatch |
| NEG-006 | CueHandle supplied where StageActor expected | wrong checked type/producer at precedence position; no payload-label fallback |
| NEG-007 | fabricated opaque payload has correct producer but malformed token record | token payload error; no ledger lookup mutation |
| NEG-008 | opaque value class says Plain for a line handle producer | exact owner mismatch |
| NEG-009 | opaque persistence says ConstantAndSnapshot for a line handle | exact owner mismatch |
| NEG-010 | producer-wide StageActor used for `look` without exact narrowing | compile/admission exact-actor error |
| NEG-011 | negative schedule duration | `NegativeCueDelay`; captures and issuance not evaluated/consumed after delay failure |
| NEG-012 | delay conversion overflow | `CueDeadlineOverflow`; no cue |
| NEG-013 | elapsed + delay overflow | `CueDeadlineOverflow`; no cue |
| NEG-014 | scheduled capture expression failure | no token/counter/child insertion; prior successful ops remain valid for activation rollback |
| NEG-015 | callback capture type mismatches child signature | RuntimePlan/AWBC admission error |
| NEG-016 | duplicate handle site id | `DuplicateLineHandleSite` |
| NEG-017 | site operation kind mismatch | structured site-kind error |
| NEG-018 | schedule site points to wrong child | schedule-child topology error |
| NEG-019 | two schedule sites point to one scheduled child | one-to-one topology error |
| NEG-020 | child trigger points back to wrong site | topology error |
| NEG-021 | `CommitDialogueResult` missing from a completing path | `MissingDialogueResult` |
| NEG-022 | two commits on one completing path | `DuplicateDialogueResult` |
| NEG-023 | commit expression type differs from R | result type error; cell remains uncommitted |
| NEG-024 | parent result target type differs from group R | RuntimePlan admission error |
| NEG-025 | malformed tuple result pattern for R | pattern admission or publication error before binding |
| NEG-026 | ordinary scheduled child contains result commit | result authority error |
| NEG-027 | explicit drop twice | second operation is use-after-move/double-drop; no second host command |
| NEG-028 | use handle after transfer to result | owner-slot error; no duplicate authority |
| NEG-029 | copy tuple containing handle | affine copy rejected by existing ownership traversal |
| NEG-030 | handle embedded beyond result depth 64 | result limit error |
| NEG-031 | result exceeds 4096 nodes | result limit error |
| NEG-032 | result exceeds 256 KiB canonical bytes | result limit error |
| NEG-033 | group captures exceed 64 | construction limit error |
| NEG-034 | callback captures exceed 32 | construction limit error |
| NEG-035 | total capture values exceed 256 or 1 MiB | construction/runtime limit error |
| NEG-036 | handle sites exceed 128 | construction limit error |
| NEG-037 | live handles exceed 256 | runtime limit error without partial issue |
| NEG-038 | scheduled callbacks exceed 128 | runtime limit error without partial arm |
| NEG-039 | task nodes exceed 512 | construction/AWBC verify limit error |
| NEG-040 | cleanup actions exceed 128 | construction/AWBC verify limit error |
| NEG-041 | host queue exceeds 256 | typed backpressure error with atomic operation rollback |
| NEG-042 | issuance reaches `u32::MAX` then another issue | `HandleIssuanceOverflow`; no reused token |

## F. AWBC schema, codec, verifier, VM, and tamper

| ID | Scenario | Required assertion |
|---|---|---|
| AWBC-001 | canonical encode/decode of final primary program | roundtrip equality and stable bytes; ABI=1, codec=1 |
| AWBC-002 | explicit opcode map | `ExecuteLineOperation=0x1e`, `CommitDialogueResult=0x20`, `Dialogue=0x86` |
| AWBC-003 | explicit function-kind map | `LineActivation=6`, existing tags unchanged |
| AWBC-004 | explicit line-operation tags | acquire=0, schedule=1, look=2, voice=3 |
| AWBC-005 | explicit final effect-kind tags | table in AWBC contract; old Register/Drop/Out tags rejected |
| AWBC-006 | old shorter Dialogue `0x86` payload | decoder/verifier rejects; no legacy interpretation |
| AWBC-007 | old `DelayNanos` trigger bytes | decoder rejects; no default schedule site |
| AWBC-008 | affine SnapshotOnly opaque in constant table | verifier rejects |
| AWBC-009 | line capability encoded as runtime type/constant | verifier rejects |
| AWBC-010 | ExecuteLineOperation id out of bounds | structure error |
| AWBC-011 | destination register wrong type | code verifier error |
| AWBC-012 | acquire with non-empty args | fixed ABI argument-count error |
| AWBC-013 | schedule missing delay | fixed ABI error |
| AWBC-014 | schedule capture count/type mismatch | verifier error |
| AWBC-015 | look actor/look Character mismatch | verifier semantic error |
| AWBC-016 | commit in ordinary child function | result authority error |
| AWBC-017 | commit source wrong type | verifier error |
| AWBC-018 | activation CFG missing commit | dataflow error |
| AWBC-019 | activation CFG duplicate commit | dataflow error |
| AWBC-020 | instruction after commit on same path | verifier unreachable/result error |
| AWBC-021 | Dialogue target R differs group R | verifier error |
| AWBC-022 | Dialogue pattern/destination frame mismatch | verifier error |
| AWBC-023 | scheduled site/child cross-link tamper | topology error |
| AWBC-024 | line group node cycle/range tamper | structure error |
| AWBC-025 | activation resume layout tamper | verifier error before VM |
| AWBC-026 | VM lazy voice suspension snapshot | exact frame/result/ledger/request roundtrip |
| AWBC-027 | VM stage look rejection | same primary/secondary diagnostic sequence as structured |
| AWBC-028 | VM budget yield at transition 4096 | same cursor and next-step trace as structured |

## G. Save/restore tamper and transaction tests

| ID | Tamper | Required precedence and transaction property |
|---|---|---|
| SAVE-001 | snapshot version not 1 | schema error; no host/engine mutation |
| SAVE-002 | active dialogues >64 | limit error before value/type validation |
| SAVE-003 | total handles >4096 | limit error |
| SAVE-004 | restore work >1,000,000 units | limit error before host preflight |
| SAVE-005 | result checked type tampered | Type category wins before producer/generation |
| SAVE-006 | handle producer tampered with otherwise wrong generation | Producer category wins before Generation |
| SAVE-007 | generation tampered with otherwise wrong activation | Generation wins before Activation |
| SAVE-008 | activation token tampered | Activation category; no host preflight |
| SAVE-009 | duplicate token in ledger | topology category |
| SAVE-010 | issuance counter below existing max token | topology category |
| SAVE-011 | owner path says local but value is in result | topology/result category according to first canonical inconsistency |
| SAVE-012 | scheduled capture type tampered | Type category |
| SAVE-013 | scheduled child/site tampered | topology category |
| SAVE-014 | committed result value type/pattern tampered | Result category after type/producer/generation/activation pass |
| SAVE-015 | pending command id does not match lease | Command category |
| SAVE-016 | host preflight rejects one actor/voice | HostReconciliation; rollback earlier reconstructed resources; candidate not installed |
| SAVE-017 | host rollback itself fails | restore remains failed; rollback error secondary; engine candidate absent |
| SAVE-018 | `Published` result while active dialogue still present | result/topology tamper error |
| SAVE-019 | trailing/duplicate snapshot bytes | codec/schema error; no permissive reader |
| SAVE-020 | canonical save bytes structured vs AWBC at same safe point | byte-for-byte identical executor-neutral state |

## H. Behavioral differential/parity matrix

| ID | Scenario | Compared outputs |
|---|---|---|
| PAR-001 | primary fixture success | requests, outcomes, observations, result binding, status, final state, diagnostics |
| PAR-002 | cue before advance | exact trace and final ledger |
| PAR-003 | cue exactly at advance | exact cue-before-advance trace |
| PAR-004 | cue after advance | exact cancellation/cleanup trace |
| PAR-005 | zero cue | commit → cue → ready order |
| PAR-006 | multiple same-deadline cues | exact stable ordering |
| PAR-007 | callback failure | primary/secondary diagnostics and final state |
| PAR-008 | actor host rejection | no ready/publish; same cleanup |
| PAR-009 | missing voice | same activation failure |
| PAR-010 | normal cancellation | same reducer/cleanup/result abandon |
| PAR-011 | completing cancellation | same single commit/publish |
| PAR-012 | explicit `_` | same affine path order and typed drop commands |
| PAR-013 | save/restore pending cue | same canonical snapshot and resumed trace |
| PAR-014 | hot replacement while active | same pinning and new-generation behavior |
| PAR-015 | native vs headless typed command corpus | same command DTO/order and logical outcomes except declared host capability rejection |
| PAR-016 | Web DTO roundtrip | all 64-bit/32-byte ids preserved; no string parser; same outcome correlation |
| PAR-017 | Agent observation | same normalized events, no executor-local ids |
| PAR-018 | CLI structured vs AWBC | same stdout/stderr/exit class and returned value |

## I. API deletion and no-fallback proof

| ID | Proof | Required assertion |
|---|---|---|
| API-001 | compile attempt `LineEffectRequest::RegisterHandle` | symbol/variant absent |
| API-002 | compile attempt `LineEffectRequest::DropHandle` | symbol/variant absent |
| API-003 | compile attempt `LineEffectRequest::Out(...)` | symbol/variant absent |
| API-004 | compile attempt construct `LineOutRequest` | type absent |
| API-005 | compile attempt `AwbcEffectKind::RegisterHandle` | variant absent |
| API-006 | compile attempt `AwbcEffectKind::DropHandle` | variant absent |
| API-007 | compile attempt `AwbcEffectKind::Out` | variant absent |
| API-008 | production search for temporary `Named` handle spellings | none outside historical/source docs and negative fixtures |
| API-009 | callable inventory | no `voice_handle` branch in `CapacityMethodId` |
| API-010 | runtime plan lowering inventory | no pure `at`, ordinary wait, fake task, no-op stage, source/callee-name recognizer |
| API-011 | RuntimeValue inventory | no second line-handle value variant/algebra; handles only existing Opaque |
| API-012 | producer inventory | no line-specific copied producer table |
| API-013 | dialogue result inventory | no string/debug-label or dynamic/untyped result slot |
| API-014 | codec inventory | one version-1 reader; no old Dialogue/Delay/effect discriminant reader |
| API-015 | fixture harness | no RUN-037 allowlist/edge skip/source-site exception |
| API-016 | static version assertions | every involved Arcweft-owned version marker equals 1 |

## J. Bundle/CLI end-to-end closure

| ID | Scenario | Required assertion |
|---|---|---|
| E2E-001 | build/check accepted project containing primary fixture | final RuntimePlan and AWBC produced without diagnostics |
| E2E-002 | bundle encode/decode | final line operation/type/group/result schema retained; stable digest |
| E2E-003 | bundle tampered with old effect kind | rejected before execution |
| E2E-004 | bundle tampered with old Dialogue payload | rejected before execution |
| E2E-005 | native CLI structured execution | `"done"`, correct trace and cleanup |
| E2E-006 | native CLI AWBC execution | identical to E2E-005 |
| E2E-007 | headless bundle execution | same logical result/trace |
| E2E-008 | save/reload bundle while active | pinned generation/resource reconciliation and result publication correct |
| E2E-009 | install hot patch while active then finish | old line finishes on old generation; next line uses new generation |
| E2E-010 | full workspace/API proof | fmt/check/clippy/tests green; no obsolete symbol/selectable fallback |
