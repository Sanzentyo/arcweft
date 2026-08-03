//! Typed Layer declaration ownership over the attached grammar tree.

use std::collections::BTreeSet;

use crate::grammar::declaration_projection::{
    PendingLayerBodyProjection, PendingLayerKind, PendingLayerMemberProjection,
    PendingLayerMemberValue, PendingLayerPolicy,
};
use crate::grammar::kinds::{
    LayerKindSyntaxValue, LayerMemberSyntaxKind, LayerPolicySyntaxValue, SyntaxKind, SyntaxRole,
    SyntaxRoleClass,
};
use crate::id_ref::SyntaxIdRefSyntax;

use super::family::ExpressionFamily;
use super::node::{
    AstNode, CloseBraceKind, ColonKind, DeclarationHeaderKind, EqualsKind,
    ErrorDeclarationMemberKind, ErrorNodeKind, LayerBodyKind, LayerDeclarationItemKind,
    LayerKindNodeKind, LayerMemberKind, LayerPolicyValueKind, MissingBodyKind,
    MissingMemberValueKind, NameReferenceKind, OpenBraceKind, RetainedReferenceKind,
    WrongFamilyReferenceKind,
};
use super::nominal::punctuation;
use super::{
    AttachedExpressionNode, AttachedItemPrefix, AttachedRequiredPunctuation,
    AttachedRetainedHeader, SyntaxAccessError, SyntaxNodeHandle, TypedItemNode,
};

/// Closed authored Layer kind or its exact parser-owned recovery node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedLayerKind {
    Background(AstNode<LayerKindNodeKind>),
    World2d(AstNode<LayerKindNodeKind>),
    Character(AstNode<LayerKindNodeKind>),
    Effects(AstNode<LayerKindNodeKind>),
    Dialogue(AstNode<LayerKindNodeKind>),
    GameView(AstNode<LayerKindNodeKind>),
    HtmlView(AstNode<LayerKindNodeKind>),
    Activity(AstNode<LayerKindNodeKind>),
    Modal(AstNode<LayerKindNodeKind>),
    Overlay(AstNode<LayerKindNodeKind>),
    Debug(AstNode<LayerKindNodeKind>),
    Agent(AstNode<LayerKindNodeKind>),
    Offscreen(AstNode<LayerKindNodeKind>),
    Custom(AstNode<LayerKindNodeKind>),
    Missing(AstNode<LayerKindNodeKind>),
    Unknown(AstNode<LayerKindNodeKind>),
}

impl AttachedLayerKind {
    pub const fn syntax(&self) -> &AstNode<LayerKindNodeKind> {
        match self {
            Self::Background(syntax)
            | Self::World2d(syntax)
            | Self::Character(syntax)
            | Self::Effects(syntax)
            | Self::Dialogue(syntax)
            | Self::GameView(syntax)
            | Self::HtmlView(syntax)
            | Self::Activity(syntax)
            | Self::Modal(syntax)
            | Self::Overlay(syntax)
            | Self::Debug(syntax)
            | Self::Agent(syntax)
            | Self::Offscreen(syntax)
            | Self::Custom(syntax)
            | Self::Missing(syntax)
            | Self::Unknown(syntax) => syntax,
        }
    }

    pub const fn has_recovery(&self) -> bool {
        matches!(self, Self::Missing(_) | Self::Unknown(_))
    }
}

/// Duplicate state selected and structurally reconciled in source order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AttachedLayerMemberState {
    duplicate: bool,
}

impl AttachedLayerMemberState {
    pub const fn is_duplicate(self) -> bool {
        self.duplicate
    }
}

/// Shared exact source ownership for one known Layer member.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedLayerMember<V> {
    syntax: AstNode<LayerMemberKind>,
    name: AstNode<NameReferenceKind>,
    source_ordinal: u16,
    state: AttachedLayerMemberState,
    assignment: AttachedRequiredPunctuation,
    trailing_recovery: Option<AstNode<ErrorNodeKind>>,
    value: V,
}

impl<V> AttachedLayerMember<V> {
    pub const fn syntax(&self) -> &AstNode<LayerMemberKind> {
        &self.syntax
    }

    pub const fn name(&self) -> &AstNode<NameReferenceKind> {
        &self.name
    }

    pub const fn source_ordinal(&self) -> u16 {
        self.source_ordinal
    }

    pub const fn state(&self) -> AttachedLayerMemberState {
        self.state
    }

