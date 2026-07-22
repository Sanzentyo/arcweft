//! Validation for dialogue-level default inline failure policies.

use crate::checker::{TypeCheckError, TypeChecker};
use arcweft_lang_hir::model::HirDialogue;
use arcweft_lang_syntax::expr::{CallArg, Expr};

impl TypeChecker<'_> {
    pub(super) fn check_dialogue_default_inline_failure_policy(&mut self, dialogue: &HirDialogue) {
        let policy_args = dialogue
            .args()
            .iter()
            .filter(|arg| {
                matches!(
                    arg.name(),
                    "inline_fallback" | "inline_error" | "inline_error_policy"
                )
            })
            .collect::<Vec<_>>();
        if policy_args.len() > 1 {
            self.errors
                .push(TypeCheckError::inline_failure_policy_conflict(format!(
                    "{} default inline policy",
                    dialogue.callee()
                )));
        }
        for arg in policy_args {
            if matches!(arg.name(), "inline_error" | "inline_error_policy")
                && let Some(policy) = unknown_default_inline_failure_policy(arg.value())
            {
                self.errors
                    .push(TypeCheckError::unknown_inline_failure_policy(
                        format!("{} default inline policy", dialogue.callee()),
                        policy,
                    ));
            }
        }
    }
}

pub(super) fn dialogue_has_default_inline_failure_policy(dialogue: &HirDialogue) -> bool {
    dialogue.args().iter().any(|arg| {
        matches!(
            arg.name(),
            "inline_fallback" | "inline_error" | "inline_error_policy"
        )
    })
}

fn unknown_default_inline_failure_policy(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Path(path) => unknown_default_inline_failure_atom(path),
        Expr::ShortVariant(name) => unknown_default_inline_failure_atom(&format!(".{name}")),
        Expr::Select(select) => match select.target() {
            Expr::Path(namespace) => {
                unknown_default_inline_failure_field(namespace.as_label(), select.member().as_str())
            }
            _ => None,
        },
        Expr::Call(call) => unknown_default_inline_failure_constructor(call.callee(), call.args()),
        _ => None,
    }
}

fn unknown_default_inline_failure_constructor(callee: &Expr, args: &[CallArg]) -> Option<String> {
    let constructor = match callee {
        Expr::Path(path) if path == "fallback" => "fallback",
        Expr::Select(select) if matches!(select.target(), Expr::Path(namespace) if namespace == "InlineFailure") => {
            select.member().as_str()
        }
        _ => return None,
    };
    if constructor != "fallback" {
        return Some(default_inline_policy_label(callee));
    }
    args.iter().find_map(|arg| match arg {
        CallArg::Positional(value) => unknown_default_inline_fallback_value(value),
        CallArg::Named { name, value } if name == "value" || name == "text" => {
            unknown_default_inline_fallback_value(value)
        }
        CallArg::Named { .. } | CallArg::Spread { .. } => None,
    })
}

fn unknown_default_inline_fallback_value(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Path(path) => unknown_default_inline_fallback_atom(path),
        Expr::ShortVariant(name) => unknown_default_inline_fallback_atom(&format!(".{name}")),
        Expr::Select(select) => match select.target() {
            Expr::Path(namespace) => unknown_default_inline_fallback_field(
                namespace.as_label(),
                select.member().as_str(),
            ),
            _ => None,
        },
        _ => None,
    }
}

fn unknown_default_inline_failure_atom(path: &str) -> Option<String> {
    let variant = path.strip_prefix('.')?;
    (!matches!(variant, "fail" | "discard" | "line_error")).then(|| path.to_owned())
}

fn unknown_default_inline_failure_field(namespace: &str, field: &str) -> Option<String> {
    (namespace == "InlineFailure" && !matches!(field, "fail" | "discard" | "line_error"))
        .then(|| format!("{namespace}.{field}"))
}

fn unknown_default_inline_fallback_atom(path: &str) -> Option<String> {
    let variant = path.strip_prefix('.')?;
    (!matches!(variant, "expr_source" | "call_source" | "value_plain")).then(|| path.to_owned())
}

fn unknown_default_inline_fallback_field(namespace: &str, field: &str) -> Option<String> {
    (namespace == "InlineFallback"
        && !matches!(field, "expr_source" | "call_source" | "value_plain"))
    .then(|| format!("{namespace}.{field}"))
}

fn default_inline_policy_label(expr: &Expr) -> String {
    match expr {
        Expr::Path(path) => path.as_label().to_owned(),
        Expr::ShortVariant(name) => format!(".{name}"),
        Expr::Select(select) => format!(
            "{}.{}",
            default_inline_policy_label(select.target()),
            select.member().as_str()
        ),
        _ => format!("{expr:?}"),
    }
}
