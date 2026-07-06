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
- Updated current samples, parser fixtures, and stable docs/examples to use the
  canonical syntax. Historical review request markdown remains unchanged.

## Verification

- `cargo check --workspace --all-targets --all-features`
- `cargo clippy --workspace --all-targets --all-features`
- `cargo test -p arcweft-core --all-features cleanup`
- `cargo test -p arcweft-runtime-plan --all-features value_position_component_handle_lowers_to_create_cleanup_and_close_cancel`
- `cargo test -p arcweft-runtime-plan --all-features awbc_product_parity_scope_cleanup_and_cancel`
- `cargo test -p arcweft-lang-syntax --all-features`
- `cargo test -p arcweft-lang-sema --all-features`
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
- `crates/arcweft-cli/src/app/bundle.rs`: 2155 physical LOC, production, no
  embedded tests; existing size warning remains.
- `crates/arcweft-lang-syntax/src/parser/items.rs`: 1354 physical LOC,
  production, no embedded tests; existing size warning remains.
- `crates/arcweft-lang-syntax/src/parser/view.rs`: 860 physical LOC,
  production, no embedded tests.

## Remaining Work

- Save/load and rollback semantics for handle tables and cleanup stacks remain
  to be integrated with the save subsystem.
- Component/image scoped capture still needs precise hidden, unmounted,
  released, and destroyed handle diagnostics and native/web/observe parity
  tests.
- Lexical cleanup integration for overlay pop and scene transition needs the
  owning overlay/scene lifecycle operations to call the cleanup drain path.
- The final UI syntax direction still needs the typed action/receive syntax,
  scroll primitive details, and richer reactive branching surface from the
  broader input/scroll syntax request.
