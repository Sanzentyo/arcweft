# Content Policy Rewrite Acceptance

Source package: `D:/sanze/Downloads/arcweft-content-policy-rewrite-20260622.zip`

Date applied: 2026-06-23

## Implemented

- Added `arcweft-content-policy` as a Sans I/O crate for classifier reports, policy profile evaluation, text redaction, RGBA masking, rendered-scene masking, and deterministic receipts.
- Added `arcweft-agent-policy` as the Agent publication gate that consumes raw `AgentResource` values and returns `PublishedAgentResource`.
- Moved Agent protocol behavior onto owning types:
  - `AgentImageKind`, `AgentImageComposition`, `AgentImageScope`, `AgentImageRenderer`
  - `AgentImageMetadata`
  - `AgentResource`, `AgentResourceKind`, `AgentResourceBody`, binary body/encoding types
  - `AgentObservedObjectContent`
- Moved confidentiality lattice helpers onto `PrivacyClass` and provider-boundary filtering onto `EmbeddingProviderScope`.
- Changed `arcweft-agent-mcp` projection APIs to accept `PublishedAgentResource`, so raw resources cannot be passed to MCP descriptor/read/tool-result conversion functions.
- Added CLI native MCP publication calls through `AgentContentPolicyGate::strict_builtin()` immediately before MCP projection.
- Updated MCP tests to use real publication through the gate and to assert moderated URIs plus external metadata scrubbing.

## Security Properties

- Unsupported classifier modalities fail closed under the strict profile.
- Sanitized text, images, and rendered scenes are classified again before publication.
- Object-id and mask attachments are withheld by default.
- Published image metadata receives opaque scope IDs and drops free-form object/diagnostic metadata.
- MCP content blocks include public policy summaries for published resources.
- Raw resource caches may still exist in host code, but MCP conversion functions no longer accept raw resource types.

## Non-Goals

- This cut does not include production model weights or an inference backend.
- This cut does not implement OS sandboxing for model execution.
- This cut does not replace confidentiality authorization; `PrivacyClass` remains a separate gate that must pass in addition to content policy.

## Validation

- `cargo check -p arcweft-content-policy`
- `cargo check -p arcweft-agent-policy`
- `cargo check -p arcweft-agent-mcp`
- `cargo check -p arcweft-cli`
- `cargo check -p arcweft-cli --all-features`
- `cargo test -p arcweft-content-policy`
- `cargo test -p arcweft-agent-policy`
- `cargo test -p arcweft-agent-mcp`

## Remaining Follow-Up

- Wire a host-provided visual classifier through `AgentContentPolicyGate` for production image and scene publication.
- Decide whether moderated URI lookup should be cached directly, or whether clients should continue to request raw internal URIs that are resolved and moderated at read time.
