//! Host request lowering for awaited capability calls.
//!
//! This module keeps runtime-plan lowering Sans I/O: it recognizes capability
//! call shapes and emits data-only requests. Host adapters decide whether and
//! how those requests are executed.

use crate::errors::{RuntimeHostRequestArgument, RuntimePlanLowerContext, RuntimePlanLowerError};
use crate::expr::lower_runtime_expr_strict;
use crate::labels::{entity_ref_label, expr_label};
use arcweft_core::task::{HostTaskArgTemplate, HostTaskRequestTemplate};
use arcweft_core::value::{RuntimeCallTarget, RuntimeExpr, RuntimeFieldExpr, RuntimeValue};
use arcweft_lang_hir::syntax::ast::common::TextRange;
use arcweft_lang_hir::syntax::expr::{CallArg, Expr, Literal};
use thiserror::Error;

const AGENT_NAMED_ARGS_VARIANT: &str = "named_args";

struct CallParts<'a> {
    capability: String,
    operation: String,
    args: &'a [CallArg],
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum HostRequestLowerError {
    #[error("await target is not a host capability call: `{expression}`")]
    UnsupportedTarget { expression: String },
    #[error("host request `{capability}.{operation}` argument `{argument}`: {reason}")]
    Argument {
        capability: String,
        operation: String,
        argument: RuntimeHostRequestArgument,
        reason: String,
    },
}

impl From<HostRequestLowerError> for RuntimePlanLowerError {
    fn from(error: HostRequestLowerError) -> Self {
        error.into_runtime_error("host request", Vec::new(), None)
    }
}

impl HostRequestLowerError {
    pub(crate) fn into_runtime_error(
        self,
        owner: impl Into<String>,
        path: impl Into<Vec<String>>,
        source_range: Option<TextRange>,
    ) -> RuntimePlanLowerError {
        let owner = owner.into();
        let path = path.into();
        match self {
            HostRequestLowerError::UnsupportedTarget { expression } => {
                RuntimePlanLowerError::in_context(
                    RuntimePlanLowerContext::host_request_target(
                        owner,
                        path,
                        expression,
                        source_range,
                    ),
                    "await target must be a capability call",
                )
            }
            HostRequestLowerError::Argument {
                capability,
                operation,
                argument,
                reason,
            } => RuntimePlanLowerError::in_context(
                RuntimePlanLowerContext::host_request_argument(
                    owner,
                    path,
                    capability,
                    operation,
                    argument,
                    source_range,
                ),
                reason,
            ),
        }
    }
}

/// Lowers an awaited expression to a runtime-evaluable task request template.
pub(crate) fn lower_host_task_request(
    expr: &Expr,
) -> Result<HostTaskRequestTemplate, HostRequestLowerError> {
    let call = call_parts(expr).ok_or_else(|| HostRequestLowerError::UnsupportedTarget {
        expression: expr_label(expr),
    })?;
    let args = call
        .args
        .iter()
        .enumerate()
        .map(|(index, arg)| lower_arg_template(&call, index, arg))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(HostTaskRequestTemplate::new(
        call.capability.clone(),
        call.operation.clone(),
        args,
    ))
}

/// Lowers an Agent Prelude call expression to a Custom host task template.
///
/// Named arguments are preserved as a trailing runtime record payload because
/// the generic `HostTaskRequest::Custom` shape carries positional payloads.
pub(crate) fn lower_agent_host_task_request(
    expr: &Expr,
) -> Result<Option<HostTaskRequestTemplate>, HostRequestLowerError> {
    let Some(call) = agent_call_parts(expr) else {
        return Ok(None);
    };
    if call.operation == "wait" {
        return lower_agent_wait_task_request(call).map(Some);
    }
    let mut positional = Vec::new();
    let mut named = Vec::new();
    for (index, arg) in call.args.iter().enumerate() {
        let value = lower_agent_host_arg_expr(arg.value())
            .map_err(|reason| host_argument_error(&call, index, arg, reason))?;
        match arg {
            CallArg::Positional(_) => {
                positional.push(HostTaskArgTemplate::positional(value));
            }
            CallArg::Named { name, .. } => {
                named.push(RuntimeFieldExpr {
                    name: name.clone(),
                    value,
                });
            }
            CallArg::Spread { .. } => {
                positional.push(HostTaskArgTemplate::spread(value));
            }
        }
    }
    if !named.is_empty() {
        positional.push(HostTaskArgTemplate::positional(agent_named_args_expr(
            named,
        )));
    }
    Ok(Some(HostTaskRequestTemplate::new(
        "agent",
        call.operation,
        positional,
    )))
}

