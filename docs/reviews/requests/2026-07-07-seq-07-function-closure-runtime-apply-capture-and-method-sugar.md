# Request: Function closure runtime apply, capture analysis, and method sugar

## Context

The 2026-07-07 implementation cuts added function type syntax, typed closure
function values, expected-type `_` placeholder abstraction, scoped `^` pipe
substitution, canonical primitive labels, strict runtime lowering for
`map(_ body)`, `filter(_ predicate)`, the standard
`choices |> filter(_.enabled) |> map(_.label)` data-last collection pipeline,
and the first executable runtime function/apply substrate:
`RuntimeValue::Function`, `RuntimeExpr::Function`, `RuntimeExpr::Apply`,
deterministic capture snapshots, partial application, and curried runtime
application when the callee expression evaluates to a function value.

The next runtime-plan cut also materialized top-level pure helper names as
function values in value position, preserved exact-arity helper calls as
`RuntimeExpr::PureCall`, and lowered non-exact helper calls through
`RuntimeExpr::Apply`. Typed call disambiguation for local function-valued path
callees such as `f(1i64)` is now threaded through compiler runtime-plan
lowering evidence.

The runtime-plan cut after that lowered explicit single-parameter function type
annotations with `_` bodies, such as `let high: i64 -> bool = _ > 80i64`, into
`RuntimeExpr::Function`. Contextual expected types that are not present in the
syntax still need typed lowering evidence.

No-`^` data-last pipe lowering is now helper-aware for named pure helpers:
non-exact helper arity lowers through function apply, while exact helper arity
continues to use `RuntimeExpr::PureCall`. Sema and runtime-plan now agree that
call RHS forms append the pipe LHS to the RHS call arguments, so
`2i64 |> add(1i64)` and `2i64 |> add(lhs = 1i64)` typecheck and lower as
data-last calls rather than as calls on the result of `add(...)`. Bare
`2i64 |> add` now typechecks as a top-level `#[pure]` function prefix partial
application and lowers through the same pure helper apply path.

Curried top-level function declarations now preserve source call-group
boundaries in sema and runtime-plan callable application. `f(a, b)(c)` and
`f(a)(b)(c, d)` are covered as staged function application samples rather than
being flattened to one call group.

Closure return type annotation has also been implemented for
`|params| -> Type { ... }` and `|| -> Type { ... }`. The parser rejects
return-typed closures without a block body, sema checks the declared return type
against the block value, and curried closure call-group shape is preserved. The
expression lexer now uses a typed `ExprOp` enum instead of stringly operator
tokens for parser decisions.

This request covers the remaining work needed to finish the revised Arcweft
function/closure/currying/pipeline specification without adding compatibility
shims or preserving removed syntax.

## Required decisions

1. Complete first-class function value integration beyond the implemented
   runtime substrate.
   - Extend the implemented pure-helper function value materialization to the
     full typed function namespace, including local variables whose type is a
     function value and non-pure callable declarations where applicable.
   - Specify and implement AWBC closure allocation and bytecode apply semantics
     instead of the current `RuntimeExpr::Function` lowering diagnostic.
   - Extend the implemented helper-aware data-last pipe path to typed
     non-helper callables. Method-chain fallback has typed runtime argument
     order for positional and named data-last helper calls; spread fallback
     remains open.
   - Do not redesign the implemented `RuntimeValue::Function` /
     `RuntimeExpr::Function` / `RuntimeExpr::Apply` substrate unless concrete
     evidence shows a flaw.
   - Top-level curried declaration calls are implemented for staged call
     syntax. Bare top-level function names in sema value position and prefix
     partial calls to top-level `#[pure]` functions are implemented for the
     pure helper runtime path. Non-pure callable function-value allocation,
     trait method curried group metadata, and AWBC closure/apply allocation
     remain open.

2. Define inference boundaries for `_`.
   - `_` with an expected function type is already implemented and should not
     be redesigned unless evidence shows a concrete flaw.
   - Preserve the implemented runtime lowering for explicit single-parameter
     function annotations with `_` bodies.
   - Specify how `let is_high = (_ >= 80)` and `let add_one = add(_, 1)` infer
     function types without an explicit annotation.
   - Specify diagnostics for ambiguous or multi-parameter placeholder regions.

3. Define method-chain fallback sugar.
   - Resolution order must be: inherent method, trait method, then data-last
     callable fallback.
   - Ambiguity diagnostics must identify the conflicting candidates.
   - The design must preserve current explicit method-call behavior where a real
     method exists.

