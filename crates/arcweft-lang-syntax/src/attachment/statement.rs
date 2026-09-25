//! Typed attachment views for keyword-owned control statements.

use super::SyntaxAccessError;
use super::access::RequiredStatementExpressionNode;
use super::expression::AttachedExpressionNode;
use super::family::ExpressionFamily;
use super::family::{StatementFamily, StatementNode};
use super::node::{
    AstKind, AstNode, BreakStatementKind, ContinueStatementKind, DeferBlockStatementKind,
    DeferStatementKind, ErrorNodeKind, GotoStatementKind, NameReferenceKind, OnStatementKind,
    OutStatementKind, SignalStatementKind,
};
use super::node::{BlockKind, ColonKind, MissingBodyKind};
use super::trigger::{AttachedTriggerPattern, attach_trigger_pattern};
use crate::ast::line_plan::DeferOutcome;
use crate::grammar::keyword_statement_projection::PendingKeywordStatementProjection;
use crate::grammar::{SyntaxKind, SyntaxRole, SyntaxRoleClass};
use crate::name::{SyntaxName, SyntaxNameIssue};
use arcweft_source::SourceSpan;

/// One parser-classified control label and its exact CST owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedControlLabel {
    syntax: AstNode<NameReferenceKind>,
    value: Result<SyntaxName, SyntaxNameIssue>,
}

impl AttachedControlLabel {
    /// Exact `NameReference` selected by the statement grammar.
    pub const fn syntax(&self) -> &AstNode<NameReferenceKind> {
        &self.syntax
    }

    /// Validated label name, or the parser-owned typed name issue.
    pub fn value(&self) -> Result<&SyntaxName, &SyntaxNameIssue> {
        self.value.as_ref()
    }

    /// Whether the authored label token is not a valid control label.
    pub const fn is_recovered(&self) -> bool {
        self.value.is_err()
    }
}

/// Complete typed `out` statement relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedOutStatement {
    syntax: AstNode<OutStatementKind>,
    label: Option<AttachedControlLabel>,
    value: RequiredStatementExpressionNode,
}

impl AttachedOutStatement {
    pub const fn syntax(&self) -> &AstNode<OutStatementKind> {
        &self.syntax
    }

    pub const fn label(&self) -> Option<&AttachedControlLabel> {
        self.label.as_ref()
    }

    pub const fn value(&self) -> &RequiredStatementExpressionNode {
        &self.value
    }
}

/// Complete typed `goto` statement relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedGotoStatement {
    syntax: AstNode<GotoStatementKind>,
    target: RequiredStatementExpressionNode,
}

impl AttachedGotoStatement {
    pub const fn syntax(&self) -> &AstNode<GotoStatementKind> {
        &self.syntax
    }

    pub const fn target(&self) -> &RequiredStatementExpressionNode {
        &self.target
    }
}

/// Complete typed expression-form `defer` statement relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedDeferStatement {
    syntax: AstNode<DeferStatementKind>,
    expression: RequiredStatementExpressionNode,
}

impl AttachedDeferStatement {
    pub const fn syntax(&self) -> &AstNode<DeferStatementKind> {
        &self.syntax
    }

    pub const fn expression(&self) -> &RequiredStatementExpressionNode {
        &self.expression
    }
}

/// Typed body owned by a block-form `defer` statement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedDeferBlockBody {
    Expression(AttachedExpressionNode),
    Missing(AstNode<MissingBodyKind>),
}

impl AttachedDeferBlockBody {
    /// Exact source owner of the authored or recovered cleanup body.
    pub fn source_span(&self) -> SourceSpan {
        match self {
            Self::Expression(expression) => expression.whole_source_span(),
            Self::Missing(missing) => missing.source_span(),
        }
    }

    pub const fn expression(&self) -> Option<&AttachedExpressionNode> {
        match self {
            Self::Expression(expression) => Some(expression),
            Self::Missing(_) => None,
        }
    }
}

