//! Attached ownership for external capability declarations and their members.

use crate::grammar::kinds::{SyntaxKind, SyntaxRole, SyntaxRoleClass};

use super::family::{ExpressionFamily, TypeFamily};
use super::node::{
    AstNode, CloseBraceKind, ErrorItemKind, ErrorNodeKind, ExternCapabilityItemKind,
    FunctionItemKind, MissingBodyKind, OpenBraceKind, ReturnTypeKind, TypeAliasItemKind,
};
use super::nominal::{optional_generics, required_name};
use super::{
    AttachedCallableReturn, AttachedExpressionNode, AttachedFixedParameterGroup,
    AttachedGenericParameterGroup, AttachedItemPrefix, AttachedRequiredName, AttachedTypeFamily,
    AttachedTypeRefNode, SyntaxAccessError, SyntaxNodeHandle, TypedItemNode,
};

/// One source-ordered external-effect expression list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedCapabilityEffects {
    open: AstNode<OpenBraceKind>,
    close: AstNode<CloseBraceKind>,
    expressions: Box<[AttachedExpressionNode]>,
}

impl AttachedCapabilityEffects {
    pub const fn open(&self) -> &AstNode<OpenBraceKind> {
        &self.open
    }

    pub const fn close(&self) -> &AstNode<CloseBraceKind> {
        &self.close
    }

    pub const fn expressions(&self) -> &[AttachedExpressionNode] {
        &self.expressions
    }

    pub fn has_recovery(&self) -> bool {
        self.open.range().is_empty()
            || self.close.range().is_empty()
            || self
                .expressions
                .iter()
                .any(|expression| expression.projection().has_recovery())
    }
}

/// One associated-type declaration owned inline by an external capability.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedCapabilityAssociatedType {
    syntax: AstNode<TypeAliasItemKind>,
    source_ordinal: u16,
    prefix: AttachedItemPrefix,
    name: AttachedRequiredName,
    generics: Option<AttachedGenericParameterGroup>,
    value: Option<AttachedTypeRefNode>,
    trailing_recovery: Box<[AstNode<ErrorNodeKind>]>,
}

impl AttachedCapabilityAssociatedType {
    pub const fn syntax(&self) -> &AstNode<TypeAliasItemKind> {
        &self.syntax
    }

    pub const fn source_ordinal(&self) -> u16 {
        self.source_ordinal
    }

    pub const fn prefix(&self) -> &AttachedItemPrefix {
        &self.prefix
    }

    pub const fn name(&self) -> &AttachedRequiredName {
        &self.name
    }

    pub const fn generics(&self) -> Option<&AttachedGenericParameterGroup> {
        self.generics.as_ref()
    }

    pub const fn value(&self) -> Option<&AttachedTypeRefNode> {
        self.value.as_ref()
    }

    pub const fn trailing_recovery(&self) -> &[AstNode<ErrorNodeKind>] {
        &self.trailing_recovery
    }

    pub fn has_recovery(&self) -> bool {
        self.name.is_missing()
            || self
                .value
                .as_ref()
                .is_some_and(|value| value.family() == AttachedTypeFamily::Recovery)
            || !self.trailing_recovery.is_empty()
    }
}

/// One bodyless callable declaration owned by an external capability.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedCapabilityFunction {
    syntax: AstNode<FunctionItemKind>,
    source_ordinal: u16,
    prefix: AttachedItemPrefix,
    name: AttachedRequiredName,
    generics: Option<AttachedGenericParameterGroup>,
    parameter_groups: Box<[AttachedFixedParameterGroup]>,
    authored_return: Option<AttachedCallableReturn>,
    effects: Option<AttachedCapabilityEffects>,
    trailing_recovery: Box<[AstNode<ErrorNodeKind>]>,
}

impl AttachedCapabilityFunction {
    pub const fn syntax(&self) -> &AstNode<FunctionItemKind> {
        &self.syntax
    }

    pub const fn source_ordinal(&self) -> u16 {
        self.source_ordinal
    }

    pub const fn prefix(&self) -> &AttachedItemPrefix {
        &self.prefix
    }

    pub const fn name(&self) -> &AttachedRequiredName {
        &self.name
    }

    pub const fn generics(&self) -> Option<&AttachedGenericParameterGroup> {
        self.generics.as_ref()
    }

    pub const fn parameter_groups(&self) -> &[AttachedFixedParameterGroup] {
        &self.parameter_groups
    }

    pub fn parameters(&self) -> impl Iterator<Item = &super::AttachedCallableParameter> {
        self.parameter_groups
            .iter()
            .flat_map(AttachedFixedParameterGroup::parameters)
    }

