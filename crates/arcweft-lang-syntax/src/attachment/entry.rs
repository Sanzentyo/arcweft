//! Typed Entry declaration ownership over the attached grammar tree.

use arcweft_source::{SourceRange, SourceSpan};

use crate::expressions::ExpressionProjection;
use crate::grammar::entry_projection::{
    EntryRoleSyntaxKind, KnownEntryHttpMethod, KnownEntryKind, PendingEntryBodyProjection,
    PendingEntryHttpMethod, PendingEntryId, PendingEntryKind, PendingEntryMemberProjection,
    PendingEntryName, PendingEntryPunctuation, PendingEntryRouteBinding, PendingEntryRouteBindings,
    PendingEntryValueState,
};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole, SyntaxRoleClass};
use crate::id_ref::SyntaxIdRefSyntax;
use crate::name::SyntaxName;

use super::node::{
    AstNode, CloseBraceKind, CloseParenKind, EntryBodyKind, EntryDeclarationItemKind,
    EntryGotoKind, EntryOptionKind, EntryRoleBindingKind, EntryRouteBindingKind, EntryRouteKind,
    ErrorNodeKind, MissingBodyKind, MissingExpressionKind, MissingNameKind, MissingTokenNodeKind,
    NameReferenceKind, OpenBraceKind, OpenParenKind, PathKind,
};
use super::source_file::AttachedPath;
use super::{
    AttachedExpressionNode, AttachedItemPrefix, AttachedTypeRefNode, SyntaxAccessError,
    SyntaxNodeHandle, TypedItemNode,
};

/// Parser-selected Entry adapter kind and its exact syntax owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedEntryKind {
    Game(AstNode<NameReferenceKind>),
    Editor(AstNode<NameReferenceKind>),
    Cli(AstNode<NameReferenceKind>),
    Server(AstNode<NameReferenceKind>),
    Activity(AstNode<NameReferenceKind>),
    Test(AstNode<NameReferenceKind>),
    Bench(AstNode<NameReferenceKind>),
    Agent(AstNode<NameReferenceKind>),
    Custom {
        syntax: AstNode<NameReferenceKind>,
        value: SyntaxName,
    },
    Missing(AstNode<MissingNameKind>),
}

impl AttachedEntryKind {
    pub const fn has_recovery(&self) -> bool {
        matches!(self, Self::Missing(_))
    }

    pub const fn custom_name(&self) -> Option<&SyntaxName> {
        match self {
            Self::Custom { value, .. } => Some(value),
            _ => None,
        }
    }
}

/// Required Entry ID represented by the typed entity-reference expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedEntryId {
    Authored {
        expression: Box<AttachedExpressionNode>,
        reference: SyntaxIdRefSyntax,
        canonical_entry_family: bool,
    },
    Missing(AstNode<MissingExpressionKind>),
}

impl AttachedEntryId {
    pub const fn reference(&self) -> Option<&SyntaxIdRefSyntax> {
        match self {
            Self::Authored { reference, .. } => Some(reference),
            Self::Missing(_) => None,
        }
    }

    pub const fn is_canonical_entry_family(&self) -> bool {
        matches!(
            self,
            Self::Authored {
                canonical_entry_family: true,
                ..
            }
        )
    }

    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Authored {
                expression,
                canonical_entry_family,
                ..
            } => !canonical_entry_family || expression.projection().has_recovery(),
            Self::Missing(_) => true,
        }
    }
}

/// Name selected by the parser without an attachment-time text lookup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedEntryName {
    Authored {
        syntax: AstNode<NameReferenceKind>,
        value: SyntaxName,
    },
    Missing(AstNode<MissingNameKind>),
}

impl AttachedEntryName {
    pub const fn value(&self) -> Option<&SyntaxName> {
        match self {
            Self::Authored { value, .. } => Some(value),
            Self::Missing(_) => None,
        }
    }

    pub const fn has_recovery(&self) -> bool {
        matches!(self, Self::Missing(_))
    }
}

/// Required punctuation represented by its exact authored span or insertion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedEntryPunctuation {
    source: SourceSpan,
    missing: bool,
}

impl AttachedEntryPunctuation {
    pub fn source_span(&self) -> SourceSpan {
        self.source.clone()
    }

    pub const fn is_missing(&self) -> bool {
        self.missing
    }
}

/// Authored typed value, exact missing node, or grammar-owned invalid node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedEntryValue<T> {
    Authored(Box<T>),
    Recovered(Box<T>),
    Missing(SyntaxNodeHandle),
    Invalid(SyntaxNodeHandle),
}

impl<T> AttachedEntryValue<T> {
    pub const fn authored(&self) -> Option<&T> {
        match self {
            Self::Authored(value) => Some(&**value),
            Self::Recovered(_) | Self::Missing(_) | Self::Invalid(_) => None,
        }
    }

