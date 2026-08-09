//! Typed Source declaration ownership over the attached grammar tree.

mod policy;
#[cfg(test)]
mod tests;

pub use policy::{
    AttachedSourceBackpressurePolicy, AttachedSourceBoundedArgument, AttachedSourceOverflowPolicy,
    AttachedSourcePrivacyPolicy, AttachedSourceReplayPolicy,
};

use arcweft_source::SourceSpan;

use crate::grammar::kinds::{SyntaxKind, SyntaxRole, SyntaxRoleClass};
use crate::grammar::source_declaration_projection::{
    PendingSourceBackpressurePolicy, PendingSourceBodyProjection, PendingSourceBoundedArgument,
    PendingSourceChildState, PendingSourceHandlerBody, PendingSourceHandlerEvent, PendingSourceId,
    PendingSourceMemberProjection, PendingSourceName, PendingSourceNamedPolicy,
    PendingSourceOverflowPolicy, PendingSourcePunctuation, PendingSourceTypeState,
    SourceContractSyntaxKind, SourcePrivacySyntaxKind, SourceReplaySyntaxKind,
};
use crate::id_ref::{SyntaxIdRefIssue, SyntaxIdRefSyntax};
use crate::name::{SyntaxName, SyntaxNameIssue};

use super::family::{FamilyNode, StatementFamily, StatementNode};
use super::node::{
    AssignmentStatementKind, AstNode, BlockKind, CallArgumentKind, CallExpressionKind,
    CloseBraceKind, DeclarationPublicIdKind, EnsuresClauseKind, ExpressionStatementKind,
    MissingBodyKind, MissingExpressionKind, MissingNameKind, MissingTypeKind, NameDefinitionKind,
    NameReferenceKind, OnStatementKind, OpenBraceKind, RequiresClauseKind, SourceItemKind,
};
use super::{
    AttachedExpressionNode, AttachedItemPrefix, AttachedPatternNode, AttachedTypeRefNode,
    SyntaxAccessError, SyntaxNodeHandle, SyntaxNodeId, TypedItemNode,
};

/// Optional Source ID selected by the entity-reference lexer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedSourceId {
    Absent,
    Authored {
        syntax: AstNode<DeclarationPublicIdKind>,
        reference: SyntaxIdRefSyntax,
        canonical_source_family: bool,
        requires_name: bool,
    },
}

impl AttachedSourceId {
    pub const fn reference(&self) -> Option<&SyntaxIdRefSyntax> {
        match self {
            Self::Absent => None,
            Self::Authored { reference, .. } => Some(reference),
        }
    }

    pub const fn is_canonical_source_family(&self) -> bool {
        matches!(
            self,
            Self::Authored {
                canonical_source_family: true,
                ..
            }
        )
    }

    pub const fn requires_name(&self) -> bool {
        matches!(
            self,
            Self::Authored {
                requires_name: true,
                ..
            }
        )
    }

    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Absent => false,
            Self::Authored {
                reference,
                canonical_source_family,
                requires_name,
                ..
            } => {
                !canonical_source_family
                    || match reference.value() {
                        Ok(_) => false,
                        Err(SyntaxIdRefIssue::MissingSuffix) if *requires_name => false,
                        Err(_) => true,
                    }
            }
        }
    }
}

/// Optional, authored, or required-missing Source local name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedSourceName {
    Absent,
    Authored {
        syntax: AstNode<NameDefinitionKind>,
        value: Result<SyntaxName, SyntaxNameIssue>,
    },
    Missing(AstNode<MissingNameKind>),
}

impl AttachedSourceName {
    pub const fn value(&self) -> Option<&Result<SyntaxName, SyntaxNameIssue>> {
        match self {
            Self::Authored { value, .. } => Some(value),
            Self::Absent | Self::Missing(_) => None,
        }
    }

    pub const fn has_recovery(&self) -> bool {
        matches!(
            self,
            Self::Missing(_) | Self::Authored { value: Err(_), .. }
        )
    }
}

/// Required Source type or its exact missing node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedSourceType {
    Authored(AttachedTypeRefNode),
    Missing {
        syntax: AstNode<MissingTypeKind>,
        node: AttachedTypeRefNode,
    },
}

impl AttachedSourceType {
    pub const fn node(&self) -> &AttachedTypeRefNode {
        match self {
            Self::Authored(node) | Self::Missing { node, .. } => node,
        }
    }

    pub const fn authored(&self) -> Option<&AttachedTypeRefNode> {
        match self {
            Self::Authored(value) => Some(value),
            Self::Missing { .. } => None,
        }
    }

    pub const fn has_recovery(&self) -> bool {
        matches!(self, Self::Missing { .. })
    }
}

/// Required punctuation represented by an authored span or exact insertion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedSourcePunctuation {
    source: SourceSpan,
    missing: bool,
}

impl AttachedSourcePunctuation {
    pub const fn source_span(&self) -> &SourceSpan {
        &self.source
    }

    pub const fn is_missing(&self) -> bool {
        self.missing
    }
}

/// Typed expression and its parser-selected recovery state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedSourceExpression {
    Authored(AttachedExpressionNode),
    Recovered(AttachedExpressionNode),
    Missing(AstNode<MissingExpressionKind>),
}