fn lower_agent_wait_task_request(
    call: CallParts<'_>,
) -> Result<HostTaskRequestTemplate, HostRequestLowerError> {
    let mut positional = Vec::new();
    let mut named = Vec::new();
    let mut positional_index = 0usize;
    for (index, arg) in call.args.iter().enumerate() {
        match arg {
            CallArg::Positional(value) => {
                let lowered = if positional_index == 0 {
                    lower_agent_predicate_expr(value)
                        .map_err(|reason| host_argument_error(&call, index, arg, reason))?
                        .map_or_else(|| lower_agent_host_arg_expr(value), Ok)
                } else {
                    lower_agent_host_arg_expr(value)
                }
                .map_err(|reason| host_argument_error(&call, index, arg, reason))?;
                positional.push(HostTaskArgTemplate::positional(lowered));
                positional_index += 1;
            }
            CallArg::Named { name, value } => {
                let lowered = if name == "predicate" {
                    lower_agent_predicate_expr(value)
                        .map_err(|reason| host_argument_error(&call, index, arg, reason))?
                        .map_or_else(|| lower_agent_host_arg_expr(value), Ok)
                } else {
                    lower_agent_host_arg_expr(value)
                }
                .map_err(|reason| host_argument_error(&call, index, arg, reason))?;
                named.push(RuntimeFieldExpr {
                    name: name.clone(),
                    value: lowered,
                });
            }
            CallArg::Spread { value } => {
                let value = lower_agent_host_arg_expr(value)
                    .map_err(|reason| host_argument_error(&call, index, arg, reason))?;
                positional.push(HostTaskArgTemplate::spread(value));
            }
        }
    }
    if !named.is_empty() {
        positional.push(HostTaskArgTemplate::positional(agent_named_args_expr(
            named,
        )));
    }
    Ok(HostTaskRequestTemplate::new(
        "agent",
        call.operation,
        positional,
    ))
}

fn call_parts(expr: &Expr) -> Option<CallParts<'_>> {
    match expr {
        Expr::Call(call) => {
            let name = expr_label(call.callee());
            let (capability, operation) = split_capability_operation(&name);
            Some(CallParts {
                capability,
                operation,
                args: call.args(),
            })
        }
        Expr::Await(awaited) => call_parts(awaited.operand()),
        Expr::Try(try_expr) => call_parts(try_expr.operand()),
        _ => None,
    }
}

fn agent_call_parts(expr: &Expr) -> Option<CallParts<'_>> {
    match expr {
        Expr::Call(call) => {
            let operation = match expr_label(call.callee()).as_str() {
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
                args: call.args(),
            })
        }
        Expr::Await(awaited) => agent_call_parts(awaited.operand()),
        Expr::Try(try_expr) => agent_call_parts(try_expr.operand()),
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

fn lower_arg_template(
    call: &CallParts<'_>,
    index: usize,
    arg: &CallArg,
) -> Result<HostTaskArgTemplate, HostRequestLowerError> {
    let value = lower_host_arg_expr(arg.value())
        .map_err(|reason| host_argument_error(call, index, arg, reason))?;
    Ok(match arg {
        CallArg::Named { name, .. } => HostTaskArgTemplate::named(name.clone(), value),
        CallArg::Spread { .. } => HostTaskArgTemplate::spread(value),
        CallArg::Positional(_) => HostTaskArgTemplate::positional(value),
    })
}

