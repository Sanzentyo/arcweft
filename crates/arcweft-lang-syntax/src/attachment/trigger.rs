//! Shared snapshot-bound trigger-pattern ownership.

use super::access::{RequiredStatementExpressionNode, required_statement_expression};
use super::expression::AttachedExpressionNode;
use super::family::PatternFamily;
use super::node::{
    AstKind, AstNode, CloseParenKind, ErrorNodeKind, EventTriggerPatternKind,
    InputTriggerPatternKind, MarkTriggerPatternKind, OpenParenKind, ScopeTriggerPatternKind,
    SelectTriggerPatternKind, SignalTriggerPatternKind, TaskTriggerPatternKind,
    TimeoutTriggerPatternKind,
};
use super::source_file::AttachedDelimiterState;
use super::{AttachedPatternNode, SyntaxAccessError, SyntaxNodeHandle};
use crate::expressions::{SyntaxDialogueMarkName, SyntaxDialogueMarkNameIssue};
use crate::grammar::{SyntaxKind, SyntaxRole, SyntaxRoleClass};
use crate::id_ref::{SyntaxIdRefComponent, SyntaxIdRefPart};
use crate::patterns::{PatternComponentRole, PatternSyntaxKind};

/// Exact delimiters and typed recovery retained by one trigger call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedTriggerDelimiters {
    open: AstNode<OpenParenKind>,
    close: AstNode<CloseParenKind>,
    recovery: Box<[AstNode<ErrorNodeKind>]>,
    trailing_recovery: Option<AstNode<ErrorNodeKind>>,
}

impl AttachedTriggerDelimiters {
    pub const fn open(&self) -> &AstNode<OpenParenKind> {
        &self.open
    }

    pub const fn close(&self) -> &AstNode<CloseParenKind> {
        &self.close
    }

    pub fn close_state(&self) -> AttachedDelimiterState {
        let source = self.close.source_span();
        if self.close.range().is_empty() {
            AttachedDelimiterState::Missing(source)
        } else {
            AttachedDelimiterState::Authored(source)
        }
    }

    pub fn recovery(&self) -> &[AstNode<ErrorNodeKind>] {
        &self.recovery
    }

    pub const fn trailing_recovery(&self) -> Option<&AstNode<ErrorNodeKind>> {
        self.trailing_recovery.as_ref()
    }

    pub fn has_recovery(&self) -> bool {
        matches!(self.close_state(), AttachedDelimiterState::Missing(_))
            || !self.recovery.is_empty()
            || self.trailing_recovery.is_some()
    }
}

/// Shared payload for Input/Event/Mark/Select/Task/Scope trigger calls.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedPatternTrigger {
    syntax: SyntaxNodeHandle,
    delimiters: AttachedTriggerDelimiters,
    pattern: AttachedPatternNode,
}

/// Typed marker trigger payload. Marker triggers own a selector, never a
/// pattern/local child that a later layer would need to reinterpret.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedMarkTrigger {
    syntax: SyntaxNodeHandle,
    delimiters: AttachedTriggerDelimiters,
    selector: SyntaxDialogueMarkName,
}

impl AttachedMarkTrigger {
    pub fn syntax(&self) -> SyntaxNodeHandle {
        self.syntax.clone()
    }

    pub const fn delimiters(&self) -> &AttachedTriggerDelimiters {
        &self.delimiters
    }

    pub const fn selector(&self) -> &SyntaxDialogueMarkName {
        &self.selector
    }

    pub fn has_recovery(&self) -> bool {
        self.delimiters.has_recovery() || self.selector.has_recovery()
    }
}

impl AttachedPatternTrigger {
    pub fn syntax(&self) -> SyntaxNodeHandle {
        self.syntax.clone()
    }

    pub const fn delimiters(&self) -> &AttachedTriggerDelimiters {
        &self.delimiters
    }

    pub const fn pattern(&self) -> &AttachedPatternNode {
        &self.pattern
    }