impl AttachedSourceExpression {
    pub const fn expression(&self) -> Option<&AttachedExpressionNode> {
        match self {
            Self::Authored(value) | Self::Recovered(value) => Some(value),
            Self::Missing(_) => None,
        }
    }

    pub fn syntax(&self) -> SyntaxNodeHandle {
        match self {
            Self::Authored(value) | Self::Recovered(value) => value.syntax().syntax(),
            Self::Missing(value) => value.syntax(),
        }
    }

    pub const fn has_recovery(&self) -> bool {
        !matches!(self, Self::Authored(_))
    }
}

/// Typed handler Pattern and its parser-selected recovery state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedSourcePattern {
    Authored(AttachedPatternNode),
    Recovered(AttachedPatternNode),
    Missing(AttachedPatternNode),
}

impl AttachedSourcePattern {
    pub const fn pattern(&self) -> &AttachedPatternNode {
        match self {
            Self::Authored(value) | Self::Recovered(value) | Self::Missing(value) => value,
        }
    }

    pub const fn has_recovery(&self) -> bool {
        !matches!(self, Self::Authored(_))
    }
}

/// Closed handler event with its exact typed Pattern or expression child.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedSourceHandlerEvent {
    Item(AttachedSourcePattern),
    Error(AttachedSourcePattern),
    Progress(AttachedSourcePattern),
    Disconnected(AttachedSourceExpression),
    PermissionRevoked(AttachedSourceExpression),
    End(AttachedSourceExpression),
    Unknown {
        value: Option<SyntaxName>,
        condition: AttachedSourceExpression,
    },
}

impl AttachedSourceHandlerEvent {
    pub const fn child_state_has_recovery(&self) -> bool {
        match self {
            Self::Item(pattern) | Self::Error(pattern) | Self::Progress(pattern) => {
                pattern.has_recovery()
            }
            Self::Disconnected(condition)
            | Self::PermissionRevoked(condition)
            | Self::End(condition)
            | Self::Unknown { condition, .. } => condition.has_recovery(),
        }
    }

    pub const fn has_recovery(&self) -> bool {
        matches!(self, Self::Unknown { .. }) || self.child_state_has_recovery()
    }
}

/// Missing, single-statement, or statement-only braced handler body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedSourceHandlerBody {
    Missing(AstNode<MissingBodyKind>),
    Statement(StatementNode),
    Block {
        syntax: AstNode<BlockKind>,
        statements: Box<[StatementNode]>,
        closed: bool,
    },
}

impl AttachedSourceHandlerBody {
    pub fn statements(&self) -> &[StatementNode] {
        match self {
            Self::Missing(_) => &[],
            Self::Statement(statement) => core::slice::from_ref(statement),
            Self::Block { statements, .. } => statements,
        }
    }

    pub const fn has_recovery(&self) -> bool {
        matches!(self, Self::Missing(_) | Self::Block { closed: false, .. })
    }
}

/// Typed but non-executable Source contract retained for final-HIR recovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedSourceContract {
    Requires(AstNode<RequiresClauseKind>),
    Ensures(AstNode<EnsuresClauseKind>),
}

/// One direct Source body member in exact source order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedSourceMember {
    From {
        syntax: AstNode<ExpressionStatementKind>,
        source_ordinal: u32,
        value: AttachedSourceExpression,
        duplicate: bool,
    },
    Backpressure {
        syntax: AstNode<AssignmentStatementKind>,
        source_ordinal: u32,
        assignment: AttachedSourcePunctuation,
        policy: Box<AttachedSourceBackpressurePolicy>,
        duplicate: bool,
    },
    Replay {
        syntax: AstNode<AssignmentStatementKind>,
        source_ordinal: u32,
        assignment: AttachedSourcePunctuation,
        policy: AttachedSourceReplayPolicy,
        duplicate: bool,
    },
    Privacy {
        syntax: AstNode<AssignmentStatementKind>,
        source_ordinal: u32,
        assignment: AttachedSourcePunctuation,
        policy: AttachedSourcePrivacyPolicy,
        duplicate: bool,
    },
    Handler {
        syntax: AstNode<OnStatementKind>,
        source_ordinal: u32,
        event: AttachedSourceHandlerEvent,
        arrow: AttachedSourcePunctuation,
        body: AttachedSourceHandlerBody,
    },
    UnsupportedContract {
        syntax: AttachedSourceContract,
        source_ordinal: u32,
        family_ordinal: u16,
        condition: AttachedSourceExpression,
        out_of_order: bool,
    },
    Recovery {
        syntax: StatementNode,
        source_ordinal: u32,
    },
}

impl AttachedSourceMember {
    pub const fn source_ordinal(&self) -> u32 {
        match self {
            Self::From { source_ordinal, .. }
            | Self::Backpressure { source_ordinal, .. }
            | Self::Replay { source_ordinal, .. }
            | Self::Privacy { source_ordinal, .. }
            | Self::Handler { source_ordinal, .. }
            | Self::UnsupportedContract { source_ordinal, .. }
            | Self::Recovery { source_ordinal, .. } => *source_ordinal,
        }
    }

