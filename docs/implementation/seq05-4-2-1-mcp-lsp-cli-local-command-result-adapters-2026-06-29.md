# Seq05.4.2.1 MCP/LSP CLI-local command-result adapters

Date: 2026-06-29
Package: `arcweft-seq05.4.2.1-mcp-lsp-cli-local-command-result-adapters-2026-06-29.zip`

## Summary

This cut extends the current MCP `arcweft.repl.command` path so it can classify
raw input with the same two typed parser stages as the CLI. Shared REPL inputs
continue through `McpReplCommandEndpoint` and serialize `ReplCommandResult`.
CLI-local inputs serialize `CliReplCommandResult` through a new CLI-owned
protocol JSON projection.

The implementation does not execute CLI-local commands through MCP/LSP generic
REPL adapters. Instead, it returns deterministic typed diagnostics unless a
future endpoint borrows the required CLI process state. This keeps terminal
formatting, filesystem, capture, debug-store, and transport dependencies out of
`arcweft-agent-repl`, `arcweft-agent-mcp`, LSP crates, and product player crates.

## Changed files

- `crates/arcweft-cli/src/app/agent/native/repl_cli_command.rs`
- `crates/arcweft-cli/src/app/agent/native/repl_cli_command/protocol.rs`
- `crates/arcweft-cli/src/app/agent/native/repl_cli_command/format.rs`
- `crates/arcweft-cli/src/app/agent/native/mcp_protocol.rs`
- `crates/arcweft-agent-mcp/src/repl_command.rs`
- `docs/implementation/seq05-4-2-1-mcp-lsp-cli-local-command-result-adapters-2026-06-29.md`

## Design decisions

### CLI-local JSON owner

The CLI-local result JSON stays in `arcweft-cli`. It is moved from the terminal
formatter to `repl_cli_command::protocol` so protocol adapters can serialize
it without depending on human formatting.

### MCP endpoint

The existing `arcweft.repl.command` MCP tool is the stable endpoint for this
cut. The CLI MCP host routes the request through `parse_agent_repl_input`:

- `Shared(ReplInput)` delegates to the existing `McpReplCommandEndpoint`.
- `Cli(CliReplCommand)` returns typed `CliReplCommandResult` protocol JSON.
- parse failures after both stages return typed shared diagnostics.

### LSP endpoint

No stable LSP REPL command-result endpoint was found. LSP implementation remains
a follow-up rather than adding speculative protocol shims.

### Read-only trace mode

The existing `McpReplCommandRequest.trace_policy` field represents read-only
trace policy for both shared and CLI-local commands. CLI-local mutating commands
return `read_only_trace_rejected` before the generic unavailable diagnostic.

## Validation

Applied and validated in this checkout. The package patch file was malformed for
`git apply`, so the overlay files were copied and the focused MCP/shared-test
hunks were applied manually against current `main`.

Commands run:

```bash
cargo fmt --all -- --check
cargo test -p arcweft-agent-mcp repl_command --all-features -- --nocapture
cargo test -p arcweft-cli mcp_repl_command --all-features -- --nocapture
cargo test -p arcweft-cli repl_cli_protocol --all-features -- --nocapture
cargo test -p arcweft-cli repl_cli_inspection --all-features -- --nocapture
cargo test -p arcweft-cli repl_command_bridge --all-features -- --nocapture
cargo check -p arcweft-agent-mcp -p arcweft-cli --all-targets --all-features
cargo clippy -p arcweft-agent-mcp -p arcweft-cli --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
```

All commands passed. Structural audit reported 0 error(s) and 121 warning(s).

## Remaining work

- Add LSP command-result adapter only after a stable LSP endpoint exists.
- Add executable CLI-local protocol behavior only through dedicated state-owning
  protocol tools or a future CLI-hosted adapter that explicitly borrows required
  state.

## Design deviations

None from seq05.4.2.1. The package implements the stable MCP endpoint and records
LSP as unavailable in current main.
