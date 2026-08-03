//! Snapshot-bound typed Choice lifecycle-plan ownership.

use super::super::access::{RequiredStatementExpressionNode, required_statement_expression};
use super::super::family::PatternFamily;
use super::super::node::{
    AstKind, AstNode, ChoicePlanAssignmentKind, ChoicePlanBodyKind, ChoicePlanCancelKind,
    ChoicePlanKind, ChoicePlanOnSelectKind, ChoicePlanTimeoutKind, EqualsKind, ErrorNodeKind,
    MissingBodyKind, NameReferenceKind,
};
use super::super::thread_body::{
    AttachedRequiredNestedThreadFlowBody, required_nested_thread_flow_body,
};
use super::super::trigger::{AttachedTriggerPattern, attach_trigger_pattern};
use super::super::{SyntaxAccessError, SyntaxNodeHandle};
use super::{
    AttachedChoiceSuiteSource, choice_suite_source, invalid, pattern_has_recovery,
    required_expression_has_recovery,
};
use crate::grammar::{SyntaxKind, SyntaxRole, SyntaxRoleClass};
use crate::name::{SyntaxName, SyntaxNameIssue};

/// Complete typed `with { ... }` lifecycle plan attached to one Choice.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedChoicePlan {
    syntax: AstNode<ChoicePlanKind>,
    body: AttachedRequiredChoicePlanBody,
    header_recovery: Option<AstNode<ErrorNodeKind>>,
    trailing_recovery: Option<AstNode<ErrorNodeKind>>,
}

impl AttachedChoicePlan {
    pub const fn syntax(&self) -> &AstNode<ChoicePlanKind> {
        &self.syntax
    }

    pub const fn body(&self) -> &AttachedRequiredChoicePlanBody {
        &self.body
    }

    pub const fn header_recovery(&self) -> Option<&AstNode<ErrorNodeKind>> {
        self.header_recovery.as_ref()
    }

    pub const fn trailing_recovery(&self) -> Option<&AstNode<ErrorNodeKind>> {
        self.trailing_recovery.as_ref()
    }

    pub fn has_recovery(&self) -> bool {
        self.header_recovery.is_some()
            || self.trailing_recovery.is_some()
            || self.body.has_recovery()
    }
}

/// Present lifecycle-plan body or its exact missing-body insertion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedRequiredChoicePlanBody {
    Present(AttachedChoicePlanBody),
    Missing(AstNode<MissingBodyKind>),
}

impl AttachedRequiredChoicePlanBody {
    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Present(body) => body.has_recovery(),
            Self::Missing(_) => true,
        }
    }
}

/// Source-ordered lifecycle-plan body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedChoicePlanBody {
    syntax: AstNode<ChoicePlanBodyKind>,
    source: AttachedChoiceSuiteSource,
    items: Box<[AttachedChoicePlanItem]>,
    recovery: Box<[AstNode<ErrorNodeKind>]>,
}

impl AttachedChoicePlanBody {
    pub const fn syntax(&self) -> &AstNode<ChoicePlanBodyKind> {
        &self.syntax
    }

    pub const fn source(&self) -> &AttachedChoiceSuiteSource {
        &self.source
    }

    pub fn items(&self) -> &[AttachedChoicePlanItem] {
        &self.items
    }

    pub fn recovery(&self) -> &[AstNode<ErrorNodeKind>] {
        &self.recovery
    }

    pub fn has_recovery(&self) -> bool {
        self.source.has_recovery()
            || !self.recovery.is_empty()
            || self.items.iter().any(AttachedChoicePlanItem::has_recovery)
    }

