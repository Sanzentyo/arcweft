//! Runtime expression and effect-call lowering.

use crate::labels::{
    call_arg_label, duration_expr, expr_label, literal_label, named_arg_label, named_arg_value,
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
    ast::line_plan::LinePlanItem,
    ast::pattern::Pattern,
    expr::{BinaryOp, CallArg, Expr, FloatSuffix, Literal, MatchExprArm, UnaryOp},
};
use std::collections::BTreeMap;

/// Lowers an expression into a runtime value expression, preserving a lossy
/// string label for adapter-facing values that are not executable by the core.
pub(crate) fn lower_runtime_expr(expr: &Expr) -> RuntimeExpr {
    match expr {
        Expr::Literal(literal) => RuntimeExpr::Value(lower_runtime_literal(literal)),
        Expr::EntityRef(entity) => RuntimeExpr::EntityRef(entity.body().to_owned()),
        Expr::Path(path) => RuntimeExpr::Local(path.clone()),
        Expr::Tuple(items) if items.is_empty() => RuntimeExpr::Value(RuntimeValue::Unit),
        Expr::Tuple(items) => RuntimeExpr::Tuple(items.iter().map(lower_runtime_expr).collect()),
        Expr::BracketSeq(items) => lower_runtime_bracket_seq(items),
        Expr::NumericBracketSeq(seq) => lower_runtime_numeric_bracket_seq(seq),
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
        Expr::Field { target, field } => lower_runtime_field_expr(target, field),
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
        Expr::Call { callee, args } => RuntimeExpr::Call {
            callee: RuntimeCallTarget::from_label(expr_label(callee)),
            args: args.iter().map(lower_runtime_call_arg).collect(),
        },
        Expr::MethodCall {
            receiver,
            method,
            args,
        } => lower_runtime_path_method_call(receiver, method, args).unwrap_or_else(|| {
            RuntimeExpr::MethodCall {
                receiver: Box::new(lower_runtime_expr(receiver)),
                method: runtime_method_name(method).to_owned(),
                args: args.iter().map(lower_runtime_call_arg).collect(),
            }
        }),
        Expr::Index { target, index } => {
            lower_runtime_index_expr(target, index).unwrap_or_else(|| lower_runtime_expr(target))
        }
        Expr::Try { expr } | Expr::Await { expr, .. } | Expr::Pipe { lhs: expr, .. } => {
            lower_runtime_expr(expr)
        }
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
        Expr::EntityRef(entity) => Ok(RuntimeExpr::EntityRef(entity.body().to_owned())),
        Expr::Path(path) => Ok(constructor_path(path).map_or_else(
            || RuntimeExpr::Local(path.clone()),
            |(path, name)| RuntimeExpr::Variant {
                path,
                name,
                payload: None,
            },
        )),
        Expr::Tuple(items) if items.is_empty() => Ok(RuntimeExpr::Value(RuntimeValue::Unit)),
        Expr::Tuple(items) => items
            .iter()
            .map(|item| lower_runtime_expr_strict_with_helpers(item, helpers))
            .collect::<Result<Vec<_>, _>>()
            .map(RuntimeExpr::Tuple),
        Expr::BracketSeq(items) => lower_runtime_bracket_seq_strict(items, helpers),
        Expr::NumericBracketSeq(seq) => Ok(lower_runtime_numeric_bracket_seq(seq)),
        Expr::ArrayRepeat { value, len } => lower_runtime_array_repeat_strict(value, len, helpers),
        Expr::Record { fields, .. } | Expr::RecordLiteral(fields) => fields
            .iter()
            .map(|(name, value)| {
                Ok(RuntimeFieldExpr {
                    name: name.clone(),
                    value: lower_runtime_expr_strict_with_helpers(value, helpers)?,
                })
            })
            .collect::<Result<Vec<_>, String>>()
            .map(RuntimeExpr::Record),
        Expr::Field { target, field } => lower_strict_field_expr(target, field, helpers),
        Expr::Unary { op, expr } => Ok(RuntimeExpr::Unary {
            op: lower_runtime_unary_op(*op),
            expr: Box::new(lower_runtime_expr_strict_with_helpers(expr, helpers)?),
        }),
        Expr::Binary { lhs, op, rhs } => {
            let Some(op) = lower_runtime_binary_op(*op) else {
                return Err(format!(
                    "unsupported runtime binary expression `{}`",
                    expr_label(expr)
                ));
            };
            Ok(RuntimeExpr::Binary {
                lhs: Box::new(lower_runtime_expr_strict_with_helpers(lhs, helpers)?),
                op,
                rhs: Box::new(lower_runtime_expr_strict_with_helpers(rhs, helpers)?),
            })
        }
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
        Expr::Block { value, .. }
        | Expr::ComputationBlock { value, .. }
        | Expr::MemoBlock { value, .. }
        | Expr::NamedBlock { value, .. } => lower_strict_block_value(value.as_deref(), helpers),
        Expr::Call { callee, args } => lower_strict_call_expr(callee, args, helpers),
        Expr::MethodCall {
            receiver,
            method,
            args,
        } => match lower_strict_path_method_call(receiver, method, args, helpers) {
            Some(lowered) => lowered,
            None => lower_strict_method_call_expr(receiver, method, args, helpers),
        },
        Expr::DialogueCall { plan, .. } => Ok(lower_dialogue_call_value(plan.as_ref())),
        Expr::Index { target, index } => lower_strict_index_expr(target, index, helpers),
        Expr::Pipe { .. }
        | Expr::Try { .. }
        | Expr::Await { .. }
        | Expr::Thread { .. }
        | Expr::Range { .. }
        | Expr::Closure { .. }
        | Expr::LifetimePath { .. }
        | Expr::Placeholder(_)
        | Expr::Raw(_) => unsupported_strict_runtime_expr(expr),
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
        Some("i32") => collect_dense(seq.values(), i32::try_from, runtime_sequence_dense_i32),
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
        _ => runtime_sequence_dense_i64(seq.values().to_vec()),
    })
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
    tuple_index_ordinal(target, index).map(|ordinal| RuntimeExpr::ProjectTuple {
        target: Box::new(lower_runtime_expr(target)),
        ordinal,
    })
}

