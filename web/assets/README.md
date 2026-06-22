# WebGPU demo assets

This directory contains the browser demo's checked-in visual assets:

- `arcweft-demo.ttf`: Noto Sans Regular, licensed under the SIL Open Font
  License. The license text is checked in as `LICENSE-NotoSans.txt`.
- `generated-background.png`, `generated-character.png`, `generated-pulse.gif`,
  and `generated-pulse.webp`: generated Arcweft demo fixtures.

Regenerate the project-owned image fixtures from the repository root:

```bash
cargo +nightly -Zscript tools/generate-webgpu-demo-assets.rs
```

The same font bytes should be registered through `SharedRenderer::register_font_bytes`
in native and browser visual-parity jobs.