    pub const fn assignment(&self) -> &AttachedRequiredPunctuation {
        &self.assignment
    }

    pub const fn trailing_recovery(&self) -> Option<&AstNode<ErrorNodeKind>> {
        self.trailing_recovery.as_ref()
    }

    pub fn has_recovery(&self) -> bool {
        self.state.is_duplicate()
            || self.assignment.is_missing()
            || self.trailing_recovery.is_some()
    }

    pub const fn value(&self) -> &V {
        &self.value
    }
}

/// Family-constrained Layer reference with lexer-owned typed ID syntax.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedLayerReference {
    Retained {
        syntax: AstNode<RetainedReferenceKind>,
        reference: SyntaxIdRefSyntax,
    },
    WrongFamily {
        syntax: AstNode<WrongFamilyReferenceKind>,
        reference: SyntaxIdRefSyntax,
    },
    Missing(AstNode<MissingMemberValueKind>),
}

impl AttachedLayerReference {
    pub const fn reference(&self) -> Option<&SyntaxIdRefSyntax> {
        match self {
            Self::Retained { reference, .. } | Self::WrongFamily { reference, .. } => {
                Some(reference)
            }
            Self::Missing(_) => None,
        }
    }

    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Retained { reference, .. } => reference.value().is_err(),
            Self::WrongFamily { .. } | Self::Missing(_) => true,
        }
    }
}

/// Expression-valued Layer member or its exact missing-value node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedLayerExpression {
    Authored(Box<AttachedExpressionNode>),
    Missing(AstNode<MissingMemberValueKind>),
}

impl AttachedLayerExpression {
    pub fn expression(&self) -> Option<&AttachedExpressionNode> {
        match self {
            Self::Authored(expression) => Some(expression.as_ref()),
            Self::Missing(_) => None,
        }
    }

    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Authored(expression) => expression.projection().has_recovery(),
            Self::Missing(_) => true,
        }
    }
}

/// Closed Layer policy value across all five policy member families.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedLayerPolicy {
    PhaseBackground(AstNode<LayerPolicyValueKind>),
    PhaseWorld(AstNode<LayerPolicyValueKind>),
    PhaseCharacters(AstNode<LayerPolicyValueKind>),
    PhaseEffects(AstNode<LayerPolicyValueKind>),
    PhaseDialogue(AstNode<LayerPolicyValueKind>),
    PhaseGameView(AstNode<LayerPolicyValueKind>),
    PhaseHtmlView(AstNode<LayerPolicyValueKind>),
    PhaseModal(AstNode<LayerPolicyValueKind>),
    PhaseDebug(AstNode<LayerPolicyValueKind>),
    PhaseAgentOverlay(AstNode<LayerPolicyValueKind>),
    InputIgnore(AstNode<LayerPolicyValueKind>),
    InputPassThrough(AstNode<LayerPolicyValueKind>),
    InputHitTest(AstNode<LayerPolicyValueKind>),
    InputModal(AstNode<LayerPolicyValueKind>),
    InputCapture(AstNode<LayerPolicyValueKind>),
    HitTestNone(AstNode<LayerPolicyValueKind>),
    HitTestBounds(AstNode<LayerPolicyValueKind>),
    HitTestViewTree(AstNode<LayerPolicyValueKind>),
    HitTestObjectIdMask(AstNode<LayerPolicyValueKind>),
    CaptureNone(AstNode<LayerPolicyValueKind>),
    CaptureColor(AstNode<LayerPolicyValueKind>),
    CaptureObjectId(AstNode<LayerPolicyValueKind>),
    CaptureMask(AstNode<LayerPolicyValueKind>),
    CaptureAll(AstNode<LayerPolicyValueKind>),
    AccessibilityHidden(AstNode<LayerPolicyValueKind>),
    AccessibilityExposed(AstNode<LayerPolicyValueKind>),
    AccessibilityContainer(AstNode<LayerPolicyValueKind>),
    Invalid(AstNode<LayerPolicyValueKind>),
    Missing(AstNode<MissingMemberValueKind>),
}

impl AttachedLayerPolicy {
    pub const fn has_recovery(&self) -> bool {
        matches!(self, Self::Invalid(_) | Self::Missing(_))
    }
}

