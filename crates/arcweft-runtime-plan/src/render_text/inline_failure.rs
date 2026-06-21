use arcweft_lang_hir::syntax::ast::dialogue::LineArg;
use arcweft_lang_hir::syntax::expr::{CallArg, Expr, Literal};
use arcweft_render_text::{FallbackStylePolicy, InlineFailurePolicy, InlineFallback};

use crate::labels::expr_label;

use super::helpers::expr_style_value;

pub(crate) fn inline_failure_policy(
    expr: &Expr,
    default: Option<&InlineFailurePolicy>,
) -> InlineFailurePolicy {
    match expr {
        Expr::Call { args, .. } | Expr::MethodCall { args, .. } => {
            inline_failure_policy_from_args(args)
                .or_else(|| default.cloned())
                .unwrap_or(InlineFailurePolicy::FailLine)
        }
        _ => default.cloned().unwrap_or(InlineFailurePolicy::FailLine),
    }
}

pub(crate) fn inline_fallback_source_label(expr: &Expr) -> String {
    match expr {
        Expr::Call { args, .. } | Expr::MethodCall { args, .. } => args
            .iter()
            .find_map(|arg| match arg {
                CallArg::Positional(value) => Some(expr_label(value)),
                CallArg::Named { name, value } if name == "value" || name == "input" => {
                    Some(expr_label(value))
                }
                CallArg::Named { .. } | CallArg::Spread { .. } => None,
            })
            .unwrap_or_else(|| expr_label(expr)),
        _ => expr_label(expr),
    }
}

fn inline_failure_policy_from_args(args: &[CallArg]) -> Option<InlineFailurePolicy> {
    args.iter().find_map(|arg| match arg {
        CallArg::Named { name, value } if name == "fallback" || name == "none" => {
            Some(inline_fallback_from_expr(value))
        }
        CallArg::Named { name, value } if name == "discard_error" => {
            is_truthy_policy_value(value).then_some(InlineFailurePolicy::Discard)
        }
        CallArg::Named { name, value } if name == "on_error" => {
            Some(inline_failure_policy_from_expr(value))
        }
        CallArg::Positional(_) | CallArg::Named { .. } | CallArg::Spread { .. } => None,
    })
}

fn is_truthy_policy_value(expr: &Expr) -> bool {
    matches!(expr, Expr::Literal(Literal::Bool(true)))
        || matches!(expr, Expr::Path(path) if path == "true")
}

fn inline_failure_policy_from_expr(expr: &Expr) -> InlineFailurePolicy {
    match enum_variant_name(expr) {
        Some((_, "fail" | "line_error")) => InlineFailurePolicy::FailLine,
        Some((_, "discard")) => InlineFailurePolicy::Discard,
        _ => inline_failure_constructor(expr).unwrap_or(InlineFailurePolicy::FailLine),
    }
}

fn inline_failure_constructor(expr: &Expr) -> Option<InlineFailurePolicy> {
    let args = match expr {
        Expr::Call { callee, args } if constructor_name(callee)? == "fallback" => args,
        Expr::MethodCall {
            receiver,
            method,
            args,
        } if matches!(receiver.as_ref(), Expr::Path(namespace) if namespace == "InlineFailure")
            && method == "fallback" =>
        {
            args
        }
        _ => return None,
    };
    let fallback = args
        .iter()
        .find_map(|arg| match arg {
            CallArg::Positional(value) => Some(inline_fallback_value(value)),
            CallArg::Named { name, value } if name == "value" || name == "text" => {
                Some(inline_fallback_value(value))
            }
            CallArg::Named { .. } | CallArg::Spread { .. } => None,
        })
        .unwrap_or_else(|| InlineFallback::Text {
            text: String::new(),
            style: FallbackStylePolicy::Plain,
        });
    Some(InlineFailurePolicy::Fallback { fallback })
}

fn inline_fallback_from_expr(expr: &Expr) -> InlineFailurePolicy {
    InlineFailurePolicy::Fallback {
        fallback: inline_fallback_value(expr),
    }
}

fn inline_fallback_value(expr: &Expr) -> InlineFallback {
    match enum_variant_name(expr) {
        Some((_, "expr_source")) => InlineFallback::ExprSource {
            style: FallbackStylePolicy::Plain,
        },
        Some((_, "call_source")) => InlineFallback::CallSource {
            style: FallbackStylePolicy::Plain,
        },
        Some((_, "value_plain")) => InlineFallback::ValuePlain,
        _ => InlineFallback::Text {
            text: expr_style_value(expr),
            style: FallbackStylePolicy::Plain,
        },
    }
}

fn constructor_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Path(name) if name == "fallback" => Some("fallback"),
        Expr::Field { target, field } if matches!(target.as_ref(), Expr::Path(namespace) if namespace == "InlineFailure") => {
            Some(field)
        }
        _ => None,
    }
}

fn enum_variant_name(expr: &Expr) -> Option<(&str, &str)> {
    match expr {
        Expr::Path(value) => value.strip_prefix('.').map(|variant| ("", variant)),
        Expr::MethodCall {
            receiver,
            method,
            args,
        } if args.is_empty() => match receiver.as_ref() {
            Expr::Path(namespace) => Some((namespace.as_str(), method.as_str())),
            _ => None,
        },
        Expr::Field { target, field } => match target.as_ref() {
            Expr::Path(namespace) => Some((namespace.as_str(), field.as_str())),
            _ => None,
        },
        _ => None,
    }
}

pub(crate) fn lower_default_inline_failure_policy(args: &[LineArg]) -> Option<InlineFailurePolicy> {
    args.iter().find_map(|arg| match arg.name() {
        "inline_fallback" => Some(inline_fallback_from_expr(arg.value())),
        "inline_error" | "inline_error_policy" => {
            Some(inline_failure_policy_from_expr(arg.value()))
        }
        _ => None,
    })
}

pub(crate) fn inline_default_from_named_expr(
    name: &str,
    value: &Expr,
) -> Option<InlineFailurePolicy> {
    match name {
        "inline_fallback" => Some(inline_fallback_from_expr(value)),
        "inline_error" | "inline_error_policy" => Some(inline_failure_policy_from_expr(value)),
        _ => None,
    }
}
