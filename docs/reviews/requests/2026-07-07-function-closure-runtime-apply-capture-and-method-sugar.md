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

This request covers the remaining work needed to finish the revised Arcweft
function/closure/currying/pipeline specification without adding compatibility
shims or preserving removed syntax.

## Required decisions

1. Complete first-class function value integration beyond the implemented
   runtime substrate.
   - Specify and implement how bare named top-level functions materialize as
     function values when used as expressions.
   - Specify and implement AWBC closure allocation and bytecode apply semantics
     instead of the current `RuntimeExpr::Function` lowering diagnostic.
   - Decide whether the current direct data-last pipe lowering remains only an
     optimization or is replaced by function apply lowering.
   - Do not redesign the implemented `RuntimeValue::Function` /
     `RuntimeExpr::Function` / `RuntimeExpr::Apply` substrate unless concrete
     evidence shows a flaw.

2. Define inference boundaries for `_`.
   - `_` with an expected function type is already implemented and should not
     be redesigned unless evidence shows a concrete flaw.
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

5. Define LSP/tooling evidence.
   - Inlay hints for inferred closure/function types.
   - Lints for numeric fallback inside inferred closure bodies.
   - Structured diagnostics for removed or ambiguous function syntax.

## Implementation order

1. Add named top-level function-as-value materialization and tests.
2. Add AWBC closure allocation / apply semantics or explicitly split that work
   into an AWBC-focused request with instruction/table design.
3. Lower expected-type `_` abstractions into the runtime function/apply
   representation when they escape collection `map`/`filter` optimizations.
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
- `fn f(a: i64)(b: i64) -> i64 { ... }` called as `f(1i64)(2i64)`
- ambiguity between an inherent method and a data-last callable fallback
- capture across `await` rejected with a lifetime/effect diagnostic

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
