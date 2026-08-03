//! Snapshot-bound typed Choice syntax without detached or string-backed readers.

mod candidate;
mod plan;

pub use candidate::{
    AttachedChoiceCompactAction, AttachedChoiceCompactArm, AttachedChoiceOption,
    AttachedChoiceOptionBody, AttachedChoiceOptionField, AttachedChoiceOptionFor,
    AttachedChoiceSelect, AttachedChoiceView, AttachedChoiceViewBody, AttachedChoiceViewEntry,
    AttachedRequiredChoiceOptionBody, AttachedRequiredChoiceViewBody,
};

pub use plan::{
    AttachedChoicePlan, AttachedChoicePlanAssignment, AttachedChoicePlanBody,
    AttachedChoicePlanCancel, AttachedChoicePlanItem, AttachedChoicePlanKey,
    AttachedChoicePlanOnSelect, AttachedChoicePlanTimeout, AttachedRequiredChoicePlanBody,
};

use plan::attach_choice_plan;

use super::access::{RequiredStatementExpressionNode, required_statement_expression};
use super::expression::AttachedExpressionNode;
use super::family::{ExpressionFamily, FamilyNode, PatternFamily, StatementFamily, StatementNode};
use super::node::{
    AstKind, AstNode, ChoiceBodyKind, ChoiceCompactArmKind, ChoiceEnabledFieldKind,
    ChoiceExpressionKind, ChoiceForItemKind, ChoiceGotoActionKind, ChoiceHotkeyFieldKind,
    ChoiceIdFieldKind, ChoiceIfBranchKind, ChoiceIfItemKind, ChoiceLabelFieldKind,
    ChoiceMatchArmKind, ChoiceMatchItemKind, ChoiceOptionBodyKind, ChoiceOptionForKind,
    ChoiceOptionKind, ChoiceOrderFieldKind, ChoiceOutActionKind, ChoiceSelectFieldKind,
    ChoiceStatementKind, ChoiceValueFieldKind, ChoiceViewBodyKind, ChoiceViewFieldKind,
    ChoiceVisibleFieldKind, CloseBraceKind, ColonKind, ErrorNodeKind, ExactAstKind,
    LetChoiceStatementKind, MissingBodyKind, MissingExpressionKind, OpenBraceKind,
};
use super::source_file::AttachedDelimiterState;
use super::thread_body::required_nested_thread_flow_body;
use super::{SyntaxAccessError, SyntaxNodeHandle};
use crate::expressions::ExpressionProjection;
use crate::grammar::keyword_statement_projection::PendingKeywordStatementProjection;
use crate::grammar::{SyntaxKind, SyntaxRole, SyntaxRoleClass};
use crate::id_ref::SyntaxIdRefSyntax;

/// One static entity reference retained by a Choice identity or compact action.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedChoiceEntityReference {
    expression: AttachedExpressionNode,
}

impl AttachedChoiceEntityReference {
    pub const fn expression(&self) -> &AttachedExpressionNode {
        &self.expression
    }

    pub fn value(&self) -> &SyntaxIdRefSyntax {
        let ExpressionProjection::EntityReference(value) = self.expression.projection() else {
            unreachable!("validated Choice entity-reference projection changed kind")
        };
        value
    }

    pub fn has_recovery(&self) -> bool {
        self.value().value().is_err()
    }
}

/// Required static entity reference or its exact missing-expression insertion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedRequiredChoiceEntityReference {
    Reference(AttachedChoiceEntityReference),
    Missing(AstNode<MissingExpressionKind>),
}

impl AttachedRequiredChoiceEntityReference {
    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Reference(reference) => reference.has_recovery(),
            Self::Missing(_) => true,
        }
    }
}

/// Complete typed Choice statement relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedChoiceStatement {
    syntax: AstNode<ChoiceStatementKind>,
    expression: AttachedChoiceExpression,
}

impl AttachedChoiceStatement {
    pub const fn syntax(&self) -> &AstNode<ChoiceStatementKind> {
        &self.syntax
    }

    pub const fn expression(&self) -> &AttachedChoiceExpression {
        &self.expression
    }
}

/// Typed binding statement whose initializer is the shared Choice expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedLetChoiceStatement {
    syntax: AstNode<LetChoiceStatementKind>,
    pattern: super::AttachedPatternNode,
    expression: AttachedChoiceExpression,
}

impl AttachedLetChoiceStatement {
    pub const fn syntax(&self) -> &AstNode<LetChoiceStatementKind> {
        &self.syntax
    }

    pub const fn pattern(&self) -> &super::AttachedPatternNode {
        &self.pattern
    }

    pub const fn expression(&self) -> &AttachedChoiceExpression {
        &self.expression
    }

    pub fn has_recovery(&self) -> bool {
        pattern_has_recovery(&self.pattern) || self.expression.has_recovery()
    }
}

/// Complete typed Choice expression relation shared by direct and `let`
/// statement owners.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedChoiceExpression {
    syntax: AstNode<ChoiceExpressionKind>,
    id: Option<AttachedChoiceEntityReference>,
    body: AttachedRequiredChoiceBody,
    plan: Option<AttachedChoicePlan>,
    header_recovery: Option<AstNode<ErrorNodeKind>>,
}

