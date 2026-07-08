//! Source declaration lowering into core source runtime data.

use crate::errors::RuntimePlanLowerError;
use crate::expr::{
    RuntimePureHelperLookup, lower_runtime_expr, lower_runtime_expr_strict_with_pure,
    runtime_call_effect,
};
use crate::labels::{expr_label, type_label};
use crate::pattern::lower_runtime_pattern;
use arcweft_core::effect::{LineEffectRequest, RuntimeAssignment};
use arcweft_core::source::{
    BackpressurePolicy, OverflowPolicy, PrivacyPolicy, ReplayPolicy, SourceHandlerPlan, SourceId,
    SourceOp, SourcePlan, SourcePolicy,
};
use arcweft_core::value::{RuntimeExpr, RuntimeValue};
use arcweft_lang_hir::syntax::{
    ast::{
        flow::Stmt,
        source::{
            SourceBackpressurePolicy, SourceEventPattern, SourceHandler, SourceHeader, SourceItem,
            SourceOverflowPolicy, SourcePrivacyPolicy, SourceReplayPolicy,
        },
    },
    types::TypeRef,
};

/// Lowers a checked source declaration into a Sans I/O source plan.
pub(crate) fn lower_source_plan(
    source: &SourceItem,
    pure_helpers: RuntimePureHelperLookup<'_, '_, '_>,
) -> Result<SourcePlan, Vec<RuntimePlanLowerError>> {
    let mut errors = Vec::new();
    let id = source.id().map_or_else(
        || SourceId(source.name().unwrap_or("anonymous").to_owned()),
        |id| SourceId(id.body().to_owned()),
    );
    let (item_ty, error_ty) = source
        .source_ty()
        .and_then(source_type_labels)
        .unwrap_or_else(|| {
            errors.push(RuntimePlanLowerError::new(
                "source plan requires `Source<T, E>` type".to_owned(),
            ));
            ("Unit".to_owned(), "Unit".to_owned())
        });
    let from = source
        .headers()
        .iter()
        .find_map(|header| match header {
            SourceHeader::From(expr) => Some(lower_runtime_expr_with_pure(expr, pure_helpers)),
            _ => None,
        })
        .unwrap_or_else(|| {
            errors.push(RuntimePlanLowerError::new(
                "source plan requires `from` header".to_owned(),
            ));
            RuntimeExpr::Value(RuntimeValue::Unit)
        });
    let Some(policy) = lower_source_policy(source.headers()) else {
        errors.push(RuntimePlanLowerError::new(
            "source plan requires backpressure, replay, and privacy policies".to_owned(),
        ));
        return Err(errors);
    };
    let handlers = source
        .handlers()
        .iter()
        .map(|handler| lower_source_handler(handler, pure_helpers))
        .collect::<Vec<_>>();
    if errors.is_empty() {
        Ok(SourcePlan {
            id,
            item_ty,
            error_ty,
            from,
            policy,
            handlers,
        })
    } else {
        Err(errors)
    }
}

fn lower_source_handler(
    handler: &SourceHandler,
    pure_helpers: RuntimePureHelperLookup<'_, '_, '_>,
) -> SourceHandlerPlan {
    let ops = lower_source_stmt_list(handler.body(), pure_helpers);
    match handler.event() {
        SourceEventPattern::Item(pattern) => SourceHandlerPlan::Item {
            pattern: lower_runtime_pattern(pattern),
            ops,
        },
        SourceEventPattern::Error(pattern) => SourceHandlerPlan::Error {
            pattern: lower_runtime_pattern(pattern),
            ops,
        },
        SourceEventPattern::Progress(pattern) => SourceHandlerPlan::Progress {
            pattern: lower_runtime_pattern(pattern),
            ops,
        },
        SourceEventPattern::Disconnected => SourceHandlerPlan::Disconnected { ops },
        SourceEventPattern::PermissionRevoked => SourceHandlerPlan::PermissionRevoked { ops },
        SourceEventPattern::End | SourceEventPattern::Raw(_) => SourceHandlerPlan::End { ops },
    }
}

fn lower_source_stmt_list(
    statements: &[Stmt],
    pure_helpers: RuntimePureHelperLookup<'_, '_, '_>,
) -> Vec<SourceOp> {
    statements
        .iter()
        .map(|stmt| lower_source_stmt(stmt, pure_helpers))
        .collect()
}

