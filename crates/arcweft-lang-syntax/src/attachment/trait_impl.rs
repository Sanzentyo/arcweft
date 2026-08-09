//! Attached ownership for Trait and Impl declarations and their inline members.

use crate::grammar::kinds::{SyntaxKind, SyntaxRole, SyntaxRoleClass};

use super::callable::{attach_function_body, method_parameter_shape_has_recovery};
use super::family::{FamilyNode, TypeFamily};
use super::node::{
    AstNode, CloseBraceKind, ErrorItemKind, ErrorNodeKind, FunctionBodyKind, FunctionItemKind,
    ImplItemKind, MissingBodyKind, OpenBraceKind, ReturnTypeKind, TraitItemKind, TypeAliasItemKind,
};
use super::nominal::{optional_generics, required_name, required_type, where_clauses};
use super::{
    AttachedCallableReturn, AttachedFunctionBody, AttachedGenericParameterGroup,
    AttachedItemPrefix, AttachedMethodParameter, AttachedMethodParameterGroup,
    AttachedRequiredName, AttachedTypeFamily, AttachedTypeRefNode, AttachedWhereClause,
    SyntaxAccessError, SyntaxNodeHandle, TypedItemNode,
};

/// One associated type owned inline by a Trait declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedTraitAssociatedType {
    syntax: AstNode<TypeAliasItemKind>,
    source_ordinal: u16,
    prefix: AttachedItemPrefix,
    name: AttachedRequiredName,
    generics: Option<AttachedGenericParameterGroup>,
    default: Option<AttachedTypeRefNode>,
    trailing_recovery: Box<[AstNode<ErrorNodeKind>]>,
}

impl AttachedTraitAssociatedType {
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

    pub const fn default(&self) -> Option<&AttachedTypeRefNode> {
        self.default.as_ref()
    }

    pub const fn trailing_recovery(&self) -> &[AstNode<ErrorNodeKind>] {
        &self.trailing_recovery
    }

    pub fn has_recovery(&self) -> bool {
        self.name.is_missing()
            || self
                .generics
                .as_ref()
                .is_some_and(AttachedGenericParameterGroup::has_recovery)
            || self
                .default
                .as_ref()
                .is_some_and(|value| value.family() == AttachedTypeFamily::Recovery)
            || !self.trailing_recovery.is_empty()
    }
}

/// One associated type assignment owned inline by an Impl declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedImplAssociatedType {
    syntax: AstNode<TypeAliasItemKind>,
    source_ordinal: u16,
    prefix: AttachedItemPrefix,
    name: AttachedRequiredName,
    generics: Option<AttachedGenericParameterGroup>,
    target: AttachedTypeRefNode,
    trailing_recovery: Box<[AstNode<ErrorNodeKind>]>,
}

impl AttachedImplAssociatedType {
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

    pub const fn target(&self) -> &AttachedTypeRefNode {
        &self.target
    }

    pub const fn trailing_recovery(&self) -> &[AstNode<ErrorNodeKind>] {
        &self.trailing_recovery
    }

    pub fn has_recovery(&self) -> bool {
        self.name.is_missing()
            || self
                .generics
                .as_ref()
                .is_some_and(AttachedGenericParameterGroup::has_recovery)
            || self.target.family() == AttachedTypeFamily::Recovery
            || !self.trailing_recovery.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AttachedMethodParts {
    prefix: AttachedItemPrefix,
    name: AttachedRequiredName,
    generics: Option<AttachedGenericParameterGroup>,
    parameter_groups: Box<[AttachedMethodParameterGroup]>,
    where_clauses: Box<[AttachedWhereClause]>,
    authored_return: Option<AttachedCallableReturn>,
    body: Option<AttachedFunctionBody>,
    trailing_recovery: Box<[AstNode<ErrorNodeKind>]>,
}

impl AttachedMethodParts {
    fn parameters(&self) -> impl Iterator<Item = &AttachedMethodParameter> {
        self.parameter_groups
            .iter()
            .flat_map(AttachedMethodParameterGroup::parameters)
    }

    fn has_recovery(&self) -> bool {
        self.name.is_missing()
            || self
                .generics
                .as_ref()
                .is_some_and(AttachedGenericParameterGroup::has_recovery)
            || method_parameter_shape_has_recovery(&self.parameter_groups)
            || self
                .parameter_groups
                .iter()
                .any(AttachedMethodParameterGroup::has_recovery)
            || self
                .where_clauses
                .iter()
                .any(AttachedWhereClause::has_recovery)
            || self
                .authored_return
                .as_ref()
                .is_some_and(AttachedCallableReturn::has_recovery)
            || self
                .body
                .as_ref()
                .is_some_and(AttachedFunctionBody::has_recovery)
            || !self.trailing_recovery.is_empty()
    }
}

/// One source-ordered function signature or default method in a Trait.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedTraitFunction {
    syntax: AstNode<FunctionItemKind>,
    source_ordinal: u16,
    parts: AttachedMethodParts,
}

impl AttachedTraitFunction {
    pub const fn syntax(&self) -> &AstNode<FunctionItemKind> {
        &self.syntax
    }

