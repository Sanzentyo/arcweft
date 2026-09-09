# Frame time sampling — 2026-09-09

Inspected base: `a8db9a37c35b9d5c32ff0e9d0ce56ce06c57cd11`, existing `main`,
pushed and clean before this cut. Implementation and validation below describe
the dirty cut on that base. Supersedes the unresolved zero-pixel Ruby capture
in the [prepared text ownership record](2026-09-09-prepared-text-owner-projection.md).

## Cause and final authority

The native viewport showed the speaker name while the entire dialogue body,
including Ruby, remained hidden. The CLI selected a 60,000 ms visual sample,
but shared text preparation used the newly emitted stage's retained elapsed
time, which was still zero. Scope selection and readback succeeded; the image
correctly contained no revealed body pixels. This was a frame-input defect.

`PlayerFrameTime` replaces the separate image and visual millisecond fields
on `PlayerFrameRequest`. Runtime mode retains each stage's logical elapsed
time while using the host visual clock for image/View animation. Sample mode
uses one exact elapsed value for reveal, stage Fx and the other visual inputs
without advancing the runtime snapshot. Millisecond construction rejects
nanosecond overflow instead of wrapping or silently truncating it.

Sample mode accepts the existing text-model `DialogueRevealPolicy`. It
preserves explicit semantic completion and allows a full-scene export to
request complete text at its selected effect time. The driver owns semantic
completion; its reveal evaluation and the renderer consume the same policy.
The renderer combines that policy with the existing reduce-motion setting.
An inactive history entry is completed only within its own presentation. The
global latest-dialogue shortcut, which completed unrelated active dialogue,
is removed.

Native windows, live Web playback and runtime-stepped captures use Runtime.
Agent observation and Web parity sampling use Sample. The standalone scene
export keeps its complete-text intent; the text-parity script retains its
actual runtime stepping and records that logical clock. Both scripts migrate
to the existing prepare-candidate/publication-guard API. The Web-only inset
shadow fixture also supplies the required empty event binding inventory.

The [maintained capture contract](../04-tooling/agent-observe-capture-contract.md)
now states live versus sampled timing and presentation-local completion.
No serialized shape, contract version, runtime clock interpretation, stack
limit or Cargo job count changes in this cut.

## Validation

Logs and generated artifacts are local and ignored under
`.arcweft-local/validation/2026-09-09-frame-time-sampling/`.

| Command | Result |
| --- | --- |
| `cargo test -p arcweft-player-scene --lib --tests` | 125 passed in 10 reports; final run 34.90 s. Includes exact sample overflow, reversible Ruby reveal with unchanged layout/runtime, unrelated active dialogue, existing geometry publication and input tests |
| `cargo test -p arcweft-runtime-driver --lib dialogue` | 1 passed; 83.60 s including rebuild |
| `cargo test -p arcweft-render-wgpu --lib geometry::dialogue_prepared::tests` | 8 passed; 51.13 s including rebuild |
| `cargo check -p arcweft-player-web --target wasm32-unknown-unknown --all-features` | Passed after the event inventory correction and disk recovery; final 1.96 s |
| `cargo +nightly -Zscript check --manifest-path tools/capture-bundle-scene-frame.rs` | Passed after publication API migration; final 2.53 s |
| `cargo +nightly -Zscript check --manifest-path tools/capture-text-parity-frame.rs` | Passed after publication API migration; final 1.61 s |
| `cargo fmt --all` | Passed; 10.51 s |
| `rustfmt +nightly --edition 2024 tools/capture-bundle-scene-frame.rs tools/capture-text-parity-frame.rs` | Passed; 0.60 s |
| `cargo check --workspace --all-targets --all-features` | Passed with existing warnings; final 51.46 s |
| `cargo clippy --workspace --all-targets --all-features` | Passed with existing warnings; final 56.07 s. New default-trait-access warnings were corrected |
| `just test-workspace` | 856 passed / the same 18 callable failures in 84 reports; 367.23 s after cache recovery. The recipe stopped in compiler `callable_execution`; its later workspace/CLI targets were not run |
| `just test-doc` | 95 reports / 8 passed, no failures; 121.67 s |
| `just test-tier2` | Failed after MCP 4 passed and native observe 1 passed; 65.35 s. Exact failure and skipped stages below |
| `cargo test -p arcweft-cli --features native-capture --test check typewriter_ruby_capture_time_controls_ -- --nocapture` | 0 passed / 4 failed in CLI subprocesses; 14.53 s. Stack overflow before frame preparation |
| Canonical structural audit with `--fail-on-blocking` | 95 packages, 2,250 Rust files, 309 review triggers, 0 blocking violations; final 2.26 s |
| Documentation links and `git diff --check` | Passed; 3 documents / 37 local link targets, anchors not checked |

