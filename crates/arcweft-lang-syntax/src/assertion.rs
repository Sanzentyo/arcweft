//! Shared syntax vocabulary for proof and runtime assertion statements.

use crate::ast::common::TextRange;
use crate::expr::Expr;

/// Source assertion mode selected after `assert.`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AssertionMode {
    Prove,
    Check,
    Debug,
}

impl AssertionMode {
    /// Resolves one canonical mode keyword without duplicating string matches.
    pub fn from_keyword(keyword: &str) -> Option<Self> {
        match keyword {
            "prove" => Some(Self::Prove),
            "check" => Some(Self::Check),
            "debug" => Some(Self::Debug),
            _ => None,
        }
    }

    /// Canonical source keyword for this mode.
    pub const fn keyword(self) -> &'static str {
        match self {
            Self::Prove => "prove",
            Self::Check => "check",
            Self::Debug => "debug",
        }
    }

    /// Whether this mode always emits a runtime guard in release output.
    pub const fn has_release_runtime_instruction(self) -> bool {
        matches!(self, Self::Check)
    }

    /// Whether this mode can produce a runtime assertion guard.
    pub const fn is_runtime_capable(self) -> bool {
        matches!(self, Self::Check | Self::Debug)
    }

    /// Fact class produced by a successfully established assertion.
    pub const fn facts(self) -> AssertionFactClass {
        match self {
            Self::Prove | Self::Check => AssertionFactClass::Release,
            Self::Debug => AssertionFactClass::DebugOnly,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AssertionExpressionCall {
    Known(AssertionMode),
    UnknownMode,
}

pub(crate) fn classify_expression_call(expr: &Expr) -> Option<AssertionExpressionCall> {
    let Expr::Call(call) = expr else {
        return None;
    };
    let Expr::Select(select) = call.callee() else {
        return None;
    };
    let Expr::Path(path) = select.target() else {
        return None;
    };
    if path.as_str() != "assert" {
        return None;
    }
    Some(
        AssertionMode::from_keyword(select.member().as_str()).map_or(
            AssertionExpressionCall::UnknownMode,
            AssertionExpressionCall::Known,
        ),
    )
}

/// Safety domain in which assertion-derived facts are valid.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssertionFactClass {
    Release,
    DebugOnly,
}

/// Typed `assert.prove/check/debug` statement payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssertionStmt {
    mode: AssertionMode,
    conditions: Box<[Expr]>,
    range: TextRange,
    callee_range: TextRange,
    mode_range: TextRange,
    arguments_range: TextRange,
}

impl AssertionStmt {
    pub(crate) fn new(
        mode: AssertionMode,
        conditions: Vec<Expr>,
        range: TextRange,
        callee_range: TextRange,
        mode_range: TextRange,
        arguments_range: TextRange,
    ) -> Self {
        Self {
            mode,
            conditions: conditions.into_boxed_slice(),
            range,
            callee_range,
            mode_range,
            arguments_range,
        }
    }

    /// Returns the selected assertion mode.
    pub const fn mode(&self) -> AssertionMode {
        self.mode
    }

    /// Returns conditions in authored evaluation order.
    pub const fn conditions(&self) -> &[Expr] {
        &self.conditions
    }

    /// Returns the complete statement range.
    pub const fn range(&self) -> TextRange {
        self.range
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

#[cfg(test)]
mod tests {
    use super::{AssertionFactClass, AssertionMode, AssertionStmt};
    use crate::ast::common::TextRange;
    use crate::expr::{DottedPath, Expr};

    #[test]
    fn assertion_modes_own_runtime_and_fact_policy() {
        assert_eq!(AssertionMode::Prove.keyword(), "prove");
        assert_eq!(
            AssertionMode::from_keyword("prove"),
            Some(AssertionMode::Prove)
        );
        assert_eq!(AssertionMode::from_keyword("other"), None);
        assert!(!AssertionMode::Prove.has_release_runtime_instruction());
        assert!(!AssertionMode::Prove.is_runtime_capable());
        assert_eq!(AssertionMode::Prove.facts(), AssertionFactClass::Release);

        assert_eq!(AssertionMode::Check.keyword(), "check");
        assert!(AssertionMode::Check.has_release_runtime_instruction());
        assert!(AssertionMode::Check.is_runtime_capable());
        assert_eq!(AssertionMode::Check.facts(), AssertionFactClass::Release);

        assert_eq!(AssertionMode::Debug.keyword(), "debug");
        assert!(!AssertionMode::Debug.has_release_runtime_instruction());
        assert!(AssertionMode::Debug.is_runtime_capable());
        assert_eq!(AssertionMode::Debug.facts(), AssertionFactClass::DebugOnly);
    }

    #[test]
    fn assertion_statement_retains_order_and_exact_ranges() {
        let statement = AssertionStmt {
            mode: AssertionMode::Check,
            conditions: vec![
                Expr::Path(DottedPath::single("first")),
                Expr::Path(DottedPath::single("second")),
            ]
            .into_boxed_slice(),
            range: TextRange::new(4, 32),
            callee_range: TextRange::new(4, 16),
            mode_range: TextRange::new(11, 16),
            arguments_range: TextRange::new(16, 32),
        };
        assert_eq!(statement.mode(), AssertionMode::Check);
        assert_eq!(statement.conditions().len(), 2);
        assert!(matches!(&statement.conditions()[0], Expr::Path(path) if path.as_str() == "first"));
        assert_eq!(statement.range(), TextRange::new(4, 32));
        assert_eq!(statement.callee_range(), TextRange::new(4, 16));
        assert_eq!(statement.mode_range(), TextRange::new(11, 16));
        assert_eq!(statement.arguments_range(), TextRange::new(16, 32));
    }
}
