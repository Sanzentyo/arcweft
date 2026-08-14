# Current-source evidence
- `crates/arcweft-lang-sema/src/final_analysis/report.rs`: `FinalSemanticAnalysis` owns exact snapshots/world/revision and exhaustive `expressions: BTreeMap<ExprId, CheckedExpression>`; publication already performs duplicate collection and complete-inventory validation.
- `crates/arcweft-runtime-plan/src/semantic_facts.rs`: existing generation-bound `RuntimePlanSemanticFacts`, `RuntimeNormalizedType`, `RuntimeTypeShape`, exact semantic identity, checked projection and unsupported-shape errors.
- `crates/arcweft-runtime-plan/src/final_expr.rs`: `FinalExprLowerer` currently borrows only `HirModule` + `RuntimePlanSemanticFacts` and recursively creates `RuntimeExpr`; this is the exact migration point.
- `docs/implementation/2026-08-14-external-lowering-generation-admission-return-invalid.md`: authoritative residual blocker inventory for this child correction.