fn lower_strict_index_expr(
    target: &Expr,
    index: &Expr,
    helpers: Option<&BTreeMap<String, RuntimePureHelperId>>,
) -> Result<RuntimeExpr, String> {
    tuple_index_ordinal(target, index).map_or_else(
        || {
            unsupported_strict_runtime_expr(&Expr::Index {
                target: Box::new(target.clone()),
                index: Box::new(index.clone()),
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

fn lower_strict_call_expr(
    callee: &Expr,
    args: &[CallArg],
    helpers: Option<&BTreeMap<String, RuntimePureHelperId>>,
) -> Result<RuntimeExpr, String> {
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

fn lower_strict_method_call_expr(
    receiver: &Expr,
    method: &str,
    args: &[CallArg],
    helpers: Option<&BTreeMap<String, RuntimePureHelperId>>,
) -> Result<RuntimeExpr, String> {
    if let Some(map) = lower_strict_map_method_call(receiver, method, args, helpers) {
        return map;
    }
    if runtime_method_name(method) == "sum" && args.is_empty() {
        return lower_runtime_expr_strict_with_helpers(receiver, helpers).map(|source| {
            RuntimeExpr::Sum {
                source: Box::new(source),
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
    let Expr::Closure { params, body } = arg.value() else {
        return None;
    };
    let [param] = params.as_slice() else {
        return Some(Err(
            "runtime `map` closures must bind exactly one parameter".to_owned(),
        ));
    };
    Some(
        lower_runtime_expr_strict_with_helpers(receiver, helpers).and_then(|source| {
            lower_runtime_expr_strict_with_helpers(body, helpers).map(|body| RuntimeExpr::Map {
                source: Box::new(source),
                param: param.clone(),
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
        Literal::Int {
            value,
            suffix: Some(suffix),
            ..
        } if suffix == "i8" => {
            i8::try_from(*value).map_or(RuntimeValue::i64(*value), RuntimeValue::i8)
        }
        Literal::Int {
            value,
            suffix: Some(suffix),
            ..
        } if suffix == "i16" => {
            i16::try_from(*value).map_or(RuntimeValue::i64(*value), RuntimeValue::i16)
        }
        Literal::Int {
            value,
            suffix: Some(suffix),
            ..
        } if suffix == "i32" => {
            i32::try_from(*value).map_or(RuntimeValue::i64(*value), RuntimeValue::i32)
        }
        Literal::Int {
            value,
            suffix: Some(suffix),
            ..
        } if suffix == "i64" => RuntimeValue::i64(*value),
        Literal::Int {
            value,
            suffix: Some(suffix),
            ..
        } if suffix == "i128" => RuntimeValue::i128(i128::from(*value)),
        Literal::Int {
            value,
            suffix: Some(suffix),
            ..
        } if suffix == "isize" => RuntimeValue::isize(*value),
        Literal::Int {
            value,
            suffix: Some(suffix),
            ..
        } if suffix == "u8" => {
            u8::try_from(*value).map_or(RuntimeValue::i64(*value), RuntimeValue::u8)
        }
        Literal::Int {
            value,
            suffix: Some(suffix),
            ..
        } if suffix == "u16" => {
            u16::try_from(*value).map_or(RuntimeValue::i64(*value), RuntimeValue::u16)
        }
        Literal::Int {
            value,
            suffix: Some(suffix),
            ..
        } if suffix == "u32" => {
            u32::try_from(*value).map_or(RuntimeValue::i64(*value), RuntimeValue::u32)
        }
        Literal::Int {
            value,
            suffix: Some(suffix),
            ..
        } if suffix == "u64" => {
            u64::try_from(*value).map_or(RuntimeValue::i64(*value), RuntimeValue::u64)
        }
        Literal::Int {
            value,
            suffix: Some(suffix),
            ..
        } if suffix == "u128" => {
            u128::try_from(*value).map_or(RuntimeValue::i64(*value), RuntimeValue::u128)
        }
        Literal::Int {
            value,
            suffix: Some(suffix),
            ..
        } if suffix == "usize" => {
            u64::try_from(*value).map_or(RuntimeValue::i64(*value), RuntimeValue::usize)
        }
        Literal::Int { value, .. } => RuntimeValue::i64(*value),
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
mod tests {
    use super::*;

    #[test]
    fn strict_runtime_value_lowering_preserves_calls() {
        let expr = Expr::Call {
            callee: Box::new(Expr::Path("compute".to_owned())),
            args: Vec::new(),
        };

        let lowered = lower_runtime_expr_strict(&expr).expect("calls are runtime values");

        assert!(
            matches!(lowered, RuntimeExpr::Call { callee, .. } if callee.as_label() == "compute")
        );
    }

    #[test]
    fn strict_runtime_value_lowering_can_emit_pure_calls() {
        let expr = Expr::Call {
            callee: Box::new(Expr::Path("compute".to_owned())),
            args: vec![CallArg::Positional(Expr::Literal(Literal::Int {
                raw: "3i64".to_owned(),
                value: 3,
                suffix: Some("i64".to_owned()),
            }))],
        };
        let helpers = BTreeMap::from([("compute".to_owned(), RuntimePureHelperId(2))]);

        let lowered =
            lower_runtime_expr_strict_with_pure(&expr, &helpers).expect("pure calls lower");

        assert!(matches!(
            lowered,
            RuntimeExpr::PureCall { helper, args }
                if helper == RuntimePureHelperId(2)
                    && matches!(args.as_slice(), [RuntimeExpr::Value(value)] if value == &RuntimeValue::i64(3))
        ));
    }

    #[test]
    fn strict_runtime_array_repeat_folds_literal_value_sequence() {
        let expr = Expr::ArrayRepeat {
            value: Box::new(Expr::Literal(Literal::Int {
                raw: "2i64".to_owned(),
                value: 2,
                suffix: Some("i64".to_owned()),
            })),
            len: Box::new(Expr::Literal(Literal::Int {
                raw: "4".to_owned(),
                value: 4,
                suffix: None,
            })),
        };

        let lowered = lower_runtime_expr_strict(&expr).expect("array repeat lowers");

        assert!(matches!(
            lowered,
            RuntimeExpr::RepeatSeq { value, len: 4 }
                if matches!(value.as_ref(), RuntimeExpr::Value(value) if value == &RuntimeValue::i64(2))
        ));
    }

    #[test]
    fn suffixed_integer_literals_lower_to_width_preserving_runtime_scalars() {
        for (suffix, expected) in [
            ("i8", RuntimeValue::i8(7)),
            ("i16", RuntimeValue::i16(7)),
            ("i32", RuntimeValue::i32(7)),
            ("i64", RuntimeValue::i64(7)),
            ("i128", RuntimeValue::i128(7)),
            ("isize", RuntimeValue::isize(7)),
            ("u8", RuntimeValue::u8(7)),
            ("u16", RuntimeValue::u16(7)),
            ("u32", RuntimeValue::u32(7)),
            ("u64", RuntimeValue::u64(7)),
            ("u128", RuntimeValue::u128(7)),
            ("usize", RuntimeValue::usize(7)),
        ] {
            let expr = Expr::Literal(Literal::Int {
                raw: format!("7{suffix}"),
                value: 7,
                suffix: Some(suffix.to_owned()),
            });

            let lowered =
                lower_runtime_expr_strict(&expr).expect("suffixed integer literal lowers");

            assert_eq!(lowered, RuntimeExpr::Value(expected));
        }
    }

    #[test]
    fn strict_runtime_bracket_seq_folds_literal_values_to_dense_storage() {
        let unit_expr = Expr::BracketSeq(vec![
            Expr::Tuple(Vec::new()),
            Expr::Tuple(Vec::new()),
            Expr::Tuple(Vec::new()),
        ]);

        let lowered = lower_runtime_expr_strict(&unit_expr).expect("unit bracket seq lowers");

        assert!(matches!(
            lowered,
            RuntimeExpr::Value(RuntimeValue::Seq(seq)) if seq.unit_len() == Some(3)
        ));

        let i64_expr = Expr::BracketSeq(vec![
            Expr::Literal(Literal::Int {
                raw: "1i64".to_owned(),
                value: 1,
                suffix: Some("i64".to_owned()),
            }),
            Expr::Literal(Literal::Int {
                raw: "2i64".to_owned(),
                value: 2,
                suffix: Some("i64".to_owned()),
            }),
        ]);

        let lowered = lower_runtime_expr_strict(&i64_expr).expect("i64 bracket seq lowers");

        assert!(matches!(
            lowered,
            RuntimeExpr::Value(RuntimeValue::Seq(seq))
                if seq.as_i64_slice() == Some([1, 2].as_slice())
        ));

        let bool_expr = Expr::BracketSeq(vec![
            Expr::Literal(Literal::Bool(true)),
            Expr::Literal(Literal::Bool(false)),
        ]);
        let lowered = lower_runtime_expr_strict(&bool_expr).expect("bool bracket seq lowers");
        assert!(matches!(
            lowered,
            RuntimeExpr::Value(RuntimeValue::Seq(seq))
                if seq.as_bool_slice() == Some([true, false].as_slice())
        ));

        let char_expr = Expr::BracketSeq(vec![
            Expr::Literal(Literal::Char {
                raw: "\"a\"c".to_owned(),
                value: 'a',
            }),
            Expr::Literal(Literal::Char {
                raw: "\"b\"c".to_owned(),
                value: 'b',
            }),
        ]);
        let lowered = lower_runtime_expr_strict(&char_expr).expect("char bracket seq lowers");
        assert!(matches!(
            lowered,
            RuntimeExpr::Value(RuntimeValue::Seq(seq))
                if seq.as_chars() == Some(['a', 'b'].as_slice())
        ));

        let duration_expr = Expr::BracketSeq(vec![
            Expr::Literal(Literal::Duration {
                amount: "1".to_owned(),
                unit: arcweft_lang_hir::syntax::expr::DurationUnit::Millis,
            }),
            Expr::Literal(Literal::Duration {
                amount: "2".to_owned(),
                unit: arcweft_lang_hir::syntax::expr::DurationUnit::Millis,
            }),
        ]);
        let lowered =
            lower_runtime_expr_strict(&duration_expr).expect("duration bracket seq lowers");
        assert!(matches!(
            lowered,
            RuntimeExpr::Value(RuntimeValue::Seq(seq))
                if seq.as_durations() == Some([
                    arcweft_core::time::LogicalDuration::from_nanos(1_000_000),
                    arcweft_core::time::LogicalDuration::from_nanos(2_000_000),
                ].as_slice())
        ));
    }

    #[test]
    fn strict_runtime_bracket_seq_folds_typed_float_literals_to_dense_storage() {
        let f32_expr = Expr::BracketSeq(vec![
            Expr::Literal(Literal::Float {
                raw: "1.5f32".to_owned(),
                suffix: Some(FloatSuffix::F32),
            }),
            Expr::Literal(Literal::Float {
                raw: "2.5f32".to_owned(),
                suffix: Some(FloatSuffix::F32),
            }),
        ]);
        let f64_expr = Expr::BracketSeq(vec![
            Expr::Literal(Literal::Float {
                raw: "3.25f64".to_owned(),
                suffix: Some(FloatSuffix::F64),
            }),
            Expr::Literal(Literal::Float {
                raw: "-0.0f64".to_owned(),
                suffix: Some(FloatSuffix::F64),
            }),
        ]);

        let f32_lowered = lower_runtime_expr_strict(&f32_expr).expect("f32 bracket seq lowers");
        let f64_lowered = lower_runtime_expr_strict(&f64_expr).expect("f64 bracket seq lowers");

        assert!(matches!(
            f32_lowered,
            RuntimeExpr::Value(RuntimeValue::Seq(seq))
                if seq.as_f32_slice() == Some([
                    (1.5),
                    (2.5),
                ].as_slice())
        ));
        assert!(matches!(
            f64_lowered,
            RuntimeExpr::Value(RuntimeValue::Seq(seq))
                if seq.as_f64_slice() == Some([
                    (3.25),
                    (-0.0),
                ].as_slice())
        ));
    }

    #[test]
    fn strict_runtime_field_lowering_uses_record_projection_when_ordinal_is_known() {
        let expr = Expr::Field {
            target: Box::new(Expr::RecordLiteral(vec![
                (
                    "score".to_owned(),
                    Expr::Literal(Literal::Int {
                        raw: "7".to_owned(),
                        value: 7,
                        suffix: None,
                    }),
                ),
                (
                    "label".to_owned(),
                    Expr::Literal(Literal::String("ok".to_owned())),
                ),
            ])),
            field: "label".to_owned(),
        };

        let lowered = lower_runtime_expr_strict(&expr).expect("record field lowers");

        assert!(matches!(
            lowered,
            RuntimeExpr::ProjectRecord { ordinal: 1, .. }
        ));
    }

    #[test]
    fn strict_runtime_tuple_index_lowering_uses_tuple_projection_when_ordinal_is_known() {
        let expr = Expr::Index {
            target: Box::new(Expr::Tuple(vec![
                Expr::Literal(Literal::Int {
                    raw: "1".to_owned(),
                    value: 1,
                    suffix: None,
                }),
                Expr::Literal(Literal::Bool(true)),
            ])),
            index: Box::new(Expr::Literal(Literal::Int {
                raw: "1".to_owned(),
                value: 1,
                suffix: None,
            })),
        };

        let lowered = lower_runtime_expr_strict(&expr).expect("tuple index lowers");

        assert!(matches!(
            lowered,
            RuntimeExpr::ProjectTuple { ordinal: 1, .. }
        ));
    }

    #[test]
    fn numeric_bracket_seq_lowers_to_dense_i64_sequence() {
        let expr = Expr::NumericBracketSeq(arcweft_lang_hir::syntax::expr::NumericBracketSeq::new(
            vec![1, 2, 3],
            None,
        ));

        let lowered = lower_runtime_expr_strict(&expr).expect("numeric seq lowers");

        assert!(matches!(
            lowered,
            RuntimeExpr::Value(RuntimeValue::Seq(seq))
                if seq.as_i64_slice() == Some([1, 2, 3].as_slice())
        ));
    }

    #[test]
    fn suffixed_numeric_bracket_seq_lowers_to_width_specific_dense_sequence() {
        let i8_lowered = lower_suffixed_numeric_seq("i8");
        let i16_lowered = lower_suffixed_numeric_seq("i16");
        let i32_lowered = lower_suffixed_numeric_seq("i32");
        let i128_lowered = lower_suffixed_numeric_seq("i128");
        let isize_lowered = lower_suffixed_numeric_seq("isize");
        let u8_lowered = lower_suffixed_numeric_seq("u8");
        let u16_lowered = lower_suffixed_numeric_seq("u16");
        let u32_lowered = lower_suffixed_numeric_seq("u32");
        let u64_lowered = lower_suffixed_numeric_seq("u64");
        let u128_lowered = lower_suffixed_numeric_seq("u128");
        let usize_lowered = lower_suffixed_numeric_seq("usize");

        assert!(matches!(
            i8_lowered,
            RuntimeExpr::Value(RuntimeValue::Seq(seq))
                if seq.as_i8_slice() == Some([1, 2, 3].as_slice())
        ));
        assert!(matches!(
            i16_lowered,
            RuntimeExpr::Value(RuntimeValue::Seq(seq))
                if seq.as_i16_slice() == Some([1, 2, 3].as_slice())
        ));
        assert!(matches!(
            i32_lowered,
            RuntimeExpr::Value(RuntimeValue::Seq(seq))
                if seq.as_i32_slice() == Some([1, 2, 3].as_slice())
        ));
        assert!(matches!(
            i128_lowered,
            RuntimeExpr::Value(RuntimeValue::Seq(seq))
                if seq.as_i128_slice() == Some([1, 2, 3].as_slice())
        ));
        assert!(matches!(
            isize_lowered,
            RuntimeExpr::Value(RuntimeValue::Seq(seq))
                if seq.as_isize_values() == Some([1, 2, 3].as_slice())
        ));
        assert!(matches!(
            u8_lowered,
            RuntimeExpr::Value(RuntimeValue::Seq(seq))
                if seq.as_u8_slice() == Some([1, 2, 3].as_slice())
        ));
        assert!(matches!(
            u16_lowered,
            RuntimeExpr::Value(RuntimeValue::Seq(seq))
                if seq.as_u16_slice() == Some([1, 2, 3].as_slice())
        ));
        assert!(matches!(
            u32_lowered,
            RuntimeExpr::Value(RuntimeValue::Seq(seq))
                if seq.as_u32_slice() == Some([1, 2, 3].as_slice())
        ));
        assert!(matches!(
            u64_lowered,
            RuntimeExpr::Value(RuntimeValue::Seq(seq))
                if seq.as_u64_slice() == Some([1, 2, 3].as_slice())
        ));
        assert!(matches!(
            u128_lowered,
            RuntimeExpr::Value(RuntimeValue::Seq(seq))
                if seq.as_u128_slice() == Some([1, 2, 3].as_slice())
        ));
        assert!(matches!(
            usize_lowered,
            RuntimeExpr::Value(RuntimeValue::Seq(seq))
                if seq.as_usize_values() == Some([1, 2, 3].as_slice())
        ));
    }

    fn lower_suffixed_numeric_seq(suffix: &str) -> RuntimeExpr {
        let expr = Expr::NumericBracketSeq(arcweft_lang_hir::syntax::expr::NumericBracketSeq::new(
            vec![1, 2, 3],
            Some(suffix.to_owned()),
        ));
        lower_runtime_expr_strict(&expr).expect("suffixed numeric seq lowers")
    }
}
