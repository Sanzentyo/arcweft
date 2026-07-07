//! Runtime expression and effect-call lowering.

use crate::labels::{
    call_arg_label, duration_expr, entity_ref_label, expr_label, literal_label, named_arg_label,
    named_arg_value,
};
use crate::pattern::lower_runtime_pattern;
use arcweft_core::effect::{
    LineEffectRequest, RuntimeAssertion, RuntimeAssertionProfile, RuntimeAssignment, RuntimeCall,
    RuntimeEvent, RuntimeField, RuntimeLog,
};
use arcweft_core::plan::RuntimePureHelperId;
use arcweft_core::value::{
    RuntimeBinaryOp, RuntimeCallTarget, RuntimeExpr, RuntimeExprMatchArm, RuntimeFieldExpr,
    RuntimeUnaryOp, RuntimeValue, runtime_sequence_dense_i8, runtime_sequence_dense_i16,
    runtime_sequence_dense_i32, runtime_sequence_dense_i64, runtime_sequence_dense_i128,
    runtime_sequence_dense_isize, runtime_sequence_dense_u8, runtime_sequence_dense_u16,
    runtime_sequence_dense_u32, runtime_sequence_dense_u64, runtime_sequence_dense_u128,
    runtime_sequence_dense_usize, runtime_sequence_from_literal_values,
};
use arcweft_lang_hir::syntax::{
    ast::{flow::Stmt, line_plan::LinePlanItem, pattern::Pattern},
    expr::{BinaryOp, CallArg, Expr, FloatSuffix, Literal, MatchExprArm, Placeholder, UnaryOp},
};
use std::collections::BTreeMap;

/// Lowers an expression into a runtime value expression, preserving a lossy
/// string label for adapter-facing values that are not executable by the core.
pub(crate) fn lower_runtime_expr(expr: &Expr) -> RuntimeExpr {
    match expr {
        Expr::Literal(literal) => RuntimeExpr::Value(lower_runtime_literal(literal)),
        Expr::EntityRef(entity) => RuntimeExpr::EntityRef(entity_ref_label(entity)),
        Expr::Path(path) => RuntimeExpr::Local(path.as_label().to_owned()),
        Expr::ShortVariant(name) => RuntimeExpr::Value(RuntimeValue::String(format!(".{name}"))),
        Expr::Tuple(items) if items.is_empty() => RuntimeExpr::Value(RuntimeValue::Unit),
        Expr::Tuple(items) => RuntimeExpr::Tuple(items.iter().map(lower_runtime_expr).collect()),
        Expr::BracketSeq(items) => lower_runtime_bracket_seq(items),
        Expr::NumericBracketSeq(seq) => lower_runtime_numeric_bracket_seq(seq),
        Expr::ArrayRepeat { value, len } => lower_runtime_array_repeat(value, len),
        Expr::Range {
            start,
            end,
            inclusive,
        } => lower_runtime_range_expr_lossy(start.as_deref(), end.as_deref(), *inclusive),
        Expr::Record { fields, .. } | Expr::RecordLiteral(fields) => RuntimeExpr::Record(
            fields
                .iter()
                .map(|(name, value)| RuntimeFieldExpr {
                    name: name.clone(),
                    value: lower_runtime_expr(value),
                })
                .collect(),
        ),
        Expr::Field { target, field } => {
            lower_enum_variant_field(target, field).unwrap_or_else(|| {
                lower_std_float_constant(expr).map_or_else(
                    || lower_runtime_field_expr(target, field),
                    RuntimeExpr::Value,
                )
            })
        }
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
        Expr::Call { callee, args } => {
            lower_choice_action_call(callee, args).unwrap_or_else(|| RuntimeExpr::Call {
                callee: RuntimeCallTarget::from_label(expr_label(callee)),
                args: args.iter().map(lower_runtime_call_arg).collect(),
            })
        }
        Expr::MethodCall {
            receiver,
            method,
            args,
        } => lower_runtime_math_method_call(receiver, method, args)
            .or_else(|| lower_runtime_std_float_method_call(receiver, method, args))
            .or_else(|| lower_runtime_path_method_call(receiver, method, args))
            .or_else(|| lower_runtime_external_namespace_method_call(receiver, method, args))
            .unwrap_or_else(|| RuntimeExpr::MethodCall {
                receiver: Box::new(lower_runtime_expr(receiver)),
                method: runtime_method_name(method).to_owned(),
                args: args.iter().map(lower_runtime_call_arg).collect(),
            }),
        Expr::Index { target, index } => {
            lower_runtime_index_expr(target, index).unwrap_or_else(|| lower_runtime_expr(target))
        }
        Expr::Try { expr } | Expr::Await { expr, .. } => lower_runtime_expr(expr),
        Expr::Pipe { lhs, rhs } => lower_runtime_pipe_expr(lhs, rhs),
        _ => RuntimeExpr::Value(RuntimeValue::String(expr_label(expr))),
    }
}

/// Strict expression lowering for executable flow/runtime positions.
pub(crate) fn lower_runtime_expr_strict(expr: &Expr) -> Result<RuntimeExpr, String> {
    lower_runtime_expr_strict_with_helpers(expr, None)
}

pub(crate) fn lower_runtime_expr_strict_with_pure(
    expr: &Expr,
    helpers: &BTreeMap<String, RuntimePureHelperId>,
) -> Result<RuntimeExpr, String> {
    lower_runtime_expr_strict_with_helpers(expr, Some(helpers))
}

fn lower_runtime_expr_strict_with_helpers(
    expr: &Expr,
    helpers: Option<&BTreeMap<String, RuntimePureHelperId>>,
) -> Result<RuntimeExpr, String> {
    match expr {
        Expr::Literal(literal) => Ok(RuntimeExpr::Value(lower_runtime_literal(literal))),
        Expr::EntityRef(entity) => Ok(RuntimeExpr::EntityRef(entity_ref_label(entity))),
        Expr::Path(path) => Ok(constructor_path(path.as_label()).map_or_else(
            || RuntimeExpr::Local(path.as_label().to_owned()),
            |(path, name)| RuntimeExpr::Variant {
                path,
                name,
                payload: None,
            },
        )),
        Expr::ShortVariant(name) => Ok(RuntimeExpr::Variant {
            path: None,
            name: name.to_string(),
            payload: None,
        }),
        Expr::Tuple(items) if items.is_empty() => Ok(RuntimeExpr::Value(RuntimeValue::Unit)),
        Expr::Tuple(items) => items
            .iter()
            .map(|item| lower_runtime_expr_strict_with_helpers(item, helpers))
            .collect::<Result<Vec<_>, _>>()
            .map(RuntimeExpr::Tuple),
        Expr::BracketSeq(items) => lower_runtime_bracket_seq_strict(items, helpers),
        Expr::NumericBracketSeq(seq) => Ok(lower_runtime_numeric_bracket_seq(seq)),
        Expr::ArrayRepeat { value, len } => lower_runtime_array_repeat_strict(value, len, helpers),
        Expr::Range {
            start,
            end,
            inclusive,
        } => lower_runtime_range_expr(start.as_deref(), end.as_deref(), *inclusive, helpers),
        Expr::Record { fields, .. } | Expr::RecordLiteral(fields) => {
            lower_runtime_record_expr_strict(fields, helpers)
        }
        Expr::Field { target, field } => {
            lower_strict_field_or_constant(expr, target, field, helpers)
        }
        Expr::Unary { op, expr } => Ok(RuntimeExpr::Unary {
            op: lower_runtime_unary_op(*op),
            expr: Box::new(lower_runtime_expr_strict_with_helpers(expr, helpers)?),
        }),
        Expr::Binary { lhs, op, rhs } => lower_strict_binary_expr(expr, lhs, *op, rhs, helpers),
        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => lower_strict_if_expr(condition, then_branch, else_branch.as_deref(), helpers),
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
            helpers,
        ),
        Expr::Match { scrutinee, arms } => lower_strict_match_expr(scrutinee, arms, helpers),
        Expr::Block { statements, value }
        | Expr::ComputationBlock {
            statements, value, ..
        }
        | Expr::MemoBlock {
            statements, value, ..
        }
        | Expr::NamedBlock {
            statements, value, ..
        } => lower_strict_block_expr(statements, value.as_deref(), helpers),
        Expr::Call { callee, args } => lower_strict_call_expr(callee, args, helpers),
        Expr::MethodCall {
            receiver,
            method,
            args,
        } => lower_strict_method_call_dispatch(receiver, method, args, helpers),
        Expr::DialogueCall { plan, .. } => Ok(lower_dialogue_call_value(plan.as_ref())),
        Expr::Index { target, index } => lower_strict_index_expr(target, index, helpers),
        Expr::Try { expr } | Expr::Await { expr, .. } => {
            lower_runtime_expr_strict_with_helpers(expr, helpers)
        }
        Expr::Pipe { lhs, rhs } => lower_runtime_pipe_expr_strict(lhs, rhs, helpers),
        Expr::Thread { .. }
        | Expr::Closure { .. }
        | Expr::LifetimePath { .. }
        | Expr::Placeholder(_)
        | Expr::Raw(_) => unsupported_strict_runtime_expr(expr),
    }
}

