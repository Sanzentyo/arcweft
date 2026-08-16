//! Typed source owners for dedicated Thread/Flow statement families.

use arcweft_id::{LocaleTag, LocaleTagError};

use super::access::{RequiredStatementExpressionNode, required_statement_expression};
use super::expression::AttachedExpressionNode;
use super::family::PatternFamily;
use super::node::{
    AstNode, BlockKind, CloseBraceKind, EqualsKind, ErrorNodeKind, ForInKind, ForStatementKind,
    IncludeStatementKind, LoopStatementKind, MissingExpressionKind, MissingNameKind,
    NameDefinitionKind, NameReferenceKind, OpenBraceKind, ScopeStatementKind, SelectBranchKind,
    SelectStatementKind, SourceLocaleStatementKind, WhileLetStatementKind, WhileStatementKind,
};
use super::source_file::AttachedDelimiterState;
use super::statement::{invalid, keyword_statement_projection, optional_recovery, require_roles};
use super::thread_body::{AttachedRequiredNestedThreadFlowBody, required_nested_thread_flow_body};
use super::{AttachedPatternNode, SyntaxAccessError};
use crate::expressions::ExpressionProjection;
use crate::grammar::keyword_statement_projection::{
    PendingKeywordStatementProjection, PendingSelectBranchProjection,
};
use crate::grammar::{SyntaxKind, SyntaxRole, SyntaxRoleClass};
use crate::id_ref::SyntaxIdRefSyntax;
use crate::name::{SyntaxName, SyntaxNameIssue};
use crate::patterns::PatternSyntaxState;

/// One source-backed entity reference selected by the Thread/Flow grammar.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedThreadEntityReference {
    expression: AttachedExpressionNode,
}

impl AttachedThreadEntityReference {
    pub const fn expression(&self) -> &AttachedExpressionNode {
        &self.expression
    }

    pub fn value(&self) -> &SyntaxIdRefSyntax {
        let ExpressionProjection::EntityReference(value) = self.expression.projection() else {
            unreachable!("checked Thread/Flow entity-reference owner changed projection")
        };
        value
    }

    pub fn has_recovery(&self) -> bool {
        self.value().value().is_err()
    }
}

/// Required Include target or its exact missing-expression insertion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedRequiredIncludeTarget {
    Reference(Box<AttachedThreadEntityReference>),
    Missing(AstNode<MissingExpressionKind>),
}

impl AttachedRequiredIncludeTarget {
    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Reference(reference) => reference.has_recovery(),
            Self::Missing(_) => true,
        }
    }
}

/// Authored canonical locale value or the exact missing-name insertion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedSourceLocaleValue {
    Authored {
        syntax: AstNode<NameReferenceKind>,
        value: Result<LocaleTag, LocaleTagError>,
    },
    Missing(AstNode<MissingNameKind>),
}

impl AttachedSourceLocaleValue {
    pub fn value(&self) -> Option<Result<&LocaleTag, &LocaleTagError>> {
        match self {
            Self::Authored { value, .. } => Some(value.as_ref()),
            Self::Missing(_) => None,
        }
    }

    pub const fn has_recovery(&self) -> bool {
        match self {
            Self::Authored { value, .. } => value.is_err(),
            Self::Missing(_) => true,
        }
    }
}

/// Complete typed `source locale LocaleTag { ... }` relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedSourceLocaleStatement {
    syntax: AstNode<SourceLocaleStatementKind>,
    locale: AttachedSourceLocaleValue,
    body: AttachedRequiredNestedThreadFlowBody,
}

impl AttachedSourceLocaleStatement {
    pub const fn syntax(&self) -> &AstNode<SourceLocaleStatementKind> {
        &self.syntax
    }

    pub const fn locale(&self) -> &AttachedSourceLocaleValue {
        &self.locale
    }

    pub const fn body(&self) -> &AttachedRequiredNestedThreadFlowBody {
        &self.body
    }

    pub fn has_recovery(&self) -> bool {
        self.locale.has_recovery() || self.body.has_recovery()
    }
}

/// Optional source name owned by one lexical Scope statement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedScopeName {
    syntax: AstNode<NameDefinitionKind>,
    value: Result<SyntaxName, SyntaxNameIssue>,
}

