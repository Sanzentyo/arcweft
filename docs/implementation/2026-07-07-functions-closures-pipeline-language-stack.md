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
- Closure parameter patterns are preserved through sema: tuple/destructuring
  patterns bind body locals from the parameter type, and pattern `_` remains a
  discard rather than becoming the expression partial placeholder.
- Closure expressions now type-check as function values instead of returning an
  untyped `None`.
- Curried top-level function/task/dialogue/stream signatures are accepted by
  the parser/HIR surface where already modeled; curried `flow` signatures are
  rejected directly.
- Pipe placeholder `^` is scoped to the RHS of `|>`.
- Pipe RHS with `^` substitutes the pipe LHS into the RHS expression before
  type checking and strict runtime lowering.
- Pipe RHS without `^` appends the pipe LHS to RHS call arguments for both sema
  and strict runtime-plan lowering. For example, `2i64 |> add(1i64)` and
  `2i64 |> add(lhs = 1i64)` typecheck as data-last calls rather than as calls
  on the result of `add(...)`.
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
  `2i64 |> add` typechecks as a pure prefix partial application and lowers to
  function apply when the helper arity is not exact, while
  `2i64 |> add(1i64)` and `2i64 |> add(lhs = 1i64)` can remain exact
  `RuntimeExpr::PureCall` values.
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
- Sema now accepts prefix partial calls to top-level `#[pure]` functions, such
  as `add(2i64)` and `2i64 |> add`, returning the remaining function type. The
  same syntax remains rejected for non-pure top-level functions until their
  runtime/AWBC function-value allocation is designed.
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
- Sema now accepts named missing-input partial application for top-level
  `#[pure]` helper signatures, such as `let add_to_one = add(right = 1i64)`.
  Runtime-plan lowering emits a `RuntimeExpr::Function` whose parameters are
  the missing helper inputs and whose body calls the pure helper with provided
  named arguments in helper input order.
- Method-call syntax now has a typed data-last callable fallback for the
  positional case where no real method resolves and a function signature exists
  with the receiver as the last parameter. For example,
  `score.above(80i64)` can lower as `above(80i64, score)`. Sema records
  lowering evidence for this decision so real inherent/env/trait methods still
  win when they exist.
- Data-last method fallback now records an explicit typed runtime argument
  order. Named fallback calls such as `score.above(min = 80i64)` typecheck and
  lower as `above(80i64, score)` without relying on source argument order.
  Spread fallback candidates still produce a structured unsupported-fallback
  diagnostic instead of degrading to a generic `unknown method` error.
- Data-last pipe lowering now covers local function-valued aliases in addition
  to direct pure-helper names. For example,
  `let f = add; let partial = 2i64 |> f; let exact = 2i64 |> f(1i64)`
  typechecks through function-value evidence and lowers both pipe forms to
  `RuntimeExpr::Apply` against `Local("f")`, preserving data-last argument
  order without reclassifying the local as a helper.
- Expression lexing now represents operators as `Token::Op(ExprOp::...)`
  rather than raw operator string payloads. Parser branches for `->`, `=>`,
  `|>`, range operators, comparison operators, and closure pipes are checked by
  Rust exhaustiveness/type checking; the spelling strings live only in the
  enum-backed display/canonical spelling tables. The expression token for
  `->` is `ExprOp::ThinArrow`, matching the CST-level
  `ArcweftPunctuation::ThinArrow` naming.
- CST/parser multi-token punctuation now uses `ArcweftPunctuation` helpers for
  grammar-significant `->`, `<-`, `=>`, and `|>` splitting/prefix checks. The
  spelling strings are centralized in the CST punctuation layer rather than
  repeated across type, closure, choice, source, line-plan, view, and statement
  parsers.
- Product AWBC session save/load now rejects `RuntimeValue::Function` payloads
  with structured `BundleSessionSaveError::UnsupportedRuntimeValue` diagnostics
  before export or restore accepts the snapshot. The validator walks fiber
  frames, cleanup stacks, suspension args, await-many state, source/stream
  queues, terminal values, and nested tuple/sequence/record/variant/iterator
  values so captured closures cannot be persisted accidentally before AWBC
  closure allocation has a versioned representation.
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
- Type-check reports now include closure capture inventory keyed by stable
  expression IDs. Sema records deterministic local captures for closure bodies,
  including captures of outer closure parameters by nested closures, while
  excluding locals declared inside the closure itself.
- Sema now records suspension boundaries seen inside each closure capture
  frame and rejects borrowed closure captures that cross those boundaries. The
  diagnostic is structured as
  `sema.typecheck.borrowed_closure_capture_crosses_boundary` and carries the
  capture name, borrowed type, lifetime labels, and owning boundary; tests cover
  `await`, `thread`, and `defer`, while non-borrow captures may cross `await`.