fn lower_strict_binary_expr(
    source: &Expr,
    lhs: &Expr,
    op: BinaryOp,
    rhs: &Expr,
    helpers: Option<&BTreeMap<String, RuntimePureHelperId>>,
) -> Result<RuntimeExpr, String> {
    let Some(op) = lower_runtime_binary_op(op) else {
        return Err(format!(
            "unsupported runtime binary expression `{}`",
            expr_label(source)
        ));
    };
    Ok(RuntimeExpr::Binary {
        lhs: Box::new(lower_runtime_expr_strict_with_helpers(lhs, helpers)?),
        op,
        rhs: Box::new(lower_runtime_expr_strict_with_helpers(rhs, helpers)?),
    })
}

fn lower_strict_method_call_dispatch(
    receiver: &Expr,
    method: &str,
    args: &[CallArg],
    helpers: Option<&BTreeMap<String, RuntimePureHelperId>>,
) -> Result<RuntimeExpr, String> {
    match lower_strict_math_method_call(receiver, method, args, helpers)
        .or_else(|| lower_strict_std_float_method_call(receiver, method, args, helpers))
        .or_else(|| lower_strict_path_method_call(receiver, method, args, helpers))
        .or_else(|| lower_strict_external_namespace_method_call(receiver, method, args, helpers))
    {
        Some(lowered) => lowered,
        None => lower_strict_method_call_expr(receiver, method, args, helpers),
    }
}

fn lower_runtime_pipe_expr(lhs: &Expr, rhs: &Expr) -> RuntimeExpr {
    if expr_contains_pipe_left(rhs) {
        return lower_runtime_expr(&substitute_pipe_left(rhs, lhs));
    }
    lower_runtime_data_last_pipe(lhs, rhs)
}

fn lower_runtime_pipe_expr_strict(
    lhs: &Expr,
    rhs: &Expr,
    helpers: Option<&BTreeMap<String, RuntimePureHelperId>>,
) -> Result<RuntimeExpr, String> {
    if expr_contains_pipe_left(rhs) {
        return lower_runtime_expr_strict_with_helpers(&substitute_pipe_left(rhs, lhs), helpers);
    }
    lower_runtime_data_last_pipe_strict(lhs, rhs, helpers)
}

fn lower_runtime_data_last_pipe(lhs: &Expr, rhs: &Expr) -> RuntimeExpr {
    if let Some((method, args)) = data_last_collection_method(rhs) {
        return RuntimeExpr::MethodCall {
            receiver: Box::new(lower_runtime_expr(lhs)),
            method: method.to_owned(),
            args: args.iter().map(lower_runtime_call_arg).collect(),
        };
    }
    match rhs {
        Expr::Path(path) => RuntimeExpr::Call {
            callee: RuntimeCallTarget::from_label(path.as_label()),
            args: vec![lower_runtime_expr(lhs)],
        },
        Expr::Call { callee, args } => RuntimeExpr::Call {
            callee: RuntimeCallTarget::from_label(expr_label(callee)),
            args: args
                .iter()
                .map(lower_runtime_call_arg)
                .chain(std::iter::once(lower_runtime_expr(lhs)))
                .collect(),
        },
        _ => RuntimeExpr::Call {
            callee: RuntimeCallTarget::from_label(expr_label(rhs)),
            args: vec![lower_runtime_expr(lhs)],
        },
    }
}

fn lower_runtime_data_last_pipe_strict(
    lhs: &Expr,
    rhs: &Expr,
    helpers: Option<&BTreeMap<String, RuntimePureHelperId>>,
) -> Result<RuntimeExpr, String> {
    if let Some((method, args)) = data_last_collection_method(rhs) {
        return lower_strict_method_call_expr(lhs, method, args, helpers);
    }
    let lhs = lower_runtime_expr_strict_with_helpers(lhs, helpers)?;
    match rhs {
        Expr::Path(path) => Ok(RuntimeExpr::Call {
            callee: RuntimeCallTarget::from_label(path.as_label()),
            args: vec![lhs],
        }),
        Expr::Call { callee, args } => {
            let mut args = args
                .iter()
                .map(|arg| lower_strict_call_arg(arg, helpers))
                .collect::<Result<Vec<_>, _>>()?;
            args.push(lhs);
            Ok(RuntimeExpr::Call {
                callee: RuntimeCallTarget::from_label(expr_label(callee)),
                args,
            })
        }
        _ => Ok(RuntimeExpr::Call {
            callee: RuntimeCallTarget::from_label(expr_label(rhs)),
            args: vec![lhs],
        }),
    }
}

fn data_last_collection_method(rhs: &Expr) -> Option<(&str, &[CallArg])> {
    let Expr::Call { callee, args } = rhs else {
        return None;
    };
    let Expr::Path(path) = callee.as_ref() else {
        return None;
    };
    let method = path.as_label();
    matches!(method, "map" | "filter").then_some((method, args.as_slice()))
}

fn substitute_pipe_left(expr: &Expr, lhs: &Expr) -> Expr {
    match expr {
        Expr::Placeholder(Placeholder::PipeLeft) => lhs.clone(),
        Expr::Tuple(items) => Expr::Tuple(
            items
                .iter()
                .map(|item| substitute_pipe_left(item, lhs))
                .collect(),
        ),
        Expr::BracketSeq(items) => Expr::BracketSeq(
            items
                .iter()
                .map(|item| substitute_pipe_left(item, lhs))
                .collect(),
        ),
        Expr::ArrayRepeat { value, len } => Expr::ArrayRepeat {
            value: Box::new(substitute_pipe_left(value, lhs)),
            len: Box::new(substitute_pipe_left(len, lhs)),
        },
        Expr::Call { callee, args } => Expr::Call {
            callee: Box::new(substitute_pipe_left(callee, lhs)),
            args: args
                .iter()
                .map(|arg| substitute_pipe_left_arg(arg, lhs))
                .collect(),
        },
        Expr::MethodCall {
            receiver,
            method,
            args,
        } => Expr::MethodCall {
            receiver: Box::new(substitute_pipe_left(receiver, lhs)),
            method: method.clone(),
            args: args
                .iter()
                .map(|arg| substitute_pipe_left_arg(arg, lhs))
                .collect(),
        },
        Expr::Field { target, field } => Expr::Field {
            target: Box::new(substitute_pipe_left(target, lhs)),
            field: field.clone(),
        },
        Expr::Index { target, index } => Expr::Index {
            target: Box::new(substitute_pipe_left(target, lhs)),
            index: Box::new(substitute_pipe_left(index, lhs)),
        },
        Expr::Unary { op, expr } => Expr::Unary {
            op: *op,
            expr: Box::new(substitute_pipe_left(expr, lhs)),
        },
        Expr::Binary { lhs: left, op, rhs } => Expr::Binary {
            lhs: Box::new(substitute_pipe_left(left, lhs)),
            op: *op,
            rhs: Box::new(substitute_pipe_left(rhs, lhs)),
        },
        Expr::Record { path, fields } => Expr::Record {
            path: path.clone(),
            fields: fields
                .iter()
                .map(|(name, value)| (name.clone(), substitute_pipe_left(value, lhs)))
                .collect(),
        },
        Expr::RecordLiteral(fields) => Expr::RecordLiteral(
            fields
                .iter()
                .map(|(name, value)| (name.clone(), substitute_pipe_left(value, lhs)))
                .collect(),
        ),
        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => Expr::If {
            condition: Box::new(substitute_pipe_left(condition, lhs)),
            then_branch: Box::new(substitute_pipe_left(then_branch, lhs)),
            else_branch: else_branch
                .as_deref()
                .map(|else_branch| Box::new(substitute_pipe_left(else_branch, lhs))),
        },
        Expr::Try { expr } => Expr::Try {
            expr: Box::new(substitute_pipe_left(expr, lhs)),
        },
        Expr::Await { expr, applies_try } => Expr::Await {
            expr: Box::new(substitute_pipe_left(expr, lhs)),
            applies_try: *applies_try,
        },
        Expr::Closure { params, body } => Expr::Closure {
            params: params.clone(),
            body: Box::new(substitute_pipe_left(body, lhs)),
        },
        _ => expr.clone(),
    }
}

