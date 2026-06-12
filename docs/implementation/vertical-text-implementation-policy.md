# Vertical Text Implementation Policy

This document fixes the implementation policy for long-term vertical rich-text
rendering. The source design package is archived at
`docs/implementation/vertical-longterm-design-20260612/`; this document records
the concrete choices to follow when turning that package into production code.

## Adopted Direction

- Arcweft owns vertical text layout. glyphon remains the GPU text renderer and
  receives a pre-laid glyph stream.
- Do not implement vertical writing by inserting newlines, rotating a whole
  `TextArea`, or preserving an old rich-text flatten path through a transitional
  compatibility layer.
- Use a vendored glyphon fork at `vendor/glyphon` through `[patch.crates-io]`.
  The fork must keep existing `TextArea` behavior intact and add a sibling
  `GlyphArea` API for pre-laid glyphs.
- Add `crates/arcweft-text-layout` as a Sans I/O production crate. It may depend
  on `arcweft-render-text`, `serde`, `thiserror`, and Unicode/shaping helper
  crates, but must not depend on glyphon, wgpu, filesystem, network, clocks, or
  native windowing.
- Add `crates/arcweft-glyphon` as the adapter boundary from
  `arcweft-text-layout::LaidOutText` to glyphon `GlyphArea`. glyphon, wgpu
  renderer state, font-system lifetimes, atlas/cache details, and metadata
  packing stay above the Sans I/O layout crate.

## Implementation Sequence

1. Vendor glyphon 0.11.0 under `vendor/glyphon`, add the workspace patch, and
   prove the current native player still renders existing horizontal rich text.
2. Extend the glyphon fork with `GlyphArea`, `GlyphInstance`, `GlyphSource`,
   `GlyphTransform`, and `TextRenderer::prepare_glyph_areas`. Share atlas/cache
   allocation with the existing `TextArea` path.
3. Add affine glyph transforms to the glyphon render vertex path and WGSL shader.
   At minimum, `Identity`, `Rotate90Cw`, `Rotate90Ccw`, and a full affine matrix
   must be represented.
4. Promote the design skeleton into `crates/arcweft-text-layout`, replacing
   placeholder shaping with a real backend boundary and adding production
   models for style runs, ruby, text-combine, line/column boxes, hit maps, and
   text observations.
5. Implement vertical layout with logical axes first, then physical mapping:
   `VerticalRl` uses top-to-bottom inline progression and right-to-left column
   progression; `VerticalLr` uses top-to-bottom inline progression and
   left-to-right column progression.
6. Implement UAX #29 grapheme segmentation, UAX #50 vertical orientation, UAX
   #14 line opportunities, and Arcweft/JLREQ line-breaking policy. External
   crates are allowed where they reduce risk; generated tables are allowed for
   stable Unicode data.
7. Implement vertical shaping policy: upright CJK uses vertical alternates,
   mixed ASCII Latin uses engine-side clockwise rotation, and `vert`/`vrtr`
   paths are not mixed with pre-rotated `vrt2` paths.
8. Implement ruby and text-combine as layout items, not renderer hacks. Ruby
   collision handling must support base expansion first, then limited overhang,
   then line-break feedback.
9. Replace native rich-text drawing, bounds, mask/object-id captures, and Agent
   observations with geometry from `LaidOutText`. Pixel readback remains a
   verification/capture path, not the source of truth for text geometry.

## Acceptance Criteria

- `samples/rich-text-full-grammar.arcw` and
  `samples/rich-text-windows-fonts.arcw` render `[.vertical_rl]` as true
  vertical text in native window and headless PNG captures.
- Mixed text such as `吾輩は猫である。ABC 123 2026` renders CJK upright, ASCII
  sideways, and eligible digits as text-combine-upright.
- Ruby over/under placement works in `vertical_rl` and `vertical_lr`, including
  collision avoidance for adjacent ruby groups.
- Typewriter reveal keeps line and column breaks stable while changing only
  glyph visibility.
- Agent observation returns source ranges, text/ruby object bounds, glyph/cluster
  hit regions, and layer/object captures from the same layout geometry used for
  rendering.
- `cargo fmt`, `cargo check --workspace`,
  `cargo clippy --workspace --all-targets --all-features`, and
  `cargo test --workspace` pass.
- Native headless captures for vertical fixtures are compared with `imq` or an
  equivalent full-reference image metric in CI artifacts. Do not commit generated
  image outputs unless they are stable fixtures intentionally added for tests.

## Explicit Defaults

- Repository workflow remains `main` only unless a branch is explicitly
  requested.
- Existing public rich-text syntax is reused; no alternate vertical-writing
  syntax is introduced.
- Backward compatibility layers are not added for internal renderer/layout
  refactors. Replace the old internal model directly and fix call sites.
- Layout correctness takes priority over short-term rendering shortcuts.
