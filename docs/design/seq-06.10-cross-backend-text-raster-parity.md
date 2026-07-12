# Seq06.10 cross-backend text raster parity design

Date: 2026-06-30

## Problem

CSS-style native/Web captures now line up geometrically, including default,
compact, and HiDPI viewport scale. Remaining parity drift is dominated by text:
font fallback, glyph rasterization, antialiasing, subpixel placement, and backend
font/shaping details can differ even when both backends consume the same shared
Arcweft renderer path.

A full PNG metric is still useful, but it cannot tell whether a failure is a
layout regression, a font mismatch, text antialiasing, or unrelated scene drift.
Seq06.10 therefore adds typed text evidence and mask-based raster evidence while
preserving the existing renderer path and visual metric checks.

## Contract

Renderer-owned native/Web text parity is defined as:

```text
text-mask/layout identity with backend-specific antialias allowance
```

The contract has three layers:

1. **Frame contract**: native and Web must report the same prepared text runs in
   the same order, with identical UTF-8 text, bounds, font size, line height, and
   RGBA values after milli-unit normalization.
2. **Font contract**: CSS-style parity uses the same checked-in font bytes on
   both sides. The evidence report includes byte length and FNV-1a64 hash so a
   run cannot silently fall back to an unpinned platform font.
3. **Raster contract**: PNG text masks extracted from typed text bounds must have
   bounded divergence in ink coverage, bounding box, centroid, and mask XOR.
   Antialiasing is allowed to differ within these bounds; missing glyph ink,
   shifted glyph runs, or different text content are failures.

Exact pixel identity is not required for cross-backend text. It remains a future
native-only or single-backend golden option, but it is not the production web
parity bar in this cut.

## Evidence model

### Existing full-image evidence

The existing CSS-style route continues to run `tools/verify-webgpu-parity.rs` and
`imq compare` for default, compact, and HiDPI captures. Those reports remain the
scene-wide smoke signal for total image quality.

Ownership:

- `tools/verify-webgpu-parity.rs` owns full PNG thresholds such as PSNR, SSIM,
  MSE, MAE, maxAE, and changed-pixel ratio.
- `imq` reports remain audit artifacts and promotion evidence.
- Seq06.10 does not lower the existing CSS-style full PNG thresholds.

### New typed layout evidence

The native capture cargo-script writes:

```text
target/css-style-parity/native-<checkpoint>.frame.json
```

The Web Playwright harness writes:

```text
target/css-style-parity/web-<checkpoint>.frame.json
```

The two JSON shapes intentionally share the fields consumed by the text-raster
checker:

```json
{
  "schema_version": "arcweft.css_style_native_frame_observation.v1",
  "checkpoint": "default",
  "visual_time_millis": 9000,
  "font": {
    "path": "web/assets/arcweft-demo.ttf",
    "byte_len": 0,
    "fnv1a64": "0000000000000000"
  },
  "viewport": {
    "logical_width_milli": 1280000,
    "logical_height_milli": 720000,
    "physical_width": 1280,
    "physical_height": 720,
    "scale_factor_milli": 1000
  },
  "text_count": 1,
  "text": [
    {
      "text": "CSS-like style parity",
      "bounds": {
        "x_milli": 0,
        "y_milli": 0,
        "width_milli": 100000,
        "height_milli": 30000
      },
      "font_size_milli": 29000,
      "line_height_milli": 39150,
      "rgba": [100, 210, 200, 255]
    }
  ]
}
```

The native schema name differs from the existing Web schema name so that the
producer is clear. The checker ignores producer-specific extra fields and
compares the shared evidence fields.

### New text-raster evidence

The companion tool writes:

```text
target/css-style-parity/text-raster-<checkpoint>.json
```

The report schema is:

```text
arcweft.text_raster_parity.v1
```

For each text run, the tool records:

- native and Web typed layout/style equality;
- physical pixel region derived from typed bounds and `scale_factor_milli`;
- native/Web ink pixel counts and coverage ratios;
- mask bounding boxes and centroids;
- mask XOR ratio across the union region;
- per-run failure reasons.

## Text mask extraction

For each typed text run, the tool computes a physical crop:

```text
logical bounds in milli-units -> logical px -> physical px via scale_factor_milli
```

A pixel is considered text ink when its RGB color is close enough to the run's
reported text RGBA. The default affinity threshold is `0.35`. This intentionally
measures text-shaped color evidence rather than exact alpha coverage, because the
GPU output is final composed RGBA.

Default thresholds:

| Metric | Default | Meaning |
| --- | ---: | --- |
| `layout_milli_tolerance` | `0` | typed layout/style must match exactly after milli normalization |
| `max_bbox_delta_px` | `2.0` | glyph ink bounds may drift by at most two physical pixels |
| `max_centroid_delta_px` | `1.25` | text run ink centroid may drift slightly from antialiasing |
| `max_coverage_delta_ratio` | `0.15` | one backend may shade more edge pixels than the other |
| `max_mask_xor_ratio` | `0.45` | antialiased edge masks may differ while shape remains aligned |
| `min_ink_pixels` | `4` | non-empty text must produce visible ink |

These values are intentionally separate from full-image PSNR/SSIM thresholds.
They should be tightened only after collecting stable Windows/browser evidence.

## Font pinning

CSS-style parity uses checked-in `web/assets/arcweft-demo.ttf`. The repository's
third-party notice identifies it as Noto Sans Regular under the SIL Open Font
License 1.1. This package does not include or duplicate the font bytes; it pins
the path used by the existing Arcweft checkout and fingerprints the bytes at run
time.

Native capture:

```bash
cargo +nightly -Zscript tools/capture-text-parity-frame.rs \
  --font web/assets/arcweft-demo.ttf
```

Web capture:

```powershell
$env:ARW_TEXT_PARITY_FONT_URL = "./assets/arcweft-demo.ttf"
node web\tests\text-parity-smoke.mjs
```

Platform fallback is allowed only outside CSS-style parity or when explicitly
recorded as a non-deterministic environment blocker. A text-raster report without
a font fingerprint should not be promoted as a stable parity baseline.

## Subpixel policy

Subpixel text positions are preserved, not snapped. The shared renderer should
continue to pass logical text origins and scale into glyphon according to the
existing viewport scale contract.

Snapping glyph positions would trade a measurable raster difference for a layout
behavior change. Seq06.10 instead records typed bounds in milli-units and derives
physical crops from those values. Any future snapping policy must be a separate
renderer behavior decision with focused tests and visual review.

## Threshold ownership

| Owner | Scope | Promotion rule |
| --- | --- | --- |
| `tools/verify-webgpu-parity.rs` | full PNG scene metrics | existing CSS-style thresholds remain active |
| `imq` | comparable full-image audit artifacts | evidence only unless a checked-in golden family is created |
| `tools/verify-text-raster-parity.rs` | text layout and mask evidence | owns text-specific thresholds and failure reasons |
| `Justfile` `css-style-parity` recipe | selected checkpoint policy | wires default, compact, HiDPI checks together |
| future golden checks | platform-specific baselines | may consume text-raster JSON but must not replace it silently |

## CI posture

The route is deterministic enough for local Windows validation. In CI, it should
remain disabled or environment-gated until stable WebGPU, browser channel, and
font behavior are available. Disabled CI can still archive the command lines and
expected artifact names from this package so promotion does not depend on hidden
local practice.

## Non-goals

- hidden DOM text rendering;
- canvas-overlay text rendering;
- browser-only style paths;
- exact cross-backend text pixel identity;
- new file I/O in renderer/data crates;
- redesigning viewport scale, native offscreen capture, web capture, or the
  CSS-style sample.
