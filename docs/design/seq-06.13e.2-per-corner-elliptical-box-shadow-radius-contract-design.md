# seq06.13e.2 Per-Corner / Elliptical Box-Shadow Radius Contract Design

Date: 2026-07-04

## Goal

Seq06.13e.2 extends the existing Arcweft-owned `box-shadow` renderer contract so
outer and inset shadows can preserve Takumi's four computed border-radius
corners, including elliptical `x/y` radii, from typed lowering through planning,
uniform packing, WGSL coverage, and smoke fixtures.

This design does not redesign seq06.13d outer shadow lowering or seq06.13e inset
shadow rendering. It replaces the scalar radius boundary only where that boundary
prevented the already-rendered outer/inset path from preserving authored corner
geometry.

## Public renderer data model

`arcweft-render-wgpu::view_scene` owns the public radius contract:

```rust
pub struct ViewBoxShadowCornerRadius {
    pub x_px: f32,
    pub y_px: f32,
}

pub struct ViewBoxShadowRadii {
    pub top_left: ViewBoxShadowCornerRadius,
    pub top_right: ViewBoxShadowCornerRadius,
    pub bottom_right: ViewBoxShadowCornerRadius,
    pub bottom_left: ViewBoxShadowCornerRadius,
}

pub struct ViewBoxShadow {
    pub offset_x_px: f32,
    pub offset_y_px: f32,
    pub blur_radius_px: f32,
    pub spread_radius_px: f32,
    pub border_radii: ViewBoxShadowRadii,
    pub color: ViewColorRgba8,
    pub kind: ViewBoxShadowKind,
}
```

The existing scalar constructors remain as convenience constructors only:

```rust
ViewBoxShadow::outer(..., border_radius_px, color)
ViewBoxShadow::inset(..., border_radius_px, color)
```

They map to `ViewBoxShadowRadii::uniform(border_radius_px)`. New explicit
constructors are added for authored typed radii:

```rust
ViewBoxShadow::outer_with_radii(..., border_radii, color)
ViewBoxShadow::inset_with_radii(..., border_radii, color)
```

Outer and inset shadows share the same radius contract. The shadow kind affects
paint order, spread direction, and shader coverage, not the radius data model.
No broad root-level compatibility re-export is added; the new types are exported
through the existing `view_scene` responsibility module boundary.

## Takumi lowering

Takumi already exposes computed corner radii as `SpacePair<Length>` fields on
`ComputedStyle`:

- `border_top_left_radius`
- `border_top_right_radius`
- `border_bottom_right_radius`
- `border_bottom_left_radius`

The adapter lowers those four typed fields directly into `ViewBoxShadowRadii` via
`length_value_px`, preserving `x/y` elliptical values and CSS corner order. It no
longer computes `max(min(rx, ry))` as a scalar for shadows. No production CSS
string scanning is introduced; parse and cascade remain Takumi-owned.

## Canonicalization and diagnostics

The contract uses two stages:

1. **Typed preservation at the scene boundary.** Non-finite and negative values
   are preserved so non-empty shadows can report structured planner diagnostics.
   Transparent shadows and true zero-effect shadows still canonicalize to no-op
   entries through `ViewBoxShadowList::new`.
2. **Planning-time geometric normalization after validation.** Once a shadow is
   valid, each pass computes body and caster/shadow radii for its target rect.

Rules:

| Input form | Behavior |
| --- | --- |
| Zero radius | Supported; produces square corners. |
| Mixed corners | Preserved per corner. |
| Elliptical corners | Preserved as independent `x_px` / `y_px`. |
| Negative direct radius | `ViewBoxShadowPlanError::DegenerateRadius`; CSS should not produce this, but direct renderer callers are not silently clamped. |
| Non-finite radius | `ViewBoxShadowPlanError::NonFiniteRadius`. |
| Oversized radii | CSS border-radius overlap normalization: one shared scale factor is computed from top/bottom horizontal sums and left/right vertical sums and applied to all corner axes. |
| Spread for outer shadows | Adds `spread_radius_px` to each corner axis, floors at zero, then normalizes against the spread caster rect. |
| Spread for inset shadows | Subtracts `spread_radius_px` from each corner axis, floors at zero, then normalizes against the inset caster/clear rect. Negative inset spread therefore expands the caster and increases radii deterministically. |
| Blur | Does not mutate radii. The shader samples the per-corner/elliptical caster coverage through the existing deterministic 9-tap blur approximation. |

## Planner contract

`ViewBoxShadowPass` stores both normalized radius sets:

```rust
pub struct ViewBoxShadowPass {
    pub shadow_index: usize,
    pub shadow: ViewBoxShadow,
    pub body_rect: HitRect,
    pub shadow_rect: HitRect,
    pub body_radii: ViewBoxShadowRadii,
    pub shadow_radii: ViewBoxShadowRadii,
}
```

Existing scalar-radius tests remain valid through the scalar constructors and by
asserting `ViewBoxShadowRadii::uniform(...)` in plan output. The public scalar
fields are not kept as compatibility shims on `ViewBoxShadowPass`; code that needs
radii reads the typed contract.

## Uniform and WGSL contract

No new bind group layout, texture binding, or compositor pass enum is added.
The existing uniform layout has unused slots for `PASS_BOX_SHADOW` and packs
radius rows as follows:

| Uniform field | Meaning for box-shadow |
| --- | --- |
| `matrix[0]` | body rect: `x, y, width, height` |
| `matrix[1]` | shadow/caster rect: `x, y, width, height` |
| `matrix[2]` | body top-left `(x,y)`, top-right `(x,y)` |
| `matrix[3]` | body bottom-right `(x,y)`, bottom-left `(x,y)` |
| `clip_vertices[0]` | shadow top-left `(x,y)`, top-right `(x,y)` |
| `clip_vertices[1]` | shadow bottom-right `(x,y)`, bottom-left `(x,y)` |
| `params0.x` | blur radius |
| `params0.w` | kind flag: `0 = outer`, `1 = inset` |
| `offset` | RGBA shadow color |

WGSL coverage chooses the active corner by testing the fragment position against
corner bands. If the point is in a rounded corner band, it tests an ellipse:

```text
((x - cx) / rx)^2 + ((y - cy) / ry)^2 <= 1
```

Circular corners are the special case `rx == ry`. If either axis is zero, the
corner is square. Oversized radii are already normalized by the planner.

Outer and inset coverage continue to use the seq06.13e rule:

```text
outer = caster * (1 - body)
inset = body * (1 - caster)
```

## Visual evidence policy

This package adds analytic planner/lowering tests and updates the ignored GPU
smoke path so it covers:

- at least one mixed-corner outer shadow;
- at least one elliptical inset shadow.

Exact PNG promotion is intentionally deferred. A package may not claim exact
native/web visual parity until the pinned visual-golden environment runs the
promotion suite.

## Non-goals

- No DOM, browser CSS renderer, SVG filter, canvas, bitmap, or CPU raster fallback.
- No redesign of text, clip, mask, blend, resource tables, or unrelated compositor contracts.
- No broad compatibility re-export or duplicate scalar pass contract.
- No exact PNG baseline promotion in this package.
