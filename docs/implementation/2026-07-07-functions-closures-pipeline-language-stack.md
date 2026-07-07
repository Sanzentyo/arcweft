# Functions, closures, currying, and pipeline language stack — 2026-07-07

Source briefs:

- `C:\Users\sanze\.codex\attachments\d352da6f-4ba7-4807-a050-504287f3559f\pasted-text.txt`
- `C:\Users\sanze\.codex\attachments\232a9edf-275f-4c4a-86b9-c447fe38e452\pasted-text.txt`

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
  Non-canonical primitive spellings such as `string` now produce diagnostics
  pointing to `String` instead of silently becoming user-defined nominal types.
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
- Bare top-level function names now type-check as first-class function values
  in value position when their `FunctionSignature` has parameter metadata. For
  example, `let f = add; let add_two = f(2i64)` records function-value call
  evidence for `f`.
- Sema now preserves curried declaration call-group boundaries for top-level
  and extern capability function signatures. For example,
  `tuple_tail(a, b)(c) -> (i64, i64, i64)` is modeled as a first call group
  returning `c -> (i64, i64, i64)`, and `chain(a)(b)(c, d) -> i64` retains the
  two remaining groups after `chain(a)`.
- Sema now preserves curried trait/impl method call-group boundaries during
  method-call checking. For example, `fn above(self, min: i64)(value: i64)
  -> bool` makes `score.above(80i64)` typecheck as `i64 -> bool`, while
  `score.above(80i64, 81i64)` is rejected as a flattened curried call group.
- Compiler lowering now converts sema typed-lowering evidence into
  runtime-plan-local evidence and threads it through `RuntimePlanLowerOptions`.
- Runtime-plan lowering shares one typed expression cursor across flow, stream,
  and source lowering, and Agent bundle compilation now passes typecheck
  evidence into the Agent controller runtime-plan entrypoint.
- Strict runtime expression lowering consumes function-valued call evidence so
  path calls such as `f(1i64)` lower to `RuntimeExpr::Apply` when sema proved
  `f` is a function value. Without that evidence, the same unknown path call
  remains an adapter-facing `RuntimeExpr::Call`.
- Runtime-plan lowering preserves curried pure-helper application as staged
  `RuntimeExpr::Apply`, including `tuple_tail(1i64, 2i64)(3i64)` as `[2, 1]`
  argument groups and `chain(1i64)(2i64)(3i64, 4i64)` as `[1, 1, 2]`.
- Strict runtime expression lowering consumes expected-function evidence so
  placeholder abstractions in function-argument positions, such as
  `accept(_ > 80i64)` where `accept` expects `i64 -> bool`, lower the argument
  to `RuntimeExpr::Function`.
- Sema now infers `_` placeholder function values without an explicit expected
  function type for unambiguous binary expressions whose non-placeholder side
  has a local/static type, such as `let high = _ > 80i64` and
  `let high = (_ > 80i64)`.
- Sema now infers partial-call abstraction for known positional callable
  signatures, such as `let add_one = add(_, 1i64)`, without hard-coding the
  callable name. Runtime-plan lowering consumes the inferred evidence and
  lowers both forms to `RuntimeExpr::Function`.
- Method-call syntax now has a typed data-last callable fallback for the
  positional case where no real method resolves and a function signature exists
  with the receiver as the last parameter. For example,
  `score.above(80i64)` can lower as `above(80i64, score)`. Sema records
  lowering evidence for this decision so real inherent/env/trait methods still
  win when they exist.
- Data-last method fallback candidates that use named or spread arguments now
  produce a structured unsupported-fallback diagnostic instead of degrading to
  a generic `unknown method` error. Executable fallback lowering remains
  positional-only.
- Expression lexing now represents operators with a dedicated `ExprOp` enum
  instead of string tokens such as `Op("->")`, so parser branches for `->`,
  `=>`, `|>`, range operators, comparison operators, and closure pipes are
  checked by Rust exhaustiveness/type checking rather than string literals.