    pub const fn value(&self) -> Option<&T> {
        match self {
            Self::Authored(value) | Self::Recovered(value) => Some(&**value),
            Self::Missing(_) | Self::Invalid(_) => None,
        }
    }

    pub const fn has_recovery(&self) -> bool {
        !matches!(self, Self::Authored(_))
    }
}

/// Shared exact syntax of one typed Entry role binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedEntryRoleBinding<T> {
    syntax: AstNode<EntryRoleBindingKind>,
    source_ordinal: u32,
    assignment: AttachedEntryPunctuation,
    value: AttachedEntryValue<T>,
    trailing_recovery: Option<AstNode<ErrorNodeKind>>,
}

impl<T> AttachedEntryRoleBinding<T> {
    pub const fn syntax(&self) -> &AstNode<EntryRoleBindingKind> {
        &self.syntax
    }

    pub const fn source_ordinal(&self) -> u32 {
        self.source_ordinal
    }

    pub const fn assignment(&self) -> &AttachedEntryPunctuation {
        &self.assignment
    }

    pub const fn value(&self) -> &AttachedEntryValue<T> {
        &self.value
    }

    pub const fn has_trailing_recovery(&self) -> bool {
        self.trailing_recovery.is_some()
    }

    pub fn has_recovery(&self) -> bool {
        self.assignment.is_missing()
            || self.value.has_recovery()
            || self.trailing_recovery.is_some()
    }
}

/// Closed supported HTTP method or current-grammar recovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedEntryHttpMethod {
    Get(AstNode<NameReferenceKind>),
    Post(AstNode<NameReferenceKind>),
    Put(AstNode<NameReferenceKind>),
    Patch(AstNode<NameReferenceKind>),
    Delete(AstNode<NameReferenceKind>),
    Head(AstNode<NameReferenceKind>),
    Options(AstNode<NameReferenceKind>),
    Unsupported {
        syntax: AstNode<NameReferenceKind>,
        value: Option<SyntaxName>,
    },
    Missing(AstNode<MissingNameKind>),
}

impl AttachedEntryHttpMethod {
    pub const fn has_recovery(&self) -> bool {
        matches!(self, Self::Unsupported { .. } | Self::Missing(_))
    }
}

/// One ordered `parameter = :capture` route binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedEntryRouteBinding {
    syntax: AstNode<EntryRouteBindingKind>,
    source_ordinal: u16,
    parameter: AttachedEntryName,
    equals: AttachedEntryPunctuation,
    colon: AttachedEntryPunctuation,
    capture: AttachedEntryName,
    trailing_recovery: Option<AstNode<ErrorNodeKind>>,
}

impl AttachedEntryRouteBinding {
    pub const fn syntax(&self) -> &AstNode<EntryRouteBindingKind> {
        &self.syntax
    }

    pub const fn source_ordinal(&self) -> u16 {
        self.source_ordinal
    }

    pub const fn parameter(&self) -> &AttachedEntryName {
        &self.parameter
    }

    pub const fn capture(&self) -> &AttachedEntryName {
        &self.capture
    }

    pub const fn equals(&self) -> &AttachedEntryPunctuation {
        &self.equals
    }

    pub const fn colon(&self) -> &AttachedEntryPunctuation {
        &self.colon
    }

    pub const fn has_trailing_recovery(&self) -> bool {
        self.trailing_recovery.is_some()
    }

    pub fn has_recovery(&self) -> bool {
        self.parameter.has_recovery()
            || self.equals.is_missing()
            || self.colon.is_missing()
            || self.capture.has_recovery()
            || self.trailing_recovery.is_some()
    }
}

/// Optional route binding list and exact delimiter ownership.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedEntryRouteBindings {
    Absent,
    Parenthesized {
        open: AstNode<OpenParenKind>,
        close: AstNode<CloseParenKind>,
        bindings: Box<[AttachedEntryRouteBinding]>,
    },
}

impl AttachedEntryRouteBindings {
    pub fn bindings(&self) -> &[AttachedEntryRouteBinding] {
        match self {
            Self::Absent => &[],
            Self::Parenthesized { bindings, .. } => bindings,
        }
    }

    pub fn has_recovery(&self) -> bool {
        matches!(self, Self::Parenthesized { close, .. } if close.range().is_empty())
            || self
                .bindings()
                .iter()
                .any(AttachedEntryRouteBinding::has_recovery)
    }

    pub fn is_closed(&self) -> bool {
        !matches!(self, Self::Parenthesized { close, .. } if close.range().is_empty())
    }
}

