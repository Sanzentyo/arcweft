//! Typed nominal declaration ownership over the attached grammar tree.

use super::family::{NameFamily, TypeFamily};
use super::node::{
    AstNode, CloseAngleKind, CloseBraceKind, ColonKind, DocBlockKind, EnumItemKind, EqualsKind,
    GenericParameterGroupKind, GenericParameterKind, LifetimeParameterKind, MissingBodyKind,
    OpenAngleKind, OpenBraceKind, OuterAttributeKind, RecordFieldKind, StructItemKind,
    TypeAliasItemKind, TypeParameterKind, WhereClauseKind, WherePredicateKind,
};
use super::source_file::AttachedDelimiterState;
use super::{
    AttachedDocumentation, AttachedItemPrefix, AttachedOuterAttribute, AttachedTypeFamily,
    AttachedTypeRefNode, NameNode, SyntaxAccessError, SyntaxNodeHandle, TypedItemNode,
};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole, SyntaxRoleClass};
use crate::name::SyntaxName;

/// Required declaration/member name without a fabricated recovery spelling.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedRequiredName {
    Resolved { syntax: NameNode, value: SyntaxName },
    Missing { syntax: NameNode },
}

impl AttachedRequiredName {
    pub const fn syntax(&self) -> &NameNode {
        match self {
            Self::Resolved { syntax, .. } | Self::Missing { syntax } => syntax,
        }
    }

    pub const fn value(&self) -> Option<&SyntaxName> {
        match self {
            Self::Resolved { value, .. } => Some(value),
            Self::Missing { .. } => None,
        }
    }

    pub const fn is_missing(&self) -> bool {
        matches!(self, Self::Missing { .. })
    }
}

/// One required punctuation token retained as authored bytes or insertion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedRequiredPunctuation {
    Authored(arcweft_source::SourceSpan),
    Missing(arcweft_source::SourceSpan),
}

impl AttachedRequiredPunctuation {
    pub const fn source_span(&self) -> &arcweft_source::SourceSpan {
        match self {
            Self::Authored(source) | Self::Missing(source) => source,
        }
    }

    pub const fn is_missing(&self) -> bool {
        matches!(self, Self::Missing(_))
    }
}

/// One final generic parameter attached to exact type children.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedGenericParameter {
    Lifetime {
        syntax: AstNode<GenericParameterKind>,
        name: AttachedRequiredName,
        has_recovery: bool,
    },
    Type {
        syntax: AstNode<GenericParameterKind>,
        name: AttachedRequiredName,
        colon: Option<AttachedRequiredPunctuation>,
        bounds: Box<[AttachedTypeRefNode]>,
        has_recovery: bool,
    },
}

impl AttachedGenericParameter {
    pub const fn syntax(&self) -> &AstNode<GenericParameterKind> {
        match self {
            Self::Lifetime { syntax, .. } | Self::Type { syntax, .. } => syntax,
        }
    }

    pub const fn name(&self) -> &AttachedRequiredName {
        match self {
            Self::Lifetime { name, .. } | Self::Type { name, .. } => name,
        }
    }

    pub fn bounds(&self) -> &[AttachedTypeRefNode] {
        match self {
            Self::Lifetime { .. } => &[],
            Self::Type { bounds, .. } => bounds,
        }
    }

    pub const fn colon(&self) -> Option<&AttachedRequiredPunctuation> {
        match self {
            Self::Lifetime { .. } => None,
            Self::Type { colon, .. } => colon.as_ref(),
        }
    }

    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Lifetime {
                name, has_recovery, ..
            } => name.is_missing() || *has_recovery,
            Self::Type {
                name,
                bounds,
                has_recovery,
                ..
            } => {
                name.is_missing()
                    || *has_recovery
                    || bounds
                        .iter()
                        .any(|bound| bound.family() == AttachedTypeFamily::Recovery)
            }
        }
    }
}

/// One optional generic parameter group and its exact delimiters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedGenericParameterGroup {
    syntax: AstNode<GenericParameterGroupKind>,
    open: AstNode<OpenAngleKind>,
    close: AstNode<CloseAngleKind>,
    parameters: Box<[AttachedGenericParameter]>,
}