fn host_argument_error(
    call: &CallParts<'_>,
    index: usize,
    arg: &CallArg,
    reason: String,
) -> HostRequestLowerError {
    let argument = match arg {
        CallArg::Positional(_) => RuntimeHostRequestArgument::Positional(index),
        CallArg::Named { name, .. } => RuntimeHostRequestArgument::Named(name.clone()),
        CallArg::Spread { .. } => RuntimeHostRequestArgument::Spread(index),
    };
    HostRequestLowerError::Argument {
        capability: call.capability.clone(),
        operation: call.operation.clone(),
        argument,
        reason,
    }
}

fn lower_host_arg_expr(expr: &Expr) -> Result<RuntimeExpr, String> {
    match expr {
        Expr::Call(call)
            if matches!(
                expr_label(call.callee()).as_str(),
                "path.save" | "path.asset" | "path.temp" | "path.export"
            ) && call.args().len() == 1 =>
        {
            Ok(RuntimeExpr::Call {
                callee: RuntimeCallTarget::from_label(expr_label(call.callee())),
                args: vec![lower_host_arg_expr(call.args()[0].value())?],
            })
        }
        other => lower_runtime_expr_strict(other),
    }
}

fn lower_agent_host_arg_expr(expr: &Expr) -> Result<RuntimeExpr, String> {
    match expr {
        Expr::Call(call) if expr_label(call.callee()) == "viewport_point" => {
            lower_agent_viewport_point_expr(call.args())
        }
        Expr::ShortVariant(name) => Ok(RuntimeExpr::Value(RuntimeValue::String(name.to_string()))),
        _ => lower_host_arg_expr(expr),
    }
}

fn lower_agent_viewport_point_expr(args: &[CallArg]) -> Result<RuntimeExpr, String> {
    let mut fields = Vec::new();
    for (index, arg) in args.iter().enumerate() {
        match arg {
            CallArg::Positional(value) if index == 0 => {
                fields.push(runtime_field_expr("x", lower_agent_host_arg_expr(value)?));
            }
            CallArg::Positional(value) if index == 1 => {
                fields.push(runtime_field_expr("y", lower_agent_host_arg_expr(value)?));
            }
            CallArg::Named { name, value } if name == "x" || name == "y" => {
                fields.push(runtime_field_expr(name, lower_agent_host_arg_expr(value)?));
            }
            CallArg::Positional(value) => {
                fields.push(runtime_field_expr(
                    &format!("extra_{index}"),
                    lower_agent_host_arg_expr(value)?,
                ));
            }
            CallArg::Named { value, .. } | CallArg::Spread { value } => {
                fields.push(runtime_field_expr(
                    &format!("extra_{index}"),
                    lower_agent_host_arg_expr(value.as_ref())?,
                ));
            }
        }
    }
    Ok(runtime_record_expr(fields))
}

fn agent_named_args_expr(fields: Vec<RuntimeFieldExpr>) -> RuntimeExpr {
    RuntimeExpr::Variant {
        path: Some("agent".to_owned()),
        name: AGENT_NAMED_ARGS_VARIANT.to_owned(),
        payload: Some(Box::new(RuntimeExpr::Record(fields))),
    }
}