/// Closed Entry body member inventory in exact source order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedEntryMember {
    StateType(AttachedEntryRoleBinding<AttachedTypeRefNode>),
    Initializer(AttachedEntryRoleBinding<AttachedPath>),
    EventType(AttachedEntryRoleBinding<AttachedTypeRefNode>),
    Reducer(AttachedEntryRoleBinding<AttachedPath>),
    Controller(AttachedEntryRoleBinding<AttachedPath>),
    Goto {
        syntax: AstNode<EntryGotoKind>,
        source_ordinal: u32,
        target: AttachedEntryValue<AttachedExpressionNode>,
        trailing_recovery: Option<AstNode<ErrorNodeKind>>,
    },
    Route {
        syntax: AstNode<EntryRouteKind>,
        source_ordinal: u32,
        method: AttachedEntryHttpMethod,
        path: AttachedEntryValue<AttachedExpressionNode>,
        arrow: AttachedEntryPunctuation,
        target: AttachedEntryValue<AttachedExpressionNode>,
        bindings: AttachedEntryRouteBindings,
        trailing_recovery: Option<AstNode<ErrorNodeKind>>,
    },
    Option {
        syntax: AstNode<EntryOptionKind>,
        source_ordinal: u32,
        name: AttachedEntryName,
        assignment: AttachedEntryPunctuation,
        value: AttachedEntryValue<AttachedExpressionNode>,
        trailing_recovery: Option<AstNode<ErrorNodeKind>>,
    },
    Error {
        source_ordinal: u32,
        syntax: AstNode<ErrorNodeKind>,
    },
}

impl AttachedEntryMember {
    pub const fn source_ordinal(&self) -> u32 {
        match self {
            Self::StateType(binding) | Self::EventType(binding) => binding.source_ordinal(),
            Self::Initializer(binding) | Self::Reducer(binding) | Self::Controller(binding) => {
                binding.source_ordinal()
            }
            Self::Goto { source_ordinal, .. }
            | Self::Route { source_ordinal, .. }
            | Self::Option { source_ordinal, .. }
            | Self::Error { source_ordinal, .. } => *source_ordinal,
        }
    }

    pub fn has_recovery(&self) -> bool {
        match self {
            Self::StateType(binding) | Self::EventType(binding) => binding.has_recovery(),
            Self::Initializer(binding) | Self::Reducer(binding) | Self::Controller(binding) => {
                binding.has_recovery()
                    || binding
                        .value()
                        .value()
                        .is_some_and(AttachedPath::has_recovery)
            }
            Self::Goto {
                target,
                trailing_recovery,
                ..
            } => expression_value_has_recovery(target) || trailing_recovery.is_some(),
            Self::Route {
                method,
                path,
                arrow,
                target,
                bindings,
                trailing_recovery,
                ..
            } => {
                method.has_recovery()
                    || expression_value_has_recovery(path)
                    || arrow.is_missing()
                    || expression_value_has_recovery(target)
                    || bindings.has_recovery()
                    || trailing_recovery.is_some()
            }
            Self::Option {
                name,
                assignment,
                value,
                trailing_recovery,
                ..
            } => {
                name.has_recovery()
                    || assignment.is_missing()
                    || expression_value_has_recovery(value)
                    || trailing_recovery.is_some()
            }
            Self::Error { .. } => true,
        }
    }
}

/// Missing or authored Entry body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedEntryBody {
    Missing(AstNode<MissingBodyKind>),
    Braced {
        syntax: AstNode<EntryBodyKind>,
        open: AstNode<OpenBraceKind>,
        close: AstNode<CloseBraceKind>,
        members: Box<[AttachedEntryMember]>,
    },
}

impl AttachedEntryBody {
    pub fn members(&self) -> &[AttachedEntryMember] {
        match self {
            Self::Missing(_) => &[],
            Self::Braced { members, .. } => members,
        }
    }

    pub fn has_recovery(&self) -> bool {
        matches!(self, Self::Missing(_))
            || matches!(self, Self::Braced { close, .. } if close.range().is_empty())
            || self.members().iter().any(AttachedEntryMember::has_recovery)
    }

    pub fn is_closed(&self) -> bool {
        matches!(self, Self::Braced { close, .. } if !close.range().is_empty())
    }
}

/// One source-bound Entry declaration and its parser-owned schema projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedEntryDeclaration {
    syntax: AstNode<EntryDeclarationItemKind>,
    prefix: AttachedItemPrefix,
    kind: AttachedEntryKind,
    id: AttachedEntryId,
    body: AttachedEntryBody,
    trailing_header_recovery: Option<AstNode<ErrorNodeKind>>,
}

impl AttachedEntryDeclaration {
    pub const fn syntax(&self) -> &AstNode<EntryDeclarationItemKind> {
        &self.syntax
    }

    pub const fn prefix(&self) -> &AttachedItemPrefix {
        &self.prefix
    }

    pub const fn kind(&self) -> &AttachedEntryKind {
        &self.kind
    }

    pub const fn id(&self) -> &AttachedEntryId {
        &self.id
    }

