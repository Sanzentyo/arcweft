# Native IME renderer/editor geometry normalization

Date: 2026-07-07

This cut implements the concrete repair shape for the native IME glyph-range overlap crash observed while converting Japanese IME input in `samples/modern-feedback-ui`.

## Root-cause ranking

1. **Renderer/editor geometry contract mismatch** is the implemented root cause. `arcweft-render-wgpu` receives renderer glyph layout where multiple glyphs may map to the same editable source byte range. Passing those raw renderer glyphs into `TextEditorLayout` violates the editor geometry invariant that ranges are ordered and non-overlapping.
2. **Native IME UI suppression / white composition window behavior** is a separate earlier issue. This failure happens after Arcweft has accepted IME operations and is preparing geometry for caret, selection, and IME rectangles.
3. **Trace `composition: null` around `set_composition`** remains a trace/snapshot timing question rather than the primary crash cause. The current native trace records the focused control snapshot around routed text input; it does not yet include renderer raw glyphs or normalized editor clusters.

## Implemented repair

- `crates/arcweft-render-wgpu/src/text_editor_geometry.rs` now normalizes renderer glyph geometry at the renderer-to-editor boundary before constructing `TextEditorLayout`.
- Renderer glyphs are sorted by source range and visual position.
- Identical non-empty source ranges are merged into one editor cluster by unioning their bounds.
- Collapsed ranges are preserved when they represent distinct caret stops, such as soft-wrap or newline anchors.
- Collapsed ranges strictly inside a non-empty source cluster are dropped before editor validation so they cannot create false overlap or duplicate character geometry.
- Partial non-empty overlaps are not silently normalized. They remain mapping bugs and continue to surface through `TextEditorLayoutError::OverlappingGlyphRange`.

## Contract decisions

`TextEditorLayout` stays strict. It remains the renderer-agnostic editing geometry model used by caret placement, hit-testing, selection rectangles, composition rectangles, and IME candidate placement.

Normalization is owned by `TextEditorGeometryPump` in `arcweft-render-wgpu`, not by `arcweft-presentation`, because duplicate renderer glyph ranges are a renderer/glyphon-facing artifact. `LaidOutText` remains a renderer text-layout output and is not redefined as an editor cluster model.

The public type `TextEditorGlyphGeometry` is not renamed in this cut to avoid mixing the crash repair with a broader API churn. After this normalization boundary, values passed to `TextEditorLayout` should be treated as editor cluster geometry. A future direct rename to `TextEditorClusterGeometry` can be done as a separate internal API cleanup without compatibility aliases.

## Tests added

The `arcweft-render-wgpu` unit tests now cover:

- duplicate identical non-empty source ranges merge into one editor cluster;
- `( ﾟДﾟ)`-style mixed script / fallback duplicate ranges no longer reach `TextEditorLayout` as duplicates;
- collapsed ranges inside a non-empty cluster are dropped;
- collapsed ranges at distinct caret stops are preserved;
- partial non-empty overlaps still produce `TextEditorLayoutError::OverlappingGlyphRange`.

## Trace/schema follow-up

No trace schema change is included in this cut. The next useful trace addition is a renderer geometry diagnostic record containing:

- text control target id;
- source text length and display text length, redacted for secure controls;
- raw renderer glyph ranges and bounds;
- normalized editor cluster ranges and bounds;
- counts of merged identical ranges and dropped collapsed anchors;
- partial-overlap error fields when normalization cannot produce a valid editor layout.

That record should be emitted from the native/player trace path after focused text-control layout is produced, not from `arcweft-presentation`.

## Non-goals

- No disabling native IME or candidate conversion paths.
- No acceptance of arbitrary overlapping editor geometry in `arcweft-presentation`.
- No silent union of partial overlaps such as `18..22` and `20..25`.
- No shaping-cluster model redesign beyond identical-range normalization.
- No previous-geometry or monospaced fallback for native player layout errors.
- No checked-in real-machine traces from `target/`.

## Validation

The code change was committed through the GitHub connector. Local validation could not be executed in this environment because `cargo`, `rustc`, and `rustfmt` are not available on `PATH`.

Intended focused validation commands:

```bash
cargo test -p arcweft-render-wgpu text_editor_geometry --all-features
cargo test -p arcweft-render-wgpu --all-targets --all-features
cargo fmt --all -- --check
cargo clippy -p arcweft-render-wgpu --all-targets --all-features -- -D warnings
```

Manual native validation remains the real-machine MS IME reproduction from the review request, using `samples/modern-feedback-ui` with `--text-input-trace-out` and converting `kaomoji` to `( ﾟДﾟ)`.

## Structure audit

Changed production file:

- `crates/arcweft-render-wgpu/src/text_editor_geometry.rs`
  - owning crate: `arcweft-render-wgpu`
  - role: renderer-owned conversion from `LaidOutText` to text-editor geometry, plus focused unit tests
  - expected size after this cut: approximately 390 physical LOC
  - threshold status: below production warning/error thresholds and within the preferred ordinary responsibility module range

The checked-in structure audit command was not run because `cargo` is unavailable in this environment.
