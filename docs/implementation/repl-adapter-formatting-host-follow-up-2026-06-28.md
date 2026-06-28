# REPL Adapter Formatting And Host Adapter Follow-Up

Date: 2026-06-28
Sequence: seq05.2.1

## Summary

This implementation keeps `arcweft-agent-repl` as the typed Sans-I/O owner and moves CLI presentation into `arcweft-cli`. It adds a CLI formatter for `ReplCommandResult`, a typed bridge from `parse_repl_input` into the current CLI REPL loop, and typed unsupported/read-only diagnostics for task/cancel and warm/codegen cases that the current legacy CLI cannot execute directly.

## Files

- `crates/arcweft-cli/src/app/agent/native/repl_command_format.rs`
- `crates/arcweft-cli/src/app/agent/native/repl_command_bridge.rs`
- `crates/arcweft-cli/src/app/agent/native.rs`
- `crates/arcweft-cli/src/app/agent/native/repl.rs`
- `crates/arcweft-cli/Cargo.toml`

## Design decisions

### CLI formatting owner

`arcweft-cli` owns human-readable output. `arcweft-agent-repl` remains typed and Sans I/O.

### Formatter API

The CLI module exposes a private adapter boundary:

```rust
pub(super) struct ReplCommandFormatOptions;
pub(super) struct ReplCommandFormattedOutput;
pub(super) trait ReplCommandResultFormatter;
```

This mirrors the requested shape while avoiding premature public API until MCP/LSP have concrete consumers.

### JSON mode

JSON mode uses the structured projection created from typed evidence. Human text is not reparsed. In non-JSON mode, the existing CLI cell report value includes `formatted_text` to let the current printer emit stable human text without changing the serialized report schema broadly.

### Evidence coverage

The formatter is exhaustive over all current `ReplCommandEvidence` variants. Warm/codegen are formatted from public seq05.3 evidence fields and do not inspect tiering manager internals.

### CLI typed bridge

The applied checkout splits the typed CLI bridge into
`repl_command_bridge.rs` rather than keeping the bridge helpers inside
`repl.rs`. This avoids pushing the already large legacy REPL module over the
structural audit error threshold.

The bridge parses input through `parse_repl_input`, formats typed results via
`ReplCommandResult`, and only falls back to the existing CLI meta path for
commands that are not owned by the seq05 typed command model. `:reset` is not
routed to the old CLI state reset because that would give one typed command name
two meanings. Until the CLI owns a full seq05.1 session-backed adapter, typed
`:reset` returns a structured `HostUnavailable` diagnostic.

### Runtime-driver task/cancel

The current runtime-driver provides deterministic host task dispatch and cancellation event conversion, but not a stable list/cancel registry. The implementation does not duplicate scheduler state. It keeps the existing `ReplCommandHost` task/cancel seam and returns typed unsupported/read-only diagnostics from the CLI adapter where no host task owner exists.

## MCP/LSP status

No immediate MCP/LSP adapter is added. Current source inspection found Agent MCP surfaces but no REPL command-result surface consuming `ReplCommandResult`. LSP documentation exists, but no current Agent REPL command-output endpoint was identified. A future implementation should either promote the formatter JSON projection to a shared tooling crate or create a transport-specific adapter once the endpoint exists.

## Validation status

Applied in the local checkout and validated with:

```bash
cargo fmt --all
cargo check -p arcweft-cli --all-targets --all-features
cargo test -p arcweft-agent-repl repl_command --all-features
cargo test -p arcweft-agent-repl repl_trace --all-features
cargo test -p arcweft-cli repl --all-features
cargo test -p arcweft-runtime-driver task --all-features
cargo clippy -p arcweft-agent-repl -p arcweft-cli -p arcweft-runtime-driver --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root . --write target\structure-audit-seq05-2-1
```

The structural audit reported 0 errors and 115 warnings. The new formatter file
is 1,053 physical LOC, below the production-file error threshold but above the
preferred ordinary responsibility module target; it is intentionally separated
from the legacy REPL module so future formatter sharing or MCP/LSP promotion can
happen without growing `repl.rs`.
