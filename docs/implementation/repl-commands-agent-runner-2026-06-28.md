# Seq05.2 REPL commands and Agent runner/session integration

This note records the seq05.2 implementation overlay for typed Agent REPL
commands on top of the accepted seq05.1 `arcweft-agent-repl` transaction
substrate.

## Baseline

- The package was authored against seq05.1 commit
  `8b89da4c984f068574157b7f82d99fdc6eb5c631`.
- It was applied on the current `main` lineage after
  `4a8603c4` (`Apply seq06.7 exact native golden drift stabilization`).
- Seq05.1 is treated as implemented substrate. This overlay does not reapply or
  redesign seq05.1 transaction internals.
- Existing CLI REPL code is migration evidence only. It is not the source of
  truth for the final command API.

## Ownership

`arcweft-agent-repl::command` owns typed user-visible command parsing,
read-only trace policy, deterministic command result/evidence types, and
extension hooks for seq05.3. It delegates:

- overlay cell state, `:cells`, `:undo`, `:reset`, capability inspection,
  generation/binding/tier evidence, and base replacement effects to the public
  seq05.1 `ReplSession` API;
- host observation/step/task/cancel behavior to `ReplCommandHost` adapters over
  existing Agent/runtime boundaries;
- project loading/reloading to `ReplProjectLoader` so this crate remains Sans
  I/O;
- `:warm` and `:codegen` to `ReplBackgroundRequestSink` or a composed
  `ReplCommandHandler` supplied by seq05.3.

## Command input split

`parse_repl_input` checks the first non-whitespace character. If it is `:`, the
input is parsed as a typed `ReplCommand`; otherwise it is preserved as a
`ReplCellInput`. This matches the seq05.1 command delegation guard and prevents
command text from falling into source cell classification.

## Read-only trace mode

`ReplTracePolicy::ReadOnlyTrace` rejects source cell submission and command
classes that mutate session state, host task state, or background tier state.
It allows deterministic inspection commands plus host-read replay commands such
as `:observe` and `:step` when the provided host adapter is a replay/read-only
adapter.

## Evidence

Every command returns `ReplCommandResult` with a deterministic `ReplCommandId`,
`ReplCommandStatus`, typed `ReplCommandEvidence`, and stable diagnostics. Undo
and reset evidence includes post-operation binding/generation projections and
tier invalidation tokens gathered through the seq05.1 API.

Applied refinement: command evidence intentionally avoids stringly projections
where Arcweft already owns an enum. `ReplCapabilityReport` now exposes
`RuntimeAgentCapability` values, and `ReplDebugEventCount` records
`DebugEventKind`. Display adapters can format those enums through their
existing label methods.

Applied refinement: `:undo` and `:reset` command evidence does not store the
full seq05.1 outcome objects. `ReplUndoOutcome` contains a full removed
`ReplCellRecord`, which made `ReplCommandEvidence` disproportionately large.
The command API instead returns `ReplUndoSummary` / `ReplResetSummary` plus the
post-command binding, generation, and tier-invalidation evidence required by
seq05.2. This keeps the result value owned and stack-friendly without adding
`Box` indirection to the public command evidence enum.

## Extension hooks

Seq05.3 can implement `:warm` and `:codegen` by composing `ReplCommandHandler`
or by providing a `ReplBackgroundRequestSink`. No stringly command table edits
are required.

## Validation plan

The package is designed for this validation shape after applying the overlay:

```bash
cargo fmt --all -- --check
cargo test -p arcweft-agent-repl repl_command --all-features
cargo test -p arcweft-agent-repl repl_trace --all-features
cargo check -p arcweft-agent-repl -p arcweft-agent-runner -p arcweft-runtime-driver --all-targets --all-features
cargo clippy -p arcweft-agent-repl -p arcweft-agent-runner -p arcweft-runtime-driver --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
```

The package authoring environment could not execute Cargo because the container
has no Rust toolchain and no direct network clone access. Source inspection used
the GitHub connector against `Sanzentyo/arcweft` `main`.

Local application validation executed after applying and refining the package:

```bash
cargo fmt --all -- --check
cargo test -p arcweft-agent-repl repl_command --all-features
cargo test -p arcweft-agent-repl repl_trace --all-features
cargo check -p arcweft-agent-repl -p arcweft-agent-runner -p arcweft-runtime-driver --all-targets --all-features
cargo clippy -p arcweft-agent-repl -p arcweft-agent-runner -p arcweft-runtime-driver --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
```

The structural audit reported `1796` files scanned, `954` Rust files,
`454135` Rust physical LOC, `0 error(s)`, and `110 warning(s)`.

## Structural audit note

The command API is implemented as a responsibility module family rather than a
single large file:

- `command.rs`: 35 LOC facade;
- `command/types.rs`: 633 LOC typed command/result/evidence model;
- `command/parse.rs`: 329 LOC parser and stable parser diagnostics;
- `command/host.rs`: 116 LOC host/project-loader adapter boundary;
- `command/dispatch.rs`: 476 LOC trace policy and builtin dispatch.

All production Rust files added by this overlay remain below the AGENTS.md
1,200 LOC warning threshold.

Remaining adapter follow-up work from the package is split into
`docs/reviews/requests/2026-06-28-seq-05.2.1-repl-adapter-follow-up-package.md`:
adapt the CLI REPL to format `ReplCommandResult`, add MCP/LSP formatting
adapters if needed, and provide runtime-driver task/cancel host adapters.

Seq05.3 tiering handlers for `:warm` and `:codegen` are tracked separately by
`docs/reviews/requests/2026-06-28-seq-05.3-repl-executor-tiering-warm-codegen-package.md`.
