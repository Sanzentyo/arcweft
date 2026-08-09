//! Typed declaration-prefix ownership shared by final item lowerers.

use arcweft_source::SourceSpan;

use super::family::{ExprNode, RecoveryFamily, RecoveryNode};
use super::node::{
    AstNode, CallArgumentKind, CloseBracketKind, DocBlockKind, ExactAstKind, InnerAttributeKind,
    OpenBracketKind, OuterAttributeKind, PathKind,
};
use super::source_file::{
    AttachedDelimiterState, AttachedPath, AttachedPathRoot, AttachedPathSegmentKind,
    AttachedVisibility,
};
use super::{AttachedExpressionNode, SyntaxAccessError, TypedItemNode};
use crate::expressions::{
    ExpressionComponentRole, PendingExpressionComponent, SyntaxCallArgumentListTerminator,
    SyntaxCallArgumentPart, SyntaxCallArgumentProjection, SyntaxExpressionSlot,
};
use crate::grammar::attribute_projection::{PendingOuterAttributeForm, PendingOuterAttributeIssue};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole, SyntaxRoleClass};

/// HIR-ready documentation content paired with its exact syntax owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedDocumentation {
    syntax: AstNode<DocBlockKind>,
    markdown: Box<str>,
}

impl AttachedDocumentation {
    pub const fn syntax(&self) -> &AstNode<DocBlockKind> {
        &self.syntax
    }

    pub fn markdown(&self) -> &str {
        &self.markdown
    }
}

impl AstNode<DocBlockKind> {
    /// Decodes the documentation payload owned by this exact doc block.
    pub fn semantics(&self) -> Result<AttachedDocumentation, SyntaxAccessError> {
        Ok(AttachedDocumentation {
            syntax: self.clone(),
            markdown: documentation_markdown(self)?,
        })
    }
}

/// Authored or missing value owned by one ordinary attribute argument.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedAttributeValue {
    Authored(Box<AttachedExpressionNode>),
    Missing(ExprNode),
}

/// One exact revision-bound call component owned directly by an attribute.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedAttributeComponent {
    role: ExpressionComponentRole,
    source: SourceSpan,
}

impl AttachedAttributeComponent {
    pub const fn role(&self) -> ExpressionComponentRole {
        self.role
    }

    pub const fn source_span(&self) -> &SourceSpan {
        &self.source
    }
}

/// One source-ordered ordinary argument retained by an attribute Call form.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedAttributeArgument {
    syntax: AstNode<CallArgumentKind>,
    projection: SyntaxCallArgumentProjection,
    value: AttachedAttributeValue,
}

impl AttachedAttributeArgument {
    pub const fn syntax(&self) -> &AstNode<CallArgumentKind> {
        &self.syntax
    }

    pub const fn projection(&self) -> &SyntaxCallArgumentProjection {
        &self.projection
    }

    pub const fn value(&self) -> &AttachedAttributeValue {
        &self.value
    }
}

/// Current grammar form retained by a source or item attribute.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedOuterAttributeForm {
    Marker,
    Parenthesized {
        arguments: Box<[AttachedAttributeArgument]>,
        terminator: SyntaxCallArgumentListTerminator,
    },
}

impl AttachedOuterAttributeForm {
    pub fn arguments(&self) -> &[AttachedAttributeArgument] {
        match self {
            Self::Marker => &[],
            Self::Parenthesized { arguments, .. } => arguments,
        }
    }

    pub const fn terminator(&self) -> Option<SyntaxCallArgumentListTerminator> {
        match self {
            Self::Marker => None,
            Self::Parenthesized { terminator, .. } => Some(*terminator),
        }
    }
}

/// Parser-selected attribute recovery without a fabricated path or Call.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AttachedOuterAttributeIssue {
    MissingPath,
    InvalidShape,
}

