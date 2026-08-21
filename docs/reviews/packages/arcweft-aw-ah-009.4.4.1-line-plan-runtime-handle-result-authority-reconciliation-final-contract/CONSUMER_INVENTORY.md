# Required consumer inventory

Legend:

- **observed** — directly inspected at fixed SHA during this design pass;
- **parent** — maintained contract/source used as precedence;
- **change** — production consumer that must change in the atomic cut;
- **delete** — old string/fallback consumer to remove;
- **test** — required proof consumer.

## 1. Policy, overview, and maintained contracts

| Status | Path / owner | Required coverage |
|---|---|---|
| observed | `AGENTS.md` | spec-first, deterministic, bounded, typed errors, no speculative runtime semantics |
| observed | `crates/AGENTS.md` | crate layering, Sans-I/O lower crates, compile/test discipline |
| observed | `docs/implementation/AGENTS.md` | implementation evidence and current-source precedence |
| observed | `README.md` | Arcweft project direction |
| observed | `docs/00-overview/crate-map.md` | owner/layer inventory |
| parent | `docs/reviews/packages/arcweft-aw-ah-009.4-character-dialogue-first-class-runtime-final-contract/FINAL_CONTRACT.md` | CharacterDialogue domain and runtime operation rule |
| parent/test | same package `TEST_MATRIX.md` | RUN-037 and maintained behavior rows |
| parent | same package `RUNTIME_WIRE_PERSISTENCE.md` | dialogue/AWBC persistence base |
| observed | `docs/implementation/2026-08-21-dialogue-line-plan-typed-ownership.md` | exact current blocker and no-string requirement |
| observed | `docs/01-language/dialogue-line-handles-and-returns.md` | `out R`, destructuring, `_`, scoped handles |
| observed | `docs/01-language/dialogue-calls-scopes-cancellation.md` | cancellation, child, cleanup, nonlocal control |
| change | maintained runtime-core, AWBC-parity, save/replay, and presentation chapters | update final authority/timelines and remove string routes |

## 2. Syntax, HIR, sema, compiler facts

| Status | Path / owner | Required coverage |
|---|---|---|
| observed | `crates/arcweft-lang-hir/src/dialogue_application.rs` | `HirDialogueApplication`, `HirLinePlan`, source-ordered items, `Out` |
| observed | `crates/arcweft-lang-sema/src/final_analysis/analyzer/expressions.rs` | source-order inference and exact one `out` facts |
| observed | `crates/arcweft-lang-sema/src/types.rs` | replace temporary `Named` handle/capability spellings |
| observed/change | `crates/arcweft-lang-sema/src/callable/identity.rs` | keep `StageMethodId`; add dedicated line-context/schedule identities; remove capacity voice branch |
| observed/change | `crates/arcweft-lang-sema/src/callable/schema/families.rs` | exact signatures/results/effects |
| change | compiler final checked semantic fact projection | emit direct line operation facts and exact runtime types; delete StageMethod exclusion |
| observed/change | `crates/arcweft-runtime-plan/src/semantic_facts.rs` | accept exact callable/type facts without names |
| observed/change | `crates/arcweft-runtime-plan/src/final_flow.rs` | lower HIR line items to final RuntimePlan owners |

## 3. RuntimePlan and core

| Status | Path / owner | Required coverage |
|---|---|---|
| observed/change | `crates/arcweft-core/src/pattern.rs` | extend existing `RuntimeOpaqueTypeOwner`; result pattern transaction |
| observed/change | `crates/arcweft-core/src/value.rs` | exhaustive RuntimeValue traversal remains sole algebra |
| observed/change | `crates/arcweft-core/src/value/opaque.rs` | affine class/persistence and exact token payload |
| observed/change | `crates/arcweft-core/src/value/ownership.rs` | direct opaque affine ownership branch |
| observed/change | `crates/arcweft-core/src/plan.rs` | add `LineOperation`, `CommitDialogueResult`, typed Dialogue result target |
| observed/change | `crates/arcweft-core/src/plan/dialogue_content.rs` | retain group id, no detached line-plan owner |
| observed/change/delete | `crates/arcweft-core/src/line_task.rs` | extend group/reducer/live snapshot; replace Delay for at; delete `LineOutRequest` |
| observed/change/delete | `crates/arcweft-core/src/effect.rs` | delete RegisterHandle/DropHandle/Out variants |
| observed/change | `crates/arcweft-core/src/engine.rs` | activation id/phase/result/ledger/command state |
| change | `crates/arcweft-core/src/engine/flow.rs` and suspension modules | execute activation ops, publish result, unwind control |
| delete | `crates/arcweft-core/src/observation.rs` old string-effect mappings | replace with typed normalized observations |

