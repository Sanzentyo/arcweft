# Seq06.10 cross-backend text raster parity implementation

> **Superseded (2026-07-13):** Arcweft now has a native-only typed Style path. The CSS authoring, Takumi adapter, and CSS-named sample/tooling paths described below were removed; the remaining text is retained only as historical implementation evidence.

Date: 2026-06-30

## Source request

- `docs/reviews/requests/2026-06-30-seq-06.10-cross-backend-text-raster-parity-package.md`

## Substrate treated as already implemented

This cut consumes the existing CSS-style parity route as substrate:

- `RenderViewport::scale_factor` is the viewport scale contract.
- Native offscreen capture already renders the CSS-style sample through the
  shared renderer.
- The browser WebGPU capture already uses the shared Web player path.
- `samples/css-style-parity/main.arcw` remains the fixture source.
- `just css-style-parity` already captures default, compact, and HiDPI native/Web
  PNGs and writes full-image `verify-webgpu-parity` and `imq` reports.

Seq06.10 does not redesign any of the above. It adds text-specific evidence for
the remaining antialias/font/raster gap.

## Implemented overlay

### 1. Native frame evidence

`overlay/tools/capture-css-style-parity-native-frame.rs` replaces the existing
script with an evidence-producing version.

Changes:

- reads the configured font bytes once and registers those exact bytes with
  `SharedOffscreenCapture`;
- writes `target/css-style-parity/native-<checkpoint>.frame.json` unless
  `--no-frame-report` is supplied;
- records checkpoint, visual time, font path, font byte length, FNV-1a64 hash,
  viewport, rectangle/image/text/choice counts, and per-text-run typed evidence;
- keeps file I/O in the tool layer rather than renderer/data crates.

The report is intentionally path-bearing and fixture-specific. It is not a
renderer command stream.

### 2. Web frame/font evidence

`overlay/web/tests/css-style-parity-smoke.mjs` replaces the existing CSS-style
Playwright smoke test.

Changes:

- adds `ARW_CSS_STYLE_PARITY_FONT_URL`, defaulting to
  `./assets/arcweft-demo.ttf`;
- passes the font URL to `index.html?bundle=...&font=...`, which the existing web
  player already consumes;
- reads the same font file from the served fixture root and records byte length
  and FNV-1a64 hash;
- writes `target/css-style-parity/web-<checkpoint>.frame.json` with the existing
  `arcweft.web_frame_observation.v2` fields plus checkpoint, visual time, and
  font evidence;
- preserves the existing assertions that the sample uses the canvas renderer and
  not DOM buttons/text.

### 3. Text-raster verifier

`overlay/tools/verify-text-raster-parity.rs` is a new `cargo +nightly -Zscript`
tool.

It compares:

- native/Web PNG dimensions;
- PNG dimensions against frame viewport physical size;
- native/Web typed viewport evidence;
- native/Web text run count and order;
- text content, bounds, font size, line height, and RGBA;
- color-affinity text masks extracted from the PNGs within typed text bounds;
- mask XOR, ink coverage, bounding box delta, and centroid delta.

It writes:

```text
target/css-style-parity/text-raster-<checkpoint>.json
```

The tool also provides:

```bash
cargo +nightly -Zscript tools/verify-text-raster-parity.rs --self-test
```

The self-test creates small in-memory images and verifies that matching typed
text evidence with near-identical text-colored rectangles passes.

### 4. Justfile recipe wiring

`patches/0001-css-style-parity-text-raster-recipe.patch` updates
`css-style-parity` so default, compact, and HiDPI native captures emit native
frame JSON, the Web smoke pins the same font URL, and each checkpoint runs the
new text-raster verifier after full PNG and `imq` checks.

The existing full-image thresholds remain unchanged.

When seq06.10a styled paragraphs are applied, those thresholds are no longer
sufficiently aligned with the longer styled paragraph fixture. This checkout
keeps the thresholds strict and records the resulting failure as seq06.10b input
instead of loosening them in place.

### 5. Fixtures and documentation

`overlay/tests/fixtures/css_style_text_raster/evidence-checkpoints.json`
records the expected checkpoint inputs and default text-raster thresholds.