    pub const fn source_ordinal(&self) -> u16 {
        self.source_ordinal
    }

    pub const fn prefix(&self) -> &AttachedItemPrefix {
        &self.parts.prefix
    }

    pub const fn name(&self) -> &AttachedRequiredName {
        &self.parts.name
    }

    pub const fn generics(&self) -> Option<&AttachedGenericParameterGroup> {
        self.parts.generics.as_ref()
    }

    pub const fn parameter_groups(&self) -> &[AttachedMethodParameterGroup] {
        &self.parts.parameter_groups
    }

    pub fn parameters(&self) -> impl Iterator<Item = &AttachedMethodParameter> {
        self.parts.parameters()
    }

    pub fn has_parameter_shape_recovery(&self) -> bool {
        method_parameter_shape_has_recovery(&self.parts.parameter_groups)
    }

    pub const fn where_clauses(&self) -> &[AttachedWhereClause] {
        &self.parts.where_clauses
    }

    pub const fn authored_return(&self) -> Option<&AttachedCallableReturn> {
        self.parts.authored_return.as_ref()
    }

    pub const fn body(&self) -> Option<&AttachedFunctionBody> {
        self.parts.body.as_ref()
    }

    pub const fn trailing_recovery(&self) -> &[AstNode<ErrorNodeKind>] {
        &self.parts.trailing_recovery
    }

    pub fn has_recovery(&self) -> bool {
        self.parts.has_recovery()
    }
}

/// One source-ordered function signature or authored method in an Impl.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedImplFunction {
    syntax: AstNode<FunctionItemKind>,
    source_ordinal: u16,
    parts: AttachedMethodParts,
}

impl AttachedImplFunction {
    pub const fn syntax(&self) -> &AstNode<FunctionItemKind> {
        &self.syntax
    }

    pub const fn source_ordinal(&self) -> u16 {
        self.source_ordinal
    }

    pub const fn prefix(&self) -> &AttachedItemPrefix {
        &self.parts.prefix
    }

    pub const fn name(&self) -> &AttachedRequiredName {
        &self.parts.name
    }

    pub const fn generics(&self) -> Option<&AttachedGenericParameterGroup> {
        self.parts.generics.as_ref()
    }

    pub const fn parameter_groups(&self) -> &[AttachedMethodParameterGroup] {
        &self.parts.parameter_groups
    }

    pub fn parameters(&self) -> impl Iterator<Item = &AttachedMethodParameter> {
        self.parts.parameters()
    }

    pub fn has_parameter_shape_recovery(&self) -> bool {
        method_parameter_shape_has_recovery(&self.parts.parameter_groups)
    }

    pub const fn where_clauses(&self) -> &[AttachedWhereClause] {
        &self.parts.where_clauses
    }

    pub const fn authored_return(&self) -> Option<&AttachedCallableReturn> {
        self.parts.authored_return.as_ref()
    }

    pub const fn body(&self) -> Option<&AttachedFunctionBody> {
        self.parts.body.as_ref()
    }

    pub const fn trailing_recovery(&self) -> &[AstNode<ErrorNodeKind>] {
        &self.parts.trailing_recovery
    }

    pub fn has_recovery(&self) -> bool {
        self.parts.has_recovery()
    }
}

/// Closed Trait member family in source order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedTraitMember {
    AssociatedType(AttachedTraitAssociatedType),
    Function(AttachedTraitFunction),
    Error {
        syntax: AstNode<ErrorItemKind>,
        source_ordinal: u16,
    },
}

impl AttachedTraitMember {
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

/// Closed Impl member family in source order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedImplMember {
    AssociatedType(AttachedImplAssociatedType),
    Function(AttachedImplFunction),
    Error {
        syntax: AstNode<ErrorItemKind>,
        source_ordinal: u16,
    },
}

impl AttachedImplMember {
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

/// Missing or braced Trait declaration body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedTraitBody {
    Missing(AstNode<MissingBodyKind>),
    Braced {
        open: AstNode<OpenBraceKind>,
        close: AstNode<CloseBraceKind>,
        members: Box<[AttachedTraitMember]>,
    },
}

