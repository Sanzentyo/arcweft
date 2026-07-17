# Native physical box geometry production reconciliation

## Scope and source

This implementation follows the normative contract from
`arcweft-seq-06.11d.4.1.2.1-native-physical-box-geometry-production-reconciliation-final-contract(1).zip`.
The package SHA-256 is
`9B39DB1B342B899A885B08217815D4C6A10465BFE2672C31BDC9F5A34BDF8548`.
All 11 manifest entries were present and `OPEN_QUESTIONS.md` contained `none`.

The package inspected `cb6d787166f609d75385e14354bedf76fb16bd5a`. Work began
from `009e33b71839a6ef55a98be1c43e91d5976f81f3` and was finally integrated on
top of `fe130cf187a1d64e552d6c89bf63d248a21aa2b3`. The intervening View
exported-part, public View identity/catalog, callable, and hot-swap changes were
reviewed during rebase. They are preserved and do not create a second physical
occurrence, box, clip, or cache authority.

The implementation is Jujutsu change
`slmrzpyuytlszopplrowzpskwotzsyot`. Its final commit is the repository revision
that contains this note; the stable change ID is recorded here so later
rewrites remain traceable without embedding a stale self-referential commit ID.

No compatibility alias, dual reader, migration shim, CSS/Takumi route,
serialized geometry packet, source-spelling gate, or second geometry kernel is
introduced.

## Implementation result

Phases 1 through 8 of the contract are implemented.

1. `arcweft-view` now owns the exact milli-pixel box, transform, closed clip,
   consumer, and collision-proof revision substrate. `ViewStyleNodeKey` is the
   only physical occurrence identity. Explicit zero border boxes and positive
   edges follow the corrected BX-016 rule.
2. Bundle projection produces one transient
   `ViewRuntimePhysicalNodeStyle`. Element ownership, display participation,
   container flow, gaps, leaf restrictions, and product cardinality are
   validated before player preparation.
3. Runtime-driver performs the total View owner/path conversion and passes the
   combined packet without restoring layout/composite geometry fields.
4. Player-scene owns deterministic inventory, intrinsic measurement,
   postorder measurement, preorder placement, finalization, exact staged
   caches, signed scroll, consumer indexes, and the committed
   `Arc<ViewCommittedGeometryFrame>`.
5. WGPU final lowering, native/Web input and publication, headless capture,
   CLI/Agent observation, hit/focus/avoidance/scroll, and text/surface consumers
   use the committed geometry packet. Platform conversion is checked and
   preserves typed context; saturating/clamping geometry fallbacks were removed.
6. The old player layout authority and obsolete WGPU View authority were
   deleted:
   `frame/view_style/layout.rs`, `render-wgpu/src/view.rs`, its old tests, and
   its showcase example.
7. Candidate preparation and publication are atomic. Failed and stale
   candidates preserve the prior frame, cache generation, input state, and
   adapter state. Successful publication swaps the prepared frame and performs
   private adapter commit exactly once.
8. The integrated tree passed formatting, workspace check, all-feature clippy,
   the normal workspace test gate, focused physical/adaptor tests, the wasm
   build, browser smoke tests, and the canonical structural audit. The one full
   CLI integration failure was reproduced on unchanged `main` and is recorded
   below as an external baseline defect rather than hidden or attributed to
   this geometry change.

## Publication design deviation

The contract describes all fallible capture preparation as preceding an
exclusive publication guard, while also requiring a stale headless candidate
to perform zero GPU queue submission and zero readback staging. GPU readback
preparation itself submits work, so those requirements cannot both be met by a
single late guard.

The implementation uses `PlayerFramePublicationGuard::preflight_candidate` as
a generation lease. A stale candidate fails before any side-effecting capture
staging. A valid guard retains the planner's exclusive mutable borrow while the
fallible headless capture is prepared. Player state replacement still begins
only inside `publish_with`, which rechecks the generation and then performs the
infallible adapter commit. This preserves the required observable atomicity
without making queue submission part of a rollbackable player-state mutation.

Direct evidence is provided by:

