//! Candidate-local views for keyword-owned control statements.

use super::{
    AttachedCandidateNode, AttachedCandidateStatement, AttachedCandidateStatementExpression,
    exact_optional_child,
};
use crate::grammar::keyword_statement_projection::PendingKeywordStatementProjection;
use crate::grammar::{SyntaxKind, SyntaxRole};
use crate::name::{SyntaxName, SyntaxNameIssue};

/// One candidate-local control label and its parser-owned typed value.
#[derive(Clone, Copy)]
pub struct AttachedCandidateControlLabel<'a> {
    syntax: AttachedCandidateNode<'a>,
    value: &'a Result<SyntaxName, SyntaxNameIssue>,
}

impl<'a> AttachedCandidateControlLabel<'a> {
    /// Exact candidate-local `NameReference` selected by the grammar.
    pub const fn syntax(self) -> AttachedCandidateNode<'a> {
        self.syntax
    }

    /// Validated label name, or the parser-owned typed name issue.
    pub fn value(self) -> Result<&'a SyntaxName, &'a SyntaxNameIssue> {
        self.value.as_ref()
    }

    /// Whether the authored label token is not a valid control label.
    pub const fn is_recovered(self) -> bool {
        self.value.is_err()
    }
}

/// Exact typed payload of one candidate-local keyword statement.
#[derive(Clone, Copy)]
pub enum AttachedCandidateKeywordStatement<'a> {
    Out {
        statement: AttachedCandidateStatement<'a>,
        label: Option<AttachedCandidateControlLabel<'a>>,
        value: AttachedCandidateStatementExpression<'a>,
    },
    Goto {
        statement: AttachedCandidateStatement<'a>,
        target: AttachedCandidateStatementExpression<'a>,
    },
    Defer {
        statement: AttachedCandidateStatement<'a>,
        expression: AttachedCandidateStatementExpression<'a>,
    },
    Signal {
        statement: AttachedCandidateStatement<'a>,
        target: AttachedCandidateStatementExpression<'a>,
        value: AttachedCandidateStatementExpression<'a>,
        arrow_recovery: Option<AttachedCandidateNode<'a>>,
    },
    Break {
        statement: AttachedCandidateStatement<'a>,
        label: Option<AttachedCandidateControlLabel<'a>>,
        value: Option<AttachedCandidateStatementExpression<'a>>,
    },
    Continue {
        statement: AttachedCandidateStatement<'a>,
        label: Option<AttachedCandidateControlLabel<'a>>,
        forbidden_suffix: Option<AttachedCandidateNode<'a>>,
    },
}

impl<'a> AttachedCandidateKeywordStatement<'a> {
    /// Candidate-local statement that owns this exact payload.
    pub const fn statement(self) -> AttachedCandidateStatement<'a> {
        match self {
            Self::Out { statement, .. }
            | Self::Goto { statement, .. }
            | Self::Defer { statement, .. }
            | Self::Signal { statement, .. }
            | Self::Break { statement, .. }
            | Self::Continue { statement, .. } => statement,
        }
    }
}

impl<'a> AttachedCandidateStatement<'a> {
    /// Complete keyword-statement relation selected without source reads.
    pub fn keyword_statement_view(self) -> Option<AttachedCandidateKeywordStatement<'a>> {
        let projection = self.node.keyword_statement_projection()?;
        match (self.kind(), projection) {
            (SyntaxKind::OutStatement, PendingKeywordStatementProjection::Out { label }) => {
                candidate_roles(self.node, &[SyntaxRole::Label(0), SyntaxRole::Initializer])?;
                Some(AttachedCandidateKeywordStatement::Out {
                    statement: self,
                    label: candidate_label(self.node, label.as_ref())?,
                    value: self.required_expression(SyntaxRole::Initializer)?,
                })
            }
            (SyntaxKind::GotoStatement, PendingKeywordStatementProjection::Goto) => {
                candidate_roles(self.node, &[SyntaxRole::Target])?;
                Some(AttachedCandidateKeywordStatement::Goto {
                    statement: self,
                    target: self.required_expression(SyntaxRole::Target)?,
                })
            }
            (SyntaxKind::DeferStatement, PendingKeywordStatementProjection::Defer) => {
                candidate_roles(self.node, &[SyntaxRole::Initializer])?;
                Some(AttachedCandidateKeywordStatement::Defer {
                    statement: self,
                    expression: self.required_expression(SyntaxRole::Initializer)?,
                })
            }
            (SyntaxKind::SignalStatement, PendingKeywordStatementProjection::Signal) => {
                candidate_roles(
                    self.node,
                    &[
                        SyntaxRole::Target,
                        SyntaxRole::Initializer,
                        SyntaxRole::Recovery(0),
                    ],
                )?;
                Some(AttachedCandidateKeywordStatement::Signal {
                    statement: self,
                    target: self.required_expression(SyntaxRole::Target)?,
                    value: self.required_expression(SyntaxRole::Initializer)?,
                    arrow_recovery: candidate_recovery(self.node)?,
                })
            }
            (SyntaxKind::BreakStatement, PendingKeywordStatementProjection::Break { label }) => {
                candidate_roles(self.node, &[SyntaxRole::Label(0), SyntaxRole::Initializer])?;
                Some(AttachedCandidateKeywordStatement::Break {
                    statement: self,
                    label: candidate_label(self.node, label.as_ref())?,
                    value: self.optional_expression(SyntaxRole::Initializer)?,
                })
            }
            (
                SyntaxKind::ContinueStatement,
                PendingKeywordStatementProjection::Continue { label },
            ) => {
                candidate_roles(self.node, &[SyntaxRole::Label(0), SyntaxRole::Recovery(0)])?;
                Some(AttachedCandidateKeywordStatement::Continue {
                    statement: self,
                    label: candidate_label(self.node, label.as_ref())?,
                    forbidden_suffix: candidate_recovery(self.node)?,
                })
            }
            _ => None,
        }
    }
}

#[allow(
    clippy::option_option,
    reason = "outer absence rejects an invalid candidate relation while inner absence is an omitted optional label"
)]
fn candidate_label<'a>(
    owner: AttachedCandidateNode<'a>,
    value: Option<&'a Result<SyntaxName, SyntaxNameIssue>>,
) -> Option<Option<AttachedCandidateControlLabel<'a>>> {
    let syntax = exact_optional_child(owner, SyntaxRole::Label(0))?;
    match (syntax, value) {
        (None, None) => Some(None),
        (Some(syntax), Some(value)) if syntax.kind() == SyntaxKind::NameReference => {
            Some(Some(AttachedCandidateControlLabel { syntax, value }))
        }
        _ => None,
    }
}

#[allow(
    clippy::option_option,
    reason = "outer absence rejects an invalid candidate relation while inner absence is an omitted recovery node"
)]
fn candidate_recovery(
    owner: AttachedCandidateNode<'_>,
) -> Option<Option<AttachedCandidateNode<'_>>> {
    match exact_optional_child(owner, SyntaxRole::Recovery(0))? {
        None => Some(None),
        Some(recovery) if recovery.kind() == SyntaxKind::ErrorNode => Some(Some(recovery)),
        Some(_) => None,
    }
}

fn candidate_roles(owner: AttachedCandidateNode<'_>, accepted: &[SyntaxRole]) -> Option<()> {
    owner
        .children()
        .all(|child| accepted.contains(&child.role()))
        .then_some(())
}