impl AttachedScopeName {
    pub const fn syntax(&self) -> &AstNode<NameDefinitionKind> {
        &self.syntax
    }

    pub fn value(&self) -> Result<&SyntaxName, &SyntaxNameIssue> {
        self.value.as_ref()
    }

    pub const fn has_recovery(&self) -> bool {
        self.value.is_err()
    }
}

/// Required Select binding name without a fabricated recovery spelling.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedSelectBindingName {
    Authored {
        syntax: AstNode<NameDefinitionKind>,
        value: Result<SyntaxName, SyntaxNameIssue>,
    },
    Missing(AstNode<MissingNameKind>),
}

impl AttachedSelectBindingName {
    pub const fn has_recovery(&self) -> bool {
        match self {
            Self::Authored { value, .. } => value.is_err(),
            Self::Missing(_) => true,
        }
    }
}

/// Complete typed named or anonymous lexical Scope relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedScopeStatement {
    syntax: AstNode<ScopeStatementKind>,
    name: Option<AttachedScopeName>,
    body: AttachedRequiredNestedThreadFlowBody,
    header_recovery: Option<AstNode<ErrorNodeKind>>,
}

impl AttachedScopeStatement {
    pub const fn syntax(&self) -> &AstNode<ScopeStatementKind> {
        &self.syntax
    }

    pub const fn name(&self) -> Option<&AttachedScopeName> {
        self.name.as_ref()
    }

    pub const fn body(&self) -> &AttachedRequiredNestedThreadFlowBody {
        &self.body
    }

    pub const fn header_recovery(&self) -> Option<&AstNode<ErrorNodeKind>> {
        self.header_recovery.as_ref()
    }

    pub fn has_recovery(&self) -> bool {
        self.name
            .as_ref()
            .is_some_and(AttachedScopeName::has_recovery)
            || self.header_recovery.is_some()
            || self.body.has_recovery()
    }
}

/// Complete typed `include EntityRef` relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedIncludeStatement {
    syntax: AstNode<IncludeStatementKind>,
    target: AttachedRequiredIncludeTarget,
    trailing_recovery: Option<AstNode<ErrorNodeKind>>,
}

impl AttachedIncludeStatement {
    pub const fn syntax(&self) -> &AstNode<IncludeStatementKind> {
        &self.syntax
    }

    pub const fn target(&self) -> &AttachedRequiredIncludeTarget {
        &self.target
    }

    pub const fn trailing_recovery(&self) -> Option<&AstNode<ErrorNodeKind>> {
        self.trailing_recovery.as_ref()
    }

    pub fn has_recovery(&self) -> bool {
        self.target.has_recovery() || self.trailing_recovery.is_some()
    }
}

/// Complete typed `loop { ... }` relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedLoopStatement {
    syntax: AstNode<LoopStatementKind>,
    body: AttachedRequiredNestedThreadFlowBody,
}

impl AttachedLoopStatement {
    pub const fn syntax(&self) -> &AstNode<LoopStatementKind> {
        &self.syntax
    }

    pub const fn body(&self) -> &AttachedRequiredNestedThreadFlowBody {
        &self.body
    }

    pub fn has_recovery(&self) -> bool {
        self.body.has_recovery()
    }
}

/// Complete typed `while CONDITION { ... }` relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedWhileStatement {
    syntax: AstNode<WhileStatementKind>,
    condition: RequiredStatementExpressionNode,
    body: AttachedRequiredNestedThreadFlowBody,
}

impl AttachedWhileStatement {
    pub const fn syntax(&self) -> &AstNode<WhileStatementKind> {
        &self.syntax
    }

    pub const fn condition(&self) -> &RequiredStatementExpressionNode {
        &self.condition
    }

    pub const fn body(&self) -> &AttachedRequiredNestedThreadFlowBody {
        &self.body
    }

    pub fn has_recovery(&self) -> bool {
        matches!(self.condition, RequiredStatementExpressionNode::Missing(_))
            || self.body.has_recovery()
    }
}

/// Complete typed `while let PATTERN = SCRUTINEE when GUARD { ... }` relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedWhileLetStatement {
    syntax: AstNode<WhileLetStatementKind>,
    pattern: AttachedPatternNode,
    scrutinee: RequiredStatementExpressionNode,
    guard: Option<RequiredStatementExpressionNode>,
    body: AttachedRequiredNestedThreadFlowBody,
}