/// Closed Layer body-entry inventory in exact source order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedLayerEntry {
    Parent(AttachedLayerMember<AttachedLayerReference>),
    Phase(AttachedLayerMember<AttachedLayerPolicy>),
    Z(AttachedLayerMember<AttachedLayerExpression>),
    Visible(AttachedLayerMember<AttachedLayerExpression>),
    Transform(AttachedLayerMember<AttachedLayerExpression>),
    Input(AttachedLayerMember<AttachedLayerPolicy>),
    HitTest(AttachedLayerMember<AttachedLayerPolicy>),
    Capture(AttachedLayerMember<AttachedLayerPolicy>),
    Accessibility(AttachedLayerMember<AttachedLayerPolicy>),
    View(AttachedLayerMember<AttachedLayerReference>),
    Activity(AttachedLayerMember<AttachedLayerReference>),
    Recovery {
        source_ordinal: u16,
        syntax: AstNode<ErrorDeclarationMemberKind>,
    },
}

impl AttachedLayerEntry {
    pub const fn source_ordinal(&self) -> u16 {
        match self {
            Self::Parent(member) | Self::View(member) | Self::Activity(member) => {
                member.source_ordinal()
            }
            Self::Phase(member)
            | Self::Input(member)
            | Self::HitTest(member)
            | Self::Capture(member)
            | Self::Accessibility(member) => member.source_ordinal(),
            Self::Z(member) | Self::Visible(member) | Self::Transform(member) => {
                member.source_ordinal()
            }
            Self::Recovery { source_ordinal, .. } => *source_ordinal,
        }
    }

    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Parent(member) | Self::View(member) | Self::Activity(member) => {
                member.has_recovery() || member.value().has_recovery()
            }
            Self::Phase(member)
            | Self::Input(member)
            | Self::HitTest(member)
            | Self::Capture(member)
            | Self::Accessibility(member) => member.has_recovery() || member.value().has_recovery(),
            Self::Z(member) | Self::Visible(member) | Self::Transform(member) => {
                member.has_recovery() || member.value().has_recovery()
            }
            Self::Recovery { .. } => true,
        }
    }
}

/// Missing or authored Layer body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedLayerBody {
    Missing(AstNode<MissingBodyKind>),
    Braced {
        syntax: AstNode<LayerBodyKind>,
        open: AstNode<OpenBraceKind>,
        close: AstNode<CloseBraceKind>,
        entries: Box<[AttachedLayerEntry]>,
    },
}

impl AttachedLayerBody {
    pub fn entries(&self) -> &[AttachedLayerEntry] {
        match self {
            Self::Missing(_) => &[],
            Self::Braced { entries, .. } => entries,
        }
    }

    pub const fn is_missing(&self) -> bool {
        matches!(self, Self::Missing(_))
    }

    pub fn is_unclosed(&self) -> bool {
        matches!(self, Self::Braced { close, .. } if close.range().is_empty())
    }

    pub fn has_recovery(&self) -> bool {
        self.is_missing()
            || self.is_unclosed()
            || self.entries().iter().any(AttachedLayerEntry::has_recovery)
    }
}

/// One source-bound Layer declaration and its parser-owned schema projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedLayerDeclaration {
    syntax: AstNode<LayerDeclarationItemKind>,
    prefix: AttachedItemPrefix,
    header: AttachedRetainedHeader,
    colon: AttachedRequiredPunctuation,
    kind: AttachedLayerKind,
    body: AttachedLayerBody,
    trailing_recovery: Option<AstNode<ErrorNodeKind>>,
}

impl AttachedLayerDeclaration {
    pub const fn syntax(&self) -> &AstNode<LayerDeclarationItemKind> {
        &self.syntax
    }

    pub const fn prefix(&self) -> &AttachedItemPrefix {
        &self.prefix
    }

    pub const fn header(&self) -> &AttachedRetainedHeader {
        &self.header
    }

    pub const fn colon(&self) -> &AttachedRequiredPunctuation {
        &self.colon
    }

    pub const fn kind(&self) -> &AttachedLayerKind {
        &self.kind
    }

    pub const fn body(&self) -> &AttachedLayerBody {
        &self.body
    }

    pub const fn trailing_recovery(&self) -> Option<&AstNode<ErrorNodeKind>> {
        self.trailing_recovery.as_ref()
    }

    pub fn has_recovery(&self) -> bool {
        self.colon.is_missing()
            || self.kind.has_recovery()
            || self.body.has_recovery()
            || self.trailing_recovery.is_some()
    }
}

