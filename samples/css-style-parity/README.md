# CSS Style Parity Sample

This sample is a renderer-only Web/native parity fixture for CSS-like text and
choice styling. It intentionally uses no image assets and no DOM-rendered game
UI. The visible content should come from the shared Arcweft renderer on both
native offscreen capture and the WebGPU player.

Run the parity route from the repository root:

```bash
just css-style-parity
```

Generated bundles and PNG reports are written under `web/local/` and
`target/css-style-parity/`.
