# Seq-03.5 Windowed Runtime Owner Scene Integration (2026-06-28)

## Summary

This overlay wires `scene_windowed.rs` to use `WindowedRuntimeOwner` as the single owner of the active native patch endpoint, runtime session, image catalog, patch queue, and retained patch report.

## Design decisions

- `NativeSceneState` exposes runtime state to rendering and input code by borrowing through `WindowedRuntimeOwner`: `session()`, `session_mut()`, and `images()`. It no longer stores a direct `BundleSession` plus `BundleImageCatalog` pair.
- The exact `AfterRenderSubmitted` site is `NativeSceneState::redraw` immediately after `self.render(&prepared)?` returns. `render` has submitted the command buffer and presented the surface frame before returning.
- The first production scene integration drains all queued typed events at the safe boundary instead of processing only one patch per frame.
- Patch reports are surfaced through `WindowedRuntimeOwner::last_patch_report()` and queue depth through `queued_patch_count()`; no visible overlay UI is added in this cut.
- Renderer pipelines, font registration, and glyph caches survive content/code patches. The current renderer uploads image textures per frame and rebuilds text buffers per frame, so applied/restarted outcomes invalidate the prepared frame/input hit data rather than resetting GPU renderer state.

## Implementation notes

- `WindowedRuntimeOwner::from_bundle` builds the image catalog from the decoded bundle and starts the AWFB-backed native endpoint from encoded product bytes.
- `WindowedRuntimeOwner::drain_patch_boundary` processes all queued events, but `WindowedPatchQueue::pop_ready` still rejects unsafe boundaries before popping.
- Invalid patch material is reported as `WindowedRuntimeOutcome::Rejected` and recorded in the retained report. The owner validates the materialized target image catalog before endpoint mutation for patch-bundle and transport-sidecar events.
- `ApplyBundle` represents AWFB patch-bundle bytes. Full-bundle replacement remains `RestartWithBundle`.
- `NativePatchEndpoint::patch_bytes_from_transport_json_bytes` is added so the owner can reuse the endpoint's transport JSON validation without duplicating private envelope parsing.

## Tests added

- `scene_windowed::tests::native_scene_state_stores_runtime_owner_not_session_catalog_pair`
- `scene_windowed::tests::after_render_submitted_boundary_is_after_surface_present_returns`
- `windowed_runtime::tests::unsafe_boundaries_do_not_pop_queued_owner_events`
- `windowed_runtime::tests::invalid_patch_report_leaves_previous_session_and_image_catalog_observable`
- `windowed_runtime::tests::content_only_patch_refreshes_image_catalog_at_safe_boundary`

## Validation status

The package was applied to the local Arcweft checkout together with seq03.6.
The following validation passed after adapting the package to the current
`winit` and runtime-driver APIs:

```bash
cargo fmt --all -- --check
cargo check -p arcweft-runtime-driver -p arcweft-player-native --all-targets --all-features
cargo test -p arcweft-player-native --lib patch_endpoint --all-features -- --nocapture
cargo test -p arcweft-player-native --lib scene_windowed --all-features -- --nocapture
cargo test -p arcweft-player-native --lib windowed_patch --all-features -- --nocapture
cargo test -p arcweft-player-native --lib windowed_runtime --all-features -- --nocapture
cargo clippy -p arcweft-runtime-driver -p arcweft-player-native --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
just test-workspace
```

`scene_windowed` currently has no matching unit tests for the filter name, so
that command confirms the library test binary builds. A real `winit`/GPU smoke
fixture remains part of seq03.7.

The `(1)` package re-application on 2026-06-28 confirmed the production wiring
was already present, then restored the two source-level `scene_windowed` tests
listed above without copying the older overlay over newer main-branch fixes.

## Non-goals

- Filesystem watching, socket servers, network fetch, AWFR trust verification, and release publication.
- Visible debug overlay/logging UI.
- Real window/GPU smoke fixtures; those remain seq03.7.
