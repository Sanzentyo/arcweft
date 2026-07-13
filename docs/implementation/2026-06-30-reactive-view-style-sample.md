# Reactive View Style Sample

> **Superseded (2026-07-13):** Arcweft now has a native-only typed Style path. The CSS authoring, Takumi adapter, and CSS-named sample/tooling paths described below were removed; the remaining text is retained only as historical implementation evidence.

## Summary

Added `samples/reactive-view-style` as the current best runnable sample for
reactive View styling.

The sample has two layers:

- `samples/reactive-view-style/src/main.arcw` uses the existing player-rendered
  dialogue/choice path, so hover and pressed choice styling can be exercised
  through the shared native/web renderer path already used by parity fixtures.
- `samples/reactive-view-style/.arcweft/content/view.program.json`,
  `view.style.json`, and `view.text.json` record the retained product View style
  contract, including both Arcweft-style source identity and CSS source identity
  plus `hover`, `active`, `disabled`, and `focus_visible` selector rules.

`just reactive-view-style-sample` builds
`web/local/reactive-view-style.awfb` and writes retained View interaction SVG
artifacts through the existing `arcweft-render-wgpu` showcase:

- `target/reactive-view-style/interaction-states/neutral.svg`
- `target/reactive-view-style/interaction-states/hovered.svg`
- `target/reactive-view-style/interaction-states/focused.svg`
- `target/reactive-view-style/interaction-states/pressed.svg`

## Current Implementation Boundary

The retained View substrate already has typed hover/focus/pressed style resolution:

- `arcweft-view::ViewStyleTable`
- `arcweft-view::ViewInteractionSelector`
- `arcweft-view::DisplayList::resolve_interaction_styles`
- `arcweft-render-wgpu::view::ViewPaintPlan`
- `arcweft-render-wgpu/examples/view_interaction_showcase.rs`

The product bundle substrate also stores the intended style contract:

- `ViewProgramResource`
- `ViewStyleResource`
- `ViewStyleSelectorPart::Interaction`
- `StyleSourceIdentity` for both Arcweft and CSS authoring sources

The missing production connection is player/runtime use of those product View
resources to instantiate a retained `ViewLayerOutput` for each frame. Today the
sample's visible player path is dialogue/choice View, while the product retained
UI sidecars are carried in the AWFB and validated as data.

## CSS Support Snapshot

Already represented in current substrate:

- direct solid/rounded rects, borders, images, linear gradients, rectangular
  clips, opacity, transforms, and text placeholders through Takumi/direct View
  lowering;
- compositing scene/effect contracts for `filter`, `backdrop-filter`, `mask`,
  `clip-path`, and `mix-blend-mode`;
- renderer planning for blur, drop-shadow, masks, clip geometry, and many blend
  modes.

Not end-to-end player-supported yet:

- authored CSS pseudo-state selectors such as `:hover`, `:active`,
  `:focus-visible`, `:disabled` resolving from live player interaction state;
- authored Arcweft View style syntax plus CSS style source layers resolving into
  the same retained View frame;
- CSS cascade/specificity/inheritance/custom properties/media/container query
  behavior as a product View contract;
- flex/grid/positioning/min-max/aspect-ratio/gap layout coverage exposed as
  stable native/web visual fixtures;
- CSS transitions, keyframes, animation timelines, easing, and reduced-motion
  behavior;
- explicit advanced value gaps such as `filter: url(...)`, `clip-path: url(...)`,
  CSS `path()` clip tessellation, `mask: element(...)`, gradient masks, full
  box-shadow parity, and HSL-family blend modes.

## Follow-Up Requests

- `docs/reviews/requests/2026-06-30-seq-06.11-reactive-view-authoring-style-resolution-package.md`
  now targets the renamed `samples/reactive-view-style` sample and View
  authoring terminology.
- `docs/reviews/requests/2026-06-30-seq-06.11a-css-computed-style-direct-paint-extractor-package.md`
- `docs/reviews/requests/2026-06-30-seq-06.11b-view-scene-compositor-player-path-integration-package.md`
- `docs/reviews/requests/2026-06-30-seq-06.12-css-layout-cascade-coverage-package.md`
- `docs/reviews/requests/2026-06-30-seq-06.13-css-motion-effects-coverage-package.md`
- `docs/reviews/requests/2026-06-30-seq-06.13a-clip-path-and-mask-render-closure-package.md`
- `docs/reviews/requests/2026-06-30-seq-06.13b-box-shadow-and-blend-render-closure-package.md`

Seq06.11 is the required implementation path before the sample can become a
fully player-owned retained View sample. Seq06.11a and seq06.11b are split out
from a follow-up review against an older `main` snapshot that called out two
practical missing connections: CSS computed style to direct paint extraction,
and `ViewScene` / `ViewCompositor` integration into the normal player path.

Seq06.12, seq06.13, seq06.13a, and seq06.13b can be designed in parallel, but
production integration should not bypass the seq06.11b frame connection.
