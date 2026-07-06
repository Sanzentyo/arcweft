# Scoped Presentation Handles And Final UI Syntax - 2026-07-06

This cut implements the first production slice of scoped presentation resource
handles and final component/View authoring syntax.

## Implemented

- Added runtime `FlowOp::RegisterCleanup` and `FlowOp::CancelCleanup` support.
  The structured engine records cleanup effects on the active lexical scope,
  drains them in LIFO order on scope exit, drains root cleanups on flow return or
  goto, and allows explicit cancellation for manual release/dispose paths.
- Added AWBC instruction, codec, verifier, VM, and product parity support for
  cleanup registration and cancellation.
- Lowered value-position `let panel = component(...)` and `let image =
  image(...)` calls to `presentation.handle.create`, scoped cleanup
  registration, and a stable string handle binding.
- Lowered handle methods `show`, `hide`, `unmount`, `release`, `destroy`,
  `close`, and `dispose` to `presentation.handle.*` effects. Terminal methods
  cancel the registered cleanup.
- Split presentation-handle helper logic into
  `crates/arcweft-runtime-plan/src/flow/presentation.rs` so the main flow
  lowerer stays below the structure-audit error threshold.
- Changed current component/View authoring syntax to `component Name() { ... }`
  with canonical `Panel`, `Column`, `Row`, and `Stack` elements. Removed
  `component ... -> View` and `Surface` / `VStack` / `HStack` as accepted
  component body syntax; they now produce structured parse diagnostics.
- Added canonical `Box` and `Scroll` View element vocabulary to parser,
  bundle-side `UiElementKind`, style selectors, and component/View sidecar
  lowering. `Box` lowers as a stack-style container and `Scroll` lowers as a
  typed vertical container so authored resource contracts no longer collapse to
  custom elements.
- Added presentation handle table epochs to runtime display snapshots. Create,
  live-state, and terminal transitions now advance a deterministic operation
  epoch, serialize created/updated epochs, preserve tombstones through serde
  roundtrips, and reject stale operations after rollback.
- Added AWBC fiber checkpoint coverage for root and lexical cleanup stacks so
  cleanup registrations survive serde save/load-style restoration in the core
  fiber state.
- Added the first typed action declaration substrate: `action` is now a
  canonical entity declaration family, parses as `EntityDeclKind::Action`,
  lowers through HIR declarations, registers as `EntityKind::Action`, and
  resolves `@action...` references. Payload signature checking and event
  dispatch are intentionally left to the follow-up action/receive slice.
- Updated current samples, parser fixtures, and stable docs/examples to use the
  canonical syntax. Historical review request markdown remains unchanged.

## Verification

- `cargo check --workspace --all-targets --all-features`
- `cargo clippy --workspace --all-targets --all-features`
- `cargo test -p arcweft-core --all-features cleanup`
- `cargo test -p arcweft-runtime-plan --all-features value_position_component_handle_lowers_to_create_cleanup_and_close_cancel`
- `cargo test -p arcweft-runtime-plan --all-features awbc_product_parity_scope_cleanup_and_cancel`
- `cargo test -p arcweft-lang-syntax --all-features`
- `cargo test -p arcweft-lang-syntax --all-features component_view_box_and_scroll_parse_as_canonical_elements`
- `cargo test -p arcweft-lang-sema --all-features`
- `cargo test -p arcweft-core --all-features fiber_checkpoint_and_serde_preserve_cleanup_stacks`
- `cargo test -p arcweft-runtime-driver --all-features presentation`
- `cargo test -p arcweft-cli --all-features component_view_box_and_scroll_lower_to_typed_ui_resources`
- `cargo test -p arcweft-lang-syntax --all-features action_declaration_parses_as_typed_entity`
- `cargo test -p arcweft-lang-sema --all-features action_entity`
- `cargo test -p arcweft-lang-sema --all-features parses_entity_declarations_used_by_presentation_docs`
- `cargo test -p arcweft-cli --all-features --test native_text_input_sample_sidecars`
- `cargo test -p arcweft-cli --all-features --test native_text_input_native_interactive_smoke`
- `cargo test -p arcweft-cli --all-features --test css_style_parity_sample`
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write target\structure-audit\current`

The final structure audit reported 0 errors and 138 warnings. Relevant current
file sizes:

- `crates/arcweft-runtime-plan/src/flow.rs`: 2413 physical LOC, production,
  no embedded tests.
- `crates/arcweft-runtime-plan/src/flow/presentation.rs`: 105 physical LOC,
  production, no embedded tests.
- `crates/arcweft-core/src/engine/flow.rs`: 988 physical LOC, production, no
  embedded tests.
- `crates/arcweft-core/src/awbc/vm.rs`: 1461 physical LOC, production, no
  embedded tests; existing size warning remains.
- `crates/arcweft-cli/src/app/bundle.rs`: 2157 physical LOC, production, no
  embedded tests; existing size warning remains.
- `crates/arcweft-lang-syntax/src/parser/items.rs`: 1354 physical LOC,
  production, no embedded tests; existing size warning remains.
- `crates/arcweft-lang-syntax/src/parser/view.rs`: 860 physical LOC,
  production, no embedded tests.

## Remaining Work

- End-to-end save subsystem wiring still needs to consume the runtime display
  snapshot and AWBC fiber cleanup checkpoint evidence added here. This cut
  verifies serde roundtrip and rollback substrate, not a full player save/load
  scenario.
- Component/image scoped capture still needs precise hidden, unmounted,
  released, and destroyed handle diagnostics and native/web/observe parity
  tests.
- Lexical cleanup integration for overlay pop and scene transition needs the
  owning overlay/scene lifecycle operations to call the cleanup drain path.
- `Scroll` is now a typed resource and sidecar element, but scroll offsets,
  clipping, input routing, save/restore of scroll state, and native/web/observe
  parity tests still need the dedicated scroll runtime behavior slice.
- The final UI syntax direction still needs action payload sema/resource
  contracts, `action.invoke`, `receive action(...)`, generic callback block
  sugar, and richer reactive branching surface from the broader input/scroll
  syntax request.