impl AttachedChoiceExpression {
    pub const fn syntax(&self) -> &AstNode<ChoiceExpressionKind> {
        &self.syntax
    }

    /// Projects this specialized relation back to its sole generic expression
    /// owner without detaching, reparsing, or changing snapshot identity.
    ///
    /// Direct `choice` and `let choice` statement wrappers use this route so
    /// the central expression transaction reserves and finalizes the same
    /// source-backed `ExprId` as ordinary expression consumers.
    pub fn expression_node(&self) -> Result<AttachedExpressionNode, SyntaxAccessError> {
        AttachedExpressionNode::from_syntax(self.syntax.syntax())
    }

    pub const fn id(&self) -> Option<&AttachedChoiceEntityReference> {
        self.id.as_ref()
    }

    pub const fn body(&self) -> &AttachedRequiredChoiceBody {
        &self.body
    }

    pub const fn plan(&self) -> Option<&AttachedChoicePlan> {
        self.plan.as_ref()
    }

    pub const fn header_recovery(&self) -> Option<&AstNode<ErrorNodeKind>> {
        self.header_recovery.as_ref()
    }

    pub fn has_recovery(&self) -> bool {
        self.id
            .as_ref()
            .is_some_and(AttachedChoiceEntityReference::has_recovery)
            || self.header_recovery.is_some()
            || self.body.has_recovery()
            || self
                .plan
                .as_ref()
                .is_some_and(AttachedChoicePlan::has_recovery)
    }
}

/// Present Choice body or its exact missing-body insertion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedRequiredChoiceBody {
    Present(AttachedChoiceBody),
    Missing(AstNode<MissingBodyKind>),
}

impl AttachedRequiredChoiceBody {
    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Present(body) => body.has_recovery(),
            Self::Missing(_) => true,
        }
    }
}

/// Source-ordered Choice candidate body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedChoiceBody {
    syntax: AstNode<ChoiceBodyKind>,
    source: AttachedChoiceSuiteSource,
    items: Box<[AttachedChoiceItem]>,
    recovery: Box<[AstNode<ErrorNodeKind>]>,
}

/// Exact authored source form of a Choice-owned suite.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedChoiceSuiteSource {
    Braced {
        open: AstNode<OpenBraceKind>,
        close: AstNode<CloseBraceKind>,
    },
    Indented {
        colon: AstNode<ColonKind>,
    },
}

impl AttachedChoiceSuiteSource {
    pub fn has_recovery(&self) -> bool {
        matches!(
            self,
            Self::Braced { close, .. } if close.range().is_empty()
        )
    }
}

impl AttachedChoiceBody {
    pub const fn syntax(&self) -> &AstNode<ChoiceBodyKind> {
        &self.syntax
    }

    pub const fn source(&self) -> &AttachedChoiceSuiteSource {
        &self.source
    }

    pub fn items(&self) -> &[AttachedChoiceItem] {
        &self.items
    }

    pub fn recovery(&self) -> &[AstNode<ErrorNodeKind>] {
        &self.recovery
    }

    pub fn has_recovery(&self) -> bool {
        self.source.has_recovery()
            || !self.recovery.is_empty()
            || self.items.iter().any(AttachedChoiceItem::has_recovery)
    }

    fn from_syntax(syntax: AstNode<ChoiceBodyKind>) -> Result<Self, SyntaxAccessError> {
        if syntax.syntax().children().iter().any(|child| {
            !matches!(
                child.role(),
                SyntaxRole::OpenDelimiter
                    | SyntaxRole::CloseDelimiter
                    | SyntaxRole::Colon
                    | SyntaxRole::ChoiceItem(_)
                    | SyntaxRole::Recovery(_)
            )
        }) {
            return Err(invalid(&syntax));
        }
        let source = choice_suite_source(&syntax)?;
        let items = syntax
            .syntax()
            .ordered_children(SyntaxRoleClass::ChoiceItem)?
            .into_iter()
            .map(AttachedChoiceItem::from_syntax)
            .collect::<Result<Vec<_>, _>>()?
            .into_boxed_slice();
        let recovery = syntax
            .ordered_exact_children::<ErrorNodeKind>(SyntaxRoleClass::Recovery)?
            .into_boxed_slice();
        Ok(Self {
            syntax,
            source,
            items,
            recovery,
        })
    }
}

/// Closed direct child family of one Choice candidate body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedChoiceItem {
    Let(StatementNode),
    If(AttachedChoiceIf),
    For(AttachedChoiceFor),
    Match(AttachedChoiceMatch),
    Option(AttachedChoiceOption),
    OptionFor(AttachedChoiceOptionFor),
    CompactArm(AttachedChoiceCompactArm),
    Recovered(AstNode<ErrorNodeKind>),
}

