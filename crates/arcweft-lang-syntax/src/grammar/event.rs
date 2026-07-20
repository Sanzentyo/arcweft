//! Balanced grammar events shared by document and nested parsers.

#![allow(
    dead_code,
    reason = "the shadow grammar remains crate-private until the atomic syntax switch"
)]

use arcweft_source::SourceRange;

use super::kinds::{SyntaxKind, SyntaxRole};

/// Token class expected at a zero-width recovery insertion.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct ExpectedToken(SyntaxKind);

impl ExpectedToken {
    /// Creates an expected-token value from a real token kind.
    pub(crate) fn try_new(kind: SyntaxKind) -> Option<Self> {
        (kind.is_token() && !matches!(kind, SyntaxKind::MissingToken | SyntaxKind::EofToken))
            .then_some(Self(kind))
    }

    /// Returns the expected grammar token kind.
    pub(crate) const fn kind(self) -> SyntaxKind {
        self.0
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
}