4. Define closure capture, lifetime, and effect diagnostics.
   - Specify capture inventory format.
   - Specify diagnostics for captures crossing `await`, `yield`, thread,
     line-task, and defer boundaries.
   - Specify how closure effects compose into existing effect rows.
   - Specify and implement `return expr` binding to the nearest closure or
     function-like boundary. Return type annotation and final-expression body
     typing are implemented; closure-local return control flow is still open.

5. Define LSP/tooling evidence.
   - Inlay hints for inferred closure/function types.
   - Lints for numeric fallback inside inferred closure bodies.
   - Structured diagnostics for removed or ambiguous function syntax.

## Implementation order

1. Complete typed function-valued path call disambiguation and tests. Done for
   pure helper bare paths, non-exact helper calls, top-level `#[pure]` prefix
   partial calls, and local function-valued path callees through typed lowering
   evidence. Non-pure callable allocation remains tied to AWBC/runtime design.
2. Add AWBC closure allocation / apply semantics or explicitly split that work
   into an AWBC-focused request with instruction/table design.
3. Extend expected-type `_` runtime lowering beyond explicit syntax-level
   annotations by adding expression-level typed lowering evidence from sema.
4. Add inference for `_` without an expected function type.
5. Add method-chain fallback sugar and ambiguity diagnostics.
6. Add capture/effect/lifetime diagnostics.
7. Add LSP/inlay/lint evidence.

## Tests to specify

- `let high: i64 -> bool = _ > 80i64`
- `let high = (_ > 80i64)`
- `let add_one = add(_, 1i64)`
- `values.map(_ + 1i64).sum()`
- `threshold |> choices.filter(_.score >= ^)`
- `fn f(a: i64)(b: i64) -> i64 { ... }` called as `f(1i64)(2i64)`.
  Basic staged call behavior is covered by
  `curried_function_declaration_preserves_call_group_semantics`.
- `fn tuple_tail(a: i64, b: i64)(c: i64) -> (i64, i64, i64)` called as
  `tuple_tail(1i64, 2i64)(3i64)`. Covered by
  `curried_function_declaration_handles_multi_param_groups_and_tuple_return_samples`
  and `runtime_plan_preserves_curried_call_group_application_samples`.
- `fn chain(a: i64)(b: i64)(c: i64, d: i64) -> i64` called as
  `chain(1i64)(2i64)(3i64, 4i64)`. Covered by the same sample tests.
- ambiguity between an inherent method and a data-last callable fallback
- capture across `await` rejected with a lifetime/effect diagnostic
- `|score: i64| -> bool { score >= 80i64 }` typechecks as `i64 -> bool`.
  Covered by `closure_return_type_annotation_checks_body`.
- `|score: i64| -> bool score >= 80i64` is rejected with a block-body
  diagnostic. Covered by syntax parser tests.
- `|min: i64| |value: i64| -> bool { value >= min }` typechecks as
  `i64 -> (i64 -> bool)`. Covered by
  `curried_closure_return_type_annotation_preserves_remaining_function`.

## Constraints

- Do not reintroduce `Bool`/`Char` aliases or old removed syntax.
- Do not add compatibility shims for old call or method forms.
- Do not redesign the implemented expected-type `_` path unless a concrete bug
  is found.
- Do not redesign the implemented executable `filter` path unless a concrete
  bug is found.
- Do not redesign the implemented standard `filter`/`map` data-last pipeline
  subset unless a concrete bug is found.
- Keep lower crates Sans I/O and preserve existing crate boundary direction.

## Expected output

- Updated design docs in `docs/01-language/functions-and-pipeline.md` if the
  final runtime/apply model changes the stable language contract.
- Implementation note under `docs/implementation/`.
- Focused parser/HIR/sema/runtime-plan/core eval tests.
- A structural audit entry if runtime/core representation changes cross crate
  boundaries.

## Split Follow-Ups

- `docs/reviews/requests/2026-07-07-seq-07.1-function-stack-typed-expression-lowering-evidence.md`
- `docs/reviews/requests/2026-07-07-seq-07.2-function-stack-placeholder-inference-and-method-fallback.md`
- `docs/reviews/requests/2026-07-07-seq-07.4-function-stack-capture-effect-lsp.md`
- `docs/reviews/requests/2026-07-07-seq-07.5-function-stack-awbc-closure-apply.md`