impl AttachedGenericParameterGroup {
    pub const fn syntax(&self) -> &AstNode<GenericParameterGroupKind> {
        &self.syntax
    }

    pub const fn open(&self) -> &AstNode<OpenAngleKind> {
        &self.open
    }

    pub const fn close(&self) -> &AstNode<CloseAngleKind> {
        &self.close
    }

    pub const fn parameters(&self) -> &[AttachedGenericParameter] {
        &self.parameters
    }

    pub fn close_state(&self) -> AttachedDelimiterState {
        delimiter_state(&self.close)
    }

    pub fn has_recovery(&self) -> bool {
        matches!(self.close_state(), AttachedDelimiterState::Missing(_))
            || self
                .parameters
                .iter()
                .any(AttachedGenericParameter::has_recovery)
    }
}

/// One source-ordered `where` predicate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedWherePredicate {
    syntax: AstNode<WherePredicateKind>,
    subject: AttachedTypeRefNode,
    colon: AttachedRequiredPunctuation,
    bounds: Box<[AttachedTypeRefNode]>,
}

impl AttachedWherePredicate {
    pub const fn syntax(&self) -> &AstNode<WherePredicateKind> {
        &self.syntax
    }

    pub const fn subject(&self) -> &AttachedTypeRefNode {
        &self.subject
    }

    pub const fn colon(&self) -> &AttachedRequiredPunctuation {
        &self.colon
    }

    pub const fn bounds(&self) -> &[AttachedTypeRefNode] {
        &self.bounds
    }

    pub fn has_recovery(&self) -> bool {
        self.colon.is_missing()
            || self.subject.family() == AttachedTypeFamily::Recovery
            || self
                .bounds
                .iter()
                .any(|bound| bound.family() == AttachedTypeFamily::Recovery)
    }
}

/// One authored `where` clause retaining its local predicate order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedWhereClause {
    syntax: AstNode<WhereClauseKind>,
    predicates: Box<[AttachedWherePredicate]>,
}

impl AttachedWhereClause {
    pub const fn syntax(&self) -> &AstNode<WhereClauseKind> {
        &self.syntax
    }

    pub const fn predicates(&self) -> &[AttachedWherePredicate] {
        &self.predicates
    }

    pub fn has_recovery(&self) -> bool {
        self.predicates
            .iter()
            .any(AttachedWherePredicate::has_recovery)
    }
}

/// Field-level prefix retained for nominal inline records.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedNominalFieldPrefix {
    documentation: Option<AttachedDocumentation>,
    attributes: Box<[AttachedOuterAttribute]>,
}

impl AttachedNominalFieldPrefix {
    pub const fn documentation(&self) -> Option<&AttachedDocumentation> {
        self.documentation.as_ref()
    }

    pub const fn attributes(&self) -> &[AttachedOuterAttribute] {
        &self.attributes
    }
}

/// One ordered Struct field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedStructField {
    syntax: AstNode<RecordFieldKind>,
    prefix: AttachedNominalFieldPrefix,
    name: AttachedRequiredName,
    colon: AttachedRequiredPunctuation,
    ty: AttachedTypeRefNode,
}

impl AttachedStructField {
    pub const fn syntax(&self) -> &AstNode<RecordFieldKind> {
        &self.syntax
    }

    pub const fn prefix(&self) -> &AttachedNominalFieldPrefix {
        &self.prefix
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

    pub fn has_recovery(&self) -> bool {
        self.name.is_missing()
            || self.colon.is_missing()
            || self.ty.family() == AttachedTypeFamily::Recovery
            || !self.prefix.attributes.is_empty()
    }
}

/// One ordered Enum variant with an optional payload type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedEnumVariant {
    syntax: AstNode<RecordFieldKind>,
    prefix: AttachedNominalFieldPrefix,
    name: AttachedRequiredName,
    payload: Option<AttachedTypeRefNode>,
}

impl AttachedEnumVariant {
    pub const fn syntax(&self) -> &AstNode<RecordFieldKind> {
        &self.syntax
    }

    pub const fn prefix(&self) -> &AttachedNominalFieldPrefix {
        &self.prefix
    }

    pub const fn name(&self) -> &AttachedRequiredName {
        &self.name
    }

    pub const fn payload(&self) -> Option<&AttachedTypeRefNode> {
        self.payload.as_ref()
    }

