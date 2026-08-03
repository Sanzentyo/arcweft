//! Typed Activity interface ownership over the attached grammar tree.

use std::collections::BTreeSet;

use arcweft_source::{SourceRange, SourceSpan};

use super::family::ExpressionFamily;
use super::node::{
    ActivityBodyKind, ActivityContractBlockKind, ActivityDeclarationItemKind,
    ActivityInputBlockKind, ActivityLifecycleMemberKind, ActivityModeMemberKind,
    ActivityOutputBlockKind, ActivityPortKind, AstNode, CloseBraceKind, ColonKind,
    DeclarationHeaderKind, EnsuresClauseKind, EqualsKind, ErrorDeclarationMemberKind,
    ErrorNodeKind, MissingBodyKind, MissingMemberValueKind, NameReferenceKind, OpenBraceKind,
    RequiresClauseKind,
};
use super::nominal::{punctuation, required_name, required_type};
use super::{
    AttachedExpressionNode, AttachedItemPrefix, AttachedRequiredName, AttachedRequiredPunctuation,
    AttachedRetainedHeader, AttachedTypeFamily, AttachedTypeRefNode, ExactAstKind,
    SyntaxAccessError, SyntaxNodeHandle, TypedItemNode,
};
use crate::grammar::kinds::{ActivityPolicySyntaxValue, SyntaxKind, SyntaxRole, SyntaxRoleClass};
use crate::name::SyntaxName;

/// Duplicate and source-order recovery derived from the typed section list.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AttachedActivitySectionState {
    duplicate: bool,
    out_of_order: bool,
}

impl AttachedActivitySectionState {
    pub const fn is_duplicate(self) -> bool {
        self.duplicate
    }

    pub const fn is_out_of_order(self) -> bool {
        self.out_of_order
    }

    pub const fn has_recovery(self) -> bool {
        self.duplicate || self.out_of_order
    }
}

/// Closed source mode or its parser-owned recovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedActivityMode {
    Deterministic(AstNode<NameReferenceKind>),
    CheckpointedRealtime(AstNode<NameReferenceKind>),
    ExternalRealtime(AstNode<NameReferenceKind>),
    Missing(AstNode<MissingMemberValueKind>),
    Invalid(AstNode<ErrorNodeKind>),
}

impl AttachedActivityMode {
    pub const fn has_recovery(&self) -> bool {
        matches!(self, Self::Missing(_) | Self::Invalid(_))
    }
}

/// Closed source lifecycle or its parser-owned recovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedActivityLifecycle {
    Stateless(AstNode<NameReferenceKind>),
    Snapshot(AstNode<NameReferenceKind>),
    Missing(AstNode<MissingMemberValueKind>),
    Invalid(AstNode<ErrorNodeKind>),
}

impl AttachedActivityLifecycle {
    pub const fn has_recovery(&self) -> bool {
        matches!(self, Self::Missing(_) | Self::Invalid(_))
    }
}

/// One source-ordered `mode` section.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedActivityModeMember {
    syntax: AstNode<ActivityModeMemberKind>,
    source_ordinal: u16,
    state: AttachedActivitySectionState,
    assignment: AttachedRequiredPunctuation,
    value: AttachedActivityMode,
}

impl AttachedActivityModeMember {
    pub const fn syntax(&self) -> &AstNode<ActivityModeMemberKind> {
        &self.syntax
    }

    pub const fn source_ordinal(&self) -> u16 {
        self.source_ordinal
    }

    pub const fn state(&self) -> AttachedActivitySectionState {
        self.state
    }

    pub const fn assignment(&self) -> &AttachedRequiredPunctuation {
        &self.assignment
    }

    pub const fn value(&self) -> &AttachedActivityMode {
        &self.value
    }
}

/// One source-ordered `lifecycle` section.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedActivityLifecycleMember {
    syntax: AstNode<ActivityLifecycleMemberKind>,
    source_ordinal: u16,
    state: AttachedActivitySectionState,
    assignment: AttachedRequiredPunctuation,
    value: AttachedActivityLifecycle,
}

impl AttachedActivityLifecycleMember {
    pub const fn syntax(&self) -> &AstNode<ActivityLifecycleMemberKind> {
        &self.syntax
    }

    pub const fn source_ordinal(&self) -> u16 {
        self.source_ordinal
    }

    pub const fn state(&self) -> AttachedActivitySectionState {
        self.state
    }

    pub const fn assignment(&self) -> &AttachedRequiredPunctuation {
        &self.assignment
    }

