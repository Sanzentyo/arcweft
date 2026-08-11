# Amended Source and provisional-Stream deletion inventory

Deletion means physical product-code removal, not hiding behind a feature/source gate,
deprecation, alias, compatibility module, or legacy decoder. Paths are current-main
owners or owning integration sites; typed compilation and codec behavior, not source-text
search, prove completion.

## Delete or replace

| ID | Current owner/path | Exact obsolete target | Action | Sole replacement | Acceptance evidence |
| --- | --- | --- | --- | --- | --- |
| SRC-DEL-001 | crates/arcweft-core/src/source.rs | SourcePlan, SourceHandlerPlan, SourceOp, SourceRuntimeState, SourceId, SourcePolicy, SourceEvent/Kind and normalization | Delete product module/types after corrected tests exist | RuntimeStreamDefinition, StreamInstanceTable, RuntimeStreamEvent and existing generic CFG/effect owners | STR-DEL-001..005, STR-ABI old-tag tests |
| SRC-DEL-002 | crates/arcweft-core/src/lib.rs and public reexports | `pub mod source` / Source reexports | Remove | No compatibility module or alias | External compile-fail fixtures |
| SRC-DEL-003 | crates/arcweft-core/src/engine/source.rs | Source event application/queue/close path | Delete | arcweft-core::engine::stream staged sole-table transitions | STR-OWN, STR-EXH, STR-DROP |
| SRC-DEL-004 | crates/arcweft-runtime-plan/src/source.rs | Runtime Source lowering/validation | Delete | Corrected RuntimePlan Stream definition/origin/policy lowering | STR-PLAN, STR-CALL |
| SRC-DEL-005 | crates/arcweft-core/src/plan.rs and RuntimePlan constructors | `sources`/`source_plans` fields and Source imports | Remove | `effect_sets`, `stream_definitions` | RuntimePlan typed construction/round trip |
| SRC-DEL-006 | crates/arcweft-core/src/step.rs | `source_events`, emitted `source_events`, old emitted `stream_events`, `source_close`; Source statistics | Remove/replace in place | `stream_events`, `stream_event_outcomes`, `stream_observations`, `stream_requests`, nested Stream stats | STR-JSON + host parity |
| STR-DEL-007 | crates/arcweft-core/src/stream.rs | StreamRuntimeId/StreamPlan/old StreamRuntimeState/old StreamEvent using SourceEventKind | Replace owning implementations, not parallel helpers | Exact types in RUST_SCHEMAS.md | STR-OWN/RPL/EXH/DROP |
| STR-DEL-008 | crates/arcweft-core/src/engine/stream.rs | Old transform/queue close behavior and unchecked counters | Replace in place | Staged sole-table lifecycle with checked arithmetic | STR-EXH atomicity; terminal queue tests |
| AWBC-DEL-001 | crates/arcweft-core/src/awbc/schema.rs | AwbcSourcePlanId, AwbcSourcePlan, source_plans; old AwbcStreamPlan/table/IDs | Delete/replace in codec-8 schema | AwbcStreamDefinitionId/table only | STR-ABI table order and OOB tests |
| AWBC-DEL-002 | crates/arcweft-core/src/awbc/schema.rs | Function kinds StreamTransform/SourceOpen/SourceHandler | Delete; tags 3/4/5 unknown | Ordinary=8, GeneratorProducer=9 | Removed-tag decoder tests |
| AWBC-DEL-003 | crates/arcweft-core/src/awbc/schema.rs + codec/code.rs | StreamYield 0x1c, StreamClose 0x1d, SourceClose 0x1e, SourceYield 0x20 | Delete from codec8 reader/writer/VM | OpenStream 0x27, FinishStream 0x28, NextStream 0x8f, YieldStream 0x90 | Golden bytes and unknown-tag tests |
| AWBC-DEL-004 | crates/arcweft-core/src/awbc/codec/runtime.rs | Old Stream/Source table record read/write | Delete | One AwbcStreamDefinition record codec | Codec8 canonical round trip |
| AWBC-DEL-005 | crates/arcweft-core/src/awbc/codec.rs / metadata.rs | Codec7 program order including two tables | Replace atomically | Codec8 31-table order | Full table golden fixture |
| AWBC-DEL-006 | crates/arcweft-core/src/awbc/verify/* | Old Source/Stream bounds/opcode/function-kind validation | Delete/replace | Corrected definition/signature/effect/affine/safe-point validation | Tamper matrix |
| AWBC-DEL-007 | crates/arcweft-core/src/awbc/fiber.rs | FiberSourceState/sources and old FiberStreamState/streams | Delete | Fiber id, producer_stream reference; handle stays in ordinary runtime values | Restore affine/producer tests |
| AWBC-DEL-008 | crates/arcweft-core/src/awbc/product_step/snapshot.rs | `stream_sequences`; rebuild_facade_source_states_from_compact; rebuild_facade_stream_states_from_compact | Delete | One StreamInstanceTableSnapshot and candidate restore | Snapshot/restore tests |
| AWBC-DEL-009 | crates/arcweft-core/src/awbc/product_step.rs, vm.rs and compiled-region exchange | Old Source/Stream state synchronization/dispatch | Replace in existing owners | Sole table + shared FiberState refs | VM/compiled parity |
| PLAN-DEL-001 | crates/arcweft-runtime-plan AWBC lowering | Old Source/Stream table/opcode projection or any provisional Lang-01.1.1 projection | Delete; do not translate | Corrected codec8 direct lowering | No old writer; combined ABI tests |
| BND-DEL-001 | crates/arcweft-bundle/src/lib.rs | BundleAwbcEncoding::AwbcV1; runtime summary stream_plans/source_plans | Replace with schema6-only shape | AwbcV2 and stream_definitions | Bundle old-version rejection/golden bytes |
| BND-DEL-002 | crates/arcweft-bundle/src/product_awbc.rs | Codec7/ABI1 product wrapper path | Replace; no legacy dispatch | Codec8/ABI2 wrapper only | STR-ABI old-format rejection |
| SAVE-DEL-001 | crates/arcweft-runtime-driver/src/session_save.rs | Schema1 Stream/Source/compact assumptions | Replace in schema2 cut | Sole table, blockers, replay/tombstones, pins | STR-SAVE |
| SAVE-DEL-002 | crates/arcweft-runtime-driver/src/session/persistence.rs + arcweft-save registration | Schema1 migration/legacy lookup if any would be considered | Do not register; direct reject | Schema2 exact decoder only | Old schema rejection |
| HOST-DEL-001 | native/web/Agent adapters and fixtures | Source event/close DTOs or old Stream endpoint schemas | Delete/replace atomically | Pass shared core RuntimeStream* bytes/types directly | Cross-host parity fixtures |
| TEST-DEL-001 | Old Source/Stream direct tests | Tests asserting obsolete public/wire shapes | Delete only after invariant has a corrected stable test ID | TEST_MATRIX.md coverage | Cut 9 review checklist |
| LANG-DEL-001 | Any unlanded Lang-01.1.1 design branch/docs-generated product code | Provisional StreamPlan/handle/state/event/opcode/table/writer/save shape | Never land; no translator | Corrected contract only | Codec/public compile-fail rejection |

## Explicitly preserve

| ID | Substrate | Rule |
| --- | --- | --- |
| KEEP-001 | arcweft-source crate, SourceDocumentId, source documents/anchors/ranges | Preserve unchanged; “Source” here means source code evidence, not deleted runtime Source |
| KEEP-002 | AwbcSourceMapId, AWBC source_map and display_map debug evidence | Preserve; remain excluded from semantic/executable identity as currently accepted |
| KEEP-003 | Shared callable catalog/resolver and CallableParameterSource spans | Preserve; extend only the owning RuntimePlan/AWBC projection |
| KEEP-004 | Accepted-HIR lifecycle, definition-source index, external binding publication, query budgets | Preserve unless an independently demonstrated concrete defect appears |
| KEEP-005 | Ordinary-function direct call/suspension, typed await, CFG, frame/resume/safe-point substrate | Preserve and consume; do not duplicate under Stream-local names |
| KEEP-006 | Existing non-Stream RuntimePlan/AWBC tables, verifier budgets, FiberState exchange | Preserve relative order/owners except the narrow fields explicitly changed |
| KEEP-007 | Proof/concurrency, presentation/style/environment/view/rich-text/character/Need/task behavior | Out of scope; no redesign |
| KEEP-008 | CSS and Takumi dependencies/paths | Must not be touched or introduced |

## Completion rule

A deletion row closes only after its useful invariant is represented by one or more stable
`STR-*` tests. Cut 9 then proves public unreachability, old-wire rejection, snapshot
single ownership, Cargo dependency direction, and the structured audit. No raw grep result
is acceptance evidence.
