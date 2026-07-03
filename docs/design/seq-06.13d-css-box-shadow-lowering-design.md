# seq06.13d CSS Box-Shadow Lowering Design

## Goal

Lower authored CSS `box-shadow` from Takumi's typed computed style into the
Arcweft retained `UiScene` compositor graph. This closes the gap left after
seq06.13b: the renderer can draw `UiBoxShadow` passes, but the Takumi adapter was
still emitting `UiBoxShadowList::default()` for every computed style.

This design does not redesign seq06.13b's renderer substrate. The current
substrate already owns:

- `UiBoxShadow`, `UiBoxShadowKind`, and `UiBoxShadowList` in
  `arcweft-render-wgpu::ui_scene`;
- `UiBoxShadowPassPlan` and `UiBoxShadowPlanError` in `ui_box_shadow`;
- compositor pass order where shadows are drawn into the group target before
  children and before filter/clip/mask/blend;
- WGSL `PASS_BOX_SHADOW`.

## Source model decision

Takumi at Arcweft's pinned revision already exposes typed computed box-shadow:

```rust
pub box_shadow: Option<BoxShadows>,
```

where `BoxShadows = Box<[BoxShadow]>`, and each `BoxShadow` stores:

```rust
pub inset: bool,
pub offset_x: Length,
pub offset_y: Length,
pub blur_radius: Length,
pub spread_radius: Length,
pub color: ColorInput,
```

Therefore seq06.13d does **not** add an ad hoc CSS parser and does **not** add a
new Takumi-facing computed-style extension. The adapter lowers that typed model
only after Takumi has parsed, cascaded, resolved variables, and made computed
values.

## Lowering contract

`TakumiCompositingStyle::from_computed_style` now calls a private adapter-owned
lowering step:

```rust
box_shadows: box_shadow_list_from_takumi(
    style.box_shadow.as_deref(),
    style,
    sizing,
    current_color,
),
```

Each Takumi `BoxShadow` lowers to one Arcweft `UiBoxShadow` with:

- `offset_x_px`: resolved CSS/device pixels, preserving sign;
- `offset_y_px`: resolved CSS/device pixels, preserving sign;
- `blur_radius_px`: resolved pixels clamped at zero;
- `spread_radius_px`: resolved pixels, preserving negative spread;
- `border_radius_px`: deterministic scalar radius derived from the computed
  border radii;
- `color`: `ColorInput::resolve(current_color)` converted to `UiColorRgba8`;
- `kind`: `Outer` unless Takumi's typed value has `inset: true`.

The lowered list is built through `UiBoxShadowList::new`, so transparent and
identity shadows are canonicalized by the owning Arcweft type instead of by local
adapter branches.

## Border radius scalar decision

`UiBoxShadow` currently carries a single scalar `border_radius_px`, while CSS and
Takumi expose four possibly elliptical corners. Seq06.13d keeps the existing
renderer contract and chooses a deterministic scalar rather than redesigning
seq06.13b.

For each corner, the adapter resolves the horizontal and vertical radius lengths
and takes `min(horizontal, vertical)`. It then takes the maximum of the four
corner values. This scalar is conservative for the shadow caster: rounded boxes
stay visibly rounded, mixed radii remain deterministic, and no rendering path is
silently dropped. Exact per-corner shadow parity is a future renderer-contract
extension, not part of this lowering package.

## Diagnostics and unsupported forms

Unsupported forms must not be silently dropped:

| Form | Behavior |
| --- | --- |
| Malformed shadow list | Takumi CSS parse error before adapter lowering. |
| Unsupported color function / unresolved color | Takumi parse or cascade diagnostic before adapter lowering. |
| Transparent shadow | Lowered, then canonicalized to identity by `UiBoxShadowList::new`. |
| `inset` shadow | Lowered to `UiBoxShadowKind::Inset`; seq06.13b planner emits `UiBoxShadowPlanError::InsetUnsupported`. |
| Non-finite numeric field | Lowered as typed data; seq06.13b planner emits `UiBoxShadowPlanError::NonFinite`. |
| Mixed / elliptical border radii | Deterministic scalar selection as described above. |

This package does not add source-string gates such as `value.contains("inset")`.
The no-fallback rule is enforced through typed Takumi values and renderer plan
errors.

## Drop-shadow separation

`filter: drop-shadow(...)` remains a subtree-alpha filter and lowers to
`UiFilter::DropShadow`. It must not populate `UiCompositingEffects::box_shadows`.
The focused test suite includes this separation explicitly.

## Pass order

Seq06.13d preserves seq06.13b pass order:

```text
clear group target
box-shadow passes
child direct/group paint nodes
foreground filters
clip-path
masks
backdrop-filter copy/filter
blend/opacity composite into parent
```

No WGSL changes are included. If a later test finds a renderer bug, that belongs
in a renderer-focused follow-up rather than this CSS lowering package.

## CSS coverage decision

Outer `box-shadow` graduates to direct-wgpu ready status only because the package
adds both typed lowering and compositor-plan tests. Inset remains a typed
diagnostic path and does not claim visual support.