- Closure return type annotation is accepted as `|params| -> Type { ... }`
  and `|| -> Type { ... }`. Return-typed closures require block bodies.
  Parser tests cover top-level, zero-arg, call-argument, and missing-block
  cases.
- Sema checks declared closure return types against both expected function
  types and the block body result. Curried closures such as
  `|min: i64| |value: i64| -> bool { value >= min }` typecheck as
  `i64 -> (i64 -> bool)`.
- Sema now treats closure bodies as their own return boundary. `return expr`
  inside `|| -> Type { ... }` checks against the closure return type rather
  than an outer function/flow return type, and unannotated closures still block
  outer return expectations from leaking inward.
- Return statements now compare their value type against the active
  function-like return boundary instead of relying only on tail-expression body
  checking. This catches mismatches such as `|| -> bool { return 1i64 }`.
- Flow statement parsing now keeps multiline return-typed closure literals
  together as a single `let` statement by tracking existing CST punctuation
  depth while consuming statement continuations.
- `expr.rs` was split further by moving closure source splitting and character
  literal decoding into `expr/closure_source.rs` and `expr/char_literal.rs`,
  keeping the expression parser below the structure-audit error threshold.
- UI interaction view-surface examples were updated from removed
  `ForEach(...) |item| { ... }` / unsupported `Grid(...)` authoring to the
  current `for item in items key = item.id { ... }` View DSL and supported
  container elements.

## Current boundaries

- `_` without an expected function type is inferred only when the parameter type
  is available without speculative expression checking. The current cut covers
  unambiguous binary expressions and positional calls to known function
  signatures, including parenthesized binary placeholder expressions.
- Partial call abstraction such as `add(_, 1)` is inferred for known positional
  signatures. Named/spread partial-call inference and ambiguous multi-candidate
  callables remain open.
- `_` expected-type runtime lowering consumes explicit syntax-level function
  annotations and sema expected-function evidence threaded through compiler
  options.
- Top-level pure helper functions now materialize as function values in runtime
  expression lowering, and flow lowering tracks local aliases/partial applies
  that are known function values. Sema function-valued path call evidence is
  now threaded into flow, stream, source, and Agent bundle runtime-plan
  lowering.
- AWBC does not yet allocate runtime closure values. `RuntimeExpr::Function`
  currently emits an AWBC lowering diagnostic, and function state is not encoded
  as an AWBC constant. `RuntimeExpr::Apply` is represented as a
  `function.apply` intrinsic for bytecode inventory purposes.
- Pipe no-`^` runtime lowering is helper-aware for named pure helpers. Method
  syntax fallback now has typed lowering evidence for positional data-last
  helper signatures. Named/spread fallback candidates are diagnosed as
  unsupported rather than silently becoming unknown methods, but executable
  named/spread fallback lowering, curried call-group runtime fallback metadata,
  and non-helper callable runtime lowering remain open.
- Curried declaration call-group metadata is now preserved for sema/runtime-plan
  callable application and sema trait/impl method calls. AWBC closure/apply
  allocation remains open.
- Closure `return expr` now binds to the nearest closure/function-like sema
  boundary for type checking. Strict runtime block lowering already preserves
  simple early-return shape by discarding later block statements after a
  lowered `return`, but structured closure control-flow lowering beyond the
  current `RuntimeExpr::Function` subset remains tied to the AWBC closure/apply
  work.
- Method-chain fallback sugar resolves after existing env/builtin/integer/
  handle/trait method checks, preserving real methods before data-last fallback.
  Ambiguity diagnostics that compare real method and fallback candidates remain
  open.
- Closure capture analysis, suspension-boundary lifetime diagnostics, and
  effect-row integration for closure captures remain future work.
- LSP inlays and lints for inferred closure/function types and numeric fallback
  are not implemented in this cut.

## Follow-up request

