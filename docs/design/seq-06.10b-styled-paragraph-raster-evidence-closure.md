# seq-06.10b styled paragraph raster evidence closure

## Goal

Make CSS-style parity useful again after dialogue body layout moved to `RenderStyledParagraph`. The native and Web reports must carry renderer-owned paragraph evidence rich enough to diagnose backend drift without hidden DOM text, browser CSS text, canvas 2D fallback text, screenshot layout inference, or unchecked full-image threshold loosening.

## Decisions

### 1. Paragraph evidence shape

Each styled paragraph report uses schema `arcweft.web_frame_observation.v1` / `arcweft.css_style_native_frame_observation.v1` and emits:

- paragraph text, paragraph bounds, byte length, and `visible_end`;
- default font/style evidence: font size, line height, RGBA, family, weight, and slant;
- authored span evidence with source byte ranges and `node_index`;
- line boxes from glyphon layout runs;
- per-glyph/per-cluster bounds with line index, source byte range, reveal state, effective style/color, and optional transform metadata;
- paragraph transform spans with deterministic sampled offset and explicit render support state;
- font fingerprint at the report envelope level.

The verifier consumes glyph/cluster evidence as the required styled paragraph input. Styled paragraph reports that omit renderer-owned line/glyph evidence are rejected instead of being expanded from authored spans.

### 2. Ownership

`arcweft-render-wgpu` owns evidence extraction because it already owns the renderer-prepared paragraph model and glyphon shaping path. Browser and native adapters only serialize the typed evidence that the renderer crate prepares. The Web crate does not gain a direct `glyphon` dependency and no `arcweft-render-wgpu -> arcweft-glyphon` dependency is introduced.

The API added by the patch is:

```rust
SharedRenderer::frame_styled_paragraph_layout_evidence(&mut self, frame: &PreparedFrame)
StyledParagraphEvidenceFontContext::frame_styled_paragraph_layout_evidence(&mut self, frame: &PreparedFrame)
```

`SharedRenderer` is used by Web so evidence uses the registered renderer font system. The font context is used by the native capture script so report generation stays in tools/adapters and renderer crates remain Sans I/O.

### 3. Glyph transforms

Wave/shake/jitter are not rendered through a new wasm glyph-area path in this cut. Rendering remains deterministic and shared, while transform requests are serialized as typed unsupported metadata:

```text
transform_support = metadata_only_unsupported
rendered = false
support = metadata_only_unsupported
sampled_offset_y_milli = deterministic RenderGlyphMotion::offset_y(...)
```

This avoids the earlier wasm-unsafe glyph-area route and makes the remaining rendering gap explicit.

### 4. Text raster comparison model

The verifier uses a combined layout-first plus mask-tolerance model:

1. Compare typed viewport and report counts.
2. Flatten text blocks plus styled paragraph glyph evidence into text raster runs.
3. For each glyph/cluster run, compare text/source range/visibility/style/layout first.
4. Build masks only inside the union of native/Web typed glyph bounds, with existing color-affinity masking.
5. Compare mask XOR, bounding box, centroid, and coverage.
6. Aggregate failures by run with paragraph index, line index, byte range, and reveal state.

Paragraph span expansion to full bounds is removed from the styled paragraph raster path. Span evidence remains present as authored metadata, but glyph/cluster evidence is the only source used for styled paragraph raster runs.

### 5. Full-image threshold interaction

Full-image `verify-webgpu-parity` remains useful for gross framebuffer regressions, but it is no longer the first and only failing gate. The Justfile delegates text parity gates to `tools/run-text-parity-gates.rs`, which runs text-raster verification for all checkpoints first, then full-image verification, then optional IMQ comparison, and only then exits with failure if any gate failed. CSS style parity is one caller of this shared harness.

This means failed runs still leave:

- native/Web PNGs;
- native/Web frame JSON;
- text-raster JSON for default, compact, and HiDPI;
- full-image parity JSON for default, compact, and HiDPI;
- IMQ JSON when `imq` is available.

### 6. Evidence-before-failure policy

`just css-style-parity` still fails when a gate fails, but no longer aborts before generating text-specific evidence for later checkpoints. The gate runner accumulates failures and prints a compact summary after writing all requested reports.

## Non-goals

- No hidden DOM mirror.
- No HTML/CSS/browser text overlay.
- No canvas 2D fallback text.
- No viewport scale contract redesign.
- No native/Web launch path redesign.
- No root styled paragraph model redesign.
- No glyph transform rendering implementation until a wasm-safe shared glyph-area abstraction exists.
