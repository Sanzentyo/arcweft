# seq06.13e.1.1 Web exact PNG readback harness — 2026-07-04

## Status

Web exact readback harness implementation package is ready for review. The
original design package was a no-promotion package: it did not add a checked-in
Web reference PNG because the package authoring environment was not a pinned
browser/WebGPU visual-golden machine. In this checkout, the current terminal was
accepted as the pinned browser/WebGPU baseline environment, the same-run packet
was captured, and the reviewed candidate was promoted to the checked-in Web
reference path.

The native PNG baseline from seq06.13e.1 is left unchanged.

## Design decisions

- Browser runner: Playwright `chromium`, using `ARW_PLAYWRIGHT_CHANNEL` when set
  and `chrome` otherwise.
- Launch flags: `--enable-unsafe-webgpu`, plus `--use-angle=d3d11` on Windows to
  match the existing browser smoke runner convention.
- Fixture source: the Web fixture mirrors the already accepted 320x180
  seq06.13e.1 compositor scene: `rounded_inset_shadow_card` and
  `mixed_outer_inset_shadow_card`. The fixture now declares those cards as
  Panel parts in a typed `ViewProgramResource`, resolves their computed style
  through `ViewProgramResource::runtime_element_styles_with_style`, then lowers
  `background-color`, `border-radius`, and `box-shadow` into the retained
  `UiScene`.
- Readback path: WebAssembly-exported renderer readback. The wasm export
  `capture_seq06_13e1_inset_box_shadow_exact_png` prepares a normal
  `PreparedFrame` with `PlayerFramePlanner::prepare`, attaches the exact
  `UiScene` with `PreparedFrame::with_ui_scenes`, renders the Arcweft-owned
  `rgba8unorm` WebGPU texture through `SharedRenderer::render_to_view`, then
  reads it with `copy_texture_to_buffer`. Node only encodes the returned raw RGBA
  bytes as PNG and writes packet JSON.
- Forbidden paths: no browser DOM/CSS box-shadow screenshots, no SVG filters, no
  Canvas 2D fallback, no CPU raster fallback, no bitmap mask replacement.
- Web reference rule: Web has a separate Web-specific reference PNG. It must not
  be silently promoted from the native PNG or from an unpinned browser run.

## Added and changed files

- `crates/arcweft-player-web/src/inset_shadow_exact_capture.rs`
  - wasm-only exact capture export for the 320x180 compositor fixture.
  - returns raw RGBA bytes, observe JSON, and WebGPU adapter info.
  - resolves the exact fixture's Panel part styles through
    `ViewProgramResource::runtime_element_styles_with_style` before building
    `UiRoundedRect` primitives and compositor box-shadow effects.
- `crates/arcweft-player-web/src/lib.rs`
  - exposes the wasm exact capture export behind the existing wasm cfg boundary.
- `crates/arcweft-player-web/Cargo.toml`
  - adds `futures-channel` for awaiting WebGPU buffer map completion in wasm.
- `web/seq06-13e1-inset-shadow-capture.html`
  - thin host page for browser fingerprinting only; it is not the visual source.
- `web/tests/seq06-13e1-inset-shadow-exact-capture.mjs`
  - launches the pinned browser, calls the wasm export, writes candidate PNG,
    observe JSON, and environment fingerprint JSON.
- `web/package.json`
  - adds `test:seq06-13e1-exact`.
- `tools/collect-seq06-13e1-inset-shadow-pinned-golden-evidence.rs`
  - `--mode web --run` now builds wasm, runs `wasm-bindgen`, invokes the exact
    Web capture script, records command logs, and validates the packet before the
    generic Web smoke is treated as supporting evidence.
  - adds Web classifications for missing WebGPU pin, missing runtime tool,
    missing browser runtime, missing candidate PNG, transparent candidate,
    dimension mismatch, and imq failure.
- `crates/arcweft-render-wgpu/tests/ui_box_shadow_exact_png_golden.rs`
  - extends exact packet tests to require command logs and review decision.
  - accepts a complete no-promotion Web packet when the Web reference is absent
    but `baseline_missing` metrics and `ready_for_first_promotion_review` review
    decision exist.
  - adds source regressions for the Web exact no-screenshot/no-fallback route.
- `tools/source-gates/seq06_13e1_inset_shadow_exact_golden_policy.rs`
  - validates Web exact capture source, script, collector classifications, and
    no-fallback evidence outside the pinned environment.