- Sema effect collection now gives closure literals their own synthetic
  private callable, so `let f = || { effectful_call() }` records the body
  effects on `closure.expr.N` instead of treating function-value creation as an
  immediate effect in the enclosing flow/function. Direct calls through the
  local closure binding, such as `f()`, compose that synthetic callable back
  into the caller's effect graph and therefore still respect explicit effect
  bounds. Partial application of a local closure binding carries the same
  synthetic callable to the partial alias without composing effects until that
  alias is called.
- Parenthesized closure literals are now parsed as normal prefix expressions,
  so immediate closure calls such as `(|| -> String { ... })()` and partial
  immediate closure application such as `(|path: String, suffix: String| ->
  String { ... })("story.arcw")` lower to structured call expressions instead
  of raw expression fallbacks. Sema composes closure body effects for exact
  immediate calls and carries the same synthetic closure callable to partial
  immediate aliases without performing the body effects until the alias is
  called.
- Built-in collection higher-order methods now compose closure argument body
  effects into the caller for `map` and `filter`. This covers direct closure
  arguments, local closure aliases, and partial closure aliases, while still
  leaving function-value creation and partial application themselves
  effect-free.
- User-defined top-level higher-order function calls now compose callback
  closure body effects into the caller when sema proves the callee function
  directly invokes the corresponding function-typed parameter. The checker
  records pending call-site edges so this works even though flows are checked
  before top-level function bodies. It covers direct closure arguments and
  local closure aliases, exact later curried call groups, and the same
  pending-edge path is used by data-last method fallback calls. Functions that
  merely retain/pass a callback without invoking the parameter remain
  effect-free at the call site.
- Sema now emits `sema.numeric.fallback_in_inferred_closure` warnings when an
  unsuffixed numeric literal or numeric sequence falls back to a stable default
  primitive type inside a closure body whose return type is inferred. Explicit
  closure return annotations and concrete expected function return types
  suppress the warning because they provide the numeric contract.
- LSP inlay hints now include inferred function-valued `let` bindings when the
  source binding has no explicit type ascription and sema proves the binding is
  a `TypeKind::Function`. This covers inferred closures such as `let f = || 1`
  and inferred partial-placeholder functions such as `let p = _ > 80i64`,
  using the same canonical `TypeKind::source_label()` surface spelling as sema
  diagnostics.
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
- Partial call abstraction such as `add(_, 1)` is inferred for known signatures
  with positional arguments and fixed named arguments. Repeated `_`
  placeholders in one partial-call region use the same generated parameter when
  all placeholder positions infer the same parameter type. Runtime lowering
  reorders named pure-helper arguments by helper input name before emitting the
  call body. Named missing-input partial application is implemented for
  top-level `#[pure]` helper signatures. Spread partial-call inference and
  ambiguous multi-candidate callables remain open.
- `_` expected-type runtime lowering consumes explicit syntax-level function
  annotations and sema expected-function evidence threaded through compiler
  options.
- Top-level pure helper functions now materialize as function values in runtime
  expression lowering, and flow lowering tracks local aliases/partial applies
  that are known function values. Sema function-valued path call evidence is
  now threaded into flow, stream, source, and Agent bundle runtime-plan
  lowering. Prefix partial calls to top-level `#[pure]` functions are accepted
  in sema and remain scoped to the pure helper runtime path.
- AWBC does not yet allocate runtime closure values. `RuntimeExpr::Function`
  currently emits an AWBC lowering diagnostic, and function state is not encoded
  as an AWBC constant. `RuntimeExpr::Apply` is represented as a
  `function.apply` intrinsic for bytecode inventory purposes.
- Pipe no-`^` runtime lowering is helper-aware for named pure helpers. Method
  syntax fallback now has typed lowering evidence for data-last helper
  signatures, including named method arguments. No-`^` pipe lowering also
  preserves local function-valued aliases as `RuntimeExpr::Apply` targets for
  both bare and call RHS forms. Spread fallback candidates are diagnosed as
  unsupported rather than silently becoming unknown methods. Multiple viable
  data-last fallback candidates from the module and external type-check
  environment now report `sema.typecheck.ambiguous_data_last_method_fallback`
  with all candidate labels instead of selecting one by merge order. Real
  env/inherent/trait methods still win, and viable data-last fallback
  candidates hidden by that real method now produce
  `sema.typecheck.shadowed_data_last_method_fallback` warnings. Executable
  spread fallback lowering, curried call-group runtime fallback metadata, and
  non-pure top-level callable runtime allocation remain open.
- Curried declaration call-group metadata is now preserved for sema/runtime-plan
  callable application and sema trait/impl method calls. AWBC closure/apply
  allocation remains open.