fn lower_agent_predicate_expr(expr: &Expr) -> Result<Option<RuntimeExpr>, String> {
    match expr {
        Expr::Call(call)
            if selected_callee_method(call.callee())
                .and_then(agent_compare_op)
                .is_some() =>
        {
            let receiver = selected_callee_receiver(call.callee())
                .ok_or_else(|| "agent comparison is missing its receiver".to_owned())?;
            let method = selected_callee_method(call.callee())
                .ok_or_else(|| "agent comparison is missing its method".to_owned())?;
            let [CallArg::Positional(value)] = call.args() else {
                return Err("agent comparison requires one positional value".to_owned());
            };
            Ok(Some(runtime_record_expr([
                runtime_field_expr("kind", runtime_string_expr("compare")),
                runtime_field_expr("probe", lower_agent_probe_expr(receiver)?),
                runtime_field_expr(
                    "op",
                    runtime_string_expr(
                        agent_compare_op(method)
                            .ok_or_else(|| "unsupported agent comparison method".to_owned())?,
                    ),
                ),
                runtime_field_expr("value", lower_agent_host_arg_expr(value)?),
            ])))
        }
        Expr::Call(call) if expr_label(call.callee()) == "exists" => {
            let [CallArg::Positional(probe)] = call.args() else {
                return Err("exists(...) requires one positional probe".to_owned());
            };
            Ok(Some(runtime_record_expr([
                runtime_field_expr("kind", runtime_string_expr("exists")),
                runtime_field_expr("probe", lower_agent_probe_expr(probe)?),
            ])))
        }
        Expr::Call(call) if expr_label(call.callee()) == "action_enabled" => {
            let [CallArg::Positional(target)] = call.args() else {
                return Err("action_enabled(...) requires one positional target".to_owned());
            };
            Ok(Some(runtime_record_expr([
                runtime_field_expr("kind", runtime_string_expr("action_enabled")),
                runtime_field_expr(
                    "target",
                    RuntimeExpr::Field {
                        target: Box::new(lower_agent_host_arg_expr(target)?),
                        field: "target".to_owned(),
                    },
                ),
            ])))
        }
        Expr::Call(call) if matches!(expr_label(call.callee()).as_str(), "all" | "any") => {
            let kind = expr_label(call.callee());
            let predicates = lower_agent_predicate_args(call.args())?;
            Ok(Some(runtime_record_expr([
                runtime_field_expr("kind", runtime_string_expr(&kind)),
                runtime_field_expr("predicates", RuntimeExpr::Tuple(predicates)),
            ])))
        }
        Expr::Call(call) if expr_label(call.callee()) == "not" => {
            let [CallArg::Positional(predicate)] = call.args() else {
                return Err("not(...) requires one positional predicate".to_owned());
            };
            let predicate = lower_agent_predicate_expr(predicate)?
                .ok_or_else(|| "not(...) argument is not an Agent predicate".to_owned())?;
            Ok(Some(runtime_record_expr([
                runtime_field_expr("kind", runtime_string_expr("not")),
                runtime_field_expr("predicate", predicate),
            ])))
        }
        Expr::Call(call)
            if selected_callee_method(call.callee()) == Some("has_error")
                && selected_callee_receiver(call.callee())
                    .is_some_and(is_agent_diagnostics_call)
                && call.args().is_empty() =>
        {
            Ok(Some(runtime_record_expr([runtime_field_expr(
                "kind",
                runtime_string_expr("diagnostics_has_error"),
            )])))
        }
        _ => Ok(None),
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
        Expr::Call(call)
            if expr_label(call.callee()) == "diagnostics" && call.args().is_empty()
    )
}

fn lower_agent_predicate_args(args: &[CallArg]) -> Result<Vec<RuntimeExpr>, String> {
    if let [CallArg::Positional(value)] = args
        && let Expr::BracketSeq(items) = value.as_ref()
    {
        return items
            .iter()
            .map(|item| {
                lower_agent_predicate_expr(item)?
                    .ok_or_else(|| "all/any sequence item is not an Agent predicate".to_owned())
            })
            .collect();
    }
    args.iter()
        .map(|arg| match arg {
            CallArg::Positional(value) => lower_agent_predicate_expr(value)?
                .ok_or_else(|| "all/any argument is not an Agent predicate".to_owned()),
            CallArg::Named { .. } | CallArg::Spread { .. } => {
                Err("all/any predicates must be positional".to_owned())
            }
        })
        .collect()
}