impl AstNode<LayerDeclarationItemKind> {
    /// Binds the sole one-pass Layer projection without source rediscovery.
    pub fn semantics(&self) -> Result<AttachedLayerDeclaration, SyntaxAccessError> {
        let pending = self
            .syntax()
            .layer_projection()
            .cloned()
            .ok_or(SyntaxAccessError::MissingLayerProjection { id: self.id() })?;
        let item = TypedItemNode::Layer(self.clone());
        let header = self.required_exact_child::<DeclarationHeaderKind>(SyntaxRole::Element(0))?;
        let colon_syntax = header.required_exact_child::<ColonKind>(SyntaxRole::Colon)?;
        validate_range(self.id(), colon_syntax.range(), pending.colon().range())?;
        let kind = attach_kind(&header, pending.kind(), self.id())?;
        let body = attach_body(self, pending.body())?;
        let recoveries = self.ordered_exact_children::<ErrorNodeKind>(SyntaxRoleClass::Recovery)?;
        let trailing_recovery = match (pending.has_trailing_syntax(), recoveries.as_slice()) {
            (false, []) => None,
            (true, [recovery]) => Some(recovery.clone()),
            _ => return Err(SyntaxAccessError::InvalidLayerProjection { id: self.id() }),
        };
        Ok(AttachedLayerDeclaration {
            syntax: self.clone(),
            prefix: item.attached_prefix()?,
            header: header.retained_semantics()?,
            colon: punctuation(&colon_syntax),
            kind,
            body,
            trailing_recovery,
        })
    }
}

fn attach_kind(
    header: &AstNode<DeclarationHeaderKind>,
    pending: PendingLayerKind,
    owner: super::SyntaxNodeId,
) -> Result<AttachedLayerKind, SyntaxAccessError> {
    let syntax = header.required_exact_child::<LayerKindNodeKind>(SyntaxRole::Kind)?;
    let children = syntax.syntax().children();
    match (pending, children.as_slice()) {
        (PendingLayerKind::Authored(value), [child])
            if child.kind() == SyntaxKind::NameReference
                && child.role() == SyntaxRole::LayerKindValue(value) =>
        {
            Ok(match value {
                LayerKindSyntaxValue::Background => AttachedLayerKind::Background(syntax),
                LayerKindSyntaxValue::World2d => AttachedLayerKind::World2d(syntax),
                LayerKindSyntaxValue::Character => AttachedLayerKind::Character(syntax),
                LayerKindSyntaxValue::Effects => AttachedLayerKind::Effects(syntax),
                LayerKindSyntaxValue::Dialogue => AttachedLayerKind::Dialogue(syntax),
                LayerKindSyntaxValue::GameView => AttachedLayerKind::GameView(syntax),
                LayerKindSyntaxValue::HtmlView => AttachedLayerKind::HtmlView(syntax),
                LayerKindSyntaxValue::Activity => AttachedLayerKind::Activity(syntax),
                LayerKindSyntaxValue::Modal => AttachedLayerKind::Modal(syntax),
                LayerKindSyntaxValue::Overlay => AttachedLayerKind::Overlay(syntax),
                LayerKindSyntaxValue::Debug => AttachedLayerKind::Debug(syntax),
                LayerKindSyntaxValue::Agent => AttachedLayerKind::Agent(syntax),
                LayerKindSyntaxValue::Offscreen => AttachedLayerKind::Offscreen(syntax),
                LayerKindSyntaxValue::Custom => AttachedLayerKind::Custom(syntax),
            })
        }
        (PendingLayerKind::Missing, [child])
            if child.kind() == SyntaxKind::MissingMemberValue
                && child.role() == SyntaxRole::Recovery(0) =>
        {
            Ok(AttachedLayerKind::Missing(syntax))
        }
        (PendingLayerKind::Unknown, [child])
            if child.kind() == SyntaxKind::ErrorNode && child.role() == SyntaxRole::Recovery(0) =>
        {
            Ok(AttachedLayerKind::Unknown(syntax))
        }
        _ => Err(SyntaxAccessError::InvalidLayerProjection { id: owner }),
    }
}

