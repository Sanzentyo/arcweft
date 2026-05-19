//! Runtime expression and effect-call lowering.

use crate::labels::{duration_expr, expr_label, literal_label, named_arg_label, named_arg_value};
use crate::pattern::lower_runtime_pattern;
use arcweft_core::effect::{
    LineEffectRequest, RuntimeAssignment, RuntimeCall, RuntimeEvent, RuntimeField, RuntimeLog,
};
use arcweft_core::value::{
    RuntimeBinaryOp, RuntimeExpr, RuntimeExprMatchArm, RuntimeFieldExpr, RuntimeUnaryOp,
    RuntimeValue,
};
use arcweft_lang_hir::syntax::{
    ast::pattern::Pattern,
    expr::{BinaryOp, Expr, Literal, MatchExprArm, UnaryOp},
};

/// Lowers an expression into a runtime value expression, preserving a lossy
/// string label for adapter-facing values that are not executable by the core.
pub(crate) fn lower_runtime_expr(expr: &Expr) -> RuntimeExpr {
    match expr {
        Expr::Literal(literal) => RuntimeExpr::Value(lower_runtime_literal(literal)),
        Expr::EntityRef(entity) => RuntimeExpr::EntityRef(entity.body().to_owned()),
        Expr::Path(path) => RuntimeExpr::Local(path.clone()),
        Expr::Tuple(items) => RuntimeExpr::Tuple(items.iter().map(lower_runtime_expr).collect()),
        Expr::BracketSeq(items) => {
            RuntimeExpr::BracketSeq(items.iter().map(lower_runtime_expr).collect())
        }
        Expr::ArrayRepeat { value, len } => lower_runtime_array_repeat(value, len),
        Expr::Record { fields, .. } | Expr::RecordLiteral(fields) => RuntimeExpr::Record(
            fields
                .iter()
                .map(|(name, value)| RuntimeFieldExpr {
                    name: name.clone(),
                    value: lower_runtime_expr(value),
                })
                .collect(),
        ),
        Expr::Field { target, field } => RuntimeExpr::Field {
            target: Box::new(lower_runtime_expr(target)),
            field: field.clone(),
        },
        Expr::Unary { op, expr } => RuntimeExpr::Unary {
            op: lower_runtime_unary_op(*op),
            expr: Box::new(lower_runtime_expr(expr)),
        },
        Expr::Binary { lhs, op, rhs } => {
            if let Some(op) = lower_runtime_binary_op(*op) {
                RuntimeExpr::Binary {
                    lhs: Box::new(lower_runtime_expr(lhs)),
                    op,
                    rhs: Box::new(lower_runtime_expr(rhs)),
                }
            } else {
                RuntimeExpr::Value(RuntimeValue::String(expr_label(expr)))
            }
        }
        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => RuntimeExpr::If {
            condition: Box::new(lower_runtime_expr(condition)),
            then_expr: Box::new(lower_runtime_expr(then_branch)),
            else_expr: Box::new(
                else_branch
                    .as_deref()
                    .map_or(RuntimeExpr::Value(RuntimeValue::Unit), lower_runtime_expr),
            ),
        },
        Expr::Match { scrutinee, arms } => RuntimeExpr::Match {
            scrutinee: Box::new(lower_runtime_expr(scrutinee)),
            arms: arms
                .iter()
                .map(|arm| RuntimeExprMatchArm {
                    pattern: lower_runtime_pattern(arm.pattern()),
                    guard: arm.guard().map(lower_runtime_expr),
                    value: lower_runtime_expr(arm.value()),
                })
                .collect(),
        },
        Expr::Call { .. } | Expr::MethodCall { .. } => {
            RuntimeExpr::Value(RuntimeValue::String(expr_label(expr)))
        }
        Expr::NamedArg { value, .. } => lower_runtime_expr(value),
        Expr::Try { expr }
        | Expr::Await { expr, .. }
        | Expr::Index { target: expr, .. }
        | Expr::Pipe { lhs: expr, .. } => lower_runtime_expr(expr),
        _ => RuntimeExpr::Value(RuntimeValue::String(expr_label(expr))),
    }
}

