# 2026-07-04 Component-scoped render capture implementation note

## Summary

This patch implements first-class component-scoped capture for Agent observe/capture. It extends the existing viewport/layer/object capture axis with `component` as an owned typed variant across layout metadata, Agent protocol resources, CLI selectors, MCP capture arguments, selected-capture metadata, and resource listing.

## What changed

- Added `CaptureScope::Component { id }` in `arcweft-layout`.
- Added component variants to Agent image scope, selected-capture scope, capture source identity, and controller `CaptureTarget`.
- Added `AgentObservedComponent`, `AgentComponentCaptureRefs`, `AgentComponentCaptureRef`, and `AgentImageComponentRef`.
- Added `AgentObservationReport.components` plus `components_resource()` and `AgentResourceKind::Components`.
- Added component capture refs and `arcweft://.../component.*` URI parsing.
- Added `--component` and MCP `component` selector, mutually exclusive with layer/object selectors.
- Added component capture to resource listing and `--resource all` traversal.
- Reused existing layer/object raster paths for component member selection, debug masks, and masked framebuffer crops.
- Documented the component URI/resource families in the Agent observe/capture contract.

## Design choice

Component identity is not represented as a stringly helper layered on top of object IDs. It is a first-class boundary type in the same owned enums that already define viewport/layer/object scope. This keeps the CLI/MCP surface, capture metadata, and resource introspection consistent.

## Validation performed

```sh
cargo test -p arcweft-agent-protocol
cargo check -p arcweft-cli --features native-capture
cargo test -p arcweft-cli --features native-capture component -- --nocapture
cargo test -p arcweft-agent-mcp
cargo test -p arcweft-agent-policy
cargo fmt
cargo clippy --workspace --all-targets --all-features
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

`cargo test -p arcweft-cli --features native-capture` was also attempted as a
broader package pass, but it exceeded the local 304s command timeout in this
checkout. The focused component/native-capture route above passed and covers
the new component selector, component resource construction, and scoped
capture member selection.

## Known follow-up

The patch includes a conservative component table builder based on object grouping already visible in Agent observations. The renderer/UI lowering integration should replace or enrich that builder with authored component IDs from the prepared-frame component table once that typed table is available.
