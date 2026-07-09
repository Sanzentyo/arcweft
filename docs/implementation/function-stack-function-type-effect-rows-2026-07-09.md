# Function Type Effect Rows - 2026-07-09

This note records the first row-bearing callable-value slice for the active
function/closure/currying/pipeline goal and the 07.8 effect-row final-contract
request.

## Implemented

- `TypeKind::Function` now carries an `EffectRow` alongside parameter and
  return types.
- `TypeKind::function(...)` constructs an unknown-row function type for source
  or inferred shapes where the final row is not known yet.
- `TypeKind::function_with_effects(...)` constructs a function type with an
  explicit row.
- Environment and project function signatures can now project a first-class
  function value type with a known closed row from their registered effects.
- Function-value partial application preserves the callee row on the remaining
  function type.
- Signature partial calls preserve the referenced function's registered row
  on the returned function value.
- Function type compatibility now compares row payloads instead of silently
  ignoring them. Unknown rows remain permissive while closed rows require the
  actual row to be covered by the expected row.
- Function source labels render closed non-empty rows, for example
  `String -> String effects { fs.read }`.

## Evidence

- `environment_function_value_type_carries_closed_effect_row` verifies that an
  environment function value reference carries a closed `fs.read` row in both
  the let-binding type judgment and function-value reference lowering evidence.
- `cargo check -p arcweft-lang-sema --all-targets --all-features` passed after
  the representation change.
- Structure audit output was written to
  `docs/implementation/structure-audits/function-stack-function-type-effect-rows-2026-07-09`.
  The audit reports one size error for the pre-existing 2510-LOC
  `checker/expr.rs`; the scoped exception and required split follow-up are
  recorded in that audit directory.
  The follow-up split is recorded in
  `docs/implementation/function-stack-expr-path-split-2026-07-09.md`.

## Still Open

This is not the final 07.8 contract by itself. Source-level row syntax, open-row
inference/substitution, row variables for closure and higher-order parameters,
and final runtime-plan/verifier/LSP consumption beyond current evidence remain
open under
`docs/reviews/requests/2026-07-08-seq-07.8-function-stack-closure-effect-row-final-contract.md`.
