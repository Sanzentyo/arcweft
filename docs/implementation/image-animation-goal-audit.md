# Image Animation Goal Audit

Status: complete implementation goal.

This note tracks whether static images and GIF/WebP animated images are
implemented as first-class Arcweft presentation objects. It is an
implementation-state audit, not a language chapter; stable source and
presentation contracts live in `docs/01-language/grammar.md` and
`docs/03-presentation/image-presentation-objects.md`.

## Objective

Image display must use one typed presentation-object model for static images
and animated containers. Animated frame selection must be deterministic and
debuggable through native capture, Agent observe, hit-test, direct resource
readback, MCP readback, samples, bundle metadata, and tests. The implementation
must not rely on debug-raster transition layers.

## Evidence Matrix

| Requirement | Current evidence | Status |
|---|---|---|
| Decode static PNG/JPEG/WebP and animated GIF/WebP into a shared RGBA frame model | `arcweft-image`; tests `decodes_gif_animation_to_rgba_frames`, `frame_selection_loops_animated_images`, `finite_animation_clamps_to_final_loop`; bundle sample covers PNG/JPEG/static WebP/GIF/animated WebP | Covered |
| Deterministic frame timing independent of wall clock | `DecodedImage::frame_at_time_millis`, `ImageObjectPlayback`, `UiImageSourceTable::resolve_frame`, Agent `capture_time`; hit-test/readback regressions compare frame metadata across pinned times | Covered |
| Semantic image object data model | `arcweft-presentation::image::ImagePresentationObject`; presentation tests for semantic image node and pinned playback | Covered |
| UI frame lowering preserves image sources, metadata, layer, transform, params, and proxies | `arcweft-ui::UiImagePresentationFrame`; `presentation_image_objects_lower_to_ui_sources_display_and_semantics`; `image_source_table_resolves_static_and_animated_frames` | Covered |
| Native renderer uses real textured quads, not debug raster fallback | `arcweft-render-native::capture_image_quads_rgba`; renderer tests for image pixels, debug alpha, opacity, transform, and display-list frame selection | Covered |
| Source surface can author static/animated image ids and bounded image objects | `asset @asset...` and `image @image...` entity declarations in `samples/image-animation.arcw`; `bg(...)`; bounded `image(asset = ..., ...)`; declared `image(@image...)`; sema tests for declared image assets and image object calls | Covered |
| Source-level declared image object syntax beyond bounded `image(...)` calls | `image @image... { ... }` declares reusable object metadata and `image(@image...)` lowers through the same `ImagePresentationObject` path as inline bounded calls. | Covered |
| Bundle boundary records and validates encoded image assets | `image_assets[]`; `bundle_json_packages_image_animation_sample_assets_and_run_bundle_validates_them`; `run_bundle_rejects_image_asset_metadata_mismatch_before_execution` | Covered |
| Agent observe exposes typed image objects | `AgentObservedImageContent`, `AgentImageObjectRef`, presentation tree image metadata, image-animation sample regressions | Covered |
| Direct readback preserves animated object/layer frame metadata and pixels | `agent_observe_read_uri_preserves_animated_image_object_frame_metadata`; `agent_observe_read_uri_preserves_animated_image_layer_frame_pixels` | Covered |
| MCP tool/resource readback preserves metadata and blob bytes | `agent_observe_mcp_tool_result_preserves_animated_image_object_metadata_and_raw_blob`; Tier 2 live stdio `agent_mcp_stdio_reads_animated_image_layer_resource` | Covered, live stdio is Tier 2 |
| Hit-test uses typed image object/proxy metadata and capture-time frame selection | `agent_hit_test_reports_animated_image_object_proxy_metadata`; `agent_hit_test_capture_time_updates_unpinned_animated_image_frame_metadata` | Covered |
| Samples cover static and animated formats plus bounded and declared objects | `samples/image-animation.arcw` declares PNG, JPEG, static WebP, GIF, animated WebP, a reusable declared sprite overlay, clipped object, alignment object, opacity, transform, action, params, proxy, and pinned playback | Covered |
| Docs describe the current stable surface and implementation boundary | `docs/03-presentation/image-presentation-objects.md`; `docs/implementation/image-animation-policy.md`; `docs/05-build-and-security/packaging.md` | Covered |

## Current Push Gate

Before claiming this goal complete or handing off the milestone, run or justify
the equivalent of:

```bash
just test-image-animation-goal
```

The recipe intentionally stays narrower than Tier 2 MCP stdio while covering
the core image-animation object model:

```bash
cargo test -p arcweft-image -- --nocapture
cargo test -p arcweft-presentation image -- --nocapture
cargo test -p arcweft-ui image -- --nocapture
cargo test -p arcweft-render-native image -- --nocapture
cargo test -p arcweft-lang-sema tests::declarations::parses_surface_alias_and_resource_entity_families -- --exact --nocapture
cargo test -p arcweft-lang-sema tests::typecheck::typechecks_presentation_image_object_call_with_named_asset_and_bounds -- --exact --nocapture
cargo test -p arcweft-cli app::image_declarations -- --nocapture
cargo test -p arcweft-cli app::bundle::tests::static_image_asset_refs_collects_declared_image_object_assets -- --exact --nocapture
cargo test -p arcweft-cli --test check bundle_json_packages_image_animation_sample_assets_and_run_bundle_validates_them -- --exact --nocapture
cargo test -p arcweft-cli --features native-capture --test check agent_observe_read_uri_preserves_animated_image_object_frame_metadata -- --exact --nocapture
cargo test -p arcweft-cli --features native-capture --test check agent_observe_read_uri_preserves_animated_image_layer_frame_pixels -- --exact --nocapture
cargo test -p arcweft-cli --features native-capture --test check agent_observe_mcp_tool_result_preserves_animated_image_object_metadata_and_raw_blob -- --exact --nocapture
cargo test -p arcweft-cli --features native-capture --test check agent_hit_test_reports_animated_image_object_proxy_metadata -- --exact --nocapture
cargo test -p arcweft-cli --features native-capture --test check agent_hit_test_capture_time_updates_unpinned_animated_image_frame_metadata -- --exact --nocapture
cargo test -p arcweft-cli --features native-capture --test check agent_observe_image_alignment_sample_uses_authored_alignment_geometry -- --exact --nocapture
cargo test -p arcweft-cli --features native-capture --test check agent_observe_native_captures_clipped_animated_image_object -- --exact --nocapture
cargo run -p arcweft-cli --quiet -- check samples/image-animation.arcw
```

Tier 2 still owns live MCP stdio coverage:

```bash
cargo test -p arcweft-cli --features native-capture --test check agent_mcp_stdio_reads_animated_image_layer_resource -- --ignored --exact --nocapture
```

## Remaining Work

- No known implementation gap remains for the static/animated image object
  milestone. Live MCP stdio remains Tier 2 validation and should be run for
  milestone sign-off or MCP transport changes.
