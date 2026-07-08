# Function Stack Expression Source-Range Coverage - 2026-07-08

This note records the current source-range coverage contract for the
function/closure/currying/pipeline goal. It closes the local implementation
audit for request
`docs/reviews/requests/2026-07-07-seq-07.4.1-function-stack-expression-source-range-inlays.md`.

## Contract

Expression source identity is authored at syntax/HIR boundaries and consumed by
sema through `TypeJudgment.source_range`. The current implementation uses three
paths:

- `AuthoredExpr` for statement and flow payloads that already know their exact
  source slice.
- `expr_source` plus `expr_range` on legacy statement shapes where the root
  expression has not yet been moved to `AuthoredExpr`.
- `collect_expr_source_ranges` for nested expression children inside an
  authored root.

Generated/desugared expressions may intentionally have no authored source
range. They must not borrow a misleading range from substituted or synthesized
nodes. The concrete example is pipe `^`: the substituted LHS must not pretend
the `^` token is its source site.

## Coverage Matrix

| Surface | Coverage evidence |
| --- | --- |
| Let RHS roots and children | `let_rhs_type_judgments_carry_source_ranges` |
| Function-like body values | `function_like_body_value_judgments_carry_source_ranges` |
| Nested call args and pipe RHS roots | `nested_let_rhs_expression_judgments_carry_source_ranges` |
| Numeric bracket sequences | `numeric_bracket_sequence_judgments_carry_source_ranges` |
| Thread expression bodies | `thread_expression_body_judgments_carry_source_ranges` |
| Desugared placeholders, pipes, selectors, method fallback, closures | `desugared_function_stack_expression_judgments_keep_authored_source_ranges` |
| Assignment RHS roots/children | `assignment_statement_rhs_judgments_carry_source_ranges` |
| Typed statement branch expressions: `let-else`, statement `while let`, statement `match` guards/bodies | `typed_branch_statement_judgments_carry_source_ranges` |
| Lifetime registry writes | `lifetime_set_statement_value_judgments_carry_source_ranges` |
| Action receive and defer expressions | `action_receive_and_defer_judgments_carry_source_ranges` |
| Return and expression statements | `return_and_expression_statement_judgments_carry_source_ranges` |
| Control transfer statements, `out`, `break`, `wait`, `yield`, `close`, `select` | `control_transfer_statement_judgments_carry_source_ranges` |
| Flow control statements: `if`, `while`, `while let`, `for`, `match` | `control_statement_expression_judgments_carry_source_ranges` |
| Dialogue interpolation | `dialogue_interpolation_judgments_carry_source_ranges` |
| Dialogue-call line-plan block and colon bodies | `dialogue_call_line_plan_expression_judgments_carry_source_ranges` |
| Containers and control expressions | `container_and_control_expression_judgments_carry_source_ranges` |
| Container children and record literals | `container_child_expression_judgments_carry_source_ranges` |
| Computation blocks and braced closures | `computation_and_braced_closure_judgments_carry_source_ranges` |
| Memo options and memo body values | `memo_block_option_expression_judgments_carry_source_ranges` |
| Effect and prefix expressions | `effect_and_prefix_expression_judgments_carry_source_ranges` |
| LSP expression inlay gating and trivial-site suppression | `expression_type_inlays_are_profile_gated_and_skip_trivial_sites` |

## Newly Closed Gap

The audit found that typed statement `let-else` RHS expressions, typed
statement `while let` guards, and typed statement `match` arm guards could be
checked as ordinary expressions without authored source ranges. The parser now
constructs source-backed payloads for those sites:

- `Stmt::LetElse.expr` is now `AuthoredExpr`.
- `StmtMatchArm.guard` is now `Option<AuthoredExpr>`, with `guard()` still
  exposing the inner expression for source-agnostic readers and
  `guard_authored()` exposing the authored payload for sema/source collectors.
- Typed statement parser paths now preserve body bases for inline `let-else`
  and statement match arm blocks.

Sema uses the authored expression path for those sites, and the thread
expression source collector descends through the same authored branch payloads.

## Audit Stats

`TypeCheckStats` now reports:

- `source_backed_expr_judgments`
- `source_missing_expr_judgments`

These counters are derived whenever a `TypeJudgmentSubject::Expr` judgment is
recorded. The regression test checks that the counters match the actual
`TypeCheckReport.judgments` contents. This is intentionally a reporting aid,
not a compatibility fallback path.

## Remaining Boundaries

The source-range contract is implemented for the current authored syntax/HIR
surface. Remaining function-stack work is outside this source-range audit:
spread partial/fallback semantics, resumable AWBC dynamic apply, serializable
closure snapshots, general non-helper/effectful callable allocation, and the
full closure effect-row contract.
