//! Typed configured-resource ownership over the attached grammar tree.

use arcweft_id::PublicId;

use super::family::{ExpressionFamily, NameFamily};
use super::node::{
    AstNode, CloseBraceKind, ColonKind, EqualsKind, ErrorNodeKind, MissingBodyKind, OpenBraceKind,
    ResourceBodyKind, ResourceDeclarationItemKind, ResourceFieldInitializerKind,
};
use super::nominal::{punctuation, required_name, required_type};
use super::source_file::AttachedDelimiterState;
use super::{
    AttachedExpressionNode, AttachedItemPrefix, AttachedRequiredName, AttachedRequiredPunctuation,
    AttachedTypeRefNode, SyntaxAccessError, TypedItemNode,
};
use crate::expressions::ExpressionProjection;
use crate::grammar::kinds::{SyntaxKind, SyntaxRole, SyntaxRoleClass};
use crate::id_ref::AuthoredIdRoot;
use crate::types::TypeRef;

/// Optional explicit resource identity without a fabricated recovery value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedResourcePublicId {
    Absent,
    Explicit {
        syntax: Box<AttachedExpressionNode>,
        value: PublicId,
    },
    Recovered {
        syntax: Box<AttachedExpressionNode>,
        issue: AttachedResourcePublicIdIssue,
    },
}

impl AttachedResourcePublicId {
    pub const fn value(&self) -> Option<&PublicId> {
        match self {
            Self::Explicit { value, .. } => Some(value),
            Self::Absent | Self::Recovered { .. } => None,
        }
    }

    pub const fn syntax(&self) -> Option<&AttachedExpressionNode> {
        match self {
            Self::Explicit { syntax, .. } | Self::Recovered { syntax, .. } => Some(syntax),
            Self::Absent => None,
        }
    }

    pub const fn has_recovery(&self) -> bool {
        matches!(self, Self::Recovered { .. })
    }
}

/// Parser-owned explicit-resource-ID recovery.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttachedResourcePublicIdIssue {
    Relative,
    Malformed,
}

/// Exact source state of one required resource-field initializer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedResourceInitializer {
    /// The malformed field never produced an initializer slot.
    Absent,
    /// One parser-projected expression owned by the field.
    Authored(Box<AttachedExpressionNode>),
}

impl AttachedResourceInitializer {
    pub const fn authored(&self) -> Option<&AttachedExpressionNode> {
        match self {
            Self::Authored(expression) => Some(expression),
            Self::Absent => None,
        }
    }

    pub const fn is_absent(&self) -> bool {
        matches!(self, Self::Absent)
    }
}

/// One source-ordered resource field wrapper.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedResourceField {
    syntax: AstNode<ResourceFieldInitializerKind>,
    source_ordinal: u16,
    name: AttachedRequiredName,
    assignment: AttachedRequiredPunctuation,
    initializer: AttachedResourceInitializer,
    recoveries: Box<[AstNode<ErrorNodeKind>]>,
}

impl AttachedResourceField {
    pub const fn syntax(&self) -> &AstNode<ResourceFieldInitializerKind> {
        &self.syntax
    }

    pub const fn source_ordinal(&self) -> u16 {
        self.source_ordinal
    }

    pub const fn name(&self) -> &AttachedRequiredName {
        &self.name
    }

    pub const fn assignment(&self) -> &AttachedRequiredPunctuation {
        &self.assignment
    }

    pub const fn initializer(&self) -> &AttachedResourceInitializer {
        &self.initializer
    }

    pub const fn recoveries(&self) -> &[AstNode<ErrorNodeKind>] {
        &self.recoveries
    }

    pub fn has_recovery(&self) -> bool {
        self.name.is_missing()
            || self.assignment.is_missing()
            || self.initializer.is_absent()
            || !self.recoveries.is_empty()
            || self
                .initializer
                .authored()
                .is_some_and(|initializer| initializer.projection().has_recovery())
    }
}

/// Missing or braced resource field body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedResourceBody {
    Missing(AstNode<MissingBodyKind>),
    Braced {
        syntax: AstNode<ResourceBodyKind>,
        open: AstNode<OpenBraceKind>,
        close: AstNode<CloseBraceKind>,
        fields: Box<[AttachedResourceField]>,
    },
}