Direct native viewport capture of the original combined-dialogue fixture also
succeeded. Visual inspection of the 1280x720 PNG confirmed `Hello`, Ruby base
`夢` and annotation `ゆめ`, which were absent before the timing correction.
The report retained the 60,000 ms sample metadata. The capture uses the CLI
binary built by this cut's native validation; it is diagnostic evidence, not
an exact visual-golden comparison.

The exhaustive Tier 2 run passed MCP's 4 tests and the original ignored native
observe test, including Ruby image URI readback. It then stopped in
`agent_observe_read_uri_preserves_animated_image_object_frame_metadata`:
the current parser rejects the first declaration in `samples/image-animation.arcw`.
The later auxiliary, visual-golden and proof targets were not run by that
fail-fast recipe. This does not establish complete Tier 2 conformance.

Four additional native typewriter/Ruby capture tests failed in their CLI
subprocesses with Windows main-thread stack overflow. Their mask and object-ID
assertions were not reached. These remain failures to diagnose and repair;
the shared prepared-frame tests do not replace those pixel-level obligations.
LLDB reproduced exception `0xc00000fd` at entry to
`Analyzer::prepare_content_call_application`, during nested Content expression
evaluation. Disassembly measured a 70,256-byte local frame for that function
and 28,104 bytes for its dialogue dispatcher in the inspected CLI binary.
This is separate from the previously corrected BTree call-node insertion.

The first WebAssembly check failed on the fixture's missing `events` field;
the later check passed after that typed consumer was repaired. Both standalone
tools initially failed on the removed planner `prepare` method; both now
compile through guarded publication. The final workspace checks preceded only
those standalone-tool changes, and the tool checks and structure audit were
rerun afterward. Browser playback and standalone-tool pixel exports were not
run; the matching native pixel/URI test and shared planner behavior provide
this cut's executable render evidence. Vendored glyphon is unchanged.

## Ownership and structure

The table gives exact final measurements on the dirty base above. Crate paths
are relative to `crates/`; their first component names the owning crate.
`P` means production, `T` test and `Tool` repository tool. Embedded test LOC
belong to the corresponding production owner.

