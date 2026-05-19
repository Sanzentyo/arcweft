# Runtime boundary and AWFT grammar refactor checklist

No compatibility aliases. No deprecated wrappers. No transitional re-export modules.

## Fixture activation note

The direction package is applied without rewriting future-spec fixtures into
today's grammar. Some files under `tests/fixtures/awft/current_pass/` currently
remain explicit implementation gaps and are skipped by the fixture harness with
file-name allowlists. Promote each fixture by removing it from the skip list
only after the parser, HIR, sema, runtime-plan, CLI, and LSP behavior actually
support the documented syntax.

## Core rename and structure

- [ ] Rename `crates/arcweft-core/src/frame.rs` to `crates/arcweft-core/src/step.rs`.
- [ ] Delete `FrameInput`, `FrameInputView`, `FrameOutput`, `FrameOutputWriter`.
- [ ] Add `RuntimeStepInput`, `RuntimeStepInputRef`, `RuntimeStepOutput`, `RuntimeStepOutputSink`.
- [ ] Add `RuntimeStepResult`, `RuntimeStepOptions`, `RuntimeStepBudget`, `RuntimeStepMode`, `RuntimeStepStopReason`.
- [ ] Replace `external_values` with `bindings` everywhere.
- [ ] Replace `FlowFiber.frames` with `FlowFiber.control_stack`.
- [ ] Delete `RuntimeFrame` and `RuntimeFrameKind`.
- [ ] Add `FlowControlStackEntry` and `FlowControlStackEntryKind`.
- [ ] Replace top-level `line_effects` with `RuntimeEffectBatch`.
- [ ] Replace top-level `task_requests` with `HostRequestBatch`.
- [ ] Add `RuntimePayload`.
- [ ] Replace string-only source/stream events with structured payload events.

## Runtime execution

- [ ] Change `Engine::step` signature to require `RuntimeStepOptions`.
- [ ] Implement `RuntimeStepMode::OneOp`.
- [ ] Implement `RuntimeStepMode::DrainUntilBlocked`.
- [ ] Implement `RuntimeStepMode::DrainUntilOutput`.
- [ ] Implement `RuntimeStepMode::DrainUntilPresentationChange`.
- [ ] Implement `RuntimeStepMode::DrainUntilBudget`.
- [ ] Add `RuntimeStepStopReason` reporting.
- [ ] Add `RuntimeExecutor` trait.
- [ ] Implement `VmExecutor`.
- [ ] Keep VM as semantic source of truth.

## Task / host requests

- [ ] Change `TaskSpec` to hold `HostTaskRequest` and `debug_label`.
- [ ] Remove `TaskSource.label` as an execution discriminator.
- [ ] Add file read/write requests.
- [ ] Add HTTP fetch/respond requests/effects.
- [ ] Add process run request.
- [ ] Add asset/shader/audio/TTS requests.
- [ ] Add custom capability request.

## Parser / AWFT grammar

- [ ] Add `entry` declaration to AST.
- [ ] Add `entry` parser dispatch.
- [ ] Add `entry` HIR lowering.
- [ ] Add selected entry lowering in runtime plan.
- [ ] Add `extern capability` declaration to AST.
- [ ] Add capability parser dispatch.
- [ ] Add capability HIR lowering.
- [ ] Add capability effect checking.
- [ ] Add virtual path capability examples to docs.

## Compile-gap hardening

- [ ] Replace line-based enum parsing with logical/CST item parsing.
- [ ] Replace line-based struct field parsing with logical/CST item parsing.
- [ ] Replace line-based state field parsing with logical/CST item parsing.
- [ ] Replace line-based trait member parsing with logical/CST item parsing.
- [ ] Replace raw brace-counting `collect_logical_block_items` with token/CST-aware collection.
- [ ] Parse callable bodies structurally like function bodies.
- [ ] Add pure function call lowering in executable runtime expressions.
- [ ] Add typed method/function calls or reject them with actionable diagnostics.

## CLI

- [ ] Remove `--frames`.
- [ ] Add `--steps`.
- [ ] Add `--mode one-op|drain|game|server`.
- [ ] Add `--max-ops`.
- [ ] Add `--entry`.
- [ ] Rename runtime report `frames` to `steps`.
- [ ] Add `arcw cli` after capability grammar lands.
- [ ] Add `arcw serve` after server entry/adapter lands.

## Tests and fixtures

- [ ] Add `tests/fixtures/awft/current_pass/check`.
- [ ] Add `tests/fixtures/awft/current_pass/run`.
- [ ] Add `tests/fixtures/awft/spec_should_pass/check`.
- [ ] Add `tests/fixtures/awft/spec_should_pass/run`.
- [ ] Add `tests/fixtures/awft/spec_should_fail`.
- [ ] Add sema fixture loader test.
- [ ] Add CLI check/run fixture test.
- [ ] Add focused compile gap regression test.
- [ ] Unignore spec_should_pass tests after implementation catches up.
