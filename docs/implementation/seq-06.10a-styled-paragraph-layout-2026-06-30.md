# Seq06.10a styled paragraph layout implementation note

Date: 2026-06-30

## Current implementation evidence

The current `arcweft-render-wgpu` planner emits one `RenderTextBlock` per rich-text run for dialogue body text. It clamps each run by the typewriter-visible end, computes a block origin from `offset_x`, and advances `offset_x` through `estimated_text_width`. Motion effects split a run further into one `RenderTextBlock` per character.

That makes styled dialogue body layout depend on independent buffers and estimated advances rather than shaped paragraph layout.

## Implemented patch design

The overlay patch implements the following changes:

- adds `RenderStyledParagraph`, `RenderStyledTextSpan`, `RenderTextReveal`, `RenderGlyphTransformSpan`, `RenderGlyphMotion`, and `RenderGlyphTransformKind` in `arcweft-render-wgpu::geometry`;
- adds `PreparedFrame::styled_paragraphs`;
- changes `push_dialogue_panel` so dialogue body text is emitted through `push_dialogue_styled_paragraph` instead of `push_dialogue_text_blocks`;
- removes the dialogue-body dependency on `estimated_text_width`, `estimated_char_width`, and `push_motion_text_blocks`;
- keeps speaker labels, choices, and text inputs on `RenderTextBlock`;
- uses glyphon/cosmic-text rich text spans for paragraph-wide shaping/wrapping;
- preserves glyph-transform effects as typed source-range metadata on
  `RenderStyledParagraph`;
- adds `styled_paragraph_layout_evidence` for seq06.10;
- updates Web frame observations to expose styled paragraph metadata;
- extends `samples/css-style-parity/main.arcw` with a long styled paragraph crossing style boundaries;
- adds focused `arcweft-render-wgpu` tests for planning, wrapping evidence, typewriter reveal, glyph transforms, and non-dialogue labels.

## Local application deviation

The package patch attempted to convert styled paragraph buffers into
`arcweft-glyphon` glyph areas and apply glyph transforms directly in
`arcweft-render-wgpu`. That route compiled for the native target but failed when
`just css-style-parity` built `arcweft-player-web` for
`wasm32-unknown-unknown`: the current `arcweft-glyphon` adapter imports glyphon
custom glyph-area types that are not available in that wasm build.

The applied production code therefore keeps native and Web on the shared
glyphon rich-text `TextArea` path for paragraph rendering. It keeps
`RenderGlyphTransformSpan` and `RenderGlyphMotion` as typed renderer metadata,
but does not yet apply wave/shake/jitter to glyph instances in the Web-safe
shared path.

This is not a compatibility shim for the old dialogue run-per-block layout.
Dialogue body text is still replaced at the root by one
`RenderStyledParagraph`. The remaining gap is tracked by:

`docs/reviews/requests/2026-06-30-seq-06.10b-styled-paragraph-raster-evidence-closure.md`

## API and crate boundary notes

`arcweft-render-wgpu` does not gain a dependency on `arcweft-glyphon` in the
applied version because that dependency currently breaks the wasm player build.
The paragraph route stays inside `arcweft-render-wgpu` and the existing glyphon
dependency already used by native and Web rendering.

No data-format crate gains I/O. Font loading remains through the existing renderer API (`register_font_bytes`) and parity tools.

## Effect semantics

Typewriter reveal is a mask over source byte ranges, not text truncation. The full paragraph text is always shaped. Hidden ranges receive transparent alpha attributes before shaping, so wrapping stays stable as the reveal advances.

Wave, shake, and jitter are represented by `RenderGlyphMotion` after paragraph
planning. Applying those transforms to rendered glyph instances remains a
seq06.10b task because the first-cut glyph-area route was not wasm-safe.

## CSS-style parity fixture

The sample adds a long line containing `[strong]`, `[color]`, and `[size]` spans that must wrap as one paragraph. This is intended to fail under the previous run-per-block model when the wrap boundary crosses a style-run boundary.

## Coordination with seq06.10

Seq06.10 should treat `styled_paragraph_layout_evidence` as the stable paragraph layout substrate. Raster parity should consume line boxes, span ranges, glyph bounds, and reveal status from the shared renderer path rather than inferring layout from terminal output, DOM text, CSS browser text, screenshots, or hidden overlay text.

The applied seq06.10 text-raster verifier can parse styled paragraph metadata,
but it still lacks paragraph line/glyph bounds in native/Web frame JSON. That
means it cannot yet produce a meaningful per-span mask comparison for the long
styled paragraph fixture.

## Non-goals retained

- No cross-backend antialiasing or exact pixel-identity solution.
- No viewport scale or capture pipeline redesign.
- No full CSS layout engine.
- No LSP/MCP/CLI-facing text layout API.

## Validation

Applied validation in this checkout:

```bash
cargo fmt --all
cargo test -p arcweft-render-wgpu --test styled_paragraph --all-features
cargo test -p arcweft-render-wgpu dialogue_surface_styles_are_preserved_for_styled_paragraph --all-features
node --check web/tests/css-style-parity-smoke.mjs
cargo +nightly -Zscript tools/verify-text-raster-parity.rs --self-test
cargo check -p arcweft-render-wgpu -p arcweft-player-web --all-features
cargo clippy -p arcweft-render-wgpu -p arcweft-player-web --all-targets --all-features -- -D warnings
```

`just css-style-parity` builds the wasm player and captures native/Web images,
but currently fails the existing default full-image threshold after the long
styled paragraph fixture is added:

```text
psnr=22.3596 dB, ssim=0.4607, mse=0.005808, mae=0.008701, changed_pixel_ratio=0.017637
required: psnr>=25.0, ssim>=0.60, mse<=0.0030, mae<=0.0048, changed_pixel_ratio<=0.011
```

The manually run text-raster verifier for the default checkpoint also fails
because styled paragraph spans currently share the whole paragraph bounds in
frame JSON. Seq06.10b must close that evidence gap rather than simply relaxing
thresholds.