/// One exact, structured outer attribute.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedOuterAttribute {
    syntax: AstNode<OuterAttributeKind>,
    open: AstNode<OpenBracketKind>,
    close: AstNode<CloseBracketKind>,
    path: AttachedPath,
    form: AttachedOuterAttributeForm,
    issue: Option<AttachedOuterAttributeIssue>,
    recovery: Option<RecoveryNode>,
    components: Box<[AttachedAttributeComponent]>,
}

/// One exact, structured source-level inner attribute.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedInnerAttribute {
    syntax: AstNode<InnerAttributeKind>,
    open: AstNode<OpenBracketKind>,
    close: AstNode<CloseBracketKind>,
    path: AttachedPath,
    form: AttachedOuterAttributeForm,
    issue: Option<AttachedOuterAttributeIssue>,
    recovery: Option<RecoveryNode>,
    components: Box<[AttachedAttributeComponent]>,
}

impl AttachedOuterAttribute {
    pub const fn syntax(&self) -> &AstNode<OuterAttributeKind> {
        &self.syntax
    }

    pub const fn open(&self) -> &AstNode<OpenBracketKind> {
        &self.open
    }

    pub const fn close(&self) -> &AstNode<CloseBracketKind> {
        &self.close
    }

    pub const fn path(&self) -> &AttachedPath {
        &self.path
    }

    pub const fn form(&self) -> &AttachedOuterAttributeForm {
        &self.form
    }

    pub fn arguments(&self) -> &[AttachedAttributeArgument] {
        self.form.arguments()
    }

    pub const fn issue(&self) -> Option<AttachedOuterAttributeIssue> {
        self.issue
    }

    pub const fn recovery(&self) -> Option<&RecoveryNode> {
        self.recovery.as_ref()
    }

    pub fn components(&self) -> &[AttachedAttributeComponent] {
        &self.components
    }

    pub fn component(&self, role: ExpressionComponentRole) -> Option<&SourceSpan> {
        self.components
            .iter()
            .find(|component| component.role == role)
            .map(|component| &component.source)
    }

    pub fn close_state(&self) -> AttachedDelimiterState {
        delimiter_state(&self.close)
    }
}

impl AttachedInnerAttribute {
    pub const fn syntax(&self) -> &AstNode<InnerAttributeKind> {
        &self.syntax
    }

    pub const fn open(&self) -> &AstNode<OpenBracketKind> {
        &self.open
    }

    pub const fn close(&self) -> &AstNode<CloseBracketKind> {
        &self.close
    }

    pub const fn path(&self) -> &AttachedPath {
        &self.path
    }

    pub const fn form(&self) -> &AttachedOuterAttributeForm {
        &self.form
    }

    pub fn arguments(&self) -> &[AttachedAttributeArgument] {
        self.form.arguments()
    }

    pub const fn issue(&self) -> Option<AttachedOuterAttributeIssue> {
        self.issue
    }

    pub const fn recovery(&self) -> Option<&RecoveryNode> {
        self.recovery.as_ref()
    }

    pub fn components(&self) -> &[AttachedAttributeComponent] {
        &self.components
    }

    pub fn component(&self, role: ExpressionComponentRole) -> Option<&SourceSpan> {
        self.components
            .iter()
            .find(|component| component.role == role)
            .map(|component| &component.source)
    }

    pub fn close_state(&self) -> AttachedDelimiterState {
        delimiter_state(&self.close)
    }
}

/// Common prefix retained once by a source item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedItemPrefix {
    documentation: Option<AttachedDocumentation>,
    attributes: Box<[AttachedOuterAttribute]>,
    visibility: Option<AttachedVisibility>,
}

impl AttachedItemPrefix {
    pub const fn documentation(&self) -> Option<&AttachedDocumentation> {
        self.documentation.as_ref()
    }

    pub const fn attributes(&self) -> &[AttachedOuterAttribute] {
        &self.attributes
    }

    pub const fn visibility(&self) -> Option<&AttachedVisibility> {
        self.visibility.as_ref()
    }

    pub(crate) fn remove_proof_trust_attributes(&mut self) {
        self.attributes = self
            .attributes
            .iter()
            .filter(|attribute| !is_verify_trusted_attribute(attribute))
            .cloned()
            .collect::<Vec<_>>()
            .into_boxed_slice();
    }
}

