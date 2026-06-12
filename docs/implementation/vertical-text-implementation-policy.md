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
  on `LaidOutGlyph`; native visual-plan/debug geometry and Agent
  `rich_text_cluster` refs preserve that request so observations can distinguish
  fallback rotation from missing vertical alternate shaping. The native
  GlyphArea path uses that metadata to shape affected clusters through
  cosmic-text with `vert` for `UprightAlternate` and `vrtr` for
  `RotatedAlternate` before resolving glyphon cache keys, using the active
  rich-text font family, weight, style, and size metrics instead of a fixed
  renderer default.
- Vertical column breaking uses `unicode-linebreak` UAX #14 opportunities as
  initial break candidates. When a column overflows, the layout only moves the
  next cluster to a new column if the cluster boundary is a break opportunity,
  keeping closing punctuation, small kana, dash/prolonged-sound marks, and
  middle dots out of column heads as an initial kinsoku rule. The layout also
  applies an initial JLREQ line-end prohibition for opening punctuation by
  moving the opening punctuation to the next column when it would otherwise be
  stranded at the previous column end. JLREQ punctuation policy lives in
  `arcweft-text-layout/src/jlreq_punctuation.rs`; reviewable source ranges live
  in `arcweft-text-layout/data/jlreq_punctuation_ranges.txt`; reviewable
  pair/cost source rules live in
  `arcweft-text-layout/data/jlreq_pair_adjustments.txt`; the checked-in
  generated tables live in
  `arcweft-text-layout/src/jlreq_punctuation_data.rs`.
  `tools/generate_jlreq_punctuation_data.rs` regenerates or checks those
  tables, and `just generate-jlreq-punctuation` /
  `just check-jlreq-punctuation` wrap the tool. The range table classifies
  opening/closing punctuation, small kana, dash marks, leaders, middle dots,
  and repeat marks, including fullwidth, halfwidth, vertical presentation, and
  broader paired-bracket codepoints. The generated pair/cost table keeps
  iteration marks with the previous cluster, supplies loose/normal/strict
  strictness presets through `TextLayoutConfig::jlreq_strictness`, keeps
  compact bracket pairs, small kana, dash/prolonged-sound marks, leader marks,
  repeated dash/leader marks, and strict middle-dot pairs together according to
  the chosen preset, and supplies preset-specific planner break penalties for
  weaker punctuation pairs. Authors can select the per-span preset with layout
  attributes such as `[.vertical_rl jlreq=strict]`; `jlreq=auto`/omitted
  inherits the host textbox `TextLayoutConfig` preset.
  Initial punctuation compression reduces the inline advance of compressible
  closing punctuation and middle dots to half a body cell. When such punctuation
  must remain at the column end for kinsoku, the layout applies half-cell
  hanging by moving its glyph origin upward while keeping full cell bounds for
  capture/hit geometry from the same rendered placement. Break decisions for a
  vertical run are collected into a `VerticalColumnPlan` before glyph placement.
  The plan builder now uses paragraph-level dynamic programming across each
  explicit line-break-separated segment. Candidate columns include
  ruby-required inline extent, JLREQ line-head prohibition, line-end prohibition
  for opening punctuation, separation-prohibited punctuation pairs, and
  generated pair break penalties. The cost model preserves existing
  fill-forward behavior for ties, treats accepted kinsoku overhang as allowed
  overhang instead of bad overflow, and balances columns when paragraph badness
  is materially lower.
- `TextCombineUpright` layout clusters may resolve to multiple shaped glyphon
  cache keys. The adapter emits one `GlyphInstance` per resolved key inside the
  cluster cell instead of collapsing the cluster to a single glyph. The resolver
  carries shaped glyph advances with those cache keys, and the adapter uses their
  summed advance to apply deterministic horizontal affine compression so 2-4
  digit clusters fit inside one vertical cell while preserving shared
  source-cluster metadata.
- The native window path and native headless full-frame capture path render body
  text through `GlyphArea` when a display-map layout source is available. Ruby
  annotations are shaped with glyphon buffers, then submitted as absolute
  `GlyphArea` instances positioned from `LaidOutText` ruby geometry. Vertical
  ruby annotations keep glyphon/cosmic-text shaping for cache keys, but their
  glyph instances are stacked top-to-bottom from the `LaidOutRuby::ruby_bounds`
  cell width and vertical advance so rendered ruby tracks match Agent geometry.
