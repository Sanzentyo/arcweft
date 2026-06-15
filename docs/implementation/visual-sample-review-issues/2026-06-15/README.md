# Sample Visual Review Issues - 2026-06-15

This directory keeps sample-rendering PNGs that showed visible issues during
manual review. Do not remove these PNGs while the issue remains open. When a
fix lands, either update the status to closed with the validation command and
replacement evidence, or move the superseded issue to a clearly marked closed
section.

All PNGs in this directory were captured with:

```bash
cargo build --release -p arcweft-cli --quiet
target\release\arcw.exe agent observe <sample.arcw> --json --image png --out <issue>.png --mode drain --steps 4 --max-ops 128
```

Object/page captures additionally used `--object <object-id>` and `--page 0`.
The full `*.observe.json` files are intentionally not checked in because they
are large; regenerate them with the command above when geometry details are
needed. `capture-results.json` records the captured PNG names, byte sizes, and
command exit status.

The provenance validation PNGs in this directory were captured with the same
release `agent observe` path after adding `dialogue defaults` and line-level
rich-text overrides to the samples:

```bash
target\release\arcw.exe agent observe samples/rich-text-showcase.arcw --json --image png --out docs\implementation\visual-sample-review-issues\2026-06-15\provenance-rich-text-showcase.png --mode drain --steps 4 --max-ops 128
target\release\arcw.exe agent observe samples/rich-text-full-grammar.arcw --json --image png --out docs\implementation\visual-sample-review-issues\2026-06-15\provenance-rich-text-full-grammar.png --mode drain --steps 4 --max-ops 128
```

The generated observation JSON was reviewed but not checked in. It confirmed
that `rich_text.ruby.size` and `rich_text.ruby.gap` contributions from
`dialogue_defaults` are shadowed by line-level `rich_text` overrides, while raw
authored values such as `12px` remain available in `style_contributions`.

## Open Issues

No open issues.

## Current Formatting Support State

Ruby placement is exposed as `ruby_over`, `ruby_under`, and
`ruby_inter_character`. The 2026-06-15 follow-up added explicit presentation
fields for ruby typography: `ruby_size`, `ruby_gap`, `ruby_overhang`, and
`ruby_collision_gap` on layout selectors such as:

```arcw
[.ruby_over ruby_size=13px ruby_gap=1px ruby_overhang=4px ruby_collision_gap=3px]
|[夢](ゆめ)
[/]
```

These values now flow through rich-text presentation data into Sans I/O layout,
and native ruby style construction preserves the resolved presentation instead
of resetting ruby annotations to default metadata. The remaining open issues in
this directory are not closed until the corresponding sample PNGs are
regenerated, reviewed, and either shown fixed or replaced with narrower
follow-up issues.

## Closed Issues

