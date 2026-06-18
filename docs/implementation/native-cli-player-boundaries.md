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

`arcweft-runtime-host` now owns the first presentation-action dispatch boundary:
it accepts routed `arcweft-presentation` actions, classifies them by
`SemanticRole`, and produces an ordered dispatch plan for runtime, TextBox,
Activity, and UI-entity handlers. The same module can execute that plan against
host-owned handler implementations while collecting follow-up `ActionBatch` and
`HostEventBatch` output as pure data. A registration-backed
`PresentationActionHandlerRegistry` is available for concrete host adapters:
registered actions emit configured pure-data effects, while unregistered
actions fail with structured handler errors instead of being ignored. This keeps
semantic action partitioning and handler orchestration in the host layer
instead of pushing TextBox, Activity, or UI routing concepts down into
`arcweft-core`.

## UI / Activity / Input Direction

The unified UI design is adopted as the long-term boundary for future work:

- `arcweft-core` does not interpret raw pointer, key, hover, drag, focus, IME,
  window, GPU, or Activity framebuffer state.
- `arcweft-presentation` will own LayerTree, InputRouter, InteractionState,
  HitTree contracts, TextBox presentation state, and Activity presentation
  descriptors. The first pure-data boundary lives in
  `arcweft_presentation::input`: `RawInputEvent`, routed `InputEvent`,
  `InteractionTarget`, `Action` / `ActionBatch`, and `HostEvent` /
  `HostEventBatch`. The companion `arcweft_presentation::layer` module now
  provides the first shared `LayerTree` data model: `LayerNode`, `LayerOrder`,
  `LayerInputPolicy`, and `LayerContent` cover render order, future input
  routing order, and TextBox/Activity/UI content ownership without introducing
  public compatibility concepts such as `ActivityViewport` or `UiEvent`.
  `arcweft_presentation::hit`, `arcweft_presentation::interaction`, and
  `arcweft_presentation::router` now add the first routing boundary:
  `HitTree`, `HitRecord`, `InteractionState`, `FocusState`, pointer capture
  records, `InputRouter`, and auditable `RouteDecision` values. Pointer,
  keyboard, text, and Agent semantic input all route through the same
  LayerTree/HitTree checks, and modal layers block lower targets instead of
  allowing Agent or focus paths to bypass visibility and layer policy. The
  first hover boundary is also in `arcweft_presentation::hover`: hit records
  can carry stable root-to-leaf hover paths, and `HoverTransition` diffs those
  paths so unchanged common parents do not receive spurious leave/enter events.
  Replay verification has an initial pure-data boundary in
  `arcweft_presentation::replay`: `routing_hash` hashes LayerTree, HitTree, and
  InteractionState routing inputs with a deterministic hasher, while
  `route_fingerprint` combines that routing hash with the routed decision for
  raw/routed replay comparisons. Layer nodes also carry a deterministic
  fixed-point `LayerTransform`; pointer routing maps viewport coordinates into
  layer-local hit bounds before consulting `HitTree`, and non-invertible
  transforms are skipped for pointer hit-testing instead of panicking or
  creating a fallback router. `arcweft_presentation::gesture` now provides the
  first Sans I/O `GestureArena` boundary for tap, drag, horizontal scroll, and
  vertical scroll arbitration. It records stable pointer sessions and resolves
  winners from deterministic movement thresholds without adding per-Activity
  gesture routers. `arcweft_presentation::semantic` now provides the first
  shared `SemanticTree` boundary: `SemanticNode`, `SemanticRole`, and
  `SemanticActionError` normalize TextBox, Activity, and UI observation nodes,
  derive ordinary `HitTree` records from semantic bounds, and lower declared
  semantic actions to `ActionTarget` only after Agent semantic invocation has
  passed through `InputRouter` modal, visibility, and layer-policy checks. This
  keeps semantic actions from introducing `UiEvent`, `ActivityViewport`, or a
  separate Agent-only invoke path.
- `arcweft-ui` now owns the first Sans I/O UI state boundaries. Typed
  component descriptors live in `component`: `ComponentId`,
  `ComponentSchemaId`, `UiProgramId`, `RustComponentId`,
  `ComponentDescriptor`, and `ComponentRegistry` resolve public component names
  to dense load-time IDs without hot-path string lookup. Stateful UI component
  instances live in `entity`: `RawEntity`, `Entity<T>`, `DirtyFlags`, and
  `EntityStore` provide safe generational handles, reject stale reused slots,
  and track dirty state without `unsafe`, leaked state, or public compatibility
  aliases. `semantics` still owns `UiSemanticNode`, `UiSemanticFragment`, and
  `UiSemanticFragmentBuilder`, which produce ordered UI semantic nodes and
  lower them into `arcweft_presentation::semantic::SemanticTree` without
  introducing `UiEvent` or a separate UI router. `fragment` now owns the first
  retained flat fragment boundary: `ViewFragment`, `ViewFragmentBuilder`,
  `FragmentNode`, `FragmentKind`, `Span32`, and sidecar child/event vectors keep
  rich text, plain text, images, stateful components, and custom host elements
  in one deterministic node list. Fragment event bindings are handler IDs plus
  event kinds, not a public `UiEvent` compatibility family. `style` now owns
  the first property-binding invalidation boundary: `PropertyBinding`,
  `PropertyBindingTable`, `UiPropertyKind`, `ValueSourceId`, and `Invalidation`
  distinguish paint-only changes such as opacity, color, and transforms from
  layout, semantic, and structural fragment changes. Later cuts will extend
  this crate with reactivity, layout integration, and display-list generation.
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

