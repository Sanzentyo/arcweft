# Request: Layout Fit Mode Coordinate Contract

## Background

`arcweft-layout-scaling-units-and-capture.zip` asks for raw and fit/scale modes
with a typed `ContentRect`, shared rendering/capture/hit-test behavior, and
letterbox/pillarbox reporting for non-16:9 outputs. The package did not include
a patch, and its requirement document lists the canonical design viewport,
`px` meaning, Agent observe default mode, and shared planner ownership as open
questions.

## Decision Needed

Please decide the coordinate contract for fit mode before implementation is
widened beyond the raw-mode metadata slice:

- Is the canonical design viewport fixed at 1280x720, or configured per
  project/profile/bundle?
- Does `px` mean design-space logical pixel, raw output pixel, or host CSS-like
  logical pixel?
- Should `arcw agent observe` default to raw mode or contain/fit mode?
- Should fit/contain/cover/stretch transforms be applied to observed object
  bboxes, polygons, hit regions, capture refs, layer bboxes, hit-test input
  points, and capture crop origins before serialization?
- Where should the shared planner live: `arcweft-render-wgpu`, a new Sans I/O
  layout crate, `arcweft-player-scene`, or another presentation-layer crate?
- How should negative origins from `cover` and letterbox/pillarbox regions from
  `contain` be represented in reports without losing unsigned viewport bbox
  compatibility?

## Current Implemented Slice

The current implementation adds typed primitives
`LayoutSize`/`LayoutPoint`/`LayoutRect`/`ScalePolicy`/`ContentRect` in
`arcweft-render-wgpu::geometry` and publishes raw-mode
`scene_graph[].kind = "layout.viewport_scale"` metadata from native Agent
observe. It intentionally does not transform observed geometry.

## Acceptance Criteria For The Answer

- Defines the default Agent observe scale policy.
- Defines the canonical design viewport source.
- Defines how all serialized geometry and capture metadata are transformed.
- Defines how hit-test coordinates are interpreted in raw and fit modes.
- Identifies the crate that owns the shared planner and why that dependency
  direction is acceptable.