impl AttachedChoiceItem {
    fn from_syntax(syntax: SyntaxNodeHandle) -> Result<Self, SyntaxAccessError> {
        match syntax.kind() {
            kind if is_choice_let_kind(kind) => {
                Ok(Self::Let(FamilyNode::<StatementFamily>::new(syntax)?))
            }
            SyntaxKind::ChoiceIfItem => Ok(Self::If(attach_choice_if(syntax.cast()?)?)),
            SyntaxKind::ChoiceForItem => Ok(Self::For(attach_choice_for(syntax.cast()?)?)),
            SyntaxKind::ChoiceMatchItem => Ok(Self::Match(attach_choice_match(syntax.cast()?)?)),
            SyntaxKind::ChoiceOption => Ok(Self::Option(attach_choice_option(syntax.cast()?)?)),
            SyntaxKind::ChoiceOptionFor => {
                Ok(Self::OptionFor(attach_choice_option_for(syntax.cast()?)?))
            }
            SyntaxKind::ChoiceCompactArm => {
                Ok(Self::CompactArm(attach_choice_compact_arm(syntax.cast()?)?))
            }
            SyntaxKind::ErrorNode => Ok(Self::Recovered(syntax.cast()?)),
            _ => Err(SyntaxAccessError::InvalidChoiceShape { id: syntax.id() }),
        }
    }

    pub fn syntax(&self) -> SyntaxNodeHandle {
        match self {
            Self::Let(statement) => statement.syntax(),
            Self::If(value) => value.syntax().syntax(),
            Self::For(value) => value.syntax().syntax(),
            Self::Match(value) => value.syntax().syntax(),
            Self::Option(value) => value.syntax().syntax(),
            Self::OptionFor(value) => value.syntax().syntax(),
            Self::CompactArm(value) => value.syntax().syntax(),
            Self::Recovered(value) => value.syntax(),
        }
    }

    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Let(statement) => syntax_has_recovery(&statement.syntax()),
            Self::If(value) => value.has_recovery(),
            Self::For(value) => value.has_recovery(),
            Self::Match(value) => value.has_recovery(),
            Self::Option(value) => value.has_recovery(),
            Self::OptionFor(value) => value.has_recovery(),
            Self::CompactArm(value) => value.has_recovery(),
            Self::Recovered(_) => true,
        }
    }
}

/// Typed candidate gate with isolated then/else Choice scopes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedChoiceIf {
    syntax: AstNode<ChoiceIfItemKind>,
    branches: Box<[AttachedChoiceIfBranch]>,
    else_body: Option<AttachedRequiredChoiceBody>,
}

/// One source-ordered branch in a flat, stack-safe Choice `if` chain.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedChoiceIfBranch {
    syntax: AstNode<ChoiceIfBranchKind>,
    condition: RequiredStatementExpressionNode,
    then_body: AttachedRequiredChoiceBody,
}

impl AttachedChoiceIfBranch {
    pub const fn syntax(&self) -> &AstNode<ChoiceIfBranchKind> {
        &self.syntax
    }

    pub const fn condition(&self) -> &RequiredStatementExpressionNode {
        &self.condition
    }

    pub const fn then_body(&self) -> &AttachedRequiredChoiceBody {
        &self.then_body
    }

    pub fn has_recovery(&self) -> bool {
        required_expression_has_recovery(&self.condition) || self.then_body.has_recovery()
    }
}

impl AttachedChoiceIf {
    pub const fn syntax(&self) -> &AstNode<ChoiceIfItemKind> {
        &self.syntax
    }

    pub fn branches(&self) -> &[AttachedChoiceIfBranch] {
        &self.branches
    }

    pub const fn else_body(&self) -> Option<&AttachedRequiredChoiceBody> {
        self.else_body.as_ref()
    }

    pub fn has_recovery(&self) -> bool {
        self.branches
            .iter()
            .any(AttachedChoiceIfBranch::has_recovery)
            || self
                .else_body
                .as_ref()
                .is_some_and(AttachedRequiredChoiceBody::has_recovery)
    }
}

/// Typed dynamic candidate loop.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedChoiceFor {
    syntax: AstNode<ChoiceForItemKind>,
    pattern: super::AttachedPatternNode,
    source: RequiredStatementExpressionNode,
    body: AttachedRequiredChoiceBody,
}

impl AttachedChoiceFor {
    pub const fn syntax(&self) -> &AstNode<ChoiceForItemKind> {
        &self.syntax
    }

    pub const fn pattern(&self) -> &super::AttachedPatternNode {
        &self.pattern
    }

    pub const fn source(&self) -> &RequiredStatementExpressionNode {
        &self.source
    }

    pub const fn body(&self) -> &AttachedRequiredChoiceBody {
        &self.body
    }

    pub fn has_recovery(&self) -> bool {
        pattern_has_recovery(&self.pattern)
            || required_expression_has_recovery(&self.source)
            || self.body.has_recovery()
    }
}

/// Typed Choice Match and ordered arms.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedChoiceMatch {
    syntax: AstNode<ChoiceMatchItemKind>,
    scrutinee: RequiredStatementExpressionNode,
    body: AttachedRequiredChoiceMatchBody,
}

impl AttachedChoiceMatch {
    pub const fn syntax(&self) -> &AstNode<ChoiceMatchItemKind> {
        &self.syntax
    }

