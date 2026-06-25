# Content Policy Gate Stabilization

Date applied: 2026-06-23

## Implemented

- Added an MCP-side `AgentPublishedResourceCache` keyed by moderated public URI, with source-to-public URI bookkeeping for replacement and invalidation.
- `resources/list`, `arcweft.observe`, `arcweft.session.info`, `arcweft.trace.read`, `arcweft.capture`, and resource reads now publish through `AgentMcpState` before returning MCP DTOs.
- `resources/read` and `arcweft.resource.read` first honor cached moderated URIs, then resolve raw URIs through the existing raw privacy check, publication gate, cache store, and read projection.
- Added `--content-policy-mode strict|local-dev` to Agent MCP and Agent observe MCP-format output. The default is `strict`.
- `strict` continues to withhold image and scene content when only the deterministic text rule classifier is present.
- `local-dev` combines the strict text rule classifier with a no-finding local visual classifier for host-generated color captures only; object-id and mask auxiliary captures remain withheld.
- Image publication failures now fail safe as review placeholders instead of MCP request errors for missing metadata, missing bytes, unsupported encodings, and decode failures.

## Preserved Boundaries

- `arcweft-content-policy` remains Sans I/O.
- `PublishedAgentResource::new` remains crate-private.
- `arcweft-agent-mcp` still only accepts `PublishedAgentResource` for descriptor/read/tool-result projection.
- Protocol enum behavior remains on owning protocol types.
- `PrivacyClass` remains a separate confidentiality gate. Published image resources, including withheld image placeholders, still require an appropriate `max_privacy` when their resource kind is image.

## Tests Added Or Updated

- `resources/list` moderated URIs can be read from the MCP cache.
- Raw URI reads cache a moderated URI that can be read afterward.
- Missing image metadata is a withheld JSON resource, not a request error.
- Strict mode withholds color captures without a real visual classifier.
- Local-dev mode allows color captures while preserving moderated URIs and metadata scrubbing.
- Local-dev mode still withholds object-id and mask auxiliary captures.

## Validation

- `cargo test -p arcweft-content-policy`
- `cargo test -p arcweft-agent-policy`
- `cargo test -p arcweft-agent-mcp`
- `cargo test -p arcweft-cli --features "native-capture agent-repl" agent_mcp_resource_read_enforces_capture_privacy --lib`
- `cargo test -p arcweft-cli --features "native-capture agent-repl" agent_mcp_ --lib`
- `cargo check --workspace --all-targets --all-features`
- `cargo clippy -p arcweft-content-policy -p arcweft-agent-policy -p arcweft-agent-mcp -p arcweft-cli --all-targets --all-features -- -D warnings`
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audit-content-policy-stabilization-2026-06-23`

## Remaining Follow-Up

- Wire a production visual classifier through the publication gate before changing the `strict` default behavior.
- Decide whether published withheld image placeholders should receive a non-image resource kind in a future contract revision; this cut keeps the existing privacy-class behavior.
