//! Typed attachment views for keyword-owned control statements.

use super::SyntaxAccessError;
use super::access::RequiredStatementExpressionNode;
use super::expression::AttachedExpressionNode;
use super::family::ExpressionFamily;
use super::node::{
    AstKind, AstNode, BreakStatementKind, ContinueStatementKind, DeferStatementKind, ErrorNodeKind,
    GotoStatementKind, NameReferenceKind, OutStatementKind, SignalStatementKind,
};
use crate::grammar::keyword_statement_projection::PendingKeywordStatementProjection;
use crate::grammar::{SyntaxRole, SyntaxRoleClass};
use crate::name::{SyntaxName, SyntaxNameIssue};

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