    pub const fn scrutinee(&self) -> &RequiredStatementExpressionNode {
        &self.scrutinee
    }

    pub const fn body(&self) -> &AttachedRequiredChoiceMatchBody {
        &self.body
    }

    pub fn has_recovery(&self) -> bool {
        required_expression_has_recovery(&self.scrutinee) || self.body.has_recovery()
    }
}

/// Present Choice Match body or exact missing-body insertion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedRequiredChoiceMatchBody {
    Present(AttachedChoiceMatchBody),
    Missing(AstNode<MissingBodyKind>),
}

impl AttachedRequiredChoiceMatchBody {
    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Present(body) => body.has_recovery(),
            Self::Missing(_) => true,
        }
    }
}

/// Delimited source-ordered Choice Match arms.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedChoiceMatchBody {
    syntax: AstNode<ChoiceBodyKind>,
    open: AstNode<OpenBraceKind>,
    arms: Box<[AttachedChoiceMatchArm]>,
    close: AstNode<CloseBraceKind>,
}

impl AttachedChoiceMatchBody {
    pub const fn syntax(&self) -> &AstNode<ChoiceBodyKind> {
        &self.syntax
    }

    pub const fn open(&self) -> &AstNode<OpenBraceKind> {
        &self.open
    }

    pub fn arms(&self) -> &[AttachedChoiceMatchArm] {
        &self.arms
    }

    pub fn close_state(&self) -> AttachedDelimiterState {
        self.close.delimiter_state()
    }

    pub fn has_recovery(&self) -> bool {
        matches!(self.close_state(), AttachedDelimiterState::Missing(_))
            || self.arms.iter().any(AttachedChoiceMatchArm::has_recovery)
    }
}

/// One Choice Match arm and its isolated candidate body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedChoiceMatchArm {
    syntax: AstNode<ChoiceMatchArmKind>,
    pattern: super::AttachedPatternNode,
    guard: Option<FamilyNode<ExpressionFamily>>,
    body: AttachedChoiceMatchArmBody,
}

impl AttachedChoiceMatchArm {
    pub const fn syntax(&self) -> &AstNode<ChoiceMatchArmKind> {
        &self.syntax
    }

    pub const fn pattern(&self) -> &super::AttachedPatternNode {
        &self.pattern
    }

    pub const fn guard(&self) -> Option<&FamilyNode<ExpressionFamily>> {
        self.guard.as_ref()
    }

    pub const fn body(&self) -> &AttachedChoiceMatchArmBody {
        &self.body
    }

    pub fn has_recovery(&self) -> bool {
        pattern_has_recovery(&self.pattern)
            || self
                .guard
                .as_ref()
                .is_some_and(|guard| syntax_has_recovery(&guard.syntax()))
            || self.body.has_recovery()
    }
}

/// Block, single candidate item, or missing Choice Match arm body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedChoiceMatchArmBody {
    Block(AttachedChoiceBody),
    Single(Box<AttachedChoiceItem>),
    Missing(AstNode<MissingBodyKind>),
}

impl AttachedChoiceMatchArmBody {
    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Block(body) => body.has_recovery(),
            Self::Single(item) => item.has_recovery(),
            Self::Missing(_) => true,
        }
    }
}

impl AstNode<ChoiceStatementKind> {
    pub fn semantics(&self) -> Result<AttachedChoiceStatement, SyntaxAccessError> {
        if self.syntax().keyword_statement_projection()
            != Some(&PendingKeywordStatementProjection::Choice)
        {
            return Err(invalid(self));
        }
        if self.syntax().children().iter().any(|child| {
            child.role() != SyntaxRole::Initializer || child.kind() != SyntaxKind::ChoiceExpression
        }) {
            return Err(invalid(self));
        }
        let expression = self
            .required_exact_child::<ChoiceExpressionKind>(SyntaxRole::Initializer)?
            .semantics()?;
        Ok(AttachedChoiceStatement {
            syntax: self.clone(),
            expression,
        })
    }
}

impl AstNode<LetChoiceStatementKind> {
    pub fn semantics(&self) -> Result<AttachedLetChoiceStatement, SyntaxAccessError> {
        if self.syntax().children().len() != 2
            || self
                .syntax()
                .children()
                .iter()
                .any(|child| !matches!(child.role(), SyntaxRole::Pattern | SyntaxRole::Initializer))
        {
            return Err(invalid(self));
        }
        let pattern = self
            .required_family_child::<PatternFamily>(SyntaxRole::Pattern)?
            .semantic()?;
        let expression = self
            .required_exact_child::<ChoiceExpressionKind>(SyntaxRole::Initializer)?
            .semantics()?;
        Ok(AttachedLetChoiceStatement {
            syntax: self.clone(),
            pattern,
            expression,
        })
    }
}

