# Converged language/content surface adoption — 2026-08-17

Inspected revision: `82cfc4c976f030291220654ed2db0eefa42bf8aa`

Working tree at inspection: clean.

## Performed

- Adopted the converged language, Dialogue/RichText, View, and presentation
  authoring decisions into the maintained stable chapter
  `docs/01-language/converged-language-surface.md`.
- Updated the documentation indexes and the directly conflicting pipeline,
  Try, and reactive View sections.
- Kept implementation state separate from the stable target specification.

## Current source evidence

- Prefix Try is the selected expression carrier; production search found no
  `PostfixQuestion` or `HirTryForm::PostfixQuestion` symbol in the inspected
  language/runtime-plan crates.
- The current Dialogue surface scanner still contains `DollarParen` and
  `AsciiCompact` variants in
  `crates/arcweft-lang-syntax/src/text/dialogue_surface.rs`.
- The final generic `#call(...)[content]` role-preserving vertical slice,
  removal of legacy Ruby/tag readers, and reactive View `match Need` product
  projection were not validated as implemented by this documentation cut.

## Passed

- Not yet run for this documentation-only cut.

## Not run

- Cargo checks and tests. No Rust source changed in this cut.
- Link checking and documentation rendering.

## Remaining implementation work

- Delete `$(expr)`, compact-curly Ruby, paired Ruby tags, `[! ...]`, and unknown
  dot-selector fallback after their final typed replacements are connected.
- Connect recursive `#call(...)[content]` and callee-owned content roles through
  syntax, HIR, sema, runtime plan, runtime/AWBC, formatter, and LSP.
- Project View-context ordinary `match Need` into the retained reactive product
  and delete every `AwaitView` success path.
- Complete the maintained stable-doc sweep so older detailed examples no longer
  show replaced aliases.

## Known Try conflicts at the inspected revision

The final surface is already prefix-only at syntax/HIR level, but the checked
and runtime behavior has not yet converged:

- `final_analysis/analyzer/expressions.rs` checks a Try operand directly. A bare
  placeholder receives no expected type and fails as `ExpressionTypeUnavailable`;
  the ordinary implicit callable boundary described for `try _` is not
  constructed or tested there.
- The same analyzer checks a Pipe left and right independently. It does not
  publish a typed pipe-left binding consumed by `HirPlaceholderKind::PipeLeft`,
  so the specified `try ^` composition is not yet an admitted checked path.
- Result Try validates the enclosing propagation error, while Option Try simply
  unwraps its operand type in the visible branch. The two carriers therefore do
  not yet share one checked propagation-boundary authority.
- `runtime-plan/final_expr.rs` rejects `HirExprKind::Try` from pure expression
  lowering. `final_flow.rs` recognizes Try only when its operand contains an
  Await/Loop flow value and represents it as
  `RuntimeFlowValueContinuation::Try`, so generic Try is not yet lowered once.
- Await lowering still special-cases that continuation: an Error handler that
  falls through returns the explicit error `fallthrough from an Await Error
  handler requires a typed generic Try propagation fact`. This is the remaining
  ad-hoc Try/Await fusion. The final model requires Await to yield its checked
  residual `Result` and ordinary Try to consume it; handler coverage may reduce
  the residual error to `Never`, but must not create an Await-specific Try path.

These conflicts are deliberately recorded rather than repaired in this
documentation cut.
