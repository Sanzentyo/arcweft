# Pro Review 33 Completion Audit

Status: implemented for the explicit `pro_review33` boundary goal.

This audit records the implementation evidence for
`docs/reviews/pro_review33.md` and the additional boundary constraints adopted
from `arcweft-unified-ui-activity-input-design.md`, supplied on 2026-06-19.
The unified UI design is broader than the pro-review goal, so this document
separates completed boundary work from long-term UI, Activity, and input work.

## Completion Scope

The completed scope is the `pro_review33` implementation goal:

1. Keep the native product player separate from the developer CLI.
2. Split the large CLI application module by responsibility.
3. Remove the unconditional CLI dependency on the native player and native
   GPU/window stack.
4. Move shared source-to-runtime-plan driver behavior behind
   `arcweft-compiler`.
5. Split native renderer/capture responsibilities from product player host
   orchestration.
6. Make the native product player default to `.awfb` bundle execution, with
   source execution and capture metadata behind developer features.
7. Preserve the no-shim policy: do not introduce compatibility aliases,
   removed parser branches, `ActivityViewport`, `TextBoxComponent`, `UiEvent`,
   or per-Activity input routers.
8. Record tests and validation at reviewable cut points.

The unified UI and Activity input design is not treated as fully implemented by
this audit. It is adopted as the next boundary source of truth for future cuts:
LayerTree routing, semantic observation, retained UI fragments, TextBox as a
domain object, Activity step borrowing, Agent routing, replay, gesture, and
performance counters must continue to move toward that design.

## Implementation Evidence

### CLI and player separation

`arcweft-cli` no longer depends on `arcweft-player-native`.
`crates/arcweft-cli/Cargo.toml` keeps native capture optional:

```toml
default = []
native-capture = ["dep:arcweft-render-native"]
```

`arcweft-player-native` remains a separate product-player crate and binary.
It depends on `arcweft-render-native`, but the CLI does not depend on the
player crate for Agent observe or capture.

The CLI application module has been split into responsibility modules:

```text
crates/arcweft-cli/src/app.rs
crates/arcweft-cli/src/app/agent.rs
crates/arcweft-cli/src/app/agent/native.rs
crates/arcweft-cli/src/app/bundle.rs
crates/arcweft-cli/src/app/commands.rs
crates/arcweft-cli/src/app/jit.rs
crates/arcweft-cli/src/app/project.rs
crates/arcweft-cli/src/app/runtime.rs
crates/arcweft-cli/src/app/shared.rs
crates/arcweft-cli/src/app/tooling.rs
crates/arcweft-cli/src/app/verify.rs
```

`app.rs` is now the top-level argument parsing and command dispatch surface;
command implementation details live in the responsibility modules.

### Compiler driver boundary

Shared parse, HIR lowering, typecheck, runtime-plan lowering, line-task
lowering, and text pure-helper candidate lowering are exposed through
`arcweft-compiler`.

Direct `arcweft-runtime-plan` imports are confined to `arcweft-compiler` and
`arcweft-runtime-plan` itself, with runtime-plan regression coverage living in
the runtime-plan crate.

Renderer-facing helper export no longer imports HIR or syntax directly in
`arcweft-render-native`; categorized shader, effect, and motion candidates
cross the boundary through compiler-owned data.

### Native renderer and player split

`arcweft-render-native` owns native rendering and capture concerns:

```text
wgpu / winit / glyphon
offscreen capture
window surface
native text layout submission
renderer effect / shader / motion registries
pixel/object geometry
```

`arcweft-player-native` owns product-player orchestration:

```text
runtime-host bundle execution
headless frame collection
native product entrypoint
render-native orchestration
```

The player `.awfb` execution path calls `arcweft-runtime-host` bundle execution
and resolves display frames from runtime-host flow events. Source execution is
behind the `dev-source` feature, and capture metadata is behind `dev-capture`.
Default product-player JSON reports do not expose debug capture metadata.

### Runtime-host boundaries

`arcweft-runtime-host` now owns these host-layer join points:

- `PresentationActionDispatchPlan`
- `PresentationActionHandlerRegistry`
- `ActivityHostRegistry`
- `ActivityStepInputRef`
- `ActivityStepOutputSink`
- `UiFrameCommitBuilder`