impl AstNode<ChoiceExpressionKind> {
    pub fn semantics(&self) -> Result<AttachedChoiceExpression, SyntaxAccessError> {
        if self.syntax().expression_projection()
            != Some(&crate::expressions::PendingExpressionProjection::new(
                ExpressionProjection::Choice,
                Vec::new(),
            ))
            || self.syntax().children().iter().any(|child| {
                !matches!(
                    child.role(),
                    SyntaxRole::PublicId
                        | SyntaxRole::Body
                        | SyntaxRole::Plan
                        | SyntaxRole::Recovery(0)
                )
            })
        {
            return Err(invalid(self));
        }
        let id = self
            .syntax()
            .optional_unique_child(SyntaxRole::PublicId)?
            .map(attach_choice_entity_reference)
            .transpose()?;
        let header_recovery = self.optional_exact_child(SyntaxRole::Recovery(0))?;
        let plan = self
            .syntax()
            .optional_unique_child(SyntaxRole::Plan)?
            .map(|syntax| attach_choice_plan(syntax.cast()?))
            .transpose()?;
        Ok(AttachedChoiceExpression {
            syntax: self.clone(),
            id,
            body: required_choice_body(self, SyntaxRole::Body)?,
            plan,
            header_recovery,
        })
    }
}

fn attach_choice_if(
    syntax: AstNode<ChoiceIfItemKind>,
) -> Result<AttachedChoiceIf, SyntaxAccessError> {
    let branches = syntax
        .ordered_exact_children::<ChoiceIfBranchKind>(SyntaxRoleClass::Branch)?
        .into_iter()
        .map(|branch| {
            Ok(AttachedChoiceIfBranch {
                condition: required_statement_expression(&branch, SyntaxRole::Condition)?,
                then_body: required_choice_body(&branch, SyntaxRole::ThenBranch)?,
                syntax: branch,
            })
        })
        .collect::<Result<Vec<_>, SyntaxAccessError>>()?
        .into_boxed_slice();
    if branches.is_empty() {
        return Err(invalid(&syntax));
    }
    let else_body = syntax
        .syntax()
        .optional_unique_child(SyntaxRole::ElseBranch)?
        .map(|_| required_choice_body(&syntax, SyntaxRole::ElseBranch))
        .transpose()?;
    Ok(AttachedChoiceIf {
        syntax,
        branches,
        else_body,
    })
}

#[cfg(test)]
mod tests;

fn attach_choice_for(
    syntax: AstNode<ChoiceForItemKind>,
) -> Result<AttachedChoiceFor, SyntaxAccessError> {
    Ok(AttachedChoiceFor {
        pattern: syntax
            .required_family_child::<PatternFamily>(SyntaxRole::Pattern)?
            .semantic()?,
        source: required_statement_expression(&syntax, SyntaxRole::Initializer)?,
        body: required_choice_body(&syntax, SyntaxRole::Body)?,
        syntax,
    })
}

fn attach_choice_match(
    syntax: AstNode<ChoiceMatchItemKind>,
) -> Result<AttachedChoiceMatch, SyntaxAccessError> {
    let body = syntax
        .syntax()
        .optional_unique_child(SyntaxRole::Body)?
        .ok_or_else(|| invalid(&syntax))?;
    let body = match body.kind() {
        SyntaxKind::ChoiceBody => {
            AttachedRequiredChoiceMatchBody::Present(attach_choice_match_body(body.cast()?)?)
        }
        SyntaxKind::MissingBody if body.range().is_empty() => {
            AttachedRequiredChoiceMatchBody::Missing(body.cast()?)
        }
        _ => return Err(invalid(&syntax)),
    };
    Ok(AttachedChoiceMatch {
        scrutinee: required_statement_expression(&syntax, SyntaxRole::Scrutinee)?,
        syntax,
        body,
    })
}

fn attach_choice_match_body(
    syntax: AstNode<ChoiceBodyKind>,
) -> Result<AttachedChoiceMatchBody, SyntaxAccessError> {
    if syntax.syntax().children().iter().any(|child| {
        !matches!(
            child.role(),
            SyntaxRole::OpenDelimiter | SyntaxRole::CloseDelimiter | SyntaxRole::MatchArm(_)
        )
    }) {
        return Err(invalid(&syntax));
    }
    let open = syntax.required_exact_child::<OpenBraceKind>(SyntaxRole::OpenDelimiter)?;
    let arms = syntax
        .ordered_exact_children::<ChoiceMatchArmKind>(SyntaxRoleClass::MatchArm)?
        .into_iter()
        .map(attach_choice_match_arm)
        .collect::<Result<Vec<_>, _>>()?
        .into_boxed_slice();
    let close = syntax.required_exact_child::<CloseBraceKind>(SyntaxRole::CloseDelimiter)?;
    Ok(AttachedChoiceMatchBody {
        syntax,
        open,
        arms,
        close,
    })
}