    pub fn has_recovery(&self) -> bool {
        match self {
            Self::From {
                value, duplicate, ..
            } => value.has_recovery() || *duplicate,
            Self::Backpressure {
                assignment,
                policy,
                duplicate,
                ..
            } => assignment.is_missing() || policy.has_recovery() || *duplicate,
            Self::Replay {
                assignment,
                policy,
                duplicate,
                ..
            } => assignment.is_missing() || policy.has_recovery() || *duplicate,
            Self::Privacy {
                assignment,
                policy,
                duplicate,
                ..
            } => assignment.is_missing() || policy.has_recovery() || *duplicate,
            Self::Handler {
                event, arrow, body, ..
            } => event.has_recovery() || arrow.is_missing() || body.has_recovery(),
            Self::UnsupportedContract { .. } | Self::Recovery { .. } => true,
        }
    }
}

/// Missing or authored Source body with its exact member inventory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedSourceBody {
    Missing(AstNode<MissingBodyKind>),
    Braced {
        syntax: AstNode<BlockKind>,
        open: AstNode<OpenBraceKind>,
        close: AstNode<CloseBraceKind>,
        members: Box<[AttachedSourceMember]>,
    },
}

impl AttachedSourceBody {
    pub fn members(&self) -> &[AttachedSourceMember] {
        match self {
            Self::Missing(_) => &[],
            Self::Braced { members, .. } => members,
        }
    }

    pub fn is_closed(&self) -> bool {
        matches!(self, Self::Braced { close, .. } if !close.range().is_empty())
    }

    pub fn has_recovery(&self) -> bool {
        matches!(self, Self::Missing(_))
            || !self.is_closed()
            || self
                .members()
                .iter()
                .any(AttachedSourceMember::has_recovery)
    }
}

/// Complete attached Source declaration semantics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedSourceDeclaration {
    syntax: AstNode<SourceItemKind>,
    prefix: AttachedItemPrefix,
    id: AttachedSourceId,
    name: AttachedSourceName,
    source_type: AttachedSourceType,
    missing_type_colon: bool,
    body: AttachedSourceBody,
}

impl AttachedSourceDeclaration {
    pub const fn syntax(&self) -> &AstNode<SourceItemKind> {
        &self.syntax
    }

    pub const fn prefix(&self) -> &AttachedItemPrefix {
        &self.prefix
    }

    pub const fn id(&self) -> &AttachedSourceId {
        &self.id
    }

    pub const fn name(&self) -> &AttachedSourceName {
        &self.name
    }

    pub const fn source_type(&self) -> &AttachedSourceType {
        &self.source_type
    }

    pub const fn has_missing_type_colon(&self) -> bool {
        self.missing_type_colon
    }

    pub const fn body(&self) -> &AttachedSourceBody {
        &self.body
    }

    pub fn has_recovery(&self) -> bool {
        self.id.has_recovery()
            || self.name.has_recovery()
            || self.source_type.has_recovery()
            || self.missing_type_colon
            || self.body.has_recovery()
    }
}

impl AstNode<SourceItemKind> {
    /// Binds the sole parser-owned Source projection without source rediscovery.
    pub fn semantics(&self) -> Result<AttachedSourceDeclaration, SyntaxAccessError> {
        let pending = self
            .syntax()
            .source_declaration_projection()
            .cloned()
            .ok_or(SyntaxAccessError::MissingSourceDeclarationProjection { id: self.id() })?;
        Ok(AttachedSourceDeclaration {
            syntax: self.clone(),
            prefix: TypedItemNode::Source(self.clone()).attached_prefix()?,
            id: attach_source_id(self, &pending.id)?,
            name: attach_source_name(self, &pending.name)?,
            source_type: attach_source_type(self, pending.source_type)?,
            missing_type_colon: pending.missing_type_colon,
            body: attach_source_body(self, &pending.body)?,
        })
    }
}

fn attach_source_id(
    owner: &AstNode<SourceItemKind>,
    pending: &PendingSourceId,
) -> Result<AttachedSourceId, SyntaxAccessError> {
    let syntax = owner.public_id()?;
    match (pending, syntax) {
        (PendingSourceId::Absent, None) => Ok(AttachedSourceId::Absent),
        (
            PendingSourceId::Authored {
                value,
                source,
                canonical_source_family,
                requires_name,
            },
            Some(syntax),
        ) => {
            validate_range(owner.id(), syntax.range(), *source)?;
            Ok(AttachedSourceId::Authored {
                syntax,
                reference: value.clone(),
                canonical_source_family: *canonical_source_family,
                requires_name: *requires_name,
            })
        }
        _ => Err(invalid_source(owner.id())),
    }
}

fn attach_source_name(
    owner: &AstNode<SourceItemKind>,
    pending: &PendingSourceName,
) -> Result<AttachedSourceName, SyntaxAccessError> {
    let syntax = owner
        .header()?
        .syntax()
        .optional_unique_child(SyntaxRole::Name)?;
    match (pending, syntax) {
        (PendingSourceName::Absent, None) => Ok(AttachedSourceName::Absent),
        (PendingSourceName::Authored { value, source }, Some(syntax))
            if syntax.kind() == SyntaxKind::NameDefinition =>
        {
            validate_range(owner.id(), syntax.range(), *source)?;
            Ok(AttachedSourceName::Authored {
                syntax: syntax.cast()?,
                value: value.clone(),
            })
        }
        (PendingSourceName::Missing { insertion }, Some(syntax))
            if syntax.kind() == SyntaxKind::MissingName =>
        {
            validate_range(owner.id(), syntax.range(), *insertion)?;
            Ok(AttachedSourceName::Missing(syntax.cast()?))
        }
        _ => Err(invalid_source(owner.id())),
    }
}

