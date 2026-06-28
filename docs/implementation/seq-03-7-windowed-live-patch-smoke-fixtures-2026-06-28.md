# Seq-03.7 Windowed Live-Patch Smoke Fixtures (2026-06-28)

## Summary

This overlay adds deterministic smoke coverage and regeneration support for the
windowed native live-patch path. It validates a running `WindowedRuntimeOwner`
through the same typed event path used by `scene_windowed.rs`: a patch event is
enqueued as `WindowedPatchEvent`, and mutation is drained only at
`FrameBoundary::AfterRenderSubmitted`.

The first smoke harness is a deterministic event-loop model rather than a
manual native player command. It simulates the native scene shell identities
(window, renderer shell, input controller, visual clock, prepared frame) while
using the real `WindowedRuntimeOwner`, `BundleSession`, `BundleImageCatalog`,
patch materialization, and typed boundary drain. This keeps CI deterministic and
avoids treating manual visual inspection as acceptance evidence.

## Design decisions

- **First smoke path:** deterministic event-loop harness. A fully automated
  `winit`/GPU window smoke is still platform-sensitive and will be better
  attached to the seq03.8 ingress adapter once there is an external typed patch
  source for a real live window. The current harness proves the owner boundary,
  runtime state, and renderer-facing image catalog without requiring a local
  window server.
- **Preservation observable:** every smoke report snapshots a synthetic but
  stable shell identity tuple: `window_shell_id`, `renderer_shell_id`,
  `input_controller_id`, and `visual_clock_id`. It also records
  `presented_frames`, `prepared_frame_valid`, active generation, current fiber
  generation, retired generation count, active content root, patch report,
  presentation text, presentation image count, and a direct RGBA image-catalog
  probe.
- **Outcome/report snapshot fields:** `WindowedRuntimeOutcome` snapshots include
  outcome kind, generation, compatibility, content root, rejection source, and
  rejection message. `WindowedPatchReport` snapshots include state, source,
  message, and compatibility.
- **Owned enum behavior:** stable labels and outcome accessors are added as
  inherent methods on Arcweft-owned boundary types instead of local test helpers.
- **Fixture staleness control:** all AWFB bundles, patch bundles, malformed patch
  bytes, manifest JSON, and expected smoke reports are generated in one pass by
  `tools/regenerate-windowed-live-patch-fixtures.rs`. The default mode is
  `--check`; `--apply` rewrites the full related set and removes stale managed
  generated files.

## Implementation contents

- `crates/arcweft-player-native/tests/fixtures/windowed_live_patch/src/*.arcw`
  repository-owned source fixtures for base, content-only target,
  code-generational target, restart-required target, wrong-base source, and
  pending-task code-generational fixtures.
- `crates/arcweft-player-native/tests/support/windowed_live_patch_fixtures.rs`
  deterministic fixture builder, AWFB/patch generator, smoke harness, and JSON
  report model.
- `crates/arcweft-player-native/tests/windowed_live_patch_smoke.rs`
  integration tests for compatibility classification, content-only refresh,
  code-generational commit, pending-task generation preservation,
  restart-required session restart, wrong-base rejection, malformed rejection,
  and regeneration file completeness.
- `tools/regenerate-windowed-live-patch-fixtures.rs` Rust script for check/apply
  fixture regeneration.
- `crates/arcweft-player-native/src/windowed_patch.rs` and
  `crates/arcweft-player-native/src/windowed_runtime.rs` small observable API
  additions, supplied as `overlay/patches/0001-windowed-live-patch-observable-labels.patch`.

## Smoke cases

### Content-only

Base and target bundles differ in display text and a 1x1 PNG asset while keeping
the executable flow graph unchanged. The smoke expects:

- `WindowedRuntimeOutcome::Applied` with compatibility `content-only`.
- active image catalog probe changes from red `[255, 0, 0, 255]` to blue
  `[0, 0, 255, 255]`.
- shell identities are preserved.
- the old foreground fiber remains valid at commit, and a fresh foreground entry
  on the committed active generation observes the target presentation text.

### Code-generational

The target changes the default foreground executable from dialogue+choice to a
return-only flow. The smoke expects:

