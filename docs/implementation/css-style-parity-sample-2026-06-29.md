# CSS Style Parity Sample - 2026-06-29

## Scope

Added an image-free Web/native parity sample for CSS-like Arcweft styling. The
sample is intentionally renderer-only: visible text, colors, size changes,
emphasis, ruby, textbox chrome, and canvas output come from Arcweft renderer
state rather than DOM game UI or image assets.

## Added Files

- `samples/css-style-parity/main.arcw`
- `samples/css-style-parity/README.md`
- `tools/capture-css-style-parity-native-frame.rs`
- `web/tests/css-style-parity-smoke.mjs`

`just css-style-parity` builds the sample bundle into ignored
`web/local/css-style-parity.awfb`, captures native offscreen PNGs, captures Web
canvas PNGs through Playwright/Chrome WebGPU, and compares the pairs with
`tools/verify-webgpu-parity.rs`.

## Validation

Executed from `D:/git/arcweft`:

```bash
just css-style-parity
```

Results:

| checkpoint | PSNR | SSIM | MSE | MAE | maxAE | changed pixels |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| default | 25.6187 dB | 0.6072 | 0.002742 | 0.004373 | 0.850066 | 0.010041 |
| compact | 25.3803 dB | 0.5128 | 0.002897 | 0.004677 | 0.850066 | 0.010787 |

The SSIM threshold is intentionally lower than the broad `webgpu-parity` demo
because this fixture is sparse text-only content. PSNR, MSE, MAE, and changed
pixel ratio are the stronger signals for this sample.

Generated evidence paths:

```text
target/css-style-parity/native-default.png
target/css-style-parity/web-default.png
target/css-style-parity/parity-default.json
target/css-style-parity/native-compact.png
target/css-style-parity/web-compact.png
target/css-style-parity/parity-compact.json
```

## Notes

- The sample uses no product image assets.
- The Web harness asserts there are no DOM game text or button renderers.
- The default viewport validates multiple visible styled runs including color
  and bold text. The compact viewport validates the same sample's responsive
  first-line rendering, because the textbox visible area clips before the second
  styled line.
