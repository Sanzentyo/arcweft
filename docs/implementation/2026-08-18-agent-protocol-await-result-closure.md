# Agent protocol Await Result closure — 2026-08-18

## Inspected state

- Inspected Git revision: `171d0c315ae218a4f51b3d1a293d85c22e218ea7`.
- Working tree at inspection: clean on `main`, matching `origin/main`.
- Implementation state recorded here: dirty direct-checkout cut, before its
  commit.

## Established boundary

AWBC task suspension materializes terminal task completion as the canonical
`Result<T, E>` value. Agent protocol response records therefore bind through an
`Ok` variant pattern, not directly as `T`.

`RuntimeCheckedType` now has one closed Agent operational predicate. It accepts
an executable `RuntimeValue::Agent` only for the exact operational family, and
accepts the existing protocol-record physical carrier only for the closed Agent
families whose field coordinates permit that carrier. RuntimePlan and AWBC type
projection preserve this predicate. Generic `Probe<T>` remains operational-only
because an operational Agent tag would discard its checked child type.

AWBC field verification resolves a wire label through
`RuntimeAgentField::from_owner_label`; an owner-independent string is not a
successful field coordinate. The VM uses the same closed protocol-record
admission rule when checking a Result payload.

## Performed and passed

- `cargo check -p arcweft-core -p arcweft-runtime-plan -p arcweft-agent-runner --all-targets --all-features`
- `cargo test -p arcweft-core --lib checked_agent_type_accepts_only_its_owned_runtime_carriers`
  — 1 passed.
- `cargo test -p arcweft-core --lib protocol_field_wire_label_requires_the_exact_owner`
  — 1 passed.
- `cargo test -p arcweft-agent-runner --lib controller_awbc_resumes_ -- --nocapture`
  — 5 passed.
- `cargo test -p arcweft-agent-runner --lib` — 47 passed.
- `just structure-audit` — 0 blocking violations.
- `cargo fmt --all` and `git diff --check`.

## Failed or blocked validation

- Strict Clippy was not green before reaching this cut. The workspace run first
  fails in `arcweft-lang-syntax` on the pre-existing
  `AttachedViewFragmentEntry` `large_enum_variant`. A core-only continuation
  additionally reports pre-existing strict warnings in AWBC product state,
  checked-type projection, runtime evaluation, and task construction. This cut
  does not claim strict Clippy closure.
- `just test-workspace` reached the LSP library and failed 3 of 212 LSP tests:
  two pre-existing runtime-plan-lower fixture failures and one stale diagnostic
  expectation for `sema.await.error_mismatch` where sema now emits
  `sema.try.error_mismatch`. All earlier workspace test groups, including core
  213/213 and sema 830 passed with 8 ignored, completed successfully. None of
  the three failed tests or their production owners are changed by this cut.

## Structural review

- `crates/arcweft-core/src/value/agent.rs` remains the cohesive owner of the
  closed Agent value, field-coordinate, field-owner, and field-result algebra.
  Resolving the AWBC wire label there avoids a verifier-local copied field
  table. Decomposition into a second registry would create a parallel
  authority, so the owner remains cohesive despite the size review trigger.
- `crates/arcweft-core/src/awbc/fiber.rs` retains runtime value/type admission
  beside fiber restore and frame validation; this cut adds only the Agent
  protocol-record arm shared with the checked predicate.
- `crates/arcweft-core/src/awbc/verify/code.rs` remains the instruction
  dataflow verifier. It consumes, rather than redefines, Agent field behavior.
- `crates/arcweft-agent-runner/src/tests.rs` is a large maintained integration
  fixture owner. The five cases share one typed seed factory and remain together
  because they exercise one controller/AWBC/host-response boundary.

## Non-goals

- This cut does not change unary `Need<T>`, prefix `try`, Await handlers, or the
  generic Try CFG lowering. Those are a separate coherent language/runtime cut.
- This cut does not repair the three unrelated LSP workspace failures or the
  repository-wide strict Clippy backlog.
