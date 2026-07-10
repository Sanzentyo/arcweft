//! Checked lowering for executable effect statements.

use super::{RuntimePureHelperLookup, lower_runtime_expr_strict_with_pure};
use crate::labels::{call_arg_label, expr_label, named_arg_label, named_arg_value};
use arcweft_core::effect::{
    LineEffectRequest, RuntimeAssertion, RuntimeAssertionProfile, RuntimeAssignment, RuntimeCall,
    RuntimeEffectExpr, RuntimeEffectFieldExpr, RuntimeEvent, RuntimeField, RuntimeLog,
};
use arcweft_core::value::{RuntimeExpr, RuntimeFieldExpr, RuntimeValue};
use arcweft_lang_hir::syntax::expr::{CallArg, Expr};

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum LoweredRuntimeEffect {
    Static(LineEffectRequest),
    Evaluated(RuntimeEffectExpr),
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

/// Lowers one executable effect without converting a failed value expression
/// back into a source-text payload.
///
/// Built-in effects own typed dynamic arguments. The generic
/// `LineEffectRequest::Call` ABI remains source-label based and therefore only
/// accepts arguments that are already closed runtime constants. A generic call
/// requiring runtime evaluation is rejected until it has a typed adapter/effect
/// boundary; named and spread arguments are never flattened into positional
/// dynamic values.
pub(crate) fn lower_runtime_effect_strict_with_pure(
    expr: &Expr,
    helpers: RuntimePureHelperLookup<'_, '_, '_>,
) -> Result<LoweredRuntimeEffect, String> {
    if let Some(command) = crate::audio::lower_audio_call_checked(expr) {
        return command.map(|command| {
            LoweredRuntimeEffect::Static(LineEffectRequest::Audio(Box::new(command)))
        });
    }
    match expr {
        Expr::Path(_) | Expr::ShortVariant(_) => {
            Ok(LoweredRuntimeEffect::Static(runtime_call_effect(expr)))
        }
        Expr::Call {
            callee,
            args: authored_args,
        } => {
            let lowered = lower_runtime_expr_strict_with_pure(expr, helpers)?;
            let (RuntimeExpr::Call { args, .. } | RuntimeExpr::MethodCall { args, .. }) = lowered
            else {
                return Err(format!(
                    "effect statement `{}` does not resolve to a host effect call",
                    expr_label(expr)
                ));
            };
            lower_effect_call(expr, &expr_label(callee), authored_args, args)
        }
        _ => Err(format!(
            "effect statement must be a host call or callable path, found `{}`",
            expr_label(expr)
        )),
    }
}

fn lower_effect_call(
    authored_expr: &Expr,
    callee: &str,
    authored_args: &[CallArg],
    args: Vec<RuntimeExpr>,
) -> Result<LoweredRuntimeEffect, String> {
    let evaluated = match callee {
        "signal.set" | "metric.set" => {
            let mut args = bind_fixed_effect_args(
                callee,
                authored_args,
                args,
                &[("target", true), ("value", true)],
            )?;
            let target = take_bound_arg(&mut args, 0, callee, "target")?;
            let value = take_bound_arg(&mut args, 1, callee, "value")?;
            if callee == "signal.set" {
                RuntimeEffectExpr::SignalWrite { target, value }
            } else {
                RuntimeEffectExpr::MetricWrite { target, value }
            }
        }
        "event.emit" => {
            let (event, fields) = bind_field_effect_args(callee, "event", authored_args, args)?;
            RuntimeEffectExpr::EmitEvent { event, fields }
        }
        "panic" | "fail" | "bail" => {
            let mut args =
                bind_fixed_effect_args(callee, authored_args, args, &[("message", true)])?;
            let message = take_bound_arg(&mut args, 0, callee, "message")?;
            match callee {
                "panic" => RuntimeEffectExpr::Panic(message),
                "fail" => RuntimeEffectExpr::Fail(message),
                "bail" => RuntimeEffectExpr::Bail(message),
                _ => unreachable!("matched control effect callee"),
            }
        }
        "ensure" | "assert" | "debug_assert" => {
            let mut args = bind_fixed_effect_args(
                callee,
                authored_args,
                args,
                &[("condition", true), ("message", false)],
            )?;
            let condition = take_bound_arg(&mut args, 0, callee, "condition")?;
            let message = args[1].take().unwrap_or_else(|| {
                RuntimeExpr::Value(RuntimeValue::String("assertion failed".to_owned()))
            });
            if callee == "ensure" {
                RuntimeEffectExpr::Ensure { condition, message }
            } else {
                RuntimeEffectExpr::Assert {
                    condition,
                    message,
                    profile: if callee == "debug_assert" {
                        RuntimeAssertionProfile::DebugOnly
                    } else {
                        RuntimeAssertionProfile::Always
                    },
                }
            }
        }
        callee if callee.starts_with("log.") => {
            let (message, fields) = bind_field_effect_args(callee, "message", authored_args, args)?;
            RuntimeEffectExpr::Log {
                level: callee.trim_start_matches("log.").to_owned(),
                message,
                fields,
            }
        }
        _ => {
            if args.iter().all(is_closed_generic_effect_arg) {
                return Ok(LoweredRuntimeEffect::Static(runtime_call_effect(
                    authored_expr,
                )));
            }
            return Err(format!(
                "generic effect call `{callee}` has runtime-valued arguments but no typed effect boundary"
            ));
        }
    };
    Ok(LoweredRuntimeEffect::Evaluated(evaluated))
}

fn bind_fixed_effect_args(
    callee: &str,
    authored_args: &[CallArg],
    values: Vec<RuntimeExpr>,
    parameters: &[(&str, bool)],
) -> Result<Vec<Option<RuntimeExpr>>, String> {
    if authored_args.len() != values.len() {
        return Err(format!(
            "{callee} argument shape changed during runtime lowering"
        ));
    }
    let mut bound = vec![None; parameters.len()];
    let mut next_positional = 0;
    for (argument, value) in authored_args.iter().zip(values) {
        let index = match argument {
            CallArg::Positional(_) => {
                let Some(index) =
                    (next_positional..bound.len()).find(|index| bound[*index].is_none())
                else {
                    return Err(format!("{callee} received too many positional arguments"));
                };
                next_positional = index.saturating_add(1);
                index
            }
            CallArg::Named { name, .. } => parameters
                .iter()
                .position(|(parameter, _)| parameter == name)
                .ok_or_else(|| format!("{callee} has no `{name}` argument"))?,
            CallArg::Spread { .. } => {
                return Err(format!(
                    "{callee} does not accept spread arguments at the effect boundary"
                ));
            }
        };
        if bound[index].replace(value).is_some() {
            return Err(format!(
                "{callee} argument `{}` was provided more than once",
                parameters[index].0
            ));
        }
    }
    for (index, (name, required)) in parameters.iter().enumerate() {
        if *required && bound[index].is_none() {
            return Err(format!("{callee} requires a `{name}` argument"));
        }
    }
    Ok(bound)
}

fn take_bound_arg(
    args: &mut [Option<RuntimeExpr>],
    index: usize,
    callee: &str,
    name: &str,
) -> Result<RuntimeExpr, String> {
    args.get_mut(index)
        .and_then(Option::take)
        .ok_or_else(|| format!("{callee} requires a `{name}` argument"))
}

fn bind_field_effect_args(
    callee: &str,
    head_name: &str,
    authored_args: &[CallArg],
    values: Vec<RuntimeExpr>,
) -> Result<(RuntimeExpr, Vec<RuntimeEffectFieldExpr>), String> {
    if authored_args.len() != values.len() {
        return Err(format!(
            "{callee} argument shape changed during runtime lowering"
        ));
    }
    let mut head = None;
    let mut fields = Vec::new();
    for (index, (argument, value)) in authored_args.iter().zip(values).enumerate() {
        match argument {
            CallArg::Named { name, .. } if name == head_name => {
                if head.replace(value).is_some() {
                    return Err(format!(
                        "{callee} argument `{head_name}` was provided more than once"
                    ));
                }
            }
            CallArg::Positional(_) if head.is_none() => head = Some(value),
            CallArg::Named { name, .. } => fields.push(RuntimeEffectFieldExpr {
                name: name.clone(),
                value,
            }),
            CallArg::Positional(_) => fields.push(RuntimeEffectFieldExpr {
                name: format!("arg{index}"),
                value,
            }),
            CallArg::Spread { .. } => {
                return Err(format!(
                    "{callee} does not accept spread fields at the effect boundary"
                ));
            }
        }
    }
    head.map(|head| (head, fields))
        .ok_or_else(|| format!("{callee} requires a `{head_name}` argument"))
}

fn is_closed_generic_effect_arg(expr: &RuntimeExpr) -> bool {
    match expr {
        RuntimeExpr::Value(_) | RuntimeExpr::EntityRef(_) => true,
        RuntimeExpr::Tuple(items) | RuntimeExpr::BracketSeq(items) => {
            items.iter().all(is_closed_generic_effect_arg)
        }
        RuntimeExpr::RepeatSeq { value, .. } | RuntimeExpr::SpreadArg(value) => {
            is_closed_generic_effect_arg(value)
        }
        RuntimeExpr::Range { start, end, .. } => {
            start.as_deref().is_none_or(is_closed_generic_effect_arg)
                && end.as_deref().is_none_or(is_closed_generic_effect_arg)
        }
        RuntimeExpr::Record(fields) => fields
            .iter()
            .all(|RuntimeFieldExpr { value, .. }| is_closed_generic_effect_arg(value)),
        RuntimeExpr::Variant { payload, .. } => {
            payload.as_deref().is_none_or(is_closed_generic_effect_arg)
        }
        RuntimeExpr::Local(_)
        | RuntimeExpr::Let { .. }
        | RuntimeExpr::Field { .. }
        | RuntimeExpr::ProjectTuple { .. }
        | RuntimeExpr::ProjectRecord { .. }
        | RuntimeExpr::AssignField { .. }
        | RuntimeExpr::Call { .. }
        | RuntimeExpr::Function { .. }
        | RuntimeExpr::Apply { .. }
        | RuntimeExpr::TraitCall { .. }
        | RuntimeExpr::PureCall { .. }
        | RuntimeExpr::MethodCall { .. }
        | RuntimeExpr::Map { .. }
        | RuntimeExpr::Filter { .. }
        | RuntimeExpr::Sum { .. }
        | RuntimeExpr::Unary { .. }
        | RuntimeExpr::Binary { .. }
        | RuntimeExpr::If { .. }
        | RuntimeExpr::IfLet { .. }
        | RuntimeExpr::Match { .. } => false,
    }
}

fn runtime_call(expr: &Expr) -> RuntimeCall {
    match expr {
        Expr::Call { callee, args } => RuntimeCall {
            callee: expr_label(callee),
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
            .map(|(index, value)| RuntimeField {
                name: named_arg_label(value).unwrap_or_else(|| format!("arg{index}")),
                value: named_arg_value(value).unwrap_or_else(|| value.clone()),
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
            .map(|(index, value)| RuntimeField {
                name: named_arg_label(value).unwrap_or_else(|| format!("arg{index}")),
                value: named_arg_value(value).unwrap_or_else(|| value.clone()),
            })
            .collect(),
    })
}
