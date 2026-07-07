//! Host request lowering for awaited capability calls.
//!
//! This module keeps runtime-plan lowering Sans I/O: it recognizes capability
//! call shapes and emits data-only requests. Host adapters decide whether and
//! how those requests are executed.

use crate::expr::lower_runtime_expr_strict;
use crate::labels::{entity_ref_label, expr_label};
use arcweft_core::task::{HostTaskArgTemplate, HostTaskRequestTemplate};
use arcweft_core::value::{RuntimeCallTarget, RuntimeExpr, RuntimeFieldExpr, RuntimeValue};
use arcweft_lang_hir::syntax::expr::{CallArg, Expr, Literal};

const AGENT_NAMED_ARGS_VARIANT: &str = "named_args";

struct CallParts<'a> {
    capability: String,
    operation: String,
    args: &'a [CallArg],
}

/// Lowers an awaited expression to a runtime-evaluable task request template.
pub(crate) fn lower_host_task_request(expr: &Expr) -> HostTaskRequestTemplate {
    let Some(call) = call_parts(expr) else {
        return HostTaskRequestTemplate::new(
            "await",
            "expr",
            [HostTaskArgTemplate::positional(RuntimeExpr::Value(
                RuntimeValue::String(expr_label(expr)),
            ))],
        );
    };
    HostTaskRequestTemplate::new(
        call.capability.clone(),
        call.operation.clone(),
        call.args.iter().map(lower_arg_template),
    )
}

/// Lowers an Agent Prelude call expression to a Custom host task template.
///
/// Named arguments are preserved as a trailing runtime record payload because
/// the generic `HostTaskRequest::Custom` shape carries positional payloads.
pub(crate) fn lower_agent_host_task_request(expr: &Expr) -> Option<HostTaskRequestTemplate> {
    let call = agent_call_parts(expr)?;
    if call.operation == "wait" {
        return Some(lower_agent_wait_task_request(call));
    }
    let mut positional = Vec::new();
    let mut named = Vec::new();
    for arg in call.args {
        match arg {
            CallArg::Positional(value) => {
                positional.push(HostTaskArgTemplate::positional(lower_agent_host_arg_expr(
                    value,
                )));
            }
            CallArg::Named { name, value } => {
                named.push(RuntimeFieldExpr {
                    name: name.clone(),
                    value: lower_agent_host_arg_expr(value),
                });
            }
            CallArg::Spread { value } => {
                positional.push(HostTaskArgTemplate::spread(lower_agent_host_arg_expr(
                    value,
                )));
            }
        }
    }
    if !named.is_empty() {
        positional.push(HostTaskArgTemplate::positional(agent_named_args_expr(
            named,
        )));
    }
    Some(HostTaskRequestTemplate::new(
        "agent",
        call.operation,
        positional,
    ))
}

fn lower_agent_wait_task_request(call: CallParts<'_>) -> HostTaskRequestTemplate {
    let mut positional = Vec::new();
    let mut named = Vec::new();
    let mut positional_index = 0usize;
    for arg in call.args {
        match arg {
            CallArg::Positional(value) => {
                let lowered = if positional_index == 0 {
                    lower_agent_predicate_expr(value)
                        .unwrap_or_else(|| lower_agent_host_arg_expr(value))
                } else {
                    lower_agent_host_arg_expr(value)
                };
                positional.push(HostTaskArgTemplate::positional(lowered));
                positional_index += 1;
            }
            CallArg::Named { name, value } => {
                let lowered = if name == "predicate" {
                    lower_agent_predicate_expr(value)
                        .unwrap_or_else(|| lower_agent_host_arg_expr(value))
                } else {
                    lower_agent_host_arg_expr(value)
                };
                named.push(RuntimeFieldExpr {
                    name: name.clone(),
                    value: lowered,
                });
            }
            CallArg::Spread { value } => {
                positional.push(HostTaskArgTemplate::spread(lower_agent_host_arg_expr(
                    value,
                )));
            }
        }
    }
    if !named.is_empty() {
        positional.push(HostTaskArgTemplate::positional(agent_named_args_expr(
            named,
        )));
    }
    HostTaskRequestTemplate::new("agent", call.operation, positional)
}