/// Complete typed block-form `defer` relation, including outcome ownership.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedDeferBlockStatement {
    syntax: AstNode<DeferBlockStatementKind>,
    outcome: DeferOutcome,
    outcome_source: Option<AstNode<NameReferenceKind>>,
    body: AttachedDeferBlockBody,
}

impl AttachedDeferBlockStatement {
    pub const fn syntax(&self) -> &AstNode<DeferBlockStatementKind> {
        &self.syntax
    }

    pub const fn outcome(&self) -> DeferOutcome {
        self.outcome
    }

    /// Exact source span of the authored outcome word, absent for `Always`.
    pub fn outcome_source_span(&self) -> Option<SourceSpan> {
        self.outcome_source.as_ref().map(AstNode::source_span)
    }

    /// Exact name node that supplied the qualified outcome, when authored.
    pub const fn outcome_source(&self) -> Option<&AstNode<NameReferenceKind>> {
        self.outcome_source.as_ref()
    }

    pub const fn body(&self) -> &AttachedDeferBlockBody {
        &self.body
    }
}

/// Complete typed `signal TARGET <- VALUE` statement relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedSignalStatement {
    syntax: AstNode<SignalStatementKind>,
    target: RequiredStatementExpressionNode,
    value: RequiredStatementExpressionNode,
    arrow_recovery: Option<AstNode<ErrorNodeKind>>,
}

impl AttachedSignalStatement {
    pub const fn syntax(&self) -> &AstNode<SignalStatementKind> {
        &self.syntax
    }

    pub const fn target(&self) -> &RequiredStatementExpressionNode {
        &self.target
    }

    pub const fn value(&self) -> &RequiredStatementExpressionNode {
        &self.value
    }

    /// Exact zero-width recovery node when the required `<-` is absent.
    pub const fn arrow_recovery(&self) -> Option<&AstNode<ErrorNodeKind>> {
        self.arrow_recovery.as_ref()
    }
}

/// Complete typed `break` statement relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedBreakStatement {
    syntax: AstNode<BreakStatementKind>,
    label: Option<AttachedControlLabel>,
    value: Option<AttachedExpressionNode>,
}

impl AttachedBreakStatement {
    pub const fn syntax(&self) -> &AstNode<BreakStatementKind> {
        &self.syntax
    }

    pub const fn label(&self) -> Option<&AttachedControlLabel> {
        self.label.as_ref()
    }

    pub const fn value(&self) -> Option<&AttachedExpressionNode> {
        self.value.as_ref()
    }
}

/// Complete typed `continue` statement relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedContinueStatement {
    syntax: AstNode<ContinueStatementKind>,
    label: Option<AttachedControlLabel>,
    forbidden_suffix: Option<AstNode<ErrorNodeKind>>,
}

/// The exact source form of one event handler body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedOnStatementBody {
    Arrow(StatementNode),
    Braced(AstNode<BlockKind>),
    Indented {
        colon: AstNode<ColonKind>,
        block: AstNode<BlockKind>,
    },
    Missing(AstNode<MissingBodyKind>),
}

impl AttachedOnStatementBody {
    /// Statements executed by the handler in their authored order.
    pub fn statements(&self) -> Result<Vec<StatementNode>, SyntaxAccessError> {
        match self {
            Self::Arrow(statement) => Ok(vec![statement.clone()]),
            Self::Braced(block) | Self::Indented { block, .. } => block.statements(),
            Self::Missing(_) => Ok(Vec::new()),
        }
    }

    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Arrow(statement) => statement.kind() == SyntaxKind::ErrorStatement,
            Self::Braced(block) => {
                block
                    .close_delimiter()
                    .map_or(true, |close| close.range().is_empty())
                    || block.statements().map_or(true, |items| {
                        items
                            .iter()
                            .any(|item| item.kind() == SyntaxKind::ErrorStatement)
                    })
            }
            Self::Indented { block, .. } => block.statements().map_or(true, |items| {
                items
                    .iter()
                    .any(|item| item.kind() == SyntaxKind::ErrorStatement)
            }),
            Self::Missing(_) => true,
        }
    }
}