fn attach_body(
    owner: &AstNode<LayerDeclarationItemKind>,
    pending: &PendingLayerBodyProjection,
) -> Result<AttachedLayerBody, SyntaxAccessError> {
    let body = owner
        .syntax()
        .optional_unique_child(SyntaxRole::Body)?
        .ok_or(SyntaxAccessError::InvalidLayerProjection { id: owner.id() })?;
    match pending {
        PendingLayerBodyProjection::Missing if body.kind() == SyntaxKind::MissingBody => {
            Ok(AttachedLayerBody::Missing(body.cast()?))
        }
        PendingLayerBodyProjection::Braced { closed, members }
            if body.kind() == SyntaxKind::LayerBody =>
        {
            let syntax = body.cast::<LayerBodyKind>()?;
            let open = syntax.required_exact_child::<OpenBraceKind>(SyntaxRole::OpenDelimiter)?;
            let close =
                syntax.required_exact_child::<CloseBraceKind>(SyntaxRole::CloseDelimiter)?;
            if close.range().is_empty() == *closed {
                return Err(SyntaxAccessError::InvalidLayerProjection { id: owner.id() });
            }
            let children = syntax.syntax().ordered_children(SyntaxRoleClass::Member)?;
            if children.len() != members.len() {
                return Err(SyntaxAccessError::InvalidLayerProjection { id: owner.id() });
            }
            let mut seen = BTreeSet::new();
            let entries = children
                .into_iter()
                .zip(members)
                .map(|(child, pending)| attach_entry(&child, pending, &mut seen, owner.id()))
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice();
            Ok(AttachedLayerBody::Braced {
                syntax,
                open,
                close,
                entries,
            })
        }
        _ => Err(SyntaxAccessError::InvalidLayerProjection { id: owner.id() }),
    }
}

struct LayerMemberParts {
    syntax: AstNode<LayerMemberKind>,
    name: AstNode<NameReferenceKind>,
    source_ordinal: u16,
    state: AttachedLayerMemberState,
    assignment: AttachedRequiredPunctuation,
    trailing_recovery: Option<AstNode<ErrorNodeKind>>,
}

impl LayerMemberParts {
    fn with_value<V>(self, value: V) -> AttachedLayerMember<V> {
        AttachedLayerMember {
            syntax: self.syntax,
            name: self.name,
            source_ordinal: self.source_ordinal,
            state: self.state,
            assignment: self.assignment,
            trailing_recovery: self.trailing_recovery,
            value,
        }
    }

    fn with_reference(
        self,
        pending: &PendingLayerMemberValue,
        role: SyntaxRole,
        owner: super::SyntaxNodeId,
    ) -> Result<AttachedLayerMember<AttachedLayerReference>, SyntaxAccessError> {
        let value = attach_reference(&self.syntax, pending, role, owner)?;
        Ok(self.with_value(value))
    }

    fn with_policy(
        self,
        pending: &PendingLayerMemberValue,
        member: LayerMemberSyntaxKind,
        owner: super::SyntaxNodeId,
    ) -> Result<AttachedLayerMember<AttachedLayerPolicy>, SyntaxAccessError> {
        let value = attach_policy(&self.syntax, pending, member, owner)?;
        Ok(self.with_value(value))
    }

    fn with_expression(
        self,
        pending: &PendingLayerMemberValue,
        owner: super::SyntaxNodeId,
    ) -> Result<AttachedLayerMember<AttachedLayerExpression>, SyntaxAccessError> {
        let value = attach_expression(&self.syntax, pending, owner)?;
        Ok(self.with_value(value))
    }
}

fn attach_entry(
    syntax: &SyntaxNodeHandle,
    pending: &PendingLayerMemberProjection,
    seen: &mut BTreeSet<LayerMemberSyntaxKind>,
    owner: super::SyntaxNodeId,
) -> Result<AttachedLayerEntry, SyntaxAccessError> {
    let ordinal = pending.source_ordinal();
    if syntax.role() != SyntaxRole::Member(ordinal) {
        return Err(SyntaxAccessError::InvalidLayerProjection { id: owner });
    }
    let Some(kind) = pending.kind() else {
        return if syntax.kind() == SyntaxKind::ErrorDeclarationMember {
            Ok(AttachedLayerEntry::Recovery {
                source_ordinal: ordinal,
                syntax: syntax.cast()?,
            })
        } else {
            Err(SyntaxAccessError::InvalidLayerProjection { id: owner })
        };
    };
    if syntax.kind() != SyntaxKind::LayerMember {
        return Err(SyntaxAccessError::InvalidLayerProjection { id: owner });
    }
    let syntax = syntax.cast::<LayerMemberKind>()?;
    let duplicate = !seen.insert(kind);
    if duplicate != pending.duplicate() {
        return Err(SyntaxAccessError::InvalidLayerProjection { id: owner });
    }
    let name = syntax
        .syntax()
        .optional_unique_child(SyntaxRole::LayerMemberName(kind))?
        .ok_or(SyntaxAccessError::InvalidLayerProjection { id: owner })?
        .cast::<NameReferenceKind>()?;
    let assignment_syntax = syntax.required_exact_child::<EqualsKind>(SyntaxRole::Equals)?;
    let assignment = pending
        .assignment()
        .ok_or(SyntaxAccessError::InvalidLayerProjection { id: owner })?;
    validate_range(owner, assignment_syntax.range(), assignment.range())?;
    let state = AttachedLayerMemberState { duplicate };
    let assignment = punctuation(&assignment_syntax);
    let recoveries = syntax.ordered_exact_children::<ErrorNodeKind>(SyntaxRoleClass::Recovery)?;
    let trailing_recovery = match (pending.has_trailing_recovery(), recoveries.as_slice()) {
        (false, []) => None,
        (true, [recovery]) => Some(recovery.clone()),
        _ => return Err(SyntaxAccessError::InvalidLayerProjection { id: owner }),
    };
    let value = pending
        .value()
        .ok_or(SyntaxAccessError::InvalidLayerProjection { id: owner })?;
    let parts = LayerMemberParts {
        syntax,
        name,
        source_ordinal: ordinal,
        state,
        assignment,
        trailing_recovery,
    };
    attach_known_entry(kind, parts, value, owner)
}

