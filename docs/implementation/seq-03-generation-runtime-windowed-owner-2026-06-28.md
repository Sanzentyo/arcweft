# Seq-03 Generation Runtime and Windowed Owner Implementation (2026-06-28)

Source package:
`D:/sanze/Downloads/arcweft-seq03-generation-runtime-windowed-live-patch-package.zip`

This note records the production application of the seq03.1 through seq03.4
overlay package. The applied cut covers the generation runtime table,
generation-bound host task dispatch, mixed-generation `BundleSession`
commit/new-entry behavior, and the first `WindowedRuntimeOwner` API.

## Applied Scope

- Added `arcweft-runtime-driver::generation_runtime` with
  `GenerationRuntimeImage` and `GenerationRuntimeTable` keyed by
  `GenerationId`.
- Added `GenerationId` to `HostTaskDispatch`, plus `BundleSession`
  introspection for the active fiber generation and outstanding task
  generation.
- Changed supported code-generational swaps so the old active fiber continues
  on its old executor while the committed active generation becomes the binding
  target for new entries.
- Added `BundleSession::restart_active_entry_on_current_generation` as the
  first single-active-entry API for rebinding the foreground entry to the latest
  committed generation.
- Added generation runtime image pruning tied to the `SwapSession` retire
  lifecycle. The applied implementation explicitly releases table-only retired
  runtime images before `SwapSession::retire_unused`, so the runtime table's
  `Arc<ProgramGeneration>` does not itself keep otherwise unused generations
  alive.
- Added `arcweft-player-native::windowed_runtime::WindowedRuntimeOwner`, which
  owns the native patch endpoint, image catalog, and typed patch queue and
  processes at most one patch event at `FrameBoundary::AfterRenderSubmitted`.
- Added `NativePatchEndpoint::options` so windowed restarts reuse the endpoint's
  existing `BundleSessionOptions` rather than duplicating options in the owner.

## Package Deviations

- The package patch files applied in order. The initial check of seq03.3 against
  a clean checkout failed because seq03.1/seq03.2 had not yet been applied; after
  applying the dependency slices, seq03.3 applied cleanly.
- `GenerationRuntimeImage::generation_id` was changed from `const fn` to `fn`.
  Stable Rust does not allow dereferencing `Arc<ProgramGeneration>` inside a
  const function.
- `WindowedRuntimeOwner::restart_from_bundle_bytes` was adjusted to read
  `BundleSessionOptions` from `NativePatchEndpoint::options`.
- Runtime image retire accounting was corrected during validation. Without the
  correction, the runtime table's own `Arc<ProgramGeneration>` prevented
  code-compatible old generations from retiring after task pins were released.
- The package README mentions a small `scene_windowed.rs` integration hunk, but
  the actual seq03.4 patch only added `windowed_runtime.rs` and its module
  declaration. The event-loop scene integration remains a follow-up request.
- The package fixtures are review examples only, not full AWFB binaries. They
  were not copied into runtime fixtures in this cut.

## Follow-Up Requests

- `docs/reviews/requests/2026-06-28-seq-03.5-windowed-runtime-owner-scene-integration-package.md`
- `docs/reviews/requests/2026-06-28-seq-03.6-multi-entry-generation-runtime-api-package.md`
- `docs/reviews/requests/2026-06-28-seq-03.7-windowed-live-patch-smoke-fixtures-package.md`

Seq03.5 and seq03.6 can be designed in parallel because seq03.5 is primarily
`arcweft-player-native` event-loop ownership while seq03.6 is primarily
`arcweft-runtime-driver` multi-entry runtime API design. Apply them sequentially
to `main`, with seq03.5 first unless seq03.6 explicitly changes the
`BundleSession` API that the scene owner must consume. Seq03.7 should be
designed in parallel if desired, but its implementation should wait for the
seq03.5 scene integration.

## Non-Goals In This Cut

- No filesystem watcher, local socket, release fetcher, or network transport
  was added to `arcweft-runtime-driver`.
- No release publish/trust verifier dependency was introduced.
- `scene_windowed.rs` still owns the live `BundleSession` and
  `BundleImageCatalog` directly; it is not yet wired to `WindowedRuntimeOwner`.
- No GPU/window smoke test was added for an actual running `winit` live-patch
  path.
- `restart_active_entry_on_current_generation` remains the first single-entry
  API and is not yet a general multi-entry/multi-fiber scheduler.

## Validation

Commands run after applying and adjusting the package:

```bash
cargo fmt --all -- --check
cargo check -p arcweft-runtime-driver -p arcweft-player-native --all-targets --all-features
cargo test -p arcweft-runtime-driver --lib generation_runtime --all-features
cargo test -p arcweft-runtime-driver --lib task --all-features
cargo test -p arcweft-runtime-driver --test session pending_task_pin_survives_code_compatible_runtime_rebuild --all-features -- --nocapture
cargo test -p arcweft-runtime-driver --test session code_generational --all-features -- --nocapture
cargo test -p arcweft-runtime-driver --test session hot_swap --all-features -- --nocapture
cargo test -p arcweft-player-native --lib windowed_patch --all-features -- --nocapture
cargo test -p arcweft-player-native --lib windowed_runtime --all-features -- --nocapture
cargo clippy -p arcweft-runtime-driver -p arcweft-player-native --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
just test-workspace
git diff --check
```

Results:

- Focused runtime-driver and player-native validation passed.
- `cargo test -p arcweft-player-native --lib windowed_runtime --all-features`
  passed but matched `0` tests. Direct owner tests remain part of seq03.5.
- Structural audit scanned `1629` files, `895` Rust files, and `433930` Rust
  physical LOC with `0` errors and `107` warnings.
- `just test-workspace` and `git diff --check` passed at the reviewable cut
  point.