    /// Whether positional-rest structure violates the shared callable grammar.
    ///
    /// The attached parameter kind and default expression remain available for
    /// exact recovery even when this returns `true`.
    pub fn has_parameter_shape_recovery(&self) -> bool {
        super::callable::parameter_shape_has_recovery(&self.parameter_groups)
    }

    pub const fn authored_return(&self) -> Option<&AttachedCallableReturn> {
        self.authored_return.as_ref()
    }

    pub const fn effects(&self) -> Option<&AttachedCapabilityEffects> {
        self.effects.as_ref()
    }

    pub const fn trailing_recovery(&self) -> &[AstNode<ErrorNodeKind>] {
        &self.trailing_recovery
    }

    pub fn has_recovery(&self) -> bool {
        self.name.is_missing()
            || self.has_parameter_shape_recovery()
            || self
                .parameter_groups
                .iter()
                .any(AttachedFixedParameterGroup::has_recovery)
            || self
                .authored_return
                .as_ref()
                .is_some_and(AttachedCallableReturn::has_recovery)
            || self
                .effects
                .as_ref()
                .is_some_and(AttachedCapabilityEffects::has_recovery)
            || !self.trailing_recovery.is_empty()
    }
}

/// Closed external-capability member family in source order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedCapabilityMember {
    AssociatedType(AttachedCapabilityAssociatedType),
    Function(AttachedCapabilityFunction),
    Error {
        syntax: AstNode<ErrorItemKind>,
        source_ordinal: u16,
    },
}

impl AttachedCapabilityMember {
    pub const fn source_ordinal(&self) -> u16 {
        match self {
            Self::AssociatedType(member) => member.source_ordinal(),
            Self::Function(member) => member.source_ordinal(),
            Self::Error { source_ordinal, .. } => *source_ordinal,
        }
    }

    pub fn syntax(&self) -> SyntaxNodeHandle {
        match self {
            Self::AssociatedType(member) => member.syntax().syntax(),
            Self::Function(member) => member.syntax().syntax(),
            Self::Error { syntax, .. } => syntax.syntax(),
        }
    }

    pub fn has_recovery(&self) -> bool {
        match self {
            Self::AssociatedType(member) => member.has_recovery(),
            Self::Function(member) => member.has_recovery(),
            Self::Error { .. } => true,
        }
    }
}

/// Missing or braced external-capability body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedExternCapabilityBody {
    Missing(AstNode<MissingBodyKind>),
    Braced {
        open: AstNode<OpenBraceKind>,
        close: AstNode<CloseBraceKind>,
        members: Box<[AttachedCapabilityMember]>,
    },
}

impl AttachedExternCapabilityBody {
    pub const fn members(&self) -> &[AttachedCapabilityMember] {
        match self {
            Self::Missing(_) => &[],
            Self::Braced { members, .. } => members,
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
            || self
                .members()
                .iter()
                .any(AttachedCapabilityMember::has_recovery)
    }
}

/// Complete attached external-capability declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedExternCapabilityDeclaration {
    syntax: AstNode<ExternCapabilityItemKind>,
    prefix: AttachedItemPrefix,
    name: AttachedRequiredName,
    body: AttachedExternCapabilityBody,
    header_recovery: Box<[AstNode<ErrorNodeKind>]>,
}

impl AttachedExternCapabilityDeclaration {
    pub const fn syntax(&self) -> &AstNode<ExternCapabilityItemKind> {
        &self.syntax
    }

    pub const fn prefix(&self) -> &AttachedItemPrefix {
        &self.prefix
    }

    pub const fn name(&self) -> &AttachedRequiredName {
        &self.name
    }

    pub const fn body(&self) -> &AttachedExternCapabilityBody {
        &self.body
    }

    pub const fn header_recovery(&self) -> &[AstNode<ErrorNodeKind>] {
        &self.header_recovery
    }

    pub fn has_recovery(&self) -> bool {
        self.name.is_missing() || self.body.has_recovery() || !self.header_recovery.is_empty()
    }
}

impl AstNode<ExternCapabilityItemKind> {
    /// Binds the capability declaration and all nested members to one snapshot.
    pub fn semantics(&self) -> Result<AttachedExternCapabilityDeclaration, SyntaxAccessError> {
        Ok(AttachedExternCapabilityDeclaration {
            syntax: self.clone(),
            prefix: TypedItemNode::ExternCapability(self.clone()).attached_prefix()?,
            name: required_name(&self.syntax(), false)?,
            body: attach_body(self)?,
            header_recovery: self
                .ordered_exact_children::<ErrorNodeKind>(SyntaxRoleClass::Recovery)?
                .into_boxed_slice(),
        })
    }
}

