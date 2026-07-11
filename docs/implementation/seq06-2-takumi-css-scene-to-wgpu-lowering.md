# seq06.2 Takumi CSS Scene to Direct wgpu Lowering

## Pin

The integration pins `kane50613/takumi` at commit
`38cb0ba6276981cb9bccc074acde7730f68dbde4`, package version
`takumi 2.0.0-beta.14`, with `default-features = false` and
`features = ["unstable", "woff2", "woff", "svg"]`.

The pin is intentionally a commit rather than a semver range because the direct
wgpu path uses Takumi's `unstable` backend-agnostic core surface:
`takumi::unstable::base::{context, layout, scene}`. The stable `takumi` facade is
kept for `Node`, `StyleSheet`, `Fonts`, `Viewport`, and resource input types; the
raster entry point is never used in the primary path.

## Modules used

- CSS parsing/cascade: `takumi::prelude::StyleSheet` backed by `takumi-css`.
- Node input: `takumi::prelude::Node` with Arcweft metadata encoded as
  `data-aw-*` attributes plus a typed `TakumiMetadataMap` sidecar.
- Layout: `takumi::unstable::base::layout::tree::{RenderNode, LayoutTree,
  LayoutResults}`.
- Render context: `takumi::unstable::base::context::RenderContext` and
  `takumi::unstable::base::layout::style::{ComputedStyle, SizingContext}`.
- Stacking scene: `takumi::unstable::base::scene::{build_stacking_contexts,
  StackingContextNode, PaintItemKind, NodePaint}`.

## Arcweft metadata mapping

`arcweft-takumi-adapter::TakumiAdapter` converts a `ViewFragment` root into a
Takumi `Node` tree. Every node receives deterministic attributes:

- `data-aw-node`, `data-aw-key`, `data-aw-kind`, `data-aw-style`;
- `data-aw-component`, `data-aw-program`, `data-aw-part` when supplied;
- `data-aw-semantic`, `data-aw-handlers`, `data-aw-agent` when present;
- `data-aw-takumi-path` for lookup into the sidecar.

The same information is preserved in typed form in `TakumiMetadataMap`; capture
records clone `ArcweftNodeMetadata` instead of reparsing string attributes. This
keeps Component, exported part, handler, semantic, and Agent ids attached without
requiring Takumi internals to grow Arcweft-specific fields.

## Text and TextField measurement bridge

Arcweft text and TextField participants are represented to Takumi by an object
replacement placeholder (`U+FFFC`) and an `ArcweftInlineParticipant` entry that
stores measured width, height, baseline, and canonical `PreparedTextId` data. The
final wgpu scene emits `ViewPrimitive::Text` values from
`ArcweftTextLayoutBridge`; Takumi is not asked
to perform final glyph layout or draw text into an RGBA surface. TextField caret,
selection, and composition geometry belongs to the referenced prepared item.

## Stacking lowering

`TakumiSceneLowerer` builds Takumi layout results and stacking contexts, then
walks Takumi's paint buckets in order:

1. root paint;
2. negative z-index bucket;
3. auto/zero/in-flow bucket;
4. positive z-index bucket;
5. nested stacking contexts recursively at their ordered point.

Every paint node that emits primitives becomes one `ViewSceneContext` with the
Takumi affine transform, opacity, clip, and the exact `ViewPrimitiveRange` that was
emitted. This supports per-node transforms even when multiple nodes live inside
one CSS stacking context, while preserving the ordered context list expected by
the direct wgpu renderer.

## First-cut direct CSS features

The implementation-ready feature set is intentionally limited to direct wgpu
primitives that already exist in the seq06 contract:

- solid and rounded rects;
- borders;
- image primitives keyed by renderer resource index;
- linear gradients;
- rectangular and rounded clips;
- opacity and affine transform;
- text layout placeholders that route Arcweft prepared items to
  `ViewPrimitive::Text`.

Box shadows are classified as paint-only, but the renderer support decision is
kept explicit: they must be emitted only when the wgpu renderer grows a matching
primitive. Filters, backdrop filters, masks, clip-path, and blend modes produce
unsupported-direct diagnostics and never trigger a CPU raster fallback.

## Invalidation and cache keys

`CssPropertyClass` maps properties to `CssInvalidationClass`:

- paint-only GPU updates: `opacity`, `transform`, color/background paint, border
  color, and box-shadow metadata;
- resource updates: image/background resource references;
- layout scene rebuilds: display, position, sizing, padding, margin, flex/grid,
  overflow, z-index, and font metrics;
- unsupported direct: filter/backdrop-filter/masks/clip-path/mix-blend-mode.

`TakumiSceneCacheKey` is keyed by program, fragment, style, text, image, and
viewport revisions. `TakumiPaintCacheKey` adds renderer resource revisions so
paint-only buffer/texture changes do not force a layout rebuild.

## Diagnostics

Unsupported-but-accepted CSS is reported through deterministic
`TakumiDiagnosticCode::UnsupportedDirectCss` diagnostics. A separate
`CpuRasterFallbackForbidden` diagnostic documents the invariant that this request
must not call Takumi's raster renderer or upload full View surfaces every frame.

## Capture and Agent metadata

`TakumiCaptureRecord` stores the same local bounds, affine transform, clip, and
`ViewPrimitiveRange` emitted to `ViewScene`. It also stores typed
`ArcweftNodeMetadata`, so Agent observation can use the exact rendered coordinate
space while still resolving component/part/semantic/handler identity through
Arcweft's normal systems.

## Integration boundary

This request deliberately does not implement platform IME adapters, product View
resource serialization, HTML/Servo DOM support, or a Takumi fork. If Takumi later
stabilizes layout/scene extraction, the adapter should move off the `unstable`
feature without changing the Arcweft-facing crate API.

## Applied validation

This checkout applied
`arcweft-seq06.2-takumi-css-scene-to-wgpu-lowering.zip` after seq06.1 and
validated the new adapter crate against the pinned Takumi commit.

Validation run:

- `cargo test -p arcweft-takumi-adapter --all-features -- --nocapture`
- `cargo fmt --all -- --check`
- `cargo check -p arcweft-takumi-adapter --all-targets --all-features`
- `cargo clippy -p arcweft-takumi-adapter --all-targets --all-features -- -D warnings`
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`
- `git diff --check`
