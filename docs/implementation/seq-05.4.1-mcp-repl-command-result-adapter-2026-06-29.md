# Seq05.4.1 MCP REPL command-result adapter

Date: 2026-06-29

## Summary

Introduces a narrow MCP REPL command-result surface where a stable endpoint exists. The endpoint parses raw REPL input via `parse_repl_input`, dispatches typed meta-commands through `ReplCommandContext` / `ReplCommandHandler`, and serializes structured `ReplCommandResult` evidence without reparsing terminal text.

## Crate boundaries

- `arcweft-agent-repl` owns `ReplCommandResult` JSON projection because it owns the typed command/result/evidence model and already depends on the lower tooling crate.
- `arcweft-agent-mcp` owns the MCP tool name, schema, DTO request, and Sans I/O execution adapter.
- `arcweft-cli` keeps human terminal formatting and only delegates JSON mode to the shared projection.
- Product-player crates do not depend on REPL, CLI, MCP, or LSP tooling crates.

## LSP decision

No LSP execution endpoint is added in this package. The current LSP session has no existing REPL host/runtime owner state. Adding one would violate the request's non-goals by creating task/session state in the LSP adapter. A follow-up request records the required owner boundary.

The package draft named that follow-up `seq05.4.2`, but this checkout already uses `seq05.4.2` for the CLI REPL inspection/debug typed adapter request. The LSP owner-boundary follow-up was therefore recorded as `docs/reviews/requests/2026-06-29-seq-05.4.3-lsp-repl-command-result-adapter.md`.

## Runtime task handling

`McpReplCommandEndpoint` accepts an optional borrowed `RuntimeTaskOwner`. When a host and task owner are both supplied, it constructs `RuntimeTaskReplCommandHost` and delegates `:tasks` / `:cancel` through the runtime-driver owner. No registry is introduced.

## Read-only trace mode

Protocol requests use `trace_policy: "read_only_trace"`. Rejections preserve `ReplCommandDiagnosticCode::ReadOnlyTraceRejected`, surfaced as `read_only_trace_rejected` in structured JSON.

## Tests added

- host unavailable returns typed `host_unavailable`,
- read-only mutating command returns typed rejection,
- task list uses existing runtime owner path,
- cancel all/task/scope uses existing runtime owner path,
- warm/codegen unsupported backend status remains structured JSON evidence.

## Local validation

Run in `D:\git\arcweft` after applying the package to current `main`:

```bash
cargo fmt --all
cargo check -p arcweft-agent-repl -p arcweft-agent-mcp -p arcweft-cli --features native-player --all-targets
cargo test -p arcweft-agent-repl -p arcweft-agent-mcp
cargo test -p arcweft-cli --features native-player repl_command_formatter
cargo clippy -p arcweft-agent-repl -p arcweft-agent-mcp -p arcweft-cli --features native-player --all-targets
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

Results:

- focused check passed,
- REPL/MCP tests passed,
- CLI REPL formatter focused tests passed,
- focused clippy passed without package-origin warnings,
- structural audit reported `0 error(s), 119 warning(s)` across `1998` scanned files and `1016` Rust files.

## Design deviations

- The package patch file was corrupt and could not be applied directly, so the overlay files were applied and reconciled against current `main`.
- The CLI stdio MCP host exposes `arcweft.repl.command` and initializes a minimal REPL session, but it does not synthesize a runtime task owner. Runtime task ownership remains available through `McpReplCommandEndpoint::with_runtime_tasks(...)` for concrete hosts that already own a `RuntimeTaskOwner`.
