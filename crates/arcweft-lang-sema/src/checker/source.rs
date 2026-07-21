//! Source-declaration type checking.

use super::helpers::let_else_bindings;
use super::{
    LocalBindingSnapshot, SourceBackpressurePolicy, SourceEventPattern, SourceHeader,
    SourcePrivacyPolicy, SourceReplayPolicy, TypeCheckError, TypeChecker, TypeKind, YieldContext,
    source_return_types,
};
use arcweft_lang_syntax::{
    ast::{
        flow::AuthoredExpr,
        source::{SourceHeaderInventory, SourceItem, SourceOverflowPolicy},
    },
    expr::{Expr, Literal},
};

impl TypeChecker<'_> {
    pub(super) fn check_source_item(&mut self, item: &SourceItem) {
        if item.name().is_some() {
            self.errors.push(TypeCheckError::new(
                "function-like `source name() -> Source<T, E>` is not canonical; use `source @source.id: Source<T, E> { ... }`".to_owned(),
            ));
        }
        let Some((item_ty, error_ty)) = item
            .source_ty()
            .and_then(|ty| source_return_types(ty.value()))
        else {
            self.errors.push(TypeCheckError::new(
                "`source` must declare `: Source<T, E>`".to_owned(),
            ));
            return;
        };
        if let Some(inventory) = self.check_source_policy(item.headers())
            && let Some(expr) = inventory.from()
        {
            self.check_authored_expr(expr);
        }
        for _ in item.body_statements() {
            self.errors.push(TypeCheckError::new(
                "source body statements must be inside an `on` handler".to_owned(),
            ));
        }
        for handler in item.handlers() {
            let local_snapshot =
                self.bind_source_handler_pattern(handler.event(), &item_ty, &error_ty);
            self.yield_stack.push(YieldContext::Source {
                item_ty: item_ty.clone(),
                error_ty: error_ty.clone(),
                yield_count: 0,
            });
            for stmt in handler.body() {
                self.check_stmt(stmt);
            }
            self.yield_stack.pop();
            self.restore_scoped_locals(local_snapshot);
        }
    }

    fn check_source_policy<'headers>(
        &mut self,
        headers: &'headers [SourceHeader],
    ) -> Option<SourceHeaderInventory<'headers>> {
        let inventory = match SourceHeaderInventory::try_from(headers) {
            Ok(inventory) => inventory,
            Err(duplicate) => {
                self.errors.push(TypeCheckError::new(format!(
                    "source header `{}` may appear only once",
                    duplicate.kind().as_str()
                )));
                return None;
            }
        };
        if inventory.from().is_none() {
            self.errors
                .push(TypeCheckError::new("source is missing `from`".to_owned()));
        }
        if let Some((policy, _)) = inventory.backpressure() {
            self.check_source_backpressure_policy(policy);
        } else {
            self.errors.push(TypeCheckError::new(
                "source is missing `backpressure` policy".to_owned(),
            ));
        }
        if let Some((SourceReplayPolicy::Raw(raw), _)) = inventory.replay() {
            self.errors.push(TypeCheckError::new(format!(
                "unknown source replay policy `{raw}`"
            )));
        } else if inventory.replay().is_none() {
            self.errors.push(TypeCheckError::new(
                "source is missing `replay` policy".to_owned(),
            ));
        }
        if let Some((SourcePrivacyPolicy::Raw(raw), _)) = inventory.privacy() {
            self.errors.push(TypeCheckError::new(format!(
                "unknown source privacy policy `{raw}`"
            )));
        } else if inventory.privacy().is_none() {
            self.errors.push(TypeCheckError::new(
                "source is missing `privacy` policy".to_owned(),
            ));
        }
        if matches!(inventory.privacy(), Some((SourcePrivacyPolicy::Private, _)))
            && matches!(inventory.replay(), Some((SourceReplayPolicy::Full, _)))
        {
            self.errors.push(TypeCheckError::new(
                "`privacy = private` is incompatible with `replay = full`".to_owned(),
            ));
        }
        Some(inventory)
    }

    fn check_source_backpressure_policy(&mut self, policy: &SourceBackpressurePolicy) {
        match policy {
            SourceBackpressurePolicy::Latest | SourceBackpressurePolicy::BlockingNotAllowed => {}
            SourceBackpressurePolicy::Bounded { capacity, overflow } => {
                match capacity.as_deref().map(AuthoredExpr::expr) {
                    None => self.errors.push(TypeCheckError::new(
                        "bounded source policy requires a `capacity` option".to_owned(),
                    )),
                    Some(Expr::Literal(Literal::Int(literal))) => match literal.magnitude() {
                        Ok(0) => self.errors.push(TypeCheckError::new(
                            "bounded source capacity must be greater than zero".to_owned(),
                        )),
                        Ok(value) if usize::try_from(value).is_err() => {
                            self.errors.push(TypeCheckError::new(
                                "bounded source capacity exceeds usize".to_owned(),
                            ));
                        }
                        Ok(_) => {}
                        Err(error) => self.errors.push(TypeCheckError::new(format!(
                            "invalid bounded source capacity: {error}"
                        ))),
                    },
                    Some(_) => self.errors.push(TypeCheckError::new(
                        "bounded source capacity must be an integer literal".to_owned(),
                    )),
                }
                self.check_source_overflow_policy(overflow);
            }
            SourceBackpressurePolicy::Raw(raw) => {
                self.errors.push(TypeCheckError::new(format!(
                    "unknown source backpressure policy `{raw}`"
                )));
            }
        }
    }

    fn check_source_overflow_policy(&mut self, policy: &SourceOverflowPolicy) {
        match policy {
            SourceOverflowPolicy::DropOldest
            | SourceOverflowPolicy::DropNewest
            | SourceOverflowPolicy::Error
            | SourceOverflowPolicy::Coalesce => {}
            SourceOverflowPolicy::Missing => self.errors.push(TypeCheckError::new(
                "bounded source policy requires an `overflow` option".to_owned(),
            )),
            SourceOverflowPolicy::Raw { value, .. } => {
                self.errors.push(TypeCheckError::new(format!(
                    "unknown source overflow policy `{value}`"
                )));
            }
        }
    }

    fn bind_source_handler_pattern(
        &mut self,
        event: &SourceEventPattern,
        item_ty: &TypeKind,
        error_ty: &TypeKind,
    ) -> LocalBindingSnapshot {
        let pattern_ty = match event {
            SourceEventPattern::Item(pattern) => Some((pattern, item_ty)),
            SourceEventPattern::Error(pattern) => Some((pattern, error_ty)),
            SourceEventPattern::Progress(pattern) => {
                let ty = TypeKind::String;
                return self.insert_scoped_locals(let_else_bindings(pattern, Some(&ty)));
            }
            SourceEventPattern::Raw(raw) => {
                self.errors.push(TypeCheckError::new(format!(
                    "unknown source event pattern `{raw}`"
                )));
                None
            }
            SourceEventPattern::Disconnected
            | SourceEventPattern::PermissionRevoked
            | SourceEventPattern::End => None,
        };
        if let Some((pattern, ty)) = pattern_ty {
            self.insert_scoped_locals(let_else_bindings(pattern, Some(ty)))
        } else {
            LocalBindingSnapshot::default()
        }
    }
}
