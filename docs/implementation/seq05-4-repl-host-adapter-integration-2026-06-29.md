# Seq05.4 REPL host adapter integration

Date: 2026-06-29
Package: `arcweft-seq05.4-repl-host-adapter-integration-2026-06-29.zip`

## Summary

This package wires the concrete CLI Agent REPL bridge to the accepted typed REPL command/result model, the CLI-owned command formatter, seq05.3 tiering status evidence, and the runtime-driver task owner adapter. It replaces the temporary seq05.2.1 stub branches for `:tasks`, `:cancel`, `:warm`, and `:codegen` with typed dispatch through `ReplCommandContext` and `ReplTierCommandHandler`.

The integration point for runtime task ownership is intentionally at the CLI host boundary. When an active `BundleSession` is present, the CLI bridge wraps the Agent-session host in `RuntimeTaskReplCommandHost`. When no runtime session is active, the builtin host fallback returns typed host-unavailable diagnostics.

## Changed files

- `crates/arcweft-agent-repl/src/command/dispatch.rs`
- `crates/arcweft-cli/src/app/agent/native/repl.rs`
- `crates/arcweft-cli/src/app/agent/native/repl_command_bridge.rs`
- `crates/arcweft-cli/tests/repl_host_adapter_source_gates.rs`
- `docs/implementation/seq05-4-repl-host-adapter-integration-2026-06-29.md`
- `docs/reviews/requests/2026-06-29-seq-05.4.1-mcp-lsp-repl-command-result-adapters.md`

## Implementation details

### Command dispatch

`repl_command_bridge.rs` now uses:

```text
parse_repl_input
  -> ReplCommandContext
  -> ReplTierCommandHandler
  -> ReplCommandResult
  -> CliReplCommandFormatter
```

The old branch that matched selected `ReplCommand` variants and manufactured local error evidence is removed. The CLI no longer creates fake task lists or cancel outcomes.

### Command IDs

`ReplCommandContext::with_next_command_id` lets the CLI seed command IDs from the existing cell index. This is an inherent method on the owned context type rather than a CLI helper or compatibility shim.

### Session-backed commands

The CLI state now initializes a `ReplSession` and a deterministic `CliAgentSession` from the existing Agent Script project index. Session-owned commands (`:cells`, `:undo`, `:reset`, `:capabilities`, `:generations`) operate on that seq05.1 session. Cell submissions are routed through `ReplSession::evaluate_cell`, so command evidence and tier invalidation see the same overlay state.

### Host reads

Local CLI host reads use `AgentSessionReplCommandHost<CliAgentSession>`. Remote MCP-backed CLI sessions use `AgentSessionReplCommandHost<McpAgentSession<StdioMcpTransport>>` for `:observe` and `:step` where the active CLI connection is remote.

### Runtime tasks

The CLI bridge constructs `RuntimeTaskReplCommandHost` when `state.runtime_session` is present. The task state remains inside `BundleSession` / `RuntimeTaskOwner`. The source gate prevents `RuntimeTaskRegistry` from appearing in `arcweft-cli` or `arcweft-agent-repl` source.

### Read-only trace mode

The bridge sets `ReplTracePolicy::ReadOnlyTrace` on `ReplCommandContext`. The existing command-effect gate rejects `:cancel` before the host adapter is called. The formatter emits typed `read_only_trace_rejected` diagnostics.

### Warm/codegen

A persistent `ReplTierCommandHandler` in CLI state handles `:warm` and `:codegen`. Unsupported full-script backend status appears as typed evidence with `backend_status=unsupported`, `fallback=bytecode_vm`, and `reason=full_script_backend_not_available`.

## MCP/LSP

MCP is partially present as an Agent-session host in the CLI remote connection path. No stable MCP or LSP REPL command-result output surface was identified for this cut. Protocol adapters are deferred by the follow-up request included in `docs/reviews/requests/2026-06-29-seq-05.4.1-mcp-lsp-repl-command-result-adapters.md`.

## Validation

Local repository validation after applying the package:

```bash
cargo fmt --all -- --check
cargo test -p arcweft-agent-repl repl_command --all-features -- --nocapture
cargo test -p arcweft-agent-repl repl_task_adapter --all-features -- --nocapture
cargo test -p arcweft-agent-repl repl_tiering --all-features -- --nocapture
cargo test -p arcweft-runtime-driver task --all-features -- --nocapture
cargo test -p arcweft-cli repl --all-features -- --nocapture
cargo check -p arcweft-agent-repl -p arcweft-cli -p arcweft-runtime-driver --all-targets --all-features
cargo clippy -p arcweft-agent-repl -p arcweft-runtime-driver --all-targets --all-features -- -D warnings
cargo clippy -p arcweft-cli --all-targets --no-default-features --features agent-repl,native-capture -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
```

Results:

- all focused tests above passed;
- `cargo check` passed for the package target crates;
- `arcweft-agent-repl` and `arcweft-runtime-driver` clippy passed with
  `-D warnings`;
- `arcweft-cli` clippy passed with the REPL/native-capture feature set that
  excludes the unrelated native-player dependency;
- structure audit scanned 1,998 files and 1,010 Rust files, reporting 0 errors
  and 119 warnings;
- formatting and whitespace checks passed.

Blocked by pre-existing warnings outside this cut:

```bash
cargo clippy -p arcweft-agent-repl -p arcweft-cli -p arcweft-runtime-driver --all-targets --all-features -- -D warnings
```

This all-features command reaches `arcweft-player-native` through
`arcweft-cli/native-player` and fails on existing dead-code warnings in
`native_audio.rs`, `window_driver.rs`, and `windowed.rs`.

## Remaining work

- Populate `AgentReplState::runtime_session` from the native-player runtime session creator once that CLI mode owns a live `BundleSession` during Agent REPL sessions.
- Implement protocol-level MCP/LSP adapters only after stable `ReplCommandResult` endpoints are introduced.

## Design deviations

- The package patch files were malformed for `git apply`, so the overlay was
  reconciled manually against current `main`.
- The package overlay removed all CLI meta fallback. The applied implementation
  keeps a narrow fallback only for CLI-specific inspection commands that are not
  seq05 typed commands, such as `:trace`, `:ast`, `:hir`, `:bytecode`,
  `:capture`, `:query`, `:save`, and `:connect`.
- The old compiled-cell execution path was deleted from production code after
  typed `ReplSession::evaluate_cell` became the single cell submission path.
  Snapshot helper coverage remains test-only.
- MCP/LSP command-result adapters are deferred because no stable transport endpoint was found.
- Normal CLI Agent REPL startup currently has no active `BundleSession`; the package wires the adapter path and no-session typed diagnostics without inventing a scheduler registry.
