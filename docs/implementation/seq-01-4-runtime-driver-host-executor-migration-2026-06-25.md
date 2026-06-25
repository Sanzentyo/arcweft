# Seq-01.4 Runtime Driver / Host Executor Migration Implementation

## Source Package

- `D:/sanze/Downloads/arcweft-seq-01.4-runtime-driver-host-executor-migration.zip`
- Package status: source-unavailable fallback design, no patch overlay.

## Implemented Scope

- Added `arcweft_core::executor::ArcweftRuntimeExecutor` as the shared
  application-facing executor facade.
- Added `arcweft_core::executor::ArcweftExecutionTier` for the current
  structured VM and structured AOT compatibility tiers.
- Moved `BytecodeVmExecutor` and `AotExecutor` construction behind the core
  facade for:
  - `arcweft-runtime-driver`
  - `arcweft-runtime-host`
  - `arcweft-cli`
  - `arcweft-agent-runner`
- Added a source gate in `arcweft-runtime-host` tests so application-facing
  runtime crates do not mention `BytecodeVmExecutor` or `AotExecutor` directly.

## Non-Goals For This Cut

- Product AWFB migration.
- Product default AWBC-only behavior.
- Structured product bytecode payload deletion.
- `compact_bytecode` deletion.
- Full compact AWBC VM product-step parity with `RuntimeStepResult`.

The compact AWBC product-step parity design is split into
`docs/reviews/requests/2026-06-25-seq-01.4.1-compact-awbc-runtime-step-parity.md`.

## Design Deviation

The fallback package requests migration to the AWBC execution facade, but the
available design does not define how compact AWBC `VmExit` / `VmObservation`
become the existing product `RuntimeStepResult` surface. This implementation
therefore completes the implementation-ready part of seq-01.4: application
runtime crates no longer construct low-level structured executors directly, and
the compatibility tiers are centralized behind the shared facade.

## Verification

- `cargo fmt --all -- --check` passed.
- `rg -n "BytecodeVmExecutor|AotExecutor" crates/arcweft-agent-runner/src crates/arcweft-runtime-driver/src crates/arcweft-runtime-host/src crates/arcweft-cli/src -g "*.rs"` returned no matches.
- `cargo test -p arcweft-core executor --all-targets` passed.
- `cargo test -p arcweft-runtime-driver` passed.
- `cargo test -p arcweft-runtime-host` passed, including the new source gate.
- `cargo test -p arcweft-agent-runner` passed.
- Focused CLI runtime executor coverage passed:
  - `run_json_can_select_aot_executor`
  - `run_json_steps_runtime_plan`
  - `profile_json_can_select_aot_executor_without_absolute_source`
  - `bench_json_can_measure_aot_executor_sections`
  - `verify_types_json_reports_type_and_runtime_validation_without_absolute_source`
- `cargo check --workspace --all-targets --all-features` passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` passed.
- `just test-workspace` passed.
- `cargo +nightly -Zscript tools/structure-audit.rs --root .` passed with
  `1492` files scanned, `823` Rust files, `403061` Rust physical LOC, `89`
  package manifests, `0` errors, and `99` warnings.

`cargo test -p arcweft-cli` was attempted but timed out after six minutes; the
runtime executor paths touched by this cut were covered by the focused CLI
tests above and by `just test-workspace`.