fn lower_source_stmt(stmt: &Stmt, pure_helpers: RuntimePureHelperLookup<'_, '_, '_>) -> SourceOp {
    match stmt {
        Stmt::Yield(expr) => {
            SourceOp::Yield(lower_runtime_expr_with_pure(expr.expr(), pure_helpers))
        }
        Stmt::Signal { target, value } => SourceOp::SignalWrite(RuntimeAssignment {
            target: expr_label(target.expr()),
            value: expr_label(value.expr()),
        }),
        Stmt::Expr { expr, .. } => match runtime_call_effect(expr) {
            LineEffectRequest::Log(log) => SourceOp::Log(log),
            effect => SourceOp::Effect(effect),
        },
        Stmt::Close(expr) => SourceOp::Close(SourceId(expr_label(expr.expr()))),
        _ => SourceOp::Noop,
    }
}

fn lower_runtime_expr_with_pure(
    expr: &arcweft_lang_hir::syntax::expr::Expr,
    pure_helpers: RuntimePureHelperLookup<'_, '_, '_>,
) -> RuntimeExpr {
    lower_runtime_expr_strict_with_pure(expr, pure_helpers)
        .unwrap_or_else(|_| lower_runtime_expr(expr))
}

fn source_type_labels(ty: &TypeRef) -> Option<(String, String)> {
    match ty {
        TypeRef::Generic { base, args } if base == "Source" && args.len() == 2 => {
            Some((type_label(&args[0]), type_label(&args[1])))
        }
        _ => None,
    }
}

fn lower_source_policy(headers: &[SourceHeader]) -> Option<SourcePolicy> {
    let backpressure = headers.iter().find_map(|header| match header {
        SourceHeader::Backpressure(policy) => lower_backpressure(policy),
        _ => None,
    })?;
    let replay = headers.iter().find_map(|header| match header {
        SourceHeader::Replay(policy) => lower_replay(policy),
        _ => None,
    })?;
    let privacy = headers.iter().find_map(|header| match header {
        SourceHeader::Privacy(policy) => lower_privacy(policy),
        _ => None,
    })?;
    let max_queue = match &backpressure {
        BackpressurePolicy::LatestOnly | BackpressurePolicy::BlockingNotAllowed => 1,
        BackpressurePolicy::BoundedQueue { capacity, .. } => *capacity,
    };
    Some(SourcePolicy {
        backpressure,
        replay,
        privacy,
        max_queue,
    })
}

fn lower_backpressure(policy: &SourceBackpressurePolicy) -> Option<BackpressurePolicy> {
    match policy {
        SourceBackpressurePolicy::Latest => Some(BackpressurePolicy::LatestOnly),
        SourceBackpressurePolicy::BlockingNotAllowed => {
            Some(BackpressurePolicy::BlockingNotAllowed)
        }
        SourceBackpressurePolicy::Bounded { capacity, overflow } => {
            Some(BackpressurePolicy::BoundedQueue {
                capacity: expr_label(capacity).parse().unwrap_or(1),
                on_overflow: lower_overflow(overflow),
            })
        }
        SourceBackpressurePolicy::Raw(_) => None,
    }
}

fn lower_overflow(policy: &SourceOverflowPolicy) -> OverflowPolicy {
    match policy {
        SourceOverflowPolicy::DropOldest => OverflowPolicy::DropOldest,
        SourceOverflowPolicy::DropNewest => OverflowPolicy::DropNewest,
        SourceOverflowPolicy::Error | SourceOverflowPolicy::Raw(_) => OverflowPolicy::Error,
        SourceOverflowPolicy::Coalesce => OverflowPolicy::Coalesce,
    }
}

fn lower_replay(policy: &SourceReplayPolicy) -> Option<ReplayPolicy> {
    match policy {
        SourceReplayPolicy::Full => Some(ReplayPolicy::Full),
        SourceReplayPolicy::HashOnly => Some(ReplayPolicy::HashOnly),
        SourceReplayPolicy::Summary => Some(ReplayPolicy::Summary),
        SourceReplayPolicy::EventOnly => Some(ReplayPolicy::EventOnly),
        SourceReplayPolicy::None => Some(ReplayPolicy::None),
        SourceReplayPolicy::Raw(_) => None,
    }
}

fn lower_privacy(policy: &SourcePrivacyPolicy) -> Option<PrivacyPolicy> {
    match policy {
        SourcePrivacyPolicy::Transient => Some(PrivacyPolicy::Transient),
        SourcePrivacyPolicy::Redacted => Some(PrivacyPolicy::Redacted),
        SourcePrivacyPolicy::Recordable => Some(PrivacyPolicy::Recordable),
        SourcePrivacyPolicy::Private => Some(PrivacyPolicy::Private),
        SourcePrivacyPolicy::Raw(_) => None,
    }
}