    pub const fn value(&self) -> &AttachedActivityLifecycle {
        &self.value
    }
}

/// One ordered input or output port.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedActivityPort {
    syntax: AstNode<ActivityPortKind>,
    source_ordinal: u16,
    name: AttachedRequiredName,
    colon: AttachedRequiredPunctuation,
    ty: AttachedTypeRefNode,
    initializer_recovery: Option<AstNode<ErrorNodeKind>>,
    duplicate: bool,
}

impl AttachedActivityPort {
    pub const fn syntax(&self) -> &AstNode<ActivityPortKind> {
        &self.syntax
    }

    pub const fn source_ordinal(&self) -> u16 {
        self.source_ordinal
    }

    pub const fn name(&self) -> &AttachedRequiredName {
        &self.name
    }

    pub const fn colon(&self) -> &AttachedRequiredPunctuation {
        &self.colon
    }

    pub const fn ty(&self) -> &AttachedTypeRefNode {
        &self.ty
    }

    pub const fn initializer_recovery(&self) -> Option<&AstNode<ErrorNodeKind>> {
        self.initializer_recovery.as_ref()
    }

    pub const fn is_duplicate(&self) -> bool {
        self.duplicate
    }

    pub fn has_recovery(&self) -> bool {
        self.name.is_missing()
            || self.colon.is_missing()
            || self.ty.family() == AttachedTypeFamily::Recovery
            || self.initializer_recovery.is_some()
            || self.duplicate
    }
}

/// Missing or braced port-section body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedActivityPortBody {
    Missing(AstNode<MissingBodyKind>),
    Braced {
        open: AstNode<OpenBraceKind>,
        close: AstNode<CloseBraceKind>,
        ports: Box<[AttachedActivityPort]>,
    },
}

impl AttachedActivityPortBody {
    pub fn ports(&self) -> &[AttachedActivityPort] {
        match self {
            Self::Missing(_) => &[],
            Self::Braced { ports, .. } => ports,
        }
    }

    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Missing(_) => true,
            Self::Braced { close, ports, .. } => {
                close.range().is_empty() || ports.iter().any(AttachedActivityPort::has_recovery)
            }
        }
    }
}

/// One source-ordered input section.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedActivityInputMember {
    syntax: AstNode<ActivityInputBlockKind>,
    source_ordinal: u16,
    state: AttachedActivitySectionState,
    body: AttachedActivityPortBody,
}

impl AttachedActivityInputMember {
    pub const fn syntax(&self) -> &AstNode<ActivityInputBlockKind> {
        &self.syntax
    }

    pub const fn source_ordinal(&self) -> u16 {
        self.source_ordinal
    }

    pub const fn state(&self) -> AttachedActivitySectionState {
        self.state
    }

    pub const fn body(&self) -> &AttachedActivityPortBody {
        &self.body
    }
}

/// One source-ordered output section.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedActivityOutputMember {
    syntax: AstNode<ActivityOutputBlockKind>,
    source_ordinal: u16,
    state: AttachedActivitySectionState,
    body: AttachedActivityPortBody,
}

impl AttachedActivityOutputMember {
    pub const fn syntax(&self) -> &AstNode<ActivityOutputBlockKind> {
        &self.syntax
    }

    pub const fn source_ordinal(&self) -> u16 {
        self.source_ordinal
    }

    pub const fn state(&self) -> AttachedActivitySectionState {
        self.state
    }

    pub const fn body(&self) -> &AttachedActivityPortBody {
        &self.body
    }
}

/// Authored contract expression or its exact missing insertion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedActivityContractCondition {
    Authored(Box<AttachedExpressionNode>),
    Missing(Box<AttachedExpressionNode>),
}

impl AttachedActivityContractCondition {
    /// The exact source-backed expression owner selected by the parser.
    pub const fn expression(&self) -> &AttachedExpressionNode {
        match self {
            Self::Authored(expression) | Self::Missing(expression) => expression,
        }
    }

    pub const fn is_missing(&self) -> bool {
        matches!(self, Self::Missing(_))
    }
}

/// One `requires` or `ensures` clause in exact contract source order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedActivityContractClause {
    Requires {
        syntax: AstNode<RequiresClauseKind>,
        source_ordinal: u16,
        family_ordinal: u16,
        condition: AttachedActivityContractCondition,
        out_of_order: bool,
    },
    Ensures {
        syntax: AstNode<EnsuresClauseKind>,
        source_ordinal: u16,
        family_ordinal: u16,
        condition: AttachedActivityContractCondition,
    },
}