impl TypedItemNode {
    /// Binds the common declaration prefix without reparsing source text.
    pub fn attached_prefix(&self) -> Result<AttachedItemPrefix, SyntaxAccessError> {
        let mut prefix = self.attached_prefix_including_proof_trust()?;
        prefix.remove_proof_trust_attributes();
        Ok(prefix)
    }

    pub(crate) fn attached_prefix_including_proof_trust(
        &self,
    ) -> Result<AttachedItemPrefix, SyntaxAccessError> {
        let documentation = self
            .documentation()?
            .map(|syntax| syntax.semantics())
            .transpose()?;
        let attributes = self
            .attributes()?
            .into_iter()
            .map(|attribute| attribute.semantics())
            .collect::<Result<Vec<_>, _>>()?
            .into_boxed_slice();
        let visibility = self
            .visibility()?
            .map(|visibility| visibility.semantics())
            .transpose()?;
        Ok(AttachedItemPrefix {
            documentation,
            attributes,
            visibility,
        })
    }
}

pub(crate) fn is_verify_trusted_attribute(attribute: &AttachedOuterAttribute) -> bool {
    matches!(attribute.path().root(), AttachedPathRoot::ImplicitCrate)
        && matches!(
            attribute.path().segments(),
            [verify, trusted]
                if verify.kind() == AttachedPathSegmentKind::Identifier
                    && trusted.kind() == AttachedPathSegmentKind::Identifier
                    && verify.source_text() == "verify"
                    && trusted.source_text() == "trusted"
        )
}

impl AstNode<OuterAttributeKind> {
    /// Returns the parser-owned dotted path and ordinary argument grammar.
    pub fn semantics(&self) -> Result<AttachedOuterAttribute, SyntaxAccessError> {
        let AttachedAttributeParts {
            open,
            close,
            path,
            form,
            issue,
            recovery,
            components,
        } = attach_attribute(self)?;
        Ok(AttachedOuterAttribute {
            syntax: self.clone(),
            open,
            close,
            path,
            form,
            issue,
            recovery,
            components,
        })
    }
}

impl AstNode<InnerAttributeKind> {
    /// Returns the parser-owned source-level attribute path and arguments.
    pub fn semantics(&self) -> Result<AttachedInnerAttribute, SyntaxAccessError> {
        let AttachedAttributeParts {
            open,
            close,
            path,
            form,
            issue,
            recovery,
            components,
        } = attach_attribute(self)?;
        Ok(AttachedInnerAttribute {
            syntax: self.clone(),
            open,
            close,
            path,
            form,
            issue,
            recovery,
            components,
        })
    }
}

struct AttachedAttributeParts {
    open: AstNode<OpenBracketKind>,
    close: AstNode<CloseBracketKind>,
    path: AttachedPath,
    form: AttachedOuterAttributeForm,
    issue: Option<AttachedOuterAttributeIssue>,
    recovery: Option<RecoveryNode>,
    components: Box<[AttachedAttributeComponent]>,
}