    pub fn has_recovery(&self) -> bool {
        self.delimiters.has_recovery() || pattern_has_recovery(&self.pattern)
    }
}

/// Shared payload for a Timeout trigger call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedExpressionTrigger {
    syntax: SyntaxNodeHandle,
    delimiters: AttachedTriggerDelimiters,
    expression: RequiredStatementExpressionNode,
}

impl AttachedExpressionTrigger {
    pub fn syntax(&self) -> SyntaxNodeHandle {
        self.syntax.clone()
    }

    pub const fn delimiters(&self) -> &AttachedTriggerDelimiters {
        &self.delimiters
    }

    pub const fn expression(&self) -> &RequiredStatementExpressionNode {
        &self.expression
    }

    pub fn has_recovery(&self) -> bool {
        self.delimiters.has_recovery() || required_expression_has_recovery(&self.expression)
    }
}

/// Target and optional value pattern of a Signal trigger call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedSignalTrigger {
    syntax: SyntaxNodeHandle,
    delimiters: AttachedTriggerDelimiters,
    target: RequiredStatementExpressionNode,
    value: Option<AttachedPatternNode>,
}

impl AttachedSignalTrigger {
    pub fn syntax(&self) -> SyntaxNodeHandle {
        self.syntax.clone()
    }

    pub const fn delimiters(&self) -> &AttachedTriggerDelimiters {
        &self.delimiters
    }

    pub const fn target(&self) -> &RequiredStatementExpressionNode {
        &self.target
    }

    pub const fn value(&self) -> Option<&AttachedPatternNode> {
        self.value.as_ref()
    }

    pub fn has_recovery(&self) -> bool {
        self.delimiters.has_recovery()
            || required_expression_has_recovery(&self.target)
            || self.value.as_ref().is_some_and(pattern_has_recovery)
    }
}

/// Closed typed trigger family shared by cancellation-capable syntax.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedTriggerPattern {
    Input(AttachedPatternTrigger),
    Event(AttachedPatternTrigger),
    Signal(AttachedSignalTrigger),
    Timeout(AttachedExpressionTrigger),
    Mark(AttachedMarkTrigger),
    Select(AttachedPatternTrigger),
    Task(AttachedPatternTrigger),
    Scope(AttachedPatternTrigger),
    Expr(Box<AttachedExpressionNode>),
}

impl AttachedTriggerPattern {
    pub fn syntax(&self) -> SyntaxNodeHandle {
        match self {
            Self::Input(value)
            | Self::Event(value)
            | Self::Select(value)
            | Self::Task(value)
            | Self::Scope(value) => value.syntax(),
            Self::Mark(value) => value.syntax(),
            Self::Signal(value) => value.syntax(),
            Self::Timeout(value) => value.syntax(),
            Self::Expr(value) => value.syntax().syntax(),
        }
    }

    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Input(value)
            | Self::Event(value)
            | Self::Select(value)
            | Self::Task(value)
            | Self::Scope(value) => value.has_recovery(),
            Self::Mark(value) => value.has_recovery(),
            Self::Signal(value) => value.has_recovery(),
            Self::Timeout(value) => value.has_recovery(),
            Self::Expr(value) => syntax_has_recovery(&value.syntax().syntax()),
        }
    }
}

