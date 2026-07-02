# seq-06.10b styled paragraph raster evidence closure — implementation note

Date: 2026-07-01

## Summary

This implementation closes the report/verifier gap for renderer-owned styled paragraphs. It enriches native and Web frame JSON with renderer-prepared line boxes, glyph/cluster bounds, source byte ranges, reveal state, effective style/color, font metrics, and typed glyph-transform metadata. The text-raster verifier now compares styled paragraph glyph evidence directly instead of expanding every span to the full paragraph bounds.

## Changed files in the overlay patch

- `crates/arcweft-render-wgpu/src/renderer.rs`
  - Adds `StyledParagraphEvidenceFontContext` for native/tool-side evidence extraction without I/O in renderer crates.
  - Adds `SharedRenderer::frame_styled_paragraph_layout_evidence` for Web report generation using the already registered renderer font system.
  - Extends `StyledParagraphLayoutEvidence` with paragraph bounds, default style, visible end, line boxes, per-glyph evidence, transform spans, and explicit transform support.
  - Keeps transform rendering out of this cut and records metadata-only unsupported evidence.

- `crates/arcweft-player-web/src/report.rs`
  - Promotes frame report schema to `arcweft.web_frame_observation.v3`.
  - Adds serializable style, line, glyph, and transform evidence fields.
  - Makes `from_prepared_frame(frame, paragraph_evidence)` require renderer-owned paragraph evidence and return a typed error when counts diverge.

- `crates/arcweft-player-web/src/app.rs`
  - Generates paragraph evidence from `SharedRenderer` after GPU/font initialization and before render submission.
  - Emits the v3 frame observation with the evidence payload.

- `tools/capture-css-style-parity-native-frame.rs`
  - Uses `StyledParagraphEvidenceFontContext` and `serde_json` to write v3 native frame JSON.
  - Keeps file writes in the tool layer.

- `tools/verify-text-raster-parity.rs`
  - Parses v3 paragraph line/glyph evidence.
  - Flattens styled paragraph glyph/cluster evidence into diagnostic raster runs.
  - Rejects styled paragraph reports that omit renderer-owned line/glyph evidence instead of falling back to span bounds.
  - Records paragraph index, line index, byte range, reveal state, visibility, and source kind in the output report.
  - Adds a styled paragraph self-test.

- `tools/run-css-style-parity-gates.rs`
  - New Rust script gate runner that executes all text and full-image gates before failing.

- `Justfile`
  - Replaces early-failing CSS-style gate commands with the gate runner.

- `web/tests/css-style-parity-smoke.mjs`
  - Expects v3 frame report schema and asserts line/glyph evidence exists.

- `crates/arcweft-render-wgpu/tests/styled_paragraph.rs`
  - Adds focused tests for reveal-state and transform metadata evidence.

- `crates/arcweft-player-web/tests/parity.rs`
  - Updates stale frame-report expectations to styled-paragraph semantics.
  - Adds a serialization test for v3 paragraph evidence.

## Architecture notes

The renderer crate remains Sans I/O. The new evidence context accepts font bytes already loaded by caller tools/adapters and does not read paths or write reports. Native and Web both consume the same renderer-owned extraction path; only serialization is adapter-specific.

The Web crate still does not depend on glyphon directly. It calls `SharedRenderer::frame_styled_paragraph_layout_evidence`, so renderer font registration stays single-sourced.

## Validation status in this package

- Source inspection: performed through the GitHub connector.
- Patch generation: completed in this package.
- Zip structural validation: completed with `unzip -t`.
- Cargo compile/test/clippy/WebGPU validation: not executed in this sandbox because the repository could not be cloned over HTTPS (`Could not resolve host: github.com`) and the target checkout/assets were not present.

See `verification/VALIDATION.md` for the exact commands to run in a real checkout.

## Local application validation

Applied on the local checkout after the package patch was ported to current `main`.

- `cargo test -p arcweft-render-wgpu --test styled_paragraph --all-features`: pass.
- `cargo test -p arcweft-player-web --test parity --all-features`: pass.
- `cargo +nightly -Zscript tools/verify-text-raster-parity.rs --self-test`: pass.
- `cargo build -p arcweft-player-web --target wasm32-unknown-unknown`: pass.
- `cargo check -p arcweft-render-wgpu -p arcweft-player-web --all-features`: pass.
- `cargo clippy -p arcweft-render-wgpu -p arcweft-player-web --all-targets --all-features -- -D warnings`: pass.
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`: pass with `0 error(s), 127 warning(s)`.
- `git diff --check`: pass.

`just css-style-parity` was executed after one transient Windows file-lock retry on `target/debug/arcw.exe`. It generated native/Web PNGs, frame JSON, text-raster reports, full-image parity reports, and IMQ reports for `default`, `compact`, and `hidpi`, then failed because the newly strict evidence gates expose current native/Web text raster drift:

| checkpoint | text runs | failed text runs | max mask xor | max bbox delta px | max centroid delta px | max coverage delta | full-image PSNR | full-image SSIM |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| default | 223 | 223 | 1.000000 | 56 | 56.002 | 1.000000 | 22.360 | 0.461 |
| compact | 127 | 127 | 0.988806 | 45 | 24.255 | 0.977636 | 20.260 | 0.431 |
| hidpi | 104 | 104 | 1.000000 | 54 | 26.724 | 0.925714 | 19.818 | 0.430 |

This is a remaining parity defect, not a fallback path. The reports under `target/css-style-parity/` now show the renderer-owned glyph/line evidence needed to debug the drift. The next implementation slice should align the native and Web styled-paragraph shaping/raster scale inputs, or intentionally split per-backend raster tolerance only after proving the typed layout evidence is identical.

Follow-up request: [`docs/reviews/requests/2026-07-03-seq-06.10c-styled-paragraph-raster-drift-closure.md`](../reviews/requests/2026-07-03-seq-06.10c-styled-paragraph-raster-drift-closure.md).
