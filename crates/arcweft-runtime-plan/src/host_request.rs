//! Host request lowering for awaited capability calls.
//!
//! This module keeps runtime-plan lowering Sans I/O: it recognizes capability
//! call shapes and emits data-only requests. Host adapters decide whether and
//! how those requests are executed.

use crate::expr::lower_runtime_expr_strict;
use crate::labels::expr_label;
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
        Expr::MethodCall {
            receiver,
            method,
            args,
        } => Some(CallParts {
            capability: expr_label(receiver),
            operation: method_name(method).to_owned(),
            args,
        }),
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
                "wait" => "wait",
                "choose" => "choose",
                "invoke" => "invoke",
                "read_resource" => "read_resource",
                _ => return None,
            };
            Some(CallParts {
                capability: "agent".to_owned(),
                operation: operation.to_owned(),
                args,
            })
        }
        Expr::MethodCall {
            receiver,
            method,
            args,
        } if expr_label(receiver) == "pointer" && method_name(method) == "click" => {
            Some(CallParts {
                capability: "agent".to_owned(),
                operation: "pointer.click".to_owned(),
                args,
            })
        }
        Expr::MethodCall {
            receiver,
            method,
            args,
        } if expr_label(receiver) == "rag" && method_name(method) == "query" => Some(CallParts {
            capability: "agent".to_owned(),
            operation: "rag.query".to_owned(),
            args,
        }),
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
        Expr::MethodCall {
            receiver,
            method,
            args,
        } if matches!(receiver.as_ref(), Expr::Path(path) if path == "path")
            && matches!(method.as_str(), "save" | "asset" | "temp" | "export")
            && args.len() == 1 =>
        {
            RuntimeExpr::Call {
                callee: RuntimeCallTarget::from_label(format!("path.{method}")),
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
        Expr::Path(path) if path.starts_with('.') => {
            RuntimeExpr::Value(RuntimeValue::String(path.to_owned()))
        }
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
        Expr::MethodCall {
            receiver,
            method,
            args,
        } if agent_compare_op(method_name(method)).is_some() => {
            let [CallArg::Positional(value)] = args.as_slice() else {
                return None;
            };
            Some(runtime_record_expr([
                runtime_field_expr("kind", runtime_string_expr("compare")),
                runtime_field_expr("probe", lower_agent_probe_expr(receiver)?),
                runtime_field_expr(
                    "op",
                    runtime_string_expr(agent_compare_op(method_name(method))?),
                ),
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
        Expr::Call { callee, args } if matches!(expr_label(callee).as_str(), "all" | "any") => {
            let kind = expr_label(callee);
            let predicates = args
                .iter()
                .map(|arg| match arg {
                    CallArg::Positional(value) => lower_agent_predicate_expr(value),
                    CallArg::Named { .. } | CallArg::Spread { .. } => None,
                })
                .collect::<Option<Vec<_>>>()?;
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
        _ => None,
    }
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
            Some(runtime_record_expr([
                runtime_field_expr("kind", runtime_string_expr(&expr_label(callee))),
                runtime_field_expr("path", runtime_string_expr(&agent_string_label(path)?)),
            ]))
        }
        _ => None,
    }
}

fn agent_id_label(expr: &Expr) -> Option<String> {
    match expr {
        Expr::EntityRef(entity) => Some(entity.body().to_owned()),
        Expr::Path(path) => Some(path.trim_start_matches('@').to_owned()),
        Expr::Literal(Literal::String(value)) => Some(value.clone()),
        _ => None,
    }
}

fn agent_string_label(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Literal(Literal::String(value)) => Some(value.clone()),
        Expr::Path(path) => Some(path.clone()),
        Expr::EntityRef(entity) => Some(entity.body().to_owned()),
        _ => None,
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