fn substitute_pipe_left_arg(arg: &CallArg, lhs: &Expr) -> CallArg {
    match arg {
        CallArg::Positional(value) => CallArg::Positional(substitute_pipe_left(value, lhs)),
        CallArg::Named { name, value } => CallArg::Named {
            name: name.clone(),
            value: Box::new(substitute_pipe_left(value, lhs)),
        },
        CallArg::Spread { value } => CallArg::Spread {
            value: Box::new(substitute_pipe_left(value, lhs)),
        },
    }
}

fn expr_contains_pipe_left(expr: &Expr) -> bool {
    match expr {
        Expr::Placeholder(Placeholder::PipeLeft) => true,
        Expr::Tuple(items) | Expr::BracketSeq(items) => items.iter().any(expr_contains_pipe_left),
        Expr::ArrayRepeat { value, len }
        | Expr::Binary {
            lhs: value,
            rhs: len,
            ..
        } => expr_contains_pipe_left(value) || expr_contains_pipe_left(len),
        Expr::Call { callee, args } => {
            expr_contains_pipe_left(callee) || args.iter().any(call_arg_contains_pipe_left)
        }
        Expr::MethodCall { receiver, args, .. } => {
            expr_contains_pipe_left(receiver) || args.iter().any(call_arg_contains_pipe_left)
        }
        Expr::Field { target, .. } | Expr::Try { expr: target } => expr_contains_pipe_left(target),
        Expr::Index { target, index } => {
            expr_contains_pipe_left(target) || expr_contains_pipe_left(index)
        }
        Expr::Unary { expr, .. } | Expr::Await { expr, .. } | Expr::Closure { body: expr, .. } => {
            expr_contains_pipe_left(expr)
        }
        Expr::Record { fields, .. } | Expr::RecordLiteral(fields) => fields
            .iter()
            .any(|(_, value)| expr_contains_pipe_left(value)),
        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_contains_pipe_left(condition)
                || expr_contains_pipe_left(then_branch)
                || else_branch.as_deref().is_some_and(expr_contains_pipe_left)
        }
        _ => false,
    }
}

fn call_arg_contains_pipe_left(arg: &CallArg) -> bool {
    match arg {
        CallArg::Positional(value) => expr_contains_pipe_left(value),
        CallArg::Named { value, .. } | CallArg::Spread { value } => expr_contains_pipe_left(value),
    }
}

fn substitute_partial_placeholder(expr: &Expr, param_name: &str) -> Expr {
    match expr {
        Expr::Placeholder(Placeholder::Partial) => Expr::Path(param_name.into()),
        Expr::Tuple(items) => Expr::Tuple(
            items
                .iter()
                .map(|item| substitute_partial_placeholder(item, param_name))
                .collect(),
        ),
        Expr::BracketSeq(items) => Expr::BracketSeq(
            items
                .iter()
                .map(|item| substitute_partial_placeholder(item, param_name))
                .collect(),
        ),
        Expr::ArrayRepeat { value, len } => Expr::ArrayRepeat {
            value: Box::new(substitute_partial_placeholder(value, param_name)),
            len: Box::new(substitute_partial_placeholder(len, param_name)),
        },
        Expr::Call { callee, args } => Expr::Call {
            callee: Box::new(substitute_partial_placeholder(callee, param_name)),
            args: args
                .iter()
                .map(|arg| substitute_partial_placeholder_arg(arg, param_name))
                .collect(),
        },
        Expr::MethodCall {
            receiver,
            method,
            args,
        } => Expr::MethodCall {
            receiver: Box::new(substitute_partial_placeholder(receiver, param_name)),
            method: method.clone(),
            args: args
                .iter()
                .map(|arg| substitute_partial_placeholder_arg(arg, param_name))
                .collect(),
        },
        Expr::Field { target, field } => Expr::Field {
            target: Box::new(substitute_partial_placeholder(target, param_name)),
            field: field.clone(),
        },
        Expr::Index { target, index } => Expr::Index {
            target: Box::new(substitute_partial_placeholder(target, param_name)),
            index: Box::new(substitute_partial_placeholder(index, param_name)),
        },
        Expr::Unary { op, expr } => Expr::Unary {
            op: *op,
            expr: Box::new(substitute_partial_placeholder(expr, param_name)),
        },
        Expr::Binary { lhs, op, rhs } => Expr::Binary {
            lhs: Box::new(substitute_partial_placeholder(lhs, param_name)),
            op: *op,
            rhs: Box::new(substitute_partial_placeholder(rhs, param_name)),
        },
        Expr::Record { path, fields } => Expr::Record {
            path: path.clone(),
            fields: fields
                .iter()
                .map(|(name, value)| {
                    (
                        name.clone(),
                        substitute_partial_placeholder(value, param_name),
                    )
                })
                .collect(),
        },
        Expr::RecordLiteral(fields) => Expr::RecordLiteral(
            fields
                .iter()
                .map(|(name, value)| {
                    (
                        name.clone(),
                        substitute_partial_placeholder(value, param_name),
                    )
                })
                .collect(),
        ),
        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => Expr::If {
            condition: Box::new(substitute_partial_placeholder(condition, param_name)),
            then_branch: Box::new(substitute_partial_placeholder(then_branch, param_name)),
            else_branch: else_branch.as_deref().map(|else_branch| {
                Box::new(substitute_partial_placeholder(else_branch, param_name))
            }),
        },
        Expr::Try { expr } => Expr::Try {
            expr: Box::new(substitute_partial_placeholder(expr, param_name)),
        },
        Expr::Await { expr, applies_try } => Expr::Await {
            expr: Box::new(substitute_partial_placeholder(expr, param_name)),
            applies_try: *applies_try,
        },
        _ => expr.clone(),
    }
}

fn substitute_partial_placeholder_arg(arg: &CallArg, param_name: &str) -> CallArg {
    match arg {
        CallArg::Positional(value) => {
            CallArg::Positional(substitute_partial_placeholder(value, param_name))
        }
        CallArg::Named { name, value } => CallArg::Named {
            name: name.clone(),
            value: Box::new(substitute_partial_placeholder(value, param_name)),
        },
        CallArg::Spread { value } => CallArg::Spread {
            value: Box::new(substitute_partial_placeholder(value, param_name)),
        },
    }
}

fn expr_contains_partial_placeholder(expr: &Expr) -> bool {
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
        Expr::Call { callee, args } => {
            expr_contains_partial_placeholder(callee)
                || args.iter().any(call_arg_contains_partial_placeholder)
        }
        Expr::MethodCall { receiver, args, .. } => {
            expr_contains_partial_placeholder(receiver)
                || args.iter().any(call_arg_contains_partial_placeholder)
        }
        Expr::Field { target, .. } | Expr::Try { expr: target } => {
            expr_contains_partial_placeholder(target)
        }
        Expr::Index { target, index } => {
            expr_contains_partial_placeholder(target) || expr_contains_partial_placeholder(index)
        }
        Expr::Unary { expr, .. } | Expr::Await { expr, .. } => {
            expr_contains_partial_placeholder(expr)
        }
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
        _ => false,
    }
}

