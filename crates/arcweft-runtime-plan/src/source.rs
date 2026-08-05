//! Source declaration lowering into core source runtime data.

use crate::errors::RuntimePlanLowerError;
use crate::expr::{
    LoweredRuntimeEffect, RuntimePureHelperLookup, lower_runtime_effect_strict_with_pure,
    lower_runtime_expr_strict_with_pure,
};
use crate::labels::{expr_label, type_label};
use crate::lowering_context::ExecutableLoweringLocation;
use crate::pattern::lower_runtime_pattern_checked;
use arcweft_core::effect::LineEffectRequest;
use arcweft_core::source::{
    BackpressurePolicy, OverflowPolicy, PrivacyPolicy, ReplayPolicy, SourceHandlerPlan, SourceId,
    SourceOp, SourcePlan, SourcePolicy,
};
use arcweft_core::value::{RuntimeExpr, RuntimeValue};
use arcweft_lang_hir::model::{HirModule, HirSource};
use arcweft_lang_syntax::{
    ast::{
        flow::Stmt,
        source::{
            SourceBackpressurePolicy, SourceEventPattern, SourceHandler, SourceHeaderInventory,
            SourceOverflowPolicy, SourcePrivacyPolicy, SourceReplayPolicy,
        },
    },
    expr::{Expr, Literal},
    types::TypeRef,
};

/// Lowers a checked source declaration into a Sans I/O source plan.
pub(crate) fn lower_source_plan(
    module: &HirModule,
    source: &HirSource,
    pure_helpers: RuntimePureHelperLookup<'_, '_, '_>,
) -> Result<SourcePlan, Vec<RuntimePlanLowerError>> {
    let source_item = source.item();
    let mut errors = Vec::new();
    let id = source_item.id().map_or_else(
        || SourceId::from_local_name(source_item.name().unwrap_or("anonymous")),
        |id| SourceId(id.body().to_owned()),
    );
    let owner = format!("source `{}`", id.0);
    let location =
        ExecutableLoweringLocation::in_module(owner.clone(), module, source.module_path());
    for (index, statement) in source_item.body_statements().iter().enumerate() {
        errors.push(
            location
                .child("body")
                .statement(index)
                .unsupported_statement(statement),
        );
    }
    let header_inventory = match SourceHeaderInventory::try_from(source_item.headers()) {
        Ok(inventory) => inventory,
        Err(duplicate) => {
            let header = duplicate.kind().as_str();
            errors.push(
                location
                    .child(format!("header.{header}"))
                    .named_expression_error(
                        "header",
                        header,
                        duplicate.second_range(),
                        format!("source header `{header}` may appear only once"),
                    ),
            );
            return Err(errors);
        }
    };
    let (item_ty, error_ty) = source_item
        .source_ty()
        .and_then(|ty| source_type_labels(ty.value()))
        .unwrap_or_else(|| {
            errors.push(RuntimePlanLowerError::new(
                "source plan requires `Source<T, E>` type".to_owned(),
            ));
            ("Unit".to_owned(), "Unit".to_owned())
        });
    let from = if let Some(expr) = header_inventory.from() {
        lower_runtime_expr_strict_with_pure(expr.expr(), pure_helpers).unwrap_or_else(|reason| {
            errors.push(location.child("header.from").named_expression_error(
                "header",
                "from",
                expr.range(),
                reason,
            ));
            RuntimeExpr::Value(RuntimeValue::Unit)
        })
    } else {
        errors.push(location.child("header.from").named_expression_error(
            "header",
            "from",
            None,
            "source plan requires `from` header",
        ));
        RuntimeExpr::Value(RuntimeValue::Unit)
    };
    let policy = match lower_source_policy(header_inventory) {
        Ok(policy) => policy,
        Err(error) => {
            errors.push(
                location
                    .child(format!("header.{}", error.header))
                    .named_expression_error(
                        "header",
                        error.header,
                        error.source_range,
                        error.reason,
                    ),
            );
            return Err(errors);
        }
    };
    let mut handlers = Vec::new();
    for handler in source_item.handlers() {
        match lower_source_handler(handler, pure_helpers, &owner, &location) {
            Ok(handler) => handlers.push(handler),
            Err(mut handler_errors) => errors.append(&mut handler_errors),
        }
    }
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
    source_owner: &str,
    source_location: &ExecutableLoweringLocation<'_>,
) -> Result<SourceHandlerPlan, Vec<RuntimePlanLowerError>> {
    let event = source_event_label(handler.event());
    let owner = format!("{source_owner} handler `{event}`");
    let location = source_location.with_owner(owner.clone());
    let ops = lower_source_stmt_list(handler.body(), pure_helpers, &location)?;
    Ok(match handler.event() {
        SourceEventPattern::Item(pattern) => SourceHandlerPlan::Item {
            pattern: lower_runtime_pattern_checked(pattern).map_err(|reason| {
                vec![
                    location
                        .child("event")
                        .named_pattern_error("handler", "event", None, reason),
                ]
            })?,
            ops,
        },
        SourceEventPattern::Error(pattern) => SourceHandlerPlan::Error {
            pattern: lower_runtime_pattern_checked(pattern).map_err(|reason| {
                vec![
                    location
                        .child("event")
                        .named_pattern_error("handler", "event", None, reason),
                ]
            })?,
            ops,
        },
        SourceEventPattern::Progress(pattern) => SourceHandlerPlan::Progress {
            pattern: lower_runtime_pattern_checked(pattern).map_err(|reason| {
                vec![
                    location
                        .child("event")
                        .named_pattern_error("handler", "event", None, reason),
                ]
            })?,
            ops,
        },
        SourceEventPattern::Disconnected => SourceHandlerPlan::Disconnected { ops },
        SourceEventPattern::PermissionRevoked => SourceHandlerPlan::PermissionRevoked { ops },
        SourceEventPattern::End => SourceHandlerPlan::End { ops },
        SourceEventPattern::Raw(raw) => {
            return Err(vec![RuntimePlanLowerError::new(format!(
                "{owner} cannot lower raw event pattern `{raw}`"
            ))]);
        }
    })
}