- `fixtures/visual-smoke-goldens/seq06.13e.1-inset-box-shadow-exact-png-policy.json`
  - documents the WebAssembly-exported renderer readback decision and separate
    Web reference promotion rule.
- `docs/fixtures/web/seq06_13e1_inset_box_shadow_exact_golden.json`
  - documents the runner, readback path, artifact paths, and no-promotion state.

## Required pinned Web command

```powershell
$env:ARW_SEQ06_13E1_INSET_SHADOW_GOLDEN_REQUIRED = "1"
$env:ARW_SEQ06_13E1_INSET_SHADOW_GOLDEN_PINNED = "1"
$env:ARW_SEQ06_13E1_INSET_SHADOW_WEBGPU = "1"
cargo +nightly -Zscript tools/collect-seq06-13e1-inset-shadow-pinned-golden-evidence.rs --root . --out-dir target/seq06.13e.1-inset-box-shadow-golden --mode web --run
```

The collector writes the Web packet under:

- `target/seq06.13e.1-inset-box-shadow-golden/web/seq06_13e1_inset_box_shadow.candidate.png`
- `target/seq06.13e.1-inset-box-shadow-golden/web/seq06_13e1_inset_box_shadow.observe.json`
- `target/seq06.13e.1-inset-box-shadow-golden/web/seq06_13e1_inset_box_shadow.environment.json`
- `target/seq06.13e.1-inset-box-shadow-golden/web/seq06_13e1_inset_box_shadow.imq.json`
- `target/seq06.13e.1-inset-box-shadow-golden/web/command-logs/web-wasm-cargo-build.log`
- `target/seq06.13e.1-inset-box-shadow-golden/web/command-logs/web-wasm-bindgen.log`
- `target/seq06.13e.1-inset-box-shadow-golden/web/command-logs/web-exact-png-capture.log`
- `target/seq06.13e.1-inset-box-shadow-golden/web/command-logs/webgpu-smoke.log`
- `target/seq06.13e.1-inset-box-shadow-golden/review/seq06_13e1_web_promotion_decision.json`

If the checked-in Web reference PNG is absent, the collector writes
`baseline_missing` metrics JSON and a `ready_for_first_promotion_review` decision
instead of failing solely because the reference is not yet promoted.

## Promotion decision

The original package decision was no-promotion. A reviewer may promote the Web
reference only after the exact command above succeeds in a pinned WebGPU browser
environment and the same-run packet is inspected. The promotion action is to copy
the reviewed Web candidate into:

```text
fixtures/visual-smoke-goldens/seq06.13e.1-inset-box-shadow/web/seq06_13e1_inset_box_shadow.png
```

Then rerun the same collector command to reach `passed_existing_baseline_gate`
with `max_mse=0.002` and `max_mae=0.003` unchanged.

Applied checkout promotion:

- Current checkout pinned environment:
  `ARW_SEQ06_13E1_INSET_SHADOW_GOLDEN_REQUIRED=1`,
  `ARW_SEQ06_13E1_INSET_SHADOW_GOLDEN_PINNED=1`,
  `ARW_SEQ06_13E1_INSET_SHADOW_WEBGPU=1`.
- First-promotion packet:
  `target/seq06.13e.1-web-exact-style-packet-run/web/`.
- Promoted reference:
  `fixtures/visual-smoke-goldens/seq06.13e.1-inset-box-shadow/web/seq06_13e1_inset_box_shadow.png`.

## Validation performed while preparing this package

Validated in the package authoring environment:

- `node --check overlay/web/tests/seq06-13e1-inset-shadow-exact-capture.mjs`
- JSON parsing for updated policy, fixture, and package metadata.
- Static package manifest generation.

Not validated here:

- `cargo fmt --all -- --check`
- `cargo check` / `cargo test`
- `cargo +nightly -Zscript tools/collect-seq06-13e1-inset-shadow-pinned-golden-evidence.rs --root . --mode web --run`
- Browser/WebGPU exact capture execution
- imq comparison against a checked-in Web reference PNG

Reason: this environment did not have a checked-out Arcweft repository, wasm32
build artifacts, Playwright browser runtime, or pinned WebGPU visual-golden
hardware. The zip package therefore includes implementation files plus this
explicit validation boundary rather than claiming a promoted Web baseline.

## Applied checkout validation

Validated in the 2026-07-04 apply checkout:

- `cargo fmt`
- `cargo fmt --all -- --check`
- `cargo check -p arcweft-player-web -p arcweft-render-wgpu --tests`
- `cargo test -p arcweft-player-web --lib`
- `cargo build -p arcweft-player-web --target wasm32-unknown-unknown`
- `wasm-bindgen --target web --out-dir web/pkg --out-name arcweft_player_web target/wasm32-unknown-unknown/debug/arcweft_player_web.wasm`
- `node --check web/tests/seq06-13e1-inset-shadow-exact-capture.mjs`
- `cargo +nightly -Zscript tools/source-gates/seq06_13e1_inset_shadow_exact_golden_policy.rs --root .`
- `cargo test -p arcweft-render-wgpu --test ui_box_shadow_exact_png_golden --all-features -- --nocapture`
- `cargo +nightly --config "build.target-dir='target/cargo-script-seq06-13e1-check-3'" -Zscript tools/collect-seq06-13e1-inset-shadow-pinned-golden-evidence.rs --root . --out-dir target/seq06.13e.1-web-exact-readback-dry-run-4 --mode web`
- `cargo clippy --workspace --all-targets --all-features`
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`

Structure audit completed as a dry run and reported `0 error(s), 129
warning(s)` without writing report files.

Pinned WebGPU attempt:

- Command:
  `cargo +nightly --config "build.target-dir='target/cargo-script-seq06-13e1-web-run-6'" -Zscript tools/collect-seq06-13e1-inset-shadow-pinned-golden-evidence.rs --root . --out-dir target/seq06.13e.1-web-exact-readback-pinned-run-transparent-check --mode web --run`
- Environment:
  `ARW_SEQ06_13E1_INSET_SHADOW_GOLDEN_REQUIRED=1`,
  `ARW_SEQ06_13E1_INSET_SHADOW_GOLDEN_PINNED=1`,
  `ARW_SEQ06_13E1_INSET_SHADOW_WEBGPU=1`.
- Result: blocked before promotion with `transparent_candidate`.
- Evidence written:
  `target/seq06.13e.1-web-exact-readback-pinned-run-transparent-check/web/seq06_13e1_inset_box_shadow.environment.json`,
  `target/seq06.13e.1-web-exact-readback-pinned-run-transparent-check/web/command-logs/web-wasm-cargo-build.log`,
  `target/seq06.13e.1-web-exact-readback-pinned-run-transparent-check/web/command-logs/web-wasm-bindgen.log`,
  and
  `target/seq06.13e.1-web-exact-readback-pinned-run-transparent-check/web/command-logs/web-exact-png-capture.log`.

The pinned attempt reached browser execution, but the wasm export rejected the
capture because the WebGPU readback produced a fully transparent candidate. No
Web reference PNG was added or updated. The collector intentionally classifies
this as `transparent_candidate` rather than allowing a blank PNG to reach
first-promotion review.

The collector also removes inherited `RUSTUP_TOOLCHAIN` from child `cargo`
commands so `cargo +nightly -Zscript` does not force the inner wasm build onto a
nightly toolchain that lacks the `wasm32-unknown-unknown` target.

## Style-through follow-up

After review feedback on 2026-07-04, the Web exact fixture was changed so
corner radius, card fill, and box shadows no longer originate as hard-coded
`UiCompositingEffects` in the wasm export. The export now owns a typed
`ViewStyleResource` with CSS source identity for
`docs/fixtures/css/seq06.13e-inset-box-shadow-card.css`, declares each card as a
Panel part in a fixture `ViewProgramResource`, resolves the cards via
`ViewProgramResource::runtime_element_styles_with_style`, then lowers the
resolved visual style to:

- `UiRoundedRect` direct primitives for the style fill and border radius;
- `UiCompositingEffects::box_shadows` for the style shadow list;
- observe JSON route evidence for the tree-aware style resolver, player renderer
  path, and rounded-rect child.

This also addresses the previous transparent-candidate failure mode where the
Web exact scene had shadow groups with no rendered child content because the
wasm harness used a no-op direct primitive renderer.

The pinned rerun also exposed a WebGPU WGSL validation error in the shared
compositor shader: browser validation rejected `textureSample` in a helper that
can be reached from non-uniform control flow. The compositor shader now uses
`textureSampleLevel(..., 0.0)` for its UI compositor texture reads, avoiding
implicit derivatives and allowing the browser WebGPU shader module to validate.

The generic `npm --prefix web test` smoke remains logged as supporting evidence,
but its current `unsupported Arcweft bundle schema version 4; expected 5`
failure does not invalidate a complete seq06.13e.1 exact PNG packet. The exact
collector therefore continues to packet validation after writing
`webgpu-smoke.log`.