fn call_parts(expr: &Expr) -> Option<CallParts<'_>> {
    match expr {
        Expr::Call { callee, args } => {
            let name = expr_label(callee);
            let (capability, operation) = split_capability_operation(&name);
            Some(CallParts {
                capability,
                operation,
                args,
            })
        }
        Expr::Await { expr, .. } | Expr::Try { expr } => call_parts(expr),
        _ => None,
    }
}

fn agent_call_parts(expr: &Expr) -> Option<CallParts<'_>> {
    match expr {
        Expr::Call { callee, args } => {
            let operation = match expr_label(callee).as_str() {
                "observe" => "observe",
                "advance_text" => "advance_text",
                "capture" => "capture",
                "entity_meta" => "entity_meta",
                "project_neighbors" => "project_neighbors",
                "wait" => "wait",
                "choose" => "choose",
                "invoke" => "invoke",
                "read_resource" => "read_resource",
                "attach" => "attach",
                "expect" => "expect",
                "deny" => "deny",
                "pointer.click" => "pointer.click",
                "rag.query" => "rag.query",
                _ => return None,
            };
            Some(CallParts {
                capability: "agent".to_owned(),
                operation: operation.to_owned(),
                args,
            })
        }
        Expr::Await { expr, .. } | Expr::Try { expr } => agent_call_parts(expr),
        _ => None,
    }
}

fn split_capability_operation(name: &str) -> (String, String) {
    name.rsplit_once('.').map_or_else(
        || ("await".to_owned(), name.to_owned()),
        |(capability, operation)| (capability.to_owned(), operation.to_owned()),
    )
}

fn method_name(method: &str) -> &str {
    method.split_once('<').map_or(method, |(name, _)| name)
}

fn agent_compare_op(method: &str) -> Option<&'static str> {
    Some(match method {
        "eq" => "eq",
        "ne" | "not_eq" => "not_eq",
        "gt" | "greater" => "greater",
        "ge" | "greater_or_equal" => "greater_or_equal",
        "lt" | "less" => "less",
        "le" | "less_or_equal" => "less_or_equal",
        _ => return None,
    })
}

fn lower_arg_template(arg: &CallArg) -> HostTaskArgTemplate {
    match arg {
        CallArg::Named { name, value } => {
            HostTaskArgTemplate::named(name.clone(), lower_host_arg_expr(value))
        }
        CallArg::Spread { value } => HostTaskArgTemplate::spread(lower_host_arg_expr(value)),
        CallArg::Positional(value) => HostTaskArgTemplate::positional(lower_host_arg_expr(value)),
    }
}

fn lower_host_arg_expr(expr: &Expr) -> RuntimeExpr {
    match expr {
        Expr::Call { callee, args }
            if matches!(
                expr_label(callee).as_str(),
                "path.save" | "path.asset" | "path.temp" | "path.export"
            ) && args.len() == 1 =>
        {
            RuntimeExpr::Call {
                callee: RuntimeCallTarget::from_label(expr_label(callee)),
                args: vec![lower_host_arg_expr(args[0].value())],
            }
        }
        other => lower_runtime_expr_strict(other)
            .unwrap_or_else(|_| RuntimeExpr::Value(RuntimeValue::String(expr_label(other)))),
    }
}

fn lower_agent_host_arg_expr(expr: &Expr) -> RuntimeExpr {
    match expr {
        Expr::Call { callee, args } if expr_label(callee) == "viewport_point" => {
            lower_agent_viewport_point_expr(args)
        }
        Expr::ShortVariant(name) => RuntimeExpr::Value(RuntimeValue::String(name.to_string())),
        _ => lower_host_arg_expr(expr),
    }
}

