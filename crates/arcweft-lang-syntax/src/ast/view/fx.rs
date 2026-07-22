//! Typed View-side Fx application syntax and traversal.

use crate::{ast::common::TextRange, expr::Expr};

use super::{ViewExpr, ViewModifier};

/// Authored ordinal of an `.fx(...)` application within one View modifier chain.
///
/// The ordinal is independent of the resolved Fx definition. Retained View
/// lowering combines it with the node key and optional local key so repeated
/// applications of the same Fx function keep distinct instance identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewFxApplicationOrdinal(u32);

/// One View-side application of a reusable Fx function.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewFxApplication {
    call: Expr,
    key: Option<Expr>,
    ordinal: ViewFxApplicationOrdinal,
    range: TextRange,
}

impl ViewFxApplicationOrdinal {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

impl ViewFxApplication {
    pub const fn new(
        call: Expr,
        key: Option<Expr>,
        ordinal: ViewFxApplicationOrdinal,
        range: TextRange,
    ) -> Self {
        Self {
            call,
            key,
            ordinal,
            range,
        }
    }

    /// Typed function-call syntax to be resolved to a `#[fx]` declaration.
    pub const fn call(&self) -> &Expr {
        &self.call
    }

    /// Optional application-local identity expression.
    pub const fn key(&self) -> Option<&Expr> {
        self.key.as_ref()
    }

    pub const fn ordinal(&self) -> ViewFxApplicationOrdinal {
        self.ordinal
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }
}

pub(super) fn collect_fx_applications<'a>(
    expr: &'a ViewExpr,
    applications: &mut Vec<&'a ViewFxApplication>,
) {
    let modifiers = match expr {
        ViewExpr::Element(element) => Some(element.modifiers()),
        ViewExpr::ViewCall(call) => Some(call.modifiers()),
        ViewExpr::Text(text) => Some(text.modifiers()),
        ViewExpr::Image(image) => Some(image.modifiers()),
        ViewExpr::TextField(field) => Some(field.modifiers()),
        ViewExpr::Button(button) => Some(button.modifiers()),
        ViewExpr::Fragment(_)
        | ViewExpr::Let(_)
        | ViewExpr::If(_)
        | ViewExpr::Match(_)
        | ViewExpr::ForEach(_)
        | ViewExpr::Await(_)
        | ViewExpr::Expr(_)
        | ViewExpr::Raw(_) => None,
    };
    applications.extend(modifiers.into_iter().flatten().filter_map(|modifier| {
        if let ViewModifier::Fx(application) = modifier {
            Some(application.as_ref())
        } else {
            None
        }
    }));

    match expr {
        ViewExpr::Fragment(children) => {
            for child in children {
                collect_fx_applications(child, applications);
            }
        }
        ViewExpr::Element(element) => {
            for child in element.children() {
                collect_fx_applications(child, applications);
            }
        }
        ViewExpr::If(view_if) => {
            collect_fx_applications(view_if.then_branch(), applications);
            if let Some(else_branch) = view_if.else_branch() {
                collect_fx_applications(else_branch, applications);
            }
        }
        ViewExpr::Match(view_match) => {
            for arm in view_match.arms() {
                collect_fx_applications(arm.value(), applications);
            }
        }
        ViewExpr::ForEach(view_for_each) => {
            collect_fx_applications(view_for_each.body(), applications);
        }
        ViewExpr::Await(view_await) => {
            for branch in view_await.branches() {
                collect_fx_applications(branch.value(), applications);
            }
        }
        ViewExpr::ViewCall(_)
        | ViewExpr::Text(_)
        | ViewExpr::Image(_)
        | ViewExpr::TextField(_)
        | ViewExpr::Button(_)
        | ViewExpr::Let(_)
        | ViewExpr::Expr(_)
        | ViewExpr::Raw(_) => {}
    }
}
