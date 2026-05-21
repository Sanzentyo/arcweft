# Runtime boundary and arcw grammar refactor checklist

No compatibility aliases. No deprecated wrappers. No transitional re-export modules.

## Fixture activation note

The direction package is applied without rewriting future-spec fixtures into
today's grammar. Files under `tests/fixtures/arcw/current_pass/` and
`tests/fixtures/arcw/spec_should_pass/` are now expected to pass without
file-name allowlists. Negative fixtures under `spec_should_fail/` are active
and cover removed syntax, missing capability effects, forbidden OS paths in
filesystem capability calls, and old RuntimeFrame naming.

## Core rename and structure

- [x] Rename `crates/arcweft-core/src/frame.rs` to `crates/arcweft-core/src/step.rs`.
- [x] Delete old frame-boundary exports and the `arcweft_core::frame` module.
- [x] Add `RuntimeStepInput`, `RuntimeStepInputRef`, `RuntimeStepOutput`, `RuntimeStepOutputSink`.
- [x] Add `RuntimeStepResult`, `RuntimeStepOptions`, `RuntimeStepBudget`, `RuntimeStepMode`, `RuntimeStepStopReason`.
- [x] Replace `external_values` with `bindings` everywhere.
- [x] Replace `FlowFiber.frames` with `FlowFiber.control_stack`.
- [x] Keep flow-control stack names as `FlowControlStackEntry` and `FlowControlStackEntryKind`.
- [x] Replace top-level `line_effects` with `RuntimeEffectBatch`.
- [x] Replace top-level `task_requests` with `HostRequestBatch`.
- [x] Add `RuntimePayload`.
- [x] Replace string-only source/stream events with structured payload events.

## Runtime execution

- [x] Change `Engine::step` signature to require `RuntimeStepOptions`.
- [x] Implement `RuntimeStepMode::OneOp`.
- [x] Add first `RuntimeStepMode::{Drain, Game, Server}` API surface.
- [x] Teach `Engine::step` to internally drain according to mode.
- [x] Add presentation-change-specific drain semantics.
- [x] Enforce `RuntimeStepBudget` inside the VM loop instead of only reporting API shape.
- [x] Add `RuntimeStepStopReason` reporting.
- [x] Add `RuntimeExecutor` trait.
- [x] Implement `VmExecutor`.
- [x] Keep VM as semantic source of truth.

## Task / host requests

- [x] Change `TaskSpec` to hold `HostTaskRequest` and `debug_label`.
- [x] Remove `TaskSource.label` as an execution discriminator.
- [x] Add file read/write requests.
- [x] Add HTTP fetch/respond requests/effects.
- [x] Add process run request.
- [x] Add asset/shader/audio/TTS requests.
- [x] Add custom capability request.

## Parser / arcw grammar

- [x] Add `entry` declaration to AST.
- [x] Add `entry` parser dispatch.
- [x] Add `entry` HIR lowering.
- [x] Add selected entry lowering in runtime plan.
- [x] Add `extern capability` declaration to AST.
- [x] Add capability parser dispatch.
- [x] Add capability HIR lowering.
- [x] Add capability effect checking.
- [x] Add virtual path capability examples to docs.

## Compile-gap hardening

- [x] Replace line-based enum parsing with logical/CST item parsing.
- [x] Replace line-based struct field parsing with logical/CST item parsing.
- [x] Replace line-based state field parsing with logical/CST item parsing.
- [x] Replace line-based trait member parsing with logical/CST item parsing.
- [x] Replace raw brace-counting `collect_logical_block_items` with token/CST-aware collection.
- [x] Parse callable bodies structurally like function bodies.
- [x] Add pure function call lowering in executable runtime expressions.
- [x] Add typed method/function calls or reject them with actionable diagnostics.

## CLI

- [x] Remove `--frames`.
- [x] Add `--steps`.
- [x] Add `--mode one-op|drain|game|server`.
- [x] Add `--max-ops`.
- [x] Add `--entry`.
- [x] Rename runtime report `frames` to `steps`.
- [x] Add `arcw cli` after capability grammar lands.
- [x] Add `arcw serve` as a Sans I/O server-entry route-plan command.
- [x] Add native server adapters that consume `arcw serve` route plans.
- [x] Add AOT executor as another `RuntimeExecutor` implementation.

## Tests and fixtures

- [x] Add `tests/fixtures/arcw/current_pass/check`.
- [x] Add `tests/fixtures/arcw/current_pass/run`.
- [x] Add `tests/fixtures/arcw/spec_should_pass/check`.
- [x] Add `tests/fixtures/arcw/spec_should_pass/run`.
- [x] Add `tests/fixtures/arcw/spec_should_fail`.
- [x] Add sema fixture loader test.
- [x] Add CLI check/run fixture test.
- [x] Add focused compile gap regression coverage through current-pass fixtures.
- [x] Unignore spec_should_pass tests after implementation catches up.