fn attach_source_type(
    owner: &AstNode<SourceItemKind>,
    pending: PendingSourceTypeState,
) -> Result<AttachedSourceType, SyntaxAccessError> {
    let syntax = owner
        .header()?
        .syntax()
        .optional_unique_child(SyntaxRole::Type)?
        .ok_or_else(|| invalid_source(owner.id()))?;
    match pending {
        PendingSourceTypeState::Authored if syntax.kind().is_type_node() => Ok(
            AttachedSourceType::Authored(AttachedTypeRefNode::from_syntax(syntax)?),
        ),
        PendingSourceTypeState::Missing if syntax.kind() == SyntaxKind::MissingType => {
            Ok(AttachedSourceType::Missing {
                syntax: syntax.clone().cast()?,
                node: AttachedTypeRefNode::from_syntax(syntax)?,
            })
        }
        _ => Err(invalid_source(owner.id())),
    }
}

fn attach_source_body(
    owner: &AstNode<SourceItemKind>,
    pending: &PendingSourceBodyProjection,
) -> Result<AttachedSourceBody, SyntaxAccessError> {
    let body = owner
        .syntax()
        .optional_unique_child(SyntaxRole::Body)?
        .ok_or_else(|| invalid_source(owner.id()))?;
    match pending {
        PendingSourceBodyProjection::Missing if body.kind() == SyntaxKind::MissingBody => {
            Ok(AttachedSourceBody::Missing(body.cast()?))
        }
        PendingSourceBodyProjection::Braced { members, closed }
            if body.kind() == SyntaxKind::Block =>
        {
            let syntax = body.cast::<BlockKind>()?;
            let open = syntax.required_exact_child::<OpenBraceKind>(SyntaxRole::OpenDelimiter)?;
            let close =
                syntax.required_exact_child::<CloseBraceKind>(SyntaxRole::CloseDelimiter)?;
            if close.range().is_empty() == *closed {
                return Err(invalid_source(owner.id()));
            }
            let children = syntax
                .syntax()
                .children()
                .into_iter()
                .filter(|child| {
                    matches!(
                        child.role().class(),
                        SyntaxRoleClass::Statement | SyntaxRoleClass::ContractClause
                    )
                })
                .collect::<Vec<_>>();
            if children.len() != members.len() {
                return Err(invalid_source(owner.id()));
            }
            let members = children
                .into_iter()
                .zip(members)
                .enumerate()
                .map(|(ordinal, (syntax, pending))| {
                    let ordinal = u32::try_from(ordinal).map_err(|_| invalid_source(owner.id()))?;
                    if pending.source_ordinal() != ordinal {
                        return Err(invalid_source(owner.id()));
                    }
                    attach_source_member(syntax, pending, owner.id())
                })
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice();
            Ok(AttachedSourceBody::Braced {
                syntax,
                open,
                close,
                members,
            })
        }
        _ => Err(invalid_source(owner.id())),
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "the exhaustive Source member projection keeps each typed member family in one closed dispatcher"
)]
fn attach_source_member(
    syntax: SyntaxNodeHandle,
    pending: &PendingSourceMemberProjection,
    declaration: SyntaxNodeId,
) -> Result<AttachedSourceMember, SyntaxAccessError> {
    match pending {
        PendingSourceMemberProjection::From {
            source_ordinal,
            statement_ordinal,
            value,
            duplicate,
        } if syntax.kind() == SyntaxKind::ExpressionStatement
            && syntax.role() == SyntaxRole::Statement(*statement_ordinal) =>
        {
            Ok(AttachedSourceMember::From {
                value: attach_expression(&syntax, SyntaxRole::Initializer, *value, declaration)?,
                syntax: syntax.cast()?,
                source_ordinal: *source_ordinal,
                duplicate: *duplicate,
            })
        }
        PendingSourceMemberProjection::Backpressure {
            source_ordinal,
            statement_ordinal,
            assignment,
            policy,
            duplicate,
        } if syntax.kind() == SyntaxKind::AssignmentStatement
            && syntax.role() == SyntaxRole::Statement(*statement_ordinal) =>
        {
            attach_backpressure_member(
                &syntax,
                *source_ordinal,
                *assignment,
                policy,
                *duplicate,
                declaration,
            )
        }
        PendingSourceMemberProjection::Replay {
            source_ordinal,
            statement_ordinal,
            assignment,
            policy,
            duplicate,
        } if syntax.kind() == SyntaxKind::AssignmentStatement
            && syntax.role() == SyntaxRole::Statement(*statement_ordinal) =>
        {
            let expression = attach_expression(
                &syntax,
                SyntaxRole::Initializer,
                pending_named_state(policy),
                declaration,
            )?;
            Ok(AttachedSourceMember::Replay {
                syntax: syntax.cast()?,
                source_ordinal: *source_ordinal,
                assignment: attach_punctuation(&syntax, *assignment, declaration)?,
                policy: attach_replay_policy(expression, policy),
                duplicate: *duplicate,
            })
        }
        PendingSourceMemberProjection::Privacy {
            source_ordinal,
            statement_ordinal,
            assignment,
            policy,
            duplicate,
        } if syntax.kind() == SyntaxKind::AssignmentStatement
            && syntax.role() == SyntaxRole::Statement(*statement_ordinal) =>
        {
            let expression = attach_expression(
                &syntax,
                SyntaxRole::Initializer,
                pending_named_state(policy),
                declaration,
            )?;
            Ok(AttachedSourceMember::Privacy {
                syntax: syntax.cast()?,
                source_ordinal: *source_ordinal,
                assignment: attach_punctuation(&syntax, *assignment, declaration)?,
                policy: attach_privacy_policy(expression, policy),
                duplicate: *duplicate,
            })
        }
        PendingSourceMemberProjection::Handler {
            source_ordinal,
            statement_ordinal,
            event,
            arrow,
            body,
        } if syntax.kind() == SyntaxKind::OnStatement
            && syntax.role() == SyntaxRole::Statement(*statement_ordinal) =>
        {
            attach_handler_member(&syntax, *source_ordinal, event, *arrow, *body, declaration)
        }
        PendingSourceMemberProjection::UnsupportedContract {
            source_ordinal,
            contract_ordinal,
            family,
            family_ordinal,
            condition,
            out_of_order,
        } => attach_contract(
            &syntax,
            *source_ordinal,
            *contract_ordinal,
            *family,
            *family_ordinal,
            *condition,
            *out_of_order,
            declaration,
        ),
        PendingSourceMemberProjection::Recovery {
            source_ordinal,
            statement_ordinal,
        } if syntax.role() == SyntaxRole::Statement(*statement_ordinal) => {
            Ok(AttachedSourceMember::Recovery {
                syntax: FamilyNode::<StatementFamily>::new(syntax)?,
                source_ordinal: *source_ordinal,
            })
        }
        _ => Err(invalid_source(declaration)),
    }
}

