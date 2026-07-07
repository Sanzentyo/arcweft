# Request: Function Stack Placeholder Inference and Method Fallback

## Context

Expected-type `_` is implemented for explicit function annotations and the
collection `map`/`filter` optimization paths. No-expected-type inference and
partial-call abstraction now exist for the constrained cases recorded below,
but parenthesized grouping, broader inference, and method-chain fallback remain
open.

Method-chain fallback is also not implemented: real inherent/trait methods
should win first, and only unresolved method syntax should fall back to
data-last callable application.

Implementation progress on 2026-07-07:

- Sema now infers no-expected `_` abstraction for unambiguous binary expressions
  where the non-placeholder operand has a static/local type, such as
  `let high = _ > 80i64`.
- Sema now infers partial-call abstraction for known positional function
  signatures, such as `let add_one = add(_, 1i64)`.
- Runtime-plan lowering consumes the inferred function evidence and lowers both
  forms to `RuntimeExpr::Function`.
- Parenthesized grouping such as `let is_high = (_ > 80i64)` is still open
  because the current parser/HIR expression shape does not preserve that group
  as the binary expression for type checking.

## Required Decisions

- Extend inference rules for `_` regions without an expected function type
  beyond the implemented unambiguous binary and known positional callable
  cases.
- Define multi-placeholder behavior beyond the implemented "repeated `_` means
  the same generated parameter when all inferred parameter types agree" rule.
- Extend partial call abstraction lowering beyond known positional signatures,
  including named/spread arguments if they are accepted.
- Define method-chain resolution order:
  1. inherent method;
  2. trait method;
  3. data-last callable fallback.
- Define ambiguity diagnostics that report the conflicting method/callable
  candidates.

## Implementation Order

1. Extend sema inference for no-expected `_` regions. Done for unambiguous
   binary expressions with a static/local non-placeholder operand.
2. Implement partial call abstraction as function values. Done for known
   positional callable signatures.
3. Add parser/HIR grouping support so parenthesized partial expressions preserve
   the inferred body expression.
4. Implement method-chain fallback using typed callable evidence, not string
   labels alone.
5. Add diagnostics for ambiguous or unsupported fallback cases.

## Tests To Specify

- `let is_high = _ > 80i64`. Covered by
  `infers_partial_placeholder_function_without_expected_type`.
- `let is_high = (_ > 80i64)`. Open until parser/HIR grouping is implemented.
- `let add_one = add(_, 1i64)`. Covered by
  `infers_partial_call_abstraction_without_expected_type` and
  `runtime_plan_lowers_inferred_partial_placeholder_functions`.
- Repeated `_` in one abstraction region with compatible inferred parameter
  types.
- Inherent method wins over data-last fallback.
- Trait method wins over data-last fallback.
- Ambiguous fallback reports all candidates.

## Constraints

- Do not accept removed syntax.
- Do not add formatter normalization shims.
- Do not special-case a single callable such as `add`.

## Expected Output

- Sema inference tests.
- Runtime-plan function/apply lowering tests.
- Structured diagnostics and docs updates.
