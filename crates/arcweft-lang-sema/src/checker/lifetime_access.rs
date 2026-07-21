//! Lifetime registry access and drop semantics.

use super::{
    Expr, LifetimeAccessMode, LifetimeKey, LifetimeScopeKind, TypeCheckError, TypeChecker,
    TypeKind, is_drop_callee, lifetime_key, lifetime_value_type,
};

impl TypeChecker<'_> {
    pub(super) fn check_lifetime_path_expr(
        &mut self,
        key: &LifetimeKey,
        optional: bool,
    ) -> Option<TypeKind> {
        self.check_lifetime_access(key, LifetimeAccessMode::Read);
        if self.dropped_lifetime_keys.contains(key) {
            self.errors.push(TypeCheckError::new(format!(
                "lifetime registry key `{}` was already dropped",
                key.as_dotted()
            )));
            return None;
        }
        let value = lifetime_value_type(key);
        if optional || self.lifetime_guarantees.contains(key) {
            return Some(if optional {
                TypeKind::Option(Box::new(value))
            } else {
                value
            });
        }
        self.errors.push(TypeCheckError::new(format!(
            "lifetime registry key `{}` is not statically guaranteed; use `{}?` or initialize it first",
            key.as_dotted(),
            key.as_dotted()
        )));
        Some(TypeKind::Option(Box::new(value)))
    }

    pub(super) fn check_lifetime_pipe(&mut self, lhs: &Expr, rhs: &Expr) -> Option<()> {
        let key = lifetime_key(lhs)?;
        match rhs {
            Expr::Path(path) if matches!(path.as_str(), "drop" | "drop_optional" | "on_drop") => {
                self.drop_lifetime_key(&key);
                Some(())
            }
            Expr::Call(call) if matches!(call.callee(), Expr::Path(path) if matches!(path.as_str(), "drop" | "drop_optional" | "on_drop")) =>
            {
                self.drop_lifetime_key(&key);
                Some(())
            }
            _ => None,
        }
    }

    pub(super) fn drop_lifetime_key(&mut self, key: &LifetimeKey) {
        self.check_lifetime_access(key, LifetimeAccessMode::Drop);
        if !self.retain_dropped_lifetime_key(key.clone()) {
            self.errors.push(TypeCheckError::new(format!(
                "lifetime registry key `{}` was dropped more than once",
                key.as_dotted()
            )));
        }
        self.release_lifetime_guarantee(key);
    }

    pub(super) fn release_direct_drop_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Call(call) if is_drop_callee(call.callee()) => {
                if let Expr::Select(select) = call.callee()
                    && let Expr::Path(name) = select.target()
                {
                    self.release_borrow_local(name.as_label());
                }
                for arg in call.args() {
                    if let Expr::Path(name) = arg.value() {
                        self.release_borrow_local(name.as_label());
                    }
                }
            }
            Expr::Pipe { lhs, rhs } if is_drop_callee(rhs) => {
                if let Expr::Path(name) = lhs.as_ref() {
                    self.release_borrow_local(name.as_label());
                }
            }
            _ => {}
        }
    }

    pub(super) fn check_lifetime_access(&mut self, key: &LifetimeKey, mode: LifetimeAccessMode) {
        if !self.lifetime_available(key.scope()) {
            self.errors.push(TypeCheckError::new(format!(
                "lifetime `{}` is not available in this scope",
                key.scope().as_str()
            )));
        }
        if matches!(mode, LifetimeAccessMode::Write)
            && !matches!(key.scope(), LifetimeScopeKind::Line)
        {
            self.record_static_effect(
                &format!("state.write({})", key.scope().as_str()),
                format!("write `{}`", key.as_dotted()),
            );
        }
    }

    fn lifetime_available(&self, scope: &LifetimeScopeKind) -> bool {
        !matches!(scope, LifetimeScopeKind::Line | LifetimeScopeKind::Cue)
            || self.available_lifetimes.contains(scope)
    }
}
