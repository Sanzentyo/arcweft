# seq06.16.4 runtime-control style contract

## Decision

This is implemented as a narrow, typed runtime-control visual style bridge that is intentionally replaceable by the broader seq06.11 retained UI style resolver. It does not create a DOM overlay, browser-CSS path, sample-specific geometry path, or a duplicate shadow renderer.

The bridge resolves already-decoded `UiStyleResource` data into player-owned runtime control payloads:

- `UiRuntimeTextControl.style`
- `UiRuntimeActionButton.style`

`arcweft-bundle` remains Sans I/O and data-only. It performs deterministic value mapping and records structured diagnostics; it does not parse external CSS files, rasterize, allocate platform handles, or read assets.

## Data path

```text
UiStyleResource
  -> UiRuntimeControlStyle / UiRuntimeControlStyleDiagnostic
  -> UiRuntimeTextControl / UiRuntimeActionButton
  -> RuntimeTextControlLowerer / RuntimeActionButtonLowerer
  -> RenderControlStyle on RenderTextInputControl / RenderActionButton
  -> SharedFramePlanner rectangles/text/focus ring/border/shadow plans
```

Native, web, and Agent observation stay on the same `BundlePresentationSnapshot` / `PlayerFramePlanner` path.

## Typed payload

`UiRuntimeControlStyle` has five deterministic slots:

- `normal`
- `hover`
- `pressed`
- `focus_visible`
- `disabled`

Each slot is a `UiRuntimeControlVisualStyle` carrying:

- optional fill color (`RgbaColor`), including alpha;
- optional text color;
- optional border color and width;
- optional focus-ring color, width, and offset;
- optional opacity in milli-units (`0..=1000`);
- optional radius in milli-pixels;
- a list of runtime shadows.

The renderer-facing mirror type is `RenderControlStyle`; conversion happens in `arcweft-player-scene`, so `arcweft-render-wgpu` does not depend on `arcweft-bundle`.

## State resolution

State precedence is:

```text
disabled > pressed > focus_visible > hover > normal
```

The state slot overlays the normal slot field-by-field. Missing state fields inherit normal values. Shadow lists replace the inherited list only when the state slot contains at least one shadow, matching CSS state override behavior for this focused bridge.

## Selector subset

For runtime controls, the bridge accepts:

- element selectors for `Button`, `TextField`, `TextArea`, `SecureField`;
- part/public-id selectors matching the control public id;
- explicit action-button style ids stored on `UiActionButtonResource.style`;
- state selectors: `:hover`, `:active`, `:disabled`, `:focus-visible`.

Unsupported selector combinators (`Descendant`, `Child`) and environment predicates are not guessed. They produce structured diagnostics when they would otherwise apply to a runtime control.

## Cascading

The bridge deliberately follows the compiled resource order instead of adding a CSS parser:

1. global `rules` are processed in stored order;
2. `part_rules` matching the target public id or explicit style id are processed after global rules;
3. state rules write into their specific state slot;
4. later declarations replace earlier declarations, except `box-shadow` with `Append`, which appends to the shadow list.

Inline Arcweft/CSS patches must be lowered into `UiStyleResource.rules` / `part_rules` by the compiler. Because this bridge consumes only the typed resource section, it cannot recover text from `UiStyleApplyRef::InlineCss { patch_id }` by itself. If a stored declaration cannot affect runtime controls yet, a diagnostic is emitted.

## Renderer behavior

The renderer uses normal prepared-frame primitives for visible supported properties:

- fill/background and opacity: `PaintRect` fill color;
- text color: `RenderTextBlock.rgba`;
- border: four deterministic `PaintRect` strips;
- focus ring: configurable four-strip ring around the bounds;
- box-shadow: converted into `UiBoxShadowList` and planned by `UiBoxShadowPassPlan` from the existing seq06.13e substrate.

Radius is carried and used as the shadow border-radius input. Rounded fill clipping of player-owned controls remains a non-goal of this focused bridge because current `PreparedFrame.rectangles` do not carry rounded-rect primitives. The broader retained UI path can render rounded fills later without changing the runtime style payload.

## Diagnostics

`UiRuntimeControlStyleDiagnostic` contains:

- target public id;
- property or selector fragment;
- reason (`unsupported_property`, `unsupported_value`, `token_not_found`, `unsupported_selector`).

Runtime session construction preserves these as display diagnostics so unsupported stored properties are not silent no-ops.

## Non-goals

- Restoring top-level `ui text_input`, `ui text_area`, or `ui secure_field` declarations.
- CSS parsing, external CSS loading, DOM overlays, canvas/image fallback, browser-native controls, or sample-specific geometry.
- Duplicating Takumi or seq06.13e box-shadow rendering.
- Full CSS specificity, cascade layers, inheritance, media queries, pseudo-element support, or rounded fill rasterization.
