use crate::labels::expr_label;
use arcweft_core::plan::FlowRuntimeId;
use arcweft_lang_hir::syntax::{
    ast::{dialogue::DialogueContent, ids::EntityRef, line_plan::LinePlan},
    expr::{CallArg, Expr, Literal},
};

pub(super) fn dialogue_call_parts(
    expr: &Expr,
) -> Option<(&Expr, &DialogueContent, Option<&LinePlan>)> {
    match expr {
        Expr::DialogueCall {
            callee,
            content,
            plan,
        } => Some((callee.as_ref(), content.as_ref(), plan.as_ref())),
        Expr::Try(try_expr) => dialogue_call_parts(try_expr.operand()),
        _ => None,
    }
}

pub(crate) fn sanitize_task_id_part(name: &str) -> String {
    name.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

pub(super) fn agent_task_name(expr: &Expr) -> String {
    expr_label(expr)
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '.'
            }
        })
        .collect::<String>()
        .trim_matches('.')
        .to_owned()
}

pub(super) fn flow_runtime_id(id: &EntityRef) -> FlowRuntimeId {
    FlowRuntimeId::from_runtime_target_value(id.body()).expect("HIR flow ID should be valid")
}

pub(super) fn method_name(method: &str) -> &str {
    method.split_once('<').map_or(method, |(name, _)| name)
}

pub(super) fn selected_call_parts(expr: &Expr) -> Option<(&Expr, &str, &[CallArg])> {
    let Expr::Call(call) = expr else {
        return None;
    };
    let Expr::Select(select) = call.callee() else {
        return None;
    };
    Some((select.target(), select.member().as_str(), call.args()))
}

pub(super) fn traverse_callee(args: &[CallArg]) -> Result<&Expr, String> {
    let [arg] = args else {
        return Err("traverse(...) requires exactly one positional task function".to_owned());
    };
    if arg.name().is_some() || arg.is_spread() {
        return Err("traverse(...) task function must be a positional argument".to_owned());
    }
    Ok(arg.value())
}

pub(super) fn split_capability_operation(name: &str) -> Result<(String, String), String> {
    name.rsplit_once('.').map_or_else(
        || {
            Err(format!(
                "traverse task function `{name}` must be capability-qualified"
            ))
        },
        |(capability, operation)| Ok((capability.to_owned(), operation.to_owned())),
    )
}

pub(super) fn parallel_limit(args: &[CallArg]) -> Result<usize, String> {
    let [arg] = args else {
        return Err("parallel(...) requires exactly `limit = N`".to_owned());
    };
    if arg.name() != Some("limit") || arg.is_spread() {
        return Err("parallel(...) requires a named `limit = N` argument".to_owned());
    }
    let Expr::Literal(Literal::Int(literal)) = arg.value() else {
        return Err("parallel limit must be an integer literal".to_owned());
    };
    literal
        .magnitude()
        .ok()
        .and_then(|value| usize::try_from(value).ok())
        .filter(|value| *value > 0)
        .ok_or_else(|| "parallel limit must be greater than zero".to_owned())
}
