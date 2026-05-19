//! Arcweft lifetime-registry helpers for semantic checking.

use crate::types::{HandleState, TypeKind};
use arcweft_lang_syntax::{Expr, LifetimeKey, LifetimeScopeKind};

pub(crate) fn lifetime_key(expr: &Expr) -> Option<LifetimeKey> {
    match expr {
        Expr::LifetimePath { key, .. } => Some(key.clone()),
        _ => None,
    }
}

pub(crate) fn lifetime_value_type(key: &LifetimeKey) -> TypeKind {
    if key.scope() == &LifetimeScopeKind::Line
        && key.path().first().is_some_and(|part| part == "focus")
    {
        TypeKind::Handle {
            name: "FocusHandle".to_owned(),
            lifetime: key.scope().clone(),
            state: HandleState::Live,
            must_drop: true,
        }
    } else {
        TypeKind::Named("LifetimeValue".to_owned())
    }
}

pub(crate) fn collect_type_kind_lifetimes(ty: &TypeKind, lifetimes: &mut Vec<String>) {
    match ty {
        TypeKind::BorrowRef { lifetime, inner } => {
            if let Some(lifetime) = lifetime
                && !is_static_lifetime(lifetime)
            {
                lifetimes.push(lifetime.as_str().to_owned());
            }
            collect_type_kind_lifetimes(inner, lifetimes);
        }
        TypeKind::Vec(inner)
        | TypeKind::Array { item: inner, .. }
        | TypeKind::Slice(inner)
        | TypeKind::Seq(inner)
        | TypeKind::Option(inner)
        | TypeKind::ThreadHandle(inner)
        | TypeKind::Shared(inner) => collect_type_kind_lifetimes(inner, lifetimes),
        TypeKind::Map { key, value, .. } => {
            collect_type_kind_lifetimes(key, lifetimes);
            collect_type_kind_lifetimes(value, lifetimes);
        }
        TypeKind::Need { ready, error }
        | TypeKind::Stream { item: ready, error }
        | TypeKind::Source { item: ready, error }
        | TypeKind::Result { ok: ready, error } => {
            collect_type_kind_lifetimes(ready, lifetimes);
            collect_type_kind_lifetimes(error, lifetimes);
        }
        TypeKind::Tuple(items) => {
            for item in items {
                collect_type_kind_lifetimes(item, lifetimes);
            }
        }
        _ => {}
    }
}

pub(crate) fn type_contains_borrow_ref(ty: &TypeKind) -> bool {
    match ty {
        TypeKind::BorrowRef { lifetime, .. } => !lifetime.as_ref().is_some_and(is_static_lifetime),
        TypeKind::Vec(inner)
        | TypeKind::Array { item: inner, .. }
        | TypeKind::Slice(inner)
        | TypeKind::Seq(inner)
        | TypeKind::Option(inner)
        | TypeKind::ThreadHandle(inner)
        | TypeKind::Shared(inner) => type_contains_borrow_ref(inner),
        TypeKind::Map { key, value, .. } => {
            type_contains_borrow_ref(key) || type_contains_borrow_ref(value)
        }
        TypeKind::Need { ready, error }
        | TypeKind::Stream { item: ready, error }
        | TypeKind::Source { item: ready, error }
        | TypeKind::Result { ok: ready, error } => {
            type_contains_borrow_ref(ready) || type_contains_borrow_ref(error)
        }
        TypeKind::Tuple(items) => items.iter().any(type_contains_borrow_ref),
        _ => false,
    }
}

pub(crate) fn is_static_lifetime(lifetime: &LifetimeScopeKind) -> bool {
    matches!(lifetime, LifetimeScopeKind::Named(name) if name == "static")
}