fn call_arg_contains_partial_placeholder(arg: &CallArg) -> bool {
    match arg {
        CallArg::Positional(value) => expr_contains_partial_placeholder(value),
        CallArg::Named { value, .. } | CallArg::Spread { value } => {
            expr_contains_partial_placeholder(value)
        }
    }
}

fn lower_runtime_bracket_seq(items: &[Expr]) -> RuntimeExpr {
    let lowered = items.iter().map(lower_runtime_expr).collect::<Vec<_>>();
    fold_value_sequence(lowered)
}

fn lower_runtime_numeric_bracket_seq(
    seq: &arcweft_lang_hir::syntax::expr::NumericBracketSeq,
) -> RuntimeExpr {
    RuntimeExpr::Value(match seq.suffix() {
        Some("i8") => collect_dense(seq.values(), i8::try_from, runtime_sequence_dense_i8),
        Some("i16") => collect_dense(seq.values(), i16::try_from, runtime_sequence_dense_i16),
        Some("i32") | None => {
            collect_dense(seq.values(), i32::try_from, runtime_sequence_dense_i32)
        }
        Some("i128") => collect_dense(seq.values(), i128::try_from, runtime_sequence_dense_i128),
        Some("isize") => collect_dense(
            seq.values(),
            Ok::<i64, std::convert::Infallible>,
            runtime_sequence_dense_isize,
        ),
        Some("u8") => collect_dense(seq.values(), u8::try_from, runtime_sequence_dense_u8),
        Some("u16") => collect_dense(seq.values(), u16::try_from, runtime_sequence_dense_u16),
        Some("u32") => collect_dense(seq.values(), u32::try_from, runtime_sequence_dense_u32),
        Some("u64") => collect_dense(seq.values(), u64::try_from, runtime_sequence_dense_u64),
        Some("u128") => collect_dense(seq.values(), u128::try_from, runtime_sequence_dense_u128),
        Some("usize") => collect_dense(seq.values(), u64::try_from, runtime_sequence_dense_usize),
        Some(_) => runtime_sequence_dense_i64(seq.values().to_vec()),
    })
}

fn lower_choice_action_call(callee: &Expr, args: &[CallArg]) -> Option<RuntimeExpr> {
    if expr_label(callee) != "choice_action" {
        return None;
    }
    let [CallArg::Positional(choice)] = args else {
        return Some(RuntimeExpr::Value(RuntimeValue::String(format!(
            "choice_action({})",
            args.iter()
                .map(call_arg_label)
                .collect::<Vec<_>>()
                .join(", ")
        ))));
    };
    let choice = expr_label(choice).trim_start_matches('@').to_owned();
    Some(RuntimeExpr::Record(vec![
        runtime_field_expr(
            "id",
            RuntimeExpr::Value(RuntimeValue::String(format!(
                "action.select_choice.{choice}"
            ))),
        ),
        runtime_field_expr("target", RuntimeExpr::Value(RuntimeValue::String(choice))),
        runtime_field_expr(
            "action",
            RuntimeExpr::Value(RuntimeValue::String("select_choice".to_owned())),
        ),
        runtime_field_expr(
            "kind",
            RuntimeExpr::Value(RuntimeValue::String("semantic".to_owned())),
        ),
        runtime_field_expr("enabled", RuntimeExpr::Value(RuntimeValue::Bool(true))),
    ]))
}

fn runtime_field_expr(name: &str, value: RuntimeExpr) -> RuntimeFieldExpr {
    RuntimeFieldExpr {
        name: name.to_owned(),
        value,
    }
}

fn collect_dense<T, E>(
    values: &[i64],
    convert: impl Fn(i64) -> Result<T, E>,
    wrap: impl Fn(Vec<T>) -> RuntimeValue,
) -> RuntimeValue {
    values
        .iter()
        .copied()
        .map(convert)
        .collect::<Result<Vec<_>, _>>()
        .map_or_else(|_| runtime_sequence_dense_i64(values.to_vec()), wrap)
}

fn lower_runtime_bracket_seq_strict(
    items: &[Expr],
    helpers: Option<&BTreeMap<String, RuntimePureHelperId>>,
) -> Result<RuntimeExpr, String> {
    let lowered = items
        .iter()
        .map(|item| lower_runtime_expr_strict_with_helpers(item, helpers))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(fold_value_sequence(lowered))
}

fn lower_runtime_range_expr(
    start: Option<&Expr>,
    end: Option<&Expr>,
    inclusive: bool,
    helpers: Option<&BTreeMap<String, RuntimePureHelperId>>,
) -> Result<RuntimeExpr, String> {
    Ok(RuntimeExpr::Range {
        start: start
            .map(|start| lower_runtime_expr_strict_with_helpers(start, helpers))
            .transpose()?
            .map(Box::new),
        end: end
            .map(|end| lower_runtime_expr_strict_with_helpers(end, helpers))
            .transpose()?
            .map(Box::new),
        inclusive,
    })
}

fn lower_runtime_range_expr_lossy(
    start: Option<&Expr>,
    end: Option<&Expr>,
    inclusive: bool,
) -> RuntimeExpr {
    RuntimeExpr::Range {
        start: start.map(lower_runtime_expr).map(Box::new),
        end: end.map(lower_runtime_expr).map(Box::new),
        inclusive,
    }
}

fn lower_runtime_record_expr_strict(
    fields: &[(String, Expr)],
    helpers: Option<&BTreeMap<String, RuntimePureHelperId>>,
) -> Result<RuntimeExpr, String> {
    fields
        .iter()
        .map(|(name, value)| {
            Ok(RuntimeFieldExpr {
                name: name.clone(),
                value: lower_runtime_expr_strict_with_helpers(value, helpers)?,
            })
        })
        .collect::<Result<Vec<_>, String>>()
        .map(RuntimeExpr::Record)
}

fn fold_value_sequence(items: Vec<RuntimeExpr>) -> RuntimeExpr {
    if !items
        .iter()
        .all(|item| matches!(item, RuntimeExpr::Value(_)))
    {
        return RuntimeExpr::BracketSeq(items);
    }
    RuntimeExpr::Value(runtime_sequence_from_literal_values(
        items
            .into_iter()
            .filter_map(|item| match item {
                RuntimeExpr::Value(value) => Some(value),
                _ => None,
            })
            .collect(),
    ))
}

fn lower_runtime_field_expr(target: &Expr, field: &str) -> RuntimeExpr {
    record_field_ordinal(target, field).map_or_else(
        || RuntimeExpr::Field {
            target: Box::new(lower_runtime_expr(target)),
            field: field.to_owned(),
        },
        |ordinal| RuntimeExpr::ProjectRecord {
            target: Box::new(lower_runtime_expr(target)),
            ordinal,
        },
    )
}

fn lower_enum_variant_field(target: &Expr, field: &str) -> Option<RuntimeExpr> {
    let Expr::Path(path) = target else {
        return None;
    };
    is_uppercase_path_segment(path.as_label())
        .then_some(field)
        .filter(|field| is_uppercase_path_segment(field))
        .map(|field| RuntimeExpr::Variant {
            path: Some(path.as_label().to_owned()),
            name: field.to_owned(),
            payload: None,
        })
}

fn lower_strict_field_or_constant(
    expr: &Expr,
    target: &Expr,
    field: &str,
    helpers: Option<&BTreeMap<String, RuntimePureHelperId>>,
) -> Result<RuntimeExpr, String> {
    if let Some(value) = lower_enum_variant_field(target, field) {
        return Ok(value);
    }
    lower_std_float_constant(expr)
        .map(RuntimeExpr::Value)
        .map_or_else(|| lower_strict_field_expr(target, field, helpers), Ok)
}

fn lower_strict_field_expr(
    target: &Expr,
    field: &str,
    helpers: Option<&BTreeMap<String, RuntimePureHelperId>>,
) -> Result<RuntimeExpr, String> {
    let target_expr = lower_runtime_expr_strict_with_helpers(target, helpers)?;
    Ok(if let Some(ordinal) = record_field_ordinal(target, field) {
        RuntimeExpr::ProjectRecord {
            target: Box::new(target_expr),
            ordinal,
        }
    } else {
        RuntimeExpr::Field {
            target: Box::new(target_expr),
            field: field.to_owned(),
        }
    })
}

