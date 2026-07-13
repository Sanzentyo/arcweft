# CSS layout/cascade support matrix — seq06.12 first cut

> **Superseded Style-path premise (2026-07-13):** The Arcweft CSS/Takumi authoring, lowering, and evidence path assumed below was removed by the [native-only typed Style path](../implementation/native-only-style-path-2026-07-13.md). The body is retained as historical rationale and is not a current Style contract.

Status definitions:

- **Supported now**: accepted by the retained View CSS path and expected to flow through Takumi layout/cascade into Arcweft-owned direct/composited `ViewScene` data.
- **Product data only**: represented by Arcweft data structures or evidence, but not yet fully rendered through the player path.
- **Structured diagnostic**: accepted at parse/coverage time, but emits a typed diagnostic rather than a silent approximation.
- **Intentionally rejected**: excluded from this cut because supporting it would synthesize nodes, require an unsupported runtime query model, or belong to a later sequence.

| Area | Feature | Status | Decision |
| --- | --- | --- | --- |
| Selector | element selectors (`div`, `span`, `img`) | Supported now | Adapter emits deterministic tag names from retained fragment kinds. |
| Selector | class selectors (`.aw-container`, `.aw-text`) | Supported now | Adapter already emits stable class names. |
| Selector | id selectors (`#aw-node-7`) | Supported now | Node IDs are deterministic per retained frame. |
| Selector | part selectors | Supported now | Use Arcweft-owned `[data-aw-part="..."]` attribute; CSS pseudo-elements are not used. |
| Selector | descendant combinator | Supported now | Takumi computes matching before Arcweft lowering. |
| Selector | child combinator (`>`) | Supported now | Takumi computes matching before Arcweft lowering. |
| Selector | `:hover`, `:focus`, `:active`, `:disabled` | Product data only | Maps to retained interaction state; final player-state feed is gated on seq06.11. |
| Selector | pseudo-elements (`::before`, `::after`, `::part`) | Intentionally rejected | They synthesize or remap nodes outside this retained View cut. |
| Selector | structural selectors (`:nth-child`, `:has`, `:not`, `:is`, `:where`) | Structured diagnostic | Not approximated; specificity/evidence emits `UnsupportedCssSelector`. |
| Cascade | Arcweft base/view layers | Supported now | Ordered before CSS author layers. |
| Cascade | CSS reset/base/view/inline layers | Supported now | Deterministic first-cut order: reset < base < view < inline after Arcweft layers. |
| Cascade | specificity | Supported now | Tuple `(ids, classes_or_attributes_or_pseudos, elements)`. |
| Cascade | source order | Supported now | Later source order wins when importance, layer, and specificity tie. |
| Cascade | `!important` | Supported now | Supported as priority bit; important declarations outrank non-important declarations. |
| Cascade | inheritance fields | Product data only | Evidence names inheritable fields; full snapshot extraction from Takumi remains a follow-up. |
| Tokens | CSS custom property declarations (`--x`) | Product data only | Recorded in coverage data; not lowered to `StyleTokenBinding` in this cut. |
| Tokens | `var(--x)` resolution | Supported now for coverage | Declared variable or fallback is accepted; missing variable emits `UnresolvedCssVariable`. |
| Tokens | Arcweft style tokens + CSS variables | Product data only | Arcweft tokens remain authoritative product data; CSS variables do not override token registry entries yet. |
| Layout | block / block-like retained View | Supported now | Takumi layout result is the source; Arcweft lowers layout boxes into `ViewScene` evidence. |
| Layout | inline retained View | Supported now | Inline containers/text participate through Takumi and existing seq06.10/06.10a text substrate. |
| Layout | flexbox | Supported now | First cut supports flex container/item properties, gap, margin, padding, and size constraints. |
| Layout | grid | Structured diagnostic | `display:grid` and `grid-*` declarations are not rendered in this cut. |
| Layout | margin / padding / gap | Supported now | Classified as layout-scene invalidation. |
| Layout | width / height / min / max / aspect-ratio | Supported now | Classified as layout-scene invalidation. |
| Layout | position / inset / z-index | Supported now | Lowered through Takumi layout/stacking; absolute/fixed semantics remain evidence-sensitive. |
| Layout | overflow visible/hidden/clip | Supported now | Evidence distinguishes visible versus clip behavior. |
| Queries | color scheme | Structured diagnostic | Environment exists in Arcweft data, but CSS media binding is not rendered yet. |
| Queries | contrast | Structured diagnostic | Environment exists; CSS media binding is not rendered yet. |
| Queries | reduced motion | Structured diagnostic | Environment exists; animation is seq06.13, so CSS motion queries are diagnostic-only here. |
| Queries | text scale | Structured diagnostic | Arcweft environment carries text scale; CSS media syntax is reserved as `arcweft-text-scale`. |
| Queries | viewport media queries | Structured diagnostic | Accepted and diagnosed; no viewport-query branch rendering in this cut. |
| Queries | container queries | Intentionally rejected | Requires retained container-query dependency graph and invalidation model. |
| Effects | transitions / keyframes / timelines | Intentionally rejected | Non-goal for seq06.12; owned by seq06.13. |

The implementation source of truth for this matrix is `CssCoverageFeature`,
`CssCoverageStatus`, and `CSS_COVERAGE_MATRIX` in
`arcweft-takumi-adapter::coverage`.