- `docs/reviews/requests/2026-07-07-seq-07-function-closure-runtime-apply-capture-and-method-sugar.md`
- `docs/reviews/requests/2026-07-07-seq-07.1-function-stack-typed-expression-lowering-evidence.md`
- `docs/reviews/requests/2026-07-07-seq-07.2-function-stack-placeholder-inference-and-method-fallback.md`
- `docs/reviews/requests/2026-07-07-seq-07.4-function-stack-capture-effect-lsp.md`
- `docs/reviews/requests/2026-07-07-seq-07.5-function-stack-awbc-closure-apply.md`

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
cargo test -p arcweft-lang-sema --all-features infers_partial_placeholder_function_without_expected_type
cargo test -p arcweft-lang-sema --all-features infers_parenthesized_partial_placeholder_function_without_expected_type
cargo test -p arcweft-lang-sema --all-features infers_partial_call_abstraction_without_expected_type
cargo test -p arcweft-lang-sema --all-features method_chain
cargo test -p arcweft-lang-sema --all-features curried_function_declaration
cargo test -p arcweft-lang-sema --all-features closure_return
cargo test -p arcweft-lang-sema --all-features
cargo test -p arcweft-compiler --all-features runtime_plan_uses_typecheck_evidence_for_function_value_calls
cargo test -p arcweft-compiler --all-features runtime_plan_uses_expected_function_evidence_for_placeholder_args
cargo test -p arcweft-compiler --all-features runtime_plan_uses_typecheck_evidence_across_stream_and_source_exprs
cargo test -p arcweft-compiler --all-features runtime_plan_lowers_inferred_partial_placeholder_functions
cargo test -p arcweft-compiler --all-features runtime_plan_lowers_typed_data_last_method_fallback
cargo test -p arcweft-compiler --all-features runtime_plan_preserves_curried_call_group_application_samples
cargo test -p arcweft-compiler --all-features
cargo test -p arcweft-lang-syntax --all-features closure
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
symbol calls, bare top-level function names in value position, local
function-value calls after partial application, expected-function evidence for
`_` placeholder abstraction, and full `arcweft-lang-sema` tests.

The compiler/runtime-plan typed evidence cut has passing coverage for
function-valued path calls lowering to `RuntimeExpr::Apply` only when typecheck
evidence is supplied, and for expected-function placeholder arguments lowering
to `RuntimeExpr::Function`. The shared-cursor follow-up has passing coverage
for function-valued calls inside stream and source lowering after earlier flow
expressions have consumed typed expression IDs.

The inferred partial-placeholder and method-fallback cuts have passing sema
coverage for unannotated binary placeholder inference, parenthesized binary
placeholder inference, partial-call abstraction from known function signatures,
typed positional data-last method fallback, and real method priority. Compiler
coverage confirms the inferred placeholder forms lower to `RuntimeExpr::Function`
and typed data-last method fallback lowers to a pure helper call with the
receiver appended as the last argument.

The data-last fallback diagnostic cut has passing sema coverage for named and
spread method-call syntax that matches a data-last fallback candidate, ensuring
it reports `UnsupportedDataLastMethodFallback` instead of a generic unknown
method.

The closure return type cut has passing parser coverage for `|params| -> Type
{ ... }`, `|| -> Type { ... }`, call-argument closures, and the required block
body diagnostic. Sema coverage confirms declared closure return types typecheck
against body values, mismatch diagnostics are produced, curried closure return
types preserve remaining function values, and multiline return-typed closure
lets are consumed as one statement.

The curried trait method metadata cut has passing sema coverage for preserving
the remaining call-group function type after a method call and rejecting
flattened curried trait method arguments.

The canonical primitive spelling cut has passing coverage for rejecting
`Bool`, `Char`, and `string` in type annotations/signatures with direct
canonical replacement diagnostics. The native text input sample now uses
`String` return annotations.

The final validation cut reports structure audit 0 errors / 146 warnings after
the expression parser module split. `cargo clippy --workspace --all-targets
--all-features` still reports the existing `TraitMember` and `ImplMember`
`large_enum_variant` warnings in `arcweft-lang-syntax/src/ast/items.rs`; no
new clippy warning remains from the function/closure changes.
