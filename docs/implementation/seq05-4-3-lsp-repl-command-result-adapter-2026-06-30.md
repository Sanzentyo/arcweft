# Seq05.4.3 LSP REPL command-result adapter

Date: 2026-06-30
Package: `arcweft-seq05.4.3-lsp-repl-command-result-adapter-2026-06-30.zip`

## Summary

This cut defines and implements the LSP owner boundary for a REPL command-result
adapter without turning `arcweft-lsp` into a task scheduler or REPL runtime host.
The stable custom request is `arcweft/replCommand`.

The implementation factors the existing protocol-facing MCP command execution
logic into `arcweft-agent-repl::command::ReplCommandEndpoint`. MCP and LSP then
share the same typed parser, typed command dispatch, runtime-task borrow point,
read-only trace handling, cell-execution rejection, and structured JSON evidence
projection.

## Required design decisions

### 1. LSP workspace session ownership

`ArcweftLspSession` must not own a `ReplSession`. The stdio LSP session remains a
document/LSP request state owner only. REPL execution is available only when a
native runtime/debug host passes a borrowed `LspReplCommandExecutor` for a single
request.

The default `ArcweftLspSession::handle_request` route still handles
`arcweft/replCommand`, but it returns a typed `host_unavailable` command result
instead of creating hidden REPL state.

### 2. Method and DTOs

Request method:

```text
arcweft/replCommand
```

Request params:

```json
{
  "input": ":tasks --include-completed",
  "command_id": 1,
  "trace_policy": "read_write",
  "max_items": 32,
  "max_string_bytes": 240,
  "include_diagnostics": true
}
```

Response result:

```json
{
  "result": {
    "command_id": 1,
    "status": "ok|queued|rejected|error|exit_requested",
    "evidence": { "kind": "..." },
    "diagnostics": []
  },
  "diagnostics": [],
  "is_error": false
}
```

`result` is the shared `ReplCommandResult` JSON projection. `diagnostics` mirrors
request-scoped REPL command diagnostics for clients that do not want to traverse
`result.diagnostics`.

### 3. RuntimeTaskOwner source

An LSP request obtains runtime task access only through the borrowed host path:

1. a native/debug host owns an existing `ReplSession`, `ReplCommandHost`, and
   runtime-driver `RuntimeTaskOwner` such as a live runtime session;
2. the host builds `ReplCommandEndpoint::new(session, handler)` and optionally
   adds `.with_host(...)` and `.with_runtime_tasks(...)`;
3. the host wraps it in `LspReplCommandEndpoint` and passes it into
   `ArcweftLspSession::handle_request_with_repl_executor(...)`.

No scheduler registry, global lookup table, or task state is added to
`arcweft-lsp`.

### 4. Diagnostics mapping

LSP REPL diagnostics mirror `ReplCommandDiagnostic` directly in the custom
response. They are not published as editor diagnostics because command failures
are request-scoped and usually have no document range. Existing document
analysis diagnostics continue through `textDocument/publishDiagnostics`.

### 5. Cell execution scope

Cell execution is out of scope for `arcweft/replCommand`, matching the MCP
meta-command endpoint. A cell request returns `unhandled_extension` unless the
request is already rejected by read-only trace policy.

## Changed files

- `crates/arcweft-agent-repl/src/command.rs`
- `crates/arcweft-agent-repl/src/command/endpoint.rs`
- `crates/arcweft-agent-mcp/src/repl_command.rs`
- `crates/arcweft-lsp/Cargo.toml`
- `crates/arcweft-lsp/src/lib.rs`
- `crates/arcweft-lsp/src/custom.rs`
- `crates/arcweft-lsp/src/repl_command.rs`
- `crates/arcweft-lsp/src/session.rs`
- `crates/arcweft-lsp/src/session/tests.rs`
- `docs/implementation/seq05-4-3-lsp-repl-command-result-adapter-2026-06-30.md`

## Validation run

Executed on this checkout:

```bash
cargo fmt --all
cargo check -p arcweft-agent-repl -p arcweft-agent-mcp -p arcweft-lsp --all-targets --all-features
cargo test -p arcweft-agent-repl repl_command --all-features -- --nocapture
cargo test -p arcweft-agent-mcp repl_command --all-features -- --nocapture
cargo test -p arcweft-lsp repl_command --all-features -- --nocapture
cargo clippy -p arcweft-agent-repl -p arcweft-agent-mcp -p arcweft-lsp --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root . --write target/structure-audit-seq05-4-3
```

Results:

- `cargo check`: passed.
- `cargo test -p arcweft-agent-repl repl_command`: 8 passed.
- `cargo test -p arcweft-agent-mcp repl_command`: 7 passed across unit and
  integration targets.
- `cargo test -p arcweft-lsp repl_command`: 3 passed.
- `cargo clippy ... -D warnings`: passed after extracting the LSP custom REPL
  request branch into a small associated helper so `try_handle_request` stays
  under the active line-count lint.
- Structural audit: 2,119 files scanned, 1,044 Rust files, 493,375 Rust
  physical LOC, 0 error(s), 124 warning(s).

Structure measurements for the touched production files:

- `crates/arcweft-agent-repl/src/command/endpoint.rs`: 7,687 bytes, 204
  physical LOC, production, no embedded tests.
- `crates/arcweft-lsp/src/repl_command.rs`: 9,340 bytes, 275 physical LOC,
  production, embedded focused tests.
- `crates/arcweft-agent-mcp/src/repl_command.rs`: 7,618 bytes, 223 physical
  LOC, production, embedded focused tests.
- `crates/arcweft-lsp/src/session.rs`: 20,511 bytes, 494 physical LOC,
  production, no embedded tests.

## Remaining work

- Wire `handle_request_with_repl_executor` from a concrete native/debug LSP host
  once that host owns or borrows a real REPL/runtime session.
- Keep stdio LSP document-only unless a later package explicitly changes the host
  architecture.

## Design deviations

None from the Seq05.4.3 request. The implementation intentionally returns a typed
`host_unavailable` result for document-only LSP rather than inventing task state
inside the LSP adapter.
