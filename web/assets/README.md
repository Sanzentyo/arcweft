# WebGPU demo assets

This directory contains the browser demo's checked-in visual assets:

- `arcweft-demo.ttf`: Noto Sans Regular, licensed under the SIL Open Font
  License. The license text is checked in as `LICENSE-NotoSans.txt`.
- `noto-sans-jp-vf.ttf`: Noto Sans JP variable font, licensed under the SIL
  Open Font License. The default Web player registers this SFNT font with the
  shared renderer so browser canvas text uses Japanese glyphs without relying
  on host system fonts.
- `noto-emoji-regular.ttf`: Noto Emoji regular font, licensed under the SIL
  Open Font License. The default Web player registers this SFNT font so emoji
  glyphs are available in browser canvas text without relying on platform font
  fallback.
- `noto-sans-jp-*-wght-normal.woff2`: Noto Sans JP variable-font
  unicode-range shards from `@fontsource-variable/noto-sans-jp`, retained for
  future WOFF2 decoder compatibility tests. They are not part of the default
  Web player font list because the current Rust-side WOFF2 decoder rejects
  these shards before renderer registration.
- `generated-background.png`, `generated-character.png`, `generated-pulse.gif`,
  and `generated-pulse.webp`: generated Arcweft demo fixtures.

Regenerate the project-owned image fixtures from the repository root:

```bash
cargo +nightly -Zscript tools/generate-webgpu-demo-assets.rs
```

The same font bytes should be registered through `SharedRenderer::register_font_bytes`
in native and browser visual-parity jobs.