    fn from_syntax(syntax: AstNode<ChoicePlanBodyKind>) -> Result<Self, SyntaxAccessError> {
        if syntax.syntax().children().iter().any(|child| {
            !matches!(
                child.role(),
                SyntaxRole::OpenDelimiter
                    | SyntaxRole::CloseDelimiter
                    | SyntaxRole::Colon
                    | SyntaxRole::ChoicePlanItem(_)
                    | SyntaxRole::Recovery(_)
            )
        }) {
            return Err(invalid(&syntax));
        }
        let source = choice_suite_source(&syntax)?;
        let items = syntax
            .syntax()
            .ordered_children(SyntaxRoleClass::ChoicePlanItem)?
            .into_iter()
            .map(AttachedChoicePlanItem::from_syntax)
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

/// Closed direct-child family of one Choice lifecycle plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedChoicePlanItem {
    Assignment(AttachedChoicePlanAssignment),
    Timeout(AttachedChoicePlanTimeout),
    Cancel(AttachedChoicePlanCancel),
    OnSelect(AttachedChoicePlanOnSelect),
    Recovered(AstNode<ErrorNodeKind>),
}

impl AttachedChoicePlanItem {
    fn from_syntax(syntax: SyntaxNodeHandle) -> Result<Self, SyntaxAccessError> {
        match syntax.kind() {
            SyntaxKind::ChoicePlanAssignment => {
                Ok(Self::Assignment(attach_assignment(syntax.cast()?)?))
            }
            SyntaxKind::ChoicePlanTimeout => Ok(Self::Timeout(attach_timeout(syntax.cast()?)?)),
            SyntaxKind::ChoicePlanCancel => Ok(Self::Cancel(attach_cancel(syntax.cast()?)?)),
            SyntaxKind::ChoicePlanOnSelect => Ok(Self::OnSelect(attach_on_select(syntax.cast()?)?)),
            SyntaxKind::ErrorNode => Ok(Self::Recovered(syntax.cast()?)),
            _ => Err(SyntaxAccessError::InvalidChoiceShape { id: syntax.id() }),
        }
    }

    pub fn syntax(&self) -> SyntaxNodeHandle {
        match self {
            Self::Assignment(value) => value.syntax().syntax(),
            Self::Timeout(value) => value.syntax().syntax(),
            Self::Cancel(value) => value.syntax().syntax(),
            Self::OnSelect(value) => value.syntax().syntax(),
            Self::Recovered(value) => value.syntax(),
        }
    }

    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Assignment(value) => value.has_recovery(),
            Self::Timeout(value) => value.has_recovery(),
            Self::Cancel(value) => value.has_recovery(),
            Self::OnSelect(value) => value.has_recovery(),
            Self::Recovered(_) => true,
        }
    }
}

/// One validated lifecycle-plan assignment key and its exact syntax owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedChoicePlanKey {
    syntax: AstNode<NameReferenceKind>,
    value: Result<SyntaxName, SyntaxNameIssue>,
}

impl AttachedChoicePlanKey {
    pub const fn syntax(&self) -> &AstNode<NameReferenceKind> {
        &self.syntax
    }

    pub fn value(&self) -> Result<&SyntaxName, &SyntaxNameIssue> {
        self.value.as_ref()
    }

    pub const fn has_recovery(&self) -> bool {
        self.value.is_err()
    }
}

/// Open-ended Choice presentation/configuration assignment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedChoicePlanAssignment {
    syntax: AstNode<ChoicePlanAssignmentKind>,
    key: AttachedChoicePlanKey,
    equals: AstNode<EqualsKind>,
    value: RequiredStatementExpressionNode,
    key_recovery: Option<AstNode<ErrorNodeKind>>,
}

impl AttachedChoicePlanAssignment {
    pub const fn syntax(&self) -> &AstNode<ChoicePlanAssignmentKind> {
        &self.syntax
    }

    pub const fn key(&self) -> &AttachedChoicePlanKey {
        &self.key
    }

    pub const fn equals(&self) -> &AstNode<EqualsKind> {
        &self.equals
    }

    pub const fn value(&self) -> &RequiredStatementExpressionNode {
        &self.value
    }

    pub const fn key_recovery(&self) -> Option<&AstNode<ErrorNodeKind>> {
        self.key_recovery.as_ref()
    }

