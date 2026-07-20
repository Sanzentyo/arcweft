# Seq06.14 responsive stage placement design

## Scope

This design defines one deterministic placement contract for presentation image objects and character-stage objects. It is deliberately renderer-independent and belongs in `arcweft-layout`, because the contract crosses bundle encoding, runtime/player frame planning, shared wgpu rendering, native/web output, Agent observe metadata, selected capture metadata, and visual golden fixtures.

The immediate bug motivating the design is `samples/zundamon-stand-switch/src/main.arcw`: the standing image is authored as 1280x720-era absolute pixels, so 1920x1080 and 2560x1440 output keeps the character at the old location while the rest of the frame expands. The new contract keeps old absolute fields as explicit absolute mode, but makes responsive anchor placement the intended authoring path.

## Coordinate spaces

Arcweft already has layout coordinate spaces; seq06.14 uses them without adding a renderer dependency.

| Space | Meaning | Owner |
|---|---|---|
| source/canvas | image-local or character-manifest canvas pixels; `.awchar` composition remains outside this sequence except for mount compatibility | image/character manifests |
| design viewport | authored logical viewport, default `1280x720` for current samples | `arcweft-layout::stage_placement` |
| resolved viewport/output | output logical pixels after design viewport fit transform | player-prepared frame |
| physical output | output pixels after device-pixel-ratio/scale-factor multiplication | capture/render adapters |

For responsive placement, authors specify values in design viewport space. The player resolves them into output logical pixels before the frame reaches backend rendering. Physical output is reported for capture/observe evidence; it is not an authoring space.

## Viewport fit transform

The default viewport policy is `contain`, using the existing `ContentRect` fit-transform machinery. `contain` preserves aspect ratio from the design viewport to output viewport and records bars/crop metadata. `cover` and `raw` remain representable through the existing layout model, but `stretch` is diagnosed for responsive anchored objects because it introduces independent x/y scaling and visually deforms the authored relation.

High-DPI does not alter logical placement. It only multiplies resolved output rectangles into physical rectangles. A 1920x1080 logical viewport at scale factor 2.0 resolves the same logical bbox as scale factor 1.0, and reports a physical bbox doubled on both axes.

## Authored placement primitives

### Absolute mode

Existing fields continue to mean explicit output-logical absolute pixels:

```arcw
x = 930px
y = 20px
width = 250px
height = 430px
```

This mode intentionally does **not** scale from 1280x720 to larger outputs. It preserves old behavior and makes old non-responsive content unambiguous.

### Responsive anchor mode

Responsive placement is authored with `position = anchor(...)` and explicit responsive size fields:

```arcw
position = anchor(top_right)
object_anchor = top_right
margin.top = 20px
margin.right = 100px
size.width = 250px
size.height = 430px
scale = design
fit = "contain"
alignment.x = "center"
alignment.y = "bottom"
```

`anchor` selects a point in the design viewport or safe-area-adjusted design viewport. `object_anchor` selects the corresponding point on the object box. Margins inset the available rectangle before the anchor point is chosen. For `top_right`, `margin.right = 100px` means the object right edge remains 100 design pixels from the design viewport right edge. At 1920x1080 this becomes 150 output pixels; at 2560x1440 it becomes 200 output pixels.

### Character baseline / ground anchor

Character-stage objects can mount the same contract by using `baseline = ground` with a bottom object anchor. Seq06.14 does not solve PSD or `.awchar` composition, but the contract carries enough data to align a manifest anchor/canvas baseline later:

```arcw
position = anchor(bottom_right)
object_anchor = bottom_right
baseline = ground
margin.right = 100px
margin.bottom = 40px
size.width = 250px
size.height = 430px
```

For flat image objects, `baseline = ground` behaves as a bottom-edge baseline and is metadata for observe/capture until character manifests provide a source/canvas anchor.

## Fit policy versus placement policy

Placement resolves the **outer object box**. `fit` and `alignment` still resolve the image source inside that box. This preserves current `contain`, `cover`, `stretch`, and `intrinsic` semantics already used by renderers.

`stretch` as an image fit is still allowed because it is local to the image inside the resolved box. `stretch` as a viewport scale policy is rejected for responsive anchor placement unless a future design explicitly opts into non-uniform responsive layout.

## Diagnostics

Structured diagnostics use stable codes and severities:

| Code | Severity | Trigger |
|---|---|---|
| `mixed_absolute_and_anchor` | error | `position = anchor(...)` is combined with `x`, `y`, `width`, or `height` |
| `missing_size` | error | anchored placement lacks `size.width` or `size.height` |
| `conflicting_fit_and_scale` | error | unsupported responsive scale/fit combination such as non-uniform viewport stretch |
| `independent_axis_scale_rejected` | error | author tries `scale.x` / `scale.y` for stage placement |
| `object_exceeds_viewport` | warning | resolved object bbox exceeds output viewport |
| `object_exceeds_safe_area` | warning | safe-area-aware placement exceeds safe-area rectangle |
| `non_finite_geometry` | error | computed geometry is not finite |
| `empty_viewport` | error | design or output viewport is non-positive |

Diagnostics are produced before renderer work and are deterministic across native, web, and offscreen observe.

## Observe report contract

Agent observe reports both authored and resolved placement:

```json
{
  "content": {
    "kind": "image",
    "source": "image.zundamon.stand",
    "authored_placement": { "mode": "anchor", "anchor": { "kind": "top_right" } },
    "resolved_placement": {
      "design_bbox": { "origin": { "x": 930.0, "y": 20.0 }, "size": { "width": 250.0, "height": 430.0 } },
      "output_bbox": { "origin": { "x": 1395.0, "y": 30.0 }, "size": { "width": 375.0, "height": 645.0 } },
      "physical_bbox": { "origin": { "x": 1395.0, "y": 30.0 }, "size": { "width": 375.0, "height": 645.0 } },
      "fit_transform": { "policy": "contain", "scale_x": 1.5, "scale_y": 1.5 }
    }
  }
}
```

The ordinary object `bbox` continues to be the renderer-visible output bbox so existing hit-test/capture users can still consume the report. Capture metadata uses the same output bbox as the crop basis, not a second independently computed crop.

## Authoring syntax decisions

1. Extend `image @... { ... }`; do not introduce a second image declaration family.
2. Use `position = anchor(name)` for responsive placement.
3. Use `object_anchor = name` so author intent is explicit.
4. Use `size.width`/`size.height` instead of overloading old `width`/`height` inside anchored mode.
5. Existing `x/y/width/height` remains only absolute mode.
6. Do not add compatibility layers for ambiguous syntax; mixed syntax is a typed diagnostic.

## Non-goal boundary

This sequence does not redesign CSS layout, animation/keyframes, or `.awchar` PSD composition. It only ensures that `.awchar` stage objects can use the same placement contract when the character composition path mounts a renderable canvas.
