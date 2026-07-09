# Source Function Type Effect Row Syntax - 2026-07-09

This note records the first source-syntax cut for the final closure/function
effect-row contract.

## Implemented

- Function type syntax now accepts a closed effect-row suffix:
  `A -> B effects { fs.read }`.
- Parenthesized function types can also carry that suffix:
  `(A -> B) effects { fs.read }`.
- Non-function types reject an `effects { ... }` suffix.
- Syntax preserves the row as `TypeEffectRow` on `TypeRef::Function`.
- Sema lowers that row to `TypeKind::Function { effects:
  EffectRow::closed(...) }`, including local type ascriptions and trait/member
  type conversion paths.
- Invalid effect labels in function type rows are diagnosed by the existing
  type-reference shape check.

## Evidence

- `function_types_keep_closed_effect_rows` covers direct, parenthesized, and
  non-function syntax behavior.
- `source_function_type_effect_row_becomes_closed_semantic_row` verifies a
  source type ascription preserves the closed row in semantic function type
  evidence and that the semantic source label round-trips the row.

## Still Open

This is only the closed source row surface. Open-row inference/substitution,
row variables for closure and higher-order parameters, upper-bound syntax, and
final verifier/runtime-plan/LSP policy remain under
`docs/reviews/requests/2026-07-08-seq-07.8-function-stack-closure-effect-row-final-contract.md`.