## 4. AWBC

| Status | Path / owner | Required coverage |
|---|---|---|
| observed/change | `crates/arcweft-runtime-plan/src/awbc_lower/line.rs` | line operation/site/group lowering |
| observed/change/delete | `crates/arcweft-runtime-plan/src/awbc_lower/flow.rs` | result destination/pattern, activation function, remove Out effect path |
| change/delete | `crates/arcweft-runtime-plan/src/awbc_lower/inventory.rs` | line-operation table and removal of old effect descriptors |
| observed/change | `crates/arcweft-core/src/awbc/schema.rs` | final in-place types/opcodes/group/terminator/effect kinds |
| change | `crates/arcweft-core/src/awbc/codec.rs` and `codec/*` | one writer/reader, exact tags/grammar |
| change | `crates/arcweft-core/src/awbc/verify/structure.rs` | tables, sites, ranges, topology, limits |
| change | `crates/arcweft-core/src/awbc/verify/code.rs` | register types, operation ABI, result dataflow |
| change | `crates/arcweft-core/src/awbc/vm.rs` | typed activation/operation/result execution |
| change | `crates/arcweft-core/src/awbc/fiber.rs` | activation frame/result/ledger/suspension |
| change/delete | `crates/arcweft-core/src/awbc/product_step.rs` and `product_step/*` | common reducer, snapshots, no old effect mapping |
| change | `crates/arcweft-core/src/awbc/parity.rs` | normalized line trace comparison |
| test | `crates/arcweft-core/src/awbc/tests.rs` and runtime-plan AWBC tests | codec/tamper/VM/differential rows |

## 5. Presentation, dialogue, hosts, runtime driver

| Status | Owner | Required coverage |
|---|---|---|
| change | `arcweft-dialogue` CharacterDialogue runtime types | exact Character/voice/presentation activation facts |
| change | core/presentation command owner | typed acquire/look/release/cancel command and outcome |
| change | runtime driver | deterministic command queue, correlation, cleanup, result resume |
| observed/change | `crates/arcweft-player-native/src/lib.rs`, `windowed_runtime.rs`, `dev_capture.rs` | native command adapter and trace |
| change | native patch endpoint and live-patch fixtures | generation pinning, no active remap |
| change | Web player/host consumer | same tagged DTO and lossless ids; no label parsing |
| change | headless runtime/test host | deterministic validation/outcomes and trace |
| observed/change/delete | `crates/arcweft-runtime-host/src/bundle_runner.rs` | typed outcomes, delete old string effects |

## 6. Bundle, save, replay, hot reload, Agent, CLI

| Status | Path / owner | Required coverage |
|---|---|---|
| observed/change | `crates/arcweft-bundle/src/product.rs` | final AWBC/runtime-plan shape and version 1 |
| observed/change | `crates/arcweft-bundle/src/patch.rs` | pinned active generation behavior |
| observed/change | `crates/arcweft-bundle/src/resource_codec/runtime.rs` | exact type/producer/line schema |
| change | save/snapshot owner | activation, ledger, schedule, result and transaction validation |
| change | replay owner | typed request/outcome log and token stability |
| change | Agent observation owner | typed normalized observations only |
| observed/change/delete | `crates/arcweft-cli/src/output.rs` | render typed observations; delete old string effect arms |
| observed/change/delete | `crates/arcweft-cli/src/app/bundle.rs` | bundle/run parity and old effect deletion |
| test | CLI check/run/AWBC suites | unchanged fixture through all four paths |

## 7. Maintained fixtures and tests

| Status | Path | Required coverage |
|---|---|---|
| observed/test | `tests/fixtures/arcw/spec_should_pass/run/011_dialogue_line_value_and_handle_discard.arcw` | unchanged RUN-037 primary fixture |
| observed/test | `tests/fixtures/arcw/current_pass/check/011_dialogue_with_plan.arcw` | mark-triggered simpler plan |
| test | current line-task reducer and cancellation tests | preserve reducer topology while adding schedule capture/ledger authority |
| test | current Dialogue/AWBC parity tests | add exact result and host-command ordering |
| test | current save/replay/hot-reload tests | transactional identity/result/handle rows |
| delete | obsolete string RegisterHandle/DropHandle/Out tests | compile-fail old API; no compatibility expectations |
| delete | any fixture edge exception or allowlist | primary fixture must use ordinary pipeline |
