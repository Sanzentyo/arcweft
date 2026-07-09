# Closure Effect-Row LSP Hover - 2026-07-09

This note records a 07.8 consumer slice for the active function/closure/
currying/pipeline goal.

## Implemented

- LSP hover now uses closure expression `FunctionEffectCallable` evidence to
  display the closure expression's closed effect row.
- The hover is limited to the closure header `|...|` range so body expressions
  keep their own hover behavior and do not masquerade as the closure value.
- The hover consumes the same `ClosedEffectRowReport` boundary as declaration
  effect-row hover instead of reading sema effect-graph internals.

## Evidence

- `hover_describes_closure_expression_closed_effect_row` verifies that hovering
  a closure header reports `inferred: { fs.read }` for a closure body that
  reads through an effectful capability.
- The same regression verifies that body hover content does not become
  the closure-expression effect-row hover.

## Still Open

This is still current closed-row evidence, not the final row language. Source
row syntax, open-row inference/substitution, row variables for closure and
higher-order parameters, and final verifier/runtime-plan consumption remain
under
`docs/reviews/requests/2026-07-08-seq-07.8-function-stack-closure-effect-row-final-contract.md`.
