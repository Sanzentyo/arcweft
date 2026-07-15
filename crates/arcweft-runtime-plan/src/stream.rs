//! Stream-function lowering into core stream runtime data.

use crate::errors::RuntimePlanLowerError;
use crate::expr::{
    RuntimePureHelperLookup, lower_runtime_expr_strict_with_expected_type,
    lower_runtime_expr_strict_with_pure,
};
use crate::lowering_context::ExecutableLoweringLocation;
use crate::pattern::lower_runtime_pattern_checked;
use arcweft_core::stream::{StreamMatchArm, StreamOp, StreamPlan, StreamRuntimeId};
use arcweft_core::value::{RuntimeExpr, RuntimeValue};
use arcweft_lang_hir::syntax::{
    ast::{common::TextRange, flow::Stmt},
    types::TypeRef,
};

/// Lowers a HIR stream function into a Sans I/O stream plan.
pub(crate) fn lower_stream_function(
    module: &arcweft_lang_hir::model::HirModule,
    function: &arcweft_lang_hir::model::HirFunction,
    pure_helpers: RuntimePureHelperLookup<'_, '_, '_>,
) -> Result<StreamPlan, Vec<RuntimePlanLowerError>> {
    let owner = format!("stream function `{}`", function.name());
    let Some((item_ty, error_ty)) = function
        .signature()
        .return_type()
        .and_then(stream_type_labels)
    else {
        return Err(vec![RuntimePlanLowerError::new(format!(
            "{owner} requires `Stream<T, E>` return type"
        ))]);
    };
    let location = ExecutableLoweringLocation::in_module(owner, module, function.module_path());
    let mut errors = Vec::new();
    let ops = match lower_stream_stmt_list(function.statements(), pure_helpers, &location) {
        Ok(ops) => ops,
        Err(statement_errors) => {
            errors.extend(statement_errors);
            Vec::new()
        }
    };
    if let Some(value) = function.value() {
        errors.push(location.child("body.value").named_expression_error(
            "body",
            "final value",
            value.range(),
            "stream function body cannot end with a value expression; use an explicit stream statement",
        ));
    }
    if errors.is_empty() {
        Ok(StreamPlan {
            id: StreamRuntimeId::canonical(function.name())
                .expect("stream function names are valid canonical runtime IDs"),
            item_ty,
            error_ty,
            ops,
        })
    } else {
        Err(errors)
    }
}

fn lower_stream_stmt_list(
    statements: &[Stmt],
    pure_helpers: RuntimePureHelperLookup<'_, '_, '_>,
    location: &ExecutableLoweringLocation<'_>,
) -> Result<Vec<StreamOp>, Vec<RuntimePlanLowerError>> {
    let mut ops = Vec::new();
    let mut errors = Vec::new();
    for (index, statement) in statements.iter().enumerate() {
        match lower_stream_stmt(statement, pure_helpers, &location.statement(index)) {
            Ok(op) => ops.push(op),
            Err(mut statement_errors) => errors.append(&mut statement_errors),
        }
    }
    if errors.is_empty() {
        Ok(ops)
    } else {
        Err(errors)
    }
}

fn lower_stream_stmt(
    stmt: &Stmt,
    pure_helpers: RuntimePureHelperLookup<'_, '_, '_>,
    location: &ExecutableLoweringLocation<'_>,
) -> Result<StreamOp, Vec<RuntimePlanLowerError>> {
    match stmt {
        Stmt::Let { .. } => lower_stream_let(stmt, pure_helpers, location),
        Stmt::For { .. } => lower_stream_for(stmt, pure_helpers, location),
        Stmt::Yield(expr) => Ok(StreamOp::Yield {
            expr: lower_stream_expr(
                stmt,
                "value",
                expr.expr(),
                expr.range(),
                pure_helpers,
                location,
            )?,
        }),
        Stmt::If { .. } => lower_stream_if(stmt, pure_helpers, location),
        Stmt::Match { .. } => lower_stream_match(stmt, pure_helpers, location),
        Stmt::Close(expr) => Ok(StreamOp::Close {
            source: lower_stream_expr(
                stmt,
                "source",
                expr.expr(),
                expr.range(),
                pure_helpers,
                location,
            )?,
        }),
        Stmt::Return {
            expr, expr_range, ..
        } => {
            let value =
                lower_stream_expr(stmt, "value", expr, *expr_range, pure_helpers, location)?;
            if value != RuntimeExpr::Value(RuntimeValue::Unit) {
                return Err(vec![location.expression_error(
                    stmt,
                    "value",
                    *expr_range,
                    "stream `return` cannot discard a value; use `return ()`",
                )]);
            }
            Ok(StreamOp::Return)
        }
        Stmt::Assertion(_)
        | Stmt::LetElse { .. }
        | Stmt::LetScope { .. }
        | Stmt::LetLoop { .. }
        | Stmt::LetAwait { .. }
        | Stmt::LetActionReceive { .. }
        | Stmt::LetChoice { .. }
        | Stmt::Expr { .. }
        | Stmt::Out { .. }
        | Stmt::Defer { .. }
        | Stmt::Goto(_)
        | Stmt::Select(_)
        | Stmt::Break { .. }
        | Stmt::Continue { .. }
        | Stmt::Assign { .. }
        | Stmt::Signal { .. }
        | Stmt::LifetimeSet { .. }
        | Stmt::Wait(_)
        | Stmt::Thread(_)
        | Stmt::DeferBlock { .. }
        | Stmt::On { .. }
        | Stmt::Loop { .. }
        | Stmt::UnsafeLifetime { .. }
        | Stmt::While { .. }
        | Stmt::WhileLet { .. }
        | Stmt::Raw(_) => Err(vec![location.unsupported_statement(stmt)]),
    }
}

