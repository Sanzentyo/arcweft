# Seq06.10a styled paragraph layout design

## Decision summary

Dialogue body text must be a renderer-owned styled paragraph. The renderer-facing model keeps `RenderTextBlock` for single-style UI surfaces and adds `RenderStyledParagraph` for dialogue body text.

This avoids blurring two responsibilities:

- `RenderTextBlock`: single-style labels and controls, including speaker labels, choice labels, and text inputs.
- `RenderStyledParagraph`: inline-style dialogue body content that must wrap as one paragraph/text box.

## Renderer-facing model

```rust
pub struct RenderStyledParagraph {
    pub text: String,
    pub bounds: HitRect,
    pub default_style: RenderTextStyle,
    pub spans: Vec<RenderStyledTextSpan>,
    pub reveal: RenderTextReveal,
    pub glyph_transforms: Vec<RenderGlyphTransformSpan>,
    pub visual_time_millis: u64,
}
```

`PreparedFrame` gains:

```rust
pub styled_paragraphs: Vec<RenderStyledParagraph>
```

`PreparedFrame::text` remains `Vec<RenderTextBlock>` and continues to carry non-dialogue single-style text surfaces.

## Span ownership

`RenderStyledTextSpan::range` is a half-open UTF-8 byte range into `RenderStyledParagraph::text`, matching `RichTextRange` and `RichTextTextRun` ownership. The planner validates that ranges are non-empty and align to `str` boundaries before emitting spans.

Style inheritance is resolved by starting from the dialogue body `base_style` computed from `RenderDialogue::base_styles`, then applying each run's typed `RichTextStyle` values with the existing renderer lowering rules. That preserves the current base-style cascade while removing the old layout split.

Mixed font sizes and line heights are lowered into glyphon/cosmic-text span attributes. The paragraph buffer default metrics come from `RenderStyledParagraph::default_style`; each span can override metrics. cosmic-text then computes line height from the shaped line's participating spans instead of from independent blocks.

## Rendering path

The renderer builds one glyphon/cosmic-text buffer per `RenderStyledParagraph` using `Buffer::set_rich_text`. It then adapts the shaped buffer into an `OwnedGlyphArea` through `arcweft-glyphon`, so glyph transforms can be applied after shaping and wrapping.

The rendering path becomes:

1. Build regular `TextArea`s for `RenderTextBlock` entries.
2. Build rich text buffers for `RenderStyledParagraph` entries.
3. Convert styled paragraph buffers to glyph areas.
4. Apply reveal and glyph-transform effects by source/glyph ranges.
5. Submit regular text areas and styled paragraph glyph areas through one glyphon renderer call.

Native and Web still share the same `PreparedFrame` and WGPU renderer path. No DOM text, screenshot parsing, canvas 2D text, or browser-only layout is introduced.

## Typewriter reveal

Typewriter reveal is represented by:

```rust
pub struct RenderTextReveal {
    pub visible_end: usize,
}
```

The paragraph text and span ranges are never truncated for normal typewriter reveal. During buffer construction, the renderer splits attribute segments at `visible_end` and assigns transparent alpha to hidden source ranges. This keeps shaping and wrapping stable while preserving the visible typewriter mask.

## Glyph transforms

Wave, shake, and jitter are represented as typed range metadata:

```rust
pub struct RenderGlyphTransformSpan {
    pub range: RichTextRange,
    pub motion: RenderGlyphMotion,
    pub node_index: usize,
}
```

`RenderGlyphMotion` owns the deterministic transform behavior. The renderer applies it to shaped glyph instances whose cluster byte ranges intersect the transform span. No normal styling effect is allowed to create one independent text buffer per glyph.

## Migrated and non-migrated surfaces

Migrated in this cut:

- dialogue body text from `RenderDialogue`.

Kept as single-style blocks in this cut:

- speaker labels;
- choice labels;
- text input controls.

Those surfaces remain correct as blocks because they are not the inline styled paragraph substrate targeted by seq06.10a.

## Layout evidence for seq06.10

The renderer exposes `styled_paragraph_layout_evidence`, producing:

- paragraph line boxes;
- span byte ranges and style metadata;
- glyph byte ranges and bounds;
- per-glyph reveal state.

Seq06.10 raster parity can use this evidence as its stable text-layout input and focus on glyph raster metrics, font pinning, and antialiasing tolerances.

## Non-goals

- Cross-backend antialiasing or exact pixel identity.
- Viewport scale redesign.
- Screenshot capture redesign.
- Full CSS layout.
- LSP/MCP/CLI-facing text layout APIs.