`docs/design/seq-06.10-cross-backend-text-raster-parity.md` records the stable
contract, evidence model, threshold ownership, font pinning, subpixel policy,
non-goals, and CI posture.

## Apply order

From the target Arcweft checkout root:

```bash
cp -R path/to/package/overlay/tools/. tools/
cp -R path/to/package/overlay/web/tests/. web/tests/
cp -R path/to/package/overlay/tests/fixtures/. tests/fixtures/
git apply path/to/package/patches/0001-css-style-parity-text-raster-recipe.patch
```

Then inspect the diff before running validation:

```bash
git diff -- tools/capture-css-style-parity-native-frame.rs tools/verify-text-raster-parity.rs web/tests/css-style-parity-smoke.mjs Justfile tests/fixtures/css_style_text_raster
```

## Expected artifacts after `just css-style-parity`

```text
target/css-style-parity/native-default.png
target/css-style-parity/web-default.png
target/css-style-parity/parity-default.json
target/css-style-parity/imq-default.json
target/css-style-parity/native-default.frame.json
target/css-style-parity/web-default.frame.json
target/css-style-parity/text-raster-default.json

target/css-style-parity/native-compact.png
target/css-style-parity/web-compact.png
target/css-style-parity/parity-compact.json
target/css-style-parity/imq-compact.json
target/css-style-parity/native-compact.frame.json
target/css-style-parity/web-compact.frame.json
target/css-style-parity/text-raster-compact.json

target/css-style-parity/native-hidpi.png
target/css-style-parity/web-hidpi.png
target/css-style-parity/parity-hidpi.json
target/css-style-parity/imq-hidpi.json
target/css-style-parity/native-hidpi.frame.json
target/css-style-parity/web-hidpi.frame.json
target/css-style-parity/text-raster-hidpi.json
```

## Validation commands

Target checkout validation:

```bash
cargo +nightly -Zscript tools/verify-text-raster-parity.rs --self-test
just css-style-parity
just css-style-parity-profile
cargo check -p arcweft-render-wgpu -p arcweft-player-web --all-features
cargo clippy --workspace --all-targets --all-features
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
```

Optional focused Web syntax check, before running Playwright:

```bash
node --check web/tests/css-style-parity-smoke.mjs
```

## Thresholds

The full-image CSS-style thresholds remain in `Justfile` and
`tools/verify-webgpu-parity.rs`.

The new default text thresholds live in `tools/verify-text-raster-parity.rs`:

| Field | Value |
| --- | ---: |
| `layout_milli_tolerance` | `0` |
| `max_bbox_delta_px` | `2.0` |
| `max_centroid_delta_px` | `1.25` |
| `max_coverage_delta_ratio` | `0.15` |
| `max_mask_xor_ratio` | `0.45` |
| `min_ink_pixels` | `4` |
| `ink_affinity_threshold` | `0.35` |

Promotion should tighten these only after collecting stable evidence across the
three CSS-style checkpoints on the intended Windows/browser/WebGPU environment.

## Structural audit note

This package adds one new cargo-script tool and updates one existing cargo-script
tool plus one Playwright harness. It does not add a crate, dependency edge,
public renderer API, or renderer/data crate I/O.

Approximate package-local file sizes measured in the zip assembly sandbox:

| Path | Bytes | Physical LOC | Classification |
| --- | ---: | ---: | --- |
| `overlay/tools/verify-text-raster-parity.rs` | 35164 | 1089 | tool cargo-script |
| `overlay/tools/capture-css-style-parity-native-frame.rs` | 15716 | 456 | tool cargo-script |
| `overlay/web/tests/css-style-parity-smoke.mjs` | 9386 | 286 | Playwright test harness |

Because the change touches rendering evidence, pixel processing, browser capture,
and tool orchestration, the target checkout should still run the repository
structural audit before promotion.

## Remaining boundaries

- No exact cross-backend text pixel identity is claimed.
- No checked-in PNG goldens are added.
- No new browser-only rendering path is introduced.
- No renderer/data crate file I/O is introduced.
- Stable CI enablement remains gated on available WebGPU/browser/font behavior.
- Styled paragraph line/glyph evidence and full-image threshold closure after
  seq06.10a are tracked by:
  `docs/reviews/requests/2026-06-30-seq-06.10b-styled-paragraph-raster-evidence-closure.md`
