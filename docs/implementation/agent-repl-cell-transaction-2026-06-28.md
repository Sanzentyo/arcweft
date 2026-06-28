# Agent REPL cell transaction substrate — seq05.1 implementation note

This implementation adds `arcweft-agent-repl` as the owner of Agent REPL transaction state.

## Boundaries

- `arcweft-agent-repl` owns base snapshot, overlay cells, transaction phases, binding/generation evidence, rollback invariants, and tiering projections.
- `arcweft-agent-runner` remains the controller VM execution and host-call dispatch boundary.
- `arcweft-cli` should migrate to command adaptation over `ReplSession` instead of owning compiler-dependent state.
- Product player crates do not depend on the new crate.

## Transaction model

For a typed cell input, the session performs:

1. fragment parse/classification;
2. synthetic source construction;
3. Agent bundle compilation against the base `ProjectSemanticIndex`;
4. bytecode verifier gate;
5. effect/capability authorization;
6. runtime project-hash preflight;
7. deterministic committed-cell record construction;
8. immediate VM execution through `AgentRunner::run_controller_bundle`;
9. execution/generation/binding/tier evidence publication.

Pre-commit failures leave committed cells, bindings, overlay hash, and invalidation tokens unchanged.

## Execution failure after commit

VM failure after commit is retained as cell evidence. The record stores status, error string, event counts, host-call counts, and `partially_effectful`. Undo and reset are state mutations only; they intentionally do not reverse arbitrary external host effects.

## Tiering projection

`ReplExecutableSnapshot` exposes bytecode and stable hashes for successfully executed cells. `ReplTierInvalidationToken` streams changes after commit, failed execution, undo, reset, base project change, generation change, and tier status updates.

## Validation

Validated locally on 2026-06-28:

```bash
cargo fmt --all
cargo check -p arcweft-agent-repl -p arcweft-agent-runner -p arcweft-compiler -p arcweft-runtime-driver --all-targets --all-features
cargo clippy -p arcweft-agent-repl -p arcweft-agent-runner -p arcweft-compiler -p arcweft-runtime-driver --all-targets --all-features -- -D warnings
cargo test -p arcweft-agent-repl repl_cell --all-features
cargo test -p arcweft-agent-repl repl_transaction --all-features
cargo test -p arcweft-compiler repl_cell --all-features
cargo test -p arcweft-cli --test regression_harness --all-features
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
just test-workspace
```

The implementation was also formatted after clippy-driven cleanup of the main evaluation path. `ReplSession::evaluate_cell` now delegates pre-commit validation and commit-record construction to separate private domain steps, keeping rollback invariants visible without transitional fallback APIs.

## Remaining Work

- Wire the CLI REPL command surface to `ReplSession` instead of carrying command-state ownership in the adapter.
- Add seq05.2/seq05.3 adapters for persistent runner tiers and agent-script integration over `ReplExecutableSnapshot` and invalidation tokens.
- Add broader workspace validation at the next main cut point after adjacent seq03.7 fixture changes land.

## Design Deviations

No intentional design deviation from the seq05.1 package. The only implementation adjustment was splitting validation and commit construction inside `session.rs` to satisfy the active clippy limits while preserving the package's transaction order and avoiding transitional fallback layers.