- Typewriter reveal is applied after layout as glyph color alpha in the GlyphArea
  path. It does not recompute `LaidOutText`, so line and column breaks remain
  stable while captures can vary glyph visibility by capture time.
- Ruby geometry applies deterministic same-track collision separation in
  `arcweft-text-layout` before native rendering and Agent bounds consume it.
  Long ruby annotations first expand the base allocation along the writing
  mode's inline axis, then use a bounded overhang allowance before same-track
  collision separation keeps adjacent horizontal and vertical annotations from
  occupying the same annotation track. Vertical ruby base starts feed their
  expanded inline allocation back into column breaking, so a long annotation or
  multi-cluster base moves to the next column instead of placing its expanded
  base past the column end. `vertical_rl` ruby uses the right annotation track
  and `vertical_lr` ruby uses the left annotation track. Over-height vertical
  ruby annotations split into multiple `LaidOutRuby` segments with the same
  source ruby index; native rendering emits one ruby glyph area per segment and
  Agent/native element geometry unions the segments back to the authored ruby
  object while also exposing viewport-space base and annotation bboxes on the
  ruby ref for debugging.
- Debug/object/color capture geometry is measured from `LaidOutText`; pixel
  readback remains only a verification and capture output path. Native
  offscreen debug and isolated-color captures use the same GlyphArea path as
  normal body rendering, so vertical glyph cluster bboxes, object-id masks, and
  color crops share one placement model instead of falling back to glyphon's
  horizontal TextArea layout.
- CLI regression coverage includes an `imq` native vertical capture parity check
  that compares repeated headless PNG captures for the same `vertical_rl`
  fixture. Fast native Agent coverage also exercises a `vertical_lr` fixture with
  ruby and a 4-digit text-combine run, asserting that the observed
  layer/run/ruby and text-combine cluster geometry is produced by the native
  renderer. The checked-in rich-text samples
  `samples/rich-text-full-grammar.arcw` and
  `samples/rich-text-windows-fonts.arcw` are now also observed through the native
  `dialogue.rich_text` layer path so their authored `vertical_rl`/`vertical_lr`
  runs, source ranges, masks, and column-shaped bounds remain covered.
  `rich_text_cluster` child objects expose glyph-cluster source ranges and
  renderer-facing orientation/vertical-form metadata plus object-local
  color/object-id/mask capture refs; the full-grammar sample test follows a
  vertical cluster mask URI and asserts the raw capture contains rendered
  pixels. Focused native Agent coverage checks sideways, upright-alternate,
  rotated-alternate, and text-combine-upright cluster metadata. It also checks
  adjacent `vertical_rl` and `vertical_lr` ruby annotations through separate
  `ruby_base_bbox` / `ruby_annotation_bbox` metadata so collision separation is
  observable in Agent output.
  `tests/fixtures/native_capture/` contains a checked-in Windows native PNG
  golden for a vertical `Tu`/`Tr` alternate fixture, and the CLI test compares a
  fresh native capture against it with `imq` when both Windows fonts and the
  `imq` binary are available.
  The expanded JLREQ pair profile is also exercised through native Agent
  fixtures that observe `dialogue.rich_text` cluster geometry for leader marks,
  compact bracket pairs, small kana, prolonged-sound marks, iteration marks,
  and strict middle-dot pairs in vertical text, matching the published JLREQ
  line-composition treatment of punctuation classes and unbreakable character
  sequences (`https://www.w3.org/TR/2008/WD-jlreq-20081015/`, section 3.1).
  Native Agent coverage also checks hanging punctuation at a vertical column
  end and half-cell punctuation compression for adjacent Japanese punctuation,
  so those JLREQ placement decisions are visible in observed glyph-cluster
  bboxes and not only in Sans I/O layout unit tests.
- Remaining work includes JLREQ refinements beyond the current kinsoku,
  separation, punctuation-compression, half-cell hanging, generated range table,
  generated strictness-aware pair/cost table, and paragraph-DP column planner,
  especially adding broader published JLREQ paragraph examples, preset-specific
  visual goldens, and edge-case fixtures beyond the current punctuation-pair
  native Agent coverage.

## Explicit Defaults

- Repository workflow remains `main` only unless a branch is explicitly
  requested.
- Existing public rich-text syntax is reused; no alternate vertical-writing
  syntax is introduced.
- Backward compatibility layers are not added for internal renderer/layout
  refactors. Replace the old internal model directly and fix call sites.
- Layout correctness takes priority over short-term rendering shortcuts.