impl AttachedResourceBody {
    pub fn fields(&self) -> &[AttachedResourceField] {
        match self {
            Self::Missing(_) => &[],
            Self::Braced { fields, .. } => fields,
        }
    }

    pub const fn is_missing(&self) -> bool {
        matches!(self, Self::Missing(_))
    }

    pub fn is_unclosed(&self) -> bool {
        matches!(
            self,
            Self::Braced { close, .. }
                if matches!(close.delimiter_state(), AttachedDelimiterState::Missing(_))
        )
    }
}

/// One generic `res` declaration bound to common type and expression owners.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedResourceDeclaration {
    syntax: AstNode<ResourceDeclarationItemKind>,
    prefix: AttachedItemPrefix,
    public_id: AttachedResourcePublicId,
    name: AttachedRequiredName,
    colon: AttachedRequiredPunctuation,
    resource_type: AttachedTypeRefNode,
    body: AttachedResourceBody,
}

impl AttachedResourceDeclaration {
    pub const fn syntax(&self) -> &AstNode<ResourceDeclarationItemKind> {
        &self.syntax
    }

    pub const fn prefix(&self) -> &AttachedItemPrefix {
        &self.prefix
    }

    pub const fn public_id(&self) -> &AttachedResourcePublicId {
        &self.public_id
    }

    pub const fn name(&self) -> &AttachedRequiredName {
        &self.name
    }

    pub const fn colon(&self) -> &AttachedRequiredPunctuation {
        &self.colon
    }

    pub const fn resource_type(&self) -> &AttachedTypeRefNode {
        &self.resource_type
    }

    pub const fn body(&self) -> &AttachedResourceBody {
        &self.body
    }

    /// Whether the canonical type transaction selected a nominal path head.
    pub fn has_nominal_type_head(&self) -> bool {
        matches!(
            self.resource_type.value(),
            TypeRef::Path(_) | TypeRef::Generic { .. }
        )
    }
}

impl AstNode<ResourceDeclarationItemKind> {
    /// Binds the one-pass `res` grammar without consulting the resource registry.
    pub fn semantics(&self) -> Result<AttachedResourceDeclaration, SyntaxAccessError> {
        let item = TypedItemNode::Resource(self.clone());
        let public_id = self
            .optional_family_child::<ExpressionFamily>(SyntaxRole::PublicId)?
            .map(|syntax| syntax.semantic().and_then(attach_public_id))
            .transpose()?
            .unwrap_or(AttachedResourcePublicId::Absent);
        Ok(AttachedResourceDeclaration {
            syntax: self.clone(),
            prefix: item.attached_prefix()?,
            public_id,
            name: required_name(&self.syntax(), false)?,
            colon: punctuation(&self.required_exact_child::<ColonKind>(SyntaxRole::Colon)?),
            resource_type: required_type(&self.syntax(), SyntaxRole::Type)?,
            body: attach_body(self)?,
        })
    }
}

fn attach_public_id(
    syntax: AttachedExpressionNode,
) -> Result<AttachedResourcePublicId, SyntaxAccessError> {
    let ExpressionProjection::EntityReference(reference) = syntax.projection() else {
        return Err(SyntaxAccessError::InvalidItemProjection { id: syntax.id() });
    };
    let projected = match reference.value() {
        Ok(reference) => match reference.root() {
            AuthoredIdRoot::Absolute { .. } => {
                let spelling = reference
                    .segments()
                    .iter()
                    .map(crate::id_ref::AuthoredIdSegment::as_str)
                    .collect::<Vec<_>>()
                    .join(".");
                PublicId::try_new(spelling).map_err(|_| AttachedResourcePublicIdIssue::Malformed)
            }
            AuthoredIdRoot::Relative { .. } | AuthoredIdRoot::FamilyRelative { .. } => {
                Err(AttachedResourcePublicIdIssue::Relative)
            }
        },
        Err(_) => Err(AttachedResourcePublicIdIssue::Malformed),
    };
    let syntax = Box::new(syntax);
    Ok(match projected {
        Ok(value) => AttachedResourcePublicId::Explicit { syntax, value },
        Err(issue) => AttachedResourcePublicId::Recovered { syntax, issue },
    })
}

