# Seq 06.16.6 scoped presentation resource handles implementation note

Date: 2026-07-06

## Package contents

This package contains a concrete design plus an implementation patch for the
runtime snapshot/lifecycle substrate of scoped presentation resource handles.

The patch adds:

- `crates/arcweft-runtime-driver/src/presentation_handles.rs`
- `pub mod presentation_handles;` in `arcweft-runtime-driver`
- `BundlePresentationSnapshot.presentation_handles`
- canonical `presentation.handle.*` operation parsing
- deterministic lifecycle application and diagnostics
- snapshot filtering for scoped images, runtime controls, action buttons, focus
  groups, and focus navigation
- session-step diagnostic propagation
- design and implementation documentation

## Why the first implementation cut is in `arcweft-runtime-driver`

The request requires language lowering, runtime lifetime, presentation snapshots,
save/resume determinism, and observe semantics. The runtime-driver cut is the
smallest coherent implementation boundary that can be validated without
redesigning existing explicit image/component declarations.

`arcweft-core` remains Sans I/O and presentation-agnostic. The existing
`LineEffectRequest::Call(RuntimeCall)` channel carries the canonical lifecycle
operations. `arcweft-runtime-driver` parses only `presentation.handle.*` calls and
applies typed state to the portable `BundlePresentationSnapshot`.

## Lowering contract implemented by the patch

The runtime expects these canonical calls:

```text
presentation.handle.create(handle = @handle..., kind = "image", resource = @image..., owner = @scope..., visible = true)
presentation.handle.show(handle = @handle...)
presentation.handle.hide(handle = @handle...)
presentation.handle.unmount(handle = @handle...)
presentation.handle.release(handle = @handle...)
presentation.handle.dispose(handle = @handle...)
presentation.handle.destroy(handle = @handle...)
```

A follow-up parser/HIR/runtime-plan implementation should lower the grammar
specified in the design document to this exact operation surface.

## Runtime behavior implemented by the patch

On every bundle session step:

1. collect scoped handle lifecycle operations from line effects;
2. apply them to `presentation_handles`;
3. resolve ordinary explicit image effects exactly as before;
4. apply visible image handles and remove hidden/unmounted/released/destroyed
   scoped image resources from the active image list;
5. filter player-owned runtime controls and focus metadata against non-visible
   component/menu/overlay/textbox/control handles;
6. increment snapshot revision when presentation handles or filtered render data
   change;
7. append structured lifecycle diagnostics to `BundleSessionStep.diagnostics`.

Because native, web, and Agent observe already share the presentation snapshot and
player-frame planner path, lower-level renderer crates remain unaware of language
scope.

## Files changed by the patch

```text
crates/arcweft-runtime-driver/src/lib.rs
crates/arcweft-runtime-driver/src/display.rs
crates/arcweft-runtime-driver/src/session.rs
crates/arcweft-runtime-driver/src/presentation_handles.rs
docs/design/seq-06.16.6-scoped-presentation-resource-handles.md
docs/implementation/seq-06.16.6-scoped-presentation-resource-handles-2026-07-06.md
```

## Application note

The package patch required manual application in this checkout. `git apply
--check` reported a corrupt patch hunk, and `git apply --recount` then rejected
the new `presentation_handles.rs` hunk. The overlay Rust module and docs were
therefore copied from the package, while `lib.rs`, `display.rs`, and `session.rs`
were connected manually against the current `main`.

Two code-shape adjustments were made while applying the package:

- struct-like lifecycle variants are constructed with explicit closures in
  `PresentationHandleOperation::from_call`;
- session presentation update and diagnostic propagation is factored into
  `BundleSession::update_presentation_snapshot` to keep `step_with_clock` below
  the active clippy line threshold.

## Validation performed in this checkout

