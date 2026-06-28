# Seq05.3 REPL executor tiering, warm, and codegen status

This note records the implementation overlay for
`arcweft-seq05.3-repl-executor-tiering-warm-codegen-2026-06-28.zip`.

## Applied scope

- `arcweft-agent-runner` keeps the product controller path bytecode-VM-first.
  The existing `run_controller_bytecode` and `run_controller_bundle` paths still
  construct `BytecodeVmAgentControllerExecutorFactory`.
- REPL/dev callers can opt into `TieredAgentControllerExecutorFactory` with an
  `AgentControllerExecutorTierPolicy`, but VM fallback remains explicit and on by
  default.
- `arcweft-agent-repl` owns the seq05.3 command implementation and status
  surface over the seq05.1 executable snapshot / invalidation API and seq05.2
  typed command hooks.
- `arcweft-runtime-codegen` remains executor-neutral. The overlay adds stable
  enum label/accessor methods to existing owned types, not helper shims.

## Runtime behavior

Immediate committed-cell execution remains `AgentRunner::run_controller_bundle`
through the bytecode VM. `:warm` and `:codegen` do not replace the execution path
or dispatch host calls. When a full-script backend is unavailable, status is
stable and deterministic:

```text
requested: true
started_background_job: false
backend_status: unsupported
fallback: bytecode_vm
reason: full_script_backend_not_available
```

The status-only manager records tier status back through `ReplSession` so
`:generations --tiers` continues to use the seq05.1 projection stream.

## Invalidation

The manager consumes `ReplTierInvalidationToken` values since its last observed
cursor. Tokens from cell commit, failed execution, undo, reset, base project
change, and generation change are rendered as invalidated artifact identities in
`:codegen` status. `TierStatusRecorded` is preserved as session evidence but does
not itself invalidate executable artifacts.

## Validation

Applied in the local Arcweft checkout and validated with:

```bash
cargo fmt --all -- --check
cargo test -p arcweft-agent-repl repl_tiering --all-features
cargo test -p arcweft-runtime-codegen --all-features
cargo check -p arcweft-agent-repl -p arcweft-agent-runner -p arcweft-runtime-codegen -p arcweft-runtime-driver --all-targets --all-features
cargo clippy -p arcweft-agent-repl -p arcweft-agent-runner -p arcweft-runtime-codegen -p arcweft-runtime-driver --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
```

The structural audit reported `0 error(s), 115 warning(s)`.

## Apply Adjustments

The package patch applied cleanly. Two local follow-up fixes were needed during
validation:

- `ReplWarmOutcome::status_record` now handles empty `warmed_cells` without
  indexing into an empty list. This is the expected status-only unsupported warm
  path.
- The runner tier-policy unit tests were moved to the end of `runner.rs`, and
  `ReplTierManager::codegen_status` now borrows invalidation tokens instead of
  consuming them, matching clippy's active lint configuration.
