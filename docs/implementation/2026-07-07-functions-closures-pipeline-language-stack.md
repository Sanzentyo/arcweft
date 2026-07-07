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
  path for general strict runtime expressions.
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
- `Vec`/array/slice/sequence `filter` now checks its argument as `item -> bool`.
- Strict runtime lowering converts `choices.filter(_.enabled)` into executable
  `RuntimeExpr::Filter`.
- Core VM pure/runtime expression evaluation executes `RuntimeExpr::Filter`
  over runtime iterators and returns a normalized value sequence.
- Standard prelude-shaped data-last collection pipeline now recognizes
  `choices |> filter(_.enabled) |> map(_.label)` and lowers it through the same
  executable `RuntimeExpr::Filter`/`RuntimeExpr::Map` path.
- Runtime function/apply substrate is now typed in the executable runtime:
  - `RuntimeValue::Function` stores parameters, body, and deterministic capture
    bindings.
  - `RuntimeExpr::Function` evaluates explicit runtime closures into captured
    function values.
  - `RuntimeExpr::Apply` applies expression callee values, supports partial
    application when fewer arguments are supplied, and supports curried
    application when an application returns another function value.
- Strict runtime lowering now converts explicit closures into
  `RuntimeExpr::Function`.
- Strict runtime lowering now converts non-path expression callee calls such as
  `make_adder(2i64)(5i64)` into `RuntimeExpr::Apply`.
- Core VM/pure evaluation now executes captured runtime functions, partial
  application, and curried function application.
- Runtime-plan, verify, accelerator, CLI, agent-runner, render-text, and host
  value labels now understand runtime function values instead of relying on
  wildcard handling.

## Current boundaries

- `_` without an expected function type is intentionally not inferred yet.
  Examples such as `let is_high = (_ >= 80)` still need the first-class
  function-value inference/apply design below.
- Partial call abstraction such as `add(_, 1)` type-checks only where a matching
  expected function type is supplied. It is not yet a general inference source.
- Named top-level functions are not yet materialized as function values when a
  bare function path is used as an expression. Expression-callee application is
  available once the callee expression evaluates to `RuntimeValue::Function`.
- AWBC does not yet allocate runtime closure values. `RuntimeExpr::Function`
  currently emits an AWBC lowering diagnostic, and function state is not encoded
  as an AWBC constant. `RuntimeExpr::Apply` is represented as a
  `function.apply` intrinsic for bytecode inventory purposes.
- Pipe no-`^` runtime lowering still uses the direct data-last call shape
  outside the standard executable `filter`/`map` collection pipeline subset.
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
cargo test -p arcweft-core --lib --all-features
cargo test -p arcweft-lang-sema --lib --all-features
cargo test -p arcweft-runtime-plan --lib --all-features
cargo test -p arcweft-core --all-features runtime_function
cargo test -p arcweft-runtime-plan --all-features closure_to_function_expr
cargo test -p arcweft-runtime-plan --all-features expression_callee_call_to_apply
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

All listed validation passed after updating the lingering
`spec_should_pass/check/025_view_body_structured.arcw` fixture from `Bool` to
canonical `bool`. The structure audit reported 0 errors and 147 existing
warnings for the first cut. After the executable `filter` cut and structural
split, the structure audit reports 0 errors and 146 warnings.

The runtime function/apply cut has focused passing coverage for captured
function application, partial application, curried application, closure strict
lowering, and expression-callee call lowering. Workspace validation for this cut
is recorded in the commit/final response.