impl AttachedActivityContractClause {
    pub const fn source_ordinal(&self) -> u16 {
        match self {
            Self::Requires { source_ordinal, .. } | Self::Ensures { source_ordinal, .. } => {
                *source_ordinal
            }
        }
    }

    pub const fn family_ordinal(&self) -> u16 {
        match self {
            Self::Requires { family_ordinal, .. } | Self::Ensures { family_ordinal, .. } => {
                *family_ordinal
            }
        }
    }

    pub const fn condition(&self) -> &AttachedActivityContractCondition {
        match self {
            Self::Requires { condition, .. } | Self::Ensures { condition, .. } => condition,
        }
    }

    /// Zero-width source anchor at the authored contract keyword.
    pub fn keyword_start_source_span(&self) -> SourceSpan {
        let syntax = match self {
            Self::Requires { syntax, .. } => syntax.syntax(),
            Self::Ensures { syntax, .. } => syntax.syntax(),
        };
        let start = syntax.range().start();
        syntax.source_span_for_range(SourceRange::new(start, start))
    }

    pub const fn is_out_of_order(&self) -> bool {
        matches!(
            self,
            Self::Requires {
                out_of_order: true,
                ..
            }
        )
    }
}

/// One known clause or unknown contract entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedActivityContractEntry {
    Clause(Box<AttachedActivityContractClause>),
    Recovery {
        source_ordinal: u16,
        syntax: AstNode<ErrorDeclarationMemberKind>,
    },
}

impl AttachedActivityContractEntry {
    pub const fn source_ordinal(&self) -> u16 {
        match self {
            Self::Clause(clause) => clause.source_ordinal(),
            Self::Recovery { source_ordinal, .. } => *source_ordinal,
        }
    }
}

/// Missing or braced contract body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedActivityContractBody {
    Missing(AstNode<MissingBodyKind>),
    Braced {
        open: AstNode<OpenBraceKind>,
        close: AstNode<CloseBraceKind>,
        entries: Box<[AttachedActivityContractEntry]>,
    },
}

impl AttachedActivityContractBody {
    pub fn entries(&self) -> &[AttachedActivityContractEntry] {
        match self {
            Self::Missing(_) => &[],
            Self::Braced { entries, .. } => entries,
        }
    }

    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Missing(_) => true,
            Self::Braced { close, entries, .. } => {
                close.range().is_empty()
                    || entries.iter().any(|entry| match entry {
                        AttachedActivityContractEntry::Recovery { .. } => true,
                        AttachedActivityContractEntry::Clause(clause) => {
                            clause.is_out_of_order()
                                || matches!(
                                    clause.condition(),
                                    AttachedActivityContractCondition::Missing(_)
                                )
                        }
                    })
            }
        }
    }
}

/// One source-ordered contract section.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedActivityContractMember {
    syntax: AstNode<ActivityContractBlockKind>,
    source_ordinal: u16,
    state: AttachedActivitySectionState,
    body: AttachedActivityContractBody,
}

impl AttachedActivityContractMember {
    pub const fn syntax(&self) -> &AstNode<ActivityContractBlockKind> {
        &self.syntax
    }

    pub const fn source_ordinal(&self) -> u16 {
        self.source_ordinal
    }

    pub const fn state(&self) -> AttachedActivitySectionState {
        self.state
    }

    pub const fn body(&self) -> &AttachedActivityContractBody {
        &self.body
    }
}

/// Closed Activity body-entry inventory in exact source order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedActivityEntry {
    Mode(AttachedActivityModeMember),
    Lifecycle(AttachedActivityLifecycleMember),
    Input(AttachedActivityInputMember),
    Output(AttachedActivityOutputMember),
    Contract(AttachedActivityContractMember),
    Recovery {
        source_ordinal: u16,
        syntax: AstNode<ErrorDeclarationMemberKind>,
    },
}

impl AttachedActivityEntry {
    pub const fn source_ordinal(&self) -> u16 {
        match self {
            Self::Mode(member) => member.source_ordinal(),
            Self::Lifecycle(member) => member.source_ordinal(),
            Self::Input(member) => member.source_ordinal(),
            Self::Output(member) => member.source_ordinal(),
            Self::Contract(member) => member.source_ordinal(),
            Self::Recovery { source_ordinal, .. } => *source_ordinal,
        }
    }
}