fn attach_body(
    owner: &AstNode<ExternCapabilityItemKind>,
) -> Result<AttachedExternCapabilityBody, SyntaxAccessError> {
    let missing = owner.optional_exact_child::<MissingBodyKind>(SyntaxRole::Body)?;
    let open = owner.optional_exact_child::<OpenBraceKind>(SyntaxRole::OpenDelimiter)?;
    let close = owner.optional_exact_child::<CloseBraceKind>(SyntaxRole::CloseDelimiter)?;
    let member_nodes = owner.syntax().ordered_children(SyntaxRoleClass::Element)?;
    match (missing, open, close) {
        (Some(missing), None, None) if member_nodes.is_empty() => {
            Ok(AttachedExternCapabilityBody::Missing(missing))
        }
        (None, Some(open), Some(close)) => {
            let members = member_nodes
                .into_iter()
                .enumerate()
                .map(|(ordinal, syntax)| {
                    let source_ordinal = u16::try_from(ordinal)
                        .map_err(|_| SyntaxAccessError::InvalidItemProjection { id: owner.id() })?;
                    attach_member(&syntax, source_ordinal)
                })
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice();
            Ok(AttachedExternCapabilityBody::Braced {
                open,
                close,
                members,
            })
        }
        _ => Err(SyntaxAccessError::InvalidItemProjection { id: owner.id() }),
    }
}

fn attach_member(
    syntax: &SyntaxNodeHandle,
    source_ordinal: u16,
) -> Result<AttachedCapabilityMember, SyntaxAccessError> {
    if syntax.role() != SyntaxRole::Element(u32::from(source_ordinal)) {
        return Err(SyntaxAccessError::InvalidItemProjection { id: syntax.id() });
    }
    match syntax.kind() {
        SyntaxKind::TypeAliasItem => {
            let syntax = syntax.cast::<TypeAliasItemKind>()?;
            Ok(AttachedCapabilityMember::AssociatedType(
                AttachedCapabilityAssociatedType {
                    prefix: TypedItemNode::TypeAlias(syntax.clone()).attached_prefix()?,
                    name: required_name(&syntax.syntax(), false)?,
                    generics: optional_generics(&syntax.syntax())?,
                    value: syntax
                        .optional_family_child::<TypeFamily>(SyntaxRole::Type)?
                        .map(|value| value.semantic())
                        .transpose()?,
                    trailing_recovery: syntax
                        .ordered_exact_children::<ErrorNodeKind>(SyntaxRoleClass::Recovery)?
                        .into_boxed_slice(),
                    syntax,
                    source_ordinal,
                },
            ))
        }
        SyntaxKind::FunctionItem => {
            let syntax = syntax.cast::<FunctionItemKind>()?;
            Ok(AttachedCapabilityMember::Function(
                AttachedCapabilityFunction {
                    prefix: TypedItemNode::Function(syntax.clone()).attached_prefix()?,
                    name: required_name(&syntax.syntax(), false)?,
                    generics: optional_generics(&syntax.syntax())?,
                    parameter_groups: syntax.callable_parameter_groups()?,
                    authored_return: syntax
                        .optional_exact_child::<ReturnTypeKind>(SyntaxRole::ReturnType)?
                        .map(|return_type| return_type.callable_semantics())
                        .transpose()?,
                    effects: attach_effects(&syntax)?,
                    trailing_recovery: syntax
                        .ordered_exact_children::<ErrorNodeKind>(SyntaxRoleClass::Recovery)?
                        .into_boxed_slice(),
                    syntax,
                    source_ordinal,
                },
            ))
        }
        SyntaxKind::ErrorItem => Ok(AttachedCapabilityMember::Error {
            syntax: syntax.cast()?,
            source_ordinal,
        }),
        _ => Err(SyntaxAccessError::InvalidItemProjection { id: syntax.id() }),
    }
}

fn attach_effects(
    syntax: &AstNode<FunctionItemKind>,
) -> Result<Option<AttachedCapabilityEffects>, SyntaxAccessError> {
    let open = syntax.optional_exact_child::<OpenBraceKind>(SyntaxRole::OpenDelimiter)?;
    let close = syntax.optional_exact_child::<CloseBraceKind>(SyntaxRole::CloseDelimiter)?;
    let expressions = syntax
        .ordered_family_children::<ExpressionFamily>(SyntaxRoleClass::Element)?
        .into_iter()
        .map(|expression| expression.semantic())
        .collect::<Result<Vec<_>, _>>()?
        .into_boxed_slice();
    match (open, close) {
        (None, None) if expressions.is_empty() => Ok(None),
        (Some(open), Some(close)) => Ok(Some(AttachedCapabilityEffects {
            open,
            close,
            expressions,
        })),
        _ => Err(SyntaxAccessError::InvalidItemProjection { id: syntax.id() }),
    }
}

#[cfg(test)]
mod tests;