fn attach_choice_match_arm(
    syntax: AstNode<ChoiceMatchArmKind>,
) -> Result<AttachedChoiceMatchArm, SyntaxAccessError> {
    let pattern = syntax
        .required_family_child::<PatternFamily>(SyntaxRole::Pattern)?
        .semantic()?;
    let guard = syntax.optional_family_child::<ExpressionFamily>(SyntaxRole::Guard)?;
    let body_node = syntax.syntax().optional_unique_child(SyntaxRole::Body)?;
    let single = syntax
        .syntax()
        .ordered_children(SyntaxRoleClass::ChoiceItem)?;
    let body = match (body_node, single.as_slice()) {
        (Some(body), []) if body.kind() == SyntaxKind::ChoiceBody => {
            AttachedChoiceMatchArmBody::Block(AttachedChoiceBody::from_syntax(body.cast()?)?)
        }
        (Some(body), []) if body.kind() == SyntaxKind::MissingBody && body.range().is_empty() => {
            AttachedChoiceMatchArmBody::Missing(body.cast()?)
        }
        (None, [item]) => AttachedChoiceMatchArmBody::Single(Box::new(
            AttachedChoiceItem::from_syntax(item.clone())?,
        )),
        _ => return Err(invalid(&syntax)),
    };
    Ok(AttachedChoiceMatchArm {
        syntax,
        pattern,
        guard,
        body,
    })
}

fn attach_choice_option(
    syntax: AstNode<ChoiceOptionKind>,
) -> Result<AttachedChoiceOption, SyntaxAccessError> {
    Ok(AttachedChoiceOption {
        id: required_statement_expression(&syntax, SyntaxRole::PublicId)?,
        body: required_choice_option_body(&syntax)?,
        syntax,
    })
}

fn attach_choice_option_for(
    syntax: AstNode<ChoiceOptionForKind>,
) -> Result<AttachedChoiceOptionFor, SyntaxAccessError> {
    Ok(AttachedChoiceOptionFor {
        pattern: syntax
            .required_family_child::<PatternFamily>(SyntaxRole::Pattern)?
            .semantic()?,
        source: required_statement_expression(&syntax, SyntaxRole::Initializer)?,
        body: required_choice_option_body(&syntax)?,
        syntax,
    })
}

fn required_choice_option_body<K: AstKind>(
    owner: &AstNode<K>,
) -> Result<AttachedRequiredChoiceOptionBody, SyntaxAccessError> {
    let body = owner
        .syntax()
        .optional_unique_child(SyntaxRole::Body)?
        .ok_or_else(|| invalid(owner))?;
    match body.kind() {
        SyntaxKind::ChoiceOptionBody => Ok(AttachedRequiredChoiceOptionBody::Present(
            attach_choice_option_body(body.cast()?)?,
        )),
        SyntaxKind::MissingBody if body.range().is_empty() => {
            Ok(AttachedRequiredChoiceOptionBody::Missing(body.cast()?))
        }
        _ => Err(invalid(owner)),
    }
}

fn attach_choice_option_body(
    syntax: AstNode<ChoiceOptionBodyKind>,
) -> Result<AttachedChoiceOptionBody, SyntaxAccessError> {
    if syntax.syntax().children().iter().any(|child| {
        !matches!(
            child.role(),
            SyntaxRole::OpenDelimiter
                | SyntaxRole::CloseDelimiter
                | SyntaxRole::Colon
                | SyntaxRole::ChoiceOptionField(_)
                | SyntaxRole::Recovery(_)
        )
    }) {
        return Err(invalid(&syntax));
    }
    let source = choice_suite_source(&syntax)?;
    let fields = syntax
        .syntax()
        .ordered_children(SyntaxRoleClass::ChoiceOptionField)?
        .into_iter()
        .map(attach_choice_option_field)
        .collect::<Result<Vec<_>, _>>()?
        .into_boxed_slice();
    let recovery = syntax
        .ordered_exact_children::<ErrorNodeKind>(SyntaxRoleClass::Recovery)?
        .into_boxed_slice();
    Ok(AttachedChoiceOptionBody {
        syntax,
        source,
        fields,
        recovery,
    })
}