    pub fn has_recovery(&self) -> bool {
        self.key.has_recovery()
            || self.equals.range().is_empty()
            || required_expression_has_recovery(&self.value)
            || self.key_recovery.is_some()
    }
}

/// `timeout duration { ... }` lifecycle branch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedChoicePlanTimeout {
    syntax: AstNode<ChoicePlanTimeoutKind>,
    duration: RequiredStatementExpressionNode,
    body: AttachedRequiredNestedThreadFlowBody,
    trailing_recovery: Option<AstNode<ErrorNodeKind>>,
}

impl AttachedChoicePlanTimeout {
    pub const fn syntax(&self) -> &AstNode<ChoicePlanTimeoutKind> {
        &self.syntax
    }

    pub const fn duration(&self) -> &RequiredStatementExpressionNode {
        &self.duration
    }

    pub const fn body(&self) -> &AttachedRequiredNestedThreadFlowBody {
        &self.body
    }

    pub const fn trailing_recovery(&self) -> Option<&AstNode<ErrorNodeKind>> {
        self.trailing_recovery.as_ref()
    }

    pub fn has_recovery(&self) -> bool {
        required_expression_has_recovery(&self.duration)
            || self.body.has_recovery()
            || self.trailing_recovery.is_some()
    }
}

/// `cancel on trigger { ... }` lifecycle branch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedChoicePlanCancel {
    syntax: AstNode<ChoicePlanCancelKind>,
    trigger: AttachedTriggerPattern,
    body: AttachedRequiredNestedThreadFlowBody,
    header_recovery: Option<AstNode<ErrorNodeKind>>,
    trailing_recovery: Option<AstNode<ErrorNodeKind>>,
}

impl AttachedChoicePlanCancel {
    pub const fn syntax(&self) -> &AstNode<ChoicePlanCancelKind> {
        &self.syntax
    }

    pub const fn trigger(&self) -> &AttachedTriggerPattern {
        &self.trigger
    }

    pub const fn body(&self) -> &AttachedRequiredNestedThreadFlowBody {
        &self.body
    }

    pub const fn header_recovery(&self) -> Option<&AstNode<ErrorNodeKind>> {
        self.header_recovery.as_ref()
    }

    pub const fn trailing_recovery(&self) -> Option<&AstNode<ErrorNodeKind>> {
        self.trailing_recovery.as_ref()
    }

    pub fn has_recovery(&self) -> bool {
        self.trigger.has_recovery()
            || self.body.has_recovery()
            || self.header_recovery.is_some()
            || self.trailing_recovery.is_some()
    }
}

/// `on select pattern { ... }` lifecycle branch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedChoicePlanOnSelect {
    syntax: AstNode<ChoicePlanOnSelectKind>,
    pattern: super::super::AttachedPatternNode,
    body: AttachedRequiredNestedThreadFlowBody,
    header_recovery: Option<AstNode<ErrorNodeKind>>,
    trailing_recovery: Option<AstNode<ErrorNodeKind>>,
}

impl AttachedChoicePlanOnSelect {
    pub const fn syntax(&self) -> &AstNode<ChoicePlanOnSelectKind> {
        &self.syntax
    }

    pub const fn pattern(&self) -> &super::super::AttachedPatternNode {
        &self.pattern
    }

    pub const fn body(&self) -> &AttachedRequiredNestedThreadFlowBody {
        &self.body
    }

    pub const fn header_recovery(&self) -> Option<&AstNode<ErrorNodeKind>> {
        self.header_recovery.as_ref()
    }

    pub const fn trailing_recovery(&self) -> Option<&AstNode<ErrorNodeKind>> {
        self.trailing_recovery.as_ref()
    }

    pub fn has_recovery(&self) -> bool {
        pattern_has_recovery(&self.pattern)
            || self.body.has_recovery()
            || self.header_recovery.is_some()
            || self.trailing_recovery.is_some()
    }
}