/// Missing or authored Activity body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedActivityBody {
    Missing(AstNode<MissingBodyKind>),
    Braced {
        syntax: AstNode<ActivityBodyKind>,
        open: AstNode<OpenBraceKind>,
        close: AstNode<CloseBraceKind>,
        entries: Box<[AttachedActivityEntry]>,
    },
}

impl AttachedActivityBody {
    pub fn entries(&self) -> &[AttachedActivityEntry] {
        match self {
            Self::Missing(_) => &[],
            Self::Braced { entries, .. } => entries,
        }
    }

    fn source_range(&self) -> arcweft_source::SourceRange {
        match self {
            Self::Missing(body) => body.range(),
            Self::Braced { syntax, .. } => syntax.range(),
        }
    }

    pub const fn is_missing(&self) -> bool {
        matches!(self, Self::Missing(_))
    }

    pub fn is_unclosed(&self) -> bool {
        matches!(self, Self::Braced { close, .. } if close.range().is_empty())
    }
}

/// One retained Activity header and its abstract interface only.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedActivityDeclaration {
    syntax: AstNode<ActivityDeclarationItemKind>,
    prefix: AttachedItemPrefix,
    header: AttachedRetainedHeader,
    body: AttachedActivityBody,
    declaration_recoveries: Box<[AstNode<ErrorNodeKind>]>,
}

impl AttachedActivityDeclaration {
    pub const fn syntax(&self) -> &AstNode<ActivityDeclarationItemKind> {
        &self.syntax
    }

    pub const fn prefix(&self) -> &AttachedItemPrefix {
        &self.prefix
    }

    pub const fn header(&self) -> &AttachedRetainedHeader {
        &self.header
    }

    pub const fn body(&self) -> &AttachedActivityBody {
        &self.body
    }

    /// Source anchor for the one synthetic requires scope.
    pub fn requires_scope_source_span(&self) -> SourceSpan {
        self.first_contract_clause(|clause| {
            matches!(clause, AttachedActivityContractClause::Requires { .. })
        })
        .map_or_else(
            || self.contract_scope_fallback_source_span(),
            AttachedActivityContractClause::keyword_start_source_span,
        )
    }

    /// Source anchor for the one synthetic ensures scope.
    pub fn ensures_scope_source_span(&self) -> SourceSpan {
        self.first_contract_clause(|clause| {
            matches!(clause, AttachedActivityContractClause::Ensures { .. })
        })
        .map_or_else(
            || self.contract_scope_fallback_source_span(),
            AttachedActivityContractClause::keyword_start_source_span,
        )
    }

    pub const fn declaration_recoveries(&self) -> &[AstNode<ErrorNodeKind>] {
        &self.declaration_recoveries
    }

    pub fn unexpected_header_recovery(&self) -> Option<&AstNode<ErrorNodeKind>> {
        let body_start = self.body.source_range().start();
        self.declaration_recoveries
            .iter()
            .find(|recovery| recovery.range().end() <= body_start)
    }

    pub fn trailing_recovery(&self) -> Option<&AstNode<ErrorNodeKind>> {
        let body_end = self.body.source_range().end();
        self.declaration_recoveries
            .iter()
            .find(|recovery| recovery.range().start() >= body_end)
    }

    fn first_contract_clause(
        &self,
        predicate: impl Fn(&AttachedActivityContractClause) -> bool,
    ) -> Option<&AttachedActivityContractClause> {
        self.body.entries().iter().find_map(|entry| {
            let AttachedActivityEntry::Contract(section) = entry else {
                return None;
            };
            section.body().entries().iter().find_map(|entry| {
                let AttachedActivityContractEntry::Clause(clause) = entry else {
                    return None;
                };
                predicate(clause).then_some(clause.as_ref())
            })
        })
    }

    fn contract_scope_fallback_source_span(&self) -> SourceSpan {
        let header = self.header.syntax().syntax();
        let end = header.range().end();
        header.source_span_for_range(SourceRange::new(end, end))
    }
}