fn lower_stream_let(
    statement: &Stmt,
    pure_helpers: RuntimePureHelperLookup<'_, '_, '_>,
    location: &ExecutableLoweringLocation<'_>,
) -> Result<StreamOp, Vec<RuntimePlanLowerError>> {
    let Stmt::Let {
        pattern,
        ty,
        expr,
        expr_range,
        ..
    } = statement
    else {
        unreachable!("stream let lowering requires a let statement");
    };
    Ok(StreamOp::Let {
        pattern: lower_runtime_pattern_checked(pattern)
            .map_err(|reason| vec![location.pattern_error(statement, "binding", reason)])?,
        expr: lower_stream_expr_with_expected_type(
            statement,
            "initializer",
            expr,
            ty.as_ref(),
            *expr_range,
            pure_helpers,
            location,
        )?,
    })
}

fn lower_stream_for(
    statement: &Stmt,
    pure_helpers: RuntimePureHelperLookup<'_, '_, '_>,
    location: &ExecutableLoweringLocation<'_>,
) -> Result<StreamOp, Vec<RuntimePlanLowerError>> {
    let Stmt::For {
        pattern,
        source,
        body,
    } = statement
    else {
        unreachable!("stream for lowering requires a for statement");
    };
    Ok(StreamOp::ForNext {
        pattern: lower_runtime_pattern_checked(pattern)
            .map_err(|reason| vec![location.pattern_error(statement, "binding", reason)])?,
        source: lower_stream_expr(
            statement,
            "source",
            source.expr(),
            source.range(),
            pure_helpers,
            location,
        )?,
        body: lower_stream_stmt_list(body, pure_helpers, &location.child("body"))?,
    })
}

fn lower_stream_if(
    statement: &Stmt,
    pure_helpers: RuntimePureHelperLookup<'_, '_, '_>,
    location: &ExecutableLoweringLocation<'_>,
) -> Result<StreamOp, Vec<RuntimePlanLowerError>> {
    let Stmt::If {
        condition,
        body,
        else_body,
    } = statement
    else {
        unreachable!("stream if lowering requires an if statement");
    };
    Ok(StreamOp::If {
        condition: lower_stream_expr(
            statement,
            "condition",
            condition.expr(),
            condition.range(),
            pure_helpers,
            location,
        )?,
        then_ops: lower_stream_stmt_list(body, pure_helpers, &location.child("then"))?,
        else_ops: lower_stream_stmt_list(else_body, pure_helpers, &location.child("else"))?,
    })
}

fn lower_stream_match(
    statement: &Stmt,
    pure_helpers: RuntimePureHelperLookup<'_, '_, '_>,
    location: &ExecutableLoweringLocation<'_>,
) -> Result<StreamOp, Vec<RuntimePlanLowerError>> {
    let Stmt::Match { expr, arms } = statement else {
        unreachable!("stream match lowering requires a match statement");
    };
    let scrutinee = lower_stream_expr(
        statement,
        "scrutinee",
        expr.expr(),
        expr.range(),
        pure_helpers,
        location,
    )?;
    let arms = arms
        .iter()
        .enumerate()
        .map(|(index, arm)| {
            let arm_location = location.child(format!("arm.{index}"));
            Ok(StreamMatchArm {
                pattern: lower_runtime_pattern_checked(arm.pattern())
                    .map_err(|reason| vec![arm_location.pattern_error(statement, "arm", reason)])?,
                guard: arm
                    .guard()
                    .map(|guard| {
                        lower_stream_expr(
                            statement,
                            "guard",
                            guard,
                            None,
                            pure_helpers,
                            &arm_location,
                        )
                    })
                    .transpose()?,
                ops: lower_stream_stmt_list(arm.body(), pure_helpers, &arm_location.child("body"))?,
            })
        })
        .collect::<Result<Vec<_>, Vec<RuntimePlanLowerError>>>()?;
    Ok(StreamOp::Match { scrutinee, arms })
}

fn lower_stream_expr(
    statement: &Stmt,
    role: &'static str,
    expr: &arcweft_lang_hir::syntax::expr::Expr,
    source_range: Option<TextRange>,
    pure_helpers: RuntimePureHelperLookup<'_, '_, '_>,
    location: &ExecutableLoweringLocation<'_>,
) -> Result<RuntimeExpr, Vec<RuntimePlanLowerError>> {
    lower_runtime_expr_strict_with_pure(expr, pure_helpers)
        .map_err(|reason| vec![location.expression_error(statement, role, source_range, reason)])
}

fn lower_stream_expr_with_expected_type(
    statement: &Stmt,
    role: &'static str,
    expr: &arcweft_lang_hir::syntax::expr::Expr,
    expected_ty: Option<&TypeRef>,
    source_range: Option<TextRange>,
    pure_helpers: RuntimePureHelperLookup<'_, '_, '_>,
    location: &ExecutableLoweringLocation<'_>,
) -> Result<RuntimeExpr, Vec<RuntimePlanLowerError>> {
    lower_runtime_expr_strict_with_expected_type(expr, expected_ty, pure_helpers)
        .map_err(|reason| vec![location.expression_error(statement, role, source_range, reason)])
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
