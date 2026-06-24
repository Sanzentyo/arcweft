# Layout Scaling, Units, and Capture - 2026-06-24

## Source package

`D:/sanze/Downloads/arcweft-layout-scaling-units-and-capture.zip` did not
contain a concrete patch or changed files. The package patch was empty and its
checkout note said the repository checkout was unavailable in the producer
environment, so this implementation is derived from the included requirement
document.

## Implemented in this cut

- Added typed viewport scaling primitives to `arcweft-render-wgpu::geometry`:
  `LayoutSize`, `LayoutPoint`, `LayoutRect`, `ScalePolicy`, and `ContentRect`.
- Added deterministic tests for raw, contain, cover, stretch, and the
  1280x720-to-1000x800 contain letterbox case.
- Made native Agent observation reports publish explicit raw viewport scaling
  metadata in `scene_graph[]` with `kind = "layout.viewport_scale"`,
  `renderer_kind = "native_rich_text_observer"`, output viewport, canonical
  1280x720 design viewport, `scale_policy = "none"`, content rect, and scale
  factors.
- Kept current Agent observe behavior in raw pixel mode. No implicit scaling is
  applied to observed object bounds or capture coordinates.
- Rejected `arcw agent observe --image overlay --out *.png` and other image
  output extension mismatches. Overlay output must use `.svg`, PNG output must
  use `.png`, and raw RGBA output must use `.rgba`.
- Updated Agent tooling docs to describe the raw viewport-scale metadata and
  output extension requirements.

## Deferred design items

The requirement document asks for more than a local implementation slice. These
items have product-level or architecture-level choices that should be answered
before code is widened:

- Whether Agent observe should default to raw mode or contain/fit mode.
- Whether the canonical design viewport is always 1280x720 or project/profile
  configurable.
- How fit/contain/cover/stretch transforms are applied across rendering,
  hit-testing, capture, diagnostics, and frame observation without splitting the
  coordinate contract.
- Whether the shared planner belongs in the current renderer crate or a new
  Sans I/O layout crate.
- The full layout unit system (`px`, `sp`, `%`, `vw/vh`, `cw/ch`, `em`, `ch`,
  safe-area units) and how it is parsed, typed, stored, and evaluated.
- Text fitting and overflow policy (`clip`, `page`, `fit_text`, `expand_box`)
  using the same shaping metrics for measurement and rendering.
- Shared WebGPU scene capture parity for images, dialogue, choices, and hit UI,
  distinct from the current native rich-text observer path.
- Zundamon/non-16:9 visual-golden routes once the fit-mode and shared-capture
  coordinate contracts are fixed.

Design requests were added under `docs/reviews/requests/` for these larger
items so they are not silently treated as completed by the raw-mode metadata
slice.

## Validation

- `cargo fmt`
- `cargo test -p arcweft-render-wgpu --test geometry`
- `cargo test -p arcweft-cli --features native-capture --lib agent_observe_`
- `cargo test -p arcweft-cli --features native-capture --no-run`

## Design deviations

The package requests fit/contain capture parity, but its own requirement file
lists the default mode, canonical design viewport, shared planner ownership, and
runtime observation scope as open questions. This cut therefore implements the
raw-mode typed foundation and report metadata without changing existing Agent
observe coordinates.
