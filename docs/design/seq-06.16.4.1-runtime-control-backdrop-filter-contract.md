# seq06.16.4.1 runtime-control backdrop-filter contract

## Decision

Runtime controls keep the player-owned overlay renderer. They do not lower into
retained `ViewScene` nodes in this slice.

The runtime-control path owns text editing, selection, caret, platform IME
geometry, shortcut policy, tab policy, focus state, action-button hit-testing,
and text-input submit activation. `backdrop-filter` therefore becomes a typed
runtime-control visual effect payload plus a dedicated renderer/effect-plan
record. A later retained-UI unification may consume the same typed effect list,
but this request does not require a second editing host or DOM/browser CSS
overlay.

## Data path

```text
UiStyleResource
  -> UiRuntimeControlStyle / UiRuntimeControlVisualStyle
  -> UiRuntimeTextControl / UiRuntimeActionButton
  -> RuntimeTextControlLowerer / RuntimeActionButtonLowerer
  -> RenderControlStyle / RenderControlVisualStyle
  -> SharedFramePlanner runtime-control effect plans
  -> native/web renderer executes effect plans in control paint order
```

`arcweft-bundle` remains Sans I/O and data-only. It parses already-decoded typed
style resource values and emits structured diagnostics. It does not create GPU
resources, read files, capture screenshots, or invoke platform APIs.

## Typed style payload

`UiRuntimeControlVisualStyle` has two optional filter-list slots:

```rust
pub filters: Option<UiRuntimeControlFilterList>,
pub backdrop_filters: Option<UiRuntimeControlFilterList>,
```

`None` means the state did not declare the property. `Some(empty)` means the
state declared `none` and clears the inherited value. This is intentionally
different from the existing `shadows: Vec<_>` bridge, where an empty vector
cannot distinguish unspecified from authored `none`.

```rust
pub struct UiRuntimeControlFilterList {
    pub filters: Vec<UiRuntimeControlFilter>,
}

pub enum UiRuntimeControlFilter {
    Blur { radius_milli: u32 },
}
```

The renderer mirror uses logical pixels:

```rust
pub struct RenderControlFilterList {
    pub filters: Vec<RenderControlFilter>,
}

pub enum RenderControlFilter {
    Blur { radius_px: f32 },
}
```

## Accepted properties and values

Accepted property names:

- `backdrop-filter`
- `-webkit-backdrop-filter`
- `filter`

Accepted values:

- `none`
- one or more whitespace-separated `blur(<length>)` functions

Accepted length syntax:

- `0`
- non-negative `<number>px`, for example `blur(12px)` and `blur(0.5px)`

Storage units:

- bundle/runtime resource payload: milli-pixels
- renderer payload: logical pixels

Unsupported values produce
`UiRuntimeControlStyleDiagnosticReason::UnsupportedValue`. Unsupported property
names continue to produce `UnsupportedProperty`.

## State resolution

Runtime-control state precedence stays unchanged:

```text
disabled > pressed > focus_visible > hover > normal
```

The state slot overlays the normal slot field-by-field. `filters` and
`backdrop_filters` replace the inherited list whenever the state slot is
`Some(...)`, including `Some(empty)` from `none`.

## Renderer ordering model

For each runtime-control item sorted by existing depth/source order:

```text
1. control backdrop-filter plan, sampling framebuffer content already painted
2. runtime-control shadow plan
3. control fill/background
4. border
5. focus ring if focused
6. text selection and caret for focused text controls
7. control text
8. optional foreground filter plan for the completed control content
9. semantic and hit-test output, unchanged
```

`PreparedControlBackdrop` records target, bounds, typed `ViewFilterList`, and a
sampling policy. `PreparedControlFilter` records target, bounds, and typed
`ViewFilterList` for foreground control filtering.

The exact GPU backend must execute these records inline with runtime-control
painting. Merely drawing the whole list before or after all rectangles would
violate this contract.

## Backdrop sampling policy

`RuntimeControlBackdropSamplePolicy::PriorFrameContentAndEarlierRuntimeControls`
is the seq06.16.4.1 policy.

A runtime control samples background and retained/runtime content already
painted into the same frame target. It does not sample its own fill, selection,
caret, or text, and it does not sample higher-depth runtime controls.

## Native/web evidence requirements

Before promoting PNG baselines:

1. Native and web must consume the same typed path.
2. Exact captures must be generated only in the approved pinned exact
   visual-golden environment.
3. Evidence must include a translucent runtime control over a deterministic
   image/background and at least one lower-depth runtime control that can be
   blurred by a higher-depth control.
4. The evidence manifest must record capture command, renderer backend, device
   pixel ratio, source commit/change id, and whether `control_backdrops` and
   `control_filters` were non-empty.
5. No checked-in PNG baseline should be updated from an unpinned local
   environment.

## Non-goals

- DOM/CSS overlays, browser-native controls, screenshots, or baked sample
  images.
- Making `arcweft-bundle` or data-format crates depend on GPU, platform,
  filesystem, or browser APIs.
- Full CSS filter support beyond `blur(...)`.
- A retained-UI editing host or replacement of current text-input /
  action-button semantics.