fn record_field_ordinal(target: &Expr, field: &str) -> Option<usize> {
    let (Expr::Record { fields, .. } | Expr::RecordLiteral(fields)) = target else {
        return None;
    };
    fields
        .iter()
        .position(|(candidate, _)| candidate.as_str() == field)
}

fn lower_runtime_index_expr(target: &Expr, index: &Expr) -> Option<RuntimeExpr> {
    tuple_index_ordinal(target, index).map_or_else(
        || {
            Some(RuntimeExpr::MethodCall {
                receiver: Box::new(lower_runtime_expr(target)),
                method: "__index".to_owned(),
                args: vec![lower_runtime_expr(index)],
            })
        },
        |ordinal| {
            Some(RuntimeExpr::ProjectTuple {
                target: Box::new(lower_runtime_expr(target)),
                ordinal,
            })
        },
    )
}

fn lower_strict_index_expr(
    target: &Expr,
    index: &Expr,
    helpers: Option<&BTreeMap<String, RuntimePureHelperId>>,
) -> Result<RuntimeExpr, String> {
    tuple_index_ordinal(target, index).map_or_else(
        || {
            Ok(RuntimeExpr::MethodCall {
                receiver: Box::new(lower_runtime_expr_strict_with_helpers(target, helpers)?),
                method: "__index".to_owned(),
                args: vec![lower_runtime_expr_strict_with_helpers(index, helpers)?],
            })
        },
        |ordinal| {
            lower_runtime_expr_strict_with_helpers(target, helpers).map(|target| {
                RuntimeExpr::ProjectTuple {
                    target: Box::new(target),
                    ordinal,
                }
            })
        },
    )
}

fn tuple_index_ordinal(target: &Expr, index: &Expr) -> Option<usize> {
    let Expr::Tuple(items) = target else {
        return None;
    };
    let ordinal = array_repeat_len(index)?;
    (ordinal < items.len()).then_some(ordinal)
}

fn runtime_method_name(method: &str) -> &str {
    method.split_once('<').map_or(method, |(name, _)| name)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RuntimeExternalNamespace {
    Conv2d,
    Data,
    Infer,
}

impl RuntimeExternalNamespace {
    fn from_receiver(receiver: &str) -> Option<Self> {
        match receiver {
            "conv2d" => Some(Self::Conv2d),
            "data" => Some(Self::Data),
            "infer" => Some(Self::Infer),
            _ => None,
        }
    }

    const fn as_label_prefix(self) -> &'static str {
        match self {
            Self::Conv2d => "conv2d",
            Self::Data => "data",
            Self::Infer => "infer",
        }
    }

    fn call_label(self, method: &str) -> String {
        format!("{}.{}", self.as_label_prefix(), runtime_method_name(method))
    }
}

fn lower_runtime_external_namespace_method_call(
    receiver: &Expr,
    method: &str,
    args: &[CallArg],
) -> Option<RuntimeExpr> {
    let Expr::Path(receiver) = receiver else {
        return None;
    };
    RuntimeExternalNamespace::from_receiver(receiver).map(|namespace| RuntimeExpr::Call {
        callee: RuntimeCallTarget::from_label(namespace.call_label(method)),
        args: args.iter().map(lower_runtime_call_arg).collect(),
    })
}

fn lower_runtime_path_method_call(
    receiver: &Expr,
    method: &str,
    args: &[CallArg],
) -> Option<RuntimeExpr> {
    let Expr::Path(receiver) = receiver else {
        return None;
    };
    let method = runtime_method_name(method);
    if receiver != "path" || !matches!(method, "save" | "asset" | "temp" | "export") {
        return None;
    }
    let [arg] = args else {
        return None;
    };
    if arg.name().is_some() || arg.is_spread() {
        return None;
    }
    Some(RuntimeExpr::Call {
        callee: RuntimeCallTarget::from_label(format!("path.{method}")),
        args: vec![lower_runtime_expr(arg.value())],
    })
}

fn lower_runtime_math_method_call(
    receiver: &Expr,
    method: &str,
    args: &[CallArg],
) -> Option<RuntimeExpr> {
    let Expr::Path(receiver) = receiver else {
        return None;
    };
    let method = runtime_method_name(method);
    if receiver != "math"
        || !matches!(
            method,
            "matmul_f32"
                | "matrix_add_f32"
                | "tensor_add_f32"
                | "matmul_f64"
                | "matrix_add_f64"
                | "tensor_add_f64"
        )
    {
        return None;
    }
    Some(RuntimeExpr::Call {
        callee: RuntimeCallTarget::from_label(format!("math.{method}")),
        args: args.iter().map(lower_runtime_call_arg).collect(),
    })
}

fn lower_runtime_std_float_method_call(
    receiver: &Expr,
    method: &str,
    args: &[CallArg],
) -> Option<RuntimeExpr> {
    let receiver = expr_label(receiver);
    let method = runtime_method_name(method);
    if !matches!(receiver.as_str(), "std.f32" | "std.f64")
        || RuntimeCallTarget::from_label(format!("{receiver}.{method}"))
            .as_intrinsic()
            .is_none()
    {
        return None;
    }
    Some(RuntimeExpr::Call {
        callee: RuntimeCallTarget::from_label(format!("{receiver}.{method}")),
        args: args.iter().map(lower_runtime_call_arg).collect(),
    })
}

fn lower_strict_path_method_call(
    receiver: &Expr,
    method: &str,
    args: &[CallArg],
    helpers: Option<&BTreeMap<String, RuntimePureHelperId>>,
) -> Option<Result<RuntimeExpr, String>> {
    let Expr::Path(receiver) = receiver else {
        return None;
    };
    let method = runtime_method_name(method);
    if receiver != "path" || !matches!(method, "save" | "asset" | "temp" | "export") {
        return None;
    }
    let [arg] = args else {
        return None;
    };
    if arg.name().is_some() || arg.is_spread() {
        return None;
    }
    Some(
        lower_runtime_expr_strict_with_helpers(arg.value(), helpers).map(|arg| RuntimeExpr::Call {
            callee: RuntimeCallTarget::from_label(format!("path.{method}")),
            args: vec![arg],
        }),
    )
}

fn lower_strict_math_method_call(
    receiver: &Expr,
    method: &str,
    args: &[CallArg],
    helpers: Option<&BTreeMap<String, RuntimePureHelperId>>,
) -> Option<Result<RuntimeExpr, String>> {
    let Expr::Path(receiver) = receiver else {
        return None;
    };
    let method = runtime_method_name(method);
    if receiver != "math"
        || !matches!(
            method,
            "matmul_f32"
                | "matrix_add_f32"
                | "tensor_add_f32"
                | "matmul_f64"
                | "matrix_add_f64"
                | "tensor_add_f64"
        )
    {
        return None;
    }
    Some(
        args.iter()
            .map(|arg| lower_strict_call_arg(arg, helpers))
            .collect::<Result<Vec<_>, _>>()
            .map(|args| RuntimeExpr::Call {
                callee: RuntimeCallTarget::from_label(format!("math.{method}")),
                args,
            }),
    )
}

fn lower_strict_std_float_method_call(
    receiver: &Expr,
    method: &str,
    args: &[CallArg],
    helpers: Option<&BTreeMap<String, RuntimePureHelperId>>,
) -> Option<Result<RuntimeExpr, String>> {
    let receiver = expr_label(receiver);
    let method = runtime_method_name(method);
    if !matches!(receiver.as_str(), "std.f32" | "std.f64")
        || RuntimeCallTarget::from_label(format!("{receiver}.{method}"))
            .as_intrinsic()
            .is_none()
    {
        return None;
    }
    Some(
        args.iter()
            .map(|arg| lower_strict_call_arg(arg, helpers))
            .collect::<Result<Vec<_>, _>>()
            .map(|args| RuntimeExpr::Call {
                callee: RuntimeCallTarget::from_label(format!("{receiver}.{method}")),
                args,
            }),
    )
}

