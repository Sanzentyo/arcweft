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

## Current Implementation Notes

- `arcweft-text-layout` keeps renderer-independent glyph/run/ruby geometry and
  preserves layout cursors across adjacent display-map text runs. Style/effect
  splitting must not reset inline progression.
- `arcweft-glyphon` converts `LaidOutText` into glyphon `GlyphArea` input. It
  owns renderer-coordinate adjustments such as glyph origin offsets; Sans I/O
  layout coordinates stay cell/bounds oriented.
- Vertical layout clustering uses `unicode-segmentation` grapheme indices for
  UAX #29 boundaries, so combining-mark Latin clusters and emoji ZWJ sequences
  stay intact before orientation and text-combine policy are applied. Mixed
  vertical orientation is resolved through a generated Unicode 17.0.0 UAX #50
  `Vertical_Orientation` range table in `arcweft-text-layout`. `Tu` and `Tr`
  now produce `GlyphVerticalForm::{UprightAlternate,RotatedAlternate}` metadata
  on `LaidOutGlyph`; native visual-plan/debug geometry preserves that request so
  Agent observations can distinguish fallback rotation from missing vertical
  alternate shaping. The native GlyphArea path uses that metadata to shape
  affected clusters through cosmic-text with `vert` for `UprightAlternate` and
  `vrtr` for `RotatedAlternate` before resolving glyphon cache keys.
- Vertical column breaking uses `unicode-linebreak` UAX #14 opportunities as
  initial break candidates. When a column overflows, the layout only moves the
  next cluster to a new column if the cluster boundary is a break opportunity,
  keeping closing punctuation out of column heads as an initial kinsoku rule.
- `TextCombineUpright` layout clusters may resolve to multiple shaped glyphon
  cache keys. The adapter emits one `GlyphInstance` per resolved key inside the
  cluster cell instead of collapsing the cluster to a single glyph. It applies
  deterministic horizontal affine compression so 2-4 digit clusters fit inside
  one vertical cell while preserving shared source-cluster metadata. Production
  shaping metrics and kerning still belong in the layout/shaping policy.
- The native window path and native headless full-frame capture path render body
  text through `GlyphArea` when a display-map layout source is available. Ruby
  annotations are shaped with glyphon buffers, then submitted as absolute
  `GlyphArea` instances positioned from `LaidOutText` ruby geometry.
- Ruby geometry applies deterministic same-track collision separation in
  `arcweft-text-layout` before native rendering and Agent bounds consume it.
  This keeps adjacent horizontal and vertical ruby annotations from occupying
  the same annotation track.
- Debug/object/color capture geometry is measured from `LaidOutText`; pixel
  readback remains only a verification and capture output path.
- CLI regression coverage includes an `imq` native vertical capture parity check
  that compares repeated headless PNG captures for the same `vertical_rl`
  fixture. Fast native Agent coverage also exercises a `vertical_lr` fixture with
  ruby and a 4-digit text-combine run, asserting that the observed layer/run/ruby
  geometry is produced by the native renderer. Checked-in golden baselines remain
  future work once rendering baselines are stable across CI targets.
- Remaining work includes production `TextCombineUpright` shaping metrics/kerning,
  full JLREQ line-breaking policy, proof against stable checked-in font/capture
  baselines for `Tu`/`Tr` alternates, full ruby base expansion/overhang/line-break
  feedback, and checked-in `imq` golden fixtures.

## Explicit Defaults

- Repository workflow remains `main` only unless a branch is explicitly
  requested.
- Existing public rich-text syntax is reused; no alternate vertical-writing
  syntax is introduced.
- Backward compatibility layers are not added for internal renderer/layout
  refactors. Replace the old internal model directly and fix call sites.
- Layout correctness takes priority over short-term rendering shortcuts.
