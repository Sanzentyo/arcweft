# CSS Style Parity Sample

This sample is a renderer-only Web/native parity fixture for CSS-like Arcweft
styling. It intentionally uses no image assets and no DOM-rendered game UI. The
visible content should come from the shared Arcweft renderer on both native
offscreen capture and the WebGPU player.

The `.arcw` source exercises two styling layers:

- visible dialogue rich text for `font`, `color`, `size`, `strong`, `em`, ruby,
  opacity, transform offset, and wave motion;
- product UI style rules for typed `ui style` tokens, `button` hover/active
  rules, focus-visible outlines, text-field caret/selection colors, and
  composition underline color.

The retained `ui style` resource is bundled into the product as CSS-like typed
style data. The current `agent observe` path primarily validates the
player-rendered dialogue layer; retained UI sidecar consumption is covered by
bundle checks and the interactive UI renderer samples.

Run the parity route from the repository root:

```bash
just css-style-parity
```

Generated bundles and PNG reports are written under `web/local/` and
`target/css-style-parity/`.

For a quick local observe capture:

```bash
cargo run -p arcweft-cli --features native-capture -- agent observe samples/css-style-parity/main.arcw --json --image png --out target/css-style-parity/observe.png --mode drain --steps 4 --max-ops 64
```