fn lower_agent_viewport_point_expr(args: &[CallArg]) -> RuntimeExpr {
    let mut fields = Vec::new();
    for (index, arg) in args.iter().enumerate() {
        match arg {
            CallArg::Positional(value) if index == 0 => {
                fields.push(runtime_field_expr("x", lower_agent_host_arg_expr(value)));
            }
            CallArg::Positional(value) if index == 1 => {
                fields.push(runtime_field_expr("y", lower_agent_host_arg_expr(value)));
            }
            CallArg::Named { name, value } if name == "x" || name == "y" => {
                fields.push(runtime_field_expr(name, lower_agent_host_arg_expr(value)));
            }
            CallArg::Positional(value) => {
                fields.push(runtime_field_expr(
                    &format!("extra_{index}"),
                    lower_agent_host_arg_expr(value),
                ));
            }
            CallArg::Named { value, .. } | CallArg::Spread { value } => {
                fields.push(runtime_field_expr(
                    &format!("extra_{index}"),
                    lower_agent_host_arg_expr(value.as_ref()),
                ));
            }
        }
    }
    runtime_record_expr(fields)
}

fn agent_named_args_expr(fields: Vec<RuntimeFieldExpr>) -> RuntimeExpr {
    RuntimeExpr::Variant {
        path: Some("agent".to_owned()),
        name: AGENT_NAMED_ARGS_VARIANT.to_owned(),
        payload: Some(Box::new(RuntimeExpr::Record(fields))),
    }
}

fn lower_agent_predicate_expr(expr: &Expr) -> Option<RuntimeExpr> {
    match expr {
        Expr::Call { callee, args }
            if selected_callee_method(callee)
                .and_then(agent_compare_op)
                .is_some() =>
        {
            let receiver = selected_callee_receiver(callee)?;
            let method = selected_callee_method(callee)?;
            let [CallArg::Positional(value)] = args.as_slice() else {
                return None;
            };
            Some(runtime_record_expr([
                runtime_field_expr("kind", runtime_string_expr("compare")),
                runtime_field_expr("probe", lower_agent_probe_expr(receiver)?),
                runtime_field_expr("op", runtime_string_expr(agent_compare_op(method)?)),
                runtime_field_expr("value", lower_agent_host_arg_expr(value)),
            ]))
        }
        Expr::Call { callee, args } if expr_label(callee) == "exists" => {
            let [CallArg::Positional(probe)] = args.as_slice() else {
                return None;
            };
            Some(runtime_record_expr([
                runtime_field_expr("kind", runtime_string_expr("exists")),
                runtime_field_expr("probe", lower_agent_probe_expr(probe)?),
            ]))
        }
        Expr::Call { callee, args } if expr_label(callee) == "action_enabled" => {
            let [CallArg::Positional(target)] = args.as_slice() else {
                return None;
            };
            Some(runtime_record_expr([
                runtime_field_expr("kind", runtime_string_expr("action_enabled")),
                runtime_field_expr(
                    "target",
                    RuntimeExpr::Field {
                        target: Box::new(lower_agent_host_arg_expr(target)),
                        field: "target".to_owned(),
                    },
                ),
            ]))
        }
        Expr::Call { callee, args } if matches!(expr_label(callee).as_str(), "all" | "any") => {
            let kind = expr_label(callee);
            let predicates = lower_agent_predicate_args(args)?;
            Some(runtime_record_expr([
                runtime_field_expr("kind", runtime_string_expr(&kind)),
                runtime_field_expr("predicates", RuntimeExpr::Tuple(predicates)),
            ]))
        }
        Expr::Call { callee, args } if expr_label(callee) == "not" => {
            let [CallArg::Positional(predicate)] = args.as_slice() else {
                return None;
            };
            Some(runtime_record_expr([
                runtime_field_expr("kind", runtime_string_expr("not")),
                runtime_field_expr("predicate", lower_agent_predicate_expr(predicate)?),
            ]))
        }
        Expr::Call { callee, args }
            if selected_callee_method(callee) == Some("has_error")
                && selected_callee_receiver(callee).is_some_and(is_agent_diagnostics_call)
                && args.is_empty() =>
        {
            Some(runtime_record_expr([runtime_field_expr(
                "kind",
                runtime_string_expr("diagnostics_has_error"),
            )]))
        }
        _ => None,
    }
}

