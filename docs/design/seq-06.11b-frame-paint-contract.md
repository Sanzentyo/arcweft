# Final frame/paint contract for seq06.11b

Seq06.11b must integrate the result into the normal native/web player path. This
contract is designed so seq06.11b does not need a private renderer path or a
rectangle-only compatibility bridge.

## Producer pipeline

```text
UiProgramResource / UiStyleResource resolved by seq06.11
    -> ViewFragment + authored CSS/Arcweft style layers
    -> TakumiAdapter::adapt
    -> Takumi computed render tree
    -> ComputedDirectPaintExtractor::extract
    -> DirectPaintCatalog
    -> TakumiSceneLowerer::lower
    -> UiScene + TakumiCaptureFrame
```

Seq06.11a owns only the computed-style-to-paint extraction and the explicit
metadata needed by the next step.

## Frame object consumed by seq06.11b

The intended normal-frame payload is:

```rust
pub struct TakumiComputedPaintFrame {
    pub scene: UiScene,
    pub capture: TakumiCaptureFrame,
    pub direct_paint: ComputedDirectPaintFrame,
}
```

### `scene`

The only visual rendering input. It contains:

- `UiScene::primitives()` with Arcweft-owned direct primitives;
- `UiScene::contexts()` with transform, opacity, clip, and primitive ranges;
- `UiScene::paint_nodes()` with direct nodes and compositing groups.

Seq06.11b should attach this to `PreparedFrame` or the equivalent normal player
frame type. It should not inspect CSS or Takumi computed style at render time.

### `capture`

The authoritative capture/evidence mapping produced by the existing lowerer. It
contains local/layout/visual/hit bounds, primitive ranges, clips, compositing ids,
paint node ids, and Arcweft node metadata.

Seq06.11b can join capture records to `direct_paint.evidence` by Arcweft node
metadata and by the stable Takumi path recorded in the direct-paint evidence.

### `direct_paint`

Extraction diagnostics and non-visual metadata:

- `catalog`: the exact `DirectPaintCatalog` used to produce the scene;
- `diagnostics`: structured unsupported CSS/property/value diagnostics;
- `resource_requirements`: image resources that must be fulfilled by
  adapter/player resource providers;
- `evidence`: path, metadata, and extracted layer order.

`direct_paint` is not a renderer input once `scene` exists. It is kept for
validation, Agent capture, diagnostics, and resource provisioning.

## Direct paint model

```rust
pub struct DirectBoxPaint {
    pub backgrounds: Vec<DirectBackground>,
    pub border: Option<DirectBorder>,
    pub clip: Option<DirectClip>,
    pub opacity: f32,
}
```

`backgrounds` is stored in painter order. This is the stable handoff that avoids
both a CSS renderer and a temporary rectangle bridge.

## Resource contract

Image backgrounds use a deterministic resource table:

```rust
pub struct DirectPaintResourceTable { ... }
```

A URL in the table produces a stable `resource_index` in
`DirectBackground::Image`. A URL outside the table produces
`DirectPaintResourceRequirement` and a diagnostic. The adapter crate does not
read files, fetch URLs, decode images, or allocate GPU textures.

Seq06.11b/native/web resource providers are responsible for resolving the same
indices to renderer resources in the normal player resource path.

## Diagnostics contract

Diagnostics are surfaced before rendering. The scene may still contain supported
layers from the same element when the policy is “preserve supported layers.”
Unsupported features never trigger browser CSS, DOM overlays, canvas 2D,
Takumi raster output, or screenshots.

## Required seq06.11b behavior

Seq06.11b should:

1. attach `UiScene` to normal prepared-frame data;
2. render the attached `UiScene` through the shared `UiDirectPrimitiveRenderer`
   and `UiCompositor` path;
3. source images/masks through native/web resource providers by stable resource
   index;
4. surface `direct_paint.diagnostics` through the existing player/runtime
   diagnostic channel;
5. verify web output contains no DOM/HTML/CSS overlay for Arcweft UI content.

Seq06.11b should not:

- parse CSS again;
- inspect Takumi computed style at render time;
- build a private UI renderer;
- temporarily convert the catalog to rectangles outside `UiScene`;
- route any Arcweft UI visual through DOM, CSS, canvas 2D, or CPU-raster output.