fn lower_source_stmt_list(
    statements: &[Stmt],
    pure_helpers: RuntimePureHelperLookup<'_, '_, '_>,
    location: &ExecutableLoweringLocation<'_>,
) -> Result<Vec<SourceOp>, Vec<RuntimePlanLowerError>> {
    let mut ops = Vec::new();
    let mut errors = Vec::new();
    for (index, statement) in statements.iter().enumerate() {
        match lower_source_stmt(statement, pure_helpers, &location.statement(index)) {
            Ok(op) => ops.push(op),
            Err(error) => errors.push(error),
        }
    }
    if errors.is_empty() {
        Ok(ops)
    } else {
        Err(errors)
    }
}

fn lower_source_stmt(
    stmt: &Stmt,
    pure_helpers: RuntimePureHelperLookup<'_, '_, '_>,
    location: &ExecutableLoweringLocation<'_>,
) -> Result<SourceOp, RuntimePlanLowerError> {
    match stmt {
        Stmt::Yield(expr) => lower_runtime_expr_strict_with_pure(expr.expr(), pure_helpers)
            .map(SourceOp::Yield)
            .map_err(|reason| location.expression_error(stmt, "value", expr.range(), reason)),
        Stmt::Signal { target, value } => Ok(SourceOp::EvaluatedEffect(
            arcweft_core::effect::RuntimeEffectExpr::SignalWrite {
                target: lower_runtime_expr_strict_with_pure(target.expr(), pure_helpers).map_err(
                    |reason| location.expression_error(stmt, "target", target.range(), reason),
                )?,
                value: lower_runtime_expr_strict_with_pure(value.expr(), pure_helpers).map_err(
                    |reason| location.expression_error(stmt, "value", value.range(), reason),
                )?,
            },
        )),
        Stmt::Expr {
            expr, expr_range, ..
        } => lower_runtime_effect_strict_with_pure(expr, pure_helpers)
            .map(|effect| match effect {
                LoweredRuntimeEffect::Static(LineEffectRequest::Log(log)) => SourceOp::Log(log),
                LoweredRuntimeEffect::Static(effect) => SourceOp::Effect(effect),
                LoweredRuntimeEffect::Evaluated(effect) => SourceOp::EvaluatedEffect(effect),
            })
            .map_err(|reason| location.expression_error(stmt, "effect", *expr_range, reason)),
        Stmt::Close(expr) => {
            let target = lower_runtime_expr_strict_with_pure(expr.expr(), pure_helpers).map_err(
                |reason| location.expression_error(stmt, "source", expr.range(), reason),
            )?;
            if !matches!(target, RuntimeExpr::EntityRef(_)) {
                return Err(location.expression_error(
                    stmt,
                    "source",
                    expr.range(),
                    "source close target must be a static source entity reference",
                ));
            }
            Ok(SourceOp::Close(SourceId(expr_label(expr.expr()))))
        }
        Stmt::Assertion(_)
        | Stmt::Let { .. }
        | Stmt::LetElse { .. }
        | Stmt::LetScope { .. }
        | Stmt::LetLoop { .. }
        | Stmt::LetAwait { .. }
        | Stmt::LetActionReceive { .. }
        | Stmt::LetChoice { .. }
        | Stmt::Return { .. }
        | Stmt::Out { .. }
        | Stmt::Defer { .. }
        | Stmt::Goto(_)
        | Stmt::Select(_)
        | Stmt::Break { .. }
        | Stmt::Continue { .. }
        | Stmt::Assign { .. }
        | Stmt::LifetimeSet { .. }
        | Stmt::Wait(_)
        | Stmt::Thread(_)
        | Stmt::DeferBlock { .. }
        | Stmt::On { .. }
        | Stmt::Loop { .. }
        | Stmt::UnsafeLifetime { .. }
        | Stmt::If { .. }
        | Stmt::While { .. }
        | Stmt::WhileLet { .. }
        | Stmt::For { .. }
        | Stmt::Match { .. }
        | Stmt::Raw(_) => Err(location.unsupported_statement(stmt)),
    }
}

