use arcweft_lang_hir::syntax::expr::{CallArg, Expr, Placeholder};

pub(super) fn expr_contains_partial_placeholder(expr: &Expr) -> bool {
    match expr {
        Expr::Placeholder(Placeholder::Partial) => true,
        Expr::Tuple(items) | Expr::BracketSeq(items) => {
            items.iter().any(expr_contains_partial_placeholder)
        }
        Expr::ArrayRepeat { value, len }
        | Expr::Binary {
            lhs: value,
            rhs: len,
            ..
        } => expr_contains_partial_placeholder(value) || expr_contains_partial_placeholder(len),
        Expr::Call(call) => {
            expr_contains_partial_placeholder(call.callee())
                || call
                    .args()
                    .iter()
                    .any(call_arg_contains_partial_placeholder)
        }
        Expr::Select(select) => expr_contains_partial_placeholder(select.target()),
        Expr::Try { expr: target } => expr_contains_partial_placeholder(target),
        Expr::Index { target, index } => {
            expr_contains_partial_placeholder(target) || expr_contains_partial_placeholder(index)
        }
        Expr::Unary { expr, .. } => expr_contains_partial_placeholder(expr),
        Expr::Await(awaited) => expr_contains_partial_placeholder(awaited.operand()),
        Expr::Record { fields, .. } | Expr::RecordLiteral(fields) => fields
            .iter()
            .any(|(_, value)| expr_contains_partial_placeholder(value)),
        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_contains_partial_placeholder(condition)
                || expr_contains_partial_placeholder(then_branch)
                || else_branch
                    .as_deref()
                    .is_some_and(expr_contains_partial_placeholder)
        }
        Expr::IfLet {
            expr,
            guard,
            then_branch,
            else_branch,
            ..
        } => {
            expr_contains_partial_placeholder(expr)
                || guard
                    .as_deref()
                    .is_some_and(expr_contains_partial_placeholder)
                || expr_contains_partial_placeholder(then_branch)
                || else_branch
                    .as_deref()
                    .is_some_and(expr_contains_partial_placeholder)
        }
        Expr::Match { scrutinee, arms } => {
            expr_contains_partial_placeholder(scrutinee)
                || arms.iter().any(|arm| {
                    arm.guard().is_some_and(expr_contains_partial_placeholder)
                        || expr_contains_partial_placeholder(arm.value())
                })
        }
        _ => false,
    }
}

fn call_arg_contains_partial_placeholder(arg: &CallArg) -> bool {
    expr_contains_partial_placeholder(arg.value())
}
