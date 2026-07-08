# Function Stack Pipe Control-Expression RHS - 2026-07-09

This note records the focused parser/sema/runtime-plan hardening slice for
pipe RHS control expressions.

## Problem

The function-stack goal says `^` is scoped to the RHS of `|>` and substitutes
the pipe LHS inside that RHS. The existing implementation handled ordinary
calls, field selections, records, tuples, arrays, binary/unary expressions, and
value `if` expressions, but it missed value-producing `if let` and `match`
expressions.

That left two inconsistent paths:

- `lhs |> if cond { ^ } else { ... }` was structured and substituted.
- `lhs |> if let PAT = ^ { ... } else { ... }` and
  `lhs |> match ^ { ... }` could fall back to raw expression parsing or
  data-last call lowering before the `^` traversal saw the control expression.

## Implemented Shape

- The expression parser now accepts `if`, `if let`, and `match` as prefix
  value expressions inside ordinary expression positions, including pipe RHS.
- `let` statement parsing now lets successfully parsed RHS expressions with
  `else` win before the let-else fallback, so a parseable
  `maybe |> if let ... else ...` remains a value expression instead of being
  misclassified as let-else.
- Sema pipe lowering now descends into `Expr::IfLet` scrutinees, guards,
  branches, and `Expr::Match` scrutinees, guards, and arm values when detecting
  or substituting `^`.
- Runtime-plan pipe lowering uses the same traversal, so checked runtime plans
  keep structured `RuntimeExpr::IfLet` and `RuntimeExpr::Match` shapes with the
  pipe LHS substituted as the control-expression scrutinee.
- Runtime-plan partial-placeholder lowering now also descends into
  `Expr::IfLet` and `Expr::Match`, keeping `_` placeholder abstraction
  traversal aligned with sema's detection path.
- Parser control-expression prefix parsing lives in
  `crates/arcweft-lang-syntax/src/expr/control_parse.rs`, and runtime-plan
  placeholder/pipe traversal lives in
  `crates/arcweft-runtime-plan/src/expr/desugar.rs`. This keeps the root
  expression modules below the production-file error threshold while preserving
  their ownership boundaries.

## Evidence

Focused regressions cover:

- direct `parse_expr` coverage for `maybe |> if let .Some(value) = ^ ...`;
- direct `parse_expr` coverage for `ready |> match ^ { ... }`;
- checked compiler/runtime-plan lowering where the resulting
  `RuntimeExpr::IfLet` scrutinee is `RuntimeExpr::Local("maybe")`;
- checked compiler/runtime-plan lowering where the resulting
  `RuntimeExpr::Match` scrutinee is `RuntimeExpr::Local("ready")`;
- the existing sema pipe placeholder/data-last fixture.

The structural audit report for this slice is checked in under
`docs/implementation/structure-audits/function-stack-pipe-control-expression-rhs-2026-07-09/`.
It reports 0 errors / 151 warnings after the split. Current touched-file
measurements include:

| File | Bytes | Physical LOC | Role |
| --- | ---: | ---: | --- |
| `crates/arcweft-lang-syntax/src/expr/control_parse.rs` | 5,786 | 160 | production |
| `crates/arcweft-lang-syntax/src/expr.rs` | 71,680 | 2,355 | production |
| `crates/arcweft-runtime-plan/src/expr/desugar.rs` | 16,464 | 433 | production |
| `crates/arcweft-runtime-plan/src/expr.rs` | 76,873 | 2,187 | production |
| `crates/arcweft-lang-sema/src/checker/expr/pipe.rs` | 10,127 | 278 | production |
| `crates/arcweft-compiler/src/tests.rs` | 127,073 | 3,935 | test |

This is a hardening of the already accepted fixed-argument pipe/placeholder
contract. It does not widen spread partial/fallback semantics, suspension-aware
dynamic apply, persisted closure snapshots, or broad non-helper callable
allocation.
