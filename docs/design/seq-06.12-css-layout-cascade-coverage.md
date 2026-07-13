# Seq06.12 CSS layout/cascade coverage implementation design

> **Superseded Style-path premise (2026-07-13):** The Arcweft CSS/Takumi authoring, lowering, and evidence path assumed below was removed by the [native-only typed Style path](../implementation/native-only-style-path-2026-07-13.md). The body is retained as historical rationale and is not a current Style contract.

## Goal

The first production cut makes CSS support explicit, deterministic, and shared
between native and web. Takumi remains the CSS cascade/layout/stacking source;
Arcweft records coverage/evidence and renders via renderer-owned `ViewScene` data.
No browser layout, DOM text, canvas 2D, Takumi CPU raster, or backend-only path is
introduced.

## New module

`crates/arcweft-takumi-adapter/src/coverage.rs` owns the coverage contract:

- `CssCoverageFeature` and `CssCoverageStatus` define the product matrix.
- `CSS_COVERAGE_MATRIX` is a stable in-code support table.
- `CssSelectorCoverage` parses selector support and specificity.
- `CssCascadeLayer`, `CssSpecificity`, `CssCascadePriority`, and
  `CssMatchedDeclaration` define deterministic cascade winner ordering.
- `CssCoverageReport` analyzes stylesheet text into declarations, at-rule
  coverage, and typed diagnostics.
- `CssComputedStyleEvidence`, `CssSelectorWinnerEvidence`, and
  `CssLayoutBoxEvidence` provide the evidence payload shape requested by the
  sequence.

The module is intentionally data-oriented and Sans I/O. It does not call platform
APIs, does not read files, and does not attempt to reimplement Takumi layout.

## Selector subset

Supported:

- element selectors emitted by the adapter (`div`, `span`, `img`);
- class selectors emitted by the adapter (`aw-container`, `aw-block`, `aw-text`, etc.);
- id selectors (`aw-node-{NodeId}`);
- part selection through `[data-aw-part="..."]` or `[part="..."]` attributes;
- descendant and child combinators;
- interaction pseudo-states `:hover`, `:focus`, `:active`, and `:disabled` as
  product data aligned with retained interaction state.

Rejected or diagnosed:

- pseudo-elements are intentionally rejected because they synthesize nodes or
  create a separate part model;
- structural and selector-list pseudo functions such as `:nth-child`, `:has`,
  `:is`, `:where`, and `:not` emit `UnsupportedCssSelector`.

Specificity is `(id_count, class_attribute_pseudo_count, element_count)`. The
coverage scanner gives unsupported pseudo functions no silent specificity magic;
it emits diagnostics instead.

## Cascade and layer policy

`CssCascadePriority` orders declarations by:

1. `!important` bit;
2. layer order;
3. specificity;
4. source order.

Layer order for this cut:

```text
ArcweftBase < ArcweftView < CssReset < CssBase < CssView < CssInline
```

This intentionally makes CSS author layers able to override Arcweft base data,
while still preserving deterministic ordering between Arcweft style patches and
CSS sheets. Inline CSS is the strongest non-important author layer.

## Token and custom-property policy

CSS custom property declarations are represented as product data only. They are
not lowered into `StyleTokenBinding` yet because that would require a bidirectional
policy between Arcweft style tokens and CSS variables.

For this cut:

- `--name: value` is recorded as `ProductDataOnly`;
- `var(--name)` is considered resolved when a declaration for `--name` appears in
  the analyzed CSS text;
- `var(--missing, fallback)` is accepted because it has a fallback;
- `var(--missing)` emits `UnresolvedCssVariable`;
- Arcweft style tokens remain owned by `arcweft-view::style_authoring`; CSS
  variables do not override them yet.

## Layout subset

Supported first cut:

- block and block-like retained View containers;
- inline retained View containers and text participants from the existing text substrate;
- flexbox container/item properties and deterministic gap/padding/margin behavior;
- width, height, min/max sizes, aspect ratio;
- position/inset/z-index via Takumi layout/stacking;
- overflow visible/hidden/clip evidence.

Out of scope:

- grid rendering: accepted as CSS text but emits structured diagnostics;
- container queries: intentionally rejected;
- transitions, keyframes, timelines: intentionally rejected and left to seq06.13.

## Environment queries

Arcweft already has a pure presentation environment for color scheme, contrast,
reduced motion, and text scale. The first cut records media/container/query
coverage but does not branch retained View rendering on CSS media queries yet.

- `@media (prefers-color-scheme: ...)`: structured diagnostic.
- `@media (prefers-contrast: ...)`: structured diagnostic.
- `@media (prefers-reduced-motion: ...)`: structured diagnostic.
- `@media (arcweft-text-scale: ...)`: reserved structured diagnostic.
- viewport media queries: structured diagnostic.
- `@container`: intentionally rejected.

## Evidence format

The first cut uses `coverage.css` as parser input and asserts the resulting
typed `CssCoverageReport`: selector support, cascade winners, declarations,
diagnostic codes, and invalidation classes. This is executable coverage of the
contract rather than a check that expected words occur in fixture files.

Computed-style JSON and visual-smoke manifests are not part of the current
contract. A future artifact format must be produced from the typed report,
decoded against a schema, and compared as a complete deterministic value before
it can be treated as validation evidence. Placeholder JSON and source-spelling
scanners are explicitly insufficient.
