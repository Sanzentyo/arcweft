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
  being classified as a package non-goal.
- Added CLI-owned `StdioMcpTransport` for line-delimited JSON-RPC MCP child
  processes and wired retained remote `McpAgentSession` connections into
  `--connect` / `:connect`. The REPL performs the remote handshake before
  swapping session state, runs remote Agent cells through `AgentRunner`, and
  routes remote `:observe` / `:capture` through typed `AgentSession` calls.
- Extended `arcw fmt` path handling to include `.awfagent`, dispatch through
  `SourceDialect::Agent`, and reject game-only sugar rewrites for Agent sources.
- Added `arcweft-data::raw` with shape-checked raw transcoding. Type labels now
  live on `RawValue`, `Number`, and `TypeShape`; the earlier external
  `raw_type_error`/label-helper shape was removed.
- Hardened the `arcweft-data-derive` attribute parser so `#[arcweft(...)]`
  parsing returns structured `syn::Result` errors instead of discarding parse
  failures. Unknown attributes, invalid `rename_all` / `bytes` / `repr` values,
  wrong container targets, `content` without `tag`, and duplicate final wire
  names are compile errors covered by trybuild fixtures. Bare `bytes` and
  `default = "path::factory"` are accepted as part of the typed grammar.
- Added `arcweft-data::DecodeBudget` and wired Arcweft Binary decoding through
  parse-time input, node, depth, collection length, string length, and bytes
  length checks before value allocation. Arcweft Binary now rejects duplicate
  map keys and enum payload flags other than `0` or `1`.
- Hardened `arcweft-save` envelope decoding with explicit `SaveDecodeOptions`,
  envelope/schema id/codec id/payload length caps, trailing-data rejection,
  exact expected schema id checks, future-version rejection, required migration
  for older versions, migration schema/version checks, and post-migration shape
  validation.

## Remaining implementation debt

- Remote REPL cells now execute through the typed MCP session, but the
  project-bound binding policy is still coarse: primitive/string/collection
  binding preservation versus session-bound binding drop on project-hash
  changes needs explicit diagnostics and tests.
- The CLI-owned stdio MCP transport is process-backed and covered by a fake
  child roundtrip, but timeout enforcement, bounded stderr retention, and
  graceful shutdown-before-kill behavior still need hardening coverage.
- The checked-in `.awfagent` formatter path is dialect-aware and diagnostic
  producing, but it is not yet a full lossless canonical formatter with golden
  coverage for comments/trivia and all Agent item families.
- Data raw transcoding covers the initial shape/value bridge. Format-specific
  JSON/TOML/YAML raw codecs, parse-time decode budgets, and strict binary raw
  coverage remain separate data-format tasks.
- The derive parser now rejects malformed attributes and covers the main
  container/field/variant grammar. Remaining derive work includes precise
  generic bounds, tuple/unit struct policy, multi-field tuple enum policy, and
  repr discriminant range validation.
- Parse-time budget coverage currently protects Arcweft Binary and provides the
  shared `DecodeBudget` API. JSON/TOML/YAML/MsgPack/CBOR and tabular codecs
  still need parser-integrated visitors/readers rather than post-parse-only
  validation.
- Save decoding now enforces the envelope identity/version gates from the ZIP
  guide. Remaining save work is to model explicit multi-step migration chains
  and decide whether the checksum contract should cover canonical header
  metadata in addition to the payload for a future envelope version.

## Validation

```bash
cargo check -p arcweft-data -p arcweft-test -p arcweft-agent-mcp -p arcweft-agent-mcp-client -p arcweft-tooling -p arcweft-cli --all-targets --all-features
cargo check -p arcweft-data -p arcweft-data-derive --all-targets --all-features
cargo check -p arcweft-core -p arcweft-cli -p arcweft-agent-runner -p arcweft-agent-mcp-client --all-targets --all-features
cargo test -p arcweft-core --all-features
cargo check -p arcweft-agent-mcp-client -p arcweft-cli --all-targets --all-features
cargo test -p arcweft-data raw_shape --test raw_shape
cargo test -p arcweft-data --features derive --test derive_attrs
cargo test -p arcweft-data --features derive derive_attribute_ui
cargo clippy -p arcweft-data -p arcweft-data-derive --all-targets --all-features -- -D warnings
cargo test -p arcweft-codec-binary --test strict_decode
cargo clippy -p arcweft-data -p arcweft-codec-binary --all-targets --all-features -- -D warnings
cargo check -p arcweft-save --all-targets --all-features
cargo test -p arcweft-save --test strict_decode
cargo clippy -p arcweft-save --all-targets --all-features -- -D warnings
cargo test -p arcweft-agent-mcp -p arcweft-agent-mcp-client -p arcweft-test --all-features
cargo test -p arcweft-tooling agent_format --all-features
cargo test -p arcweft-cli stdio_transport_roundtrips_agent_session_calls_through_fake_child --all-features
cargo test -p arcweft-cli agent_repl_parse_stdio_connection --all-features
cargo clippy --workspace --all-targets --all-features
cargo +nightly -Zscript tools/arcweft-structure-audit.rs --root . --write docs/implementation/structure-audit-2026-06-21
```

All commands above passed on Windows in this checkout.