impl AttachedWhileLetStatement {
    pub const fn syntax(&self) -> &AstNode<WhileLetStatementKind> {
        &self.syntax
    }

    pub const fn pattern(&self) -> &AttachedPatternNode {
        &self.pattern
    }

    pub const fn scrutinee(&self) -> &RequiredStatementExpressionNode {
        &self.scrutinee
    }

    pub const fn guard(&self) -> Option<&RequiredStatementExpressionNode> {
        self.guard.as_ref()
    }

    pub const fn body(&self) -> &AttachedRequiredNestedThreadFlowBody {
        &self.body
    }

    pub fn has_recovery(&self) -> bool {
        matches!(
            self.pattern.value().state(),
            PatternSyntaxState::Recovered(_)
        ) || matches!(self.scrutinee, RequiredStatementExpressionNode::Missing(_))
            || self
                .guard
                .as_ref()
                .is_some_and(|guard| matches!(guard, RequiredStatementExpressionNode::Missing(_)))
            || self.body.has_recovery()
    }
}

/// Complete typed `for PATTERN in SOURCE { ... }` relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedForStatement {
    syntax: AstNode<ForStatementKind>,
    pattern: AttachedPatternNode,
    in_keyword: AstNode<ForInKind>,
    source: RequiredStatementExpressionNode,
    body: AttachedRequiredNestedThreadFlowBody,
}

impl AttachedForStatement {
    pub const fn syntax(&self) -> &AstNode<ForStatementKind> {
        &self.syntax
    }

    pub const fn pattern(&self) -> &AttachedPatternNode {
        &self.pattern
    }

    pub const fn in_keyword(&self) -> &AstNode<ForInKind> {
        &self.in_keyword
    }

    pub const fn source(&self) -> &RequiredStatementExpressionNode {
        &self.source
    }

    pub const fn body(&self) -> &AttachedRequiredNestedThreadFlowBody {
        &self.body
    }

    pub fn has_recovery(&self) -> bool {
        matches!(
            self.pattern.value().state(),
            PatternSyntaxState::Recovered(_)
        ) || self.in_keyword.range().is_empty()
            || matches!(self.source, RequiredStatementExpressionNode::Missing(_))
            || self.body.has_recovery()
    }
}

/// Maintained unary operand or one source-ordered Select branch block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedSelectStatementForm {
    Operand(RequiredStatementExpressionNode),
    Branches(AttachedSelectBranchBlock),
}

/// Complete typed Select statement relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedSelectStatement {
    syntax: AstNode<SelectStatementKind>,
    form: AttachedSelectStatementForm,
}

impl AttachedSelectStatement {
    pub const fn syntax(&self) -> &AstNode<SelectStatementKind> {
        &self.syntax
    }

    pub const fn form(&self) -> &AttachedSelectStatementForm {
        &self.form
    }

    pub fn has_recovery(&self) -> bool {
        match &self.form {
            AttachedSelectStatementForm::Operand(RequiredStatementExpressionNode::Expression(
                _,
            )) => false,
            AttachedSelectStatementForm::Operand(RequiredStatementExpressionNode::Missing(_)) => {
                true
            }
            AttachedSelectStatementForm::Branches(branches) => branches.has_recovery(),
        }
    }
}

/// Source-ordered Select branch container with no ordinary value tail.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedSelectBranchBlock {
    syntax: AstNode<BlockKind>,
    open: AstNode<OpenBraceKind>,
    branches: Box<[AttachedSelectBranch]>,
    close: AstNode<CloseBraceKind>,
}

impl AttachedSelectBranchBlock {
    pub const fn syntax(&self) -> &AstNode<BlockKind> {
        &self.syntax
    }

    pub const fn open(&self) -> &AstNode<OpenBraceKind> {
        &self.open
    }

    pub fn branches(&self) -> &[AttachedSelectBranch] {
        &self.branches
    }

    pub const fn close(&self) -> &AstNode<CloseBraceKind> {
        &self.close
    }

    pub fn close_state(&self) -> AttachedDelimiterState {
        self.close.delimiter_state()
    }

    pub fn has_recovery(&self) -> bool {
        matches!(self.close_state(), AttachedDelimiterState::Missing(_))
            || self.branches.iter().any(AttachedSelectBranch::has_recovery)
    }
}