fn attach_backpressure_member(
    syntax: &SyntaxNodeHandle,
    source_ordinal: u32,
    assignment: PendingSourcePunctuation,
    policy: &PendingSourceBackpressurePolicy,
    duplicate: bool,
    declaration: SyntaxNodeId,
) -> Result<AttachedSourceMember, SyntaxAccessError> {
    let expression = if matches!(policy, PendingSourceBackpressurePolicy::Bounded { .. }) {
        attach_bounded_policy_expression(syntax, declaration)?
    } else {
        attach_expression(
            syntax,
            SyntaxRole::Initializer,
            pending_backpressure_state(policy),
            declaration,
        )?
    };
    Ok(AttachedSourceMember::Backpressure {
        syntax: syntax.clone().cast()?,
        source_ordinal,
        assignment: attach_punctuation(syntax, assignment, declaration)?,
        policy: Box::new(attach_backpressure_policy(
            &expression,
            policy,
            declaration,
        )?),
        duplicate,
    })
}

fn attach_handler_member(
    syntax: &SyntaxNodeHandle,
    source_ordinal: u32,
    event: &PendingSourceHandlerEvent,
    arrow: PendingSourcePunctuation,
    body: PendingSourceHandlerBody,
    declaration: SyntaxNodeId,
) -> Result<AttachedSourceMember, SyntaxAccessError> {
    Ok(AttachedSourceMember::Handler {
        syntax: syntax.clone().cast()?,
        source_ordinal,
        event: attach_handler_event(syntax, event, declaration)?,
        arrow: attach_punctuation(syntax, arrow, declaration)?,
        body: attach_handler_body(syntax, body, declaration)?,
    })
}

fn attach_expression(
    owner: &SyntaxNodeHandle,
    role: SyntaxRole,
    state: PendingSourceChildState,
    declaration: SyntaxNodeId,
) -> Result<AttachedSourceExpression, SyntaxAccessError> {
    let syntax = owner
        .optional_unique_child(role)?
        .ok_or_else(|| invalid_source(declaration))?;
    let missing = syntax.kind() == SyntaxKind::MissingExpression;
    if state == PendingSourceChildState::Missing && missing {
        return Ok(AttachedSourceExpression::Missing(syntax.cast()?));
    }
    let expression = AttachedExpressionNode::from_syntax(syntax)?;
    let recovered = expression.projection().has_recovery();
    match state {
        PendingSourceChildState::Authored if !missing && !recovered => {
            Ok(AttachedSourceExpression::Authored(expression))
        }
        PendingSourceChildState::Invalid if !missing && recovered => {
            Ok(AttachedSourceExpression::Recovered(expression))
        }
        _ => Err(invalid_source(declaration)),
    }
}

/// A bounded policy remains the selected policy even when one of its typed
/// Call children recovered. The outer Call therefore admits either authored
/// or recovered expression state while still rejecting a missing/non-Call
/// owner in `attach_backpressure_policy`.
fn attach_bounded_policy_expression(
    owner: &SyntaxNodeHandle,
    declaration: SyntaxNodeId,
) -> Result<AttachedSourceExpression, SyntaxAccessError> {
    let syntax = owner
        .optional_unique_child(SyntaxRole::Initializer)?
        .ok_or_else(|| invalid_source(declaration))?;
    if syntax.kind() == SyntaxKind::MissingExpression {
        return Err(invalid_source(declaration));
    }
    let expression = AttachedExpressionNode::from_syntax(syntax)?;
    if expression.projection().has_recovery() {
        Ok(AttachedSourceExpression::Recovered(expression))
    } else {
        Ok(AttachedSourceExpression::Authored(expression))
    }
}

