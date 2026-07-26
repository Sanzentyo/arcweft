//! Balanced grammar events shared by document and nested parsers.

#![allow(
    dead_code,
    reason = "the shadow grammar remains crate-private until the atomic syntax switch"
)]

use arcweft_source::SourceRange;

use super::kinds::{SyntaxKind, SyntaxRole};

/// Token class expected at a zero-width recovery insertion.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct ExpectedToken {
    kind: SyntaxKind,
    spelling: Option<&'static str>,
}

impl ExpectedToken {
    /// Creates an expected-token value from a real token kind.
    pub(crate) fn try_new(kind: SyntaxKind) -> Option<Self> {
        (kind.is_token() && !matches!(kind, SyntaxKind::MissingToken | SyntaxKind::EofToken))
            .then_some(Self {
                kind,
                spelling: None,
            })
    }

    pub(crate) fn try_with_spelling(kind: SyntaxKind, spelling: &'static str) -> Option<Self> {
        Self::try_new(kind).map(|expected| Self {
            spelling: Some(spelling),
            ..expected
        })
    }

    /// Returns the expected grammar token kind.
    pub(crate) const fn kind(self) -> SyntaxKind {
        self.kind
    }

    pub(crate) const fn spelling(self) -> Option<&'static str> {
        self.spelling
    }
}

/// Diagnostic staged by the event parser before snapshot attachment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingSyntaxDiagnostic {
    code: &'static str,
    range: SourceRange,
    related_range: Option<SourceRange>,
    message: String,
}

impl PendingSyntaxDiagnostic {
    pub(crate) fn new(code: &'static str, range: SourceRange, message: impl Into<String>) -> Self {
        Self {
            code,
            range,
            related_range: None,
            message: message.into(),
        }
    }

    pub(crate) const fn with_related_range(mut self, related_range: SourceRange) -> Self {
        self.related_range = Some(related_range);
        self
    }

    pub(crate) const fn code(&self) -> &'static str {
        self.code
    }

    pub(crate) const fn range(&self) -> SourceRange {
        self.range
    }

    pub(crate) const fn related_range(&self) -> Option<SourceRange> {
        self.related_range
    }

    pub(crate) fn message(&self) -> &str {
        &self.message
    }

    pub(crate) fn rebased(&self, offset: usize) -> Option<Self> {
        Some(Self {
            code: self.code,
            range: rebase_range(self.range, offset)?,
            related_range: match self.related_range {
                Some(range) => Some(rebase_range(range, offset)?),
                None => None,
            },
            message: self.message.clone(),
        })
    }
}

/// One event in the single lossless grammar stream.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SyntaxEvent {
    StartNode {
        kind: SyntaxKind,
        role: SyntaxRole,
    },
    Token {
        kind: SyntaxKind,
        range: SourceRange,
    },
    MissingToken {
        expected: ExpectedToken,
        at: usize,
    },
    Diagnostic(PendingSyntaxDiagnostic),
    FinishNode,
}

impl SyntaxEvent {
    pub(crate) const fn start(kind: SyntaxKind, role: SyntaxRole) -> Self {
        Self::StartNode { kind, role }
    }

    pub(crate) const fn token(kind: SyntaxKind, range: SourceRange) -> Self {
        Self::Token { kind, range }
    }

    pub(crate) fn rebased(&self, offset: usize) -> Option<Self> {
        match self {
            Self::StartNode { kind, role } => Some(Self::StartNode {
                kind: *kind,
                role: *role,
            }),
            Self::Token { kind, range } => Some(Self::Token {
                kind: *kind,
                range: rebase_range(*range, offset)?,
            }),
            Self::MissingToken { expected, at } => Some(Self::MissingToken {
                expected: *expected,
                at: at.checked_add(offset)?,
            }),
            Self::Diagnostic(diagnostic) => Some(Self::Diagnostic(diagnostic.rebased(offset)?)),
            Self::FinishNode => Some(Self::FinishNode),
        }
    }
}

fn rebase_range(range: SourceRange, offset: usize) -> Option<SourceRange> {
    Some(SourceRange::new(
        range.start().checked_add(offset)?,
        range.end().checked_add(offset)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::{ExpectedToken, PendingSyntaxDiagnostic, SyntaxEvent};
    use crate::grammar::kinds::{SyntaxKind, SyntaxRole};
    use arcweft_source::SourceRange;

    #[test]
    fn fragment_events_rebase_every_source_coordinate_exactly() {
        let expected = ExpectedToken::try_with_spelling(SyntaxKind::PunctuationToken, ")")
            .expect("real expected token");
        let diagnostic = PendingSyntaxDiagnostic::new(
            "syntax.fragment.test",
            SourceRange::new(1, 3),
            "test diagnostic",
        )
        .with_related_range(SourceRange::new(0, 1));

        assert_eq!(
            SyntaxEvent::start(SyntaxKind::CallExpression, SyntaxRole::Element(0)).rebased(8),
            Some(SyntaxEvent::start(
                SyntaxKind::CallExpression,
                SyntaxRole::Element(0)
            ))
        );
        assert_eq!(
            SyntaxEvent::token(SyntaxKind::IdentifierToken, SourceRange::new(0, 3)).rebased(8),
            Some(SyntaxEvent::token(
                SyntaxKind::IdentifierToken,
                SourceRange::new(8, 11)
            ))
        );
        assert_eq!(
            SyntaxEvent::MissingToken { expected, at: 3 }.rebased(8),
            Some(SyntaxEvent::MissingToken { expected, at: 11 })
        );
        assert_eq!(
            SyntaxEvent::Diagnostic(diagnostic).rebased(8),
            Some(SyntaxEvent::Diagnostic(
                PendingSyntaxDiagnostic::new(
                    "syntax.fragment.test",
                    SourceRange::new(9, 11),
                    "test diagnostic"
                )
                .with_related_range(SourceRange::new(8, 9))
            ))
        );
        assert_eq!(
            SyntaxEvent::FinishNode.rebased(8),
            Some(SyntaxEvent::FinishNode)
        );
    }

    #[test]
    fn fragment_event_rebase_rejects_coordinate_overflow() {
        let expected =
            ExpectedToken::try_new(SyntaxKind::IdentifierToken).expect("real expected token");
        assert_eq!(
            SyntaxEvent::token(
                SyntaxKind::IdentifierToken,
                SourceRange::new(usize::MAX, usize::MAX)
            )
            .rebased(1),
            None
        );
        assert_eq!(
            SyntaxEvent::MissingToken {
                expected,
                at: usize::MAX
            }
            .rebased(1),
            None
        );
        assert_eq!(
            SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
                "syntax.fragment.overflow",
                SourceRange::new(usize::MAX, usize::MAX),
                "overflow"
            ))
            .rebased(1),
            None
        );
    }
}