pub(super) fn attach_choice_plan(
    syntax: AstNode<ChoicePlanKind>,
) -> Result<AttachedChoicePlan, SyntaxAccessError> {
    if syntax.syntax().children().iter().any(|child| {
        !matches!(
            child.role(),
            SyntaxRole::Body | SyntaxRole::Recovery(0) | SyntaxRole::TrailingRecovery(0)
        )
    }) {
        return Err(invalid(&syntax));
    }
    Ok(AttachedChoicePlan {
        body: required_plan_body(&syntax)?,
        header_recovery: syntax.optional_exact_child(SyntaxRole::Recovery(0))?,
        trailing_recovery: syntax.optional_exact_child(SyntaxRole::TrailingRecovery(0))?,
        syntax,
    })
}

fn required_plan_body<K: AstKind>(
    owner: &AstNode<K>,
) -> Result<AttachedRequiredChoicePlanBody, SyntaxAccessError> {
    let body = owner
        .syntax()
        .optional_unique_child(SyntaxRole::Body)?
        .ok_or_else(|| invalid(owner))?;
    match body.kind() {
        SyntaxKind::ChoicePlanBody => Ok(AttachedRequiredChoicePlanBody::Present(
            AttachedChoicePlanBody::from_syntax(body.cast()?)?,
        )),
        SyntaxKind::MissingBody if body.range().is_empty() => {
            Ok(AttachedRequiredChoicePlanBody::Missing(body.cast()?))
        }
        _ => Err(invalid(owner)),
    }
}

fn attach_assignment(
    syntax: AstNode<ChoicePlanAssignmentKind>,
) -> Result<AttachedChoicePlanAssignment, SyntaxAccessError> {
    let key = syntax.required_exact_child::<NameReferenceKind>(SyntaxRole::Key)?;
    let value = SyntaxName::try_new(key.source_text());
    Ok(AttachedChoicePlanAssignment {
        equals: syntax.required_exact_child::<EqualsKind>(SyntaxRole::Equals)?,
        value: required_statement_expression(&syntax, SyntaxRole::Value)?,
        key_recovery: syntax.optional_exact_child(SyntaxRole::Recovery(0))?,
        key: AttachedChoicePlanKey { syntax: key, value },
        syntax,
    })
}

fn attach_timeout(
    syntax: AstNode<ChoicePlanTimeoutKind>,
) -> Result<AttachedChoicePlanTimeout, SyntaxAccessError> {
    Ok(AttachedChoicePlanTimeout {
        duration: required_statement_expression(&syntax, SyntaxRole::Operand)?,
        body: required_nested_thread_flow_body(&syntax)?,
        trailing_recovery: syntax.optional_exact_child(SyntaxRole::TrailingRecovery(0))?,
        syntax,
    })
}

fn attach_cancel(
    syntax: AstNode<ChoicePlanCancelKind>,
) -> Result<AttachedChoicePlanCancel, SyntaxAccessError> {
    Ok(AttachedChoicePlanCancel {
        trigger: attach_trigger_pattern(
            syntax
                .syntax()
                .optional_unique_child(SyntaxRole::Condition)?
                .ok_or_else(|| invalid(&syntax))?,
        )?,
        body: required_nested_thread_flow_body(&syntax)?,
        header_recovery: syntax.optional_exact_child(SyntaxRole::Recovery(0))?,
        trailing_recovery: syntax.optional_exact_child(SyntaxRole::TrailingRecovery(0))?,
        syntax,
    })
}

fn attach_on_select(
    syntax: AstNode<ChoicePlanOnSelectKind>,
) -> Result<AttachedChoicePlanOnSelect, SyntaxAccessError> {
    Ok(AttachedChoicePlanOnSelect {
        pattern: syntax
            .required_family_child::<PatternFamily>(SyntaxRole::Pattern)?
            .semantic()?,
        body: required_nested_thread_flow_body(&syntax)?,
        header_recovery: syntax.optional_exact_child(SyntaxRole::Recovery(0))?,
        trailing_recovery: syntax.optional_exact_child(SyntaxRole::TrailingRecovery(0))?,
        syntax,
    })
}