/// Exact typed Select branch head and its nested Thread/Flow body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedSelectBranch {
    Bind {
        syntax: AstNode<SelectBranchKind>,
        name: AttachedSelectBindingName,
        equals: AstNode<EqualsKind>,
        source: RequiredStatementExpressionNode,
        propagates_error: bool,
        body: AttachedRequiredNestedThreadFlowBody,
    },
    Frame {
        syntax: AstNode<SelectBranchKind>,
        pattern: AttachedPatternNode,
        body: AttachedRequiredNestedThreadFlowBody,
    },
    Event {
        syntax: AstNode<SelectBranchKind>,
        pattern: AttachedPatternNode,
        body: AttachedRequiredNestedThreadFlowBody,
    },
    Recovered {
        syntax: AstNode<SelectBranchKind>,
        recovery: AstNode<ErrorNodeKind>,
        body: AttachedRequiredNestedThreadFlowBody,
    },
}

impl AttachedSelectBranch {
    pub const fn body(&self) -> &AttachedRequiredNestedThreadFlowBody {
        match self {
            Self::Bind { body, .. }
            | Self::Frame { body, .. }
            | Self::Event { body, .. }
            | Self::Recovered { body, .. } => body,
        }
    }

    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Bind {
                name, source, body, ..
            } => {
                name.has_recovery()
                    || matches!(source, RequiredStatementExpressionNode::Missing(_))
                    || body.has_recovery()
            }
            Self::Frame { pattern, body, .. } | Self::Event { pattern, body, .. } => {
                matches!(pattern.value().state(), PatternSyntaxState::Recovered(_))
                    || body.has_recovery()
            }
            Self::Recovered { .. } => true,
        }
    }
}

impl AstNode<LoopStatementKind> {
    pub fn semantics(&self) -> Result<AttachedLoopStatement, SyntaxAccessError> {
        require_roles(self, &[SyntaxRole::Body])?;
        Ok(AttachedLoopStatement {
            syntax: self.clone(),
            body: required_nested_thread_flow_body(self)?,
        })
    }
}

impl AstNode<WhileStatementKind> {
    pub fn semantics(&self) -> Result<AttachedWhileStatement, SyntaxAccessError> {
        require_roles(self, &[SyntaxRole::Condition, SyntaxRole::Body])?;
        Ok(AttachedWhileStatement {
            syntax: self.clone(),
            condition: required_statement_expression(self, SyntaxRole::Condition)?,
            body: required_nested_thread_flow_body(self)?,
        })
    }
}

impl AstNode<WhileLetStatementKind> {
    pub fn semantics(&self) -> Result<AttachedWhileLetStatement, SyntaxAccessError> {
        require_roles(
            self,
            &[
                SyntaxRole::Pattern,
                SyntaxRole::Scrutinee,
                SyntaxRole::Guard,
                SyntaxRole::Body,
            ],
        )?;
        let guard = self
            .syntax()
            .optional_unique_child(SyntaxRole::Guard)?
            .map(|_| required_statement_expression(self, SyntaxRole::Guard))
            .transpose()?;
        Ok(AttachedWhileLetStatement {
            syntax: self.clone(),
            pattern: self
                .required_family_child::<PatternFamily>(SyntaxRole::Pattern)?
                .semantic()?,
            scrutinee: required_statement_expression(self, SyntaxRole::Scrutinee)?,
            guard,
            body: required_nested_thread_flow_body(self)?,
        })
    }
}

impl AstNode<ForStatementKind> {
    pub fn semantics(&self) -> Result<AttachedForStatement, SyntaxAccessError> {
        require_roles(
            self,
            &[
                SyntaxRole::Pattern,
                SyntaxRole::Token,
                SyntaxRole::Scrutinee,
                SyntaxRole::Body,
            ],
        )?;
        Ok(AttachedForStatement {
            syntax: self.clone(),
            pattern: self
                .required_family_child::<PatternFamily>(SyntaxRole::Pattern)?
                .semantic()?,
            in_keyword: self.required_exact_child::<ForInKind>(SyntaxRole::Token)?,
            source: required_statement_expression(self, SyntaxRole::Scrutinee)?,
            body: required_nested_thread_flow_body(self)?,
        })
    }
}

