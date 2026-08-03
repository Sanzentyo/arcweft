//! Non-authored semantic values introduced by one `for` statement.
//!
//! These payloads are not fabricated Calls. They retain the exact data-flow
//! edges that semantic trait resolution must discharge while their qualified
//! expression IDs remain statement-owned synthetic identities.

use crate::identity::{ExprId, HirModuleId};

/// One non-lexical value introduced by `for PATTERN in SOURCE BODY`.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirForSyntheticExpr {
    /// The one-time `IntoIterator::into_iter(SOURCE)` semantic value.
    Iterator { source: ExprId },
    /// The successful `Iterator::next` item for one loop iteration.
    NextValue { iterator: ExprId },
}

impl HirForSyntheticExpr {
    pub(crate) const fn iterator(source: ExprId) -> Self {
        Self::Iterator { source }
    }

    pub(crate) const fn next_value(iterator: ExprId) -> Self {
        Self::NextValue { iterator }
    }

    /// Returns the sole semantic input edge of this synthetic value.
    pub const fn input(&self) -> ExprId {
        match self {
            Self::Iterator { source } => *source,
            Self::NextValue { iterator } => *iterator,
        }
    }

    pub(crate) fn validate_module(
        &self,
        expected: HirModuleId,
    ) -> Result<(), super::HirExprInvariantError> {
        super::validate_expr(expected, self.input())
    }
}
