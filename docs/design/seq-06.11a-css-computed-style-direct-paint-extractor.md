# seq06.11a design: CSS computed style direct paint extractor

## Goal

Make the CSS paint path explicit without adding a renderer side path:

```text
CSS source
  -> Takumi cascade/layout/computed style
  -> arcweft-takumi-adapter computed direct paint extractor
  -> DirectPaintCatalog
  -> TakumiSceneLowerer
  -> arcweft-render-wgpu::view_scene::ViewScene direct primitives / ViewPaintNode graph
```

The extractor belongs in `arcweft-takumi-adapter` because it consumes Takumi
computed style and Takumi render-node paths. It does not belong in
`arcweft-render-wgpu`; the renderer should consume Arcweft-owned primitive data,
not Takumi types.

## Ownership decisions

### Owning module

Add `crates/arcweft-takumi-adapter/src/paint_extractor.rs`.

The module owns:

- Takumi computed style to `DirectPaintCatalog` extraction;
- resource-reference indexing for image backgrounds;
- direct-paint extraction diagnostics;
- direct-paint evidence records that can be joined with `TakumiCaptureFrame`.

It does not own:

- product UI resource codecs;
- retained UI program/style resolution;
- native/web player frame attachment;
- renderer pass allocation;
- filesystem or network image loading.

### Existing lowerer contract

`TakumiSceneLowerer` remains the only path that turns `DirectPaintCatalog` into
`ViewScene` primitives. Seq06.11a extends `DirectBoxPaint` from a single optional
background to ordered `backgrounds: Vec<DirectBackground>` so CSS layered
backgrounds can remain deterministic and renderer-independent.

No private renderer path is introduced. No rectangle-only bridge is introduced.

## Extractor input

```rust
pub struct ComputedDirectPaintInput<'a> {
    pub root: &'a RenderNode,
    pub metadata: &'a TakumiMetadataMap,
    pub resources: &'a DirectPaintResourceTable,
}
```

The root is the Takumi `RenderNode` after cascade/computed-style construction.
The extractor walks this tree with `TakumiPath::root().child(index)` exactly as
the existing metadata adapter and lowerer do.

Each node provides:

- `TakumiPath` for deterministic catalog addressing;
- `ComputedStyle` from Takumi;
- `SizingContext` for resolving lengths;
- `current_color` for `currentColor` resolution;
- optional `ArcweftNodeMetadata` through `TakumiMetadataMap`.

## Extractor output

```rust
pub struct ComputedDirectPaintFrame {
    pub catalog: DirectPaintCatalog,
    pub diagnostics: Vec<TakumiDiagnostic>,
    pub evidence: DirectPaintEvidenceFrame,
    pub resource_requirements: Vec<DirectPaintResourceRequirement>,
}
```

`catalog` is the only paint input to `TakumiSceneLowerer`. `diagnostics` are
structured and must be surfaced by callers. `resource_requirements` are typed
adapter/player requirements and never trigger file or network I/O inside
`arcweft-takumi-adapter`. `evidence` records explain what was extracted and why.

## Supported first-cut subset

### Background color

- `background-color` is resolved through Takumi `ColorInput::resolve`.
- Transparent colors do not create a paint layer.
- Non-transparent colors create a `DirectBackground::Solid` layer.
- If a supported uniform radius exists, the solid layer carries that radius so
  the lowerer emits a rounded rect.

### Border radius

- The first cut supports a uniform circular radius across all four corners.
- Uniform means every corner has equal horizontal and vertical radii after
  Takumi computation.
- A supported uniform radius also creates `DirectClip::RoundedRect` metadata.
- Elliptical or per-corner-mixed radii produce structured diagnostics instead of
  approximation.

### Borders

- The first cut supports visible uniform solid borders.
- Physical sides are read from Takumi computed longhands; logical shorthands are
  already expanded by Takumi before extraction.
- If visible side widths, colors, or styles differ, extraction emits an
  unsupported diagnostic. Supported layers already extracted for the node are
  preserved.

### Linear gradients

- `background-image: linear-gradient(...)` is supported when every paint stop can
  be converted to a normalized `ViewGradientStop`.
- CSS keyword directions are converted to their equivalent angle through Takumi.
- Stops with explicit percentage positions are supported.
- Missing stop positions are distributed deterministically across the extracted
  color stops.
- Pixel stop positions, color hints without colors, radial gradients, conic
  gradients, and repeating gradients are diagnosed as unsupported in this cut.

### Image backgrounds

- `url(...)` backgrounds become `DirectBackground::Image` only when the URL exists
  in `DirectPaintResourceTable`.
- Unknown URLs produce `DirectPaintResourceRequirement` entries; no file/network
  access occurs in the adapter crate.

### Opacity and transforms

- Opacity is copied into `DirectBoxPaint::opacity`.
- Transforms remain owned by the existing Takumi render node and are attached to
  `ViewSceneContext` by `TakumiSceneLowerer`. The extractor does not duplicate
  transform logic.

## Layer order

The `DirectBoxPaint.backgrounds` vector is stored in painter order:

1. `background-color` behind everything;
2. supported `background-image` layers from bottom to top, reversing the CSS
   source order because the first CSS background layer is topmost;
3. border after backgrounds.

Unsupported background layers emit diagnostics while preserving supported layers
in their deterministic painter order.

## Diagnostics policy

Unsupported CSS values are never silently dropped into browser CSS, DOM, canvas
2D, Takumi CPU raster output, screenshots, or image snapshot fallbacks.

The extractor emits `TakumiDiagnosticCode::UnsupportedDirectCss` with:

- Takumi path when known;
- property/value family in the message;
- preservation policy, when applicable.

## Seq06.11b handoff

Seq06.11b consumes the final data as normal frame data:

```rust
TakumiComputedPaintFrame {
    scene: ViewScene,
    capture: TakumiCaptureFrame,
    direct_paint: ComputedDirectPaintFrame,
}
```

The only visual output contract is `ViewScene` / `ViewPaintNode` / `ViewPrimitive`.
The `direct_paint` field is evidence/diagnostics/resource metadata for the
player and Agent capture layers; it is not a renderer path.