| Path | Class | Base → final LOC | Bytes | Embedded test LOC |
| --- | --- | --- | --- | --- |
| `arcweft-cli/src/app/agent/native/player_observation.rs` | P | 1113 → 1120 | 39300 | 0 |
| `arcweft-player-native/src/dev_capture.rs` | P | 618 → 618 | 23798 | 264 |
| `arcweft-player-native/src/scene_windowed/frame_cycle.rs` | P | 457 → 456 | 18341 | 0 |
| `arcweft-player-scene/src/frame/time.rs` | P | 0 → 108 | 3775 | 27 |
| `arcweft-player-scene/src/frame/view_geometry/tests.rs` | T | 562 → 561 | 20081 | 0 |
| `arcweft-player-scene/src/frame/view_text.rs` | P | 799 → 799 | 28405 | 0 |
| `arcweft-player-scene/src/frame.rs` | P | 941 → 943 | 32670 | 0 |
| `arcweft-player-scene/tests/dialogue_view.rs` | T | 368 → 485 | 17793 | 0 |
| `arcweft-player-scene/tests/scroll_regions.rs` | T | 792 → 783 | 29453 | 0 |
| `arcweft-player-scene/tests/view_geometry_transaction.rs` | T | 165 → 164 | 5658 | 0 |
| `arcweft-player-web/src/app.rs` | P | 951 → 950 | 35607 | 0 |
| `arcweft-player-web/src/inset_shadow_exact_capture.rs` | P | 793 → 793 | 28362 | 0 |
| `arcweft-player-web/src/parity.rs` | P | 302 → 306 | 11103 | 0 |
| `arcweft-render-wgpu/src/geometry/dialogue_prepared/tests.rs` | T | 733 → 736 | 26287 | 0 |
| `arcweft-render-wgpu/src/geometry/dialogue_prepared.rs` | P | 730 → 730 | 26428 | 0 |
| `arcweft-render-wgpu/src/geometry.rs` | P | 2162 → 2162 | 72137 | 0 |
| `arcweft-runtime-driver/src/dialogue.rs` | P | 476 → 482 | 14540 | 0 |
| `tools/capture-bundle-scene-frame.rs` (repository root) | Tool | 262 → 268 | 9842 | 0 |
| `tools/capture-text-parity-frame.rs` (repository root) | Tool | 624 → 623 | 22139 | 0 |

Workspace dependency fan-in/out, followed by development fan-in/out, is:
CLI 0/53, 0/3; native 1/22, 0/5; scene 3/13, 0/4; Web 0/16, 0/5;
render-wgpu 5/8, 0/5; runtime-driver 6/13, 2/4. No workspace dependency edge,
feature or crate is added. The standalone scene script explicitly depends on
the existing text-model policy, following its existing local-path manifest
convention outside workspace package inheritance.

Player-scene owns the frame timing intent and its projection into existing
image, View and text inputs. Driver owns retained semantic completion;
text-model owns reveal evaluation; render-wgpu owns prepared render requests.
The `geometry.rs` size trigger is reviewed: this cut replaces one existing
request field with its owning typed policy and adds no state, dependency or
unrelated responsibility to that descriptor module. Its text-stage algorithm
and tests already reside in `geometry/dialogue_prepared`. No API was widened
to enable a physical split. The new timing module is one cohesive frame-input
boundary, and its integration tests exercise actual prepared glyphs and
presentation isolation. No duplicate clock reader or alternate reveal model
is retained. Generated audit data remains in the ignored validation directory.

## Cache recovery

During final validation, D: reached zero free bytes. A native library archive
and a WebAssembly query cache failed with OS error 112. Measurement found
136.75 GiB in native incremental cache, 121.87 GiB in native dependency outputs
and 0.81 GiB in WebAssembly incremental cache. The validation batch was stopped
while checking the first standalone tool; its later gates were not run.

Automatic approval review rejected direct recursive cache removal, reporting
only a policy block; no paths were removed by that command. The Cargo dry run
for `arcweft-bundle`, `arcweft-lang-sema` and `arcweft-lang-hir` identified
20,351 generated files / 78.0 GiB. The authorized package-scoped
`cargo clean -p arcweft-bundle -p arcweft-lang-sema -p arcweft-lang-hir` then
removed those outputs. The earlier full-workspace clean was not repeated.
Source and Git state were retained, and validation restarted with free space.

## Remaining acceptance

The image-animation source failure, typewriter source stack overflow, recovered
closure candidate admission, existing callable failures and the full
Match/View/task-plan/nominal/scheduler sequence remain required by the
[active convergence goal](2026-09-08-convergence-goal-plan.md). None is waived
by this frame-input correction. The existing reveal and Fx contracts are
preserved; there is no design deviation or external design blocker.