fn attach_choice_option_field(
    syntax: SyntaxNodeHandle,
) -> Result<AttachedChoiceOptionField, SyntaxAccessError> {
    match syntax.kind() {
        SyntaxKind::ChoiceLabelField => {
            let field = syntax.cast::<ChoiceLabelFieldKind>()?;
            let text_key = field
                .syntax()
                .optional_unique_child(SyntaxRole::PublicId)?
                .map(attach_required_choice_entity_reference)
                .transpose()?;
            Ok(AttachedChoiceOptionField::Label {
                value: required_statement_expression(&field, SyntaxRole::Value)?,
                syntax: field,
                text_key,
            })
        }
        SyntaxKind::ChoiceIdField => {
            let (syntax, value) = attach_choice_value::<ChoiceIdFieldKind>(syntax)?;
            Ok(AttachedChoiceOptionField::Id { syntax, value })
        }
        SyntaxKind::ChoiceValueField => {
            let (syntax, value) = attach_choice_value::<ChoiceValueFieldKind>(syntax)?;
            Ok(AttachedChoiceOptionField::Value { syntax, value })
        }
        SyntaxKind::ChoiceVisibleField => {
            let (syntax, value) = attach_choice_value::<ChoiceVisibleFieldKind>(syntax)?;
            Ok(AttachedChoiceOptionField::Visible { syntax, value })
        }
        SyntaxKind::ChoiceEnabledField => {
            let (syntax, value) = attach_choice_value::<ChoiceEnabledFieldKind>(syntax)?;
            Ok(AttachedChoiceOptionField::Enabled { syntax, value })
        }
        SyntaxKind::ChoiceOrderField => {
            let (syntax, value) = attach_choice_value::<ChoiceOrderFieldKind>(syntax)?;
            Ok(AttachedChoiceOptionField::Order { syntax, value })
        }
        SyntaxKind::ChoiceHotkeyField => {
            let (syntax, value) = attach_choice_value::<ChoiceHotkeyFieldKind>(syntax)?;
            Ok(AttachedChoiceOptionField::Hotkey { syntax, value })
        }
        SyntaxKind::ChoiceViewField => Ok(AttachedChoiceOptionField::View(attach_choice_view(
            syntax.cast()?,
        )?)),
        SyntaxKind::ChoiceSelectField => Ok(AttachedChoiceOptionField::Select(
            attach_choice_select(syntax.cast()?)?,
        )),
        kind if is_choice_let_kind(kind) => {
            Ok(AttachedChoiceOptionField::Let(
                FamilyNode::<StatementFamily>::new(syntax)?,
            ))
        }
        SyntaxKind::ErrorNode => Ok(AttachedChoiceOptionField::Recovered(syntax.cast()?)),
        _ => Err(SyntaxAccessError::InvalidChoiceShape { id: syntax.id() }),
    }
}

fn attach_choice_value<K: ExactAstKind>(
    syntax: SyntaxNodeHandle,
) -> Result<(AstNode<K>, RequiredStatementExpressionNode), SyntaxAccessError> {
    let syntax = syntax.cast::<K>()?;
    let value = required_statement_expression(&syntax, SyntaxRole::Value)?;
    Ok((syntax, value))
}

fn attach_choice_view(
    syntax: AstNode<ChoiceViewFieldKind>,
) -> Result<AttachedChoiceView, SyntaxAccessError> {
    let body = syntax
        .syntax()
        .optional_unique_child(SyntaxRole::Body)?
        .ok_or_else(|| invalid(&syntax))?;
    let body = match body.kind() {
        SyntaxKind::ChoiceViewBody => {
            AttachedRequiredChoiceViewBody::Present(attach_choice_view_body(body.cast()?)?)
        }
        SyntaxKind::MissingBody if body.range().is_empty() => {
            AttachedRequiredChoiceViewBody::Missing(body.cast()?)
        }
        _ => return Err(invalid(&syntax)),
    };
    Ok(AttachedChoiceView { syntax, body })
}

fn attach_choice_view_body(
    syntax: AstNode<ChoiceViewBodyKind>,
) -> Result<AttachedChoiceViewBody, SyntaxAccessError> {
    let open = syntax.required_exact_child::<OpenBraceKind>(SyntaxRole::OpenDelimiter)?;
    let fields = syntax
        .ordered_exact_children::<ChoiceViewFieldKind>(SyntaxRoleClass::ChoiceViewField)?
        .into_iter()
        .map(|field| {
            Ok(AttachedChoiceViewEntry {
                key: required_statement_expression(&field, SyntaxRole::Key)?,
                value: required_statement_expression(&field, SyntaxRole::Value)?,
                syntax: field,
            })
        })
        .collect::<Result<Vec<_>, SyntaxAccessError>>()?
        .into_boxed_slice();
    let close = syntax.required_exact_child::<CloseBraceKind>(SyntaxRole::CloseDelimiter)?;
    Ok(AttachedChoiceViewBody {
        syntax,
        open,
        fields,
        close,
    })
}

fn attach_choice_select(
    syntax: AstNode<ChoiceSelectFieldKind>,
) -> Result<AttachedChoiceSelect, SyntaxAccessError> {
    Ok(AttachedChoiceSelect {
        body: required_nested_thread_flow_body(&syntax)?,
        syntax,
    })
}

fn attach_choice_compact_arm(
    syntax: AstNode<ChoiceCompactArmKind>,
) -> Result<AttachedChoiceCompactArm, SyntaxAccessError> {
    let id = syntax
        .syntax()
        .optional_unique_child(SyntaxRole::PublicId)?
        .ok_or_else(|| invalid(&syntax))?;
    let id = attach_choice_entity_reference(id)?;
    let label = required_statement_expression(&syntax, SyntaxRole::Label(0))?;
    let condition = optional_required_expression(&syntax, SyntaxRole::Condition)?;
    let action = syntax
        .syntax()
        .optional_unique_child(SyntaxRole::Plan)?
        .ok_or_else(|| invalid(&syntax))?;
    let action = match action.kind() {
        SyntaxKind::ChoiceGotoAction => {
            let action = action.cast::<ChoiceGotoActionKind>()?;
            AttachedChoiceCompactAction::Goto {
                target: required_choice_entity_reference(&action, SyntaxRole::Target)?,
                syntax: action,
            }
        }
        SyntaxKind::ChoiceOutAction => {
            let action = action.cast::<ChoiceOutActionKind>()?;
            AttachedChoiceCompactAction::Out {
                value: required_statement_expression(&action, SyntaxRole::Value)?,
                syntax: action,
            }
        }
        SyntaxKind::MissingExpression if action.range().is_empty() => {
            AttachedChoiceCompactAction::Missing(action.cast()?)
        }
        _ => return Err(invalid(&syntax)),
    };
    Ok(AttachedChoiceCompactArm {
        syntax,
        id,
        label,
        condition,
        action,
    })
}

