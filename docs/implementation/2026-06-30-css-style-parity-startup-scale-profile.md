# css-style-parity startup and scale profile

> **Superseded (2026-07-13):** Arcweft now has a native-only typed Style path. The CSS authoring, Takumi adapter, and CSS-named sample/tooling paths described below were removed; the remaining text is retained only as historical implementation evidence.

Date: 2026-06-30

## Scope

This note records the `css-style-parity.awfb` startup profile, the native/web
visual parity failure mode, and the implementation cut that closes the visible
box/text scale mismatch.

## Findings

- The sample DSL was not the cause of the scale mismatch.
- Seq06-era viewport scale data was present in `RenderViewport::scale_factor`.
- Rectangle/image geometry already reached the renderer in logical viewport
  units and was converted against the physical surface size.
- Text rendering used a glyphon viewport in physical pixels, but `TextArea`
  origin, scale, and bounds were still sent in logical pixels. That made text
  the only layer that drifted under HiDPI/device-scale captures.
- The web parity smoke also captured screenshots with Playwright `scale: "css"`.
  That produced CSS-sized PNGs for HiDPI checkpoints instead of device-pixel
  PNGs matching the native offscreen capture.
- Web dialogue typewriter time was wall-clock-relative during screenshot
  capture, while the native fixture used an explicit visual time. The smoke now
  uses a deterministic test hook so the same text state is compared.

## Startup profile

Warm profile from this checkout before the final script addition:

| Step | Elapsed |
| --- | ---: |
| bundle css-style-parity warm | 1426 ms |
| build player web wasm warm | 942 ms |
| native capture warm default | 1801 ms |
| native capture warm compact | 1564 ms |
| native capture warm hidpi | 1737 ms |
| web capture warm default compact hidpi | 9354 ms |

The reusable profiler is now:

```bash
just css-style-parity-profile
```

It writes:

```text
target/css-style-parity/startup-profile.json
```

First successful script run after this change:

| Step | Status | Elapsed |
| --- | --- | ---: |
| bundle css-style-parity | ok | 38556 ms |
| build player web wasm | ok | 8986 ms |
| wasm-bindgen player web | ok | 23061 ms |
| native capture default | ok | 1991 ms |
| native capture compact | ok | 1581 ms |
| native capture hidpi | ok | 1646 ms |
| web capture default compact hidpi | ok | 5371 ms |

That run included a rebuild of `arcweft-cli` dependencies, so the bundle step is
not the steady-state parser/compiler cost. The steady-state bundle phase itself
reported 13 ms for compile, 3 ms for AWFB encode, and less than 1 ms for write
inside the CLI progress output.

## Implementation

- `arcweft-render-wgpu` now scales glyphon `TextArea` origin, scale, and bounds
  by `RenderViewport::scale_factor`.
- `web/tests/text-parity-smoke.mjs` captures web screenshots in device
  pixels and validates default, compact, and HiDPI checkpoints.
- `arcweft-player-web` exposes a deterministic visual-time override for the
  parity harness, matching the native capture's explicit visual time.
- `just css-style-parity` now captures and verifies default, compact, and HiDPI
  native/web PNGs and writes `imq` metric reports.
- `arcw bundle`, `arcw check`, `arcw build`, and `arcw compile` now emit
  Cargo-like phase progress to stderr while preserving JSON stdout.

## Latest visual parity metrics

`just css-style-parity` passes with:

| Checkpoint | Size | PSNR | SSIM | MSE | MAE | Changed pixel ratio |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| default | 1280x720 | 25.6187 | 0.6072 | 0.002742 | 0.004373 | 0.010041 |
| compact | 960x540 | 24.1292 | 0.5519 | 0.003864 | 0.006190 | 0.014091 |
| hidpi | 1280x720 | 19.9455 | 0.4469 | 0.010126 | 0.014629 | 0.027088 |

The HiDPI images are visually aligned after the scale fix. The remaining metric
gap is dominated by native/web text antialiasing and glyph raster differences,
not by box/text layout drift.

## Follow-up boundary

No request is needed for viewport scale itself; the missing implementation was
in the renderer and is fixed in this cut.

If Arcweft needs stricter native/web text pixel identity than the current visual
golden tolerance, use:

- `docs/reviews/requests/2026-06-30-seq-06.10-cross-backend-text-raster-parity-package.md`

That follow-up must not redesign the viewport scale contract or the existing
CSS-style sample. It should focus on glyph raster metrics, font loading,
subpixel positioning, and text-specific visual evidence.