/// Strict expression lowering for executable flow/runtime positions.
pub(crate) fn lower_runtime_expr_strict(expr: &Expr) -> Result<RuntimeExpr, String> {
    match expr {
        Expr::Literal(literal) => Ok(RuntimeExpr::Value(lower_runtime_literal(literal))),
        Expr::EntityRef(entity) => Ok(RuntimeExpr::EntityRef(entity.body().to_owned())),
        Expr::Path(path) => Ok(constructor_path(path).map_or_else(
            || RuntimeExpr::Local(path.clone()),
            |(path, name)| RuntimeExpr::Variant {
                path,
                name,
                payload: None,
            },
        )),
        Expr::Tuple(items) => items
            .iter()
            .map(lower_runtime_expr_strict)
            .collect::<Result<Vec<_>, _>>()
            .map(RuntimeExpr::Tuple),
        Expr::BracketSeq(items) => items
            .iter()
            .map(lower_runtime_expr_strict)
            .collect::<Result<Vec<_>, _>>()
            .map(RuntimeExpr::BracketSeq),
        Expr::ArrayRepeat { value, len } => lower_runtime_array_repeat_strict(value, len),
        Expr::Record { fields, .. } | Expr::RecordLiteral(fields) => fields
            .iter()
            .map(|(name, value)| {
                Ok(RuntimeFieldExpr {
                    name: name.clone(),
                    value: lower_runtime_expr_strict(value)?,
                })
            })
            .collect::<Result<Vec<_>, String>>()
            .map(RuntimeExpr::Record),
        Expr::Field { target, field } => Ok(RuntimeExpr::Field {
            target: Box::new(lower_runtime_expr_strict(target)?),
            field: field.clone(),
        }),
        Expr::Unary { op, expr } => Ok(RuntimeExpr::Unary {
            op: lower_runtime_unary_op(*op),
            expr: Box::new(lower_runtime_expr_strict(expr)?),
        }),
        Expr::Binary { lhs, op, rhs } => {
            let Some(op) = lower_runtime_binary_op(*op) else {
                return Err(format!(
                    "unsupported runtime binary expression `{}`",
                    expr_label(expr)
                ));
            };
            Ok(RuntimeExpr::Binary {
                lhs: Box::new(lower_runtime_expr_strict(lhs)?),
                op,
                rhs: Box::new(lower_runtime_expr_strict(rhs)?),
            })
        }
        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => lower_strict_if_expr(condition, then_branch, else_branch.as_deref()),
        Expr::IfLet {
            pattern,
            expr,
            guard,
            then_branch,
            else_branch,
        } => lower_strict_if_let_expr(
            pattern,
            expr,
            guard.as_deref(),
            then_branch,
            else_branch.as_deref(),
        ),
        Expr::Match { scrutinee, arms } => lower_strict_match_expr(scrutinee, arms),
        Expr::NamedArg { value, .. } => lower_runtime_expr_strict(value),
        Expr::Block { value, .. }
        | Expr::ComputationBlock { value, .. }
        | Expr::MemoBlock { value, .. }
        | Expr::NamedBlock { value, .. } => lower_strict_block_value(value.as_deref()),
        Expr::Call { callee, args } => lower_constructor_call(callee, args).ok_or_else(|| {
            format!(
                "unsupported runtime value expression `{}`",
                expr_label(expr)
            )
        }),
        Expr::MethodCall { .. }
        | Expr::DialogueCall { .. }
        | Expr::Index { .. }
        | Expr::Pipe { .. }
        | Expr::Try { .. }
        | Expr::Await { .. }
        | Expr::Thread { .. }
        | Expr::Range { .. }
        | Expr::Closure { .. }
        | Expr::LifetimePath { .. }
        | Expr::Placeholder(_)
        | Expr::Raw(_) => Err(format!(
            "unsupported runtime value expression `{}`",
            expr_label(expr)
        )),
    }
}

fn lower_strict_block_value(value: Option<&Expr>) -> Result<RuntimeExpr, String> {
    value.map_or_else(
        || Ok(RuntimeExpr::Value(RuntimeValue::Unit)),
        lower_runtime_expr_strict,
    )
}

fn lower_strict_if_expr(
    condition: &Expr,
    then_branch: &Expr,
    else_branch: Option<&Expr>,
) -> Result<RuntimeExpr, String> {
    Ok(RuntimeExpr::If {
        condition: Box::new(lower_runtime_expr_strict(condition)?),
        then_expr: Box::new(lower_runtime_expr_strict(then_branch)?),
        else_expr: Box::new(else_branch.map_or(
            Ok(RuntimeExpr::Value(RuntimeValue::Unit)),
            lower_runtime_expr_strict,
        )?),
    })
}