The attached unified UI/activity/input design also adds constraints that affect
the order of future implementation cuts:

- Public vocabulary is intentionally small: `Activity`, `Layer`/`LayerTree`,
  `Component`, `TextBox`, `TextField`, `TextArea`, `RawInputEvent`,
  `InputEvent`, `Action`, and `HostEvent`. Names such as `DialogueWindow`,
  `TextBoxComponent`, `ActivityViewport`, `UiInputEvent`, and `UiEvent` must
  not become public compatibility concepts.
- `@textbox.main` is the canonical default TextBox ID. `@textbox.0` is not a
  runtime alias; it should be handled only by a one-shot migration path.
- Dialogue source keeps the `window = @textbox.main` option name because
  `window` is the dialogue-line output role, while `TextBox` is the target
  entity family.
- Rust dialogue APIs should converge on `window: Option<Ref<TextBox>>` rather
  than `text_box` fields or a dedicated `TextBoxRef` wrapper.
- Input work starts in `arcweft-presentation`, not a separate
  `arcweft-input` crate: LayerTree, HitTree, focus, modal, capture, hover,
  gesture, replay hash, TextBox presentation state, and Activity presentation
  descriptors are one Sans I/O presentation boundary.
- `arcweft-ui` starts as one crate for UI semantic node production and remains
  the future home for typed Component descriptors, retained flat fragments,
  generational Entity storage, reactivity, style/property bindings, and layout
  integration.
- Activity, TextBox, UI, Agent, and replay must share LayerTree routing and
  semantic observation. Agent actions must not bypass visibility, enabled
  state, modal policy, or routed interaction targets.
- Reveal/typewriter and paint-only effects must not invalidate base text
  layout; text layout cache keys include writing mode, font/style revisions,
  quantized size, locale, and scale, but not reveal cursor, hover, opacity, or
  paint-effect time.

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
now a thin `Cli::parse_from` / command-dispatch entrypoint; command-specific
types, implementation imports, and runtime/player details live in the
responsibility modules instead of being re-exported through the app root. The
native player bundle boundary is also covered by binary-level tests: default
input must be `.awfb`, and `.arcw` source input requires explicit `--source`
plus the `dev-source` feature. The remaining architectural cuts are:

1. Continue moving compile-driver behavior toward `arcweft-compiler`.
   The non-profiled CLI project-loading path now calls compiler-owned
   parse/lint/HIR/typecheck/line-task functions. Profiled runtime compilation
   also calls the same compiler-owned phase functions, while CLI modules keep
   developer-facing phase timing, source selection, and diagnostic printing.
2. Move remaining product-player host/task behavior onto `.awfb` execution.
   Source execution remains a developer mode, not the product-player model.
3. Continue extending `arcweft-ui` from its initial semantic, Component
   descriptor, generational Entity, retained flat fragment, and property
   invalidation boundaries toward reactivity, layout integration, and
   display-list generation, without adding public names such as
   `ActivityViewport`, `TextBoxComponent`, `UiEvent`, or per-Activity input
   routers.
4. Keep the unified TextBox model as the current source of truth: canonical
   `@textbox.main`, dialogue `window`, manifest `window`, and generic typed
   references. Runtime aliases are not used. Rust dialogue APIs already use
   `window: Option<Ref<TextBox>>` rather than `text_box` fields or a dedicated
   `TextBoxRef` wrapper.
5. Register concrete TextBox, Activity, UI, and runtime action handlers through
   `PresentationActionHandlerRegistry` / `PresentationActionHandlers`, and have
   future Component rendering feed `UiSemanticFragment` production. Later
   Component and Activity work must use `ActionBatch`, `HostEventBatch`, routed `InputEvent`,
   `LayerContent`, `LayerTransform`, `HoverPath`, `GestureArena`,
   `RoutingHash`, `RouteDecision`, `SemanticTree`,
   `PresentationActionDispatchPlan`, `PresentationActionHandlers`, and
   `PresentationActionHandlerRegistry` instead of introducing per-Activity
   routers, `ActivityViewport`, `UiEvent` aliases, or Agent-only semantic
   invoke shortcuts.

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