fn attach_known_entry(
    kind: LayerMemberSyntaxKind,
    parts: LayerMemberParts,
    value: &PendingLayerMemberValue,
    owner: super::SyntaxNodeId,
) -> Result<AttachedLayerEntry, SyntaxAccessError> {
    match kind {
        LayerMemberSyntaxKind::Parent => Ok(AttachedLayerEntry::Parent(parts.with_reference(
            value,
            SyntaxRole::Reference(0),
            owner,
        )?)),
        LayerMemberSyntaxKind::Phase => Ok(AttachedLayerEntry::Phase(
            parts.with_policy(value, kind, owner)?,
        )),
        LayerMemberSyntaxKind::Z => Ok(AttachedLayerEntry::Z(parts.with_expression(value, owner)?)),
        LayerMemberSyntaxKind::Visible => Ok(AttachedLayerEntry::Visible(
            parts.with_expression(value, owner)?,
        )),
        LayerMemberSyntaxKind::Transform => Ok(AttachedLayerEntry::Transform(
            parts.with_expression(value, owner)?,
        )),
        LayerMemberSyntaxKind::Input => Ok(AttachedLayerEntry::Input(
            parts.with_policy(value, kind, owner)?,
        )),
        LayerMemberSyntaxKind::HitTest => Ok(AttachedLayerEntry::HitTest(
            parts.with_policy(value, kind, owner)?,
        )),
        LayerMemberSyntaxKind::Capture => Ok(AttachedLayerEntry::Capture(
            parts.with_policy(value, kind, owner)?,
        )),
        LayerMemberSyntaxKind::Accessibility => Ok(AttachedLayerEntry::Accessibility(
            parts.with_policy(value, kind, owner)?,
        )),
        LayerMemberSyntaxKind::View => Ok(AttachedLayerEntry::View(parts.with_reference(
            value,
            SyntaxRole::Reference(1),
            owner,
        )?)),
        LayerMemberSyntaxKind::Activity => Ok(AttachedLayerEntry::Activity(parts.with_reference(
            value,
            SyntaxRole::Reference(2),
            owner,
        )?)),
    }
}

fn attach_reference(
    owner: &AstNode<LayerMemberKind>,
    pending: &PendingLayerMemberValue,
    role: SyntaxRole,
    declaration: super::SyntaxNodeId,
) -> Result<AttachedLayerReference, SyntaxAccessError> {
    if matches!(pending, PendingLayerMemberValue::Missing) {
        return Ok(AttachedLayerReference::Missing(
            owner.required_exact_child::<MissingMemberValueKind>(SyntaxRole::Initializer)?,
        ));
    }
    let PendingLayerMemberValue::Reference(reference) = pending else {
        return Err(SyntaxAccessError::InvalidLayerProjection { id: declaration });
    };
    let syntax = owner
        .syntax()
        .optional_unique_child(role)?
        .ok_or(SyntaxAccessError::InvalidLayerProjection { id: declaration })?;
    if reference.is_wrong_absolute_family() {
        Ok(AttachedLayerReference::WrongFamily {
            syntax: syntax.cast()?,
            reference: reference.syntax().clone(),
        })
    } else {
        Ok(AttachedLayerReference::Retained {
            syntax: syntax.cast()?,
            reference: reference.syntax().clone(),
        })
    }
}

