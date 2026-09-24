//! Distinct semantic applications may belong to the same HIR expression.

use arcweft_lang_hir::identity::ExprId;

use super::CallConstraintInvariant;

/// Constraint-component identity, independent of the source expression's
/// runtime topology. A call returning a known scheme can also be specialized
/// at that same value use without replacing either application's evidence.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum CallableConstraintApplication {
    Call(ExprId),
    Specialize(ExprId),
}

impl CallableConstraintApplication {
    pub(crate) const fn expression(self) -> ExprId {
        match self {
            Self::Call(expression) | Self::Specialize(expression) => expression,
        }
    }

    pub(crate) fn require_call(self) -> Result<ExprId, CallConstraintInvariant> {
        match self {
            Self::Call(expression) => Ok(expression),
            Self::Specialize(_) => Err(CallConstraintInvariant::PreparedCallSiteMismatch),
        }
    }
}
