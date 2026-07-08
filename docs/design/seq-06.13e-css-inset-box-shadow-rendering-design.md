# seq06.13e CSS Inset Box-Shadow Rendering Design

## Goal

Render `ViewBoxShadowKind::Inset` values in the Arcweft-owned direct wgpu View
compositor path. The input is already typed by Takumi and the Takumi adapter;
this package does not add CSS parsing, source-string scanning, DOM rendering,
canvas/SVG filters, CPU rasterization, or bitmap snapshot fallbacks.

## Pass model and ordering

`box-shadow` remains a compositor group effect owned by
`arcweft-render-wgpu`.

The group render order becomes:

```text
clear group target
outer box-shadow passes
child direct/group paint nodes
inset box-shadow passes
foreground filters
clip-path geometry pass
masks
backdrop-filter copy/filter
blend/opacity composite into parent
```

Outer shadows preserve seq06.13b behavior: they are painted into the group target
before child paint nodes and are therefore behind the group's drawn contents.

Inset shadows are painted after child paint nodes and before the filter/clip/mask
chain. This is deliberate for the retained compositor graph: Arcweft currently
models the element subtree as group children rather than as separate
background/content phases, so drawing inset before children would often let a
background direct node cover the inset shadow completely. The chosen v1 behavior
makes inset shadows a group-local overlay while still keeping them inside the
same isolation, opacity, blend, filter, clip, and mask chain as the group.

## Interaction with clip, masks, opacity, blend, and isolation

Inset shadows are first analytically clipped to the group's rounded box body in
the `PASS_BOX_SHADOW` shader. Any `clip-path` then clips the already-composited
child+inset group target. Masks run after clip-path and apply to both children
and inset shadows. Group opacity and blend are applied once when compositing the
finished group target into its parent. `ViewIsolation::Isolate` keeps its existing
meaning because box shadows already require a group offscreen surface.

## Plan API decision

`ViewBoxShadowPassPlan` grows a unified ordered pass list instead of introducing a
separate `ViewInsetBoxShadowPassPlan`.

Rationale:

- The existing plan is already the renderer-owned boundary for one CSS
  `box-shadow` list.
- CSS list order is a property of the whole list, not of separate outer/inset
  inputs.
- A unified plan keeps deterministic diagnostics and metadata in one place while
  the compositor filters the plan by `ViewBoxShadowKind` at the two paint stages.

The plan adds `visual_inset_px()` metadata next to the existing
`visual_outset_px()`. Outset remains external visual expansion and affects group
extent. Inset reach is metadata/test evidence only; it does not enlarge the
offscreen target.

## Geometry

For a group `bounds` and an inset shadow:

- `body_rect` is the group border/body rect and is the shader's receiver clip.
- `shadow_rect` is the inner clear/caster rect:
  `outset_rect(bounds, -spread_radius_px)` shifted by the shadow offset.
- Positive spread deflates the clear/caster rect, increasing the visible inner
  ring.
- Negative spread expands the clear/caster rect, reducing the visible ring and
  remaining deterministic.
- `body_radius_px` is the existing scalar border radius clamped to `body_rect`.
- `shadow_radius_px` is `(border_radius_px - spread_radius_px).max(0)` clamped
  to `shadow_rect`.
- Zero-offset, zero-blur, zero-spread inset shadows are canonical identity paint
  and are removed by `ViewBoxShadowList::new`.
- Transparent inset shadows are canonical identity paint.
- Non-empty inset shadows on zero-area receiver bounds produce a typed
  `ViewBoxShadowPlanError::DegenerateGeometry` diagnostic rather than being
  silently dropped.

Outer shadow geometry is unchanged.

## Shader contract

The existing `PASS_BOX_SHADOW` is extended with a kind flag rather than adding a
new `PASS_INSET_BOX_SHADOW`.

Uniform contract:

```text
params0.x = blur radius px
params0.y = body radius px
params0.z = shadow/caster radius px
params0.w = kind: 0.0 outer, 1.0 inset
matrix[0] = body_rect:   x, y, width, height
matrix[1] = shadow_rect: x, y, width, height
```

This keeps the bind group layout, render pipelines, pipeline keys, texture
bindings, and shader pass enum stable. The WGSL computes the same blurred rounded
rect caster coverage for both kinds:

- outer coverage: `caster * (1.0 - body)`;
- inset coverage: `body * (1.0 - caster)`.

## Diagnostics

`ViewBoxShadowPlanError::InsetUnsupported` is removed because inset rendering is
implemented in this package. `NonFinite` remains typed. A new
`DegenerateGeometry` error covers non-empty inset shadows that cannot draw
because the receiver bounds have no drawable area.

Non-empty inset shadows are not silently dropped unless they canonicalize to
identity paint: transparent color or zero offset/zero blur/zero spread.

## Visual evidence policy

The implementation includes focused analytic plan tests and an ignored GPU smoke
test that exercises one rounded inset shadow and one mixed outer+inset card.
Exact PNG promotion remains manual and is not claimed by this package, because
it requires the pinned native/web visual-golden environment.