fn attach_attribute<K: ExactAstKind>(
    owner: &AstNode<K>,
) -> Result<AttachedAttributeParts, SyntaxAccessError> {
    let pending = owner
        .syntax()
        .attribute_projection()
        .cloned()
        .ok_or(SyntaxAccessError::MissingAttributeProjection { id: owner.id() })?;
    if !pending.validates_components(owner.range()) {
        return Err(SyntaxAccessError::InvalidAttributeProjection { id: owner.id() });
    }
    let open = exact_attribute_delimiter::<K, OpenBracketKind>(owner, SyntaxRole::OpenDelimiter)?;
    let close =
        exact_attribute_delimiter::<K, CloseBracketKind>(owner, SyntaxRole::CloseDelimiter)?;
    let path =
        AttachedPath::from_syntax(owner.required_exact_child::<PathKind>(SyntaxRole::Target)?)?;
    let issue = pending.issue().map(|issue| match issue {
        PendingOuterAttributeIssue::MissingPath => AttachedOuterAttributeIssue::MissingPath,
        PendingOuterAttributeIssue::InvalidShape => AttachedOuterAttributeIssue::InvalidShape,
    });
    if !attribute_path_matches_issue(&path, issue) {
        return Err(SyntaxAccessError::InvalidAttributeProjection { id: owner.id() });
    }
    let recoveries = owner.ordered_family_children::<RecoveryFamily>(SyntaxRoleClass::Recovery)?;
    let recovery = match recoveries.as_slice() {
        [] => None,
        [recovery] => Some(recovery.clone()),
        _ => return Err(SyntaxAccessError::InvalidAttributeProjection { id: owner.id() }),
    };
    let recovery_shape_valid = match issue {
        None => recovery.is_none(),
        Some(AttachedOuterAttributeIssue::MissingPath) => true,
        Some(AttachedOuterAttributeIssue::InvalidShape) => recovery.is_some(),
    };
    if !recovery_shape_valid {
        return Err(SyntaxAccessError::InvalidAttributeProjection { id: owner.id() });
    }
    let form = match pending.form() {
        PendingOuterAttributeForm::Marker => {
            if !owner
                .ordered_exact_children::<CallArgumentKind>(SyntaxRoleClass::Argument)?
                .is_empty()
            {
                return Err(SyntaxAccessError::InvalidAttributeProjection { id: owner.id() });
            }
            AttachedOuterAttributeForm::Marker
        }
        PendingOuterAttributeForm::Parenthesized {
            arguments,
            terminator,
        } => {
            let syntax_arguments =
                owner.ordered_exact_children::<CallArgumentKind>(SyntaxRoleClass::Argument)?;
            if syntax_arguments.len() != arguments.len() {
                return Err(SyntaxAccessError::InvalidAttributeProjection { id: owner.id() });
            }
            let arguments = syntax_arguments
                .into_iter()
                .zip(arguments.iter())
                .map(|(syntax, projection)| {
                    attach_attribute_argument(owner.id(), syntax, projection, pending.components())
                })
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice();
            AttachedOuterAttributeForm::Parenthesized {
                arguments,
                terminator: *terminator,
            }
        }
    };
    Ok(AttachedAttributeParts {
        open,
        close,
        path,
        form,
        issue,
        recovery,
        components: pending
            .components()
            .iter()
            .map(|component| AttachedAttributeComponent {
                role: component.role(),
                source: owner.syntax().source_span_for_range(component.range()),
            })
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    })
}

fn exact_attribute_delimiter<O: ExactAstKind, K: ExactAstKind>(
    owner: &AstNode<O>,
    role: SyntaxRole,
) -> Result<AstNode<K>, SyntaxAccessError> {
    let candidates = owner
        .syntax()
        .children()
        .into_iter()
        .filter(|child| child.role() == role && child.kind() == K::KIND)
        .collect::<Vec<_>>();
    match candidates.as_slice() {
        [] => Err(SyntaxAccessError::MissingExactChild {
            parent: owner.id(),
            role,
            expected: K::KIND,
        }),
        [candidate] => candidate.clone().cast().map_err(Into::into),
        _ => Err(SyntaxAccessError::AmbiguousChild {
            parent: owner.id(),
            role,
            count: candidates.len(),
        }),
    }
}

fn attribute_path_matches_issue(
    path: &AttachedPath,
    issue: Option<AttachedOuterAttributeIssue>,
) -> bool {
    if !matches!(path.root(), AttachedPathRoot::ImplicitCrate)
        || path
            .segments()
            .iter()
            .any(|segment| segment.kind() != AttachedPathSegmentKind::Identifier)
    {
        return false;
    }
    match issue {
        Some(AttachedOuterAttributeIssue::MissingPath) => {
            path.segments().is_empty() && path.missing_name().is_some()
        }
        None | Some(AttachedOuterAttributeIssue::InvalidShape) => {
            !path.segments().is_empty() && path.missing_name().is_none()
        }
    }
}

