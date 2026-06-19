//! Host request lowering for awaited capability calls.
//!
//! This module keeps runtime-plan lowering Sans I/O: it recognizes capability
//! call shapes and emits data-only requests. Host adapters decide whether and
//! how those requests are executed.

use crate::expr::lower_runtime_expr_strict;
use crate::labels::expr_label;
use arcweft_core::task::{HostTaskArgTemplate, HostTaskRequestTemplate};
use arcweft_core::value::{RuntimeCallTarget, RuntimeExpr, RuntimeFieldExpr, RuntimeValue};
use arcweft_lang_hir::syntax::expr::{CallArg, Expr};

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
        positional.push(HostTaskArgTemplate::positional(RuntimeExpr::Record(named)));
    }
    Some(HostTaskRequestTemplate::new(
        "agent",
        call.operation,
        positional,
    ))
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
        Expr::Path(path) if path.starts_with('.') => {
            RuntimeExpr::Value(RuntimeValue::String(path.to_owned()))
        }
        _ => lower_host_arg_expr(expr),
    }
}