    pub fn has_recovery(&self) -> bool {
        self.name.is_missing()
            || self
                .payload
                .as_ref()
                .is_some_and(|payload| payload.family() == AttachedTypeFamily::Recovery)
            || !self.prefix.attributes.is_empty()
    }
}

/// Missing or braced Struct body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedStructBody {
    Missing(AstNode<MissingBodyKind>),
    Braced {
        open: AstNode<OpenBraceKind>,
        close: AstNode<CloseBraceKind>,
        fields: Box<[AttachedStructField]>,
    },
}

impl AttachedStructBody {
    pub fn fields(&self) -> &[AttachedStructField] {
        match self {
            Self::Missing(_) => &[],
            Self::Braced { fields, .. } => fields,
        }
    }

    pub fn is_missing_or_unclosed(&self) -> bool {
        match self {
            Self::Missing(_) => true,
            Self::Braced { close, .. } => {
                matches!(delimiter_state(close), AttachedDelimiterState::Missing(_))
            }
        }
    }
}

/// Missing or braced Enum body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedEnumBody {
    Missing(AstNode<MissingBodyKind>),
    Braced {
        open: AstNode<OpenBraceKind>,
        close: AstNode<CloseBraceKind>,
        variants: Box<[AttachedEnumVariant]>,
    },
}

impl AttachedEnumBody {
    pub fn variants(&self) -> &[AttachedEnumVariant] {
        match self {
            Self::Missing(_) => &[],
            Self::Braced { variants, .. } => variants,
        }
    }

