//! Stream-function lowering into core stream runtime data.

use crate::expr::{lower_runtime_expr, lower_runtime_expr_strict_with_pure};
use crate::pattern::lower_runtime_pattern;
use arcweft_core::plan::RuntimePureHelperId;
use arcweft_core::stream::{StreamMatchArm, StreamOp, StreamPlan, StreamRuntimeId};
use arcweft_core::value::RuntimeExpr;
use arcweft_lang_hir::syntax::{ast::flow::Stmt, types::TypeRef};
use std::collections::BTreeMap;

/// Lowers a HIR stream function into a Sans I/O stream plan.
pub(crate) fn lower_stream_function(
    function: &arcweft_lang_hir::model::HirFunction,
    pure_helpers: &BTreeMap<String, RuntimePureHelperId>,
) -> StreamPlan {
    let (item_ty, error_ty) = function
        .signature()
        .return_type()
        .and_then(stream_type_labels)
        .unwrap_or_else(|| ("Unit".to_owned(), "Unit".to_owned()));
    StreamPlan {
        id: StreamRuntimeId(function.name().to_owned()),
        item_ty,
        error_ty,
        ops: lower_stream_stmt_list(function.statements(), pure_helpers),
    }
}

fn lower_stream_stmt_list(
    statements: &[Stmt],
    pure_helpers: &BTreeMap<String, RuntimePureHelperId>,
) -> Vec<StreamOp> {
    statements
        .iter()
        .flat_map(|stmt| lower_stream_stmt(stmt, pure_helpers))
        .collect()
}

fn lower_stream_stmt(
    stmt: &Stmt,
    pure_helpers: &BTreeMap<String, RuntimePureHelperId>,
) -> Vec<StreamOp> {
    match stmt {
        Stmt::Let { pattern, expr, .. } => vec![StreamOp::Let {
            pattern: lower_runtime_pattern(pattern),
            expr: lower_runtime_expr_with_pure(expr, pure_helpers),
        }],
        Stmt::For {
            pattern,
            source,
            body,
        } => vec![StreamOp::ForNext {
            pattern: lower_runtime_pattern(pattern),
            source: lower_runtime_expr_with_pure(source, pure_helpers),
            body: lower_stream_stmt_list(body, pure_helpers),
        }],
        Stmt::Yield(expr) => vec![StreamOp::Yield {
            expr: lower_runtime_expr_with_pure(expr, pure_helpers),
        }],
        Stmt::If {
            condition,
            body,
            else_body,
        } => vec![StreamOp::If {
            condition: lower_runtime_expr_with_pure(condition, pure_helpers),
            then_ops: lower_stream_stmt_list(body, pure_helpers),
            else_ops: lower_stream_stmt_list(else_body, pure_helpers),
        }],
        Stmt::Match { expr, arms } => vec![StreamOp::Match {
            scrutinee: lower_runtime_expr_with_pure(expr, pure_helpers),
            arms: arms
                .iter()
                .map(|arm| StreamMatchArm {
                    pattern: lower_runtime_pattern(arm.pattern()),
                    guard: arm
                        .guard()
                        .map(|guard| lower_runtime_expr_with_pure(guard, pure_helpers)),
                    ops: lower_stream_stmt_list(arm.body(), pure_helpers),
                })
                .collect(),
        }],
        Stmt::Close(expr) => vec![StreamOp::Close {
            source: lower_runtime_expr_with_pure(expr, pure_helpers),
        }],
        Stmt::Return(_) => vec![StreamOp::Return],
        _ => vec![StreamOp::Noop],
    }
}

fn lower_runtime_expr_with_pure(
    expr: &arcweft_lang_hir::syntax::expr::Expr,
    pure_helpers: &BTreeMap<String, RuntimePureHelperId>,
) -> RuntimeExpr {
    lower_runtime_expr_strict_with_pure(expr, pure_helpers)
        .unwrap_or_else(|_| lower_runtime_expr(expr))
}

fn stream_type_labels(ty: &TypeRef) -> Option<(String, String)> {
    match ty {
        TypeRef::Generic { base, args } if base == "Stream" && args.len() == 2 => Some((
            crate::labels::type_label(&args[0]),
            crate::labels::type_label(&args[1]),
        )),
        _ => None,
    }
}