fn source_event_label(event: &SourceEventPattern) -> &'static str {
    match event {
        SourceEventPattern::Item(_) => "item",
        SourceEventPattern::Error(_) => "error",
        SourceEventPattern::Progress(_) => "progress",
        SourceEventPattern::Disconnected => "disconnected",
        SourceEventPattern::PermissionRevoked => "permission-revoked",
        SourceEventPattern::End => "end",
        SourceEventPattern::Raw(_) => "raw",
    }
}

fn source_type_labels(ty: &TypeRef) -> Option<(String, String)> {
    match ty {
        TypeRef::Generic { base, args }
            if base.canonical_string() == "Source" && args.len() == 2 =>
        {
            Some((type_label(&args[0]), type_label(&args[1])))
        }
        _ => None,
    }
}

struct SourcePolicyLowerError {
    header: &'static str,
    source_range: Option<arcweft_lang_syntax::ast::common::TextRange>,
    reason: String,
}

struct SourcePolicyValueError {
    source_range: Option<arcweft_lang_syntax::ast::common::TextRange>,
    reason: String,
}

fn lower_source_policy(
    headers: SourceHeaderInventory<'_>,
) -> Result<SourcePolicy, SourcePolicyLowerError> {
    let (backpressure, backpressure_range) =
        headers
            .backpressure()
            .ok_or_else(|| SourcePolicyLowerError {
                header: "backpressure",
                source_range: None,
                reason: "source plan requires a backpressure policy".to_owned(),
            })?;
    let backpressure =
        lower_backpressure(backpressure).map_err(|error| SourcePolicyLowerError {
            header: "backpressure",
            source_range: error.source_range.or(Some(backpressure_range)),
            reason: error.reason,
        })?;
    let (replay, replay_range) = headers.replay().ok_or_else(|| SourcePolicyLowerError {
        header: "replay",
        source_range: None,
        reason: "source plan requires a replay policy".to_owned(),
    })?;
    let replay = lower_replay(replay).map_err(|reason| SourcePolicyLowerError {
        header: "replay",
        source_range: Some(replay_range),
        reason,
    })?;
    let (privacy, privacy_range) = headers.privacy().ok_or_else(|| SourcePolicyLowerError {
        header: "privacy",
        source_range: None,
        reason: "source plan requires a privacy policy".to_owned(),
    })?;
    let privacy = lower_privacy(privacy).map_err(|reason| SourcePolicyLowerError {
        header: "privacy",
        source_range: Some(privacy_range),
        reason,
    })?;
    if privacy == PrivacyPolicy::Private && replay == ReplayPolicy::Full {
        return Err(SourcePolicyLowerError {
            header: "privacy",
            source_range: Some(privacy_range),
            reason: "`privacy = private` is incompatible with `replay = full`".to_owned(),
        });
    }
    let max_queue = match &backpressure {
        BackpressurePolicy::LatestOnly | BackpressurePolicy::BlockingNotAllowed => 1,
        BackpressurePolicy::BoundedQueue { capacity, .. } => *capacity,
    };
    Ok(SourcePolicy {
        backpressure,
        replay,
        privacy,
        max_queue,
    })
}