    pub fn is_missing_or_unclosed(&self) -> bool {
        match self {
            Self::Missing(_) => true,
            Self::Braced { close, .. } => {
                matches!(delimiter_state(close), AttachedDelimiterState::Missing(_))
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedTypeAliasDeclaration {
    syntax: AstNode<TypeAliasItemKind>,
    prefix: AttachedItemPrefix,
    name: AttachedRequiredName,
    generics: Option<AttachedGenericParameterGroup>,
    assignment: AttachedRequiredPunctuation,
    target: AttachedTypeRefNode,
    where_clauses: Box<[AttachedWhereClause]>,
}

impl AttachedTypeAliasDeclaration {
    pub const fn syntax(&self) -> &AstNode<TypeAliasItemKind> {
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
    pub const fn assignment(&self) -> &AttachedRequiredPunctuation {
        &self.assignment
    }
    pub const fn target(&self) -> &AttachedTypeRefNode {
        &self.target
    }
    pub const fn where_clauses(&self) -> &[AttachedWhereClause] {
        &self.where_clauses
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedStructDeclaration {
    syntax: AstNode<StructItemKind>,
    prefix: AttachedItemPrefix,
    name: AttachedRequiredName,
    generics: Option<AttachedGenericParameterGroup>,
    where_clauses: Box<[AttachedWhereClause]>,
    body: AttachedStructBody,
}

impl AttachedStructDeclaration {
    pub const fn syntax(&self) -> &AstNode<StructItemKind> {
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
    pub const fn where_clauses(&self) -> &[AttachedWhereClause] {
        &self.where_clauses
    }
    pub const fn body(&self) -> &AttachedStructBody {
        &self.body
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedEnumDeclaration {
    syntax: AstNode<EnumItemKind>,
    prefix: AttachedItemPrefix,
    name: AttachedRequiredName,
    generics: Option<AttachedGenericParameterGroup>,
    where_clauses: Box<[AttachedWhereClause]>,
    body: AttachedEnumBody,
}

/// Closed attached owner for the three ordinary nominal declaration families.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedNominalDeclaration {
    TypeAlias(AttachedTypeAliasDeclaration),
    Struct(AttachedStructDeclaration),
    Enum(AttachedEnumDeclaration),
}

impl AttachedNominalDeclaration {
    pub fn id(&self) -> super::SyntaxNodeId {
        match self {
            Self::TypeAlias(declaration) => declaration.syntax().id(),
            Self::Struct(declaration) => declaration.syntax().id(),
            Self::Enum(declaration) => declaration.syntax().id(),
        }
    }

    pub fn snapshot_id(&self) -> &super::SyntaxSnapshotId {
        match self {
            Self::TypeAlias(declaration) => declaration.syntax().snapshot_id(),
            Self::Struct(declaration) => declaration.syntax().snapshot_id(),
            Self::Enum(declaration) => declaration.syntax().snapshot_id(),
        }
    }
}

impl AttachedEnumDeclaration {
    pub const fn syntax(&self) -> &AstNode<EnumItemKind> {
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
    pub const fn where_clauses(&self) -> &[AttachedWhereClause] {
        &self.where_clauses
    }
    pub const fn body(&self) -> &AttachedEnumBody {
        &self.body
    }
}

impl AstNode<TypeAliasItemKind> {
    pub fn semantics(&self) -> Result<AttachedTypeAliasDeclaration, SyntaxAccessError> {
        let item = TypedItemNode::TypeAlias(self.clone());
        Ok(AttachedTypeAliasDeclaration {
            syntax: self.clone(),
            prefix: item.attached_prefix()?,
            name: required_name(&item.syntax(), false)?,
            generics: optional_generics(&item.syntax())?,
            assignment: punctuation(&self.required_exact_child::<EqualsKind>(SyntaxRole::Equals)?),
            target: required_type(&item.syntax(), SyntaxRole::Type)?,
            where_clauses: where_clauses(&item.syntax())?,
        })
    }
}

impl AstNode<StructItemKind> {
    pub fn semantics(&self) -> Result<AttachedStructDeclaration, SyntaxAccessError> {
        let item = TypedItemNode::Struct(self.clone());
        Ok(AttachedStructDeclaration {
            syntax: self.clone(),
            prefix: item.attached_prefix()?,
            name: required_name(&item.syntax(), false)?,
            generics: optional_generics(&item.syntax())?,
            where_clauses: where_clauses(&item.syntax())?,
            body: struct_body(&item.syntax())?,
        })
    }
}

impl AstNode<EnumItemKind> {
    pub fn semantics(&self) -> Result<AttachedEnumDeclaration, SyntaxAccessError> {
        let item = TypedItemNode::Enum(self.clone());
        Ok(AttachedEnumDeclaration {
            syntax: self.clone(),
            prefix: item.attached_prefix()?,
            name: required_name(&item.syntax(), false)?,
            generics: optional_generics(&item.syntax())?,
            where_clauses: where_clauses(&item.syntax())?,
            body: enum_body(&item.syntax())?,
        })
    }
}

pub(super) fn optional_generics(
    owner: &SyntaxNodeHandle,
) -> Result<Option<AttachedGenericParameterGroup>, SyntaxAccessError> {
    owner
        .optional_unique_child(SyntaxRole::GenericGroup)?
        .map(|syntax| {
            let syntax = syntax.cast::<GenericParameterGroupKind>()?;
            let parameters = syntax
                .ordered_exact_children::<GenericParameterKind>(SyntaxRoleClass::GenericParameter)?
                .into_iter()
                .map(attach_generic_parameter)
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice();
            Ok(AttachedGenericParameterGroup {
                open: syntax.required_exact_child::<OpenAngleKind>(SyntaxRole::OpenDelimiter)?,
                close: syntax.required_exact_child::<CloseAngleKind>(SyntaxRole::CloseDelimiter)?,
                syntax,
                parameters,
            })
        })
        .transpose()
}

fn attach_generic_parameter(
    syntax: AstNode<GenericParameterKind>,
) -> Result<AttachedGenericParameter, SyntaxAccessError> {
    let children = syntax.syntax().children_with_role(SyntaxRole::Element(0));
    let [parameter] = children.as_slice() else {
        return Err(SyntaxAccessError::InvalidItemProjection { id: syntax.id() });
    };
    let recoveries = parameter
        .ordered_children(SyntaxRoleClass::Recovery)?
        .into_iter()
        .any(|child| child.kind() == SyntaxKind::ErrorNode);
    match parameter.kind() {
        SyntaxKind::LifetimeParameter => {
            let parameter = parameter.clone().cast::<LifetimeParameterKind>()?;
            Ok(AttachedGenericParameter::Lifetime {
                name: required_name(&parameter.syntax(), true)?,
                syntax,
                has_recovery: recoveries,
            })
        }
        SyntaxKind::TypeParameter => {
            let parameter = parameter.clone().cast::<TypeParameterKind>()?;
            let bounds = parameter
                .ordered_family_children::<TypeFamily>(SyntaxRoleClass::Element)?
                .into_iter()
                .map(|bound| bound.semantic())
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice();
            let colon = parameter
                .optional_exact_child::<ColonKind>(SyntaxRole::Colon)?
                .map(|colon| punctuation(&colon));
            if colon.is_some() == bounds.is_empty() {
                return Err(SyntaxAccessError::InvalidItemProjection { id: syntax.id() });
            }
            Ok(AttachedGenericParameter::Type {
                name: required_name(&parameter.syntax(), false)?,
                syntax,
                colon,
                bounds,
                has_recovery: recoveries,
            })
        }
        _ => Err(SyntaxAccessError::InvalidItemProjection { id: syntax.id() }),
    }
}

pub(super) fn where_clauses(
    owner: &SyntaxNodeHandle,
) -> Result<Box<[AttachedWhereClause]>, SyntaxAccessError> {
    owner
        .children()
        .into_iter()
        .filter(|child| child.role() == SyntaxRole::WhereClause)
        .map(|clause| {
            let syntax = clause.cast::<WhereClauseKind>()?;
            let predicates = syntax
                .ordered_exact_children::<WherePredicateKind>(SyntaxRoleClass::WherePredicate)?
                .into_iter()
                .map(|predicate| {
                    let subject = required_type(&predicate.syntax(), SyntaxRole::LeftOperand)?;
                    let colon = punctuation(
                        &predicate.required_exact_child::<ColonKind>(SyntaxRole::Colon)?,
                    );
                    let bounds = predicate
                        .ordered_family_children::<TypeFamily>(SyntaxRoleClass::Element)?
                        .into_iter()
                        .map(|bound| bound.semantic())
                        .collect::<Result<Vec<_>, _>>()?
                        .into_boxed_slice();
                    if bounds.is_empty() {
                        return Err(SyntaxAccessError::InvalidItemProjection {
                            id: predicate.id(),
                        });
                    }
                    Ok(AttachedWherePredicate {
                        syntax: predicate,
                        subject,
                        colon,
                        bounds,
                    })
                })
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice();
            Ok(AttachedWhereClause { syntax, predicates })
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Vec::into_boxed_slice)
}

fn struct_body(owner: &SyntaxNodeHandle) -> Result<AttachedStructBody, SyntaxAccessError> {
    if let Some(missing) = owner.optional_unique_child(SyntaxRole::Body)?
        && missing.kind() == SyntaxKind::MissingBody
    {
        return Ok(AttachedStructBody::Missing(missing.cast()?));
    }
    let fields = owner
        .ordered_children(SyntaxRoleClass::Field)?
        .into_iter()
        .map(|field| attach_struct_field(field.cast()?))
        .collect::<Result<Vec<_>, _>>()?
        .into_boxed_slice();
    Ok(AttachedStructBody::Braced {
        open: required_child(owner, SyntaxRole::OpenDelimiter)?.cast::<OpenBraceKind>()?,
        close: required_child(owner, SyntaxRole::CloseDelimiter)?.cast::<CloseBraceKind>()?,
        fields,
    })
}

fn enum_body(owner: &SyntaxNodeHandle) -> Result<AttachedEnumBody, SyntaxAccessError> {
    if let Some(missing) = owner.optional_unique_child(SyntaxRole::Body)?
        && missing.kind() == SyntaxKind::MissingBody
    {
        return Ok(AttachedEnumBody::Missing(missing.cast()?));
    }
    let variants = owner
        .ordered_children(SyntaxRoleClass::Field)?
        .into_iter()
        .map(|variant| attach_enum_variant(variant.cast()?))
        .collect::<Result<Vec<_>, _>>()?
        .into_boxed_slice();
    Ok(AttachedEnumBody::Braced {
        open: required_child(owner, SyntaxRole::OpenDelimiter)?.cast::<OpenBraceKind>()?,
        close: required_child(owner, SyntaxRole::CloseDelimiter)?.cast::<CloseBraceKind>()?,
        variants,
    })
}

fn required_child(
    owner: &SyntaxNodeHandle,
    role: SyntaxRole,
) -> Result<SyntaxNodeHandle, SyntaxAccessError> {
    owner
        .optional_unique_child(role)?
        .ok_or(SyntaxAccessError::InvalidItemProjection { id: owner.id() })
}

fn attach_struct_field(
    syntax: AstNode<RecordFieldKind>,
) -> Result<AttachedStructField, SyntaxAccessError> {
    Ok(AttachedStructField {
        prefix: field_prefix(&syntax)?,
        name: required_name(&syntax.syntax(), false)?,
        colon: punctuation(&syntax.required_exact_child::<ColonKind>(SyntaxRole::Colon)?),
        ty: required_type(&syntax.syntax(), SyntaxRole::Type)?,
        syntax,
    })
}

fn attach_enum_variant(
    syntax: AstNode<RecordFieldKind>,
) -> Result<AttachedEnumVariant, SyntaxAccessError> {
    let payload = syntax
        .optional_family_child::<TypeFamily>(SyntaxRole::Type)?
        .map(|payload| payload.semantic())
        .transpose()?;
    Ok(AttachedEnumVariant {
        prefix: field_prefix(&syntax)?,
        name: required_name(&syntax.syntax(), false)?,
        syntax,
        payload,
    })
}

fn field_prefix(
    syntax: &AstNode<RecordFieldKind>,
) -> Result<AttachedNominalFieldPrefix, SyntaxAccessError> {
    let documentation = syntax
        .optional_exact_child::<DocBlockKind>(SyntaxRole::Documentation)?
        .map(|documentation| documentation.semantics())
        .transpose()?;
    let attributes = syntax
        .ordered_exact_children::<OuterAttributeKind>(SyntaxRoleClass::Attribute)?
        .into_iter()
        .map(|attribute| attribute.semantics())
        .collect::<Result<Vec<_>, _>>()?
        .into_boxed_slice();
    Ok(AttachedNominalFieldPrefix {
        documentation,
        attributes,
    })
}

pub(super) fn required_name(
    owner: &SyntaxNodeHandle,
    lifetime: bool,
) -> Result<AttachedRequiredName, SyntaxAccessError> {
    let syntax = owner
        .optional_unique_child(SyntaxRole::Name)?
        .ok_or(SyntaxAccessError::InvalidItemProjection { id: owner.id() })?;
    let syntax = super::family::FamilyNode::<NameFamily>::new(syntax)?;
    match syntax.kind() {
        SyntaxKind::NameDefinition => {
            let spelling = if lifetime {
                syntax
                    .source_text()
                    .strip_prefix('\'')
                    .ok_or(SyntaxAccessError::InvalidItemProjection { id: owner.id() })?
            } else {
                syntax.source_text()
            };
            let value = SyntaxName::try_new(spelling)
                .map_err(|_| SyntaxAccessError::InvalidItemProjection { id: owner.id() })?;
            Ok(AttachedRequiredName::Resolved { syntax, value })
        }
        SyntaxKind::MissingName => Ok(AttachedRequiredName::Missing { syntax }),
        _ => Err(SyntaxAccessError::InvalidItemProjection { id: owner.id() }),
    }
}

pub(super) fn required_type(
    owner: &SyntaxNodeHandle,
    role: SyntaxRole,
) -> Result<AttachedTypeRefNode, SyntaxAccessError> {
    owner
        .optional_unique_child(role)?
        .ok_or(SyntaxAccessError::InvalidItemProjection { id: owner.id() })
        .and_then(super::family::FamilyNode::<TypeFamily>::new)?
        .semantic()
}

pub(super) fn punctuation<K: super::node::ExactAstKind>(
    syntax: &AstNode<K>,
) -> AttachedRequiredPunctuation {
    let source = syntax.source_span();
    if syntax.range().is_empty() {
        AttachedRequiredPunctuation::Missing(source)
    } else {
        AttachedRequiredPunctuation::Authored(source)
    }
}

fn delimiter_state<K: super::node::ExactAstKind>(syntax: &AstNode<K>) -> AttachedDelimiterState {
    let source = syntax.source_span();
    if syntax.range().is_empty() {
        AttachedDelimiterState::Missing(source)
    } else {
        AttachedDelimiterState::Authored(source)
    }
}