impl AstNode<SourceLocaleStatementKind> {
    pub fn semantics(&self) -> Result<AttachedSourceLocaleStatement, SyntaxAccessError> {
        let PendingKeywordStatementProjection::SourceLocale { locale } =
            keyword_statement_projection(self)?
        else {
            return Err(invalid(self));
        };
        require_roles(self, &[SyntaxRole::Value, SyntaxRole::Body])?;
        let locale_syntax = self
            .syntax()
            .optional_unique_child(SyntaxRole::Value)?
            .ok_or_else(|| invalid(self))?;
        let locale = match (locale_syntax.kind(), locale) {
            (SyntaxKind::NameReference, Some(value)) => AttachedSourceLocaleValue::Authored {
                syntax: locale_syntax.cast()?,
                value,
            },
            (SyntaxKind::MissingName, None) if locale_syntax.range().is_empty() => {
                AttachedSourceLocaleValue::Missing(locale_syntax.cast()?)
            }
            _ => return Err(invalid(self)),
        };
        Ok(AttachedSourceLocaleStatement {
            syntax: self.clone(),
            locale,
            body: required_nested_thread_flow_body(self)?,
        })
    }
}

impl AstNode<ScopeStatementKind> {
    pub fn semantics(&self) -> Result<AttachedScopeStatement, SyntaxAccessError> {
        let PendingKeywordStatementProjection::Scope { name } = keyword_statement_projection(self)?
        else {
            return Err(invalid(self));
        };
        require_roles(
            self,
            &[SyntaxRole::Name, SyntaxRole::Body, SyntaxRole::Recovery(0)],
        )?;
        let name_syntax = self.optional_exact_child::<NameDefinitionKind>(SyntaxRole::Name)?;
        let name = match (name_syntax, name) {
            (Some(syntax), Some(value)) => Some(AttachedScopeName { syntax, value }),
            (None, None) => None,
            _ => return Err(invalid(self)),
        };
        Ok(AttachedScopeStatement {
            syntax: self.clone(),
            name,
            body: required_nested_thread_flow_body(self)?,
            header_recovery: optional_recovery(self)?,
        })
    }
}

impl AstNode<IncludeStatementKind> {
    pub fn semantics(&self) -> Result<AttachedIncludeStatement, SyntaxAccessError> {
        if keyword_statement_projection(self)? != PendingKeywordStatementProjection::Include {
            return Err(invalid(self));
        }
        require_roles(self, &[SyntaxRole::Target, SyntaxRole::Recovery(0)])?;
        Ok(AttachedIncludeStatement {
            syntax: self.clone(),
            target: required_include_target(self)?,
            trailing_recovery: optional_recovery(self)?,
        })
    }
}

impl AstNode<SelectStatementKind> {
    pub fn semantics(&self) -> Result<AttachedSelectStatement, SyntaxAccessError> {
        let PendingKeywordStatementProjection::Select { form, branches } =
            keyword_statement_projection(self)?
        else {
            return Err(invalid(self));
        };
        let form = match form {
            crate::grammar::SyntaxSelectStatementForm::Operand if branches.is_empty() => {
                require_roles(self, &[SyntaxRole::Operand])?;
                AttachedSelectStatementForm::Operand(required_statement_expression(
                    self,
                    SyntaxRole::Operand,
                )?)
            }
            crate::grammar::SyntaxSelectStatementForm::BranchBlock => {
                require_roles(self, &[SyntaxRole::Body])?;
                AttachedSelectStatementForm::Branches(attach_select_branch_block(self, &branches)?)
            }
            crate::grammar::SyntaxSelectStatementForm::Operand => return Err(invalid(self)),
        };
        Ok(AttachedSelectStatement {
            syntax: self.clone(),
            form,
        })
    }
}

fn required_include_target(
    owner: &AstNode<IncludeStatementKind>,
) -> Result<AttachedRequiredIncludeTarget, SyntaxAccessError> {
    let target = owner
        .syntax()
        .optional_unique_child(SyntaxRole::Target)?
        .ok_or_else(|| invalid(owner))?;
    if target.kind() == SyntaxKind::MissingExpression {
        return Ok(AttachedRequiredIncludeTarget::Missing(target.cast()?));
    }
    let expression = AttachedExpressionNode::from_syntax(target)?;
    if !matches!(
        expression.projection(),
        ExpressionProjection::EntityReference(_)
    ) {
        return Err(invalid(owner));
    }
    Ok(AttachedRequiredIncludeTarget::Reference(Box::new(
        AttachedThreadEntityReference { expression },
    )))
}