pub(super) fn attach_trigger_pattern(
    syntax: SyntaxNodeHandle,
) -> Result<AttachedTriggerPattern, SyntaxAccessError> {
    match syntax.kind() {
        SyntaxKind::InputTriggerPattern => Ok(AttachedTriggerPattern::Input(
            attach_pattern_trigger(&syntax.cast::<InputTriggerPatternKind>()?)?,
        )),
        SyntaxKind::EventTriggerPattern => Ok(AttachedTriggerPattern::Event(
            attach_pattern_trigger(&syntax.cast::<EventTriggerPatternKind>()?)?,
        )),
        SyntaxKind::SignalTriggerPattern => Ok(AttachedTriggerPattern::Signal(
            attach_signal_trigger(&syntax.cast()?)?,
        )),
        SyntaxKind::TimeoutTriggerPattern => Ok(AttachedTriggerPattern::Timeout(
            attach_expression_trigger(&syntax.cast()?)?,
        )),
        SyntaxKind::MarkTriggerPattern => Ok(AttachedTriggerPattern::Mark(attach_mark_trigger(
            &syntax.cast::<MarkTriggerPatternKind>()?,
        )?)),
        SyntaxKind::SelectTriggerPattern => Ok(AttachedTriggerPattern::Select(
            attach_pattern_trigger(&syntax.cast::<SelectTriggerPatternKind>()?)?,
        )),
        SyntaxKind::TaskTriggerPattern => Ok(AttachedTriggerPattern::Task(attach_pattern_trigger(
            &syntax.cast::<TaskTriggerPatternKind>()?,
        )?)),
        SyntaxKind::ScopeTriggerPattern => Ok(AttachedTriggerPattern::Scope(
            attach_pattern_trigger(&syntax.cast::<ScopeTriggerPatternKind>()?)?,
        )),
        kind if kind.is_expression() => Ok(AttachedTriggerPattern::Expr(Box::new(
            AttachedExpressionNode::from_syntax(syntax)?,
        ))),
        _ => Err(SyntaxAccessError::InvalidTriggerShape { id: syntax.id() }),
    }
}

fn attach_mark_trigger(
    syntax: &AstNode<MarkTriggerPatternKind>,
) -> Result<AttachedMarkTrigger, SyntaxAccessError> {
    validate_roles(
        syntax,
        &[
            SyntaxRole::OpenDelimiter,
            SyntaxRole::Pattern,
            SyntaxRole::CloseDelimiter,
        ],
    )?;
    let pattern = syntax
        .required_family_child::<PatternFamily>(SyntaxRole::Pattern)?
        .semantic()?;
    let delimiters = attach_delimiters(syntax)?;
    let selector = mark_selector(&pattern);
    let selector = if !selector.has_recovery()
        && (!delimiters.recovery().is_empty() || delimiters.trailing_recovery().is_some())
    {
        SyntaxDialogueMarkName::recovered(
            SyntaxDialogueMarkNameIssue::MultipleArguments,
            selector.range(),
        )
    } else {
        selector
    };
    Ok(AttachedMarkTrigger {
        selector,
        delimiters,
        syntax: syntax.syntax(),
    })
}

