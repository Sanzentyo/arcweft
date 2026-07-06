use crate::labels::expr_label;
use arcweft_core::effect::RuntimeCall;
use arcweft_core::plan::FlowRuntimeId;
use arcweft_lang_hir::syntax::expr::{CallArg, Expr};

pub(super) struct PresentationMountCall<'a> {
    pub(super) kind: &'static str,
    pub(super) resource: &'a Expr,
    pub(super) args: &'a [CallArg],
    pub(super) register_scope_cleanup: bool,
}

pub(super) fn presentation_mount_call(expr: &Expr) -> Option<PresentationMountCall<'_>> {
    let Expr::Call { callee, args } = expr else {
        return None;
    };
    let kind = match callee.as_ref() {
        Expr::Path(path) if path.is_single("view") => "view",
        Expr::Path(path) if path.is_single("image") => "image",
        Expr::Path(path) if path.is_single("menu") => "menu",
        Expr::Path(path) if path.is_single("overlay") => "overlay",
        _ => return None,
    };
    let resource = args.iter().find_map(|arg| match arg {
        CallArg::Positional(value) => Some(value),
        CallArg::Named { .. } | CallArg::Spread { .. } => None,
    })?;
    let register_scope_cleanup = named_call_arg(args, "lifetime")
        .is_none_or(|lifetime| presentation_lifetime_is_scoped(&expr_label(lifetime)));
    Some(PresentationMountCall {
        kind,
        resource,
        args,
        register_scope_cleanup,
    })
}

pub(super) fn presentation_handle_id(flow_id: &FlowRuntimeId, binding: &str) -> String {
    format!(
        "handle.{}.{}",
        sanitize_presentation_handle_part(&flow_id.0),
        sanitize_presentation_handle_part(binding)
    )
}

pub(super) fn presentation_explicit_mount_handle_id(
    flow_id: &FlowRuntimeId,
    kind: &str,
    resource: &Expr,
) -> String {
    format!(
        "handle.{}.mount.{}.{}",
        sanitize_presentation_handle_part(&flow_id.0),
        sanitize_presentation_handle_part(kind),
        presentation_resource_handle_part(resource)
    )
}

pub(super) fn presentation_create_args(
    handle_id: &str,
    flow_id: &FlowRuntimeId,
    kind: &str,
    resource: &Expr,
    source_args: &[CallArg],
) -> Vec<String> {
    let mut args = vec![
        format!("handle = @{handle_id}"),
        format!("kind = \"{kind}\""),
        format!("resource = {}", expr_label(resource)),
        format!("owner = @{}", flow_id.0),
    ];
    for name in ["visible", "layer", "depth"] {
        if let Some(value) = named_call_arg(source_args, name) {
            args.push(format!("{name} = {}", expr_label(value)));
        }
    }
    args
}

pub(super) fn presentation_handle_call(operation: &str, args: Vec<String>) -> RuntimeCall {
    RuntimeCall {
        callee: format!("presentation.handle.{operation}"),
        args,
    }
}

fn named_call_arg<'a>(args: &'a [CallArg], name: &str) -> Option<&'a Expr> {
    args.iter().find_map(|arg| match arg {
        CallArg::Named {
            name: arg_name,
            value,
        } if arg_name == name => Some(value.as_ref()),
        CallArg::Positional(_) | CallArg::Named { .. } | CallArg::Spread { .. } => None,
    })
}

fn presentation_lifetime_is_scoped(label: &str) -> bool {
    !matches!(
        label.trim_matches('"'),
        ".detached" | "detached" | ".manual" | "manual" | ".global" | "global"
    )
}

fn presentation_resource_handle_part(resource: &Expr) -> String {
    let label = expr_label(resource);
    let label = label.strip_prefix('@').unwrap_or(&label);
    sanitize_presentation_handle_part(&label.replace(":.", "."))
}

fn sanitize_presentation_handle_part(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '/' | ':') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if sanitized.is_empty() {
        "anonymous".to_owned()
    } else {
        sanitized
    }
}
