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

| ID | Status | PNG | Problem | Expected direction |
| --- | --- | --- | --- | --- |
| `SVR-2026-06-15-001` | open | [`issue-001-ruby-gap-full-frame.png`](issue-001-ruby-gap-full-frame.png) | Horizontal ruby appears too far above the base text in the full-frame sample. | Make ruby size/gap/offset configurable, then choose a less distant default. |
| `SVR-2026-06-15-002` | open | [`issue-002-ruby-object-crop-full-grammar.png`](issue-002-ruby-object-crop-full-grammar.png) | Object-level PNG capture cuts or nearly cuts the top of ruby annotations. | Expand object capture bounds using actual shaped ruby extents plus debug padding, not only coarse layout bounds. |
| `SVR-2026-06-15-003` | open | [`issue-003-ruby-object-crop-showcase.png`](issue-003-ruby-object-crop-showcase.png) | Ruby on `星屑` is at the crop top edge, making the object capture unreliable for review. | Same as `SVR-2026-06-15-002`; ruby debug captures need safe vertical padding. |
| `SVR-2026-06-15-004` | open | [`issue-004-vertical-mixed-crop-showcase.png`](issue-004-vertical-mixed-crop-showcase.png) | Mixed horizontal/vertical text capture clips the vertical columns at the crop edge. | Union vertical layout columns into the selected textbox/object capture bounds before cropping. |
| `SVR-2026-06-15-005` | open | [`issue-005-vertical-mixed-crop-windows-fonts.png`](issue-005-vertical-mixed-crop-windows-fonts.png) | Windows-font vertical sample has visible vertical text clipped in object capture. | Same as `SVR-2026-06-15-004`; validate with Windows default fonts. |
| `SVR-2026-06-15-006` | open | [`issue-006-long-line-clips-windows-fonts.png`](issue-006-long-line-clips-windows-fonts.png) | Long horizontal font-mix line is clipped at the right edge instead of wrapping or reporting overflow. | Define and implement textbox wrap/overflow behavior for long rich-text runs. |
| `SVR-2026-06-15-007` | open | [`issue-007-effect-transform-overlap-full-grammar.png`](issue-007-effect-transform-overlap-full-grammar.png) | Combined offset/rotate/scale/wave/shake/typewriter sample is hard to inspect because transformed runs overlap and reach crop edges. | Split effect validation samples or include transform-inflated capture bounds for debug images. |

## Current Formatting Support State

Ruby placement is exposed as `ruby_over`, `ruby_under`, and
`ruby_inter_character`. The 2026-06-15 follow-up added explicit presentation
fields for ruby typography: `ruby_size`, `ruby_gap`, `ruby_overhang`, and
`ruby_collision_gap` on layout selectors such as:

```arcw
[.ruby_over ruby_size=11px ruby_gap=1px ruby_overhang=4px ruby_collision_gap=3px]
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
| `SVR-2026-06-15-008` | `samples/rich-text-full-grammar.arcw` pins the escape-syntax demonstration span to `Consolas`, avoiding the Windows/Japanese U+005C Yen-glyph presentation for literal syntax review. | Original: [`issue-008-escape-yen-glyph-full-grammar.png`](issue-008-escape-yen-glyph-full-grammar.png); fixed: [`issue-008-fixed-escape-consolas-full-grammar.png`](issue-008-fixed-escape-consolas-full-grammar.png). | `target\release\arcw.exe agent observe samples\rich-text-full-grammar.arcw --json --image png --out docs\implementation\visual-sample-review-issues\2026-06-15\issue-008-fixed-escape-consolas-full-grammar.png --mode drain --steps 16 --max-ops 256 --object object.dialogue.0.7` |