fn lower_backpressure(
    policy: &SourceBackpressurePolicy,
) -> Result<BackpressurePolicy, SourcePolicyValueError> {
    match policy {
        SourceBackpressurePolicy::Latest => Ok(BackpressurePolicy::LatestOnly),
        SourceBackpressurePolicy::BlockingNotAllowed => Ok(BackpressurePolicy::BlockingNotAllowed),
        SourceBackpressurePolicy::Bounded { capacity, overflow } => {
            let Some(capacity) = capacity else {
                return Err(SourcePolicyValueError {
                    source_range: None,
                    reason: "bounded source policy requires a `capacity` option".to_owned(),
                });
            };
            let capacity_range = capacity.range();
            let Expr::Literal(Literal::Int(literal)) = capacity.expr() else {
                return Err(SourcePolicyValueError {
                    source_range: capacity_range,
                    reason: "bounded source capacity must be an integer literal".to_owned(),
                });
            };
            let capacity = literal
                .magnitude()
                .map_err(|error| SourcePolicyValueError {
                    source_range: capacity_range,
                    reason: format!("invalid bounded source capacity: {error}"),
                })
                .and_then(|magnitude| {
                    usize::try_from(magnitude).map_err(|_| SourcePolicyValueError {
                        source_range: capacity_range,
                        reason: "bounded source capacity exceeds usize".to_owned(),
                    })
                })?;
            if capacity == 0 {
                return Err(SourcePolicyValueError {
                    source_range: capacity_range,
                    reason: "bounded source capacity must be greater than zero".to_owned(),
                });
            }
            Ok(BackpressurePolicy::BoundedQueue {
                capacity,
                on_overflow: lower_overflow(overflow)?,
            })
        }
        SourceBackpressurePolicy::Raw(raw) => Err(SourcePolicyValueError {
            source_range: None,
            reason: format!("unknown source backpressure policy `{raw}`"),
        }),
    }
}

fn lower_overflow(policy: &SourceOverflowPolicy) -> Result<OverflowPolicy, SourcePolicyValueError> {
    match policy {
        SourceOverflowPolicy::DropOldest => Ok(OverflowPolicy::DropOldest),
        SourceOverflowPolicy::DropNewest => Ok(OverflowPolicy::DropNewest),
        SourceOverflowPolicy::Error => Ok(OverflowPolicy::Error),
        SourceOverflowPolicy::Coalesce => Ok(OverflowPolicy::Coalesce),
        SourceOverflowPolicy::Missing => Err(SourcePolicyValueError {
            source_range: None,
            reason: "bounded source policy requires an `overflow` option".to_owned(),
        }),
        SourceOverflowPolicy::Raw { value, range } => Err(SourcePolicyValueError {
            source_range: *range,
            reason: format!("unknown source overflow policy `{value}`"),
        }),
    }
}

