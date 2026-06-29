# Seq05.4.2 CLI REPL inspection/debug command adapter

Date: 2026-06-29
Package: `arcweft-seq05.4.2-cli-repl-inspection-debug-command-adapter-2026-06-29.zip`

## Summary

This cut removes the final transitional CLI Agent REPL meta-command parser seam. The CLI bridge now routes input through the shared `arcweft-agent-repl` parser first and a CLI-owned typed inspection/debug parser second. CLI-only commands return `CliReplCommandResult` with typed evidence and structured diagnostics before terminal or JSON formatting.

## Command ownership

Shared `arcweft-agent-repl` commands:

- `:observe`
- `:step`
- `:tasks`
- `:cancel`
- `:load`
- `:reload`
- `:cells`
- `:undo`
- `:reset`
- `:capabilities`
- `:generations`
- `:warm`
- `:codegen`
- `:help`
- `:quit`

CLI-owned typed commands:

- `:trace`
- `:actions`
- `:type`
- `:ast`
- `:hir`
- `:bytecode`
- `:capture`
- `:query`
- `:drop`
- `:save`
- `:connect`
- `:parse`
- `:classify`
- `:complete`
- `:highlight`
- `:history`
- `:bindings`

`:bindings` remains CLI-owned because current production behavior reports CLI-local `AgentReplState::bindings`, including loaded-agent and saved-cell adapter fields. This is distinct from shared session binding evidence.

## Parser behavior

`parse_agent_repl_input` returns `AgentReplParsedInput::Shared(ReplInput)` for shared commands and cells. When the shared parser returns an error, the CLI parser attempts `CliReplCommand`. If both stages fail, the final diagnostic records either the malformed command owner or an unknown-command diagnostic that names both parser stages.

## Read-only trace mode

Read-only rejection now uses `CliReplCommandKind::effect()` and shared `ReplTracePolicy::permits_command`. The deleted `agent_repl_read_only_rejects(command: &str)` table is not replaced with another string table.

## Formatting

Shared commands continue through `CliReplCommandFormatter` and `ReplCommandResult`. CLI-only commands use `CliReplLocalCommandFormatter` and `CliReplCommandResult`. Both JSON paths are typed. Human output remains terminal-owned in `arcweft-cli`.

## Deleted legacy code

- `agent_repl_cli_meta_command()`.
- The fallback branch from shared parser failure to `agent_repl_eval_meta(...)`.
- `agent_repl_eval_meta(...)` string dispatch and its sub-dispatch helpers.
- `agent_repl_read_only_rejects(command: &str)`.
- Dead legacy wrappers that only existed for the removed string evaluator, including
  the old CLI-local `:observe`, `:connect`, and `.awfagent` `:load` wrappers.

The shared typed `:load` command now routes through `ReplProjectLoader`. CLI-local
`.awfagent` load-as-binding behavior is not preserved as a hidden compatibility
path because it conflicted with the shared command owner model.

## Validation

Executed in `D:\git\arcweft` after applying the package:

```bash
cargo fmt --all
cargo test -p arcweft-agent-repl repl_command --all-features -- --nocapture
cargo test -p arcweft-cli repl_cli_inspection --all-features -- --nocapture
cargo test -p arcweft-cli repl_command_bridge --all-features -- --nocapture
cargo check -p arcweft-agent-repl -p arcweft-cli --all-targets --all-features
cargo clippy -p arcweft-agent-repl -p arcweft-cli --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
```

Results:

- Shared REPL command tests: 8 passed.
- CLI inspection/debug adapter tests: 11 passed.
- Bridge tests: 2 passed.
- Focused `cargo check`: passed with no warnings.
- Focused clippy: passed with `-D warnings`.
- Structural audit: `0 error(s), 121 warning(s)`.
- `git diff --check`: passed.

## Structural measurement

Measured after implementation in the current checkout:

| Path | Bytes | Physical LOC | Role |
| --- | ---: | ---: | --- |
| `crates/arcweft-cli/src/app/agent/native.rs` | 17,342 | 351 | facade/orchestration module |
| `crates/arcweft-cli/src/app/agent/native/repl.rs` | 48,519 | 1,395 | existing CLI Agent REPL host/state module |
| `crates/arcweft-cli/src/app/agent/native/repl_command_bridge.rs` | 21,532 | 572 | typed bridge between shared and CLI-local commands |
| `crates/arcweft-cli/src/app/agent/native/repl_cli_command.rs` | 745 | 14 | module facade |
| `crates/arcweft-cli/src/app/agent/native/repl_cli_command/types.rs` | 9,671 | 350 | CLI-local command/result types |
| `crates/arcweft-cli/src/app/agent/native/repl_cli_command/parse.rs` | 19,691 | 533 | second-stage parser and diagnostics |
| `crates/arcweft-cli/src/app/agent/native/repl_cli_command/dispatch.rs` | 14,201 | 423 | typed CLI-local dispatcher |
| `crates/arcweft-cli/src/app/agent/native/repl_cli_command/format.rs` | 12,089 | 331 | CLI-local human/JSON formatter |

`repl.rs` remains a warning-level hotspot above 1,200 physical LOC, but this cut
removes 398 legacy string-dispatch lines from it. The new responsibility modules
are in the preferred 300-800 LOC range, and the structural audit reported no
errors.

## Remaining work

- Implement MCP/LSP command-result adapters only after stable endpoints exist, using the follow-up request included in this package.
