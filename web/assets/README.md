# Web Player Static Assets

This directory contains checked-in browser-served static resources. Arcweft
authoring inputs for `web/demo.arcw` live separately under `web/bundle-assets/`.

- `arcweft-demo.ttf`: Noto Sans Regular, licensed under the SIL Open Font
  License. The license text is checked in as `LICENSE-NotoSans.txt`.
- `noto-sans-jp-vf.ttf`: Noto Sans JP variable font, licensed under the SIL
  Open Font License. The default Web player registers this SFNT font with the
  shared renderer so browser canvas text uses Japanese glyphs without relying
  on host system fonts.
- `noto-sans-jp-native-style-parity.ttf`: deterministic SFNT subset of the same
  Noto Sans JP variable font containing `星影ほしかげ`. The native Style/Web
  visual fixture registers this 13 KiB subset instead of paying the debug-Wasm
  startup cost of the 9 MiB product font. It is generated with FontTools using
  `--layout-features=* --name-IDs=* --name-legacy --name-languages=*`
  and `--no-recalc-timestamp`.
- `noto-sans-jp-unified-text-parity.ttf`: deterministic SFNT subset for the
  unified Text visual packet. It contains the Japanese base, ruby, and
  punctuation glyphs used by the vertical-RL, vertical-LR, JLREQ, and Fx
  pages while retaining all OpenType layout features and stable timestamps.
  Regenerate it from the checked-in product font with:

  ```bash
  uvx --from fonttools pyftsubset web/assets/noto-sans-jp-vf.ttf \
    --output-file=web/assets/noto-sans-jp-unified-text-parity.ttf \
    --text="星影ほしかげ天地人縦夢ゆめへ山川海波動光開示。（「」）" \
    --layout-features=* --name-IDs=* --name-legacy --name-languages=* \
    --no-recalc-timestamp
  ```
- `noto-emoji-regular.ttf`: Noto Emoji regular font, licensed under the SIL
  Open Font License. The default Web player registers this SFNT font so emoji
  glyphs are available in browser canvas text without relying on platform font
  fallback.
- `noto-sans-jp-*-wght-normal.woff2`: Noto Sans JP variable-font
  unicode-range shards from `@fontsource-variable/noto-sans-jp`, retained for
  future WOFF2 decoder compatibility tests. They are not part of the default
  Web player font list because the current Rust-side WOFF2 decoder rejects
  these shards before renderer registration.
The same font bytes should be registered through `SharedRenderer::register_font_bytes`
in native and browser visual-parity jobs.