    pub const fn body(&self) -> &AttachedEntryBody {
        &self.body
    }

    pub const fn has_header_trailing_recovery(&self) -> bool {
        self.trailing_header_recovery.is_some()
    }

    pub fn has_recovery(&self) -> bool {
        self.kind.has_recovery()
            || self.id.has_recovery()
            || self.body.has_recovery()
            || self.trailing_header_recovery.is_some()
    }
}

impl AstNode<EntryDeclarationItemKind> {
    /// Binds the sole parser-owned Entry projection without source rediscovery.
    pub fn semantics(&self) -> Result<AttachedEntryDeclaration, SyntaxAccessError> {
        let pending = self
            .syntax()
            .entry_projection()
            .cloned()
            .ok_or(SyntaxAccessError::MissingEntryProjection { id: self.id() })?;
        let kind = attach_kind(self, &pending.kind)?;
        let id = attach_id(self, pending.id)?;
        let body = attach_body(self, &pending.body)?;
        let trailing_header_recovery = attach_optional_recovery(
            self,
            SyntaxRole::Recovery(0),
            pending.trailing_header_recovery,
        )?;
        Ok(AttachedEntryDeclaration {
            syntax: self.clone(),
            prefix: TypedItemNode::Entry(self.clone()).attached_prefix()?,
            kind,
            id,
            body,
            trailing_header_recovery,
        })
    }
}

fn attach_kind(
    owner: &AstNode<EntryDeclarationItemKind>,
    pending: &PendingEntryKind,
) -> Result<AttachedEntryKind, SyntaxAccessError> {
    match pending {
        PendingEntryKind::Known { value, source } => {
            let syntax = owner.required_exact_child::<NameReferenceKind>(SyntaxRole::Type)?;
            validate_range(owner.id(), syntax.range(), *source)?;
            Ok(match value {
                KnownEntryKind::Game => AttachedEntryKind::Game(syntax),
                KnownEntryKind::Editor => AttachedEntryKind::Editor(syntax),
                KnownEntryKind::Cli => AttachedEntryKind::Cli(syntax),
                KnownEntryKind::Server => AttachedEntryKind::Server(syntax),
                KnownEntryKind::Activity => AttachedEntryKind::Activity(syntax),
                KnownEntryKind::Test => AttachedEntryKind::Test(syntax),
                KnownEntryKind::Bench => AttachedEntryKind::Bench(syntax),
                KnownEntryKind::Agent => AttachedEntryKind::Agent(syntax),
            })
        }
        PendingEntryKind::Custom { value, source } => {
            let syntax = owner.required_exact_child::<NameReferenceKind>(SyntaxRole::Type)?;
            validate_range(owner.id(), syntax.range(), *source)?;
            Ok(AttachedEntryKind::Custom {
                syntax,
                value: value.clone(),
            })
        }
        PendingEntryKind::Missing { insertion } => {
            let syntax = owner.required_exact_child::<MissingNameKind>(SyntaxRole::Type)?;
            validate_range(owner.id(), syntax.range(), *insertion)?;
            Ok(AttachedEntryKind::Missing(syntax))
        }
    }
}

fn attach_id(
    owner: &AstNode<EntryDeclarationItemKind>,
    pending: PendingEntryId,
) -> Result<AttachedEntryId, SyntaxAccessError> {
    let syntax = owner
        .syntax()
        .optional_unique_child(SyntaxRole::Reference(0))?
        .ok_or(SyntaxAccessError::InvalidEntryProjection { id: owner.id() })?;
    match pending {
        PendingEntryId::Authored {
            source,
            canonical_entry_family,
        } if syntax.kind() == SyntaxKind::EntityReferenceExpression => {
            validate_range(owner.id(), syntax.range(), source)?;
            let expression = AttachedExpressionNode::from_syntax(syntax)?;
            let ExpressionProjection::EntityReference(reference) = expression.projection() else {
                return Err(SyntaxAccessError::InvalidEntryProjection { id: owner.id() });
            };
            let reference = reference.clone();
            Ok(AttachedEntryId::Authored {
                expression: Box::new(expression),
                reference,
                canonical_entry_family,
            })
        }
        PendingEntryId::Missing { insertion } if syntax.kind() == SyntaxKind::MissingExpression => {
            validate_range(owner.id(), syntax.range(), insertion)?;
            Ok(AttachedEntryId::Missing(syntax.cast()?))
        }
        _ => Err(SyntaxAccessError::InvalidEntryProjection { id: owner.id() }),
    }
}