fn attach_pattern(
    owner: &SyntaxNodeHandle,
    state: PendingSourceChildState,
    declaration: SyntaxNodeId,
) -> Result<AttachedSourcePattern, SyntaxAccessError> {
    let syntax = owner
        .optional_unique_child(SyntaxRole::Pattern)?
        .ok_or_else(|| invalid_source(declaration))?;
    let missing = syntax.kind() == SyntaxKind::MissingPattern;
    let pattern = AttachedPatternNode::from_syntax(syntax)?;
    let recovered = !pattern.state().is_valid();
    match state {
        PendingSourceChildState::Authored if !missing && !recovered => {
            Ok(AttachedSourcePattern::Authored(pattern))
        }
        PendingSourceChildState::Missing if missing => Ok(AttachedSourcePattern::Missing(pattern)),
        PendingSourceChildState::Invalid if !missing && recovered => {
            Ok(AttachedSourcePattern::Recovered(pattern))
        }
        _ => Err(invalid_source(declaration)),
    }
}

fn attach_punctuation(
    owner: &SyntaxNodeHandle,
    pending: PendingSourcePunctuation,
    declaration: SyntaxNodeId,
) -> Result<AttachedSourcePunctuation, SyntaxAccessError> {
    let range = pending.range();
    if range.start() < owner.range().start()
        || range.end() > owner.range().end()
        || range.start() > range.end()
        || (pending.has_recovery() && !range.is_empty())
        || (!pending.has_recovery() && range.is_empty())
    {
        return Err(invalid_source(declaration));
    }
    Ok(AttachedSourcePunctuation {
        source: owner.source_span_for_range(range),
        missing: pending.has_recovery(),
    })
}

fn attach_backpressure_policy(
    expression: &AttachedSourceExpression,
    pending: &PendingSourceBackpressurePolicy,
    declaration: SyntaxNodeId,
) -> Result<AttachedSourceBackpressurePolicy, SyntaxAccessError> {
    Ok(match pending {
        PendingSourceBackpressurePolicy::Latest => {
            AttachedSourceBackpressurePolicy::Latest(expression.clone())
        }
        PendingSourceBackpressurePolicy::Bounded {
            capacity,
            overflow,
            unexpected_arguments,
            recovered_call,
        } => {
            let call = expression
                .expression()
                .ok_or_else(|| invalid_source(declaration))?
                .syntax()
                .syntax()
                .cast::<CallExpressionKind>()
                .map_err(|_| invalid_source(declaration))?;
            let arguments = call.arguments()?;
            AttachedSourceBackpressurePolicy::Bounded {
                expression: expression.clone(),
                capacity: Box::new(attach_bounded_argument(&arguments, *capacity, declaration)?),
                overflow: Box::new(attach_overflow_policy(&arguments, overflow, declaration)?),
                unexpected_arguments: *unexpected_arguments,
                recovered_call: *recovered_call,
            }
        }
        PendingSourceBackpressurePolicy::BlockingNotAllowed => {
            AttachedSourceBackpressurePolicy::BlockingNotAllowed(expression.clone())
        }
        PendingSourceBackpressurePolicy::Missing => {
            AttachedSourceBackpressurePolicy::Missing(expression.clone())
        }
        PendingSourceBackpressurePolicy::Unknown(value) => {
            AttachedSourceBackpressurePolicy::Unknown {
                expression: expression.clone(),
                value: value.clone(),
            }
        }
        PendingSourceBackpressurePolicy::Invalid => {
            AttachedSourceBackpressurePolicy::Invalid(expression.clone())
        }
    })
}

fn attach_bounded_argument(
    arguments: &[AstNode<CallArgumentKind>],
    pending: PendingSourceBoundedArgument,
    declaration: SyntaxNodeId,
) -> Result<AttachedSourceBoundedArgument, SyntaxAccessError> {
    let PendingSourceBoundedArgument::Present {
        ordinal,
        value,
        duplicate,
    } = pending
    else {
        return Ok(AttachedSourceBoundedArgument::Missing);
    };
    let syntax = arguments
        .get(usize::from(ordinal))
        .cloned()
        .ok_or_else(|| invalid_source(declaration))?;
    let attached_value =
        attach_expression(&syntax.syntax(), SyntaxRole::Operand, value, declaration)?;
    Ok(AttachedSourceBoundedArgument::Present {
        syntax,
        ordinal,
        value: Box::new(attached_value),
        duplicate,
    })
}

