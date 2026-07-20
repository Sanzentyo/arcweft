//! Bounded syntax-candidate facts retained without semantic name lookup.

use crate::{
    ast::{common::TextRange, dialogue::DialogueContent, line_plan::LinePlan},
    expr::Expr,
};
use thiserror::Error;

/// The two viable interpretations retained for semantic resolution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in super::super) enum PostfixBracketCandidates {
    Ambiguous {
        index: PostfixIndexCandidate,
        dialogue: PostfixDialogueCandidate,
    },
    Invalid {
        index: PostfixCandidateFailure,
        dialogue: PostfixCandidateFailure,
    },
}

/// Ordinary-expression payload retained for the index interpretation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in super::super) struct PostfixIndexCandidate {
    index: Box<Expr>,
    status: ApplicationRecoveryStatus,
}

/// Existing dialogue grammar payload retained for the dialogue interpretation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in super::super) struct PostfixDialogueCandidate {
    content: DialogueContent,
    plan: Option<LinePlan>,
    status: ApplicationRecoveryStatus,
}

/// Whether one viable postfix interpretation required parser recovery.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(in super::super) enum ApplicationRecoveryStatus {
    Clean,
    Recovered,
}

/// Bounded failure retained for one of the two postfix interpretations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in super::super) struct PostfixCandidateFailure {
    kind: PostfixCandidateFailureKind,
    site: PostfixCandidateFailureSite,
}

/// Grammar reason one postfix interpretation is not viable.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(in super::super) enum PostfixCandidateFailureKind {
    EmptyPayload,
    UnexpectedToken,
    MissingOperand,
    TrailingToken,
    InvalidDialogueAtom,
}

/// Exact syntax site associated with a bounded candidate failure.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(in super::super) enum PostfixCandidateFailureSite {
    Span(TextRange),
    Insertion(usize),
}

/// Invalid private candidate assembly detected before AST publication.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(in super::super) enum PostfixCandidateInvariantError {
    #[error("a clean dialogue candidate must retain at least one dialogue token")]
    EmptyCleanDialogue,
    #[error("a clean dialogue candidate cannot retain recovery diagnostics")]
    RecoveredDialogueMarkedClean,
}

impl ApplicationRecoveryStatus {
    pub const fn is_recovered(self) -> bool {
        matches!(self, Self::Recovered)
    }
}

impl PostfixCandidateFailure {
    pub(in super::super) const fn new(
        kind: PostfixCandidateFailureKind,
        site: PostfixCandidateFailureSite,
    ) -> Self {
        Self { kind, site }
    }

    pub const fn kind(&self) -> PostfixCandidateFailureKind {
        self.kind
    }

    pub const fn site(&self) -> PostfixCandidateFailureSite {
        self.site
    }
}

impl PostfixIndexCandidate {
    pub(in super::super) const fn new(index: Box<Expr>, status: ApplicationRecoveryStatus) -> Self {
        Self { index, status }
    }

    pub const fn index(&self) -> &Expr {
        &self.index
    }

    pub const fn status(&self) -> ApplicationRecoveryStatus {
        self.status
    }
}

impl PostfixDialogueCandidate {
    pub(in super::super) fn try_new(
        content: DialogueContent,
        plan: Option<LinePlan>,
        status: ApplicationRecoveryStatus,
    ) -> Result<Self, PostfixCandidateInvariantError> {
        if status == ApplicationRecoveryStatus::Clean && content.tokens().is_empty() {
            return Err(PostfixCandidateInvariantError::EmptyCleanDialogue);
        }
        if status == ApplicationRecoveryStatus::Clean && !content.diagnostics().is_empty() {
            return Err(PostfixCandidateInvariantError::RecoveredDialogueMarkedClean);
        }
        Ok(Self {
            content,
            plan,
            status,
        })
    }

    pub const fn content(&self) -> &DialogueContent {
        &self.content
    }

    pub const fn plan(&self) -> Option<&LinePlan> {
        self.plan.as_ref()
    }

    pub const fn status(&self) -> ApplicationRecoveryStatus {
        self.status
    }
}

impl PostfixBracketCandidates {
    pub(in super::super) const fn ambiguous(
        index: PostfixIndexCandidate,
        dialogue: PostfixDialogueCandidate,
    ) -> Self {
        Self::Ambiguous { index, dialogue }
    }

    pub(in super::super) const fn invalid(
        index: PostfixCandidateFailure,
        dialogue: PostfixCandidateFailure,
    ) -> Self {
        Self::Invalid { index, dialogue }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ApplicationRecoveryStatus, PostfixCandidateFailure, PostfixCandidateFailureKind,
        PostfixCandidateFailureSite,
    };
    use crate::ast::common::TextRange;

    #[test]
    fn candidate_failure_retains_one_bounded_typed_site() {
        let failure = PostfixCandidateFailure::new(
            PostfixCandidateFailureKind::TrailingToken,
            PostfixCandidateFailureSite::Span(TextRange::new(8, 9)),
        );

        assert_eq!(failure.kind(), PostfixCandidateFailureKind::TrailingToken);
        assert_eq!(
            failure.site(),
            PostfixCandidateFailureSite::Span(TextRange::new(8, 9))
        );
        assert!(!ApplicationRecoveryStatus::Clean.is_recovered());
        assert!(ApplicationRecoveryStatus::Recovered.is_recovered());
    }
}