fn lower_strict_if_let_expr(
    pattern: &Pattern,
    expr: &Expr,
    guard: Option<&Expr>,
    then_branch: &Expr,
    else_branch: Option<&Expr>,
) -> Result<RuntimeExpr, String> {
    Ok(RuntimeExpr::IfLet {
        pattern: lower_runtime_pattern(pattern),
        expr: Box::new(lower_runtime_expr_strict(expr)?),
        guard: guard
            .map(lower_runtime_expr_strict)
            .transpose()?
            .map(Box::new),
        then_expr: Box::new(lower_runtime_expr_strict(then_branch)?),
        else_expr: Box::new(else_branch.map_or(
            Ok(RuntimeExpr::Value(RuntimeValue::Unit)),
            lower_runtime_expr_strict,
        )?),
    })
}

fn lower_strict_match_expr(scrutinee: &Expr, arms: &[MatchExprArm]) -> Result<RuntimeExpr, String> {
    Ok(RuntimeExpr::Match {
        scrutinee: Box::new(lower_runtime_expr_strict(scrutinee)?),
        arms: arms
            .iter()
            .map(|arm| {
                Ok(RuntimeExprMatchArm {
                    pattern: lower_runtime_pattern(arm.pattern()),
                    guard: arm.guard().map(lower_runtime_expr_strict).transpose()?,
                    value: lower_runtime_expr_strict(arm.value())?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?,
    })
}

fn lower_constructor_call(callee: &Expr, args: &[Expr]) -> Option<RuntimeExpr> {
    let Expr::Path(callee) = callee else {
        return None;
    };
    let (path, name) = constructor_path(callee)?;
    if args.len() > 1 {
        return None;
    }
    let payload = args
        .first()
        .map(lower_runtime_expr_strict)
        .transpose()
        .ok()?
        .map(Box::new);
    Some(RuntimeExpr::Variant {
        path,
        name,
        payload,
    })
}

fn constructor_path(path: &str) -> Option<(Option<String>, String)> {
    let (prefix, name) = path
        .rsplit_once("::")
        .map_or((None, path), |(prefix, name)| {
            (Some(prefix.to_owned()), name)
        });
    let is_known_std_variant = matches!(name, "Ok" | "Err" | "Some" | "None");
    let is_uppercase_variant = name
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_uppercase());
    (is_known_std_variant || is_uppercase_variant).then(|| (prefix, name.to_owned()))
}

fn lower_runtime_literal(literal: &Literal) -> RuntimeValue {
    match literal {
        Literal::String(value) => RuntimeValue::String(value.clone()),
        Literal::Char { value, .. } => RuntimeValue::Char(*value),
        Literal::Int(value) => RuntimeValue::Int(*value),
        Literal::Float(value) => RuntimeValue::Float(value.clone()),
        Literal::Bool(value) => RuntimeValue::Bool(*value),
        Literal::Duration { .. } => duration_expr(&Expr::Literal(literal.clone())).map_or_else(
            || RuntimeValue::String(literal_label(literal)),
            RuntimeValue::Duration,
        ),
    }
}

fn lower_runtime_unary_op(op: UnaryOp) -> RuntimeUnaryOp {
    match op {
        UnaryOp::Not => RuntimeUnaryOp::Not,
        UnaryOp::Neg => RuntimeUnaryOp::Neg,
    }
}

fn lower_runtime_binary_op(op: BinaryOp) -> Option<RuntimeBinaryOp> {
    Some(match op {
        BinaryOp::Eq => RuntimeBinaryOp::Eq,
        BinaryOp::NotEq => RuntimeBinaryOp::Ne,
        BinaryOp::Lt => RuntimeBinaryOp::Lt,
        BinaryOp::Lte => RuntimeBinaryOp::Le,
        BinaryOp::Gt => RuntimeBinaryOp::Gt,
        BinaryOp::Gte => RuntimeBinaryOp::Ge,
        BinaryOp::Add => RuntimeBinaryOp::Add,
        BinaryOp::Sub => RuntimeBinaryOp::Sub,
        BinaryOp::Mul => RuntimeBinaryOp::Mul,
        BinaryOp::Div => RuntimeBinaryOp::Div,
        BinaryOp::And => RuntimeBinaryOp::And,
        BinaryOp::Or => RuntimeBinaryOp::Or,
        BinaryOp::Implies | BinaryOp::In | BinaryOp::Merge | BinaryOp::Rem => return None,
    })
}

fn runtime_call(expr: &Expr) -> RuntimeCall {
    match expr {
        Expr::Call { callee, args } => RuntimeCall {
            callee: expr_label(callee),
            args: args.iter().map(expr_label).collect(),
        },
        Expr::MethodCall {
            receiver,
            method,
            args,
        } => RuntimeCall {
            callee: format!("{}.{}", expr_label(receiver), method),
            args: args.iter().map(expr_label).collect(),
        },
        Expr::Path(path) => RuntimeCall {
            callee: path.clone(),
            args: Vec::new(),
        },
        other => RuntimeCall {
            callee: expr_label(other),
            args: Vec::new(),
        },
    }
}

/// Lowers ordinary call syntax into the canonical runtime effect request when
/// the callee names a built-in effect namespace such as `log.info`.
pub(crate) fn runtime_call_effect(expr: &Expr) -> LineEffectRequest {
    let call = runtime_call(expr);
    if let Some(log) = runtime_log_call(&call) {
        return LineEffectRequest::Log(log);
    }
    if let Some(write) = runtime_assignment_call(&call, "signal.set") {
        return LineEffectRequest::SignalWrite(write);
    }
    if let Some(write) = runtime_assignment_call(&call, "metric.set") {
        return LineEffectRequest::MetricWrite(write);
    }
    if let Some(event) = runtime_event_call(&call) {
        return LineEffectRequest::EmitEvent(event);
    }
    LineEffectRequest::Call(call)
}

fn runtime_log_call(call: &RuntimeCall) -> Option<RuntimeLog> {
    let level = call.callee.strip_prefix("log.")?;
    let (message, rest) = call.args.split_first()?;
    Some(RuntimeLog {
        level: level.to_owned(),
        message: message.trim_matches('"').to_owned(),
        fields: rest
            .iter()
            .enumerate()
            .map(|(idx, value)| RuntimeField {
                name: named_arg_label(value).unwrap_or_else(|| format!("arg{idx}")),
                value: named_arg_value(value).unwrap_or_else(|| (*value).clone()),
            })
            .collect(),
    })
}

fn runtime_assignment_call(call: &RuntimeCall, callee: &str) -> Option<RuntimeAssignment> {
    if call.callee != callee || call.args.len() < 2 {
        return None;
    }
    Some(RuntimeAssignment {
        target: call.args[0].clone(),
        value: call.args[1].clone(),
    })
}

fn runtime_event_call(call: &RuntimeCall) -> Option<RuntimeEvent> {
    if call.callee != "event.emit" {
        return None;
    }
    let (event, rest) = call.args.split_first()?;
    Some(RuntimeEvent {
        event: event.clone(),
        fields: rest
            .iter()
            .enumerate()
            .map(|(idx, value)| RuntimeField {
                name: named_arg_label(value).unwrap_or_else(|| format!("arg{idx}")),
                value: named_arg_value(value).unwrap_or_else(|| (*value).clone()),
            })
            .collect(),
    })
}

fn lower_runtime_array_repeat(value: &Expr, len: &Expr) -> RuntimeExpr {
    let Some(len) = array_repeat_len(len) else {
        return RuntimeExpr::Value(RuntimeValue::String(expr_label(&Expr::ArrayRepeat {
            value: Box::new(value.clone()),
            len: Box::new(len.clone()),
        })));
    };
    RuntimeExpr::BracketSeq((0..len).map(|_| lower_runtime_expr(value)).collect())
}

fn lower_runtime_array_repeat_strict(value: &Expr, len: &Expr) -> Result<RuntimeExpr, String> {
    let Some(len) = array_repeat_len(len) else {
        return Err(format!(
            "array repeat length must be an integer constant in `{}`",
            expr_label(len)
        ));
    };
    (0..len)
        .map(|_| lower_runtime_expr_strict(value))
        .collect::<Result<Vec<_>, _>>()
        .map(RuntimeExpr::BracketSeq)
}

fn array_repeat_len(expr: &Expr) -> Option<usize> {
    match expr {
        Expr::Literal(Literal::Int(value)) => usize::try_from(*value).ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_runtime_value_lowering_rejects_calls() {
        let expr = Expr::Call {
            callee: Box::new(Expr::Path("compute".to_owned())),
            args: Vec::new(),
        };

        let error =
            lower_runtime_expr_strict(&expr).expect_err("calls are not headless values yet");

        assert!(error.contains("unsupported runtime value expression"));
        assert!(error.contains("compute()"));
    }
}
