# Native CLI, Renderer, Player, UI, and Activity Boundaries

This note records the implementation direction adopted from
`docs/reviews/pro_review33.md` plus
`arcweft-unified-ui-activity-input-design.md`, supplied on 2026-06-19.

## Current Boundary

`arcweft-compiler` is the shared Sans I/O source compiler driver:

```text
source text
  -> parse
  -> HIR lowering
  -> type check
  -> runtime-plan lowering
  -> line display catalog
```

`arcweft-render-native` owns native rendering and capture:

```text
wgpu / winit / glyphon
offscreen capture
window surface
native text layout submission
renderer effect / shader / motion registries
pixel/object geometry
```

`arcweft-player-native` owns player orchestration:

```text
compiled program execution
headless frame collection
native product entrypoint
render-native orchestration
```

`arcweft-cli` remains the developer command surface. It may call compiler,
runtime-host, verifier, bundle, tooling, and native renderer crates, but it must
not become the native product player and must not depend on
`arcweft-player-native` for Agent observe/capture.

## UI / Activity / Input Direction

The unified UI design is adopted as the long-term boundary for future work:

- `arcweft-core` does not interpret raw pointer, key, hover, drag, focus, IME,
  window, GPU, or Activity framebuffer state.
- `arcweft-presentation` will own LayerTree, InputRouter, InteractionState,
  HitTree contracts, TextBox presentation state, and Activity presentation
  descriptors.
- A future `arcweft-ui` crate will own typed Component descriptors, retained
  fragments, generational Entity storage, reactivity, style, layout integration,
  and semantic UI nodes.
- `TextBox` is a dialogue domain object, not a Component. It may use an
  anonymous or named Component as its view implementation.
- Activity, TextBox, UI, Agent, and replay input must all route through the same
  LayerTree / HitTree / InteractionTarget model.
- `UiEvent` is not a public runtime-step concept; UI handlers lower to routed
  input, `ActionTarget`, or semantic action data owned by presentation/runtime
  host boundaries.
- Rich text display data is owned once by the line display store/catalog;
  TextBox and Component views borrow projections instead of cloning full rich
  text frames for reveal-only updates.

## Required Next Cuts

The current cuts remove the direct CLI dependency on `arcweft-player-native`,
move renderer tests into `arcweft-render-native`, extract the Clap command
surface into `app/commands.rs`, move bundle/run-bundle implementation into
`app/bundle.rs`, and isolate tooling commands (`fmt`, `ids materialize`) in
`app/tooling.rs`. Agent observe / hit-test / MCP implementation now lives in
`app/agent.rs`, keeping native observation logic out of the primary dispatch
module while it is being moved toward the unified LayerTree / InputRouter /
semantic observation model. The CLI native renderer dependency is gated behind
the `native-capture` feature, so default `arcweft-cli` builds can run ordinary
check / format / verify / bundle tooling without linking the native GPU/window
stack. Runtime plan/run/profile/serve/test/bench implementation now lives in
`app/runtime.rs`, JIT check implementation now lives in `app/jit.rs`, and
check/verify/verify-types/unsafe implementation now lives in `app/verify.rs`.
Shared source/profile selection, adapter manifest resolution, typecheck
environment construction, and checked-module loading now live in
`app/project.rs`. Bundle/run-bundle option types and bundle-only helper
conversion now live in `app/bundle.rs`. Runtime command option types, runtime
value parsers, and runtime step/executor CLI conversion helpers now live in
`app/runtime.rs`. Tooling command options and source path collection now live in
`app/tooling.rs`, while verification CLI parsers now live in `app/verify.rs`.
Launch profile CLI options now live with project selection in `app/project.rs`,
and small shared helpers now live in `app/shared.rs`. The primary `app.rs` is
now mostly dispatch and import wiring. The remaining architectural cuts are:

1. Continue splitting `arcweft-cli/src/app.rs` by command implementation,
   prioritizing import-surface cleanup and any remaining dispatch-only
   simplification without reintroducing cross-layer command logic.
2. Continue moving compile-driver behavior toward `arcweft-compiler`.
   The non-profiled CLI project-loading path now calls compiler-owned
   parse/lint/HIR/typecheck/line-task functions. Profiled runtime compilation
   also calls the same compiler-owned phase functions, while CLI modules keep
   developer-facing phase timing, source selection, and diagnostic printing.
3. Move remaining product-player host/task behavior onto `.awfb` execution.
   Source execution remains a developer mode, not the product-player model.
4. Add the presentation input and future `arcweft-ui` crates according to the
   unified UI design, without adding public names such as `ActivityViewport`,
   `TextBoxComponent`, `UiEvent`, or per-Activity input routers.

## Invariants

- No compatibility aliases or transitional parser/compiler branches are added.
- `arcweft-core` stays Sans I/O.
- The CLI can orchestrate high-level commands, but source compilation,
  rendering, runtime hosting, verification, bundles, and tooling remain in
  responsibility crates.
- Native renderer object geometry, Agent capture geometry, hit testing, and
  debug resources must share the same deterministic presentation model.
- Native player `.awfb` execution must not invoke the source compiler. Bundles
  carry line display sidecars so native window presentation and capture can
  resolve dialogue frames from bytecode input. The `arcweft-player-native`
  binary treats `.awfb` as its default input; `.arcw` source execution requires
  the explicit `--source` developer-mode flag and the `dev-source` feature.
