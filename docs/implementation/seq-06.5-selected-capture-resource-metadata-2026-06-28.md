# Seq 06.5 selected capture resource metadata

This implementation wires shared `arcweft-layout::CaptureMetadata` into protocol-owned selected capture metadata for object/layer image resources.

## Production changes

- `arcweft-agent-protocol` owns the external `image.selected_capture` schema and conversion from `arcweft-layout::CaptureMetadata`.
- Native capture refs and rendered image resources now carry selected object/layer metadata through `AgentImageMetadata`, MCP resource descriptors, MCP `resources/read`, and tool `resource_link` blocks.
- `arcweft-agent-policy` keeps scrubbed image metadata for metadata-only capture resources that are withheld because image bytes are absent. The resource body is still a moderated JSON policy placeholder.
- `arcweft-layout::LayoutRect` now uses `new(LayoutPoint, LayoutSize)` as its canonical boundary constructor, with `from_xywh(...)` for scalar construction.
- The selected-capture audit script and fixture examples were added under `tools/` and `fixtures/selected-capture-metadata/`.

## Stable protocol field

`AgentImageMetadata` and capture refs expose:

```rust
pub selected_capture: Option<AgentSelectedCaptureMetadata>
```

The field serializes as `selected_capture` and is populated for selected object and selected layer captures.

## Boundary

`arcweft-layout` remains Sans I/O and owns renderer-neutral geometry facts. `arcweft-agent-protocol` owns the external JSON schema and source identity. Native/WebGPU adapters construct layout metadata and convert it with protocol builders.

`object_count` is retained only as typed layer source identity. The transient underscore-prefixed package artifact is not part of the protocol schema.

## Lazy behavior

MCP listing derives metadata from capture refs and observed geometry. It does not render capture images. Actual `resources/read` refines the same metadata with actual crop origin and raster dimensions after rendering.

## Privacy

External image publication scrubs selected capture scope/source IDs in the same pass that already scrubs image object metadata.

Metadata-only selected captures that cannot publish image bytes use the existing moderated JSON placeholder path and keep only scrubbed `image` metadata. Auxiliary capture kinds remain withheld without image metadata by default.

## Package application notes

The package patch files were not applied directly because the patch hunks did not apply cleanly to the current checkout. The implementation was manually ported against current `main`, preserving the package acceptance criteria and using the current crate boundaries.

## Validation

Validation run:

```bash
cargo fmt --all -- --check
cargo test -p arcweft-agent-policy --all-features -- --nocapture
cargo test -p arcweft-cli --features native-capture --test check selected_capture_metadata -- --nocapture
cargo test -p arcweft-cli --features native-capture --test check agent_mcp_stdio_lists_selected_capture_metadata -- --ignored --nocapture
cargo test -p arcweft-layout --test presentation_contract --all-features
cargo test -p arcweft-agent-protocol capture_metadata --all-features -- --nocapture
cargo test -p arcweft-agent-mcp --all-features -- --nocapture
cargo check -p arcweft-layout -p arcweft-agent-protocol -p arcweft-agent-mcp -p arcweft-agent-policy -p arcweft-cli --features native-capture --all-targets
cargo clippy -p arcweft-layout -p arcweft-agent-protocol -p arcweft-agent-mcp -p arcweft-agent-mcp-client -p arcweft-agent-policy -p arcweft-cli -p arcweft-glyphon -p arcweft-render-native -p arcweft-render-wgpu --features native-capture --all-targets -- -D warnings
just test-workspace
cargo +nightly -Zscript tools/selected-capture-metadata-audit.rs --root .
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
```

The ignored MCP stdio E2E was run explicitly for this implementation slice. The structural audit reported `0 error(s), 107 warning(s)` across `927` Rust files and `445165` Rust physical LOC.

## Remaining TODOs

- Add broader visual golden coverage that compares selected object/layer crop metadata against captured pixels.
- Keep platform-native selected-capture parity under the seq06.4 adapter work; this slice only wires the shared protocol/resource schema and native observe path.
