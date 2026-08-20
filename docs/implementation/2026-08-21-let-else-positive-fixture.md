# Typed LetElse positive-fixture closure

Date: 2026-08-21

Inspected baseline: `6c9034a1becfb57ee5f4b1c1e694933204f86b4d`

Working-tree state at validation: dirty only with the LetElse syntax-access,
HIR, sema, tests, and this implementation record.

## Performed

- Added exact attached access for the parser-owned `LetElseStatement` pattern,
  initializer, and failure block.
- Lowered LetElse into its existing final `HirStmtKind::LetElse` owner with one
  statement-owned failure Block scope and success-only outer locals.
- Added dedicated LetElse binding policies that preserve `LetBinding` locals
  while allowing refutable patterns and retaining Predicate/Proof mutability
  and reserved-name rules.
- Extended independent source-freeze validation for ordinary and candidate
  expression graphs; no source-text reader or detached statement model was
  introduced.
- Inferred an unconstrained `Some(value)` result as `Option<type(value)>` and
  used that exact instantiation for both provisional and finalized callee
  facts.

## Passed

- `cargo fmt --all -- --check`
- `cargo check -p arcweft-lang-syntax -p arcweft-lang-hir -p arcweft-lang-sema --all-targets --all-features`
- `cargo test -p arcweft-lang-syntax -p arcweft-lang-hir -p arcweft-lang-sema --all-features`
  - HIR unit: 833 passed, 8 ignored; its integration/API suites passed.
  - sema unit: 207 passed; 10 API and 4 mismatch integration tests passed.
  - syntax unit: 669 passed; public API/parser and doc-test suites passed.
- `cargo test -p arcweft-lang-hir final_lowering::expression_lowering::tests::dialogue_candidate_block::candidate_let_else_owns_failure_scope_and_success_binding --all-features -- --exact --nocapture`
  - 1 focused candidate-freeze test passed after the full suite.
- `cargo run -p arcweft-cli --quiet -- check tests/fixtures/arcw/current_pass/check/008_let_else_diverge.arcw`
  - check and verification passed with two flows and no warnings or
    obligations.
- deterministic path-order scan of `tests/fixtures/arcw/current_pass/check`
  - `001` through `008` passed.
- `git diff --check`

## Failed

- The same path-order scan next fails at
  `009_choice_static_goto.arcw`: one semantic expression has no admissible
  final type. This is the next positive-fixture owner to diagnose.

## Not run

- Workspace-wide tests and Clippy were not run for this focused gate cut.

## Structural review

The large statement-lowering and block-projection files remain cohesive closed
dispatch owners for the complete typed statement family. LetElse was added to
those existing exhaustive matrices rather than split into a competing reader.
The callable refinement remains in the shared candidate probe/finalization
path so the `Some` result and callee facts cannot diverge between phases.