/// Complete typed `on TRIGGER` handler relation.
///
/// The trigger attachment retains a typed marker selector when the trigger is
/// `mark`; final HIR resolves that selector against the owning dialogue
/// content catalog and never reconstructs it from source text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedOnStatement {
    syntax: AstNode<OnStatementKind>,
    trigger: AttachedTriggerPattern,
    body: AttachedOnStatementBody,
}

impl AttachedOnStatement {
    pub const fn syntax(&self) -> &AstNode<OnStatementKind> {
        &self.syntax
    }

    pub const fn trigger(&self) -> &AttachedTriggerPattern {
        &self.trigger
    }

    /// The authored body evaluated when the trigger is accepted.
    pub const fn body(&self) -> &AttachedOnStatementBody {
        &self.body
    }

    pub fn has_recovery(&self) -> bool {
        self.trigger.has_recovery()
            || self.body.has_recovery()
            || self.syntax.syntax().children().iter().any(|child| {
                matches!(
                    child.role(),
                    SyntaxRole::Recovery(_) | SyntaxRole::TrailingRecovery(_)
                )
            })
    }
}

impl AttachedContinueStatement {
    pub const fn syntax(&self) -> &AstNode<ContinueStatementKind> {
        &self.syntax
    }

    pub const fn label(&self) -> Option<&AttachedControlLabel> {
        self.label.as_ref()
    }

    /// Exact recovery node containing a forbidden trailing value.
    pub const fn forbidden_suffix(&self) -> Option<&AstNode<ErrorNodeKind>> {
        self.forbidden_suffix.as_ref()
    }
}

impl AstNode<OutStatementKind> {
    pub fn semantics(&self) -> Result<AttachedOutStatement, SyntaxAccessError> {
        let PendingKeywordStatementProjection::Out { label } = keyword_statement_projection(self)?
        else {
            return Err(invalid(self));
        };
        require_roles(self, &[SyntaxRole::Label(0), SyntaxRole::Initializer])?;
        Ok(AttachedOutStatement {
            syntax: self.clone(),
            label: attach_label(self, label)?,
            value: required_expression(self, SyntaxRole::Initializer)?,
        })
    }
}

impl AstNode<GotoStatementKind> {
    pub fn semantics(&self) -> Result<AttachedGotoStatement, SyntaxAccessError> {
        if keyword_statement_projection(self)? != PendingKeywordStatementProjection::Goto {
            return Err(invalid(self));
        }
        require_roles(self, &[SyntaxRole::Target])?;
        Ok(AttachedGotoStatement {
            syntax: self.clone(),
            target: required_expression(self, SyntaxRole::Target)?,
        })
    }
}

impl AstNode<DeferStatementKind> {
    pub fn semantics(&self) -> Result<AttachedDeferStatement, SyntaxAccessError> {
        if keyword_statement_projection(self)? != PendingKeywordStatementProjection::Defer {
            return Err(invalid(self));
        }
        require_roles(self, &[SyntaxRole::Initializer])?;
        Ok(AttachedDeferStatement {
            syntax: self.clone(),
            expression: required_expression(self, SyntaxRole::Initializer)?,
        })
    }
}