fn attach_select_branch_block(
    owner: &AstNode<SelectStatementKind>,
    projections: &[PendingSelectBranchProjection],
) -> Result<AttachedSelectBranchBlock, SyntaxAccessError> {
    let syntax = owner.required_exact_child::<BlockKind>(SyntaxRole::Body)?;
    let open = syntax.required_exact_child::<OpenBraceKind>(SyntaxRole::OpenDelimiter)?;
    let close = syntax.required_exact_child::<CloseBraceKind>(SyntaxRole::CloseDelimiter)?;
    let branches = syntax.ordered_exact_children::<SelectBranchKind>(SyntaxRoleClass::Branch)?;
    if branches.len() != projections.len()
        || syntax.syntax().children().iter().any(|child| {
            !matches!(
                child.role(),
                SyntaxRole::OpenDelimiter | SyntaxRole::CloseDelimiter | SyntaxRole::Branch(_)
            )
        })
    {
        return Err(invalid(owner));
    }
    let branches = branches
        .into_iter()
        .zip(projections)
        .map(|(branch, projection)| attach_select_branch(branch, projection))
        .collect::<Result<Vec<_>, _>>()?
        .into_boxed_slice();
    Ok(AttachedSelectBranchBlock {
        syntax,
        open,
        branches,
        close,
    })
}

fn attach_select_branch(
    syntax: AstNode<SelectBranchKind>,
    projection: &PendingSelectBranchProjection,
) -> Result<AttachedSelectBranch, SyntaxAccessError> {
    let body = required_nested_thread_flow_body(&syntax)?;
    match projection {
        PendingSelectBranchProjection::Bind {
            name,
            propagates_error,
        } => {
            require_roles(
                &syntax,
                &[
                    SyntaxRole::Name,
                    SyntaxRole::Equals,
                    SyntaxRole::Initializer,
                    SyntaxRole::Body,
                ],
            )?;
            let name_syntax = syntax
                .syntax()
                .optional_unique_child(SyntaxRole::Name)?
                .ok_or_else(|| invalid(&syntax))?;
            let name = match (name_syntax.kind(), name) {
                (SyntaxKind::NameDefinition, value)
                    if !matches!(value, Err(SyntaxNameIssue::Missing)) =>
                {
                    AttachedSelectBindingName::Authored {
                        syntax: name_syntax.cast()?,
                        value: value.clone(),
                    }
                }
                (SyntaxKind::MissingName, Err(SyntaxNameIssue::Missing))
                    if name_syntax.range().is_empty() =>
                {
                    AttachedSelectBindingName::Missing(name_syntax.cast()?)
                }
                _ => return Err(invalid(&syntax)),
            };
            Ok(AttachedSelectBranch::Bind {
                name,
                equals: syntax.required_exact_child::<EqualsKind>(SyntaxRole::Equals)?,
                source: required_statement_expression(&syntax, SyntaxRole::Initializer)?,
                propagates_error: *propagates_error,
                syntax,
                body,
            })
        }
        PendingSelectBranchProjection::Frame | PendingSelectBranchProjection::Event => {
            require_roles(&syntax, &[SyntaxRole::Pattern, SyntaxRole::Body])?;
            let pattern = syntax
                .required_family_child::<PatternFamily>(SyntaxRole::Pattern)?
                .semantic()?;
            if matches!(projection, PendingSelectBranchProjection::Frame) {
                Ok(AttachedSelectBranch::Frame {
                    syntax,
                    pattern,
                    body,
                })
            } else {
                Ok(AttachedSelectBranch::Event {
                    syntax,
                    pattern,
                    body,
                })
            }
        }
        PendingSelectBranchProjection::Error => {
            require_roles(&syntax, &[SyntaxRole::Recovery(0), SyntaxRole::Body])?;
            Ok(AttachedSelectBranch::Recovered {
                recovery: syntax.required_exact_child::<ErrorNodeKind>(SyntaxRole::Recovery(0))?,
                syntax,
                body,
            })
        }
    }
}

#[cfg(test)]
#[path = "thread_statement/tests.rs"]
mod tests;