impl AttachedTraitBody {
    pub const fn members(&self) -> &[AttachedTraitMember] {
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
            || self.members().iter().any(AttachedTraitMember::has_recovery)
    }
}

/// Missing or braced Impl declaration body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedImplBody {
    Missing(AstNode<MissingBodyKind>),
    Braced {
        open: AstNode<OpenBraceKind>,
        close: AstNode<CloseBraceKind>,
        members: Box<[AttachedImplMember]>,
    },
}

impl AttachedImplBody {
    pub const fn members(&self) -> &[AttachedImplMember] {
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
            || self.members().iter().any(AttachedImplMember::has_recovery)
    }
}

/// Complete attached Trait declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedTraitDeclaration {
    syntax: AstNode<TraitItemKind>,
    prefix: AttachedItemPrefix,
    name: AttachedRequiredName,
    generics: Option<AttachedGenericParameterGroup>,
    supertraits: Box<[AttachedTypeRefNode]>,
    where_clauses: Box<[AttachedWhereClause]>,
    body: AttachedTraitBody,
    trailing_recovery: Box<[AstNode<ErrorNodeKind>]>,
}

impl AttachedTraitDeclaration {
    pub const fn syntax(&self) -> &AstNode<TraitItemKind> {
        &self.syntax
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

    pub const fn supertraits(&self) -> &[AttachedTypeRefNode] {
        &self.supertraits
    }

    pub const fn where_clauses(&self) -> &[AttachedWhereClause] {
        &self.where_clauses
    }

    pub const fn body(&self) -> &AttachedTraitBody {
        &self.body
    }

    pub const fn trailing_recovery(&self) -> &[AstNode<ErrorNodeKind>] {
        &self.trailing_recovery
    }

    pub fn has_recovery(&self) -> bool {
        self.name.is_missing()
            || self
                .generics
                .as_ref()
                .is_some_and(AttachedGenericParameterGroup::has_recovery)
            || self
                .supertraits
                .iter()
                .any(|ty| ty.family() == AttachedTypeFamily::Recovery)
            || self
                .where_clauses
                .iter()
                .any(AttachedWhereClause::has_recovery)
            || self.body.has_recovery()
            || !self.trailing_recovery.is_empty()
    }
}

/// Complete attached Impl declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedImplDeclaration {
    syntax: AstNode<ImplItemKind>,
    prefix: AttachedItemPrefix,
    generics: Option<AttachedGenericParameterGroup>,
    trait_ref: Option<AttachedTypeRefNode>,
    target: AttachedTypeRefNode,
    where_clauses: Box<[AttachedWhereClause]>,
    body: AttachedImplBody,
    trailing_recovery: Box<[AstNode<ErrorNodeKind>]>,
}

impl AttachedImplDeclaration {
    pub const fn syntax(&self) -> &AstNode<ImplItemKind> {
        &self.syntax
    }

    pub const fn prefix(&self) -> &AttachedItemPrefix {
        &self.prefix
    }

    pub const fn generics(&self) -> Option<&AttachedGenericParameterGroup> {
        self.generics.as_ref()
    }

    pub const fn trait_ref(&self) -> Option<&AttachedTypeRefNode> {
        self.trait_ref.as_ref()
    }

    pub const fn target(&self) -> &AttachedTypeRefNode {
        &self.target
    }

    pub const fn where_clauses(&self) -> &[AttachedWhereClause] {
        &self.where_clauses
    }

    pub const fn body(&self) -> &AttachedImplBody {
        &self.body
    }

    pub const fn trailing_recovery(&self) -> &[AstNode<ErrorNodeKind>] {
        &self.trailing_recovery
    }

    pub fn has_recovery(&self) -> bool {
        self.generics
            .as_ref()
            .is_some_and(AttachedGenericParameterGroup::has_recovery)
            || self
                .trait_ref
                .as_ref()
                .is_some_and(|ty| ty.family() == AttachedTypeFamily::Recovery)
            || self.target.family() == AttachedTypeFamily::Recovery
            || self
                .where_clauses
                .iter()
                .any(AttachedWhereClause::has_recovery)
            || self.body.has_recovery()
            || !self.trailing_recovery.is_empty()
    }
}

impl AstNode<TraitItemKind> {
    /// Binds this Trait and all inline members to one immutable snapshot.
    pub fn semantics(&self) -> Result<AttachedTraitDeclaration, SyntaxAccessError> {
        Ok(AttachedTraitDeclaration {
            syntax: self.clone(),
            prefix: TypedItemNode::Trait(self.clone()).attached_prefix()?,
            name: required_name(&self.syntax(), false)?,
            generics: optional_generics(&self.syntax())?,
            supertraits: ordered_direct_types(&self.syntax())?,
            where_clauses: where_clauses(&self.syntax())?,
            body: attach_trait_body(self)?,
            trailing_recovery: self
                .ordered_exact_children::<ErrorNodeKind>(SyntaxRoleClass::Recovery)?
                .into_boxed_slice(),
        })
    }
}