fn lower_strict_external_namespace_method_call(
    receiver: &Expr,
    method: &str,
    args: &[CallArg],
    helpers: Option<&BTreeMap<String, RuntimePureHelperId>>,
) -> Option<Result<RuntimeExpr, String>> {
    let Expr::Path(receiver) = receiver else {
        return None;
    };
    RuntimeExternalNamespace::from_receiver(receiver).map(|namespace| {
        args.iter()
            .map(|arg| lower_strict_call_arg(arg, helpers))
            .collect::<Result<Vec<_>, _>>()
            .map(|args| RuntimeExpr::Call {
                callee: RuntimeCallTarget::from_label(namespace.call_label(method)),
                args,
            })
    })
}

fn lower_strict_call_expr(
    callee: &Expr,
    args: &[CallArg],
    helpers: Option<&BTreeMap<String, RuntimePureHelperId>>,
) -> Result<RuntimeExpr, String> {
    if let Some(lowered) = lower_agent_path_constructor_call(callee, args, helpers) {
        return lowered;
    }
    if let Some(lowered) = lower_choice_action_call(callee, args) {
        return Ok(lowered);
    }
    lower_constructor_call(callee, args, helpers).map_or_else(
        || {
            let callee = expr_label(callee);
            let args = args
                .iter()
                .map(|arg| lower_strict_call_arg(arg, helpers))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(
                if let Some(helper) = helpers.and_then(|helpers| helpers.get(&callee).copied()) {
                    RuntimeExpr::PureCall { helper, args }
                } else {
                    RuntimeExpr::Call {
                        callee: RuntimeCallTarget::from_label(callee),
                        args,
                    }
                },
            )
        },
        Ok,
    )
}

fn lower_agent_path_constructor_call(
    callee: &Expr,
    args: &[CallArg],
    helpers: Option<&BTreeMap<String, RuntimePureHelperId>>,
) -> Option<Result<RuntimeExpr, String>> {
    if !matches!(
        expr_label(callee).as_str(),
        "state_path" | "observation_path"
    ) {
        return None;
    }
    Some(match args {
        [arg] if arg.name().is_none() && !arg.is_spread() => {
            lower_runtime_expr_strict_with_helpers(arg.value(), helpers)
        }
        _ => Err(format!(
            "{} requires exactly one positional path argument",
            expr_label(callee)
        )),
    })
}

