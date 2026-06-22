# UI Interaction Routing Package, 2026-06-22

This note records the repository state after accepting
`arcweft-ui-interaction-routing-3d7215bd-2026-06-22.zip`.

## Scope

The package connects retained UI component data to the shared presentation input
router so hover, focus, pressed, and disabled visual state resolve from the same
stable `InteractionTarget`.

Implemented boundaries:

- `arcweft-presentation` owns shared `InteractionState` data for focus, pointer
  capture, hover paths, and pressed targets.
- `arcweft-ui` resolves retained event bindings to `UiHandlerRouteTable` entries
  and resolves interaction styles in the order idle, hovered, focused, pressed,
  then disabled.
- `arcweft-runtime-host` carries display, semantic, handler, and style tables in
  committed UI frame layers, dispatches routed UI input, and resolves interaction
  styles before renderer lowering.
- `arcweft-render-wgpu` lowers resolved retained UI display rectangles to
  `UiPaintPlan` data without matching hover/focus/pressed in the backend.
- `arcweft-player-web` derives choice visual state from the shared
  `InteractionState`.
- `examples/ui-interaction-routing/` contains four runtime-supported dialogue
  samples and four component-surface parser/design samples.

## Non-Goals

The package does not implement typed parsing, HIR lowering, semantic checking,
bundle encoding, or runtime evaluation for arbitrary `component ... -> View`
bodies. The component-surface `.arcw` files are parser/design samples only.

IME composition is also out of scope for this cut. Existing text input remains
committed text.

## Verification

Commands run after integration:

```bash
cargo fmt --all --check
cargo test -p arcweft-presentation --test interaction_visual_state
cargo test -p arcweft-ui --test interaction_routing
cargo test -p arcweft-runtime-host --test ui_interaction_dispatch
cargo test -p arcweft-render-wgpu --test ui
cargo test -p arcweft-player-web --test interaction_visual_state
cargo test -p arcweft-lang-syntax --test ui_interaction_samples
cargo clippy -p arcweft-presentation -p arcweft-ui -p arcweft-runtime-host -p arcweft-render-wgpu -p arcweft-player-web --all-targets -- -D warnings
cargo +nightly -Zscript C:/Users/sanze/AppData/Local/Temp/arcweft-ui-interaction-routing-3d7215bd/arcweft-ui-interaction-routing-2026-06-22/tools/check_samples.rs --repo . --run
cargo run -p arcweft-render-wgpu --example ui_interaction_showcase -- --out target/ui-interaction-showcase
cargo test -p arcweft-presentation
cargo test -p arcweft-ui
cargo test -p arcweft-runtime-host
cargo test -p arcweft-render-wgpu
cargo test -p arcweft-player-web
cargo +nightly -Zscript tools/arcweft-structure-audit.rs --root .
cargo +nightly -Zscript tools/arcweft-structure-audit.rs --root . --write docs/implementation/structure-audit-ui-interaction-routing-2026-06-22
```

All commands above passed on this checkout. The structural audit reported 0
errors and 88 warnings. The warning set is recorded in
`docs/implementation/structure-audit-ui-interaction-routing-2026-06-22/violations.md`;
file metrics and dependency edges are recorded in the CSV files in the same
directory.

The package applier dry-run required overriding child git commands with
`core.autocrlf=false` on Windows so LF preimage guards matched the package text.
The actual source integration was applied from a clean temporary worktree as a
patch onto the existing dirty checkout to avoid disturbing unrelated work.
