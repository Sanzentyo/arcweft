# Component text-control layout-bounds resource contract

## Context

`seq06.16` and `seq06.16.1` made component/View-authored `TextField`,
`TextArea`, and `SecureField` controls produce typed input, text, semantic
and action-button resources without top-level `ui text_input` declarations.
The remaining gap was that runtime text-control bounds were still supplied by
`UiRuntimeTextControlBounds::default_stacked_slots`, so known component layout
was discarded at the resource/runtime boundary.

## Decision Summary

Bounds live in a dedicated `UiProgramResource::layout_bounds` table.

They do not live in `UiInputResource`, because input resources describe text
control identity and input semantics, not View layout. They also do not live
inside `UiSemanticTarget`, because semantic targets need to remain an identity
and accessibility/action map. A dedicated bounds table gives resource codecs,
runtime snapshot conversion, player scene lowering, hit testing, focus rings,
selection/caret placement, and action buttons a typed geometry contract without
smuggling geometry through sample-specific defaults.

## Resource Model

```rust
pub struct UiProgramResource {
    // existing fields ...
    pub semantic_targets: Vec<UiSemanticTarget>,
    pub layout_bounds: Vec<UiLayoutBoundsResource>,
    pub action_buttons: Vec<UiActionButtonResource>,
    // existing fields ...
}

pub struct UiLayoutBoundsResource {
    pub public_id: String,
    pub kind: UiLayoutBoundsKind,
    pub rect: UiLogicalRect,
    pub hit_rect: Option<UiLogicalRect>,
    pub source: Option<SourceRangeRef>,
}

pub enum UiLayoutBoundsKind {
    TextControl,
    SemanticTarget,
}

pub struct UiLogicalRect {
    pub x_milli: i32,
    pub y_milli: i32,
    pub width_milli: u32,
    pub height_milli: u32,
}
```

`UiLogicalRect` uses logical pixels multiplied by 1000, in the root component's
logical coordinate space. Device-pixel conversion remains the responsibility of
presentation/render adapters. `arcweft-player-scene` continues to convert
runtime milli units into `HitRect`/renderer frame units, so the low-level
resource/data crate stays Sans I/O.

## Runtime Shape

`UiRuntimeTextControlBounds` remains the public runtime-facing shape. The new
contract feeds that shape instead of replacing it:

```rust
program.text_control_bounds_for(input_id)
    .unwrap_or(default_stacked_fallback_for_input)
```

This keeps the player-rendered text-control path stable while replacing the
stacked fallback whenever component layout bounds are available.

## Component/View Lowering

No parser or AST syntax is added in this cut. Bounds are derived from existing
component/View structure:

- `Column`: children are placed vertically.
- `Row`: children are placed horizontally.
- `Stack` / `Panel`: children share an origin and the container extent is the
  maximum child extent.
- `Fragment`: uses the same vertical flow rule as a column.
- `TextField` and `SecureField`: default intrinsic size `420px x 48px`.
- `TextArea`: default intrinsic size `420px x 136px`.
- `Button`: default intrinsic size `180px x 44px` for layout flow purposes.
- Default root origin is `(48px, 48px)`, and default flow gap is `16px`.

These are deterministic default logical layout outputs, not platform widgets,
DOM/CSS screenshots, or source-string fallbacks.

For each component-authored text control, lowering emits two layout records with
identical rects:

- `kind = TextControl` for runtime text-control rendering, hit testing, caret
  and selection geometry.
- `kind = SemanticTarget` for semantic target bounds and focus-ring hit bounds.

The semantic and text-control bounds agree by construction in this cut.
Future richer layout may widen `hit_rect` without changing the visual rect.

## Defaults And Fallback

Missing layout bounds are not an error. Runtime conversion keeps the documented
fallback:

- `TextField` and `SecureField`: stacked slot height `48px`.
- `TextArea`: stacked slot height `136px`.
- stack origin `(48px, 48px)` and width `420px`.
- stack gap `16px`.

The fallback remains necessary for older sidecars, manually authored compact UI
resources, and non-component sources that do not yet provide layout metadata.

## Validation

Resource codec validation rejects:

- duplicate `(kind, public_id)` layout records;
- zero-width or zero-height visual rects;
- zero-width or zero-height hit rects.

Non-finite values cannot occur in this contract because the serialized model is
integer milli-logical-pixel data. Negative width/height cannot occur because
sizes are `u32`. Negative `x`/`y` are allowed for off-root or animated logical
placement. Overlap is allowed and remains a layout/style policy issue rather
than a resource canonicality error.

## Interaction With Styling And Rendering

Root style and component style do not directly mutate this table in this cut.
The table represents resolved deterministic fallback layout from component
structure. A future style-to-runtime-control rendering cut can replace or refine
these records using a real style/layout resolver.

Focus rings, hit testing, selection handles and caret geometry use the semantic
or text-control layout records. `hit_rect` is optional; if absent, `rect` is the
hit rect. Action-button `text_submit` placement continues to use the target
text-control runtime bounds, now preferring authored component bounds before the
legacy stacked fallback.

## Non-Goals

- No compatibility declarations for top-level `ui text_input`, `ui text_area`,
  or `ui secure_field`.
- No platform-widget, DOM, CSS screenshot, or source-string fallback behavior.
- No redesign of the seq06.16/seq06.16.1 submit substrate.
- No full CSS or style-driven layout resolver in this cut.
- No device-pixel conversion in low-level resource/data crates.