fn attach_body(
    owner: &AstNode<ResourceDeclarationItemKind>,
) -> Result<AttachedResourceBody, SyntaxAccessError> {
    let body = owner
        .syntax()
        .optional_unique_child(SyntaxRole::Body)?
        .ok_or(SyntaxAccessError::InvalidItemProjection { id: owner.id() })?;
    match body.kind() {
        SyntaxKind::MissingBody => Ok(AttachedResourceBody::Missing(body.cast()?)),
        SyntaxKind::ResourceBody => {
            let syntax = body.cast::<ResourceBodyKind>()?;
            let fields = syntax
                .syntax()
                .ordered_children(SyntaxRoleClass::Field)?
                .into_iter()
                .map(|field| field.cast::<ResourceFieldInitializerKind>())
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .enumerate()
                .map(|(position, field)| attach_field(owner.id(), position, field))
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice();
            Ok(AttachedResourceBody::Braced {
                open: syntax.required_exact_child::<OpenBraceKind>(SyntaxRole::OpenDelimiter)?,
                close: syntax.required_exact_child::<CloseBraceKind>(SyntaxRole::CloseDelimiter)?,
                syntax,
                fields,
            })
        }
        _ => Err(SyntaxAccessError::InvalidItemProjection { id: owner.id() }),
    }
}