fn selected_callee_receiver(expr: &Expr) -> Option<&Expr> {
    let Expr::Select(select) = expr else {
        return None;
    };
    Some(select.target())
}

fn selected_callee_method(expr: &Expr) -> Option<&str> {
    let Expr::Select(select) = expr else {
        return None;
    };
    Some(method_name(select.member().as_str()))
}

fn is_agent_diagnostics_call(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Call { callee, args } if expr_label(callee) == "diagnostics" && args.is_empty()
    )
}

fn lower_agent_predicate_args(args: &[CallArg]) -> Option<Vec<RuntimeExpr>> {
    if let [CallArg::Positional(Expr::BracketSeq(items))] = args {
        return items.iter().map(lower_agent_predicate_expr).collect();
    }
    args.iter()
        .map(|arg| match arg {
            CallArg::Positional(value) => lower_agent_predicate_expr(value),
            CallArg::Named { .. } | CallArg::Spread { .. } => None,
        })
        .collect()
}

fn lower_agent_probe_expr(expr: &Expr) -> Option<RuntimeExpr> {
    match expr {
        Expr::Call { callee, args }
            if matches!(expr_label(callee).as_str(), "signal" | "metric") =>
        {
            let [CallArg::Positional(target)] = args.as_slice() else {
                return None;
            };
            Some(runtime_record_expr([
                runtime_field_expr("kind", runtime_string_expr(&expr_label(callee))),
                runtime_field_expr("target", runtime_string_expr(&agent_id_label(target)?)),
            ]))
        }
        Expr::Call { callee, args }
            if matches!(expr_label(callee).as_str(), "state" | "observation") =>
        {
            let [CallArg::Positional(path)] = args.as_slice() else {
                return None;
            };
            let probe_kind = expr_label(callee);
            let constructor = match probe_kind.as_str() {
                "state" => "state_path",
                "observation" => "observation_path",
                _ => return None,
            };
            Some(runtime_record_expr([
                runtime_field_expr("kind", runtime_string_expr(&probe_kind)),
                runtime_field_expr("path", lower_agent_path_expr(path, constructor)?),
            ]))
        }
        _ => None,
    }
}

fn agent_id_label(expr: &Expr) -> Option<String> {
    match expr {
        Expr::EntityRef(entity) => Some(entity_ref_label(entity)),
        Expr::Path(path) => Some(path.trim_start_matches('@').to_owned()),
        Expr::Literal(Literal::String(value)) => Some(value.clone()),
        _ => None,
    }
}

fn lower_agent_path_expr(expr: &Expr, constructor: &str) -> Option<RuntimeExpr> {
    match expr {
        Expr::Call { callee, args } if expr_label(callee) == constructor => {
            let [CallArg::Positional(path)] = args.as_slice() else {
                return None;
            };
            Some(lower_agent_host_arg_expr(path))
        }
        _ => Some(lower_agent_host_arg_expr(expr)),
    }
}

fn runtime_record_expr(fields: impl IntoIterator<Item = RuntimeFieldExpr>) -> RuntimeExpr {
    RuntimeExpr::Record(fields.into_iter().collect())
}

fn runtime_field_expr(name: &str, value: RuntimeExpr) -> RuntimeFieldExpr {
    RuntimeFieldExpr {
        name: name.to_owned(),
        value,
    }
}

fn runtime_string_expr(value: &str) -> RuntimeExpr {
    RuntimeExpr::Value(RuntimeValue::String(value.to_owned()))
}
