# Dialogue line-plan typed ownership

Date: 2026-08-21

Inspected baseline: `cd1cf747f305ecb0f7b16f6ba4820d3bdaee8922`

Working-tree state at validation: dirty only with the syntax/HIR/sema/compiler
changes described below; no unrelated user changes were modified.

## Performed

- Added typed syntax nodes and roles for an attached dialogue `with` plan and
  its source-ordered body items.
- Normalized the indented `at(duration):` callback spelling into the ordinary
  typed Call/Closure expression graph without retaining semantic source text.
- Lowered the attached plan into the existing `HirLinePlan` owner and one
  child Block scope in the same final-HIR arenas.
- Checked plan-local `let` bindings in source order, checked the `out` value as
  the line result, and exposed that result through `DialogueLine<R>` while the
  surrounding source binding observes `R`.
- Added exact standard semantic authority needed by the fixture for
  `DialogueVoice.auto`, the Character-owned `stage` field, the line-context
  `voice_handle` method, the `at` callable, and stage method argument types.

## Passed

- `cargo fmt --all -- --check`
- `cargo check -p arcweft-lang-syntax -p arcweft-lang-hir -p arcweft-lang-sema -p arcweft-compiler --all-targets --all-features`
- `cargo test -p arcweft-lang-hir final_lowering::item_lowering::tests::flow::dialogue_line_plan_owns_typed_let_callback_and_out_items --all-features -- --exact --nocapture`
  - 1 focused test passed.
- `cargo test -p arcweft-lang-sema final_analysis::tests::dialogue_line_plan_bindings_are_inferred_in_source_order --all-features -- --exact --nocapture`
  - 1 focused test passed.
- `cargo test -p arcweft-lang-sema env::tests::standard_dialogue_voice_owns_auto_variant --all-features -- --exact --nocapture`
  - 1 focused test passed.
- `git diff --check`

## Failed

- `cargo test -p arcweft-cli --test check spec_valid_run_edge_fixture_now_executes --all-features -- --exact --nocapture`
  - the fixture reaches compiler runtime-semantic projection and is rejected
    because the checked `StageMethod` family has no typed executable runtime
    representation.

## Remaining work

- Lower plan setup, scheduled callback work, typed handle results, and plan
  `out` through the existing `Dialogue` plus `LineTaskGroup` runtime owner.
- Preserve native/AWBC parity and do not route the typed plan through the
  string-valued `RegisterHandle` or `LineOutRequest` compatibility-shaped
  effect payloads.
- Re-run the CLI fixture gate and the applicable runtime-plan/core/AWBC suites
  after that atomic executable cut.

## Structural review

The touched parser responsibility remains the typed syntax-event owner: it
adds dialogue-plan nodes and normalizes the callback spelling into existing
expression roles. Final HIR remains the only semantic tree owner, and sema
adds checked facts without a parallel plan model. Runtime execution is
intentionally not fabricated in this cut; the missing typed runtime boundary
is recorded above rather than hidden behind source-string reconstruction or
placeholder handle effects.
