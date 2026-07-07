# Request: Function Stack Typed Expression Lowering Evidence

## Context

The 2026-07-07 function-stack cuts implemented executable runtime function
values, explicit closure lowering, pure-helper function value materialization,
annotated `_` placeholder function lowering, and helper-aware data-last pipes.

The runtime-plan now tracks lexical flow locals that are known function values
from their lowered RHS, so direct cases such as
`let f = add; let add_two = f(2i64); let seven = add_two(5i64)` lower through
`RuntimeExpr::Apply`.

The remaining blocker is function-valued expression evidence that is known only
to sema. Runtime-plan currently receives HIR syntax plus coarse options;
`TypeCheckReport` records judgments but not stable expression identities that
can disambiguate opaque `f(1i64)` call sites as function apply versus
adapter-facing named calls.

## Required Decisions

- Define a stable expression identity/evidence model shared by sema and
  runtime-plan.
- Decide whether expression evidence is keyed by source range, traversal path,
  HIR node identity, or a new typed-HIR wrapper. Do not use lossy expression
  labels as keys.
- Record enough evidence to answer:
  - callee expression type is `Function { ... }`;
  - expected type for a value expression is a function type;
  - method resolution selected inherent/trait method versus data-last callable
    fallback;
  - ambiguity or unresolved callable diagnostics.
- Preserve the existing `RuntimeValue::Function`, `RuntimeExpr::Function`, and
  `RuntimeExpr::Apply` substrate unless concrete implementation evidence shows
  a flaw.

## Implementation Order

1. Extend sema output with expression-level typed lowering evidence.
2. Thread that evidence through `arcweft-compiler` into
   `RuntimePlanLowerOptions`.
3. Update runtime-plan strict call lowering for function-valued path callees
   whose type is known only through sema evidence.
4. Extend expected `_` runtime lowering beyond explicit syntax-level type
   annotations by consuming sema expected-type evidence.
5. Add diagnostics when evidence is missing for a checked build path.

## Tests To Specify

- Function-valued argument or opaque local call `f(1i64)` lowers to
  `RuntimeExpr::Apply` when sema evidence says `f` has function type.
- `let high = (_ > 80i64)` remains rejected until inference is designed.
- A function argument position with expected `i64 -> bool` lowers `_ > 80i64`
  into `RuntimeExpr::Function`.
- Unknown adapter calls remain `RuntimeExpr::Call` rather than being mistaken
  for apply.

## Constraints

- Do not add compatibility shims or syntax fallbacks.
- Do not hard-code individual function names.
- Do not make lower crates depend on CLI/compiler adapters.

## Expected Output

- Sema evidence data model with focused tests.
- Runtime-plan option plumbing and strict lowering tests.
- Updated implementation note linking validation evidence.