- Closure capture lifetime diagnostics now cover borrowed local captures that
  cross checked suspension boundaries. Closure body effect composition is
  implemented for synthetic closure callables, direct calls through local
  closure bindings, immediate closure calls, partial application aliases, and
  built-in collection higher-order execution through `map`/`filter`. Direct
  calls and data-last method fallback calls to user-defined top-level
  higher-order functions also compose closure body effects for function-typed
  parameters that the callee body directly invokes, including exact later
  curried call groups and local aliases to those staged function values.
  Partial later-group callback application, destructured callback parameters,
  callback invocations hidden inside returned/stored closures, and LSP-facing
  closure effect evidence remain open. Save/load
  currently has an explicit Product AWBC policy: runtime function values are
  rejected as non-persistable until AWBC closure allocation and snapshot
  versioning are designed. Numeric fallback lints
  inside inferred closure bodies are implemented for scalar integer/float
  fallback and numeric sequence fallback. LSP inlays are implemented for
  inferred function-valued `let` bindings; broader expression inlays still need
  a source-span contract for sema expression evidence.
- Closure `return expr` now binds to the nearest closure/function-like sema
  boundary for type checking. Strict runtime block lowering already preserves
  simple early-return shape by discarding later block statements after a
  lowered `return`, but structured closure control-flow lowering beyond the
  current `RuntimeExpr::Function` subset remains tied to the AWBC closure/apply
  work.
- Method-chain fallback sugar resolves after existing env/builtin/integer/
  handle/trait method checks, preserving real methods before data-last fallback.
  Ambiguity diagnostics for multiple viable data-last fallback candidates are
  implemented for module/environment callable overlap. Real method versus
  fallback overlap is surfaced as a non-fatal shadowing warning while preserving
  real method priority.
- Closure capture inventory collection and borrowed-capture suspension-boundary
  lifetime diagnostics exist in sema. Effect-row integration for closure
  captures and runtime-plan capture metadata policy remain open.
- LSP inlays currently cover inferred function-valued `let` bindings. Full
  arbitrary expression inlays remain open until sema expression evidence has
  source spans rather than traversal-only expression IDs.

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
cargo test -p arcweft-lang-sema --all-features data_last_pipe_through_local_function_value_records_call_evidence
cargo test -p arcweft-lang-sema --all-features curried_function_declaration
cargo test -p arcweft-lang-sema --all-features closure_return
cargo test -p arcweft-lang-sema --all-features closure_body_effects_do_not_leak_on_function_value_creation
cargo test -p arcweft-lang-sema --all-features local_closure_call_composes_body_effects_into_caller
cargo test -p arcweft-lang-sema --all-features partial_local_closure_application_does_not_compose_until_called
cargo test -p arcweft-lang-sema --all-features partial_local_closure_alias_composes_body_effects_when_called
cargo test -p arcweft-lang-sema --all-features user_higher_order_function_argument
cargo test -p arcweft-lang-sema --all-features curried_higher_order
cargo test -p arcweft-lang-sema --all-features method_chain_data_last_fallback_composes_higher_order_callback_effects
cargo test -p arcweft-lang-sema --all-features
cargo test -p arcweft-lsp --all-features inlay_hint_request
cargo test -p arcweft-lsp --all-features
cargo test -p arcweft-compiler --all-features runtime_plan_uses_typecheck_evidence_for_function_value_calls
cargo test -p arcweft-compiler --all-features runtime_plan_uses_expected_function_evidence_for_placeholder_args
cargo test -p arcweft-compiler --all-features runtime_plan_uses_typecheck_evidence_across_stream_and_source_exprs
cargo test -p arcweft-compiler --all-features runtime_plan_lowers_inferred_partial_placeholder_functions
cargo test -p arcweft-compiler --all-features runtime_plan_lowers_typed_data_last_method_fallback
cargo test -p arcweft-compiler --all-features runtime_plan_lowers_local_function_data_last_pipe_to_apply
cargo test -p arcweft-compiler --all-features runtime_plan_preserves_curried_call_group_application_samples
cargo test -p arcweft-compiler --all-features
cargo test -p arcweft-runtime-driver --all-features --test awbc_product_session
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
split, the structure audit reports 0 errors and 146 warnings. After the
user-defined higher-order function argument effect cut, focused sema tests,
full `arcweft-lang-sema` tests, workspace clippy, and structure audit passed;
the structure audit reports 0 errors and 148 warnings. Workspace clippy still
reports the existing `TraitMember` / `ImplMember` large enum warnings in
`arcweft-lang-syntax`. After the exact curried later-call-group callback
effect cut, focused `curried_higher_order` tests, full `arcweft-lang-sema`
tests, workspace check, workspace clippy, and structure audit passed; the
structure audit still reports 0 errors and 148 warnings, and clippy still only
reports the same existing large enum warnings.

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
fallback calls, pure prefix partial helper apply, exact helper pure calls, and
sema/runtime alignment for call RHS forms including bare, positional, and named
helper calls.

