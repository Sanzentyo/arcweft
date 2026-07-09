# Closure Effect Callable Evidence - 2026-07-09

This note records a narrow 07.8 effect-row boundary hardening slice for the
active function/closure/currying/pipeline goal.

## Implemented

- Closure expressions now emit `TypedLoweringEvidenceKind::FunctionEffectCallable`
  with the closure expression's `TypeExpressionId` and the synthetic
  `CallableId` used by effect analysis.
- Compiler evidence lowering preserves that callable identity as
  `RuntimeTypedLoweringEvidenceKind::FunctionEffectCallable`.
- The evidence gives downstream consumers a stable join path:
  `TypeJudgmentSubject::Expr { id, kind: "closure" }` ->
  `FunctionEffectCallable { callable }` ->
  `ClosedEffectRowReport::summary(callable)`.

## Evidence

- `closure_effect_callable_evidence_joins_type_judgment_to_closed_row` verifies
  that a closure expression with an `adapter.read_text` body exports callable
  evidence keyed to the closure expression judgment and that the same callable
  resolves to a closed inferred `fs.read` row.

## Still Open

This is not the final 07.8 row contract. Source row syntax, open-row
inference/substitution, row variables for closure and higher-order parameters,
and final verifier/LSP/runtime consumers remain open under
`docs/reviews/requests/2026-07-08-seq-07.8-function-stack-closure-effect-row-final-contract.md`.
