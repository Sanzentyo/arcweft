# WASM browser runtime debug — 2026-07-11

## Scope

This cut rebuilt `arcweft-player-web` for `wasm32-unknown-unknown`, regenerated
the browser glue and demo bundle, and exercised the result in a real
WebGPU-capable browser. It covers bootstrap, dialogue reveal/page/line advance,
choice input, letterboxed resize geometry, missing-WebGPU failure, and the Web
text-input dependency boundary.

## Defects reproduced and corrected

- Profile bundle generation loaded the complete project merely to obtain its
  package name. A launch-only manifest such as `web/arcw.toml` therefore failed
  by trying to enumerate the unrelated default `web/src` directory. Package
  identity now uses a metadata-only manifest load.
- The browser smoke test expected frame-observation schema `v1` although the
  current producer emits `v3`.
- Dialogue progression was capped at four Enter presses. That assumption did
  not represent reveal and page gates and timed out before choices were
  reached. The harness now uses a bounded state-driven loop.
- Choice geometry expectations laid out directly in the CSS canvas size. The
  renderer uses a 1280x720 reference viewport followed by aspect-fit scaling
  and letterbox translation. The test now applies that same public coordinate
  transform and allows only one milli-pixel for cross-language rounding.
- `[p]` had no browser-observable page state and the demo used it only at line
  ends. Web observation `v3` now exports the typed dialogue instance, stage,
  logical page index/count, wait state, and presentation transitions. The demo
  contains two visible pages in one dialogue line and the browser test proves
  that `[p]` changes the page without replacing the dialogue occurrence.
- `arcweft-player-web` depended on the native-oriented `arcweft-runtime-host`
  crate only for Sans-I/O text-input dispatch. That pulled runtime acceleration
  and Rayon into the WASM graph. The shared dispatch and lifecycle now live in
  `arcweft-player-text-input`, with the dependency direction
  `native/web -> player-text-input -> presentation`.
- The formerly disabled WASM workflow was briefly activated to validate its
  runner contract. GitHub Actions run `29154987754` showed that the workflow
  mixed portable/WASM validation with unprovisioned Linux native dependencies:
  the workspace job required EGL through `khronos-egl`, while the native CLI
  bundle step required ALSA through `alsa-sys`. Both failed before browser
  validation. The workflow has therefore been removed until native bundle
  tooling and target-specific validation are designed as separate jobs with
  explicit runner prerequisites.

## Remaining Fx execution gap

The rebuild also confirmed that the new source-level `#[fx] fn -> Fx` model is
not yet executable end to end in a browser:

- View `ApplyFx` instructions are retained by the codec but ignored by runtime
  control/style lowering.
- Fx definitions are not yet a first-class bundle section.
- RichText transform sampler closures are represented as source labels rather
  than executable typed expression IR.
- the WGPU glyph path implements the older built-in `wave`/`shake`/`jitter`
  descriptors, not arbitrary package-qualified Fx graphs.

Consequently this cut does not treat a successful WASM bootstrap as evidence
that custom dynamic Fx works. The executable-graph and identity work remains
specified in:

- `docs/reviews/requests/2026-07-11-seq-06.16.9.1-fx-executable-graph-and-renderer-abi.md`
- `docs/reviews/requests/2026-07-11-seq-06.16.9.2-fx-package-identity-and-cross-surface-resolution.md`

Until those contracts are implemented, browser acceptance must assert concrete
frame output rather than only `ready = true`.

## Verification evidence

The checkout was measured at Jujutsu change `wolsrqss` before description and
commit. Focused file measurements were:

| Path | Bytes | Physical LOC | Responsibility |
| --- | ---: | ---: | --- |
| `arcweft-player-text-input/src/lib.rs` | 583 | 14 | intentional crate facade |
| `arcweft-player-text-input/src/text_input_dispatch.rs` | 25,394 | 707 | typed platform-event validation |
| `arcweft-player-text-input/src/player_text_input_bridge.rs` | 20,667 | 598 | shared player focus lifecycle |
| `arcweft-player-web/src/report.rs` | 18,300 | 529 | typed runtime/frame observations |
| `arcweft-render-text/src/playback.rs` | 35,797 | 1,029 | dialogue display stages and validation |
| `arcweft-runtime-driver/src/dialogue.rs` | 7,335 | 267 | portable dialogue advance state |

The canonical structure audit scanned 2,546 files, including 1,191 Rust files
and 604,079 Rust physical LOC. It reported 0 errors and 149 existing warnings.
Detailed reports were written under
`target/structure-audit-wasm-debug-2026-07-11/`.

Commands and results:

- `cargo fmt --all -- --check`: passed.
- focused tests for `arcweft-player-text-input`, `arcweft-project-loader`, and
  `arcweft-render-text`: 13 + 89 + 20 passed.
- exact CLI launch-only manifest regression: passed.
- real profile bundle generation to `web/demo.awfb`: passed with four virtual
  files.
- `cargo build -p arcweft-player-web --target wasm32-unknown-unknown
  --all-features`: passed.
- target-specific Cargo dependency inventory: passed; WGPU/winit present and
  native host/renderer, JIT, Rayon, xcap, and enigo absent.
- `cargo test -p arcweft-player-web --all-features`: 35 passed.
- focused changed-crate clippy with all targets/features and `-D warnings`:
  passed.
- `npm.cmd --prefix web test`: seven real WebGPU cases passed, including the
  same-line page transition.
- cache-busted in-app browser launch: `ready=true`, no fatal or console error.
- Web IME contract/glue/geometry/bridge tests: passed. Its separate rendered
  smoke reported `environment_blocked` because that browser process exposed no
  WebGPU adapter; it did not install a DOM fallback.

## Withdrawn CI attempt

The removed `.github/workflows/wasm-browser.yml` must not be restored by merely
installing arbitrary system packages. A replacement needs two independent
contracts:

1. Build the bundle with a portable CLI feature set that does not pull audio,
   native windowing, EGL, or other platform adapters into the WASM job.
2. Run native all-feature coverage only on a runner whose EGL, ALSA, display,
   audio, and GPU prerequisites are intentional and documented.

The local fresh-WASM and real-browser evidence above remains valid; the failed
run was a workflow/runner-boundary failure, not a failure of the produced WASM
artifact.