- `tx_headless_preflight_rejects_stale_before_side_effecting_staging`;
- `tx_stale_generation_never_invokes_adapter_or_changes_input`;
- `tx_success_atomically_publishes_candidate_arc_and_invokes_adapter_once`.

## Validation on the integrated change

| Command | Result | Evidence |
|---|---|---|
| `cargo fmt --all -- --check` | PASS | integrated tree after all Rust edits |
| `cargo test -p arcweft-view --all-targets` | PASS | geometry and logical-axis owners |
| `cargo test -p arcweft-bundle --all-targets` | PASS | transient physical projection and validation |
| `cargo test -p arcweft-runtime-driver --all-targets` | PASS | 158 tests across runtime, save/session, hot-swap, and View integration targets |
| `cargo test -p arcweft-player-scene --all-targets` | PASS | deterministic walker, caches, publication, input, and consumers |
| `cargo test -p arcweft-render-wgpu --all-targets` | PASS | final lowering and renderer consumers |
| `cargo test -p arcweft-player-native --all-targets` | PASS | native checked conversion and publication |
| `cargo test -p arcweft-player-web --all-targets` | PASS | Web checked conversion, input, and parity after updating the obsolete authored-position oracle |
| `cargo check -p arcweft-player-web --target wasm32-unknown-unknown` | PASS | actual wasm target compiled |
| `npm test` in `web/` | PASS | 7/7 browser smoke cases, including resize, pointer, keyboard, canvas geometry, and missing-WebGPU handling |
| `cargo check --workspace --all-targets` | PASS | clean integrated workspace check |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS | clean integrated all-feature lint gate |
| `just test-workspace` | PASS | normal workspace integration gate |
| focused physical geometry/adaptor batch | PASS | 24 View geometry, 6 logical-axis, 4 bundle, 27 runtime-driver, 3 transaction, and three exact scroll/WGPU/Web parity tests |
| `cargo +nightly -Zscript tools/structure-audit.rs --root .` | PASS | 0 errors |

The first wasm attempt timed out after 124 seconds and exposed three stale View
identity uses in the wasm-only inset-shadow capture path. That attempt is not
counted as a pass. The path was converted to `ViewId`, `ViewProgramId`, and
`ViewDefinitionRef`; the exact wasm command then passed in 4.12 seconds.

`cargo test -p arcweft-cli --all-targets` reaches the existing
`agent_observe_native::agent_observe_image_alignment_sample_uses_authored_alignment_geometry`
fixture and fails before geometry preparation because the effect literal `0.5`
is resolved as runtime `I32`. The same exact test fails on unchanged
`fe130cf187a1`, proving that this is a pre-existing semantic/runtime baseline
defect. CLI library tests covering the changed native player observation owner
pass under `just test-workspace`.

A redundant combined all-target rerun after the clean gates exhausted the D:
drive and hit Windows PDB/linker limits. It produced no failing assertion. Once
space was restored, the focused physical/adaptor batch above was rerun
sequentially with incremental compilation disabled and passed in full. The
workspace target had already been fully cleaned once after accidental
cross-worktree artifact reuse was detected; generated `web/pkg` and
`web/node_modules` were removed after browser verification.

## Structural audit

The canonical audit passed with:

- files scanned: 3,168;
- Rust files: 1,597;
- Rust physical LOC: 732,021;
- Cargo manifests: 92;
- errors: 0;
- warnings: 129.

Machine-readable reports were written outside the workspace at
`D:/git/arcweft-structure-audit-physical/` as `violations.md`,
`file_metrics.csv`, `dependency_edges.csv`, and
`public_type_duplicates.csv`. No generated file and no Cargo manifest or lock
file was edited. `arcweft-view` remains Sans I/O and the workspace dependency
graph did not change.

Changed-file warning dispositions:

- `arcweft-render-wgpu/src/geometry.rs` (2,134 LOC): pre-existing size warning;
  new final lowering is split into `geometry/view_final.rs` (463 LOC).
- `arcweft-render-wgpu/src/renderer.rs` (1,620 LOC): pre-existing size warning;
  this change only adapts shared validation/publication preparation.
