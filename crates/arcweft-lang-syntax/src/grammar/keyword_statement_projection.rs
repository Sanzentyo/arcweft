//! Parser-owned semantic projection for keyword control statements.

use arcweft_id::{LocaleTag, LocaleTagError};

use crate::grammar::kinds::SyntaxKind;
use crate::name::{SyntaxName, SyntaxNameIssue};

/// Parser-selected source form of one `select` statement.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxSelectStatementForm {
    /// Maintained unary `select EXPRESSION` form.
    Operand,
    /// Source-ordered branch body introduced by `select { ... }`.
    BranchBlock,
}

/// Parser-selected semantic head of one `select` branch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PendingSelectBranchProjection {
    Bind {
        name: Result<SyntaxName, SyntaxNameIssue>,
    },
    Frame,
    Event,
    Error,
}

impl PendingSelectBranchProjection {
    pub(crate) fn has_recovery(&self) -> bool {
        matches!(self, Self::Bind { name: Err(_), .. } | Self::Error)
    }
}

/// Closed await wait-view branch family.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxAwaitBranchKind {
    Pending,
}

/// Parser-selected semantic head of one `AwaitWith` branch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PendingAwaitBranchProjection {
    kind: Option<SyntaxAwaitBranchKind>,
}

impl PendingAwaitBranchProjection {
    pub(crate) const fn new(kind: SyntaxAwaitBranchKind) -> Self {
        Self { kind: Some(kind) }
    }

    pub(crate) const fn recovered() -> Self {
        Self { kind: None }
    }

    pub(crate) const fn kind(self) -> Option<SyntaxAwaitBranchKind> {
        self.kind
    }
}

/// Exact keyword-statement family and its optional typed control label.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PendingKeywordStatementProjection {
    Out {
        label: Option<Result<SyntaxName, SyntaxNameIssue>>,
    },
    Goto,
    Defer,
    Signal,
    Break {
        label: Option<Result<SyntaxName, SyntaxNameIssue>>,
    },
    Continue {
        label: Option<Result<SyntaxName, SyntaxNameIssue>>,
    },
    Choice,
    Select {
        form: SyntaxSelectStatementForm,
        branches: Box<[PendingSelectBranchProjection]>,
    },
    SourceLocale {
        locale: Option<Result<LocaleTag, LocaleTagError>>,
    },
    Scope {
        name: Option<Result<SyntaxName, SyntaxNameIssue>>,
    },
    Include,
}

impl PendingKeywordStatementProjection {
    pub(crate) const fn accepts_kind(&self, kind: SyntaxKind) -> bool {
        matches!(
            (self, kind),
            (Self::Out { .. }, SyntaxKind::OutStatement)
                | (Self::Goto, SyntaxKind::GotoStatement)
                | (Self::Defer, SyntaxKind::DeferStatement)
                | (Self::Signal, SyntaxKind::SignalStatement)
                | (Self::Break { .. }, SyntaxKind::BreakStatement)
                | (Self::Continue { .. }, SyntaxKind::ContinueStatement)
                | (Self::Choice, SyntaxKind::ChoiceStatement)
                | (Self::Select { .. }, SyntaxKind::SelectStatement)
                | (Self::SourceLocale { .. }, SyntaxKind::SourceLocaleStatement)
                | (Self::Scope { .. }, SyntaxKind::ScopeStatement)
                | (Self::Include, SyntaxKind::IncludeStatement)
        )
    }

    pub(crate) const fn label(&self) -> Option<&Result<SyntaxName, SyntaxNameIssue>> {
        match self {
            Self::Out { label } | Self::Break { label } | Self::Continue { label } => {
                label.as_ref()
            }
            Self::Goto
            | Self::Defer
            | Self::Signal
            | Self::Choice
            | Self::Select { .. }
            | Self::SourceLocale { .. }
            | Self::Scope { .. }
            | Self::Include => None,
        }
    }

    pub(crate) fn has_recovery(&self) -> bool {
        self.label().is_some_and(Result::is_err)
            || match self {
                Self::SourceLocale { locale } => locale.as_ref().is_none_or(Result::is_err),
                Self::Scope { name } => name.as_ref().is_some_and(Result::is_err),
                Self::Select { branches, .. } => branches
                    .iter()
                    .any(PendingSelectBranchProjection::has_recovery),
                _ => false,
            }
    }

    pub(crate) const fn kind_requires_projection(kind: SyntaxKind) -> bool {
        matches!(
            kind,
            SyntaxKind::OutStatement
                | SyntaxKind::GotoStatement
                | SyntaxKind::DeferStatement
                | SyntaxKind::SignalStatement
                | SyntaxKind::BreakStatement
                | SyntaxKind::ContinueStatement
                | SyntaxKind::ChoiceStatement
                | SyntaxKind::SelectStatement
                | SyntaxKind::SourceLocaleStatement
                | SyntaxKind::ScopeStatement
                | SyntaxKind::IncludeStatement
        )
    }
}