fn lower_replay(policy: &SourceReplayPolicy) -> Result<ReplayPolicy, String> {
    match policy {
        SourceReplayPolicy::Full => Ok(ReplayPolicy::Full),
        SourceReplayPolicy::HashOnly => Ok(ReplayPolicy::HashOnly),
        SourceReplayPolicy::Summary => Ok(ReplayPolicy::Summary),
        SourceReplayPolicy::EventOnly => Ok(ReplayPolicy::EventOnly),
        SourceReplayPolicy::None => Ok(ReplayPolicy::None),
        SourceReplayPolicy::Raw(raw) => Err(format!("unknown source replay policy `{raw}`")),
    }
}

fn lower_privacy(policy: &SourcePrivacyPolicy) -> Result<PrivacyPolicy, String> {
    match policy {
        SourcePrivacyPolicy::Transient => Ok(PrivacyPolicy::Transient),
        SourcePrivacyPolicy::Redacted => Ok(PrivacyPolicy::Redacted),
        SourcePrivacyPolicy::Recordable => Ok(PrivacyPolicy::Recordable),
        SourcePrivacyPolicy::Private => Ok(PrivacyPolicy::Private),
        SourcePrivacyPolicy::Raw(raw) => Err(format!("unknown source privacy policy `{raw}`")),
    }
}

#[cfg(test)]
mod tests {
    use super::{lower_backpressure, lower_source_policy};
    use arcweft_lang_syntax::{
        ast::{
            common::TextRange,
            flow::AuthoredExpr,
            source::{SourceBackpressurePolicy, SourceHeaderInventory, SourceOverflowPolicy},
        },
        expr::{Expr, IntLiteral, Literal},
    };

    #[test]
    fn source_policy_requires_explicit_headers() {
        let inventory = SourceHeaderInventory::try_from(&[][..]).expect("empty inventory builds");
        let error =
            lower_source_policy(inventory).expect_err("missing policies must not use defaults");

        assert!(error.reason.contains("requires a backpressure policy"));
    }

    #[test]
    fn bounded_source_policy_rejects_zero_capacity() {
        let policy = SourceBackpressurePolicy::Bounded {
            capacity: Some(Box::new(AuthoredExpr::new(Expr::Literal(Literal::Int(
                IntLiteral::decimal(0, None),
            ))))),
            overflow: SourceOverflowPolicy::DropOldest,
        };

        let error = lower_backpressure(&policy).expect_err("zero capacity is not executable");
        assert!(error.reason.contains("greater than zero"));
    }

    #[test]
    fn bounded_source_policy_rejects_unknown_overflow_name_at_its_authored_range() {
        let range = TextRange::new(24, 30);
        let policy = SourceBackpressurePolicy::Bounded {
            capacity: Some(Box::new(AuthoredExpr::new(Expr::Literal(Literal::Int(
                IntLiteral::decimal(1, None),
            ))))),
            overflow: SourceOverflowPolicy::Raw {
                value: "legacy".to_owned(),
                range: Some(range),
            },
        };

        let error = lower_backpressure(&policy).expect_err("raw overflow must not become Error");
        assert_eq!(error.source_range, Some(range));
        assert!(
            error
                .reason
                .contains("unknown source overflow policy `legacy`")
        );
    }

    #[test]
    fn bounded_source_policy_distinguishes_missing_overflow() {
        let policy = SourceBackpressurePolicy::Bounded {
            capacity: Some(Box::new(AuthoredExpr::new(Expr::Literal(Literal::Int(
                IntLiteral::decimal(1, None),
            ))))),
            overflow: SourceOverflowPolicy::Missing,
        };

        let error = lower_backpressure(&policy).expect_err("missing overflow must be rejected");
        assert_eq!(error.source_range, None);
        assert_eq!(
            error.reason,
            "bounded source policy requires an `overflow` option"
        );
    }
}