- `WindowedRuntimeOutcome::Applied` with compatibility `code-generational`.
- active generation changes.
- the existing foreground fiber remains on the old generation at commit.
- starting a new foreground entry binds to the committed target generation. The
  fixture's replacement entry returns immediately, so the smoke records the new
  entry generation as deterministic evidence instead of requiring a live fiber
  pin after completion.

### Code-generational pending task

The base fixture requests a host task before patch commit. The target replaces
the foreground entry. The smoke expects:

- task generation stays pinned to the old generation after commit.
- the new foreground entry starts on the active target generation.
- old task generation is released after the task completion is delivered.

### Restart-required

The target intentionally removes image bundle sections from the base bundle. The
patch classifies as `restart-required`, and the native endpoint takes its
restart path. The smoke expects:

- `WindowedRuntimeOutcome::Restarted` with compatibility `restart-required`.
- active content root changes to the target root.
- shell identities are preserved where policy allows.

### Wrong-base and malformed

The wrong-base patch is generated from an unrelated base bundle and offered to a
running session whose active base is `base.arcw`. The malformed fixture is raw
invalid bytes. Both cases expect:

- `WindowedRuntimeOutcome::Rejected`.
- retained `WindowedPatchReport` state is `rejected`.
- active content root and direct image-catalog probe are unchanged.

## Regeneration command

```bash
cargo +nightly -Zscript tools/regenerate-windowed-live-patch-fixtures.rs --check
cargo +nightly -Zscript tools/regenerate-windowed-live-patch-fixtures.rs --apply
```

`--check` is the default and fails when any generated file is missing or stale.
`--apply` writes all bundles, patches, `reports/fixture-manifest.json`, and
per-smoke expected report JSON files in one deterministic pass.

## Platform validation matrix

| Platform | Deterministic harness | Real local window/GPU smoke | Notes |
| --- | --- | --- | --- |
| Windows | Expected supported in CI without a window server. | Requires a desktop session and a WGPU backend such as DX12/Vulkan. | The harness does not create a `winit` window, so it avoids focus, DPI, and swapchain timing variance. |
| macOS | Expected supported in CI without a window server. | Requires a foreground-capable app context and Metal. | Real `winit`/Metal smoke should remain local/manual or gated until an automation-safe app lifecycle is available. |
| Linux Wayland | Expected supported in CI without compositor. | Requires Wayland compositor, GPU, and surface creation support. | This cut does not prove compositor-specific resize/focus events. |
| Headless CI | Expected supported. | Not required. | The deterministic harness is the acceptance path; no software WGPU surface is assumed. |

## Validation status for this package build

Applied and validated in the Arcweft checkout on 2026-06-28:

```bash
cargo +nightly -Zscript tools/regenerate-windowed-live-patch-fixtures.rs --apply
cargo +nightly -Zscript tools/regenerate-windowed-live-patch-fixtures.rs --check
cargo fmt --all -- --check
cargo test -p arcweft-player-native --test windowed_live_patch_smoke --all-features -- --nocapture
cargo test -p arcweft-player-native --lib windowed_patch --all-features -- --nocapture
cargo test -p arcweft-player-native --lib windowed_runtime --all-features -- --nocapture
cargo clippy -p arcweft-player-native --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
just test-workspace
```

The regeneration script carries the same `glyphon` crates.io patch as the
workspace because `cargo +nightly -Zscript` resolves its frontmatter as a
standalone package.

## Design deviations

No intentional deviation from the seq03.7 acceptance boundary. Two smoke
expectations were made more precise during application:

- content-only patch commit refreshes the active image catalog immediately, but
  the existing foreground fiber keeps its current presentation until a fresh
  foreground entry is explicitly started on the committed generation;
- the code-generational replacement entry returns immediately, so the post-start
  snapshot records a completed foreground rather than a still-pinned fiber.

Both are existing runtime semantics rather than new compatibility behavior.

## Non-goals

- No network fetch, release trust verification, filesystem watcher, local socket,
  or runtime-driver transport is added.
- No manual visual inspection is required for acceptance.
- No seq03.8 ingress adapter is implemented here.
- No platform-specific `winit`/GPU automation is claimed as proven by this cut.
