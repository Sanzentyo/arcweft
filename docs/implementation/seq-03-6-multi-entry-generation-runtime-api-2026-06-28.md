# Seq-03.6 Multi-Entry Generation Runtime API (2026-06-28)

Source request:
`docs/reviews/requests/2026-06-28-seq-03.6-multi-entry-generation-runtime-api-package.md`

This package replaces the first single-active-entry rebinding helper with a
typed foreground-entry start API. It keeps the current runtime-driver model as a
single foreground fiber and does not introduce a background scheduler.

## Public API Changes

- Added `BundleEntryStart` as the caller-supplied entry start request:
  - `SessionDefault` reuses the entry selected into the generation runtime image
    when the bundle generation was built from `BundleSessionOptions`.
  - `Entry(AwbcEntryId)` starts an explicitly resolved AWBC entry table id.
- Added `StartedForegroundEntry { generation, entry }` so callers can observe
  the active generation used by a fresh foreground entry.
- Added `BundleEntryStartError` with deterministic typed cases for unknown AWBC
  entry ids, non-flow entries, missing generation runtime images, and product
  AWBC executor construction failures.
- Added `BundleSession::start_foreground_entry_on_current_generation`.
- Removed `BundleSession::restart_active_entry_on_current_generation`; this
  package intentionally does not preserve a transitional wrapper.
- Added narrow runtime-image introspection on `BundleSession` for focused
  lifecycle tests: `runtime_image_count` and `has_runtime_image`.
- Added owned AWBC schema methods used by the runtime-driver selection rule:
  `AwbcFunctionKind::is_flow` and `AwbcEntryTarget::function`.

## Decisions

The minimal runtime-driver handle is `StartedForegroundEntry`. It records only
the `GenerationId` and `AwbcEntryId` for the foreground fiber that was just
started. It deliberately omits a scheduler token, fiber registry key, or
executor handle because the current production runtime still owns exactly one
foreground executor.

The API is named around `start_foreground_entry`, not `restart` or `bind`.
`start` reflects that a fresh foreground executor is created from the active
generation runtime image. `foreground` states the current single-fiber ownership
model. `on_current_generation` makes the binding rule explicit without adding a
caller-specified generation id that could race the committed active generation.

Missing runtime images are reported through `BundleEntryStartError::GenerationRuntime`
with the existing `GenerationRuntimeError::MissingGeneration { generation }`.
Unknown explicit entry ids are reported as `BundleEntryStartError::UnknownEntry`
with the typed `AwbcEntryId`. Route-set entries and entries whose target function
is not an AWBC flow are reported as `BundleEntryStartError::NonFlowEntry`.

The new entry reuses the generation runtime image's `BundleSessionOptions`-bound
entry for `SessionDefault`. The caller may supply only an explicit `AwbcEntryId`
for this cut. Runtime mode, step budget, and root bindings remain session
options and are not duplicated on every entry start. Pending input is cleared and
presentation is reset because the foreground executor has been replaced; host
task pins and task sequence allocation are not reset.

The API is compatible with future multi-fiber support because it exposes a small
typed start request and observable result without reserving scheduler
abstractions. A later scheduler can add a separate multi-fiber API or extend the
result with an actual fiber handle once runtime ownership exists.

## Runtime Semantics

- Code-generational commit behavior remains unchanged: the old foreground fiber
  continues to run on its old generation until it finishes or the host explicitly
  starts a new foreground entry.
- Starting a foreground entry replaces only the current foreground executor.
- Outstanding host task pins continue to retain their original generation and
  are released deterministically by `TaskSequence` when completion events arrive.
- Old generation runtime images remain while the old foreground fiber, task
  pins, explicit pins, or windowed handoff pins hold the generation. They are
  pruned once those pins are released.
- `ProgramGeneration` remains metadata only. Runtime images stay in
  `GenerationRuntimeTable<SessionRuntime>`.

## Tests Added Or Updated

- `entry_generation_start_binds_to_new_generation_after_code_generational_commit`
  verifies that a fresh foreground entry starts on `SwapSession.active_generation_id()`
  after a code-generational commit and returns that generation in
  `StartedForegroundEntry`.
- `entry_generation_start_prunes_replaced_old_fiber_runtime_image` verifies that
  the old foreground fiber keeps the old generation live before explicit
  replacement and that replacing it prunes the old runtime image when no other
  pins exist.
- `entry_generation_start_keeps_old_runtime_image_until_task_pin_releases`
  verifies that a pending host task from the old generation keeps the old runtime
  image live after a new foreground entry starts and releases it only when the
  task completion event arrives.
- `entry_generation_start_reports_invalid_entry_selection_deterministically`
  covers typed unknown-entry errors.
- `entry_generation_start_reports_non_flow_entry_selection_deterministically`
  covers route/non-flow entry rejection.
- `table_reports_missing_generation_deterministically` covers deterministic
  missing generation runtime-image lookup.
- The native patch endpoint unit test was migrated from the removed restart
  helper to `start_foreground_entry_on_current_generation`.

## Validation Status

The package was applied to the local Arcweft checkout together with seq03.5.
The following validation passed:

```bash
cargo fmt --all -- --check
cargo check -p arcweft-runtime-driver -p arcweft-player-native --all-targets --all-features
cargo test -p arcweft-runtime-driver --lib generation_runtime --all-features
cargo test -p arcweft-runtime-driver --test session entry_generation --all-features -- --nocapture
cargo test -p arcweft-runtime-driver --test session hot_swap_changed_structured_flow_keeps_current_fiber_on_old_generation --all-features -- --nocapture
cargo test -p arcweft-runtime-driver --test session pending_task_pin_survives_code_compatible_runtime_rebuild --all-features -- --nocapture
cargo test -p arcweft-player-native --lib patch_endpoint --all-features -- --nocapture
cargo test -p arcweft-player-native --lib windowed_runtime --all-features -- --nocapture
cargo clippy -p arcweft-runtime-driver -p arcweft-player-native --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
just test-workspace
```

Structural audit reported `1691` files, `912` Rust files, `437702` Rust
physical LOC, `0` errors, and `107` warnings.

## Non-Goals

- No background scheduler, fiber registry, or multi-flow policy is introduced.
- No product player, compiler, syntax, HIR, sema, verifier, CLI, GPU, or OS
  adapter dependency is added to `arcweft-runtime-driver`.
- No transitional wrapper is kept for the removed restart helper.