- `arcweft-render-wgpu/src/view_compositor.rs` (1,562 LOC): pre-existing size
  warning; this change is one typed mask-extent error path.
- `arcweft-runtime-driver/src/view_runtime.rs` (1,203 LOC): narrowly above the
  warning threshold; the change adds only the contract-owned total conversion,
  while the player tree walker remains outside this module.

Related crate dependency fan-out/fan-in is unchanged:

| Crate | Fan-out | Fan-in |
|---|---:|---:|
| `arcweft-view` | 8 | 12 |
| `arcweft-bundle` | 24 | 10 |
| `arcweft-runtime-driver` | 15 | 6 |
| `arcweft-player-scene` | 18 | 3 |
| `arcweft-render-wgpu` | 15 | 5 |
| `arcweft-player-native` | 34 | 1 |
| `arcweft-render-web` | 6 | 1 |
| `arcweft-player-web` | 30 | 0 |

### Changed Rust file metrics

Format: `path | bytes | physical LOC | classification`. `P` is production,
`T` is test, and `D` is deleted authority. Values are from the current
checkout; deleted files are listed separately because they no longer have a
current byte/LOC measurement.

```text
crates/arcweft-bundle/src/resource_codec.rs | 4398 | 85 | P
crates/arcweft-bundle/src/resource_codec/view/runtime_control_style.rs | 9860 | 289 | P
crates/arcweft-bundle/src/resource_codec/view/runtime_control_style/physical.rs | 8873 | 251 | P
crates/arcweft-bundle/src/resource_codec/view/runtime_control_style/projection.rs | 20388 | 541 | P
crates/arcweft-bundle/tests/runtime_control_style_resolution.rs | 11674 | 318 | T
crates/arcweft-cli/src/app/agent/native/player_observation.rs | 36345 | 1048 | P
crates/arcweft-cli/src/app/agent/native/runtime_observation.rs | 26131 | 650 | P
crates/arcweft-glyphon/src/text_engine.rs | 42318 | 1188 | P
crates/arcweft-player-native/src/dev_capture.rs | 19469 | 529 | P
crates/arcweft-player-native/src/scene_windowed.rs | 39922 | 1098 | P
crates/arcweft-player-native/src/scene_windowed/frame_cycle.rs | 18720 | 466 | P
crates/arcweft-player-native/src/scene_windowed/input_cycle.rs | 18894 | 470 | P
crates/arcweft-player-native/src/windowed_environment_ingress.rs | 25217 | 760 | P
crates/arcweft-player-scene/src/action_buttons.rs | 6712 | 180 | P
crates/arcweft-player-scene/src/fonts.rs | 7681 | 241 | P
crates/arcweft-player-scene/src/frame.rs | 32770 | 944 | P
crates/arcweft-player-scene/src/frame/surfaces.rs | 29976 | 857 | P
crates/arcweft-player-scene/src/frame/view_geometry.rs | 7401 | 199 | P
crates/arcweft-player-scene/src/frame/view_geometry/cache.rs | 8512 | 268 | P
crates/arcweft-player-scene/src/frame/view_geometry/conversion.rs | 11842 | 368 | P
crates/arcweft-player-scene/src/frame/view_geometry/error.rs | 32894 | 886 | P
crates/arcweft-player-scene/src/frame/view_geometry/finalize.rs | 13028 | 336 | P
crates/arcweft-player-scene/src/frame/view_geometry/intrinsic.rs | 8259 | 212 | P
crates/arcweft-player-scene/src/frame/view_geometry/measure.rs | 7450 | 194 | P
crates/arcweft-player-scene/src/frame/view_geometry/place.rs | 15432 | 431 | P
crates/arcweft-player-scene/src/frame/view_geometry/tests.rs | 20106 | 562 | T
crates/arcweft-player-scene/src/frame/view_geometry/tree.rs | 26541 | 786 | P
crates/arcweft-player-scene/src/frame/view_style.rs | 23665 | 706 | P
crates/arcweft-player-scene/src/frame/view_style/consumer.rs | 13392 | 370 | P
crates/arcweft-player-scene/src/frame/view_style/tests.rs | 45684 | 1293 | T
crates/arcweft-player-scene/src/frame/view_text.rs | 27834 | 787 | P
crates/arcweft-player-scene/src/input.rs | 34634 | 1048 | P
crates/arcweft-player-scene/src/input/scroll.rs | 14778 | 423 | P
crates/arcweft-player-scene/src/input/state.rs | 7326 | 204 | P
crates/arcweft-player-scene/src/input/tests.rs | 36791 | 1090 | T
crates/arcweft-player-scene/src/text_controls.rs | 11101 | 278 | P
crates/arcweft-player-scene/tests/dialogue_view.rs | 7860 | 211 | T
crates/arcweft-player-scene/tests/runtime_text_controls.rs | 21231 | 561 | T
crates/arcweft-player-scene/tests/scroll_regions.rs | 27441 | 741 | T
crates/arcweft-player-scene/tests/view_geometry_transaction.rs | 5696 | 166 | T
crates/arcweft-player-web/src/app.rs | 36109 | 967 | P
crates/arcweft-player-web/src/inset_shadow_exact_capture.rs | 28324 | 792 | P
crates/arcweft-player-web/tests/input.rs | 17252 | 464 | T
crates/arcweft-player-web/tests/parity.rs | 36774 | 1069 | T
crates/arcweft-render-web/src/web.rs | 8992 | 250 | P
crates/arcweft-render-wgpu/src/geometry.rs | 71086 | 2134 | P
crates/arcweft-render-wgpu/src/geometry/scroll.rs | 11532 | 345 | P
crates/arcweft-render-wgpu/src/geometry/view_final.rs | 15380 | 463 | P
crates/arcweft-render-wgpu/src/lib.rs | 684 | 22 | P
crates/arcweft-render-wgpu/src/renderer.rs | 57336 | 1620 | P
crates/arcweft-render-wgpu/src/renderer/prepared_text.rs | 13660 | 420 | P
crates/arcweft-render-wgpu/src/view_compositor.rs | 56116 | 1562 | P
crates/arcweft-render-wgpu/src/view_direct_renderer.rs | 37089 | 1095 | P
crates/arcweft-render-wgpu/tests/geometry.rs | 41291 | 1214 | T
crates/arcweft-runtime-driver/src/view_runtime.rs | 45944 | 1203 | P
crates/arcweft-runtime-driver/src/view_runtime/style_scope.rs | 13273 | 389 | P
crates/arcweft-runtime-driver/tests/view_runtime.rs | 88202 | 2452 | T
crates/arcweft-view/src/geometry.rs | 3031 | 69 | P
crates/arcweft-view/src/geometry/box_model.rs | 24681 | 769 | P
crates/arcweft-view/src/geometry/consumer.rs | 4696 | 139 | P
crates/arcweft-view/src/geometry/error.rs | 7407 | 240 | P
crates/arcweft-view/src/geometry/primitives.rs | 23563 | 759 | P
crates/arcweft-view/src/geometry/revision.rs | 21767 | 664 | P
crates/arcweft-view/src/program.rs | 34929 | 1037 | P
crates/arcweft-view/src/style/axis.rs | 22922 | 764 | P
crates/arcweft-view/src/style/computed.rs | 18765 | 540 | P
crates/arcweft-view/src/style/resolver.rs | 29285 | 799 | P
crates/arcweft-view/src/style/value.rs | 25794 | 923 | P
crates/arcweft-view/tests/geometry_contract.rs | 24095 | 723 | T
crates/arcweft-view/tests/logical_axis_cascade.rs | 17035 | 496 | T
```

Deleted Rust authority:

```text
crates/arcweft-player-scene/src/frame/view_style/layout.rs | D
crates/arcweft-render-wgpu/examples/view_interaction_showcase.rs | D
crates/arcweft-render-wgpu/src/view.rs | D
crates/arcweft-render-wgpu/tests/view.rs | D
```

## Remaining work

There is no remaining implementation or design work inside this physical-box
geometry contract. The unrelated CLI effect-literal baseline defect remains
visible above and should be repaired in its owning semantic/runtime task; no
geometry compatibility layer or test weakening is warranted for it.