Presentation actions are partitioned and dispatched by host-owned handlers.
Activity hosts receive borrowed, already routed `InputEvent` data and matching
`HostEventSource::Activity` notifications. UI component output reaches the host
as `UiLayerOutput`, which pairs renderer-facing display data with semantic
fragment data before validation against the committed `LayerTree`.

### Unified UI design constraints adopted

The current implementation and docs adopt the unified design constraints that
matter for this boundary:

- `arcweft-core` does not interpret raw input, hover, drag, focus, IME, window,
  GPU, or Activity framebuffer state.
- Input work belongs first in `arcweft-presentation`, not in a separate
  `arcweft-input` crate.
- `Activity`, `TextBox`, UI, Agent, and replay route through the same
  `LayerTree`, `HitTree`, and `InteractionTarget` model.
- `TextBox` is a dialogue domain object, not a Component.
- `@textbox.main` is the canonical default TextBox ID.
- Dialogue source keeps `window = @textbox.main`; Rust dialogue options use
  `window: Option<Ref<TextBox>>`.
- Runtime aliases such as `@textbox.0` are not used as legacy fallbacks.
- Public concepts such as `ActivityViewport`, `TextBoxComponent`,
  `UiInputEvent`, and `UiEvent` are not introduced.

Initial Sans I/O boundaries already exist in:

```text
crates/arcweft-presentation/src/input.rs
crates/arcweft-presentation/src/layer.rs
crates/arcweft-presentation/src/hit.rs
crates/arcweft-presentation/src/interaction.rs
crates/arcweft-presentation/src/router.rs
crates/arcweft-presentation/src/hover.rs
crates/arcweft-presentation/src/gesture.rs
crates/arcweft-presentation/src/replay.rs
crates/arcweft-presentation/src/semantic.rs
crates/arcweft-ui/src/component.rs
crates/arcweft-ui/src/entity.rs
crates/arcweft-ui/src/fragment.rs
crates/arcweft-ui/src/style.rs
crates/arcweft-ui/src/reactive.rs
crates/arcweft-ui/src/layout.rs
crates/arcweft-ui/src/display.rs
crates/arcweft-ui/src/frame.rs
```

## Deferred Unified UI Work

The following work remains intentionally outside the completed
`pro_review33` goal and must be handled as later implementation goals:

- Full LayerTree router integration with every native input source.
- Complete focus, modal, pointer capture, hover, gesture, drag/drop, scroll,
  pointer lock, keyboard, IME, gamepad, Agent, and replay routing behavior.
- Full `TextBoxState`, `LineDisplayStore`, reveal-index, semantic TextBox
  handler, bbox/mask derivation, and anonymous TextBox Component integration.
- Full Arcweft Component lowering to `UiProgram`, retained fragment diffing,
  renderer submission integration, hot reload, and versioned Rust ABI.
- Activity layer content integration, semantic/hit metadata for trusted
  renderer Activity output, snapshot/restore, cancellation on unmount/reload,
  and deterministic fixed-step assignment.
- Performance counters and allocation instrumentation listed in the unified
  design.

These are not compatibility gaps. They are forward implementation work under
the boundary now documented in
`docs/implementation/native-cli-player-boundaries.md`.

## Validation Performed

The pro-review boundary cuts were validated at reviewable cut points with:

```bash
cargo fmt --all -- --check
cargo test -p arcweft-compiler --quiet
cargo test -p arcweft-render-native --quiet
cargo test -p arcweft-cli --features native-capture --lib --quiet
cargo test -p arcweft-runtime-host --quiet
cargo clippy -p arcweft-compiler -p arcweft-render-native --all-targets -- -D warnings
cargo clippy -p arcweft-cli --features native-capture --lib -- -D warnings
cargo clippy -p arcweft-runtime-host --all-targets -- -D warnings
just scan-removed-dsl
just scan-absolute-paths
just test-workspace
```

This documentation-only audit should be rechecked with the documentation gates
before pushing.

## Completion Judgement

`pro_review33` is reflected in implementation and documentation. The remaining
items are long-term unified UI, Activity, and input implementation work, not
unresolved `pro_review33` blockers.