fn attach_body(
    owner: &AstNode<EntryDeclarationItemKind>,
    pending: &PendingEntryBodyProjection,
) -> Result<AttachedEntryBody, SyntaxAccessError> {
    let body = owner
        .syntax()
        .optional_unique_child(SyntaxRole::Body)?
        .ok_or(SyntaxAccessError::InvalidEntryProjection { id: owner.id() })?;
    match pending {
        PendingEntryBodyProjection::Missing if body.kind() == SyntaxKind::MissingBody => {
            Ok(AttachedEntryBody::Missing(body.cast()?))
        }
        PendingEntryBodyProjection::Braced { members, closed }
            if body.kind() == SyntaxKind::EntryBody =>
        {
            let syntax = body.cast::<EntryBodyKind>()?;
            let open = syntax.required_exact_child::<OpenBraceKind>(SyntaxRole::OpenDelimiter)?;
            let close =
                syntax.required_exact_child::<CloseBraceKind>(SyntaxRole::CloseDelimiter)?;
            if close.range().is_empty() == *closed {
                return Err(SyntaxAccessError::InvalidEntryProjection { id: owner.id() });
            }
            let children = syntax.syntax().ordered_children(SyntaxRoleClass::Element)?;
            if children.len() != members.len() {
                return Err(SyntaxAccessError::InvalidEntryProjection { id: owner.id() });
            }
            let members = children
                .into_iter()
                .zip(members)
                .map(|(syntax, pending)| attach_member(&syntax, pending, owner.id()))
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice();
            Ok(AttachedEntryBody::Braced {
                syntax,
                open,
                close,
                members,
            })
        }
        _ => Err(SyntaxAccessError::InvalidEntryProjection { id: owner.id() }),
    }
}

fn attach_member(
    syntax: &SyntaxNodeHandle,
    pending: &PendingEntryMemberProjection,
    declaration: super::SyntaxNodeId,
) -> Result<AttachedEntryMember, SyntaxAccessError> {
    let actual_ordinal = syntax.role().ordinal();
    if actual_ordinal != Some(pending.source_ordinal()) {
        return Err(SyntaxAccessError::InvalidEntryProjection { id: declaration });
    }
    match pending {
        PendingEntryMemberProjection::Role {
            source_ordinal,
            role,
            assignment,
            value,
            trailing_recovery,
        } if syntax.kind() == SyntaxKind::EntryRoleBinding => attach_role_binding(
            syntax.cast()?,
            *source_ordinal,
            *role,
            *assignment,
            *value,
            *trailing_recovery,
            declaration,
        ),
        PendingEntryMemberProjection::Goto {
            source_ordinal,
            target,
            trailing_recovery,
        } if syntax.kind() == SyntaxKind::EntryGoto => attach_goto_member(
            syntax.cast()?,
            *source_ordinal,
            *target,
            *trailing_recovery,
            declaration,
        ),
        PendingEntryMemberProjection::Route { .. } if syntax.kind() == SyntaxKind::EntryRoute => {
            attach_route_member(syntax.cast()?, pending, declaration)
        }
        PendingEntryMemberProjection::Option {
            source_ordinal,
            name,
            assignment,
            value,
            trailing_recovery,
        } if syntax.kind() == SyntaxKind::EntryOption => attach_option_member(
            syntax.cast()?,
            *source_ordinal,
            name,
            *assignment,
            *value,
            *trailing_recovery,
            declaration,
        ),
        PendingEntryMemberProjection::Recovery { source_ordinal }
            if syntax.kind() == SyntaxKind::ErrorNode =>
        {
            Ok(AttachedEntryMember::Error {
                source_ordinal: *source_ordinal,
                syntax: syntax.cast()?,
            })
        }
        _ => Err(SyntaxAccessError::InvalidEntryProjection { id: declaration }),
    }
}

fn attach_goto_member(
    syntax: AstNode<EntryGotoKind>,
    source_ordinal: u32,
    target: PendingEntryValueState,
    trailing_recovery: bool,
    declaration: super::SyntaxNodeId,
) -> Result<AttachedEntryMember, SyntaxAccessError> {
    Ok(AttachedEntryMember::Goto {
        target: attach_expression_value(&syntax, SyntaxRole::Target, target, declaration)?,
        trailing_recovery: attach_optional_recovery(
            &syntax,
            SyntaxRole::Recovery(0),
            trailing_recovery,
        )?,
        syntax,
        source_ordinal,
    })
}

fn attach_route_member(
    syntax: AstNode<EntryRouteKind>,
    pending: &PendingEntryMemberProjection,
    declaration: super::SyntaxNodeId,
) -> Result<AttachedEntryMember, SyntaxAccessError> {
    let PendingEntryMemberProjection::Route {
        source_ordinal,
        method,
        path,
        arrow,
        target,
        bindings,
        trailing_recovery,
    } = pending
    else {
        return Err(SyntaxAccessError::InvalidEntryProjection { id: declaration });
    };
    Ok(AttachedEntryMember::Route {
        method: attach_http_method(&syntax, method, declaration)?,
        path: attach_expression_value(&syntax, SyntaxRole::Operand, *path, declaration)?,
        arrow: attach_punctuation(&syntax, *arrow, SyntaxRole::Recovery(0), declaration)?,
        target: attach_expression_value(&syntax, SyntaxRole::Target, *target, declaration)?,
        bindings: attach_route_bindings(&syntax, bindings, declaration)?,
        trailing_recovery: attach_optional_recovery(
            &syntax,
            SyntaxRole::Recovery(1),
            *trailing_recovery,
        )?,
        syntax,
        source_ordinal: *source_ordinal,
    })
}