impl AstNode<ActivityDeclarationItemKind> {
    /// Binds the one-pass Activity grammar without a detached interface reader.
    pub fn semantics(&self) -> Result<AttachedActivityDeclaration, SyntaxAccessError> {
        let item = TypedItemNode::Activity(self.clone());
        let header = self.required_exact_child::<DeclarationHeaderKind>(SyntaxRole::Element(0))?;
        let body = attach_body(self)?;
        let declaration_recoveries = self
            .ordered_exact_children::<ErrorNodeKind>(SyntaxRoleClass::Recovery)?
            .into_boxed_slice();
        let body_range = body.source_range();
        let classified_recoveries = declaration_recoveries
            .iter()
            .filter(|recovery| {
                recovery.range().end() <= body_range.start()
                    || recovery.range().start() >= body_range.end()
            })
            .count();
        if classified_recoveries != declaration_recoveries.len()
            || declaration_recoveries
                .iter()
                .filter(|recovery| recovery.range().end() <= body_range.start())
                .count()
                > 1
            || declaration_recoveries
                .iter()
                .filter(|recovery| recovery.range().start() >= body_range.end())
                .count()
                > 1
        {
            return Err(SyntaxAccessError::InvalidItemProjection { id: self.id() });
        }
        Ok(AttachedActivityDeclaration {
            syntax: self.clone(),
            prefix: item.attached_prefix()?,
            header: header.retained_semantics()?,
            body,
            declaration_recoveries,
        })
    }
}