fn attach_field(
    owner: super::SyntaxNodeId,
    position: usize,
    syntax: AstNode<ResourceFieldInitializerKind>,
) -> Result<AttachedResourceField, SyntaxAccessError> {
    let SyntaxRole::Field(source_ordinal) = syntax.role() else {
        return Err(SyntaxAccessError::InvalidItemProjection { id: owner });
    };
    if usize::from(source_ordinal) != position {
        return Err(SyntaxAccessError::InvalidItemProjection { id: owner });
    }
    let initializer = match syntax
        .syntax()
        .optional_unique_child(SyntaxRole::Initializer)?
    {
        None => AttachedResourceInitializer::Absent,
        Some(initializer) if initializer.kind().is_expression() => {
            AttachedResourceInitializer::Authored(Box::new(
                super::expression::AttachedExpressionNode::from_syntax(initializer)?,
            ))
        }
        Some(_) => return Err(SyntaxAccessError::InvalidItemProjection { id: owner }),
    };
    let recoveries = syntax
        .ordered_exact_children::<ErrorNodeKind>(SyntaxRoleClass::Recovery)?
        .into_boxed_slice();
    let name = syntax
        .optional_family_child::<NameFamily>(SyntaxRole::Name)?
        .ok_or(SyntaxAccessError::InvalidItemProjection { id: owner })?;
    let name = required_name(&syntax.syntax(), false).and_then(|attached| {
        (attached.syntax().id() == name.id())
            .then_some(attached)
            .ok_or(SyntaxAccessError::InvalidItemProjection { id: owner })
    })?;
    Ok(AttachedResourceField {
        assignment: punctuation(&syntax.required_exact_child::<EqualsKind>(SyntaxRole::Equals)?),
        syntax,
        source_ordinal,
        name,
        initializer,
        recoveries,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::num::NonZeroU64;
    use std::sync::Arc;

    use arcweft_source::identity::SourceSnapshotId;
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange};

    use super::{
        AstNode, AttachedResourceBody, AttachedResourceInitializer, AttachedResourcePublicId,
        AttachedResourcePublicIdIssue, ResourceDeclarationItemKind,
    };
    use crate::attachment::{
        AttachedTypeFamily, GrammarIdentityMap, SyntaxDatabaseId, SyntaxLineageId, SyntaxNodeId,
        SyntaxSnapshotData, SyntaxSnapshotId, attach_typed_tree,
    };
    use crate::expressions::ExpressionProjection;
    use crate::grammar::kinds::SyntaxKind;
    use crate::parser::{ParseOptions, parse_shadow_document};

    fn attach(text: &str) -> Arc<SyntaxSnapshotData> {
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcw:/resource-attachment-test").unwrap(),
                SourceName::path("resource-attachment-test.arcw"),
                text,
            )
            .unwrap(),
        );
        let build = parse_shadow_document(&document, ParseOptions::default()).unwrap();
        let database = SyntaxDatabaseId::from_raw_for_test(NonZeroU64::new(149).unwrap());
        let lineage = SyntaxLineageId::from_raw_for_test(database, NonZeroU64::new(1).unwrap());
        let snapshot = SyntaxSnapshotId::new(
            lineage,
            SourceSnapshotId::initial(document.display_name().clone()),
        );
        let identities = build
            .index()
            .entries()
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                (
                    entry.path().clone(),
                    SyntaxNodeId::new(
                        lineage,
                        NonZeroU64::new(u64::try_from(index).unwrap() + 1).unwrap(),
                    ),
                )
            })
            .collect::<HashMap<_, _>>();
        attach_typed_tree(
            &build,
            &GrammarIdentityMap::new(identities),
            snapshot,
            document,
        )
        .unwrap()
    }

    fn resources(snapshot: &Arc<SyntaxSnapshotData>) -> Vec<AstNode<ResourceDeclarationItemKind>> {
        snapshot
            .nodes()
            .filter(|node| node.kind() == SyntaxKind::ResourceDeclarationItem)
            .map(|node| node.cast().unwrap())
            .collect()
    }

    #[test]
    fn resource_attachment_owns_prefix_identity_type_and_ordered_initializers() {
        let snapshot = attach(concat!(
            "/// Configured room image\n",
            "#[tool.fixture]\n",
            "pub res @image.room room: std.presentation.Image {\n",
            "    asset = @asset.bg.room\n",
            "    visible = true\n",
            "}\n",
        ));
        let declaration = resources(&snapshot)[0].semantics().unwrap();

        assert_eq!(
            declaration.prefix().documentation().unwrap().markdown(),
            "Configured room image"
        );
        assert_eq!(declaration.prefix().attributes().len(), 1);
        assert!(declaration.prefix().visibility().is_some());
        assert!(matches!(
            declaration.public_id(),
            AttachedResourcePublicId::Explicit { value, .. }
                if value.as_str() == "image.room"
        ));
        assert_eq!(declaration.name().value().unwrap().as_str(), "room");
        assert!(!declaration.colon().is_missing());
        assert_eq!(
            declaration.resource_type().family(),
            AttachedTypeFamily::Path
        );
        assert!(declaration.has_nominal_type_head());
        let fields = declaration.body().fields();
        assert_eq!(fields.len(), 2);
        for (position, field) in fields.iter().enumerate() {
            assert_eq!(usize::from(field.source_ordinal()), position);
            assert!(!field.assignment().is_missing());
            assert!(!field.has_recovery());
        }
        assert!(matches!(
            fields[0].initializer().authored().unwrap().projection(),
            ExpressionProjection::EntityReference(_)
        ));
        assert!(matches!(
            fields[1].initializer().authored().unwrap().projection(),
            ExpressionProjection::Literal(_)
        ));
        assert!(!declaration.body().is_unclosed());
    }

    #[test]
    fn resource_attachment_retains_relative_id_non_nominal_type_and_field_recovery() {
        let source = concat!(
            "res @.room : &Image {\n",
            "    asset @asset.bg.room\n",
            "    opacity =\n",
            "}\n",
        );
        let snapshot = attach(source);
        let declaration = resources(&snapshot)[0].semantics().unwrap();

        assert!(matches!(
            declaration.public_id(),
            AttachedResourcePublicId::Recovered {
                issue: AttachedResourcePublicIdIssue::Relative,
                ..
            }
        ));
        assert!(declaration.name().is_missing());
        assert!(!declaration.has_nominal_type_head());
        let AttachedResourceBody::Braced { fields, .. } = declaration.body() else {
            panic!("resource body must remain typed");
        };
        assert_eq!(fields.len(), 2);
        assert!(fields[0].assignment().is_missing());
        assert!(matches!(
            fields[0].initializer(),
            AttachedResourceInitializer::Absent
        ));
        assert!(fields[0].has_recovery());
        assert!(matches!(
            fields[1].initializer(),
            AttachedResourceInitializer::Authored(initializer)
                if initializer.syntax().kind() == SyntaxKind::MissingExpression
                    && initializer.projection() == &ExpressionProjection::Error
        ));
        let insertion = source.find("opacity =").unwrap() + "opacity =".len();
        assert_eq!(
            fields[1]
                .initializer()
                .authored()
                .unwrap()
                .whole_source_span()
                .range(),
            SourceRange::new(insertion, insertion)
        );
        assert!(fields[1].has_recovery());
    }
}