| ID | Closed in | Evidence | Validation |
| --- | --- | --- | --- |
| `SVR-2026-06-15-001` | Ruby typography is configurable through `ruby_size`, `ruby_gap`, `ruby_overhang`, and `ruby_collision_gap`; the engine default gap now matches the documented 2px value instead of deriving a looser `font_size * 0.2`, and the full-grammar sample line keeps its explicit 1px gap override with a 13px ruby size. | Original: [`issue-001-ruby-gap-full-frame.png`](issue-001-ruby-gap-full-frame.png); fixed: [`issue-001-fixed-ruby-gap-full-frame.png`](issue-001-fixed-ruby-gap-full-frame.png). | `cargo test -p arcweft-text-layout ruby -- --nocapture`; `target\release\arcw.exe agent observe samples\rich-text-full-grammar.arcw --json --image png --out docs\implementation\visual-sample-review-issues\2026-06-15\issue-001-fixed-ruby-gap-full-frame.png --mode drain --steps 16 --max-ops 256` |
| `SVR-2026-06-15-009` | Horizontal ruby placement now uses font-size-centered horizontal glyph bounds instead of the whole line box, native ruby buffers use tight ruby metrics, and the full-grammar sample uses a readable 13px ruby override so visual gap review matches the intended typography. | Fixed: [`issue-009-fixed-horizontal-ruby-ink-bounds-full-frame.png`](issue-009-fixed-horizontal-ruby-ink-bounds-full-frame.png); zoom: [`issue-009-fixed-horizontal-ruby-ink-bounds-ruby-zoom.png`](issue-009-fixed-horizontal-ruby-ink-bounds-ruby-zoom.png). | `cargo test -p arcweft-text-layout ruby -- --nocapture`; `cargo test -p arcweft-player-native native_ruby_style_uses_tight_line_height -- --nocapture`; `cargo test -p arcweft-cli --test check agent_observe_native_renderer_writes_object_raw_crop -- --exact --nocapture`; `cargo run -p arcweft-cli --quiet -- agent observe samples\rich-text-full-grammar.arcw --json --image png --out docs\implementation\visual-sample-review-issues\2026-06-15\issue-009-fixed-horizontal-ruby-ink-bounds-full-frame.png --mode drain --steps 16 --max-ops 256` |
| `SVR-2026-06-15-002` | Native textbox object capture bounds now union the selected textbox bbox with native-measured TextRun/Ruby/GlyphCluster bounds for the requested page, so ruby annotation extents and existing debug padding are included before cropping. | Original: [`issue-002-ruby-object-crop-full-grammar.png`](issue-002-ruby-object-crop-full-grammar.png); fixed: [`issue-002-fixed-native-textbox-ruby-crop-full-grammar.png`](issue-002-fixed-native-textbox-ruby-crop-full-grammar.png). | `target\release\arcw.exe agent observe samples\rich-text-full-grammar.arcw --json --image png --out docs\implementation\visual-sample-review-issues\2026-06-15\issue-002-fixed-native-textbox-ruby-crop-full-grammar.png --mode drain --steps 16 --max-ops 256 --object object.dialogue.0.0` |
| `SVR-2026-06-15-003` | The same native-measured textbox crop expansion keeps the `星屑` ruby annotation away from the top crop edge in the showcase object capture. | Original: [`issue-003-ruby-object-crop-showcase.png`](issue-003-ruby-object-crop-showcase.png); fixed: [`issue-003-fixed-native-textbox-ruby-crop-showcase.png`](issue-003-fixed-native-textbox-ruby-crop-showcase.png). | `target\release\arcw.exe agent observe samples\rich-text-showcase.arcw --json --image png --out docs\implementation\visual-sample-review-issues\2026-06-15\issue-003-fixed-native-textbox-ruby-crop-showcase.png --mode drain --steps 16 --max-ops 256 --object object.dialogue.0.0` |
| `SVR-2026-06-15-004` | Native textbox object capture bounds now include measured vertical columns before crop, so the mixed horizontal/vertical showcase object includes the full vertical run. | Original: [`issue-004-vertical-mixed-crop-showcase.png`](issue-004-vertical-mixed-crop-showcase.png); fixed: [`issue-004-fixed-native-textbox-vertical-crop-showcase.png`](issue-004-fixed-native-textbox-vertical-crop-showcase.png). | `target\release\arcw.exe agent observe samples\rich-text-showcase.arcw --json --image png --out docs\implementation\visual-sample-review-issues\2026-06-15\issue-004-fixed-native-textbox-vertical-crop-showcase.png --mode drain --steps 16 --max-ops 256 --object object.dialogue.0.3` |
| `SVR-2026-06-15-005` | The Windows-font vertical sample uses the same native-measured textbox crop expansion and now includes the visible vertical text within the object PNG. | Original: [`issue-005-vertical-mixed-crop-windows-fonts.png`](issue-005-vertical-mixed-crop-windows-fonts.png); fixed: [`issue-005-fixed-native-textbox-vertical-crop-windows-fonts.png`](issue-005-fixed-native-textbox-vertical-crop-windows-fonts.png). | `target\release\arcw.exe agent observe samples\rich-text-windows-fonts.arcw --json --image png --out docs\implementation\visual-sample-review-issues\2026-06-15\issue-005-fixed-native-textbox-vertical-crop-windows-fonts.png --mode drain --steps 16 --max-ops 256 --object object.dialogue.0.0` |
| `SVR-2026-06-15-006` | `horizontal_tb` rich text now performs deterministic textbox-width wrapping before placing a cluster that would exceed the layout width, and native textbox object captures expand to include wrapped rows. | Original: [`issue-006-long-line-clips-windows-fonts.png`](issue-006-long-line-clips-windows-fonts.png); fixed: [`issue-006-fixed-horizontal-wrap-windows-fonts.png`](issue-006-fixed-horizontal-wrap-windows-fonts.png). | `cargo test -p arcweft-text-layout horizontal_layout -- --nocapture`; `cargo test -p arcweft-cli --test check agent_observe_native_textbox_capture_wraps_long_horizontal_lines -- --nocapture`; `target\release\arcw.exe agent observe samples\rich-text-windows-fonts.arcw --json --image png --out docs\implementation\visual-sample-review-issues\2026-06-15\issue-006-fixed-horizontal-wrap-windows-fonts.png --mode drain --steps 16 --max-ops 256 --object object.dialogue.0.4` |
| `SVR-2026-06-15-007` | `samples/rich-text-full-grammar.arcw` keeps the same inferred selector coverage but splits style/layout, transform, and effect examples across `[r]` rows so offset/rotate/scale/wave/shake/typewriter runs remain inspectable. | Original: [`issue-007-effect-transform-overlap-full-grammar.png`](issue-007-effect-transform-overlap-full-grammar.png); fixed: [`issue-007-fixed-split-effect-transform-full-grammar.png`](issue-007-fixed-split-effect-transform-full-grammar.png). | `target\release\arcw.exe agent observe samples\rich-text-full-grammar.arcw --json --image png --out docs\implementation\visual-sample-review-issues\2026-06-15\issue-007-fixed-split-effect-transform-full-grammar.png --mode drain --steps 16 --max-ops 256 --object object.dialogue.0.4` |
| `SVR-2026-06-15-008` | `samples/rich-text-full-grammar.arcw` pins the escape-syntax demonstration span to `Consolas`, avoiding the Windows/Japanese U+005C Yen-glyph presentation for literal syntax review. | Original: [`issue-008-escape-yen-glyph-full-grammar.png`](issue-008-escape-yen-glyph-full-grammar.png); fixed: [`issue-008-fixed-escape-consolas-full-grammar.png`](issue-008-fixed-escape-consolas-full-grammar.png). | `target\release\arcw.exe agent observe samples\rich-text-full-grammar.arcw --json --image png --out docs\implementation\visual-sample-review-issues\2026-06-15\issue-008-fixed-escape-consolas-full-grammar.png --mode drain --steps 16 --max-ops 256 --object object.dialogue.0.7` |