fn attach_expression(
    owner: &AstNode<LayerMemberKind>,
    pending: &PendingLayerMemberValue,
    declaration: super::SyntaxNodeId,
) -> Result<AttachedLayerExpression, SyntaxAccessError> {
    let value = owner
        .syntax()
        .optional_unique_child(SyntaxRole::Initializer)?
        .ok_or(SyntaxAccessError::InvalidLayerProjection { id: declaration })?;
    match pending {
        PendingLayerMemberValue::Expression => Ok(AttachedLayerExpression::Authored(Box::new(
            super::family::FamilyNode::<ExpressionFamily>::new(value)?.semantic()?,
        ))),
        PendingLayerMemberValue::Missing if value.kind() == SyntaxKind::MissingMemberValue => {
            Ok(AttachedLayerExpression::Missing(value.cast()?))
        }
        _ => Err(SyntaxAccessError::InvalidLayerProjection { id: declaration }),
    }
}

fn attach_policy(
    owner: &AstNode<LayerMemberKind>,
    pending: &PendingLayerMemberValue,
    member: LayerMemberSyntaxKind,
    declaration: super::SyntaxNodeId,
) -> Result<AttachedLayerPolicy, SyntaxAccessError> {
    if matches!(pending, PendingLayerMemberValue::Missing) {
        return Ok(AttachedLayerPolicy::Missing(
            owner.required_exact_child::<MissingMemberValueKind>(SyntaxRole::Initializer)?,
        ));
    }
    let PendingLayerMemberValue::Policy(policy) = pending else {
        return Err(SyntaxAccessError::InvalidLayerProjection { id: declaration });
    };
    let syntax = owner.required_exact_child::<LayerPolicyValueKind>(SyntaxRole::Policy(0))?;
    match policy {
        PendingLayerPolicy::Unknown => {
            let recovery = syntax.required_exact_child::<ErrorNodeKind>(SyntaxRole::Recovery(0))?;
            if syntax.syntax().children().len() != 1 || recovery.range().is_empty() {
                return Err(SyntaxAccessError::InvalidLayerProjection { id: declaration });
            }
            Ok(AttachedLayerPolicy::Invalid(syntax))
        }
        PendingLayerPolicy::Authored(value) if policy_belongs_to(member, *value) => {
            let authored = syntax
                .syntax()
                .optional_unique_child(SyntaxRole::LayerPolicyValue(*value))?
                .ok_or(SyntaxAccessError::InvalidLayerProjection { id: declaration })?
                .cast::<NameReferenceKind>()?;
            if syntax.syntax().children().len() != 1 || authored.range().is_empty() {
                return Err(SyntaxAccessError::InvalidLayerProjection { id: declaration });
            }
            Ok(match value {
                LayerPolicySyntaxValue::PhaseBackground => {
                    AttachedLayerPolicy::PhaseBackground(syntax)
                }
                LayerPolicySyntaxValue::PhaseWorld => AttachedLayerPolicy::PhaseWorld(syntax),
                LayerPolicySyntaxValue::PhaseCharacters => {
                    AttachedLayerPolicy::PhaseCharacters(syntax)
                }
                LayerPolicySyntaxValue::PhaseEffects => AttachedLayerPolicy::PhaseEffects(syntax),
                LayerPolicySyntaxValue::PhaseDialogue => AttachedLayerPolicy::PhaseDialogue(syntax),
                LayerPolicySyntaxValue::PhaseGameView => AttachedLayerPolicy::PhaseGameView(syntax),
                LayerPolicySyntaxValue::PhaseHtmlView => AttachedLayerPolicy::PhaseHtmlView(syntax),
                LayerPolicySyntaxValue::PhaseModal => AttachedLayerPolicy::PhaseModal(syntax),
                LayerPolicySyntaxValue::PhaseDebug => AttachedLayerPolicy::PhaseDebug(syntax),
                LayerPolicySyntaxValue::PhaseAgentOverlay => {
                    AttachedLayerPolicy::PhaseAgentOverlay(syntax)
                }
                LayerPolicySyntaxValue::InputIgnore => AttachedLayerPolicy::InputIgnore(syntax),
                LayerPolicySyntaxValue::InputPassThrough => {
                    AttachedLayerPolicy::InputPassThrough(syntax)
                }
                LayerPolicySyntaxValue::InputHitTest => AttachedLayerPolicy::InputHitTest(syntax),
                LayerPolicySyntaxValue::InputModal => AttachedLayerPolicy::InputModal(syntax),
                LayerPolicySyntaxValue::InputCapture => AttachedLayerPolicy::InputCapture(syntax),
                LayerPolicySyntaxValue::HitTestNone => AttachedLayerPolicy::HitTestNone(syntax),
                LayerPolicySyntaxValue::HitTestBounds => AttachedLayerPolicy::HitTestBounds(syntax),
                LayerPolicySyntaxValue::HitTestViewTree => {
                    AttachedLayerPolicy::HitTestViewTree(syntax)
                }
                LayerPolicySyntaxValue::HitTestObjectIdMask => {
                    AttachedLayerPolicy::HitTestObjectIdMask(syntax)
                }
                LayerPolicySyntaxValue::CaptureNone => AttachedLayerPolicy::CaptureNone(syntax),
                LayerPolicySyntaxValue::CaptureColor => AttachedLayerPolicy::CaptureColor(syntax),
                LayerPolicySyntaxValue::CaptureObjectId => {
                    AttachedLayerPolicy::CaptureObjectId(syntax)
                }
                LayerPolicySyntaxValue::CaptureMask => AttachedLayerPolicy::CaptureMask(syntax),
                LayerPolicySyntaxValue::CaptureAll => AttachedLayerPolicy::CaptureAll(syntax),
                LayerPolicySyntaxValue::AccessibilityHidden => {
                    AttachedLayerPolicy::AccessibilityHidden(syntax)
                }
                LayerPolicySyntaxValue::AccessibilityExposed => {
                    AttachedLayerPolicy::AccessibilityExposed(syntax)
                }
                LayerPolicySyntaxValue::AccessibilityContainer => {
                    AttachedLayerPolicy::AccessibilityContainer(syntax)
                }
            })
        }
        PendingLayerPolicy::Authored(_) => {
            Err(SyntaxAccessError::InvalidLayerProjection { id: declaration })
        }
    }
}

