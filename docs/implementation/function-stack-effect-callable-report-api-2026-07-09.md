# Effect Callable Report API - 2026-07-09

This note records a small boundary cleanup for the active function/closure/
currying/pipeline goal.

## Implemented

- `TypeCheckReport::function_effect_callable_for_expression(...)` now exposes
  the callable used by effect-row reports for a function-valued expression.
- LSP closure effect-row hover now uses that report API instead of matching
  `TypedLoweringEvidenceKind::FunctionEffectCallable` directly.

## Evidence

- `closure_effect_callable_evidence_joins_type_judgment_to_closed_row` now
  verifies the sema report API path from closure expression judgment to closed
  effect row.
- `hover_describes_closure_expression_closed_effect_row` verifies the LSP
  consumer still displays the closure expression's closed row through that
  boundary.

## Still Open

This is still current closed-row evidence. Source row syntax, open-row
inference/substitution, row variables for closure and higher-order parameters,
and final verifier/runtime-plan semantics remain under
`docs/reviews/requests/2026-07-08-seq-07.8-function-stack-closure-effect-row-final-contract.md`.
