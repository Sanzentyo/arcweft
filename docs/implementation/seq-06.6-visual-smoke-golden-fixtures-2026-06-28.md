# Seq 06.6 visual smoke and golden fixtures implementation

Date: 2026-06-28

## Source requests

- `docs/reviews/requests/2026-06-28-seq-06.6-visual-smoke-golden-fixtures-package.md`
- seq06.5 selected capture resource metadata package, assumed applied first.

## Implemented overlay

This cut adds deterministic, non-exact native visual smoke coverage and tightens
exact-golden artifact reporting without requiring exact image comparison in the
normal fast path.

Implemented changes:

1. `crates/arcweft-cli/tests/check/agent_observe_native/visual_smoke.rs`
   introduces three `visual_smoke_*` tests:
   - full viewport, dialogue-layer, and selected textbox-object captures;
   - layer object-id and object mask raw RGBA debug attachments;
   - long-line overflow/wrap smoke under a small viewport.
2. `agent_observe_native.rs` includes the new test file after seq06.5's
   `selected_capture_metadata.rs`, so selected object/layer smoke tests assert
   the protocol-owned `image.selected_capture` schema from seq06.5 next to image
   dimensions and non-empty content.
3. `native_vertical.rs` exact golden comparison writes the `imq` stdout JSON to a
   metrics file beside each temporary candidate and includes reference,
   candidate, and metrics paths in failure messages.
4. `Justfile` gains `test-visual-smoke` and wires it into `test-cli-native` and
   `fixture-refresh-native-capture-check`; exact visual golden remains explicit
   through `test-visual-golden`, `native-visual-artifacts`, and `test-tier2`.
5. `docs/implementation/test-execution-policy.md`,
   `docs/implementation/fixture-regeneration.md`, and
   `tests/fixtures/native_capture/README.md` document the split between visual
   smoke and exact golden validation.

## Fixture classes

| Class | Implemented check | Exact pixels? | Validation tier |
| --- | --- | --- | --- |
| viewport smoke | full-frame PNG dimensions, renderer/scope/composition, non-empty content | no | Tier 1 native smoke |
| layer crop smoke | dialogue layer PNG crop, selected-capture metadata, crop bounds | no | Tier 1 native smoke |
| selected object crop smoke | textbox object PNG crop, selected-capture metadata, source role, crop bounds | no | Tier 1 native smoke |
| object-id/mask smoke | raw RGBA byte length, object-id color coverage, mask transparent/opaque coverage, selected-capture mask metadata | no | Tier 1 native smoke |
| text overflow smoke | long text wraps in a constrained viewport and preserves selected-capture coordinate basis | no | Tier 1 native smoke |
| exact visual golden | existing checked-in vertical native PNGs compared with `imq` bounded MSE/MAE | yes, bounded | Tier 2 / Windows + pinned font + `imq` |

## Exact golden policy

Checked-in exact goldens remain the current native Windows `MS Mincho` vertical
fixtures under `tests/fixtures/native_capture/`:

- `vertical_tutr_golden.png`
- `vertical_jlreq_preset_loose_golden.png`
- `vertical_jlreq_preset_normal_golden.png`
- `vertical_lr_ruby_text_combine_golden.png`

The all-in-one `vertical_goal_clear_smoke.arcw` remains a smoke source: tests
render PNG/raw candidates from it but do not check in a stable golden PNG.

Exact golden comparison is native-only in this cut because the existing checked
in goldens are native text-path fixtures. WebGPU parity continues to use the
existing `webgpu-parity` candidate-artifact route and should become a separate
fixture family only after a dedicated WebGPU golden package pins adapter, font,
texture format, and browser/native parity tolerances.

Pinned exact environment:

- OS/font: Windows with `MS Mincho` available.
- Renderer label: current native Agent observer / native rich-text observer.
- Viewport: 1280x720.
- Device pixel ratio / scale policy: raw 1.0 Agent capture path.
- Color format: PNG produced by `arcw agent observe --image png` from native
  capture.
- Metrics: `imq image <reference> <candidate> --metrics psnr,ssim,mse,mae,maxae
  --format json`.
- Tolerance: existing bounded full-reference policy, `mse <= 0.002` and
  `mae <= 0.003`; dimension equality remains mandatory.

## Apply order

The package patch series assumed seq06.5 was already applied:

```bash
git apply patches/0001-visual-smoke-selected-capture-tests.patch
git apply patches/0002-exact-golden-imq-artifact-reporting.patch
git apply patches/0003-visual-smoke-recipes-and-docs.patch
```

In the target checkout, the first two patches applied directly. The third patch
had one stale hunk in `docs/implementation/fixture-regeneration.md`; the same
content was manually ported into the current document structure.

## Verification status

Validation run in the target checkout:

```bash
cargo fmt --all -- --check
cargo test -p arcweft-cli --features native-capture --test check visual_smoke -- --nocapture
cargo test -p arcweft-cli --features native-capture --test check native_checked_in_visual_golden_fixtures_are_well_formed --quiet
just test-cli-native
cargo check -p arcweft-cli --features native-capture --test check
cargo clippy -p arcweft-cli --features native-capture --test check -- -D warnings
just native-visual-artifacts
just fixture-refresh-all
just test-workspace
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
```

`just native-visual-artifacts` produced candidate PNGs, observe JSON, and `imq`
JSON reports under `target/arcweft-native-capture-artifacts/`.
`just fixture-refresh-all` completed without adding unrelated tracked fixture
diffs.

The explicit Tier 2 exact command was also run:

```bash
just test-visual-golden
```

It failed on the existing `vertical_tutr_golden` exact comparison in this local
environment: the candidate and reference dimensions matched at `1280x720`, but
`imq` reported `mse = 0.0030918550895167305` and
`mae = 0.004233718228315644`, above the existing `mse <= 0.002` and
`mae <= 0.003` bounds. This is recorded as a Tier 2 visual drift, not a
seq06.6 smoke failure. The improved failure output included reference,
candidate, and metrics paths as intended.

The structural audit reported `0 error(s), 107 warning(s)` across `928` Rust
files and `445596` Rust physical LOC.

## Remaining boundaries

- This package consumes seq06.5 selected-capture metadata and does not redesign
  it.
- Exact selected object/layer golden PNGs are deliberately not checked in; their
  metadata and image-content smoke coverage is deterministic enough for Tier 1,
  while exact pixels remain environment-sensitive.
- Text-fit report publication is not redesigned here. The smoke test covers the
  current observable overflow/wrap behavior and selected-capture coordinate
  basis; a future text-fit-report publication cut can add exact JSON assertions
  to the same `visual_smoke_text_overflow_*` family.
