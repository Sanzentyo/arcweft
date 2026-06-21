# ZIP Gap Audit 2026-06-21

This note records the implementation cut for
`arcweft-zip-gap-audit-2026-06-21.zip`.

## Implemented in this cut

- Added `arcweft-agent-mcp-client`, a Sans I/O MCP-backed `AgentSession`
  adapter with typed initialize, tool validation, action alias selection,
  `step_frames`, resource readback, and in-memory contract tests.
- Added reusable `arcweft-test::agent::FixtureAgentSession` for exact
  request/response fixture vectors without putting test fixtures in runner
  production APIs.
- Added `arcweft.act` and `arcweft.session.step_frames` to the MCP descriptor
  and CLI dispatcher surfaces. The action alias shares the canonical
  `arcweft.action` schema and handler.
- Made `arcweft.session.info` include typed `AgentSessionInfo` fields while
  preserving the existing debug/resource inventory payload.
- Changed Agent REPL endpoint parsing so `stdio:` / `mcp:` endpoints are
  represented as structured `AgentReplConnection::StdioMcp` values instead of
  being classified as a package non-goal. Remote execution operations currently
  fail explicitly rather than silently falling back to local source/profile
  execution.
- Extended `arcw fmt` path handling to include `.awfagent`, dispatch through
  `SourceDialect::Agent`, and reject game-only sugar rewrites for Agent sources.
- Added `arcweft-data::raw` with shape-checked raw transcoding. Type labels now
  live on `RawValue`, `Number`, and `TypeShape`; the earlier external
  `raw_type_error`/label-helper shape was removed.

## Remaining implementation debt

- `arcw agent repl --connect stdio:...` and `:connect stdio:...` now parse as
  structured remote endpoints, but the CLI has not yet installed a process
  transport and remote `McpAgentSession` into the REPL runner. Cell execution,
  `:observe`, and `:capture` report this directly when connected to a remote
  endpoint.
- The checked-in `.awfagent` formatter path is dialect-aware and diagnostic
  producing, but it is not yet a full lossless canonical formatter with golden
  coverage for comments/trivia and all Agent item families.
- Data raw transcoding covers the initial shape/value bridge. Format-specific
  JSON/TOML/YAML raw codecs, parse-time decode budgets, and strict binary raw
  coverage remain separate data-format tasks.

## Validation

```bash
cargo check -p arcweft-data -p arcweft-test -p arcweft-agent-mcp -p arcweft-agent-mcp-client -p arcweft-tooling -p arcweft-cli --all-targets --all-features
cargo check -p arcweft-agent-mcp-client -p arcweft-cli --all-targets --all-features
cargo test -p arcweft-data raw_shape --test raw_shape
cargo test -p arcweft-agent-mcp -p arcweft-agent-mcp-client -p arcweft-test --all-features
cargo test -p arcweft-tooling agent_format --all-features
cargo clippy --workspace --all-targets --all-features
cargo +nightly -Zscript tools/arcweft-structure-audit.rs --root . --write docs/implementation/structure-audit-2026-06-21
```

All commands above passed on Windows in this checkout.
