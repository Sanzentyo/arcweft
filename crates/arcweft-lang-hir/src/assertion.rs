//! Typed HIR assertion payloads.

use crate::identity::ExprId;
use arcweft_lang_syntax::assertion::AssertionMode;
use arcweft_lang_syntax::ast::common::TextRange;

/// Lowered assertion conditions and their exact surface ranges.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirAssertion {
    mode: AssertionMode,
    conditions: Box<[ExprId]>,
    callee_range: TextRange,
    mode_range: TextRange,
    arguments_range: TextRange,
}

impl HirAssertion {
    /// Returns the selected assertion mode.
    pub const fn mode(&self) -> AssertionMode {
        self.mode
    }

    /// Returns condition identities in authored evaluation order.
    pub const fn conditions(&self) -> &[ExprId] {
        &self.conditions
    }

    /// Returns the range covering `assert.<mode>`.
    pub const fn callee_range(&self) -> TextRange {
        self.callee_range
    }

    /// Returns the exact mode identifier range.
    pub const fn mode_range(&self) -> TextRange {
        self.mode_range
    }

    /// Returns the parentheses-inclusive argument range.
    pub const fn arguments_range(&self) -> TextRange {
        self.arguments_range
    }
}