fn lower_agent_probe_expr(expr: &Expr) -> Result<RuntimeExpr, String> {
    match expr {
        Expr::Call(call) if matches!(expr_label(call.callee()).as_str(), "signal" | "metric") => {
            let [CallArg::Positional(target)] = call.args() else {
                return Err("signal/metric probe requires one positional target".to_owned());
            };
            Ok(runtime_record_expr([
                runtime_field_expr("kind", runtime_string_expr(&expr_label(call.callee()))),
                runtime_field_expr(
                    "target",
                    runtime_string_expr(
                        &agent_id_label(target)
                            .ok_or_else(|| "invalid signal/metric probe target".to_owned())?,
                    ),
                ),
            ]))
        }
        Expr::Call(call)
            if matches!(expr_label(call.callee()).as_str(), "state" | "observation") =>
        {
            let [CallArg::Positional(path)] = call.args() else {
                return Err("state/observation probe requires one path".to_owned());
            };
            let probe_kind = expr_label(call.callee());
            let constructor = match probe_kind.as_str() {
                "state" => "state_path",
                "observation" => "observation_path",
                _ => return Err("unknown Agent probe kind".to_owned()),
            };
            Ok(runtime_record_expr([
                runtime_field_expr("kind", runtime_string_expr(&probe_kind)),
                runtime_field_expr("path", lower_agent_path_expr(path, constructor)?),
            ]))
        }
        _ => Err(format!(
            "unsupported Agent probe expression `{}`",
            expr_label(expr)
        )),
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

fn lower_agent_path_expr(expr: &Expr, constructor: &str) -> Result<RuntimeExpr, String> {
    match expr {
        Expr::Call(call) if expr_label(call.callee()) == constructor => {
            let [CallArg::Positional(path)] = call.args() else {
                return Err(format!("{constructor}(...) requires one positional path"));
            };
            lower_agent_host_arg_expr(path)
        }
        _ => lower_agent_host_arg_expr(expr),
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

#[cfg(test)]
mod tests {
    use super::{HostRequestLowerError, lower_host_task_request};
    use crate::errors::{
        RuntimeHostRequestArgument, RuntimePlanLowerContext, RuntimePlanLowerError,
    };
    use arcweft_core::task::HostTaskArgTemplate;
    use arcweft_core::value::{RuntimeCallTarget, RuntimeExpr};
    use arcweft_lang_hir::syntax::expr::{Expr, parse_expr};

    #[test]
    fn host_request_rejects_non_call_target_instead_of_synthesizing_payload() {
        let error = lower_host_task_request(&Expr::Path("pending_value".into()))
            .expect_err("a non-call await target must be rejected");
        assert_eq!(
            error,
            HostRequestLowerError::UnsupportedTarget {
                expression: "pending_value".to_owned(),
            }
        );
        let runtime_error = RuntimePlanLowerError::from(error);
        assert!(matches!(
            runtime_error.context(),
            Some(RuntimePlanLowerContext::HostRequestTarget { expression, .. })
                if expression == "pending_value"
        ));
    }

    #[test]
    fn host_request_rejects_unlowerable_argument_with_typed_slot_context() {
        let request = parse_expr("storage.write(_)").expect("host request fixture parses");

        let error = lower_host_task_request(&request)
            .expect_err("an executable host argument must not become a string label");
        let HostRequestLowerError::Argument {
            capability,
            operation,
            argument,
            reason,
        } = error
        else {
            panic!("expected an argument error");
        };
        assert_eq!(capability, "storage");
        assert_eq!(operation, "write");
        assert_eq!(argument, RuntimeHostRequestArgument::Positional(0));
        assert!(
            reason.contains("partial placeholder is outside a runtime binding scope"),
            "{reason}"
        );
    }

    #[test]
    fn host_request_preserves_supported_typed_path_constructor() {
        let request = parse_expr("storage.write(path = path.save(\"slot-a\"))")
            .expect("typed path request fixture parses");

        let lowered = lower_host_task_request(&request).expect("typed path argument lowers");
        assert_eq!(lowered.capability.0, "storage");
        assert_eq!(lowered.operation, "write");
        assert!(matches!(
            lowered.args.as_slice(),
            [HostTaskArgTemplate::Named {
                name,
                value: RuntimeExpr::Call { callee, args },
            }] if name == "path"
                && callee == &RuntimeCallTarget::from_label("path.save")
                && matches!(args.as_slice(), [RuntimeExpr::Value(_)])
        ));
    }
}
