# Agent Observe Player Path Convergence - 2026-07-01

## Scope

This cut makes the visible output of `arcw agent observe` use the same player
frame path as native and web player rendering. The shared owner is
`arcweft-player-scene::frame::PlayerFramePlanner`, which builds a `RenderScene`
from a `BundleSession` presentation snapshot, lowers runtime text controls,
plans focus, and prepares the `PreparedFrame` consumed by native, web, and
agent observation capture.

## Removed Legacy Observation Path

The old flow-event observation runner is removed from the native agent path.
Normal CLI observe, MCP observe/action/wait/step-frame flows, and native
AgentScript sessions now keep `NativeAgentRuntimeState` around a
`BundleSession` rather than a separate pure runtime executor plus CLI-specific
presentation projection.

The following older observation-only paths are intentionally gone:

- `run_agent_observation` and `AgentObservationRun*`
- CLI-side runtime-call image observation parsing
- source-image decode cache used only by the old observation projection
- observe-only rich-text child object decomposition from flow events

Image frames are now captured from the prepared player frame. Text input
controls, dialogue, choices, and product images therefore share player layout,
scale, focus, and capture behavior with the visible player surface.

## Boundary Notes

`arcweft-runtime-driver::BundleSessionStep` now exposes the runtime observation
state needed by agent reports. This keeps logs, signals, metrics, events, and
action availability tied to the same runtime step that produced the player
presentation snapshot.

Agent action requests remain typed Agent protocol inputs, then enter the same
`BundleSession` input queue used by player-backed observation. This cut does
not add pointer replay or native OS event synthesis; it only removes the
separate visual-observation renderer.

## Verification

The primary manual smoke command is:

```bash
cargo run -p arcweft-cli --features native-capture -- agent observe samples/native-text-input/src/main.arcw --json --image png --out target/ui-debug/native-text-input-observe-shared-player-path.png --mode drain --steps 8 --max-ops 64
```

Expected evidence:

- JSON report status is `ok`.
- JSON objects include `jp_text_field`, `jp_text_area`, and
  `secret_secure_field`.
- The emitted PNG is a native renderer capture of the prepared player frame.

## Current Validation

Validated on 2026-07-01:

```bash
cargo check -p arcweft-cli -p arcweft-player-scene -p arcweft-player-native -p arcweft-player-web -p arcweft-runtime-driver --features native-capture --all-targets
cargo run -p arcweft-cli --features native-capture -- agent observe samples/native-text-input/src/main.arcw --json --image png --out target/ui-debug/native-text-input-observe-shared-player-path.png --mode drain --steps 8 --max-ops 64
cargo clippy -p arcweft-cli -p arcweft-player-scene -p arcweft-player-native -p arcweft-player-web -p arcweft-runtime-driver --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
cargo test -p arcweft-cli --features native-capture --lib agent_observe -- --nocapture
git diff --check
```

The broader `cargo test -p arcweft-cli --features native-capture agent_observe --all-targets`
filter was attempted and timed out after 184 seconds. The narrower library
filter above passed and keeps the observe-specific unit coverage in this cut
bounded.
