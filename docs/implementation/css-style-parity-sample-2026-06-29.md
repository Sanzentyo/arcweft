# CSS Style Parity Sample - 2026-06-29

## Scope

Added an image-free Web/native parity sample for CSS-like Arcweft styling. The
sample is intentionally renderer-only: visible text, colors, size changes,
emphasis, ruby, textbox chrome, and canvas output come from Arcweft renderer
state rather than DOM game View or image assets.

## Added Files

- `samples/css-style-parity/main.arcw`
- `samples/css-style-parity/README.md`
- `tools/capture-text-parity-frame.rs` (generalized from the original
  CSS-specific capture tool on 2026-07-12)
- `web/tests/text-parity-smoke.mjs` (generalized from the original
  CSS-specific browser harness on 2026-07-12)

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

## 2026-07-03 DSL Style Expansion

Updated `samples/css-style-parity/main.arcw` so the sample uses more of the
currently implemented DSL styling surface directly from Arcweft source:

- visible rich text now covers font, color, size, strong, emphasis, ruby source
  nodes, presentation opacity, transform offset, and wave effect declarations in
  one player-rendered paragraph;
- `style css_style_parity` now authors typed CSS-like retained View tokens and
  selector rules for surface color, button hover/active/focus styling,
  text-field caret and selection colors, placeholder color, composition
  underline color, border radius, scale, opacity, and translate-y.

Validation executed from `D:/git/arcweft`:

```bash
cargo fmt --all
cargo test -p arcweft-cli --test css_style_parity_sample -- --nocapture
cargo run -p arcweft-cli -- bundle samples/css-style-parity/main.arcw --format json --output target/css-dsl-observe/css-style-parity.bundle.json --json
cargo run -p arcweft-cli --features native-capture --quiet -- agent observe samples/css-style-parity/main.arcw --json --image png --out target/css-dsl-observe/css-style-parity-after.png --mode drain --steps 4 --max-ops 64
```

Observed evidence:

- `target/css-dsl-observe/css-style-parity-after.png` shows the Arcweft-rendered
  styled paragraph with colored, larger, bold, italic, transformed, and effect
  target spans visible in the native player-backed observe path.
- `target/css-dsl-observe/css-style-parity-after.observe.json` includes
  `rich_text.presentation.opacity`, `rich_text.transform.offset`, and
  `rich_text.effect.wave` contributions from the DSL source.
- `target/css-dsl-observe/css-style-parity.bundle.json` includes the
  `view_style` product resource with `style.css_style_parity`, `color.accent`,
  hover/active interaction rules, focus-visible rules, `caret-color`, and
  `composition-underline-color`.

Known boundary: retained `style` sidecar data is bundled into the product
`view_style` resource, but the current `agent observe` route primarily visualizes
the dialogue/rich-text renderer. Retained View sidecar consumption is validated
through bundle JSON and the interactive View renderer sample path until the
retained View program is fully instantiated by the player scene.