The local function-valued data-last pipe cut has passing sema coverage for
function-value call evidence on both bare and call RHS pipe forms through a
local function alias, and compiler/runtime-plan coverage that those forms lower
to `RuntimeExpr::Apply` with `Local("f")` as the callee rather than a helper or
adapter call.

The Product AWBC save/load policy cut has passing `awbc_product_session`
coverage that a snapshot containing a runtime function value inside a cleanup
argument is rejected on both direct restore and encoded import with
`BundleSessionSaveError::UnsupportedRuntimeValue`, preserving the current
non-persistable closure boundary explicitly.
The cut also passed `cargo check --workspace --all-targets --all-features`,
`cargo clippy --workspace --all-targets --all-features` with only the existing
`TraitMember`/`ImplMember` large enum warnings, and structure audit with 0
errors / 147 warnings.

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
repeated positional partial-call placeholders, named partial-call placeholders,
typed data-last method fallback, and real method priority. Compiler
coverage confirms the inferred placeholder forms lower to `RuntimeExpr::Function`,
repeated and named partial-call placeholders reuse one generated runtime local
at each placeholder site, named pure-helper arguments are reordered by helper
input name, and typed data-last method fallback lowers to a pure helper call
using sema-proven argument order with the receiver appended as the last argument.

The data-last fallback diagnostic/order cut has passing sema coverage for named
method-call syntax that matches a data-last fallback candidate, ensuring it
records deterministic argument order evidence instead of falling back to source
order. Spread syntax still reports `UnsupportedDataLastMethodFallback` instead
of a generic unknown method. It also covers module/environment data-last fallback ambiguity with
`method_chain_reports_ambiguous_data_last_fallback_candidates`, which verifies
both candidate labels are reported and no arbitrary fallback lowering evidence
is recorded. The shadowed-fallback warning cut covers real env-method and trait
method priority with `method_chain_prefers_real_method_over_data_last_callable_fallback`
and `method_chain_prefers_trait_method_over_data_last_callable_fallback`.

The closure return type cut has passing parser coverage for `|params| -> Type
{ ... }`, `|| -> Type { ... }`, call-argument closures, and the required block
body diagnostic. Sema coverage confirms declared closure return types typecheck
against body values, mismatch diagnostics are produced, curried closure return
types preserve remaining function values, and multiline return-typed closure
lets are consumed as one statement.

The closure capture and pattern-parameter cuts have passing sema coverage for
borrowed closure captures crossing `await`, `yield`, thread, and defer
boundaries. Suspension boundaries are now represented internally as typed
`SuspensionBoundary` values and converted to stable diagnostic labels only at
the diagnostic boundary. Closure parameter pattern coverage confirms tuple
destructuring and discard parameters typecheck without confusing pattern `_`
with expression `_` placeholder abstraction.

The closure effect-composition cut has passing sema coverage that closure body
effects do not leak into the enclosing flow when the closure value is merely
created, while a direct call through the local closure binding composes those
body effects into the caller and triggers the existing explicit effect-bound
diagnostic.
The partial closure alias follow-up has passing sema coverage that partial
application of a local closure binding does not compose the closure body
effects at partial-value creation time, and that calling the partial alias
composes the body effects into the caller.
The collection higher-order effect cut has passing sema coverage that
`map`/`filter` closure arguments compose body effects into the enclosing flow,
including direct closure arguments, local closure aliases, and partial closure
aliases.
The closure effect-composition cut and partial-alias follow-up both passed
`cargo check --workspace --all-targets --all-features`,
`cargo clippy --workspace --all-targets --all-features` with only the existing
`TraitMember`/`ImplMember` large enum warnings, and structure audit with 0
errors / 148 warnings.

The curried trait method metadata cut has passing sema coverage for preserving
the remaining call-group function type after a method call and rejecting
flattened curried trait method arguments.

The canonical primitive spelling cut has passing coverage for rejecting
`Bool`, `Char`, and `string` in type annotations/signatures with direct
canonical replacement diagnostics. The native text input sample now uses
`String` return annotations.

The latest local function-valued data-last pipe validation reports structure
audit 0 errors / 147 warnings. `cargo clippy --workspace --all-targets
--all-features` still reports the existing `TraitMember` and `ImplMember`
`large_enum_variant` warnings in `arcweft-lang-syntax/src/ast/items.rs`; no
new clippy warning remains from the function/closure changes.