impl AstNode<DeferBlockStatementKind> {
    pub fn semantics(&self) -> Result<AttachedDeferBlockStatement, SyntaxAccessError> {
        require_roles(
            self,
            &[
                SyntaxRole::Body,
                SyntaxRole::Kind,
                SyntaxRole::Colon,
                SyntaxRole::Recovery(0),
            ],
        )?;
        let outcome_source = self.optional_exact_child::<NameReferenceKind>(SyntaxRole::Kind)?;
        let outcome = match outcome_source.as_ref().map(AstNode::source_text) {
            None => DeferOutcome::Always,
            Some("completed") => DeferOutcome::Completed,
            Some("cancelled") => DeferOutcome::Cancelled,
            Some("failed") => DeferOutcome::Failed,
            Some(_) => return Err(invalid(self)),
        };
        let bodies = self.syntax().children_with_role(SyntaxRole::Body);
        let [body] = bodies.as_slice() else {
            return Err(invalid(self));
        };
        let body = match body.kind() {
            crate::grammar::SyntaxKind::MissingBody => {
                AttachedDeferBlockBody::Missing(body.clone().cast::<MissingBodyKind>()?)
            }
            kind if kind.is_expression() => AttachedDeferBlockBody::Expression(
                AttachedExpressionNode::from_syntax(body.clone())?,
            ),
            _ => return Err(invalid(self)),
        };
        Ok(AttachedDeferBlockStatement {
            syntax: self.clone(),
            outcome,
            outcome_source,
            body,
        })
    }
}

impl AstNode<SignalStatementKind> {
    pub fn semantics(&self) -> Result<AttachedSignalStatement, SyntaxAccessError> {
        if keyword_statement_projection(self)? != PendingKeywordStatementProjection::Signal {
            return Err(invalid(self));
        }
        require_roles(
            self,
            &[
                SyntaxRole::Target,
                SyntaxRole::Initializer,
                SyntaxRole::Recovery(0),
            ],
        )?;
        Ok(AttachedSignalStatement {
            syntax: self.clone(),
            target: required_expression(self, SyntaxRole::Target)?,
            value: required_expression(self, SyntaxRole::Initializer)?,
            arrow_recovery: optional_recovery(self)?,
        })
    }
}

impl AstNode<BreakStatementKind> {
    pub fn semantics(&self) -> Result<AttachedBreakStatement, SyntaxAccessError> {
        let PendingKeywordStatementProjection::Break { label } =
            keyword_statement_projection(self)?
        else {
            return Err(invalid(self));
        };
        require_roles(self, &[SyntaxRole::Label(0), SyntaxRole::Initializer])?;
        Ok(AttachedBreakStatement {
            syntax: self.clone(),
            label: attach_label(self, label)?,
            value: optional_expression(self, SyntaxRole::Initializer)?,
        })
    }
}

impl AstNode<ContinueStatementKind> {
    pub fn semantics(&self) -> Result<AttachedContinueStatement, SyntaxAccessError> {
        let PendingKeywordStatementProjection::Continue { label } =
            keyword_statement_projection(self)?
        else {
            return Err(invalid(self));
        };
        require_roles(self, &[SyntaxRole::Label(0), SyntaxRole::Recovery(0)])?;
        Ok(AttachedContinueStatement {
            syntax: self.clone(),
            label: attach_label(self, label)?,
            forbidden_suffix: optional_recovery(self)?,
        })
    }
}

