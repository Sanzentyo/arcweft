//! Stream-function lowering into core stream runtime data.

use crate::expr::lower_runtime_expr;
use crate::pattern::lower_runtime_pattern;
use arcweft_core::stream::{StreamMatchArm, StreamOp, StreamPlan, StreamRuntimeId};
use arcweft_lang_syntax::{Stmt, TypeRef};

/// Lowers a HIR stream function into a Sans I/O stream plan.
pub(crate) fn lower_stream_function(function: &arcweft_lang_hir::HirFunction) -> StreamPlan {
    let (item_ty, error_ty) = function
        .signature()
        .return_type()
        .and_then(stream_type_labels)
        .unwrap_or_else(|| ("Unit".to_owned(), "Unit".to_owned()));
    StreamPlan {
        id: StreamRuntimeId(function.name().to_owned()),
        item_ty,
        error_ty,
        ops: lower_stream_stmt_list(function.statements()),
    }
}

fn lower_stream_stmt_list(statements: &[Stmt]) -> Vec<StreamOp> {
    statements.iter().flat_map(lower_stream_stmt).collect()
}

fn lower_stream_stmt(stmt: &Stmt) -> Vec<StreamOp> {
    match stmt {
        Stmt::Let { pattern, expr, .. } => vec![StreamOp::Let {
            pattern: lower_runtime_pattern(pattern),
            expr: lower_runtime_expr(expr),
        }],
        Stmt::For {
            pattern,
            source,
            body,
        } => vec![StreamOp::ForNext {
            pattern: lower_runtime_pattern(pattern),
            source: lower_runtime_expr(source),
            body: lower_stream_stmt_list(body),
        }],
        Stmt::Yield(expr) => vec![StreamOp::Yield {
            expr: lower_runtime_expr(expr),
        }],
        Stmt::If { condition, body } => vec![StreamOp::If {
            condition: lower_runtime_expr(condition),
            then_ops: lower_stream_stmt_list(body),
            else_ops: Vec::new(),
        }],
        Stmt::Match { expr, arms } => vec![StreamOp::Match {
            scrutinee: lower_runtime_expr(expr),
            arms: arms
                .iter()
                .map(|arm| StreamMatchArm {
                    pattern: lower_runtime_pattern(arm.pattern()),
                    guard: arm.guard().map(lower_runtime_expr),
                    ops: lower_stream_stmt_list(arm.body()),
                })
                .collect(),
        }],
        Stmt::Close(expr) => vec![StreamOp::Close {
            source: lower_runtime_expr(expr),
        }],
        Stmt::Return(_) => vec![StreamOp::Return],
        _ => vec![StreamOp::Noop],
    }
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