fn mark_selector(pattern: &AttachedPatternNode) -> SyntaxDialogueMarkName {
    let range = pattern
        .component(PatternComponentRole::EntityReference(
            SyntaxIdRefPart::Whole,
        ))
        .map_or_else(|| pattern.whole_source_span().range(), |span| span.range());
    let PatternSyntaxKind::EntityReference(reference) = pattern.value().kind() else {
        return SyntaxDialogueMarkName::recovered(
            SyntaxDialogueMarkNameIssue::MissingReference,
            range,
        );
    };
    if !pattern.value().state().is_valid() && reference.value().is_ok() {
        return SyntaxDialogueMarkName::recovered(SyntaxDialogueMarkNameIssue::Malformed, range);
    }
    let components = pattern
        .components()
        .into_iter()
        .filter_map(|component| match component.role() {
            PatternComponentRole::EntityReference(part) => Some(SyntaxIdRefComponent::new(
                part,
                component.source_span().range(),
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    SyntaxDialogueMarkName::from_reference(reference.clone(), range, components)
}

fn attach_pattern_trigger<K: AstKind>(
    syntax: &AstNode<K>,
) -> Result<AttachedPatternTrigger, SyntaxAccessError> {
    validate_roles(
        syntax,
        &[
            SyntaxRole::OpenDelimiter,
            SyntaxRole::Pattern,
            SyntaxRole::CloseDelimiter,
        ],
    )?;
    Ok(AttachedPatternTrigger {
        pattern: syntax
            .required_family_child::<PatternFamily>(SyntaxRole::Pattern)?
            .semantic()?,
        delimiters: attach_delimiters(syntax)?,
        syntax: syntax.syntax(),
    })
}

fn attach_expression_trigger(
    syntax: &AstNode<TimeoutTriggerPatternKind>,
) -> Result<AttachedExpressionTrigger, SyntaxAccessError> {
    validate_roles(
        syntax,
        &[
            SyntaxRole::OpenDelimiter,
            SyntaxRole::Operand,
            SyntaxRole::CloseDelimiter,
        ],
    )?;
    Ok(AttachedExpressionTrigger {
        expression: required_statement_expression(syntax, SyntaxRole::Operand)?,
        delimiters: attach_delimiters(syntax)?,
        syntax: syntax.syntax(),
    })
}

fn attach_signal_trigger(
    syntax: &AstNode<SignalTriggerPatternKind>,
) -> Result<AttachedSignalTrigger, SyntaxAccessError> {
    validate_roles(
        syntax,
        &[
            SyntaxRole::OpenDelimiter,
            SyntaxRole::Target,
            SyntaxRole::Pattern,
            SyntaxRole::CloseDelimiter,
        ],
    )?;
    let value = syntax
        .optional_family_child::<PatternFamily>(SyntaxRole::Pattern)?
        .map(|value| value.semantic())
        .transpose()?;
    Ok(AttachedSignalTrigger {
        target: required_statement_expression(syntax, SyntaxRole::Target)?,
        value,
        delimiters: attach_delimiters(syntax)?,
        syntax: syntax.syntax(),
    })
}

fn attach_delimiters<K: AstKind>(
    syntax: &AstNode<K>,
) -> Result<AttachedTriggerDelimiters, SyntaxAccessError> {
    let open = syntax.required_exact_child::<OpenParenKind>(SyntaxRole::OpenDelimiter)?;
    if open.range().is_empty() {
        return Err(SyntaxAccessError::InvalidTriggerShape { id: syntax.id() });
    }
    Ok(AttachedTriggerDelimiters {
        open,
        close: syntax.required_exact_child::<CloseParenKind>(SyntaxRole::CloseDelimiter)?,
        recovery: syntax
            .ordered_exact_children::<ErrorNodeKind>(SyntaxRoleClass::Recovery)?
            .into_boxed_slice(),
        trailing_recovery: syntax.optional_exact_child(SyntaxRole::TrailingRecovery(0))?,
    })
}

fn validate_roles<K: AstKind>(
    syntax: &AstNode<K>,
    admitted: &[SyntaxRole],
) -> Result<(), SyntaxAccessError> {
    if syntax.syntax().children().iter().any(|child| {
        !admitted.contains(&child.role())
            && child.role().class() != SyntaxRoleClass::Recovery
            && child.role() != SyntaxRole::TrailingRecovery(0)
    }) {
        return Err(SyntaxAccessError::InvalidTriggerShape { id: syntax.id() });
    }
    Ok(())
}

fn required_expression_has_recovery(value: &RequiredStatementExpressionNode) -> bool {
    match value {
        RequiredStatementExpressionNode::Expression(expression) => {
            syntax_has_recovery(&expression.syntax())
        }
        RequiredStatementExpressionNode::Missing(_) => true,
    }
}

fn pattern_has_recovery(pattern: &AttachedPatternNode) -> bool {
    matches!(
        pattern.state(),
        crate::patterns::PatternSyntaxState::Recovered(_)
    )
}

fn syntax_has_recovery(syntax: &SyntaxNodeHandle) -> bool {
    syntax.kind().is_missing_node()
        || syntax.kind().is_error_node()
        || syntax.children().iter().any(syntax_has_recovery)
}
