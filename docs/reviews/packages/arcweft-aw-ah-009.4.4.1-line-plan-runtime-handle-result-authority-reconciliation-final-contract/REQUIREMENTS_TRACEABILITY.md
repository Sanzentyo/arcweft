# Required decision and output traceability

Every request decision is closed below.  Test references are mandatory rows in
`TEST_MATRIX.md`.

| Req | Exact closure | Normative owner/API | Principal documents | Test rows |
|---:|---|---|---|---|
| 1 | classify StageApi/line context non-values, look exact entity value, three affine exact opaque handles; define producer/payload/equality/ownership/nesting/persistence/validation | `TypeKind`, `RuntimeOpaqueTypeOwner`, `RuntimeOpaqueValue`, `RuntimeValue::ownership` | `RUST_OWNERS_AND_APIS.md` §§1–3; `FINAL_CONTRACT.md` D1 | SRC-005–010, POS-005–010, NEG-006–010, API-011–012 |
| 2 | deterministic activation/site/issuance identities; replay/save/hot replacement/stale generation | `DialogueActivationId`, `RuntimeLineHandleToken`, ledger counters | `IDENTITY_LIFETIME_AND_FAILURE.md` §§1–2; `SAVE_REPLAY_HOT_REPLACEMENT.md` | ID-001–016, SAVE-006–010 |
| 3 | exact line scope, `_`, drop, completion/cancel/fail/nonlocal/join/detach rules | `RuntimeHandleOwnerSlot`, `RuntimeLineHandleLedger`, unwind order | `IDENTITY_LIFETIME_AND_FAILURE.md` §§3–7 | POS-010–012, TIME-004–022, NEG-027–029 |
| 4 | sole final RuntimePlan owner, source-order setup/children/locals/captures/cleanup/result, no second tree | existing `LineTaskGroup` with activation ops/result/sites | `RUNTIME_PLAN_AND_ADMISSION.md` §§1–4 | SRC-001–004, SRC-003, POS-013, NEG-015–020 |
| 5 | sole dialogue result target/pattern; hidden typed commit and successful close publication boundary | `FlowOp::CommitDialogueResult`, `RuntimeDialogueResultTarget`, `DialogueResultState` | `COMMAND_AND_RESULT_TIMELINES.md` §§1,6–7 | POS-009–010, TIME-012–016, NEG-021–026, AWBC-016–022 |
| 6 | real typed at schedule including eval order/captures/handle/deadline/cancel/join/failure | `RuntimeLineOperation::Schedule`, Scheduled trigger/live state | `RUNTIME_PLAN_AND_ADMISSION.md` §§2–4; `COMMAND_AND_RESULT_TIMELINES.md` §3 | POS-013–016, TIME-001–008, NEG-011–020 |
| 7 | typed Sans-I/O actor.look command with exact identity/ownership/request/outcome/host behavior | `RuntimeStageCommand::SetCharacterLook` | `RUST_OWNERS_AND_APIS.md` §7; timeline §4; parity §5 | POS-001–007, TIME-009–010, NEG-002–005, PAR-015–016 |
| 8 | voice lifecycle Ready/Lazy/Absent/Failure/cleanup/identity | `LineContextMethodId`, `RuntimeDialogueVoiceState`, `RuntimeVoiceLease` | `RUST_OWNERS_AND_APIS.md` §§1,8; timeline §5 | POS-008, POS-017–018, TIME-011, NEG-001, ID-005 |
| 9 | delete all string RegisterHandle/DropHandle/LineOut routes and prove inaccessible | original effect/AWBC enums and consumers | `DELETION_MATRIX.md` DEL-07–19; `IMPLEMENTATION_INTERLEAVE.md` I6 | API-001–010, AWBC-005–007, E2E-003–004 |
| 10 | plan construction/admission/verifier for R, locals/captures/producers/activation/schedule/commit/cleanup | `RuntimePlanBuilder`, group admission, structured errors | `RUNTIME_PLAN_AND_ADMISSION.md` §§5–8 | NEG-015–026, NEG-030–041, AWBC-008–025 |
| 11 | in-place AWBC schema/codec/verifier/VM/reducer/suspension/snapshot, all versions 1, one reader | `AwbcProgram`, opcodes 0x1e/0x20/0x86, `AwbcLineTaskGroup` | `AWBC_SCHEMA_CODEC_VM.md` | AWBC-001–028, SAVE-001, API-014–016 |
| 12 | structured/AWBC exact host/observation/result/status/diagnostic parity | common ledger/reducer/trace | `STRUCTURED_AWBC_PARITY.md` | PAR-001–018, POS-001–004 |
| 13 | bundle/save/replay/hot replacement for active schedules and result states; mismatch precedence | snapshot V1, replay V1, generation pin | `SAVE_REPLAY_HOT_REPLACEMENT.md` | ID-003–016, SAVE-001–020, E2E-002,008–009 |
| 14 | compile-clean interleave and exact deletion matrix for Named/exclusions/string carriers/fixtures/tests | original enums/impls, no shim | `IMPLEMENTATION_INTERLEAVE.md`; `DELETION_MATRIX.md` | SRC-009–010, API-001–016, E2E-010 |
| 15 | bounded items/locals/handles/callbacks/captures/nodes/cleanup/result/commands/restore | shared constants and deterministic budgets | `BOUNDED_WORK.md` | NEG-030–042, AWBC-028, SAVE-002–004 |

## Required consumer inventory trace

The mandatory maintained chapters, parent AW-AH-009.4/RUN-037, HIR/sema,
callable identities, runtime semantic facts, RuntimePlan, core value/flow/
dialogue/reducer/effects, AWBC, presentation/hosts, bundle/save/replay/hot
reload, Agent/CLI, and both fixtures are enumerated in
`CONSUMER_INVENTORY.md`.

## Required output trace

| Expected archive content | File |
|---|---|
| `OPEN_QUESTIONS.md` exactly `none` | `OPEN_QUESTIONS.md` |
| exact Rust-shaped owners/APIs | `RUST_OWNERS_AND_APIS.md` |
| RuntimePlan schema/admission | `RUNTIME_PLAN_AND_ADMISSION.md` |
| AWBC schema/codec/verifier/VM/snapshot | `AWBC_SCHEMA_CODEC_VM.md` |
| save/replay/hot replacement schema | `SAVE_REPLAY_HOT_REPLACEMENT.md` |
| command/result phase timelines | `COMMAND_AND_RESULT_TIMELINES.md` |
| identity/lifetime/failure tables | `IDENTITY_LIFETIME_AND_FAILURE.md` |
| structured/AWBC parity | `STRUCTURED_AWBC_PARITY.md` |
| deletion matrix | `DELETION_MATRIX.md` |
| compile-clean implementation interleave | `IMPLEMENTATION_INTERLEAVE.md` |
| bounded limits/accounting | `BOUNDED_WORK.md` |
| complete positive/negative/tamper/parity matrix | `TEST_MATRIX.md` |
| baseline/evidence and actual verification breadth | `VERIFICATION.md` |
| no production overlay | archive contains Markdown only plus manifest |