impl AstNode<OnStatementKind> {
    pub fn semantics(&self) -> Result<AttachedOnStatement, SyntaxAccessError> {
        let trigger = self
            .syntax()
            .optional_unique_child(SyntaxRole::Condition)?
            .ok_or(SyntaxAccessError::InvalidTriggerShape { id: self.id() })?;
        if self.syntax().children().iter().any(|child| {
            !matches!(
                child.role(),
                SyntaxRole::Condition
                    | SyntaxRole::Statement(0)
                    | SyntaxRole::Body
                    | SyntaxRole::Colon
                    | SyntaxRole::Recovery(_)
                    | SyntaxRole::TrailingRecovery(_)
            )
        }) {
            return Err(SyntaxAccessError::InvalidTriggerShape { id: self.id() });
        }
        let arrow = self.optional_family_child::<StatementFamily>(SyntaxRole::Statement(0))?;
        let colon = self.optional_exact_child::<ColonKind>(SyntaxRole::Colon)?;
        let bodies = self.syntax().children_with_role(SyntaxRole::Body);
        let body = match (arrow, colon, bodies.as_slice()) {
            (Some(statement), None, []) => AttachedOnStatementBody::Arrow(statement),
            (None, None, [body]) if body.kind() == SyntaxKind::Block => {
                AttachedOnStatementBody::Braced(body.clone().cast::<BlockKind>()?)
            }
            (None, Some(colon), [body]) if body.kind() == SyntaxKind::Block => {
                AttachedOnStatementBody::Indented {
                    colon,
                    block: body.clone().cast::<BlockKind>()?,
                }
            }
            (None, _, [body]) if body.kind() == SyntaxKind::MissingBody => {
                AttachedOnStatementBody::Missing(body.clone().cast::<MissingBodyKind>()?)
            }
            _ => return Err(SyntaxAccessError::InvalidTriggerShape { id: self.id() }),
        };
        Ok(AttachedOnStatement {
            syntax: self.clone(),
            trigger: attach_trigger_pattern(trigger)?,
            body,
        })
    }
}

pub(super) fn keyword_statement_projection<K: AstKind>(
    owner: &AstNode<K>,
) -> Result<PendingKeywordStatementProjection, SyntaxAccessError> {
    owner
        .syntax()
        .keyword_statement_projection()
        .cloned()
        .ok_or(SyntaxAccessError::MissingKeywordStatementProjection { id: owner.id() })
}

fn attach_label<K: AstKind>(
    owner: &AstNode<K>,
    value: Option<Result<SyntaxName, SyntaxNameIssue>>,
) -> Result<Option<AttachedControlLabel>, SyntaxAccessError> {
    let syntax = owner.optional_exact_child::<NameReferenceKind>(SyntaxRole::Label(0))?;
    match (syntax, value) {
        (None, None) => Ok(None),
        (Some(syntax), Some(value)) => Ok(Some(AttachedControlLabel { syntax, value })),
        _ => Err(invalid(owner)),
    }
}

fn required_expression<K: AstKind>(
    owner: &AstNode<K>,
    role: SyntaxRole,
) -> Result<RequiredStatementExpressionNode, SyntaxAccessError> {
    super::access::required_statement_expression(owner, role)
}

fn optional_expression<K: AstKind>(
    owner: &AstNode<K>,
    role: SyntaxRole,
) -> Result<Option<AttachedExpressionNode>, SyntaxAccessError> {
    owner
        .optional_family_child::<ExpressionFamily>(role)?
        .map(|expression| expression.semantic())
        .transpose()
}

pub(super) fn optional_recovery<K: AstKind>(
    owner: &AstNode<K>,
) -> Result<Option<AstNode<ErrorNodeKind>>, SyntaxAccessError> {
    let mut recovery = owner.ordered_exact_children::<ErrorNodeKind>(SyntaxRoleClass::Recovery)?;
    if recovery.len() > 1 {
        return Err(invalid(owner));
    }
    Ok(recovery.pop())
}

pub(super) fn require_roles<K: AstKind>(
    owner: &AstNode<K>,
    accepted: &[SyntaxRole],
) -> Result<(), SyntaxAccessError> {
    owner
        .syntax()
        .children()
        .iter()
        .all(|child| {
            accepted.contains(&child.role())
                || matches!(
                    child.role(),
                    SyntaxRole::OpenDelimiter | SyntaxRole::CloseDelimiter
                )
        })
        .then_some(())
        .ok_or_else(|| invalid(owner))
}

pub(super) fn invalid<K: AstKind>(owner: &AstNode<K>) -> SyntaxAccessError {
    SyntaxAccessError::InvalidKeywordStatementProjection { id: owner.id() }
}

#[cfg(test)]
#[path = "statement/tests.rs"]
mod tests;