fn attach_option_member(
    syntax: AstNode<EntryOptionKind>,
    source_ordinal: u32,
    name: &PendingEntryName,
    assignment: PendingEntryPunctuation,
    value: PendingEntryValueState,
    trailing_recovery: bool,
    declaration: super::SyntaxNodeId,
) -> Result<AttachedEntryMember, SyntaxAccessError> {
    Ok(AttachedEntryMember::Option {
        name: attach_name(&syntax, SyntaxRole::Name, name, declaration)?,
        assignment: attach_punctuation(&syntax, assignment, SyntaxRole::Recovery(1), declaration)?,
        value: attach_expression_value(&syntax, SyntaxRole::Initializer, value, declaration)?,
        trailing_recovery: attach_optional_recovery(
            &syntax,
            SyntaxRole::Recovery(0),
            trailing_recovery,
        )?,
        syntax,
        source_ordinal,
    })
}

fn attach_role_binding(
    syntax: AstNode<EntryRoleBindingKind>,
    source_ordinal: u32,
    role: EntryRoleSyntaxKind,
    assignment: PendingEntryPunctuation,
    value: PendingEntryValueState,
    trailing_recovery: bool,
    declaration: super::SyntaxNodeId,
) -> Result<AttachedEntryMember, SyntaxAccessError> {
    let _role_name = syntax.required_exact_child::<NameReferenceKind>(SyntaxRole::Name)?;
    let assignment = attach_punctuation(&syntax, assignment, SyntaxRole::Recovery(0), declaration)?;
    let trailing_recovery =
        attach_optional_recovery(&syntax, SyntaxRole::Recovery(1), trailing_recovery)?;
    if role.expects_type() {
        let binding = AttachedEntryRoleBinding {
            value: attach_type_value(&syntax, value, declaration)?,
            syntax,
            source_ordinal,
            assignment,
            trailing_recovery,
        };
        return Ok(match role {
            EntryRoleSyntaxKind::State => AttachedEntryMember::StateType(binding),
            EntryRoleSyntaxKind::Event => AttachedEntryMember::EventType(binding),
            EntryRoleSyntaxKind::Initializer
            | EntryRoleSyntaxKind::Reducer
            | EntryRoleSyntaxKind::Controller => unreachable!("role family was checked"),
        });
    }
    let binding = AttachedEntryRoleBinding {
        value: attach_path_value(&syntax, value, declaration)?,
        syntax,
        source_ordinal,
        assignment,
        trailing_recovery,
    };
    Ok(match role {
        EntryRoleSyntaxKind::Initializer => AttachedEntryMember::Initializer(binding),
        EntryRoleSyntaxKind::Reducer => AttachedEntryMember::Reducer(binding),
        EntryRoleSyntaxKind::Controller => AttachedEntryMember::Controller(binding),
        EntryRoleSyntaxKind::State | EntryRoleSyntaxKind::Event => {
            unreachable!("role family was checked")
        }
    })
}

fn attach_http_method(
    owner: &AstNode<EntryRouteKind>,
    pending: &PendingEntryHttpMethod,
    declaration: super::SyntaxNodeId,
) -> Result<AttachedEntryHttpMethod, SyntaxAccessError> {
    match pending {
        PendingEntryHttpMethod::Known { value, source } => {
            let syntax = owner.required_exact_child::<NameReferenceKind>(SyntaxRole::Name)?;
            validate_range(declaration, syntax.range(), *source)?;
            Ok(match value {
                KnownEntryHttpMethod::Get => AttachedEntryHttpMethod::Get(syntax),
                KnownEntryHttpMethod::Post => AttachedEntryHttpMethod::Post(syntax),
                KnownEntryHttpMethod::Put => AttachedEntryHttpMethod::Put(syntax),
                KnownEntryHttpMethod::Patch => AttachedEntryHttpMethod::Patch(syntax),
                KnownEntryHttpMethod::Delete => AttachedEntryHttpMethod::Delete(syntax),
                KnownEntryHttpMethod::Head => AttachedEntryHttpMethod::Head(syntax),
                KnownEntryHttpMethod::Options => AttachedEntryHttpMethod::Options(syntax),
            })
        }
        PendingEntryHttpMethod::Unsupported { value, source } => {
            let syntax = owner.required_exact_child::<NameReferenceKind>(SyntaxRole::Name)?;
            validate_range(declaration, syntax.range(), *source)?;
            Ok(AttachedEntryHttpMethod::Unsupported {
                syntax,
                value: value.clone().ok(),
            })
        }
        PendingEntryHttpMethod::Missing { insertion } => {
            let syntax = owner.required_exact_child::<MissingNameKind>(SyntaxRole::Name)?;
            validate_range(declaration, syntax.range(), *insertion)?;
            Ok(AttachedEntryHttpMethod::Missing(syntax))
        }
    }
}

