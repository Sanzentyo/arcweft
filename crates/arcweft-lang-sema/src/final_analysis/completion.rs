//! One typed algebra for normal continuation before and after the ownership seal.
//! Never prefixes do not resume; alternatives resume if either branch can.
//! The checked expression result remains the semantic authority, including non-value emissions.
use super::prepared::{PreparedExpressionResult, PreparedNonValueExpressionResult};
use super::{CheckedExpressionResult, CheckedNonValueExpressionResult};
use crate::types::TypeKind;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NormalContinuation {
    Unit,
    Never,
}
impl NormalContinuation {
    pub(crate) const fn is_never(self) -> bool {
        matches!(self, Self::Never)
    }
    pub(crate) const fn is_unit(self) -> bool {
        matches!(self, Self::Unit)
    }
    pub(crate) const fn then(self, next: Self) -> Self {
        match self {
            Self::Unit => next,
            Self::Never => Self::Never,
        }
    }
    pub(crate) const fn join(self, other: Self) -> Self {
        match (self, other) {
            (Self::Never, Self::Never) => Self::Never,
            _ => Self::Unit,
        }
    }
    /// A typed emission resumes without publishing a value or a Unit type.
    /// Rejection and ambiguity remain unavailable, rather than continuation.
    pub(crate) fn from_checked_result(result: &CheckedExpressionResult) -> Option<Self> {
        match result {
            CheckedExpressionResult::Value(value) => Some(Self::from_value_type(value.ty())),
            CheckedExpressionResult::NonValue(
                CheckedNonValueExpressionResult::ContentEmission(_),
            ) => Some(Self::Unit),
            CheckedExpressionResult::Unavailable => None,
        }
    }
    pub(crate) fn from_prepared_result(result: &PreparedExpressionResult) -> Self {
        match result {
            PreparedExpressionResult::Value(value) => Self::from_value_type(value.ty()),
            PreparedExpressionResult::NonValue(
                PreparedNonValueExpressionResult::ContentEmission(_),
            ) => Self::Unit,
        }
    }
    pub(crate) fn from_value_type(ty: &TypeKind) -> Self {
        if ty == &TypeKind::Never {
            Self::Never
        } else {
            Self::Unit
        }
    }
}
