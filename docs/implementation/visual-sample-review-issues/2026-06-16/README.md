# Visual Sample Review - 2026-06-16

> Status: historical evidence. The 2026-07-13 CSS Ruby parity review superseded
> the fixed `0.36em` cell-overlap conclusion below. Chromium's measured CSSOM
> rectangles overlap because of internal font metrics; Arcweft now stacks its
> canonical base and annotation cells without overlap so visible glyph ink does
> not collide.

This folder keeps the browser and native PNG captures used to review horizontal
ruby placement after aligning horizontal ruby GlyphAreas and Arcweft layout
geometry with an HTML `<ruby><rb><rt>` reference.

Chromium was measured with a 30px base font, 13px ruby font, and
`line-height: 1`. The browser reference places `rt.bottom` about 4.67px below
`rb.top`. Arcweft now models the same horizontal over-ruby relation as a 0.36em
natural annotation overlap before applying explicit `ruby_gap`. Integer Agent
observation bboxes round the representative cases to a 4px overlap.

| File | Purpose |
|---|---|
| `html-horizontal-ruby-reference.html` | Local browser reference source for ordinary, long-ruby, and short-ruby horizontal cases. |
| `html-horizontal-ruby-reference.png` | Chromium screenshot of the reference ruby layout. |
| `html-horizontal-ruby-reference-bboxes.json` | Browser-measured `<rb>` / `<rt>` bboxes. |
| `horizontal-ruby-html-comparison.json` | Reduced HTML-vs-Arcweft bbox comparison for `変な夢`, `政`, and `中央の帝国将官たち`. |
| `horizontal-ruby-showcase-after.png` | Showcase sample after horizontal ruby y-axis placement review. |
| `horizontal-ruby-full-grammar-after.png` | Full grammar sample with multiple horizontal ruby annotations after the placement fix. |
| `horizontal-ruby-extreme-object-after.png` | Full grammar extreme horizontal ruby object containing long annotation over short base and short annotation over long base. |