fn attach_route_bindings(
    owner: &AstNode<EntryRouteKind>,
    pending: &PendingEntryRouteBindings,
    declaration: super::SyntaxNodeId,
) -> Result<AttachedEntryRouteBindings, SyntaxAccessError> {
    let children =
        owner.ordered_exact_children::<EntryRouteBindingKind>(SyntaxRoleClass::Argument)?;
    match pending {
        PendingEntryRouteBindings::Absent if children.is_empty() => {
            Ok(AttachedEntryRouteBindings::Absent)
        }
        PendingEntryRouteBindings::Parenthesized { bindings, closed }
            if children.len() == bindings.len() =>
        {
            let open = owner.required_exact_child::<OpenParenKind>(SyntaxRole::OpenDelimiter)?;
            let close = owner.required_exact_child::<CloseParenKind>(SyntaxRole::CloseDelimiter)?;
            if close.range().is_empty() == *closed {
                return Err(SyntaxAccessError::InvalidEntryProjection { id: declaration });
            }
            let bindings = children
                .into_iter()
                .zip(bindings)
                .map(|(syntax, pending)| attach_route_binding(syntax, pending, declaration))
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice();
            Ok(AttachedEntryRouteBindings::Parenthesized {
                open,
                close,
                bindings,
            })
        }
        _ => Err(SyntaxAccessError::InvalidEntryProjection { id: declaration }),
    }
}

fn attach_route_binding(
    syntax: AstNode<EntryRouteBindingKind>,
    pending: &PendingEntryRouteBinding,
    declaration: super::SyntaxNodeId,
) -> Result<AttachedEntryRouteBinding, SyntaxAccessError> {
    if syntax.syntax().role().ordinal() != Some(u32::from(pending.source_ordinal)) {
        return Err(SyntaxAccessError::InvalidEntryProjection { id: declaration });
    }
    Ok(AttachedEntryRouteBinding {
        parameter: attach_name(&syntax, SyntaxRole::Name, &pending.parameter, declaration)?,
        equals: attach_punctuation(
            &syntax,
            pending.equals,
            SyntaxRole::Recovery(0),
            declaration,
        )?,
        colon: attach_punctuation(&syntax, pending.colon, SyntaxRole::Recovery(1), declaration)?,
        capture: attach_name(
            &syntax,
            SyntaxRole::Initializer,
            &pending.capture,
            declaration,
        )?,
        trailing_recovery: attach_optional_recovery(
            &syntax,
            SyntaxRole::Recovery(2),
            pending.trailing_recovery,
        )?,
        syntax,
        source_ordinal: pending.source_ordinal,
    })
}

fn attach_name<K: super::AstKind>(
    owner: &AstNode<K>,
    role: SyntaxRole,
    pending: &PendingEntryName,
    declaration: super::SyntaxNodeId,
) -> Result<AttachedEntryName, SyntaxAccessError> {
    match pending {
        PendingEntryName::Authored { value, source } => {
            let syntax = owner.required_exact_child::<NameReferenceKind>(role)?;
            validate_range(declaration, syntax.range(), *source)?;
            Ok(AttachedEntryName::Authored {
                syntax,
                value: value
                    .clone()
                    .map_err(|_| SyntaxAccessError::InvalidEntryProjection { id: declaration })?,
            })
        }
        PendingEntryName::Missing { insertion } => {
            let syntax = owner.required_exact_child::<MissingNameKind>(role)?;
            validate_range(declaration, syntax.range(), *insertion)?;
            Ok(AttachedEntryName::Missing(syntax))
        }
    }
}

fn attach_punctuation<K: super::AstKind>(
    owner: &AstNode<K>,
    pending: PendingEntryPunctuation,
    missing_role: SyntaxRole,
    declaration: super::SyntaxNodeId,
) -> Result<AttachedEntryPunctuation, SyntaxAccessError> {
    let (range, missing) = match pending {
        PendingEntryPunctuation::Authored(range) => (range, false),
        PendingEntryPunctuation::Missing(range) => {
            let syntax = owner.required_exact_child::<MissingTokenNodeKind>(missing_role)?;
            validate_range(declaration, syntax.range(), range)?;
            (range, true)
        }
    };
    if range.start() < owner.range().start() || range.end() > owner.range().end() {
        return Err(SyntaxAccessError::InvalidEntryProjection { id: declaration });
    }
    Ok(AttachedEntryPunctuation {
        source: owner.syntax().source_span_for_range(range),
        missing,
    })
}

