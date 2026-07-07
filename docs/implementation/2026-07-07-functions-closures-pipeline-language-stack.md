# Functions, closures, currying, and pipeline language stack — 2026-07-07

Source brief: `C:\Users\sanze\.codex\attachments\d352da6f-4ba7-4807-a050-504287f3559f\pasted-text.txt`.

## Implemented in the current sequence

- Function type syntax and semantic type substrate:
  - `A -> B`
  - right-associated function types
  - tuple call-group function parameters
- Typed closure parameters in expression and callback-block syntax.
- Closure expressions now type-check as function values instead of returning an
  untyped `None`.
- Curried top-level function/task/dialogue/stream signatures are accepted by
  the parser/HIR surface where already modeled; curried `flow` signatures are
  rejected directly.
- Pipe placeholder `^` is scoped to the RHS of `|>`.
- Pipe RHS with `^` substitutes the pipe LHS into the RHS expression before
  type checking and strict runtime lowering.
- Pipe RHS without `^` currently lowers to the existing direct data-last call
  path for strict runtime expressions.
- Canonical primitive labels are enforced across sema/runtime-facing surfaces:
  `bool` and `char` are accepted; legacy `Bool`/`Char` aliases are rejected.
- `_` partial placeholder now works when an expected one-parameter function
  type is available:
  - `let high: i64 -> bool = _ > 80i64`
  - `choices.map(_.label)`
  - repeated `_` in the same abstraction region uses the same generated
    parameter type.
- `Vec`/array/slice/sequence `map` now checks its argument as `item -> _`,
  so explicit closures and `_` placeholder bodies share the same expected-type
  path.
- Strict runtime lowering converts `values.map(_ + 1i64)` into the existing
  executable `RuntimeExpr::Map` form.

## Current boundaries

- `_` without an expected function type is intentionally not inferred yet.
  Examples such as `let is_high = (_ >= 80)` still need the first-class
  function-value inference/apply design below.
- Partial call abstraction such as `add(_, 1)` type-checks only where a matching
  expected function type is supplied. It is not yet a general inference source.
- Strict runtime does not yet have a first-class expression callee/apply form,
  so true curried runtime application `f(a)(x)` is not represented end to end.
  The existing pipe no-`^` runtime lowering still uses the direct data-last
  call shape for the currently supported executable subset.
- Executable collection `filter` is not implemented in `RuntimeExpr`/core eval.
- Method-chain fallback sugar that resolves inherent/trait methods first and
  then data-last callable methods is not implemented.
- Closure capture analysis, suspension-boundary lifetime diagnostics, and
  effect-row integration for closure captures remain future work.
- LSP inlays and lints for inferred closure/function types and numeric fallback
  are not implemented in this cut.

## Follow-up request

- `docs/reviews/requests/2026-07-07-function-closure-runtime-apply-capture-and-method-sugar.md`

## Validation

```bash
cargo test -p arcweft-lang-sema --lib --all-features
cargo test -p arcweft-runtime-plan --lib --all-features
cargo clippy -p arcweft-lang-sema --all-targets --all-features
cargo clippy -p arcweft-runtime-plan --all-targets --all-features
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

All listed validation passed after updating the lingering
`spec_should_pass/check/025_view_body_structured.arcw` fixture from `Bool` to
canonical `bool`. The structure audit reported 0 errors and 147 existing
warnings.
