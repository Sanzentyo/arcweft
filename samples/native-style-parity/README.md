# Native Style Parity Sample

This sample is a renderer-only Web/native parity fixture for Arcweft's typed,
native Style language. It intentionally uses no image assets and no DOM-rendered
game View. The visible content comes from the shared Arcweft renderer on both
native offscreen capture and the WebGPU player.

The `.arcw` source exercises two styling layers:

- visible dialogue rich text for `font`, `color`, `size`, `strong`, `em`, ruby,
  opacity, transform offset, and wave motion;
- product View style rules for typed `style` tokens, `Button` hover/active
  rules, focus-visible outlines, text-field caret/selection colors, and
  composition underline color.

The two retained native sheets are applied once, in order, on the dialogue View
root. Descendants are selected by their typed `.part(...)` identity, so tokens
remain sheet-local and the resource no longer relies on a style ID doubling as
a part name. The current `agent observe` path primarily validates the
player-rendered dialogue layer; retained View resource consumption is covered
by bundle checks and the interactive View renderer samples.

Run the parity route from the repository root:

```bash
just native-style-parity
```

Generated bundles and PNG reports are written under `web/local/` and
`target/native-style-parity/`.

For a quick local observe capture:

```bash
cargo run -p arcweft-cli --features native-capture -- agent observe samples/native-style-parity/main.arcw --json --image png --out target/native-style-parity/observe.png --mode drain --steps 4 --max-ops 64
```