fn attach_body(
    owner: &AstNode<ActivityDeclarationItemKind>,
) -> Result<AttachedActivityBody, SyntaxAccessError> {
    let body = owner
        .syntax()
        .optional_unique_child(SyntaxRole::Body)?
        .ok_or(SyntaxAccessError::InvalidItemProjection { id: owner.id() })?;
    if body.kind() == SyntaxKind::MissingBody {
        return Ok(AttachedActivityBody::Missing(body.cast()?));
    }
    let body = body.cast::<ActivityBodyKind>()?;
    let mut seen_sections = [false; 5];
    let mut highest_rank = None;
    let mut seen_ports = BTreeSet::<SyntaxName>::new();
    let entries = body
        .syntax()
        .ordered_children(SyntaxRoleClass::Member)?
        .into_iter()
        .enumerate()
        .map(|(index, syntax)| {
            let source_ordinal = u16::try_from(index)
                .map_err(|_| SyntaxAccessError::InvalidItemProjection { id: owner.id() })?;
            if syntax.role() != SyntaxRole::Member(source_ordinal) {
                return Err(SyntaxAccessError::InvalidItemProjection { id: owner.id() });
            }
            let Some(rank) = section_rank(syntax.kind()) else {
                return if syntax.kind() == SyntaxKind::ErrorDeclarationMember {
                    Ok(AttachedActivityEntry::Recovery {
                        source_ordinal,
                        syntax: syntax.cast()?,
                    })
                } else {
                    Err(SyntaxAccessError::InvalidItemProjection { id: owner.id() })
                };
            };
            let state = AttachedActivitySectionState {
                duplicate: seen_sections[rank],
                out_of_order: highest_rank.is_some_and(|highest| rank < highest),
            };
            seen_sections[rank] = true;
            highest_rank = Some(highest_rank.map_or(rank, |highest: usize| highest.max(rank)));
            attach_entry(&syntax, source_ordinal, state, &mut seen_ports, owner.id())
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_boxed_slice();
    Ok(AttachedActivityBody::Braced {
        open: body.required_exact_child::<OpenBraceKind>(SyntaxRole::OpenDelimiter)?,
        close: body.required_exact_child::<CloseBraceKind>(SyntaxRole::CloseDelimiter)?,
        syntax: body,
        entries,
    })
}

const fn section_rank(kind: SyntaxKind) -> Option<usize> {
    match kind {
        SyntaxKind::ActivityModeMember => Some(0),
        SyntaxKind::ActivityLifecycleMember => Some(1),
        SyntaxKind::ActivityInputBlock => Some(2),
        SyntaxKind::ActivityOutputBlock => Some(3),
        SyntaxKind::ActivityContractBlock => Some(4),
        _ => None,
    }
}

fn attach_entry(
    syntax: &SyntaxNodeHandle,
    source_ordinal: u16,
    state: AttachedActivitySectionState,
    seen_ports: &mut BTreeSet<SyntaxName>,
    owner: super::SyntaxNodeId,
) -> Result<AttachedActivityEntry, SyntaxAccessError> {
    match syntax.kind() {
        SyntaxKind::ActivityModeMember => {
            let syntax = syntax.cast::<ActivityModeMemberKind>()?;
            Ok(AttachedActivityEntry::Mode(AttachedActivityModeMember {
                assignment: punctuation(
                    &syntax.required_exact_child::<EqualsKind>(SyntaxRole::Equals)?,
                ),
                value: attach_mode(&syntax)?,
                syntax,
                source_ordinal,
                state,
            }))
        }
        SyntaxKind::ActivityLifecycleMember => {
            let syntax = syntax.cast::<ActivityLifecycleMemberKind>()?;
            Ok(AttachedActivityEntry::Lifecycle(
                AttachedActivityLifecycleMember {
                    assignment: punctuation(
                        &syntax.required_exact_child::<EqualsKind>(SyntaxRole::Equals)?,
                    ),
                    value: attach_lifecycle(&syntax)?,
                    syntax,
                    source_ordinal,
                    state,
                },
            ))
        }
        SyntaxKind::ActivityInputBlock => {
            let syntax = syntax.cast::<ActivityInputBlockKind>()?;
            Ok(AttachedActivityEntry::Input(AttachedActivityInputMember {
                body: attach_port_body(&syntax, SyntaxRoleClass::InputPort, seen_ports, owner)?,
                syntax,
                source_ordinal,
                state,
            }))
        }
        SyntaxKind::ActivityOutputBlock => {
            let syntax = syntax.cast::<ActivityOutputBlockKind>()?;
            Ok(AttachedActivityEntry::Output(
                AttachedActivityOutputMember {
                    body: attach_port_body(
                        &syntax,
                        SyntaxRoleClass::OutputPort,
                        seen_ports,
                        owner,
                    )?,
                    syntax,
                    source_ordinal,
                    state,
                },
            ))
        }
        SyntaxKind::ActivityContractBlock => {
            let syntax = syntax.cast::<ActivityContractBlockKind>()?;
            Ok(AttachedActivityEntry::Contract(
                AttachedActivityContractMember {
                    body: attach_contract_body(&syntax, owner)?,
                    syntax,
                    source_ordinal,
                    state,
                },
            ))
        }
        _ => Err(SyntaxAccessError::InvalidItemProjection { id: owner }),
    }
}

fn attach_mode(
    syntax: &AstNode<ActivityModeMemberKind>,
) -> Result<AttachedActivityMode, SyntaxAccessError> {
    match attach_policy_value(syntax)? {
        AttachedPolicyValue::Authored(value, ActivityPolicySyntaxValue::ModeDeterministic) => {
            Ok(AttachedActivityMode::Deterministic(value))
        }
        AttachedPolicyValue::Authored(
            value,
            ActivityPolicySyntaxValue::ModeCheckpointedRealtime,
        ) => Ok(AttachedActivityMode::CheckpointedRealtime(value)),
        AttachedPolicyValue::Authored(value, ActivityPolicySyntaxValue::ModeExternalRealtime) => {
            Ok(AttachedActivityMode::ExternalRealtime(value))
        }
        AttachedPolicyValue::Missing(value) => Ok(AttachedActivityMode::Missing(value)),
        AttachedPolicyValue::Invalid(value) => Ok(AttachedActivityMode::Invalid(value)),
        AttachedPolicyValue::Authored(_, _) => {
            Err(SyntaxAccessError::InvalidItemProjection { id: syntax.id() })
        }
    }
}

fn attach_lifecycle(
    syntax: &AstNode<ActivityLifecycleMemberKind>,
) -> Result<AttachedActivityLifecycle, SyntaxAccessError> {
    match attach_policy_value(syntax)? {
        AttachedPolicyValue::Authored(value, ActivityPolicySyntaxValue::LifecycleStateless) => {
            Ok(AttachedActivityLifecycle::Stateless(value))
        }
        AttachedPolicyValue::Authored(value, ActivityPolicySyntaxValue::LifecycleSnapshot) => {
            Ok(AttachedActivityLifecycle::Snapshot(value))
        }
        AttachedPolicyValue::Missing(value) => Ok(AttachedActivityLifecycle::Missing(value)),
        AttachedPolicyValue::Invalid(value) => Ok(AttachedActivityLifecycle::Invalid(value)),
        AttachedPolicyValue::Authored(_, _) => {
            Err(SyntaxAccessError::InvalidItemProjection { id: syntax.id() })
        }
    }
}

enum AttachedPolicyValue {
    Authored(AstNode<NameReferenceKind>, ActivityPolicySyntaxValue),
    Missing(AstNode<MissingMemberValueKind>),
    Invalid(AstNode<ErrorNodeKind>),
}

fn attach_policy_value<K: ExactAstKind>(
    owner: &AstNode<K>,
) -> Result<AttachedPolicyValue, SyntaxAccessError> {
    let values = owner
        .syntax()
        .children()
        .into_iter()
        .filter(|child| child.role().class() == SyntaxRoleClass::Value)
        .map(|child| child.cast::<NameReferenceKind>())
        .collect::<Result<Vec<_>, _>>()?;
    let recoveries = owner.syntax().ordered_children(SyntaxRoleClass::Recovery)?;
    match (values.as_slice(), recoveries.as_slice()) {
        ([value], []) => match value.role() {
            SyntaxRole::ActivityPolicyValue(kind) => {
                Ok(AttachedPolicyValue::Authored(value.clone(), kind))
            }
            _ => Err(SyntaxAccessError::InvalidItemProjection { id: owner.id() }),
        },
        ([], [recovery]) if recovery.kind() == SyntaxKind::MissingMemberValue => {
            Ok(AttachedPolicyValue::Missing(recovery.clone().cast()?))
        }
        ([], [recovery]) if recovery.kind() == SyntaxKind::ErrorNode => {
            Ok(AttachedPolicyValue::Invalid(recovery.clone().cast()?))
        }
        _ => Err(SyntaxAccessError::InvalidItemProjection { id: owner.id() }),
    }
}

fn attach_port_body<K: ExactAstKind>(
    owner: &AstNode<K>,
    role: SyntaxRoleClass,
    seen_ports: &mut BTreeSet<SyntaxName>,
    declaration: super::SyntaxNodeId,
) -> Result<AttachedActivityPortBody, SyntaxAccessError> {
    if let Some(missing) = owner.syntax().optional_unique_child(SyntaxRole::Body)? {
        return if missing.kind() == SyntaxKind::MissingBody {
            Ok(AttachedActivityPortBody::Missing(missing.cast()?))
        } else {
            Err(SyntaxAccessError::InvalidItemProjection { id: declaration })
        };
    }
    let ports = owner
        .ordered_exact_children::<ActivityPortKind>(role)?
        .into_iter()
        .enumerate()
        .map(|(index, port)| attach_port(port, role, index, seen_ports, declaration))
        .collect::<Result<Vec<_>, _>>()?
        .into_boxed_slice();
    Ok(AttachedActivityPortBody::Braced {
        open: owner.required_exact_child::<OpenBraceKind>(SyntaxRole::OpenDelimiter)?,
        close: owner.required_exact_child::<CloseBraceKind>(SyntaxRole::CloseDelimiter)?,
        ports,
    })
}

fn attach_port(
    syntax: AstNode<ActivityPortKind>,
    role: SyntaxRoleClass,
    index: usize,
    seen_ports: &mut BTreeSet<SyntaxName>,
    declaration: super::SyntaxNodeId,
) -> Result<AttachedActivityPort, SyntaxAccessError> {
    let source_ordinal = u16::try_from(index)
        .map_err(|_| SyntaxAccessError::InvalidItemProjection { id: declaration })?;
    let expected_role = match role {
        SyntaxRoleClass::InputPort => SyntaxRole::InputPort(source_ordinal),
        SyntaxRoleClass::OutputPort => SyntaxRole::OutputPort(source_ordinal),
        _ => return Err(SyntaxAccessError::InvalidItemProjection { id: declaration }),
    };
    if syntax.role() != expected_role {
        return Err(SyntaxAccessError::InvalidItemProjection { id: declaration });
    }
    let name = required_name(&syntax.syntax(), false)?;
    let duplicate = name
        .value()
        .is_some_and(|name| !seen_ports.insert(name.clone()));
    let recoveries = syntax.ordered_exact_children::<ErrorNodeKind>(SyntaxRoleClass::Recovery)?;
    let initializer_recovery = match recoveries.as_slice() {
        [] => None,
        [recovery] => Some(recovery.clone()),
        _ => return Err(SyntaxAccessError::InvalidItemProjection { id: declaration }),
    };
    Ok(AttachedActivityPort {
        colon: punctuation(&syntax.required_exact_child::<ColonKind>(SyntaxRole::Colon)?),
        ty: required_type(&syntax.syntax(), SyntaxRole::Type)?,
        syntax,
        source_ordinal,
        name,
        initializer_recovery,
        duplicate,
    })
}

fn attach_contract_body(
    syntax: &AstNode<ActivityContractBlockKind>,
    declaration: super::SyntaxNodeId,
) -> Result<AttachedActivityContractBody, SyntaxAccessError> {
    if let Some(missing) = syntax.syntax().optional_unique_child(SyntaxRole::Body)? {
        return if missing.kind() == SyntaxKind::MissingBody {
            Ok(AttachedActivityContractBody::Missing(missing.cast()?))
        } else {
            Err(SyntaxAccessError::InvalidItemProjection { id: declaration })
        };
    }
    let mut saw_ensures = false;
    let mut contract_ordinal = 0_u16;
    let mut recovery_ordinal = 0_u32;
    let mut requires_ordinal = 0_u16;
    let mut ensures_ordinal = 0_u16;
    let entries = syntax
        .syntax()
        .children()
        .into_iter()
        .filter(|child| {
            matches!(
                child.kind(),
                SyntaxKind::RequiresClause
                    | SyntaxKind::EnsuresClause
                    | SyntaxKind::ErrorDeclarationMember
            )
        })
        .enumerate()
        .map(|(index, child)| {
            let source_ordinal = u16::try_from(index)
                .map_err(|_| SyntaxAccessError::InvalidItemProjection { id: declaration })?;
            match child.kind() {
                SyntaxKind::RequiresClause => {
                    let clause = child.cast::<RequiresClauseKind>()?;
                    if clause.role() != SyntaxRole::ContractClause(contract_ordinal) {
                        return Err(SyntaxAccessError::InvalidItemProjection { id: declaration });
                    }
                    let condition = attach_condition(&clause.syntax(), declaration)?;
                    let entry = AttachedActivityContractEntry::Clause(Box::new(
                        AttachedActivityContractClause::Requires {
                            syntax: clause,
                            source_ordinal,
                            family_ordinal: requires_ordinal,
                            condition,
                            out_of_order: saw_ensures,
                        },
                    ));
                    requires_ordinal = requires_ordinal
                        .checked_add(1)
                        .ok_or(SyntaxAccessError::InvalidItemProjection { id: declaration })?;
                    contract_ordinal = contract_ordinal
                        .checked_add(1)
                        .ok_or(SyntaxAccessError::InvalidItemProjection { id: declaration })?;
                    Ok(entry)
                }
                SyntaxKind::EnsuresClause => {
                    let clause = child.cast::<EnsuresClauseKind>()?;
                    if clause.role() != SyntaxRole::ContractClause(contract_ordinal) {
                        return Err(SyntaxAccessError::InvalidItemProjection { id: declaration });
                    }
                    let condition = attach_condition(&clause.syntax(), declaration)?;
                    let entry = AttachedActivityContractEntry::Clause(Box::new(
                        AttachedActivityContractClause::Ensures {
                            syntax: clause,
                            source_ordinal,
                            family_ordinal: ensures_ordinal,
                            condition,
                        },
                    ));
                    ensures_ordinal = ensures_ordinal
                        .checked_add(1)
                        .ok_or(SyntaxAccessError::InvalidItemProjection { id: declaration })?;
                    contract_ordinal = contract_ordinal
                        .checked_add(1)
                        .ok_or(SyntaxAccessError::InvalidItemProjection { id: declaration })?;
                    saw_ensures = true;
                    Ok(entry)
                }
                SyntaxKind::ErrorDeclarationMember => {
                    if child.role() != SyntaxRole::Recovery(recovery_ordinal) {
                        return Err(SyntaxAccessError::InvalidItemProjection { id: declaration });
                    }
                    recovery_ordinal = recovery_ordinal
                        .checked_add(1)
                        .ok_or(SyntaxAccessError::InvalidItemProjection { id: declaration })?;
                    Ok(AttachedActivityContractEntry::Recovery {
                        source_ordinal,
                        syntax: child.cast()?,
                    })
                }
                _ => unreachable!("filtered Activity contract child"),
            }
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_boxed_slice();
    Ok(AttachedActivityContractBody::Braced {
        open: syntax.required_exact_child::<OpenBraceKind>(SyntaxRole::OpenDelimiter)?,
        close: syntax.required_exact_child::<CloseBraceKind>(SyntaxRole::CloseDelimiter)?,
        entries,
    })
}

fn attach_condition(
    owner: &SyntaxNodeHandle,
    declaration: super::SyntaxNodeId,
) -> Result<AttachedActivityContractCondition, SyntaxAccessError> {
    let condition = owner
        .optional_unique_child(SyntaxRole::ContractOperand(0))?
        .ok_or(SyntaxAccessError::InvalidItemProjection { id: declaration })?;
    if condition.kind() == SyntaxKind::MissingExpression {
        Ok(AttachedActivityContractCondition::Missing(Box::new(
            super::family::FamilyNode::<ExpressionFamily>::new(condition)?.semantic()?,
        )))
    } else {
        Ok(AttachedActivityContractCondition::Authored(Box::new(
            super::family::FamilyNode::<ExpressionFamily>::new(condition)?.semantic()?,
        )))
    }
}

#[cfg(test)]
mod tests;
