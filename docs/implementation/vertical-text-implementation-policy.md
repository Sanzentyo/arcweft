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
  Unit coverage now also fixes the window page-construction path for a
  `vertical_lr` page that combines side-track ruby and a 4-digit
  text-combine-upright cluster, proving the page-local layout source can be
  adapted into the same body and ruby GlyphAreas used by the actual window
  renderer. Native object-scope raw mask and object-id captures for a textbox
  are verified against glyph alpha rather than the full textbox rectangle, so
  Agent observe readback exposes rendered glyph geometry for both attachment
  kinds; CLI and MCP resource readback paths also read stored raw object-id URIs
  back as base64 bytes, and MCP resource templates advertise layer/object
  object-id URI patterns for png and raw RGBA captures.
- Typewriter reveal is applied after layout as glyph color alpha in the GlyphArea
  path. It does not recompute `LaidOutText`, so line and column breaks remain
  stable while captures can vary glyph visibility by capture time. CLI
  `--capture-time` and MCP `capture_time` pass that time into full-frame,
  layer/object color, object-id, and mask readback; native Agent coverage checks
  that a vertical typewriter cluster keeps the same observed bbox while hidden
  and visible raw mask/object-id captures differ only in rendered alpha content.
  The same
  capture-time path is also covered for a `text-combine-upright` digit cluster
  through mask and object-id raw crops, verifying that all native GlyphArea
  instances emitted for the combined cell follow the layout glyph's visibility
  without changing Agent geometry. MCP stdio coverage exercises the same
  capture-time path for text-combine object mask/object-id captures and a ruby
  object-id capture, checking hidden and visible raw RGBA blobs through
  `arcweft.capture`. Ruby annotation buffers also carry the resolved
  presentation into their absolute GlyphArea instances, so ruby object mask and
  object-id readback can hide and reveal the annotation at capture time without
  changing the observed base/annotation bboxes.
- Ruby geometry applies deterministic same-track collision separation in
  `arcweft-text-layout` before native rendering and Agent bounds consume it.
  Vertical layout reserves the inline-side annotation track before placing the
  first column, shifting `vertical_rl` base columns left and `vertical_lr` base
  columns right only when ruby annotations in that writing mode need the side
  track. This keeps short edge-adjacent ruby annotations inside the layout box
  without changing the side-of-base semantics.
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
  ruby ref for debugging. Native debug capture coverage also renders an
  over-height vertical ruby object and checks that the captured content stays
  inside the authored ruby object's union bbox while preserving the widened
  annotation bbox from split tracks. CLI Agent raw-crop coverage mirrors that
  path for both `vertical_rl` and `vertical_lr` through mask and object-id
  attachments, proving the authored ruby object bbox, split annotation tracks,
  and rendered GlyphArea pixels stay tied to the same geometry.
- Debug/object/color capture geometry is measured from `LaidOutText`; pixel
  readback remains only a verification and capture output path. Native
  offscreen debug and isolated-color captures use the same GlyphArea path as
  normal body rendering, so vertical glyph cluster bboxes, object-id masks, and
  color crops share one placement model instead of falling back to glyphon's
  horizontal TextArea layout. The vendored glyphon `GlyphArea` path now carries
  the area clip rectangle into the vertex stream and discards fragments outside
  it in WGSL, so sideways, rotated, and affine-compressed glyphs obey the same
  visible bounds contract as axis-aligned `TextArea` glyphs. Debug capture alpha
  combines selected-region style alpha with capture-time effect alpha instead of
  replacing one with the other, so transparent unselected spans stay hidden
  while typewriter-controlled spans, text-combine cells, and ruby annotations
  can still be inspected at different capture times. Native GlyphArea
  preparation treats missing
  renderer cache keys as errors instead of silently skipping layout glyphs, so a
  broken shaping/cache-key mapping fails capture preparation rather than
  producing partial debug imagery. GlyphArea `GlyphSource::Custom` instances
  are also routed through glyphon's custom-glyph rasterizer instead of being
  discarded, so future inline objects, markers, or debug overlays submitted as
  pre-laid glyphs use the same capture/render path as text glyphs. Legacy
  horizontal TextArea rendering may still place ruby from shaped glyph geometry
  when no `LaidOutText` is available, but it no longer fabricates estimated
  ruby positions; if neither layout nor shaped base glyph geometry exists, the
  ruby buffer is omitted instead of emitting a compatibility placement.