fn attach_attribute_argument(
    owner: super::SyntaxNodeId,
    syntax: AstNode<CallArgumentKind>,
    projection: &SyntaxCallArgumentProjection,
    components: &[PendingExpressionComponent],
) -> Result<AttachedAttributeArgument, SyntaxAccessError> {
    let SyntaxRole::Argument(argument) = syntax.role() else {
        return Err(SyntaxAccessError::InvalidAttributeProjection { id: owner });
    };
    let whole = attribute_component_range(
        components,
        ExpressionComponentRole::CallArgument {
            argument,
            part: SyntaxCallArgumentPart::Whole,
        },
    )
    .ok_or(SyntaxAccessError::InvalidAttributeProjection { id: owner })?;
    if syntax.range() != whole {
        return Err(SyntaxAccessError::InvalidAttributeProjection { id: owner });
    }
    let value = syntax.operand()?;
    let value_source = attribute_component_range(
        components,
        ExpressionComponentRole::CallArgument {
            argument,
            part: SyntaxCallArgumentPart::Value,
        },
    )
    .ok_or(SyntaxAccessError::InvalidAttributeProjection { id: owner })?;
    if value.range() != value_source {
        return Err(SyntaxAccessError::InvalidAttributeProjection { id: owner });
    }
    let name = syntax.name()?;
    match projection {
        SyntaxCallArgumentProjection::Named { .. } => {
            let name = name.ok_or(SyntaxAccessError::InvalidAttributeProjection { id: owner })?;
            let name_source = attribute_component_range(
                components,
                ExpressionComponentRole::CallArgument {
                    argument,
                    part: SyntaxCallArgumentPart::Name,
                },
            )
            .ok_or(SyntaxAccessError::InvalidAttributeProjection { id: owner })?;
            if name.range() != name_source {
                return Err(SyntaxAccessError::InvalidAttributeProjection { id: owner });
            }
        }
        SyntaxCallArgumentProjection::Positional { .. }
        | SyntaxCallArgumentProjection::Spread { .. }
            if name.is_some() =>
        {
            return Err(SyntaxAccessError::InvalidAttributeProjection { id: owner });
        }
        SyntaxCallArgumentProjection::Positional { .. }
        | SyntaxCallArgumentProjection::Spread { .. } => {}
    }
    let value = match (projection.value(), value.kind()) {
        (SyntaxExpressionSlot::Missing, SyntaxKind::MissingExpression) => {
            AttachedAttributeValue::Missing(value)
        }
        (SyntaxExpressionSlot::Authored, kind) if kind != SyntaxKind::MissingExpression => {
            AttachedAttributeValue::Authored(Box::new(value.semantic()?))
        }
        _ => return Err(SyntaxAccessError::InvalidAttributeProjection { id: owner }),
    };
    Ok(AttachedAttributeArgument {
        syntax,
        projection: projection.clone(),
        value,
    })
}

fn attribute_component_range(
    components: &[PendingExpressionComponent],
    role: ExpressionComponentRole,
) -> Option<arcweft_source::SourceRange> {
    components
        .iter()
        .find(|component| component.role() == role)
        .map(|component| component.range())
}

fn documentation_markdown(syntax: &AstNode<DocBlockKind>) -> Result<Box<str>, SyntaxAccessError> {
    syntax
        .source_text()
        .lines()
        .map(|line| {
            let line = line.trim_start_matches([' ', '\t']);
            line.strip_prefix("///")
                .map(|body| body.strip_prefix(' ').unwrap_or(body))
                .ok_or(SyntaxAccessError::InvalidItemProjection { id: syntax.id() })
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|lines| lines.join("\n").into_boxed_str())
}

fn delimiter_state<K: super::node::ExactAstKind>(syntax: &AstNode<K>) -> AttachedDelimiterState {
    let source = syntax.source_span();
    if syntax.range().is_empty() {
        AttachedDelimiterState::Missing(source)
    } else {
        AttachedDelimiterState::Authored(source)
    }
}
