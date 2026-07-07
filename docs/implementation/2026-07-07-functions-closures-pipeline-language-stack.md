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
- No-`^` data-last pipe lowering is helper-aware for named pure helpers:
  `2i64 |> add` lowers to function apply when the helper arity is not exact,
  while `2i64 |> add(1i64)` can remain an exact `RuntimeExpr::PureCall`.
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
- Runtime-plan expression lowering now carries a pure-helper lookup with both
  IDs and helper bodies. Bare top-level pure helper paths in value position
  materialize as `RuntimeExpr::Function`, and known helper calls with fewer or
  more than the declared helper arity lower through `RuntimeExpr::Apply` rather
  than an invalid exact-arity pure call.
- Exact-arity known pure helper calls continue to lower to
  `RuntimeExpr::PureCall`, so the existing accelerator/runtime pure call path
  remains available as an optimization.
- Intrinsic non-path callees such as `std.f64.sqrt(...)` are kept as runtime
  calls instead of being mistaken for expression-callee function apply.
- Runtime-plan lowering now turns `_` placeholder abstractions with an explicit
  single-parameter function type annotation into `RuntimeExpr::Function`, for
  example `let high: i64 -> bool = _ > 80i64`. This works for flow lets,
  stream lets, and strict runtime block lets.
- Flow runtime-plan lowering now tracks lexical local bindings that are known
  function values. Calls through those locals, such as
  `let f = add; let add_two = f(2i64); let seven = add_two(5i64)`, lower to
  `RuntimeExpr::Apply` instead of adapter-facing named calls.
- Type-check reports now include stable per-report expression IDs and
  `typed_lowering_evidence` records. Sema records function-valued call evidence
  when a callee path/expression has `TypeKind::Function`, and records
  expected-function evidence when an expression is checked in a function-typed
  context.
- Sema now accepts calls through function-valued symbols and locals instead of
  treating those path callees as unknown named functions.
- Function value calls in sema now support partial application by returning a
  remaining `TypeKind::Function` when fewer positional arguments are supplied.

## Current boundaries

- `_` without an expected function type is intentionally not inferred yet.
  Examples such as `let is_high = (_ >= 80)` still need the first-class
  function-value inference/apply design below.
- Partial call abstraction such as `add(_, 1)` type-checks only where a matching
  expected function type is supplied. It is not yet a general inference source.
- `_` expected-type runtime lowering currently consumes explicit syntax-level
  function annotations. Sema now records expected-function evidence for other
  expected-type sources, but runtime-plan still needs to consume that evidence
  before those sources can escape into runtime function values.
- Top-level pure helper functions now materialize as function values in runtime
  expression lowering, and flow lowering tracks local aliases/partial applies
  that are known function values. Sema now records function-valued path call
  evidence, but full typed call disambiguation remains open until that evidence
  is threaded into runtime-plan lowering for function arguments, opaque returns,
  or contextual expected types.
- AWBC does not yet allocate runtime closure values. `RuntimeExpr::Function`
  currently emits an AWBC lowering diagnostic, and function state is not encoded
  as an AWBC constant. `RuntimeExpr::Apply` is represented as a
  `function.apply` intrinsic for bytecode inventory purposes.
- Pipe no-`^` runtime lowering is helper-aware for named pure helpers, but
  method-chain fallback and non-helper callable resolution still need the
  final typed callable evidence path.
- Method-chain fallback sugar that resolves inherent/trait methods first and
  then data-last callable methods is not implemented.
- Closure capture analysis, suspension-boundary lifetime diagnostics, and
  effect-row integration for closure captures remain future work.
- LSP inlays and lints for inferred closure/function types and numeric fallback
  are not implemented in this cut.

## Follow-up request

- `docs/reviews/requests/2026-07-07-function-closure-runtime-apply-capture-and-method-sugar.md`
- `docs/reviews/requests/2026-07-07-function-stack-typed-expression-lowering-evidence.md`
- `docs/reviews/requests/2026-07-07-function-stack-awbc-closure-apply.md`
- `docs/reviews/requests/2026-07-07-function-stack-placeholder-inference-and-method-fallback.md`
- `docs/reviews/requests/2026-07-07-function-stack-capture-effect-lsp.md`

## Validation

```bash
cargo test -p arcweft-core --lib --all-features
cargo test -p arcweft-lang-sema --lib --all-features
cargo test -p arcweft-runtime-plan --lib --all-features
cargo test -p arcweft-core --all-features runtime_function
cargo test -p arcweft-runtime-plan --all-features closure_to_function_expr
cargo test -p arcweft-runtime-plan --all-features expression_callee_call_to_apply
cargo test -p arcweft-runtime-plan --all-features strict_runtime_lowers
cargo test -p arcweft-runtime-plan --all-features strict_runtime_value_lowering_can_emit_pure_calls
cargo test -p arcweft-runtime-plan --all-features expected_partial_placeholder
cargo test -p arcweft-runtime-plan --all-features data_last_pipe
cargo test -p arcweft-runtime-plan --all-features runtime_plan_lowers_local_function_value_calls_to_apply
cargo test -p arcweft-lang-sema --all-features records_function_value_call_lowering_evidence
cargo test -p arcweft-lang-sema --all-features typechecks_partial_function_value_application
cargo test -p arcweft-lang-sema --all-features typechecks_partial_placeholder_function_and_vec_map
cargo test -p arcweft-lang-sema --all-features
cargo test -p arcweft-runtime-plan --all-features
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

The named pure helper function-value cut has focused passing coverage for bare
helper path materialization, partial helper call lowering through
`RuntimeExpr::Apply`, exact helper calls that remain `RuntimeExpr::PureCall`,
and intrinsic non-path call preservation.

The expected partial-placeholder runtime lowering cut has focused passing
coverage for expression lowering and whole-flow runtime-plan lowering of
explicit single-parameter function annotations.

The helper-aware data-last pipe cut has focused passing coverage for direct
fallback calls, partial helper apply, and exact helper pure calls.

The local function-valued call cut has focused passing coverage for a flow that
aliases a pure helper, partially applies that local function value, and applies
the resulting local function value again.

The sema typed-lowering evidence cut has passing coverage for function-valued
symbol calls, local function-value calls after partial application,
expected-function evidence for `_` placeholder abstraction, and full
`arcweft-lang-sema` tests.