- CLI regression coverage includes an `imq` native vertical capture parity check
  that compares repeated headless PNG captures for the same `vertical_rl`
  fixture. Fast native Agent coverage also exercises a `vertical_lr` fixture with
  ruby and a 4-digit text-combine run, asserting that the observed
  layer/run/ruby and text-combine cluster geometry is produced by the native
  renderer. Focused native Agent coverage also compares the same text in
  `vertical_rl` and `vertical_lr`, asserting that column progression moves left
  for `vertical_rl` and right for `vertical_lr` from renderer-derived cluster
  bboxes. The checked-in rich-text samples
  `samples/rich-text-full-grammar.arcw` and
  `samples/rich-text-windows-fonts.arcw` are now also observed through the native
  `dialogue.rich_text` layer path so their authored `vertical_rl`/`vertical_lr`
  runs, source ranges, masks, and column-shaped bounds remain covered. The same
  layer path is also captured through raw mask and object-id attachments,
  proving layer-scoped readback uses the same isolated rich-text geometry as
  the rendered pixels. The same samples are also captured as full-frame native
  PNGs while asserting the vertical runs stay column-shaped in the capture
  report, so the acceptance path is not limited to cropped layer images.
  `rich_text_cluster` child objects expose glyph-cluster source ranges and
  renderer-facing orientation/vertical-form metadata plus object-local
  color/object-id/mask capture refs; the full-grammar sample test follows a
  vertical cluster mask URI and asserts the raw capture contains rendered
  pixels. Focused native Agent coverage checks sideways, upright-alternate,
  rotated-alternate, and text-combine-upright cluster metadata. The
  text-combine-upright path is also verified through raw native mask and
  object-id crops so compressed multi-glyph cells expose object-local readback
  from the same GlyphArea geometry and stable object color as the observed
  cluster bbox. Mirrored `vertical_lr` coverage also captures text-combine mask
  and object-id attachments as raw RGBA, tying the observed
  `text_combine_upright` bbox, stable object color, and rendered GlyphArea
  pixels together alongside the existing rightward column-progression geometry
  test. It also checks adjacent
  `vertical_rl` and `vertical_lr` ruby annotations through separate
  `ruby_base_bbox` / `ruby_annotation_bbox` metadata so collision separation is
  observable in Agent output, and captures a `vertical_lr` ruby object as raw
  native mask and object-id crops to verify that rendered ruby/base pixels and
  stable object color stay inside the observed base/annotation geometry. Long
  `vertical_rl` ruby annotations are also observed through the same native Agent
  path, asserting that the base bbox expands along the vertical inline axis, the
  annotation remains on the correct ruby track, and the authored ruby object's
  mask/crop geometry covers both base and annotation bboxes. Long ruby
  base-expansion Agent coverage now runs
  in both `vertical_rl` and `vertical_lr`, checking that the expanded base and
  side-specific annotation track are observable through native ruby refs in each
  writing mode; the same long ruby object is also captured as raw native masks
  and object-id attachments in both writing modes, tying the expanded ruby
  object bbox, stable object color, and rendered GlyphArea pixels together.
  `tests/fixtures/native_capture/` contains a checked-in Windows native PNG
  golden for a vertical `Tu`/`Tr` alternate fixture plus checked-in loose/normal
  JLREQ preset PNG goldens for the repeated-leader column-planning fixture, and
  a `vertical_lr` ruby/text-combine PNG golden that covers mirrored column
  progression, ruby annotation placement, sideways Latin, upright punctuation,
  and 4-digit text-combine-upright rendering. The
  CLI test compares fresh native captures against those PNGs with bounded `imq`
  MSE/MAE drift when both Windows fonts and the `imq` binary are available. The
  normal CLI test path also validates the checked-in fixture structure, PNG
  dimensions, and loose vs normal preset image distinctness without requiring
  renderer-exact pixel parity, so broken visual fixtures are caught before the
  Tier2 `imq` job is run.
  The expanded JLREQ pair profile is also exercised through native Agent
  fixtures that observe `dialogue.rich_text` cluster geometry for leader marks,
  compact bracket pairs, small kana, prolonged-sound marks, iteration marks,
  and strict middle-dot pairs in vertical text, matching the published JLREQ
  line-composition treatment of punctuation classes and unbreakable character
  sequences (`https://www.w3.org/TR/2008/WD-jlreq-20081015/`, section 3.1).
  The strict middle-dot pair is also captured as raw native mask and object-id
  crops, tying the observed no-break pair geometry and stable object color back
  to rendered GlyphArea pixels. The normal-preset prolonged-sound mark path is
  likewise captured as raw native mask and object-id crops, tying rotated
  alternate GlyphArea pixels to the no-break mark-with-previous-cluster
  geometry. Small-kana no-break placement is also captured as raw native mask
  and object-id crops, tying upright-alternate glyph pixels to the observed
  same-column geometry. Iteration-mark placement is also captured as raw native
  mask and object-id crops, tying the mark-with-previous-cluster geometry to
  rendered GlyphArea pixels. Compact bracket-pair placement is also captured as
  raw native mask and object-id crops, tying rotated alternate bracket glyph
  pixels to the no-break pair geometry. Leader-mark placement is also captured
  as raw native mask and object-id crops, tying its no-break same-column
  geometry to rendered GlyphArea pixels.
  Native Agent coverage also checks hanging punctuation at a vertical column
  end and half-cell punctuation compression for adjacent Japanese punctuation,
  so those JLREQ placement decisions are visible in observed glyph-cluster
  bboxes and not only in Sans I/O layout unit tests. The compressed punctuation
  path is also exercised through raw native mask and object-id crops for `、`,
  verifying that half-cell advance geometry, stable object color, and the
  rendered upright-alternate glyph pixels remain tied to the same cluster bbox.
  It also checks that
  line-end-prohibited opening punctuation moves to the next vertical column,
  matching the JLREQ 3.1.7/3.1.8 treatment of characters not starting or ending
  lines; that moved opening-punctuation cluster is also covered by raw native
  mask and object-id crops, so its rendered pixels, stable object color, crop
  origin, and observed post-kinsoku bbox are verified together. Mirrored
  `vertical_lr` native Agent coverage now checks the same line-end
  opening-punctuation movement and half-cell hanging punctuation with rightward
  column progression, so the JLREQ edge behavior is not only proven for
  `vertical_rl`. The `vertical_lr` moved opening-punctuation and hanging
  punctuation clusters are captured as raw native mask and object-id crops,
  tying the mirrored Agent bboxes, stable object colors, and crop origins back
  to rendered GlyphArea pixels. Preset-specific native Agent coverage compares
  `jlreq=loose` and
  `jlreq=normal` on the same repeated leader-mark paragraph and asserts that the
  observed column geometry changes with the selected strictness preset. Broader
  paragraph-style native Agent coverage now combines comma compression,
  iteration marks, bracket grouping, small kana, middle dots, repeated leaders,
  an overhanging leader chain, and multi-column DP placement in mirrored
  `vertical_rl` and `vertical_lr` fixtures so the integrated JLREQ behavior is
  observable beyond isolated punctuation-pair tests in both column-progression
  directions. The Sans I/O planner also covers a longer leader chain whose
  trailing separation-prohibited suffix requires more than one body cell of
  allowed overhang before ordinary text can continue in the next column.
  Mirrored `vertical_lr` coverage checks the same overhanging leader-chain rule
  with rightward column progression. Sans I/O layout coverage also mirrors
  small-kana, prolonged-sound mark, middle-dot, and iteration-mark column-head
  prohibitions in `vertical_lr`, keeping the JLREQ suffix behavior symmetric
  across both vertical writing modes before native Agent geometry consumes it.
  Native Agent raw-crop coverage also mirrors the small-kana, prolonged-sound,
  and iteration-mark `vertical_lr` paths through mask and object-id attachments,
  tying the mirrored observed bboxes and stable object colors back to rendered
  GlyphArea pixels.
- Remaining work includes JLREQ refinements beyond the current kinsoku,
  separation, punctuation-compression, half-cell hanging, generated range table,
  generated strictness-aware pair/cost table, and paragraph-DP column planner,
  especially adding more published JLREQ paragraph examples and edge-case
  fixtures beyond the current visual-golden, paragraph, and punctuation-pair
  native Agent coverage.

## Explicit Defaults

- Repository workflow remains `main` only unless a branch is explicitly
  requested.
- Existing public rich-text syntax is reused; no alternate vertical-writing
  syntax is introduced.
- Backward compatibility layers are not added for internal renderer/layout
  refactors. Replace the old internal model directly and fix call sites.
- Layout correctness takes priority over short-term rendering shortcuts.