fn required_choice_body<K: AstKind>(
    owner: &AstNode<K>,
    role: SyntaxRole,
) -> Result<AttachedRequiredChoiceBody, SyntaxAccessError> {
    let body = owner
        .syntax()
        .optional_unique_child(role)?
        .ok_or_else(|| invalid(owner))?;
    match body.kind() {
        SyntaxKind::ChoiceBody => Ok(AttachedRequiredChoiceBody::Present(
            AttachedChoiceBody::from_syntax(body.cast()?)?,
        )),
        SyntaxKind::MissingBody if body.range().is_empty() => {
            Ok(AttachedRequiredChoiceBody::Missing(body.cast()?))
        }
        _ => Err(invalid(owner)),
    }
}

fn required_choice_entity_reference<K: AstKind>(
    owner: &AstNode<K>,
    role: SyntaxRole,
) -> Result<AttachedRequiredChoiceEntityReference, SyntaxAccessError> {
    let syntax = owner
        .syntax()
        .optional_unique_child(role)?
        .ok_or_else(|| invalid(owner))?;
    attach_required_choice_entity_reference(syntax)
}

fn attach_required_choice_entity_reference(
    syntax: SyntaxNodeHandle,
) -> Result<AttachedRequiredChoiceEntityReference, SyntaxAccessError> {
    if syntax.kind() == SyntaxKind::MissingExpression && syntax.range().is_empty() {
        return Ok(AttachedRequiredChoiceEntityReference::Missing(
            syntax.cast()?,
        ));
    }
    Ok(AttachedRequiredChoiceEntityReference::Reference(
        attach_choice_entity_reference(syntax)?,
    ))
}

fn attach_choice_entity_reference(
    syntax: SyntaxNodeHandle,
) -> Result<AttachedChoiceEntityReference, SyntaxAccessError> {
    let expression = AttachedExpressionNode::from_syntax(syntax.clone())?;
    if !matches!(
        expression.projection(),
        ExpressionProjection::EntityReference(_)
    ) {
        return Err(SyntaxAccessError::InvalidChoiceShape { id: syntax.id() });
    }
    Ok(AttachedChoiceEntityReference { expression })
}

fn optional_required_expression<K: AstKind>(
    owner: &AstNode<K>,
    role: SyntaxRole,
) -> Result<Option<RequiredStatementExpressionNode>, SyntaxAccessError> {
    owner
        .syntax()
        .optional_unique_child(role)?
        .map(|_| required_statement_expression(owner, role))
        .transpose()
}

fn required_expression_has_recovery(value: &RequiredStatementExpressionNode) -> bool {
    match value {
        RequiredStatementExpressionNode::Expression(expression) => {
            syntax_has_recovery(&expression.syntax())
        }
        RequiredStatementExpressionNode::Missing(_) => true,
    }
}

fn pattern_has_recovery(pattern: &super::AttachedPatternNode) -> bool {
    matches!(
        pattern.state(),
        crate::patterns::PatternSyntaxState::Recovered(_)
    )
}

fn is_choice_let_kind(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::LetStatement
            | SyntaxKind::LetElseStatement
            | SyntaxKind::LetChoiceStatement
            | SyntaxKind::LetScopeStatement
            | SyntaxKind::LetLoopStatement
            | SyntaxKind::LetAwaitStatement
            | SyntaxKind::LetActionReceiveStatement
    )
}

fn syntax_has_recovery(syntax: &SyntaxNodeHandle) -> bool {
    syntax.kind().is_missing_node()
        || syntax.kind().is_error_node()
        || syntax.children().iter().any(syntax_has_recovery)
}

fn choice_suite_source<K: AstKind>(
    syntax: &AstNode<K>,
) -> Result<AttachedChoiceSuiteSource, SyntaxAccessError> {
    let open = syntax.optional_exact_child::<OpenBraceKind>(SyntaxRole::OpenDelimiter)?;
    let close = syntax.optional_exact_child::<CloseBraceKind>(SyntaxRole::CloseDelimiter)?;
    let colon = syntax.optional_exact_child::<ColonKind>(SyntaxRole::Colon)?;
    match (open, close, colon) {
        (Some(open), Some(close), None) if !open.range().is_empty() => {
            Ok(AttachedChoiceSuiteSource::Braced { open, close })
        }
        (None, None, Some(colon)) if !colon.range().is_empty() => {
            Ok(AttachedChoiceSuiteSource::Indented { colon })
        }
        _ => Err(invalid(syntax)),
    }
}

fn invalid<K: AstKind>(owner: &AstNode<K>) -> SyntaxAccessError {
    SyntaxAccessError::InvalidChoiceShape { id: owner.id() }
}
