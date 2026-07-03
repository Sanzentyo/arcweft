# seq06.13b Box Shadow and Blend Render Closure

## Goal

Close the seq06.13 visual gap for common card/button/popover styling by adding
deterministic `box-shadow` rendering to the Arcweft-owned `UiScene` /
`UiCompositor` path, while preserving the existing HSL-family blend support for
`hue`, `saturation`, `color`, and `luminosity`.

## Rendering Route

`box-shadow` is a compositor group effect. It is not a direct primitive, and it
is not lowered to `filter: drop-shadow(...)`.

The renderer route is:

1. `ui_scene::compositing` owns `UiBoxShadow`, `UiBoxShadowKind`, and
   `UiBoxShadowList`.
2. `ui_box_shadow` owns pure pass planning and typed diagnostics.
3. `UiCompositor` draws box-shadow passes into the group target before child
   paint nodes.
4. Existing foreground filters, clip-path, mask, backdrop-filter, blend, and
   opacity passes then operate on the group target.

This keeps native and web on the same wgpu shader path and avoids browser DOM,
CSS, canvas, or CPU bitmap fallback.

## Support Matrix

| Feature | Decision |
| --- | --- |
| Outer shadow | Implemented from border box, spread, radius, offset, and color. |
| Inset shadow | Deferred with `UiBoxShadowPlanError::InsetUnsupported`. |
| Spread radius | Implemented, including negative spread and collapsed no-op casters. |
| Multiple shadows | Implemented in CSS paint order: later list entries first, first entry on top. |
| Rounded corners | Implemented through body and shadow radii uniforms. |
| Transparent colors | Canonicalized to identity. |
| Non-finite numeric inputs | Rejected by typed diagnostic. |
| `filter: drop-shadow(...)` | Preserved as separate subtree-alpha filter behavior. |
| HSL blend family | Existing shader support is preserved and tested. |

## Paint Order

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

## Visual Evidence

Native/web smoke captures should include a rounded card shadow, multiple shadow
ordering, negative spread, and shadow with opacity/blend. Exact browser Gaussian
parity is not claimed in this cut; the validation target is deterministic
native/web wgpu parity.