impl AstNode<ImplItemKind> {
    /// Binds this Impl and all inline members to one immutable snapshot.
    pub fn semantics(&self) -> Result<AttachedImplDeclaration, SyntaxAccessError> {
        Ok(AttachedImplDeclaration {
            syntax: self.clone(),
            prefix: TypedItemNode::Impl(self.clone()).attached_prefix()?,
            generics: optional_generics(&self.syntax())?,
            trait_ref: self
                .optional_family_child::<TypeFamily>(SyntaxRole::Target)?
                .map(|ty| ty.semantic())
                .transpose()?,
            target: required_type(&self.syntax(), SyntaxRole::Type)?,
            where_clauses: where_clauses(&self.syntax())?,
            body: attach_impl_body(self)?,
            trailing_recovery: self
                .ordered_exact_children::<ErrorNodeKind>(SyntaxRoleClass::Recovery)?
                .into_boxed_slice(),
        })
    }
}

fn ordered_direct_types(
    owner: &SyntaxNodeHandle,
) -> Result<Box<[AttachedTypeRefNode]>, SyntaxAccessError> {
    owner
        .children()
        .into_iter()
        .filter(|child| child.kind().is_type_node())
        .enumerate()
        .map(|(ordinal, syntax)| {
            let ordinal = u32::try_from(ordinal)
                .map_err(|_| SyntaxAccessError::InvalidItemProjection { id: owner.id() })?;
            if syntax.role() != SyntaxRole::Element(ordinal) {
                return Err(SyntaxAccessError::InvalidItemProjection { id: syntax.id() });
            }
            FamilyNode::<TypeFamily>::new(syntax)?.semantic()
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Vec::into_boxed_slice)
}

fn attach_trait_body(
    owner: &AstNode<TraitItemKind>,
) -> Result<AttachedTraitBody, SyntaxAccessError> {
    let missing = owner.optional_exact_child::<MissingBodyKind>(SyntaxRole::Body)?;
    let open = owner.optional_exact_child::<OpenBraceKind>(SyntaxRole::OpenDelimiter)?;
    let close = owner.optional_exact_child::<CloseBraceKind>(SyntaxRole::CloseDelimiter)?;
    let members = direct_member_nodes(&owner.syntax());
    match (missing, open, close) {
        (Some(missing), None, None) if members.is_empty() => {
            Ok(AttachedTraitBody::Missing(missing))
        }
        (None, Some(open), Some(close)) => Ok(AttachedTraitBody::Braced {
            open,
            close,
            members: members
                .into_iter()
                .enumerate()
                .map(|(ordinal, syntax)| {
                    let ordinal = u16::try_from(ordinal)
                        .map_err(|_| SyntaxAccessError::InvalidItemProjection { id: owner.id() })?;
                    attach_trait_member(&syntax, ordinal)
                })
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice(),
        }),
        _ => Err(SyntaxAccessError::InvalidItemProjection { id: owner.id() }),
    }
}

fn attach_impl_body(owner: &AstNode<ImplItemKind>) -> Result<AttachedImplBody, SyntaxAccessError> {
    let missing = owner.optional_exact_child::<MissingBodyKind>(SyntaxRole::Body)?;
    let open = owner.optional_exact_child::<OpenBraceKind>(SyntaxRole::OpenDelimiter)?;
    let close = owner.optional_exact_child::<CloseBraceKind>(SyntaxRole::CloseDelimiter)?;
    let members = direct_member_nodes(&owner.syntax());
    match (missing, open, close) {
        (Some(missing), None, None) if members.is_empty() => Ok(AttachedImplBody::Missing(missing)),
        (None, Some(open), Some(close)) => Ok(AttachedImplBody::Braced {
            open,
            close,
            members: members
                .into_iter()
                .enumerate()
                .map(|(ordinal, syntax)| {
                    let ordinal = u16::try_from(ordinal)
                        .map_err(|_| SyntaxAccessError::InvalidItemProjection { id: owner.id() })?;
                    attach_impl_member(&syntax, ordinal)
                })
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice(),
        }),
        _ => Err(SyntaxAccessError::InvalidItemProjection { id: owner.id() }),
    }
}

fn direct_member_nodes(owner: &SyntaxNodeHandle) -> Vec<SyntaxNodeHandle> {
    owner
        .children()
        .into_iter()
        .filter(|child| {
            matches!(
                child.kind(),
                SyntaxKind::TypeAliasItem | SyntaxKind::FunctionItem | SyntaxKind::ErrorItem
            )
        })
        .collect()
}

fn attach_trait_member(
    syntax: &SyntaxNodeHandle,
    source_ordinal: u16,
) -> Result<AttachedTraitMember, SyntaxAccessError> {
    validate_member_role(syntax, source_ordinal)?;
    match syntax.kind() {
        SyntaxKind::TypeAliasItem => {
            let syntax = syntax.cast::<TypeAliasItemKind>()?;
            Ok(AttachedTraitMember::AssociatedType(
                AttachedTraitAssociatedType {
                    prefix: TypedItemNode::TypeAlias(syntax.clone()).attached_prefix()?,
                    name: required_name(&syntax.syntax(), false)?,
                    generics: optional_generics(&syntax.syntax())?,
                    default: syntax
                        .optional_family_child::<TypeFamily>(SyntaxRole::Type)?
                        .map(|ty| ty.semantic())
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
            Ok(AttachedTraitMember::Function(AttachedTraitFunction {
                parts: attach_method_parts(&syntax)?,
                syntax,
                source_ordinal,
            }))
        }
        SyntaxKind::ErrorItem => Ok(AttachedTraitMember::Error {
            syntax: syntax.cast()?,
            source_ordinal,
        }),
        _ => Err(SyntaxAccessError::InvalidItemProjection { id: syntax.id() }),
    }
}

fn attach_impl_member(
    syntax: &SyntaxNodeHandle,
    source_ordinal: u16,
) -> Result<AttachedImplMember, SyntaxAccessError> {
    validate_member_role(syntax, source_ordinal)?;
    match syntax.kind() {
        SyntaxKind::TypeAliasItem => {
            let syntax = syntax.cast::<TypeAliasItemKind>()?;
            Ok(AttachedImplMember::AssociatedType(
                AttachedImplAssociatedType {
                    prefix: TypedItemNode::TypeAlias(syntax.clone()).attached_prefix()?,
                    name: required_name(&syntax.syntax(), false)?,
                    generics: optional_generics(&syntax.syntax())?,
                    target: required_type(&syntax.syntax(), SyntaxRole::Type)?,
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
            Ok(AttachedImplMember::Function(AttachedImplFunction {
                parts: attach_method_parts(&syntax)?,
                syntax,
                source_ordinal,
            }))
        }
        SyntaxKind::ErrorItem => Ok(AttachedImplMember::Error {
            syntax: syntax.cast()?,
            source_ordinal,
        }),
        _ => Err(SyntaxAccessError::InvalidItemProjection { id: syntax.id() }),
    }
}

fn validate_member_role(
    syntax: &SyntaxNodeHandle,
    source_ordinal: u16,
) -> Result<(), SyntaxAccessError> {
    if syntax.role() == SyntaxRole::Element(u32::from(source_ordinal)) {
        Ok(())
    } else {
        Err(SyntaxAccessError::InvalidItemProjection { id: syntax.id() })
    }
}

fn attach_method_parts(
    syntax: &AstNode<FunctionItemKind>,
) -> Result<AttachedMethodParts, SyntaxAccessError> {
    Ok(AttachedMethodParts {
        prefix: TypedItemNode::Function(syntax.clone()).attached_prefix()?,
        name: required_name(&syntax.syntax(), false)?,
        generics: optional_generics(&syntax.syntax())?,
        parameter_groups: syntax.method_parameter_groups()?,
        where_clauses: where_clauses(&syntax.syntax())?,
        authored_return: syntax
            .optional_exact_child::<ReturnTypeKind>(SyntaxRole::ReturnType)?
            .map(|return_type| return_type.callable_semantics())
            .transpose()?,
        body: optional_method_body(syntax)?,
        trailing_recovery: syntax
            .ordered_exact_children::<ErrorNodeKind>(SyntaxRoleClass::Recovery)?
            .into_boxed_slice(),
    })
}

fn optional_method_body(
    syntax: &AstNode<FunctionItemKind>,
) -> Result<Option<AttachedFunctionBody>, SyntaxAccessError> {
    let Some(body) = syntax.optional_exact_child::<FunctionBodyKind>(SyntaxRole::Body)? else {
        return Ok(None);
    };
    let body = attach_function_body(body)?;
    if matches!(body, AttachedFunctionBody::Missing { .. }) {
        return Err(SyntaxAccessError::InvalidItemProjection { id: syntax.id() });
    }
    Ok(Some(body))
}

#[cfg(test)]
mod tests;