fn attach_type_value<K: super::AstKind>(
    owner: &AstNode<K>,
    pending: PendingEntryValueState,
    declaration: super::SyntaxNodeId,
) -> Result<AttachedEntryValue<AttachedTypeRefNode>, SyntaxAccessError> {
    let syntax = owner
        .syntax()
        .optional_unique_child(SyntaxRole::Type)?
        .ok_or(SyntaxAccessError::InvalidEntryProjection { id: declaration })?;
    match pending {
        PendingEntryValueState::Authored if syntax.kind().is_type_node() => Ok(
            AttachedEntryValue::Authored(Box::new(AttachedTypeRefNode::from_syntax(syntax)?)),
        ),
        PendingEntryValueState::Missing if syntax.kind() == SyntaxKind::MissingType => Ok(
            AttachedEntryValue::Recovered(Box::new(AttachedTypeRefNode::from_syntax(syntax)?)),
        ),
        PendingEntryValueState::Invalid if syntax.kind().is_type_node() => Ok(
            AttachedEntryValue::Recovered(Box::new(AttachedTypeRefNode::from_syntax(syntax)?)),
        ),
        PendingEntryValueState::Invalid => Ok(AttachedEntryValue::Invalid(syntax)),
        _ => Err(SyntaxAccessError::InvalidEntryProjection { id: declaration }),
    }
}

fn attach_path_value<K: super::AstKind>(
    owner: &AstNode<K>,
    pending: PendingEntryValueState,
    declaration: super::SyntaxNodeId,
) -> Result<AttachedEntryValue<AttachedPath>, SyntaxAccessError> {
    let syntax = owner
        .syntax()
        .optional_unique_child(SyntaxRole::Initializer)?
        .ok_or(SyntaxAccessError::InvalidEntryProjection { id: declaration })?;
    match pending {
        PendingEntryValueState::Authored if syntax.kind() == SyntaxKind::Path => {
            Ok(AttachedEntryValue::Authored(Box::new(
                AttachedPath::from_syntax(syntax.cast::<PathKind>()?)?,
            )))
        }
        PendingEntryValueState::Missing if syntax.kind() == SyntaxKind::MissingName => {
            Ok(AttachedEntryValue::Missing(syntax))
        }
        PendingEntryValueState::Invalid if syntax.kind() == SyntaxKind::ErrorNode => {
            Ok(AttachedEntryValue::Invalid(syntax))
        }
        _ => Err(SyntaxAccessError::InvalidEntryProjection { id: declaration }),
    }
}

fn attach_expression_value<K: super::AstKind>(
    owner: &AstNode<K>,
    role: SyntaxRole,
    pending: PendingEntryValueState,
    declaration: super::SyntaxNodeId,
) -> Result<AttachedEntryValue<AttachedExpressionNode>, SyntaxAccessError> {
    let syntax = owner
        .syntax()
        .optional_unique_child(role)?
        .ok_or(SyntaxAccessError::InvalidEntryProjection { id: declaration })?;
    match pending {
        PendingEntryValueState::Authored if syntax.kind().is_expression() => Ok(
            AttachedEntryValue::Authored(Box::new(AttachedExpressionNode::from_syntax(syntax)?)),
        ),
        PendingEntryValueState::Missing if syntax.kind() == SyntaxKind::MissingExpression => {
            Ok(AttachedEntryValue::Missing(syntax))
        }
        PendingEntryValueState::Invalid => Ok(AttachedEntryValue::Invalid(syntax)),
        _ => Err(SyntaxAccessError::InvalidEntryProjection { id: declaration }),
    }
}

fn expression_value_has_recovery(value: &AttachedEntryValue<AttachedExpressionNode>) -> bool {
    value.has_recovery()
        || value
            .value()
            .is_some_and(|expression| expression.projection().has_recovery())
}

fn attach_optional_recovery<K: super::AstKind>(
    owner: &AstNode<K>,
    role: SyntaxRole,
    expected: bool,
) -> Result<Option<AstNode<ErrorNodeKind>>, SyntaxAccessError> {
    let recovery = owner.optional_exact_child::<ErrorNodeKind>(role)?;
    if recovery.is_some() != expected {
        return Err(SyntaxAccessError::InvalidEntryProjection { id: owner.id() });
    }
    Ok(recovery)
}

fn validate_range(
    owner: super::SyntaxNodeId,
    actual: SourceRange,
    expected: SourceRange,
) -> Result<(), SyntaxAccessError> {
    if actual != expected {
        return Err(SyntaxAccessError::InvalidEntryProjection { id: owner });
    }
    Ok(())
}

#[cfg(test)]
mod tests;
