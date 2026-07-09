# Closure Effect-Row LSP Hover - 2026-07-09

This note records a 07.8 consumer slice for the active function/closure/
currying/pipeline goal.

## Implemented

- LSP hover now uses closure expression `FunctionEffectCallable` evidence to
  display the closure expression's effect row.
- Closure hover also displays an expected-function effect row as the
  closure expression's upper bound when the closure is checked against a typed
  function ascription such as `String -> String effects { fs.read }`.
- The hover is limited to the closure header `|...|` range so body expressions
  keep their own hover behavior and do not masquerade as the closure value.
- The hover consumes the same owned raw `EffectRowReport` boundary as
  declaration effect-row hover instead of reading sema effect-graph internals.

## Evidence

- `hover_describes_closure_expression_closed_effect_row` verifies that hovering
  a closure header reports `inferred: { fs.read }` for a closure body that
  reads through an effectful capability.
- The same regression verifies that body hover content does not become
  the closure-expression effect-row hover.
- `hover_describes_closure_expression_expected_effect_row_bound` verifies that
  the same hover reports `upper bound: { fs.read }` when that row comes from
  the closure's expected function type.

## Still Open

This is still current row evidence, not the final row language. Source row
syntax, open-row inference/substitution that produces rows from checked
programs, row variables for closure and higher-order parameters, and final
verifier/runtime-plan consumption remain under
`docs/reviews/requests/2026-07-08-seq-07.8-function-stack-closure-effect-row-final-contract.md`.
