# Reactive UI Style Sample

## Summary

Added `samples/reactive-ui-style` as the current best runnable sample for
reactive UI styling.

The sample has two layers:

- `samples/reactive-ui-style/src/main.arcw` uses the existing player-rendered
  dialogue/choice path, so hover and pressed choice styling can be exercised
  through the shared native/web renderer path already used by parity fixtures.
- `samples/reactive-ui-style/.arcweft/content/ui.program.json`,
  `ui.style.json`, and `ui.text.json` record the retained product UI style
  contract, including both Arcweft-style source identity and CSS source identity
  plus `hover`, `active`, `disabled`, and `focus_visible` selector rules.

`just reactive-ui-style-sample` builds
`web/local/reactive-ui-style.awfb` and writes retained UI interaction SVG
artifacts through the existing `arcweft-render-wgpu` showcase:

- `target/reactive-ui-style/interaction-states/neutral.svg`
- `target/reactive-ui-style/interaction-states/hovered.svg`
- `target/reactive-ui-style/interaction-states/focused.svg`
- `target/reactive-ui-style/interaction-states/pressed.svg`

## Current Implementation Boundary

The retained UI substrate already has typed hover/focus/pressed style resolution:

- `arcweft-ui::UiStyleTable`
- `arcweft-ui::UiInteractionSelector`
- `arcweft-ui::DisplayList::resolve_interaction_styles`
- `arcweft-render-wgpu::ui::ViewPaintPlan`
- `arcweft-render-wgpu/examples/ui_interaction_showcase.rs`

The product bundle substrate also stores the intended style contract:

- `UiProgramResource`
- `UiStyleResource`
- `ViewStyleSelectorPart::Interaction`
- `StyleSourceIdentity` for both Arcweft and CSS authoring sources

The missing production connection is player/runtime use of those product UI
resources to instantiate a retained `UiLayerOutput` for each frame. Today the
sample's visible player path is dialogue/choice UI, while the product retained
UI sidecars are carried in the AWFB and validated as data.

## CSS Support Snapshot

Already represented in current substrate:

- direct solid/rounded rects, borders, images, linear gradients, rectangular
  clips, opacity, transforms, and text placeholders through Takumi/direct UI
  lowering;
- compositing scene/effect contracts for `filter`, `backdrop-filter`, `mask`,
  `clip-path`, and `mix-blend-mode`;
- renderer planning for blur, drop-shadow, masks, clip geometry, and many blend
  modes.

Not end-to-end player-supported yet:

- authored CSS pseudo-state selectors such as `:hover`, `:active`,
  `:focus-visible`, `:disabled` resolving from live player interaction state;
- authored Arcweft UI style syntax plus CSS style source layers resolving into
  the same retained UI frame;
- CSS cascade/specificity/inheritance/custom properties/media/container query
  behavior as a product UI contract;
- flex/grid/positioning/min-max/aspect-ratio/gap layout coverage exposed as
  stable native/web visual fixtures;
- CSS transitions, keyframes, animation timelines, easing, and reduced-motion
  behavior;
- explicit advanced value gaps such as `filter: url(...)`, `clip-path: url(...)`,
  CSS `path()` clip tessellation, `mask: element(...)`, gradient masks, full
  box-shadow parity, and HSL-family blend modes.

## Follow-Up Requests

- `docs/reviews/requests/2026-06-30-seq-06.11-reactive-ui-authoring-style-resolution-package.md`
- `docs/reviews/requests/2026-06-30-seq-06.11a-css-computed-style-direct-paint-extractor-package.md`
- `docs/reviews/requests/2026-06-30-seq-06.11b-ui-scene-compositor-player-path-integration-package.md`
- `docs/reviews/requests/2026-06-30-seq-06.12-css-layout-cascade-coverage-package.md`
- `docs/reviews/requests/2026-06-30-seq-06.13-css-motion-effects-coverage-package.md`
- `docs/reviews/requests/2026-06-30-seq-06.13a-clip-path-and-mask-render-closure-package.md`
- `docs/reviews/requests/2026-06-30-seq-06.13b-box-shadow-and-blend-render-closure-package.md`

Seq06.11 is the required implementation path before the sample can become a
fully player-owned retained UI sample. Seq06.11a and seq06.11b are split out
from a follow-up review against an older `main` snapshot that called out two
practical missing connections: CSS computed style to direct paint extraction,
and `ViewScene` / `ViewCompositor` integration into the normal player path.

Seq06.12, seq06.13, seq06.13a, and seq06.13b can be designed in parallel, but
production integration should not bypass the seq06.11b frame connection.
