pub mod agent;
pub mod assertion_identity;
mod assertion_lower;
mod assertion_projection;
pub mod awbc_lower;
pub mod errors;
mod final_expr;
mod final_pattern;
#[path = "final_flow.rs"]
pub mod flow;
pub mod semantic_facts;

use arcweft_core::value::RuntimeExpr;
use arcweft_lang_hir::{identity::ExprId, module::HirModule};
use semantic_facts::RuntimePlanSemanticFacts;

/// Lowers one expression through the sole generation-bound final-HIR runtime
/// expression authority.
pub fn lower_checked_runtime_expression(
    module: &HirModule,
    facts: &RuntimePlanSemanticFacts,
    expression: ExprId,
) -> Result<RuntimeExpr, String> {
    final_expr::FinalExprLowerer::new(module, facts).lower(expression)
}
