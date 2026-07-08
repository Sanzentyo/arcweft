# Request: Layout Units, Text Fitting, and Shared Capture Parity

## Background

`arcweft-layout-scaling-units-and-capture.zip` also requests a complete layout
unit system, measured text fitting/overflow policy, shared WebGPU scene capture,
and Zundamon/non-16:9 visual regressions. These items require decisions beyond
the raw viewport metadata and typed `ContentRect` foundation implemented in the
current cut.

## Decision Needed

Please specify the design for:

- The typed layout unit model for `px`, `sp`/text, `%`, `vw`, `vh`, `cw`, `ch`,
  `em`, glyph `ch`, and safe-area units.
- Where unit parsing belongs and what typed representation crosses parser,
  HIR/sema, runtime-plan, View layout, renderer, and Agent observation
  boundaries.
- Whether text fitting is a renderer-only behavior, a runtime observation
  behavior, or a shared Sans I/O layout result.
- Required overflow policies and their serialization:
  `clip`, `page`, `fit_text`, `expand_box`, diagnostics, or another set.
- How typewriter reveal interacts with shaped/wrapped text and pagination.
- Which path is the canonical capture path for full player frames: the shared
  WebGPU scene, the native rich-text observer, or both with distinct renderer
  labels.
- What exact Zundamon and non-16:9 fixtures should assert at 1280x720,
  960x540, 640x360, and 1000x800.

## Current Implemented Slice

The current implementation keeps `arcw agent observe` in raw pixel mode,
records raw viewport scale metadata, and rejects image output extension/mime
mismatches. It does not implement the full unit evaluator, measured text
fitting, pagination, shared WebGPU capture, or visual golden suite.

## Acceptance Criteria For The Answer

- Defines the unit AST/data model and owner crate.
- Defines conversion/evaluation phases and dependency direction.
- Defines text overflow/fitting semantics and report diagnostics.
- Defines the canonical capture renderer path and renderer labels.
- Lists fixture inputs and expected observable outcomes for the Zundamon and
  non-16:9 routes.