const fn policy_belongs_to(member: LayerMemberSyntaxKind, value: LayerPolicySyntaxValue) -> bool {
    matches!(
        (member, value),
        (
            LayerMemberSyntaxKind::Phase,
            LayerPolicySyntaxValue::PhaseBackground
                | LayerPolicySyntaxValue::PhaseWorld
                | LayerPolicySyntaxValue::PhaseCharacters
                | LayerPolicySyntaxValue::PhaseEffects
                | LayerPolicySyntaxValue::PhaseDialogue
                | LayerPolicySyntaxValue::PhaseGameView
                | LayerPolicySyntaxValue::PhaseHtmlView
                | LayerPolicySyntaxValue::PhaseModal
                | LayerPolicySyntaxValue::PhaseDebug
                | LayerPolicySyntaxValue::PhaseAgentOverlay
        ) | (
            LayerMemberSyntaxKind::Input,
            LayerPolicySyntaxValue::InputIgnore
                | LayerPolicySyntaxValue::InputPassThrough
                | LayerPolicySyntaxValue::InputHitTest
                | LayerPolicySyntaxValue::InputModal
                | LayerPolicySyntaxValue::InputCapture
        ) | (
            LayerMemberSyntaxKind::HitTest,
            LayerPolicySyntaxValue::HitTestNone
                | LayerPolicySyntaxValue::HitTestBounds
                | LayerPolicySyntaxValue::HitTestViewTree
                | LayerPolicySyntaxValue::HitTestObjectIdMask
        ) | (
            LayerMemberSyntaxKind::Capture,
            LayerPolicySyntaxValue::CaptureNone
                | LayerPolicySyntaxValue::CaptureColor
                | LayerPolicySyntaxValue::CaptureObjectId
                | LayerPolicySyntaxValue::CaptureMask
                | LayerPolicySyntaxValue::CaptureAll
        ) | (
            LayerMemberSyntaxKind::Accessibility,
            LayerPolicySyntaxValue::AccessibilityHidden
                | LayerPolicySyntaxValue::AccessibilityExposed
                | LayerPolicySyntaxValue::AccessibilityContainer
        )
    )
}

fn validate_range(
    owner: super::SyntaxNodeId,
    actual: arcweft_source::SourceRange,
    expected: arcweft_source::SourceRange,
) -> Result<(), SyntaxAccessError> {
    if actual == expected {
        Ok(())
    } else {
        Err(SyntaxAccessError::InvalidLayerProjection { id: owner })
    }
}

#[cfg(test)]
mod tests;