fn lower_strict_method_call_expr(
    receiver: &Expr,
    method: &str,
    args: &[CallArg],
    helpers: Option<&BTreeMap<String, RuntimePureHelperId>>,
) -> Result<RuntimeExpr, String> {
    if let Some(map) = lower_strict_map_method_call(receiver, method, args, helpers) {
        return map;
    }
    if let Some(filter) = lower_strict_filter_method_call(receiver, method, args, helpers) {
        return filter;
    }
    if runtime_method_name(method) == "sum" && args.is_empty() {
        return lower_runtime_expr_strict_with_helpers(receiver, helpers).map(|source| {
            RuntimeExpr::Sum {
                source: Box::new(source),
            }
        });
    }
    if runtime_method_name(method) == "summary" && args.is_empty() {
        return lower_runtime_expr_strict_with_helpers(receiver, helpers).map(|source| {
            RuntimeExpr::Field {
                target: Box::new(source),
                field: "summary".to_owned(),
            }
        });
    }
    Ok(RuntimeExpr::MethodCall {
        receiver: Box::new(lower_runtime_expr_strict_with_helpers(receiver, helpers)?),
        method: runtime_method_name(method).to_owned(),
        args: args
            .iter()
            .map(|arg| lower_strict_call_arg(arg, helpers))
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn lower_strict_map_method_call(
    receiver: &Expr,
    method: &str,
    args: &[CallArg],
    helpers: Option<&BTreeMap<String, RuntimePureHelperId>>,
) -> Option<Result<RuntimeExpr, String>> {
    if runtime_method_name(method) != "map" {
        return None;
    }
    let [arg] = args else {
        return None;
    };
    if arg.name().is_some() || arg.is_spread() {
        return None;
    }
    if expr_contains_partial_placeholder(arg.value()) {
        let param_name = "_item";
        let body = substitute_partial_placeholder(arg.value(), param_name);
        return Some(
            lower_runtime_expr_strict_with_helpers(receiver, helpers).and_then(|source| {
                lower_runtime_expr_strict_with_helpers(&body, helpers).map(|body| {
                    RuntimeExpr::Map {
                        source: Box::new(source),
                        param: param_name.to_owned(),
                        body: Box::new(body),
                    }
                })
            }),
        );
    }
    let Expr::Closure { params, body } = arg.value() else {
        return None;
    };
    let [param] = params.as_slice() else {
        return Some(Err(
            "runtime `map` closures must bind exactly one parameter".to_owned(),
        ));
    };
    let Some(param_name) = param.simple_ident() else {
        return Some(Err(
            "runtime `map` closure parameter must bind a simple identifier".to_owned(),
        ));
    };
    Some(
        lower_runtime_expr_strict_with_helpers(receiver, helpers).and_then(|source| {
            lower_runtime_expr_strict_with_helpers(body, helpers).map(|body| RuntimeExpr::Map {
                source: Box::new(source),
                param: param_name.to_owned(),
                body: Box::new(body),
            })
        }),
    )
}

fn lower_strict_filter_method_call(
    receiver: &Expr,
    method: &str,
    args: &[CallArg],
    helpers: Option<&BTreeMap<String, RuntimePureHelperId>>,
) -> Option<Result<RuntimeExpr, String>> {
    if runtime_method_name(method) != "filter" {
        return None;
    }
    let [arg] = args else {
        return None;
    };
    if arg.name().is_some() || arg.is_spread() {
        return None;
    }
    if expr_contains_partial_placeholder(arg.value()) {
        let param_name = "_item";
        let body = substitute_partial_placeholder(arg.value(), param_name);
        return Some(
            lower_runtime_expr_strict_with_helpers(receiver, helpers).and_then(|source| {
                lower_runtime_expr_strict_with_helpers(&body, helpers).map(|body| {
                    RuntimeExpr::Filter {
                        source: Box::new(source),
                        param: param_name.to_owned(),
                        body: Box::new(body),
                    }
                })
            }),
        );
    }
    let Expr::Closure { params, body } = arg.value() else {
        return None;
    };
    let [param] = params.as_slice() else {
        return Some(Err(
            "runtime `filter` closures must bind exactly one parameter".to_owned(),
        ));
    };
    let Some(param_name) = param.simple_ident() else {
        return Some(Err(
            "runtime `filter` closure parameter must bind a simple identifier".to_owned(),
        ));
    };
    Some(
        lower_runtime_expr_strict_with_helpers(receiver, helpers).and_then(|source| {
            lower_runtime_expr_strict_with_helpers(body, helpers).map(|body| RuntimeExpr::Filter {
                source: Box::new(source),
                param: param_name.to_owned(),
                body: Box::new(body),
            })
        }),
    )
}

fn lower_runtime_call_arg(arg: &CallArg) -> RuntimeExpr {
    match arg {
        CallArg::Spread { value } => RuntimeExpr::SpreadArg(Box::new(lower_runtime_expr(value))),
        value => lower_runtime_expr(value.value()),
    }
}

fn lower_strict_call_arg(
    arg: &CallArg,
    helpers: Option<&BTreeMap<String, RuntimePureHelperId>>,
) -> Result<RuntimeExpr, String> {
    match arg {
        CallArg::Spread { value } => Ok(RuntimeExpr::SpreadArg(Box::new(
            lower_runtime_expr_strict_with_helpers(value, helpers)?,
        ))),
        value => lower_runtime_expr_strict_with_helpers(value.value(), helpers),
    }
}

fn unsupported_strict_runtime_expr(expr: &Expr) -> Result<RuntimeExpr, String> {
    Err(format!(
        "unsupported runtime value expression `{}`",
        expr_label(expr)
    ))
}

fn lower_dialogue_call_value(
    plan: Option<&arcweft_lang_hir::syntax::ast::line_plan::LinePlan>,
) -> RuntimeExpr {
    let Some(plan) = plan else {
        return RuntimeExpr::Value(RuntimeValue::Unit);
    };
    let Some(out) = plan.items().iter().find_map(|item| match item {
        LinePlanItem::Out(expr) => Some(expr),
        _ => None,
    }) else {
        return RuntimeExpr::Value(RuntimeValue::Unit);
    };
    match out {
        Expr::Tuple(items) => RuntimeExpr::Tuple(
            items
                .iter()
                .map(|item| RuntimeExpr::Value(RuntimeValue::String(expr_label(item))))
                .collect(),
        ),
        expr => RuntimeExpr::Value(RuntimeValue::String(expr_label(expr))),
    }
}

fn lower_strict_block_value(
    value: Option<&Expr>,
    helpers: Option<&BTreeMap<String, RuntimePureHelperId>>,
) -> Result<RuntimeExpr, String> {
    value.map_or_else(
        || Ok(RuntimeExpr::Value(RuntimeValue::Unit)),
        |value| lower_runtime_expr_strict_with_helpers(value, helpers),
    )
}

fn lower_strict_block_expr(
    statements: &[Stmt],
    value: Option<&Expr>,
    helpers: Option<&BTreeMap<String, RuntimePureHelperId>>,
) -> Result<RuntimeExpr, String> {
    let body = lower_strict_block_value(value, helpers)?;
    statements.iter().rev().try_fold(body, |body, statement| {
        lower_strict_block_statement(statement, body, helpers)
    })
}

fn lower_strict_block_statement(
    statement: &Stmt,
    body: RuntimeExpr,
    helpers: Option<&BTreeMap<String, RuntimePureHelperId>>,
) -> Result<RuntimeExpr, String> {
    match statement {
        Stmt::Let { pattern, expr, .. } => {
            let name = pattern
                .simple_binding_name()
                .ok_or_else(|| format!("unsupported runtime let pattern `{pattern:?}`"))?
                .to_owned();
            Ok(RuntimeExpr::Let {
                name,
                expr: Box::new(lower_runtime_expr_strict_with_helpers(expr, helpers)?),
                body: Box::new(body),
            })
        }
        Stmt::Assign { target, expr } => {
            let (target, field) = lower_direct_assignment_target(target, helpers)?;
            Ok(RuntimeExpr::AssignField {
                target: Box::new(target),
                field,
                expr: Box::new(lower_runtime_expr_strict_with_helpers(expr, helpers)?),
                body: Box::new(body),
            })
        }
        Stmt::Return(expr) => lower_runtime_expr_strict_with_helpers(expr, helpers),
        other => Err(format!("unsupported runtime block statement `{other:?}`")),
    }
}

fn lower_direct_assignment_target(
    target: &Expr,
    helpers: Option<&BTreeMap<String, RuntimePureHelperId>>,
) -> Result<(RuntimeExpr, String), String> {
    let Expr::Field { target, field } = target else {
        return Err(format!(
            "unsupported runtime assignment target `{}`: only direct record fields are executable",
            expr_label(target)
        ));
    };
    let receiver = lower_runtime_expr_strict_with_helpers(target, helpers)?;
    match receiver {
        RuntimeExpr::Local(_) => Ok((receiver, field.clone())),
        RuntimeExpr::Field { .. }
        | RuntimeExpr::ProjectTuple { .. }
        | RuntimeExpr::ProjectRecord { .. } => Err(format!(
            "unsupported runtime assignment target `{}`: nested assignment targets require a future lvalue model",
            expr_label(target)
        )),
        other => Err(format!(
            "unsupported runtime assignment receiver `{other}`: assignment requires a local record value"
        )),
    }
}

fn lower_strict_if_expr(
    condition: &Expr,
    then_branch: &Expr,
    else_branch: Option<&Expr>,
    helpers: Option<&BTreeMap<String, RuntimePureHelperId>>,
) -> Result<RuntimeExpr, String> {
    Ok(RuntimeExpr::If {
        condition: Box::new(lower_runtime_expr_strict_with_helpers(condition, helpers)?),
        then_expr: Box::new(lower_runtime_expr_strict_with_helpers(
            then_branch,
            helpers,
        )?),
        else_expr: Box::new(
            else_branch.map_or(Ok(RuntimeExpr::Value(RuntimeValue::Unit)), |else_branch| {
                lower_runtime_expr_strict_with_helpers(else_branch, helpers)
            })?,
        ),
    })
}

fn lower_strict_if_let_expr(
    pattern: &Pattern,
    expr: &Expr,
    guard: Option<&Expr>,
    then_branch: &Expr,
    else_branch: Option<&Expr>,
    helpers: Option<&BTreeMap<String, RuntimePureHelperId>>,
) -> Result<RuntimeExpr, String> {
    Ok(RuntimeExpr::IfLet {
        pattern: lower_runtime_pattern(pattern),
        expr: Box::new(lower_runtime_expr_strict_with_helpers(expr, helpers)?),
        guard: guard
            .map(|guard| lower_runtime_expr_strict_with_helpers(guard, helpers))
            .transpose()?
            .map(Box::new),
        then_expr: Box::new(lower_runtime_expr_strict_with_helpers(
            then_branch,
            helpers,
        )?),
        else_expr: Box::new(
            else_branch.map_or(Ok(RuntimeExpr::Value(RuntimeValue::Unit)), |else_branch| {
                lower_runtime_expr_strict_with_helpers(else_branch, helpers)
            })?,
        ),
    })
}

fn lower_strict_match_expr(
    scrutinee: &Expr,
    arms: &[MatchExprArm],
    helpers: Option<&BTreeMap<String, RuntimePureHelperId>>,
) -> Result<RuntimeExpr, String> {
    Ok(RuntimeExpr::Match {
        scrutinee: Box::new(lower_runtime_expr_strict_with_helpers(scrutinee, helpers)?),
        arms: arms
            .iter()
            .map(|arm| {
                Ok(RuntimeExprMatchArm {
                    pattern: lower_runtime_pattern(arm.pattern()),
                    guard: arm
                        .guard()
                        .map(|guard| lower_runtime_expr_strict_with_helpers(guard, helpers))
                        .transpose()?,
                    value: lower_runtime_expr_strict_with_helpers(arm.value(), helpers)?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?,
    })
}

fn lower_constructor_call(
    callee: &Expr,
    args: &[CallArg],
    helpers: Option<&BTreeMap<String, RuntimePureHelperId>>,
) -> Option<RuntimeExpr> {
    let Expr::Path(callee) = callee else {
        return None;
    };
    let (path, name) = constructor_path(callee)?;
    if args.len() > 1 {
        return None;
    }
    let payload = args
        .first()
        .and_then(|arg| match arg {
            CallArg::Positional(value) => Some(value),
            CallArg::Named { .. } | CallArg::Spread { .. } => None,
        })
        .map(|payload| lower_runtime_expr_strict_with_helpers(payload, helpers))
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
    if let Some(name) = path.strip_prefix('.')
        && is_uppercase_path_segment(name)
    {
        return Some((None, name.to_owned()));
    }
    if let Some((prefix, name)) = path.rsplit_once('.')
        && is_uppercase_path_segment(prefix)
        && is_uppercase_path_segment(name)
    {
        return Some((Some(prefix.to_owned()), name.to_owned()));
    }
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

fn is_uppercase_path_segment(name: &str) -> bool {
    name.chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_uppercase())
}

fn lower_runtime_literal(literal: &Literal) -> RuntimeValue {
    match literal {
        Literal::String(value) => RuntimeValue::String(decode_string_literal_value(value)),
        Literal::Char { value, .. } => RuntimeValue::Char(*value),
        Literal::Int { value, suffix, .. } => lower_runtime_int_literal(*value, suffix.as_deref()),
        Literal::Float {
            raw,
            suffix: Some(FloatSuffix::F32),
        } => RuntimeValue::F32(
            typed_f32_literal(raw, FloatSuffix::F32).expect("syntax parser accepted f32 literal"),
        ),
        Literal::Float {
            raw,
            suffix: Some(FloatSuffix::F64),
        } => RuntimeValue::F64(typed_f64_literal(raw, FloatSuffix::F64)),
        Literal::Float { raw, .. } => RuntimeValue::F64(parse_f64_literal(raw)),
        Literal::UnitNumber { raw, .. } => RuntimeValue::String(raw.clone()),
        Literal::Bool(value) => RuntimeValue::Bool(*value),
        Literal::Duration { .. } => duration_expr(&Expr::Literal(literal.clone())).map_or_else(
            || RuntimeValue::String(literal_label(literal)),
            RuntimeValue::Duration,
        ),
    }
}

fn lower_runtime_int_literal(value: i64, suffix: Option<&str>) -> RuntimeValue {
    match suffix {
        Some("i8") => i8::try_from(value).map_or(RuntimeValue::i64(value), RuntimeValue::i8),
        Some("i16") => i16::try_from(value).map_or(RuntimeValue::i64(value), RuntimeValue::i16),
        Some("i32") | None => {
            i32::try_from(value).map_or(RuntimeValue::i64(value), RuntimeValue::i32)
        }
        Some("i128") => RuntimeValue::i128(i128::from(value)),
        Some("isize") => RuntimeValue::isize(value),
        Some("u8") => u8::try_from(value).map_or(RuntimeValue::i64(value), RuntimeValue::u8),
        Some("u16") => u16::try_from(value).map_or(RuntimeValue::i64(value), RuntimeValue::u16),
        Some("u32") => u32::try_from(value).map_or(RuntimeValue::i64(value), RuntimeValue::u32),
        Some("u64") => u64::try_from(value).map_or(RuntimeValue::i64(value), RuntimeValue::u64),
        Some("u128") => u128::try_from(value).map_or(RuntimeValue::i64(value), RuntimeValue::u128),
        Some("usize") => u64::try_from(value).map_or(RuntimeValue::i64(value), RuntimeValue::usize),
        Some(_) => RuntimeValue::i64(value),
    }
}

fn decode_string_literal_value(value: &str) -> String {
    let mut decoded = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            decoded.push(ch);
            continue;
        }
        match chars.next() {
            Some('"') => decoded.push('"'),
            Some('\\') | None => decoded.push('\\'),
            Some('n') => decoded.push('\n'),
            Some('r') => decoded.push('\r'),
            Some('t') => decoded.push('\t'),
            Some('u') => decode_unicode_string_escape(&mut chars, &mut decoded),
            Some(other) => {
                decoded.push('\\');
                decoded.push(other);
            }
        }
    }
    decoded
}

fn decode_unicode_string_escape(chars: &mut std::str::Chars<'_>, decoded: &mut String) {
    if chars.next() != Some('{') {
        decoded.push_str("\\u");
        return;
    }
    let mut digits = String::new();
    for ch in chars.by_ref() {
        if ch == '}' {
            if let Some(ch) = u32::from_str_radix(&digits, 16)
                .ok()
                .and_then(char::from_u32)
            {
                decoded.push(ch);
            } else {
                decoded.push_str("\\u{");
                decoded.push_str(&digits);
                decoded.push('}');
            }
            return;
        }
        digits.push(ch);
    }
    decoded.push_str("\\u{");
    decoded.push_str(&digits);
}

fn lower_std_float_constant(expr: &Expr) -> Option<RuntimeValue> {
    Some(match expr_label(expr).as_str() {
        "std.f32.nan" => RuntimeValue::F32(f32::NAN),
        "std.f32.infinity" => RuntimeValue::F32(f32::INFINITY),
        "std.f32.neg_infinity" => RuntimeValue::F32(f32::NEG_INFINITY),
        "std.f32.epsilon" => RuntimeValue::F32(f32::EPSILON),
        "std.f32.min" => RuntimeValue::F32(f32::MIN),
        "std.f32.max" => RuntimeValue::F32(f32::MAX),
        "std.f32.pi" => RuntimeValue::F32(std::f32::consts::PI),
        "std.f32.tau" => RuntimeValue::F32(std::f32::consts::TAU),
        "std.f64.nan" => RuntimeValue::F64(f64::NAN),
        "std.f64.infinity" => RuntimeValue::F64(f64::INFINITY),
        "std.f64.neg_infinity" => RuntimeValue::F64(f64::NEG_INFINITY),
        "std.f64.epsilon" => RuntimeValue::F64(f64::EPSILON),
        "std.f64.min" => RuntimeValue::F64(f64::MIN),
        "std.f64.max" => RuntimeValue::F64(f64::MAX),
        "std.f64.pi" => RuntimeValue::F64(std::f64::consts::PI),
        "std.f64.tau" => RuntimeValue::F64(std::f64::consts::TAU),
        _ => return None,
    })
}

fn typed_f32_literal(raw: &str, suffix: FloatSuffix) -> Option<f32> {
    raw.strip_suffix(suffix.as_str())
        .and_then(|value| value.parse::<f32>().ok())
}

fn typed_f64_literal(raw: &str, suffix: FloatSuffix) -> f64 {
    raw.strip_suffix(suffix.as_str())
        .map_or(raw, str::trim)
        .parse::<f64>()
        .expect("syntax parser accepted f64 literal")
}

fn parse_f64_literal(raw: &str) -> f64 {
    raw.parse::<f64>()
        .expect("syntax parser accepted unsuffixed float literal")
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
            args: args.iter().map(call_arg_label).collect(),
        },
        Expr::MethodCall {
            receiver,
            method,
            args,
        } => RuntimeCall {
            callee: format!("{}.{}", expr_label(receiver), method),
            args: args.iter().map(call_arg_label).collect(),
        },
        Expr::Path(path) => RuntimeCall {
            callee: path.as_label().to_owned(),
            args: Vec::new(),
        },
        Expr::ShortVariant(name) => RuntimeCall {
            callee: format!(".{name}"),
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
    if let Some(effect) = crate::audio::lower_audio_call(expr) {
        return effect;
    }
    let call = runtime_call(expr);
    if let Some(effect) = runtime_control_call(&call) {
        return effect;
    }
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

fn runtime_control_call(call: &RuntimeCall) -> Option<LineEffectRequest> {
    match call.callee.as_str() {
        "panic" => Some(LineEffectRequest::Panic(
            call.args.first().cloned().unwrap_or_default(),
        )),
        "fail" => Some(LineEffectRequest::Fail(
            call.args.first().cloned().unwrap_or_default(),
        )),
        "bail" => Some(LineEffectRequest::Bail(
            call.args.first().cloned().unwrap_or_default(),
        )),
        "ensure" => Some(LineEffectRequest::Ensure {
            condition: call.args.first().cloned().unwrap_or_default(),
            message: call.args.get(1).cloned().unwrap_or_default(),
        }),
        "assert" => Some(LineEffectRequest::Assert(runtime_assertion(
            call,
            RuntimeAssertionProfile::Always,
        ))),
        "debug_assert" => Some(LineEffectRequest::Assert(runtime_assertion(
            call,
            RuntimeAssertionProfile::DebugOnly,
        ))),
        _ => None,
    }
}

fn runtime_assertion(call: &RuntimeCall, profile: RuntimeAssertionProfile) -> RuntimeAssertion {
    RuntimeAssertion {
        condition: call.args.first().cloned().unwrap_or_default(),
        message: call
            .args
            .get(1)
            .cloned()
            .unwrap_or_else(|| "assertion failed".to_owned()),
        profile,
    }
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
    let value = lower_runtime_expr(value);
    repeated_runtime_expr(value, len)
}

fn lower_runtime_array_repeat_strict(
    value: &Expr,
    len: &Expr,
    helpers: Option<&BTreeMap<String, RuntimePureHelperId>>,
) -> Result<RuntimeExpr, String> {
    let Some(len) = array_repeat_len(len) else {
        return Err(format!(
            "array repeat length must be an integer constant in `{}`",
            expr_label(len)
        ));
    };
    lower_runtime_expr_strict_with_helpers(value, helpers)
        .map(|value| repeated_runtime_expr(value, len))
}

fn repeated_runtime_expr(value: RuntimeExpr, len: usize) -> RuntimeExpr {
    RuntimeExpr::RepeatSeq {
        value: Box::new(value),
        len,
    }
}

fn array_repeat_len(expr: &Expr) -> Option<usize> {
    match expr {
        Expr::Literal(Literal::Int { value, .. }) => usize::try_from(*value).ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