fn attach_overflow_policy(
    arguments: &[AstNode<CallArgumentKind>],
    pending: &PendingSourceOverflowPolicy,
    declaration: SyntaxNodeId,
) -> Result<AttachedSourceOverflowPolicy, SyntaxAccessError> {
    Ok(match pending {
        PendingSourceOverflowPolicy::DropOldest(argument) => {
            AttachedSourceOverflowPolicy::DropOldest(attach_bounded_argument(
                arguments,
                *argument,
                declaration,
            )?)
        }
        PendingSourceOverflowPolicy::DropNewest(argument) => {
            AttachedSourceOverflowPolicy::DropNewest(attach_bounded_argument(
                arguments,
                *argument,
                declaration,
            )?)
        }
        PendingSourceOverflowPolicy::Error(argument) => AttachedSourceOverflowPolicy::Error(
            attach_bounded_argument(arguments, *argument, declaration)?,
        ),
        PendingSourceOverflowPolicy::Coalesce(argument) => AttachedSourceOverflowPolicy::Coalesce(
            attach_bounded_argument(arguments, *argument, declaration)?,
        ),
        PendingSourceOverflowPolicy::Missing => AttachedSourceOverflowPolicy::Missing,
        PendingSourceOverflowPolicy::Unknown { argument, value } => {
            AttachedSourceOverflowPolicy::Unknown {
                argument: attach_bounded_argument(arguments, *argument, declaration)?,
                value: value.clone(),
            }
        }
        PendingSourceOverflowPolicy::Invalid { argument } => AttachedSourceOverflowPolicy::Invalid(
            attach_bounded_argument(arguments, *argument, declaration)?,
        ),
    })
}

fn pending_backpressure_state(policy: &PendingSourceBackpressurePolicy) -> PendingSourceChildState {
    match policy {
        PendingSourceBackpressurePolicy::Missing => PendingSourceChildState::Missing,
        PendingSourceBackpressurePolicy::Invalid => PendingSourceChildState::Invalid,
        PendingSourceBackpressurePolicy::Latest
        | PendingSourceBackpressurePolicy::Bounded { .. }
        | PendingSourceBackpressurePolicy::BlockingNotAllowed
        | PendingSourceBackpressurePolicy::Unknown(_) => PendingSourceChildState::Authored,
    }
}

fn pending_named_state<T>(policy: &PendingSourceNamedPolicy<T>) -> PendingSourceChildState {
    match policy {
        PendingSourceNamedPolicy::Missing => PendingSourceChildState::Missing,
        PendingSourceNamedPolicy::Invalid => PendingSourceChildState::Invalid,
        PendingSourceNamedPolicy::Known(_) | PendingSourceNamedPolicy::Unknown(_) => {
            PendingSourceChildState::Authored
        }
    }
}

fn attach_replay_policy(
    expression: AttachedSourceExpression,
    pending: &PendingSourceNamedPolicy<SourceReplaySyntaxKind>,
) -> AttachedSourceReplayPolicy {
    match pending {
        PendingSourceNamedPolicy::Known(SourceReplaySyntaxKind::Full) => {
            AttachedSourceReplayPolicy::Full(expression)
        }
        PendingSourceNamedPolicy::Known(SourceReplaySyntaxKind::HashOnly) => {
            AttachedSourceReplayPolicy::HashOnly(expression)
        }
        PendingSourceNamedPolicy::Known(SourceReplaySyntaxKind::Summary) => {
            AttachedSourceReplayPolicy::Summary(expression)
        }
        PendingSourceNamedPolicy::Known(SourceReplaySyntaxKind::EventOnly) => {
            AttachedSourceReplayPolicy::EventOnly(expression)
        }
        PendingSourceNamedPolicy::Known(SourceReplaySyntaxKind::None) => {
            AttachedSourceReplayPolicy::None(expression)
        }
        PendingSourceNamedPolicy::Missing => AttachedSourceReplayPolicy::Missing(expression),
        PendingSourceNamedPolicy::Unknown(value) => AttachedSourceReplayPolicy::Unknown {
            expression,
            value: value.clone(),
        },
        PendingSourceNamedPolicy::Invalid => AttachedSourceReplayPolicy::Invalid(expression),
    }
}

fn attach_privacy_policy(
    expression: AttachedSourceExpression,
    pending: &PendingSourceNamedPolicy<SourcePrivacySyntaxKind>,
) -> AttachedSourcePrivacyPolicy {
    match pending {
        PendingSourceNamedPolicy::Known(SourcePrivacySyntaxKind::Transient) => {
            AttachedSourcePrivacyPolicy::Transient(expression)
        }
        PendingSourceNamedPolicy::Known(SourcePrivacySyntaxKind::Redacted) => {
            AttachedSourcePrivacyPolicy::Redacted(expression)
        }
        PendingSourceNamedPolicy::Known(SourcePrivacySyntaxKind::Recordable) => {
            AttachedSourcePrivacyPolicy::Recordable(expression)
        }
        PendingSourceNamedPolicy::Known(SourcePrivacySyntaxKind::Private) => {
            AttachedSourcePrivacyPolicy::Private(expression)
        }
        PendingSourceNamedPolicy::Missing => AttachedSourcePrivacyPolicy::Missing(expression),
        PendingSourceNamedPolicy::Unknown(value) => AttachedSourcePrivacyPolicy::Unknown {
            expression,
            value: value.clone(),
        },
        PendingSourceNamedPolicy::Invalid => AttachedSourcePrivacyPolicy::Invalid(expression),
    }
}

