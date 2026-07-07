# Request: Function closure runtime apply, capture analysis, and method sugar

## Context

The 2026-07-07 implementation cut added function type syntax, typed closure
function values, expected-type `_` placeholder abstraction, scoped `^` pipe
substitution, canonical primitive labels, and strict runtime lowering for
`map(_ body)` and `filter(_ predicate)`.

This request covers the remaining work needed to finish the revised Arcweft
function/closure/currying/pipeline specification without adding compatibility
shims or preserving removed syntax.

## Required decisions

1. Define the final runtime representation for first-class function values and
   expression callee application.
   - It must represent closure values, named function values, partial-call
     abstractions such as `add(_, 1)`, and true curried application `f(a)(x)`.
   - It must specify how captures are stored and restored in deterministic
     runtime state.
   - It must decide whether the current direct data-last pipe lowering remains
     only an optimization or is replaced by function apply lowering.

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

1. Add typed runtime function/apply representation and strict lowering tests.
2. Lower explicit closures and expected-type `_` abstractions into that runtime
   representation while keeping existing `RuntimeExpr::Map` as an optimization
   if the design chooses to.
3. Add inference for `_` without an expected function type.
4. Add method-chain fallback sugar and ambiguity diagnostics.
5. Add capture/effect/lifetime diagnostics.
6. Add LSP/inlay/lint evidence.

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
- Keep lower crates Sans I/O and preserve existing crate boundary direction.

## Expected output

- Updated design docs in `docs/01-language/functions-and-pipeline.md` if the
  final runtime/apply model changes the stable language contract.
- Implementation note under `docs/implementation/`.
- Focused parser/HIR/sema/runtime-plan/core eval tests.
- A structural audit entry if runtime/core representation changes cross crate
  boundaries.
