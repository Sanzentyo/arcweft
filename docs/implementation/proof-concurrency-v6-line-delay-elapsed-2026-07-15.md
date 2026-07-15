# Proof-concurrency v6: live line delay elapsed-time cut

Date: 2026-07-15

## Accepted implementation slice

The proof-concurrency v6 contract requires delay-triggered line-task nodes to
use accumulated logical elapsed time from activation. The AWBC product executor
already accumulated `RuntimeStepInput::dt`, while the structured `RuntimePlan`
engine compared each delay only with the current step delta.

This cut makes the structured live-line path retain a typed
`LogicalDuration`, add each suspended step delta with saturation, and pass the
accumulated value to delay-trigger evaluation. Line activation starts at zero;
the delta supplied to the step that activates the line describes time before
activation and is not counted. AWBC suspension status now projects its existing
elapsed counter into the shared `DialogueState` contract.

## Preserved invariants

- Delay readiness is `elapsed >= delay` in both structured and AWBC paths.
- Logical duration addition cannot wrap.
- A delayed node starts at most once because `started_nodes` remains the
  idempotence authority.
- Mark triggers continue to inspect only the current normalized input batch.
- The one-shot `run_line_task_group` helper retains its existing interpretation
  of `input.dt` because it has no persistent activation state.

## Explicit non-goals

This cut does not implement persistent state for every line graph node, true
sequence/start/parallel-join execution, recursive sibling cancellation and
drain, work/subscription separation, a persistent inbox, global checkpointing,
replay generations, or hot swap. Those changes require the larger execution
state and authority model from proof-concurrency cuts 8 through 11.

## Validation

- `cargo test -p arcweft-core --all-features`: 184 unit tests and 9 runtime-ID
  boundary tests passed; doc tests passed.
- `just test-fast`: core, render-text, text-layout, render-wgpu, and
  native-player library suites passed (317 tests total).
- `cargo check --workspace --all-targets --all-features`: passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed.
- `cargo fmt --all -- --check` and `git diff --check`: passed.
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`: 1,299 Rust
  files, 635,286 physical LOC, 0 errors, and 127 existing warning-level review
  findings.