fn attach_handler_event(
    owner: &SyntaxNodeHandle,
    pending: &PendingSourceHandlerEvent,
    declaration: SyntaxNodeId,
) -> Result<AttachedSourceHandlerEvent, SyntaxAccessError> {
    Ok(match pending {
        PendingSourceHandlerEvent::Item(state) => {
            require_handler_name(owner, declaration)?;
            AttachedSourceHandlerEvent::Item(attach_pattern(owner, *state, declaration)?)
        }
        PendingSourceHandlerEvent::Error(state) => {
            require_handler_name(owner, declaration)?;
            AttachedSourceHandlerEvent::Error(attach_pattern(owner, *state, declaration)?)
        }
        PendingSourceHandlerEvent::Progress(state) => {
            require_handler_name(owner, declaration)?;
            AttachedSourceHandlerEvent::Progress(attach_pattern(owner, *state, declaration)?)
        }
        PendingSourceHandlerEvent::Disconnected(state) => AttachedSourceHandlerEvent::Disconnected(
            attach_expression(owner, SyntaxRole::Condition, *state, declaration)?,
        ),
        PendingSourceHandlerEvent::PermissionRevoked(state) => {
            AttachedSourceHandlerEvent::PermissionRevoked(attach_expression(
                owner,
                SyntaxRole::Condition,
                *state,
                declaration,
            )?)
        }
        PendingSourceHandlerEvent::End(state) => AttachedSourceHandlerEvent::End(
            attach_expression(owner, SyntaxRole::Condition, *state, declaration)?,
        ),
        PendingSourceHandlerEvent::Unknown { value, condition } => {
            AttachedSourceHandlerEvent::Unknown {
                value: value.clone(),
                condition: attach_expression(
                    owner,
                    SyntaxRole::Condition,
                    *condition,
                    declaration,
                )?,
            }
        }
    })
}

fn require_handler_name(
    owner: &SyntaxNodeHandle,
    declaration: SyntaxNodeId,
) -> Result<AstNode<NameReferenceKind>, SyntaxAccessError> {
    let syntax = owner
        .optional_unique_child(SyntaxRole::Name)?
        .ok_or_else(|| invalid_source(declaration))?;
    syntax.cast().map_err(SyntaxAccessError::from)
}

fn attach_handler_body(
    owner: &SyntaxNodeHandle,
    pending: PendingSourceHandlerBody,
    declaration: SyntaxNodeId,
) -> Result<AttachedSourceHandlerBody, SyntaxAccessError> {
    let syntax = owner
        .optional_unique_child(SyntaxRole::Body)?
        .ok_or_else(|| invalid_source(declaration))?;
    match pending {
        PendingSourceHandlerBody::Missing if syntax.kind() == SyntaxKind::MissingBody => {
            Ok(AttachedSourceHandlerBody::Missing(syntax.cast()?))
        }
        PendingSourceHandlerBody::Statement if syntax.kind().is_statement() => {
            Ok(AttachedSourceHandlerBody::Statement(FamilyNode::<
                StatementFamily,
            >::new(
                syntax
            )?))
        }
        PendingSourceHandlerBody::Block { closed } if syntax.kind() == SyntaxKind::Block => {
            let syntax = syntax.cast::<BlockKind>()?;
            let close =
                syntax.required_exact_child::<CloseBraceKind>(SyntaxRole::CloseDelimiter)?;
            if close.range().is_empty() == closed || syntax.optional_tail()?.is_some() {
                return Err(invalid_source(declaration));
            }
            let statements = syntax.statements()?.into_boxed_slice();
            Ok(AttachedSourceHandlerBody::Block {
                syntax,
                statements,
                closed,
            })
        }
        _ => Err(invalid_source(declaration)),
    }
}

#[allow(clippy::too_many_arguments)]
fn attach_contract(
    syntax: &SyntaxNodeHandle,
    source_ordinal: u32,
    contract_ordinal: u16,
    family: SourceContractSyntaxKind,
    family_ordinal: u16,
    condition: PendingSourceChildState,
    out_of_order: bool,
    declaration: SyntaxNodeId,
) -> Result<AttachedSourceMember, SyntaxAccessError> {
    let expected_role = SyntaxRole::ContractClause(contract_ordinal);
    if syntax.role() != expected_role {
        return Err(invalid_source(declaration));
    }
    let contract = match family {
        SourceContractSyntaxKind::Requires if syntax.kind() == SyntaxKind::RequiresClause => {
            AttachedSourceContract::Requires(syntax.clone().cast()?)
        }
        SourceContractSyntaxKind::Ensures if syntax.kind() == SyntaxKind::EnsuresClause => {
            AttachedSourceContract::Ensures(syntax.clone().cast()?)
        }
        _ => return Err(invalid_source(declaration)),
    };
    Ok(AttachedSourceMember::UnsupportedContract {
        condition: attach_expression(
            syntax,
            SyntaxRole::ContractOperand(0),
            condition,
            declaration,
        )?,
        syntax: contract,
        source_ordinal,
        family_ordinal,
        out_of_order,
    })
}

fn validate_range(
    id: SyntaxNodeId,
    actual: arcweft_source::SourceRange,
    expected: arcweft_source::SourceRange,
) -> Result<(), SyntaxAccessError> {
    if actual == expected {
        Ok(())
    } else {
        Err(invalid_source(id))
    }
}

const fn invalid_source(id: SyntaxNodeId) -> SyntaxAccessError {
    SyntaxAccessError::InvalidSourceDeclarationProjection { id }
}
