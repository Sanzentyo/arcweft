# Seq05.2.2 runtime-driver task ownership adapter

This implementation package fills the task-ownership gap found after Seq05.2.1.
The goal is to let `:tasks` and `:cancel` observe and mutate runtime task state
through a runtime-driver-owned projection rather than duplicating scheduler state
inside `arcweft-agent-repl`, CLI, MCP, or LSP adapters.

## Ownership model

- `arcweft-runtime-driver::task` owns the stable task projection:
  `RuntimeTaskRecord`, `RuntimeTaskStatus`, `RuntimeTaskListOptions`,
  `RuntimeTaskCancelTarget`, `RuntimeTaskCancelOutcome`, `RuntimeTaskRegistry`,
  and `RuntimeTaskOwner`.
- `BundleSession` owns a `RuntimeTaskRegistry`. Each emitted
  `HostTaskDispatch` is registered with generation, logical epoch, sequence,
  task id, and cancel scope metadata before it crosses into a native/web host
  loop.
- Runtime task completions entering `BundleSession::step_with_clock` update the
  registry before entering the VM. Terminal task events continue to release
  generation pins; progress events keep the task/generation active.
- Runtime-initiated cancel-scope requests and REPL-initiated cancellation both
  enqueue deterministic `TaskEventKind::Cancelled` events in the runtime-owned
  registry. The next runtime step drains those events into VM input.
- `arcweft-agent-repl` receives only a thin `RuntimeTaskReplCommandHost` adapter
  that converts runtime-driver task records/outcomes into existing REPL evidence
  types. It stores no scheduler registry.

## Acceptance mapping

| Requirement | Implementation point |
| --- | --- |
| REPL does not own scheduler state | `RuntimeTaskReplCommandHost` delegates to `RuntimeTaskOwner`; state remains in runtime-driver. |
| CLI/MCP/LSP adapters call host/runtime owner | Adapter type can wrap any existing `ReplCommandHost` plus any `RuntimeTaskOwner`, including `BundleSession`. |
| `:tasks` lists active and optionally completed tasks | `RuntimeTaskListOptions::include_completed` and `RuntimeTaskRegistry::list`. |
| `:cancel all`, task id, scope id | `RuntimeTaskCancelTarget::{All, Task, Scope}` and `RuntimeTaskRegistry::cancel`. |
| Deterministic `ReplCancelOutcome` | Runtime outcome carries `cancelled` and `pending_after`; adapter preserves the original `ReplCancelTarget`. |
| Read-only trace rejects cancel before host mutation | Existing command-effect gate already rejects `HostMutation`; new test asserts host `cancel` is not called. |
| Tests cover listing/cancellation/filtering | Runtime registry unit tests plus REPL runtime-owner adapter tests and trace-policy cancel test. |

## Changed files

- `crates/arcweft-runtime-driver/src/task.rs`
- `crates/arcweft-runtime-driver/src/session.rs`
- `crates/arcweft-agent-repl/Cargo.toml`
- `crates/arcweft-agent-repl/src/command.rs`
- `crates/arcweft-agent-repl/src/command/runtime_task.rs`
- `crates/arcweft-agent-repl/tests/repl_task_adapter.rs`
- `crates/arcweft-agent-repl/tests/repl_trace.rs`
- `docs/implementation/seq-05.2.2-runtime-driver-task-ownership-2026-06-28.md`

## Validation commands

```bash
cargo fmt --all -- --check
cargo test -p arcweft-runtime-driver task --all-features
cargo test -p arcweft-agent-repl repl_task_adapter --all-features
cargo test -p arcweft-agent-repl repl_trace --all-features
cargo check -p arcweft-runtime-driver -p arcweft-agent-repl --all-targets --all-features
cargo clippy -p arcweft-runtime-driver -p arcweft-agent-repl --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
```

## Verification status

The package was applied in the repository checkout on 2026-06-29. Validation
completed:

- `cargo fmt --all -- --check`
- `cargo test -p arcweft-runtime-driver task --all-features -- --nocapture`
- `cargo test -p arcweft-agent-repl --test repl_task_adapter --all-features -- --nocapture`
- `cargo test -p arcweft-agent-repl --test repl_trace --all-features -- --nocapture`
- `cargo check -p arcweft-runtime-driver -p arcweft-agent-repl --all-targets --all-features`
- `cargo clippy -p arcweft-runtime-driver -p arcweft-agent-repl --all-targets --all-features -- -D warnings`
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`
- `git diff --check`

The structural audit reported `0 error(s), 117 warning(s)` across 984 Rust files
and 466,992 Rust physical LOC.

## Remaining TODOs

- Wire CLI/MCP/LSP concrete hosts to construct `RuntimeTaskReplCommandHost` at
  their existing host boundary. The reusable adapter is included; terminal/MCP/LSP
  formatting remains intentionally outside runtime-driver.

## Design deviations

None from the Seq05.2.2 request. The implementation keeps formatting outside
runtime-driver and does not redesign the background scheduler. During repository
validation, the internal `RuntimeTaskRegistry::cancel` and
`BundleSession::cancel_runtime_tasks` methods were adjusted to borrow the cancel
target rather than retain unnecessary by-value parameters.
