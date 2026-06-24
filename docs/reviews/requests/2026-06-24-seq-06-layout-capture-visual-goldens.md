# Request 06: Layout, Capture, and Visual Goldens

## Sequence Position

This is an independent design request in the integrated execution sequence.

It can be submitted in parallel with Requests 01-05 because it concerns
presentation/layout/capture contracts rather than executable runtime and patch
semantics.

## Request

Please design the remaining layout, coordinate, text-fitting, capture, and
visual golden infrastructure as one coherent presentation contract. This
request intentionally combines:

- layout fit-mode coordinate contract;
- typed layout units;
- text fitting and overflow behavior;
- shared capture metadata;
- selected object/layer capture behavior;
- visual golden policy.

The design must be concrete enough to turn into small Rust implementation cuts
with focused tests.

## Existing Request Files To Incorporate

Use these existing requests as source material, but answer them together rather
than independently:

- `docs/reviews/requests/2026-06-24-layout-fit-mode-coordinate-contract.md`
- `docs/reviews/requests/2026-06-24-layout-units-text-fitting-and-shared-capture.md`

Also incorporate the implemented contracts recorded in:

- `docs/implementation/integrated-execution-2026-06-24.md`
- `crates/arcweft-layout/src/lib.rs`
- native Agent observe/capture code that now uses shared layout geometry.

## Why These Must Be Designed Together

Layout coordinates, unit resolution, text fitting, hit testing, capture
metadata, and visual goldens describe one presentation-space contract:

- fit transforms decide design-space vs output-space coordinates;
- layout units depend on viewport/content/safe-area/text metrics;
- text fitting affects render geometry and Agent observation;
- selected layer/object captures must report the same coordinate basis;
- visual golden tolerances depend on renderer/font/GPU policy.

Designing these separately risks mismatched coordinate systems or
renderer-specific capture behavior.

## Current Implementation Evidence

The repository currently has:

- `arcweft-layout` with shared geometry, content fit policies, inverse mapping,
  layout unit expression contracts, safe-area context, text overflow/fitting
  contracts, and diagnostics;
- render/observe paths that use shared layout geometry;
- existing design requests for unresolved fit-mode coordinate ownership,
  shared WebGPU capture, and visual fixture parity;
- no final end-to-end HIR/sema/runtime-plan/UI/renderer/Agent-observe layout
  unit resolution policy.

## Required Design Decisions

Please provide concrete answers for:

1. Which coordinate spaces are public: design, content, output, physical,
   logical, object-local, layer-local?
2. Which crate owns fit transforms, inverse mapping, and hit-test conversion?
3. How are `raw`, `contain`, `cover`, and `stretch` represented in Agent
   observation and capture metadata?
4. How are layout units resolved across HIR, sema, runtime-plan, UI layout,
   renderer, and Agent observe?
5. Which units are supported in v1: `px`, `sp`, `%`, `vw`, `vh`, `cw`, `ch`,
   safe-area units, font-relative units, and content-relative units?
6. What text fitting and overflow policies are supported?
7. How does text fitting report diagnostics, truncation, scaling, pagination,
   or failure?
8. How do selected object/layer captures report crop bounds, masks, object ids,
   and coordinate bases?
9. What native/WebGPU capture behavior is shared, and what remains
   adapter-specific?
10. What visual golden policy is acceptable for fonts, GPU backend, tolerances,
    platform differences, and CI?

## Required Implementation Order In The Design

Please propose small compiling cuts in this order or justify a better order:

1. Freeze coordinate-space naming and transform ownership.
2. Wire fit-transform metadata through Agent observe/capture outputs.
3. Add layout unit resolution policy to HIR/sema/runtime-plan boundaries.
4. Add Sans I/O text fitting result and diagnostics integration.
5. Wire renderer/UI layout to shared layout contracts.
6. Add selected object/layer capture metadata parity.
7. Add deterministic metadata tests.
8. Add visual smoke/golden policy and fixtures.

## Tests To Specify

The design should include focused tests for:

- contain/cover/stretch/raw transform values;
- inverse hit-test mapping;
- non-16:9 viewport bar/crop values;
- unit resolution with viewport/content/safe-area/font inputs;
- text overflow and fitting result diagnostics;
- Agent observe coordinate metadata;
- selected layer/object crop metadata;
- visual smoke/golden commands with explicit tolerance policy.

## Constraints

- Keep `arcweft-layout` Sans I/O.
- Do not move renderer, GPU, filesystem, or capture I/O into layout.
- Do not let Agent clients infer coordinate spaces from stringly labels.
- Do not require exact visual goldens where platform fonts/GPU backends make
  them unstable; define metadata/golden tiers explicitly.

## Expected Output

Please produce one design document with:

- coordinate-space contract;
- layout-unit resolution model;
- text fitting and overflow model;
- capture metadata schema;
- renderer/Agent observe integration plan;
- visual golden policy;
- implementation cuts;
- test plan;
- explicit non-goals.

