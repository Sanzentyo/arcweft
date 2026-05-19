//! Source-declaration type checking.

use super::{
    SourceBackpressurePolicy, SourceEventPattern, SourceHeader, SourcePrivacyPolicy,
    SourceReplayPolicy, TypeCheckError, TypeChecker, TypeKind, YieldContext, let_else_bindings,
    source_return_types,
};
use arcweft_lang_syntax::ast::source::SourceItem;

impl TypeChecker<'_> {
    pub(super) fn check_source_item(&mut self, item: &SourceItem) {
        if item.name().is_some() {
            self.errors.push(TypeCheckError::new(
                "function-like `source name() -> Source<T, E>` is not canonical; use `source @source.id: Source<T, E> { ... }`".to_owned(),
            ));
        }
        let Some((item_ty, error_ty)) = item.source_ty().and_then(source_return_types) else {
            self.errors.push(TypeCheckError::new(
                "`source` must declare `: Source<T, E>`".to_owned(),
            ));
            return;
        };
        self.check_source_policy(item.headers());
        for header in item.headers() {
            if let SourceHeader::From(expr) = header {
                self.check_expr(expr);
            }
        }
        for handler in item.handlers() {
            let outer_locals = self.locals.clone();
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
            self.locals = outer_locals;
        }
    }

    fn check_source_policy(&mut self, headers: &[SourceHeader]) {
        let has_from = headers
            .iter()
            .any(|header| matches!(header, SourceHeader::From(_)));
        let backpressure = headers.iter().find_map(|header| match header {
            SourceHeader::Backpressure(policy) => Some(policy),
            _ => None,
        });
        let replay = headers.iter().find_map(|header| match header {
            SourceHeader::Replay(policy) => Some(policy),
            _ => None,
        });
        let privacy = headers.iter().find_map(|header| match header {
            SourceHeader::Privacy(policy) => Some(policy),
            _ => None,
        });
        if !has_from {
            self.errors
                .push(TypeCheckError::new("source is missing `from`".to_owned()));
        }
        if backpressure.is_none() {
            self.errors.push(TypeCheckError::new(
                "source is missing `backpressure` policy".to_owned(),
            ));
        }
        if replay.is_none() {
            self.errors.push(TypeCheckError::new(
                "source is missing `replay` policy".to_owned(),
            ));
        }
        if privacy.is_none() {
            self.errors.push(TypeCheckError::new(
                "source is missing `privacy` policy".to_owned(),
            ));
        }
        if matches!(privacy, Some(SourcePrivacyPolicy::Private))
            && matches!(replay, Some(SourceReplayPolicy::Full))
        {
            self.errors.push(TypeCheckError::new(
                "`privacy = private` is incompatible with `replay = full`".to_owned(),
            ));
        }
        if let Some(SourceBackpressurePolicy::Raw(raw)) = backpressure {
            self.errors.push(TypeCheckError::new(format!(
                "unknown source backpressure policy `{raw}`"
            )));
        }
    }

    fn bind_source_handler_pattern(
        &mut self,
        event: &SourceEventPattern,
        item_ty: &TypeKind,
        error_ty: &TypeKind,
    ) {
        let pattern_ty = match event {
            SourceEventPattern::Item(pattern) => Some((pattern, item_ty)),
            SourceEventPattern::Error(pattern) => Some((pattern, error_ty)),
            SourceEventPattern::Progress(pattern) => {
                let ty = TypeKind::String;
                for (name, binding_ty) in let_else_bindings(pattern, Some(&ty)) {
                    self.locals.insert(name, binding_ty);
                }
                None
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
            for (name, binding_ty) in let_else_bindings(pattern, Some(ty)) {
                self.locals.insert(name, binding_ty);
            }
        }
    }
}
