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
`app/runtime.rs`, and JIT check implementation now lives in `app/jit.rs`.
The primary `app.rs` is smaller, but it still owns shared project loading plus
check/verify command logic. The remaining architectural cuts are:

1. Continue splitting `arcweft-cli/src/app.rs` by command implementation,
   prioritizing check/verify next so the primary module becomes dispatch plus
   shared CLI context instead of a cross-layer implementation sink.
2. Move non-profiled CLI compile paths toward `arcweft-compiler`, while keeping
   CLI-specific profiling and diagnostics in CLI modules.
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
  resolve dialogue frames from bytecode input.
