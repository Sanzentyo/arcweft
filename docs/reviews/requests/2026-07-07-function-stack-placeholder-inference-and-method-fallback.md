# Request: Function Stack Placeholder Inference and Method Fallback

## Context

Expected-type `_` is implemented for explicit function annotations and the
collection `map`/`filter` optimization paths. No-expected-type inference and
partial-call abstraction now exist for the constrained cases recorded below,
but broader inference and method-chain fallback remain open.

Method-chain fallback is partially implemented for typed positional data-last
callables: real methods win first, and unresolved method syntax can fall back
when a callable signature takes the receiver as its last parameter.

Implementation progress on 2026-07-07:

- Sema now infers no-expected `_` abstraction for unambiguous binary expressions
  where the non-placeholder operand has a static/local type, such as
  `let high = _ > 80i64` and `let high = (_ > 80i64)`.
- Sema now infers partial-call abstraction for known positional function
  signatures, such as `let add_one = add(_, 1i64)`.
- Runtime-plan lowering consumes the inferred function evidence and lowers both
  forms to `RuntimeExpr::Function`.
- Sema now resolves positional data-last method fallback when no real method
  matches and a callable signature exists with the receiver as its last
  parameter, such as `score.above(80i64)` lowering as `above(80i64, score)`.
  The decision is exported as typed lowering evidence, and runtime-plan lowering
  consumes that evidence before emitting a helper call. Real env/inherent/trait
  methods keep priority and do not emit fallback evidence.

## Required Decisions

- Extend inference rules for `_` regions without an expected function type
  beyond the implemented unambiguous binary and known positional callable
  cases.
- Define multi-placeholder behavior beyond the implemented "repeated `_` means
  the same generated parameter when all inferred parameter types agree" rule.
- Extend partial call abstraction lowering beyond known positional signatures,
  including named/spread arguments if they are accepted.
- Extend method-chain resolution beyond the implemented typed positional
  fallback:
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
3. Parenthesized partial expressions now use the same inferred body expression
   path as unparenthesized partial expressions.
4. Method-chain fallback uses typed callable evidence for positional data-last
   signatures whose receiver is the final parameter. Named/spread fallback
   arguments, curried call-group metadata, and explicit ambiguity diagnostics
   remain open.
5. Add diagnostics for ambiguous or unsupported fallback cases.

## Tests To Specify

- `let is_high = _ > 80i64`. Covered by
  `infers_partial_placeholder_function_without_expected_type`.
- `let is_high = (_ > 80i64)`. Covered by
  `infers_parenthesized_partial_placeholder_function_without_expected_type` and
  `runtime_plan_lowers_inferred_partial_placeholder_functions`.
- `let add_one = add(_, 1i64)`. Covered by
  `infers_partial_call_abstraction_without_expected_type` and
  `runtime_plan_lowers_inferred_partial_placeholder_functions`.
- Repeated `_` in one abstraction region with compatible inferred parameter
  types.
- Inherent method wins over data-last fallback.
- Trait method wins over data-last fallback.
- Ambiguous fallback reports all candidates.
- `score.above(80i64)` where `above(min: i64, value: i64) -> bool`. Covered by
  `method_chain_falls_back_to_data_last_callable_when_no_method_matches` and
  `runtime_plan_lowers_typed_data_last_method_fallback`.
- Real method priority over fallback. Covered by
  `method_chain_prefers_real_method_over_data_last_callable_fallback`.

## Constraints

- Do not accept removed syntax.
- Do not add formatter normalization shims.
- Do not special-case a single callable such as `add`.

## Expected Output

- Sema inference tests.
- Runtime-plan function/apply lowering tests.
- Structured diagnostics and docs updates.
