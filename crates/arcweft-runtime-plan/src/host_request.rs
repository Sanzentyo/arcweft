//! Host request lowering for awaited capability calls.
//!
//! This module keeps runtime-plan lowering Sans I/O: it recognizes capability
//! call shapes and emits data-only requests. Host adapters decide whether and
//! how those requests are executed.

use crate::expr::lower_runtime_expr_strict;
use crate::labels::expr_label;
use arcweft_core::task::{HostTaskArgTemplate, HostTaskRequestTemplate};
use arcweft_core::value::{RuntimeExpr, RuntimeValue};
use arcweft_lang_hir::syntax::expr::Expr;

struct CallParts<'a> {
    capability: String,
    operation: String,
    args: &'a [Expr],
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

fn split_capability_operation(name: &str) -> (String, String) {
    name.rsplit_once('.').map_or_else(
        || ("await".to_owned(), name.to_owned()),
        |(capability, operation)| (capability.to_owned(), operation.to_owned()),
    )
}

fn method_name(method: &str) -> &str {
    method.split_once('<').map_or(method, |(name, _)| name)
}

fn lower_arg_template(arg: &Expr) -> HostTaskArgTemplate {
    match arg {
        Expr::NamedArg { name, value } => {
            HostTaskArgTemplate::named(name.clone(), lower_host_arg_expr(value))
        }
        value => HostTaskArgTemplate::positional(lower_host_arg_expr(value)),
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
                callee: format!("path.{method}"),
                args: vec![lower_host_arg_expr(&args[0])],
            }
        }
        other => lower_runtime_expr_strict(other)
            .unwrap_or_else(|_| RuntimeExpr::Value(RuntimeValue::String(expr_label(other)))),
    }
}