```bash
cargo fmt
cargo test -p arcweft-runtime-driver presentation_handles
cargo test -p arcweft-runtime-driver -- --nocapture
cargo check -p arcweft-runtime-driver
cargo clippy -p arcweft-runtime-driver --all-targets --all-features
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

Validation result:

- runtime-driver presentation handle tests: 3 passed;
- runtime-driver full crate tests: 50 passed across unit and integration tests;
- runtime-driver check: passed;
- runtime-driver clippy: passed with no warnings after application fixes;
- structural audit: 0 errors, 138 warnings.

## Remaining implementation work after this cut

This package implements the runtime snapshot/lifecycle substrate and exact
lowering contract. The following work remains as repo-side follow-up using the
acceptance criteria in the design document:

- parser/HIR surface nodes for value-position `image(...)` / `component(...)`
  handle creation;
- runtime-plan lowering of lexical cleanup stacks on block exit, flow return,
  cancellation, overlay pop, and scene transition;
- save/load and rollback snapshot schema integration, now split to
  `docs/reviews/requests/2026-07-06-seq-06.16.6.1-save-load-scoped-presentation-handles.md`;
- authored component identity propagation for component-scoped capture beyond the
  current conservative observation grouping;
- native/web/Agent parity tests once the parser/runtime-plan surface emits the
  canonical calls.

## 2026-07-06 follow-up slice: entry goto and sample lint cleanup

The next implementation slice completed the entry-dispatch part of the final UI
syntax direction and fixed a misleading sample hint observed through
`arcw run --runner native`.

Implemented:

- `entry { goto @flow.id }` now parses as a typed `EntryItem::Goto`.
- runtime-plan lowering treats entry `goto` as a flow target, preserving
  existing `start` / `run` behavior for older fixtures.
- semantic checking validates entry `goto` targets as `Flow` references.
- the project semantic graph records `entry_goto` edges separately from
  `entry_start`, `entry_run`, and `entry_route`.
- symbol-use collection includes entry `goto` targets.
- `samples/modern-feedback-view` now uses canonical entry `goto` and compact
  `image glass_bg` declaration spelling.
- direct-source runtime profile builds now emit syntax lints through the same
  structured diagnostic renderer used by `check`, so any remaining valid hints
  get source spans and patch previews instead of terse single-line output.
- `AWF0103 style::explicit_decl_id` now checks the original source spelling
  before firing. Parser-generated default ids for compact declarations such as
  `pub character concierge` no longer produce false hints.

Validation used a single Cargo feature mode (`--all-features`) for compile/test
commands in this slice after the feature-cache churn was identified:

```bash
cargo fmt
cargo test -p arcweft-lang-syntax --all-features lint::tests -- --nocapture
cargo test -p arcweft-runtime-plan --all-features entry_goto_selects_runtime_flow_from_final_syntax -- --nocapture
cargo test -p arcweft-lang-sema --all-features project_index_records_entry_and_flow_entity_relations -- --nocapture
cargo check -p arcweft-cli --all-features
cargo run -p arcweft-cli --all-features -- check --manifest-path samples\modern-feedback-view\arcw.toml
cargo run -p arcweft-cli --all-features -- run --runner web --manifest-path samples\modern-feedback-view\arcw.toml
cargo run -p arcweft-cli --all-features -- run --runner web .\samples\modern-feedback-view\src\main.arcw
cargo run -p arcweft-cli --all-features -- run --runner headless --mode drain --steps 16 --manifest-path samples\modern-feedback-view\arcw.toml
cargo clippy -p arcweft-cli --all-targets --all-features
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

Results: focused tests passed; `arcweft-cli` check and clippy passed with
`--all-features`; the modern feedback View sample checked with 0 warnings and ran
headless with 0 diagnostics; manifest and direct-source web bundle builds
succeeded without AWF0103 hints; structural audit reported 0 errors and 138
warnings.

The broader final syntax goal remains open. This slice does not implement
value-position `image(...)` / `view(...)` handle creation, lexical cleanup
stack lowering, native/web/Agent parity tests for scoped handles, or save/load
rollback integration. The save/load work is now split to
`docs/reviews/requests/2026-07-06-seq-06.16.6.1-save-load-scoped-presentation-handles.md`.

## 2026-07-06 follow-up slice: View-local input handle let binding

The final View syntax follow-up now covers the first View-local handle-binding
slice:

- `let visitor_name = input.text(@input:.visitor_name, initial = "")` parses as
  `ViewExpr::Let` inside View bodies.
- The syntax AST reports `input.text` / `input.secure` builder handles through
  `ViewBody::text_control_inputs()`.
- Bundle View program lowering emits `ViewProgramInstruction::BindLocal` with
  deterministic pattern and value schema digests.
- `TextField(visitor_name)` resolves the local handle to
  `ViewInputOptions.public_id = "input.visitor_name"` and uses the builder
  initial value for the value text source.

Validation:

```bash
cargo fmt
cargo test -p arcweft-lang-syntax --all-features view_local_let_input_handle_parses
cargo test -p arcweft-cli --all-features view_local_let_input_handle_lowers_to_program_binding
cargo test -p arcweft-lang-syntax --all-features
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features
cargo +nightly -Zscript tools/structure-audit.rs --root . --write target\structure-audit\current
```

The structure audit reported 0 errors and 139 warnings after this slice.

This slice does not close await/pending builder integration, scroll runtime
behavior, adapter parity coverage, or the save/load request linked above.
