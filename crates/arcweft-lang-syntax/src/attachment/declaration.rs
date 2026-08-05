//! Snapshot-bound Character declaration semantics.
//!
//! The parser projection is the semantic authority.  Construction here binds
//! that projection to the exact CST descendants and source components once;
//! downstream consumers never scan declaration text or parser diagnostics.

use arcweft_id::PublicId;
use arcweft_source::SourceSpan;

use super::family::NameFamily;
use super::node::{
    CharacterBodyKind, CharacterDeclarationItemKind, CharacterDisplayNameMemberKind,
    CloseBraceKind, DeclarationHeaderKind, DeclarationPublicIdKind, ErrorDeclarationMemberKind,
    ErrorNodeKind, MissingBodyKind, MissingDeclarationIdKind, MissingMemberValueKind,
    SurfaceAliasKind, WrongFamilyReferenceKind,
};
use super::source_file::AttachedDelimiterState;
use super::{
    AstNode, AttachedExpressionNode, AttachedItemPrefix, NameNode, SyntaxAccessError,
    SyntaxNodeHandle, SyntaxNodeId, TypedItemNode,
};
use crate::grammar::declaration_projection::{
    PendingCharacterAssignment, PendingCharacterBodyProjection, PendingCharacterInitializer,
    PendingCharacterMemberProjection, PendingCharacterSurfaceAlias,
    PendingDeclarationHeaderProjection, PendingDeclarationName, PendingDeclarationPublicId,
    PendingDeclarationPublicIdIssue,
};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole, SyntaxRoleClass};
use crate::name::SyntaxName;

/// Resolved or recovered public-ID state shared by declaration headers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedDeclarationPublicId {
    Derived,
    Explicit {
        syntax: AstNode<DeclarationPublicIdKind>,
        value: PublicId,
    },
    Recovered {
        syntax: AstNode<DeclarationPublicIdKind>,
        issue: AttachedDeclarationPublicIdIssue,
    },
}

/// Typed declaration-public-ID recovery selected by the parser.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedDeclarationPublicIdIssue {
    WrongFamily(PublicId),
    Malformed,
    Missing,
}

/// Shared declaration identity bound to one exact public-ID child.
///
/// Callable declarations do not have the retained declaration-header wrapper,
/// but their authored identity uses the same parser-selected public-ID state.
/// Keeping this small identity owner lets proof attachment consume the shared
/// projection without reconstructing the ID from source text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedDeclarationIdentity {
    public_id: AttachedDeclarationPublicId,
}

impl AttachedDeclarationIdentity {
    pub const fn public_id(&self) -> &AttachedDeclarationPublicId {
        &self.public_id
    }
}

/// Resolved or recovered declaration name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedRetainedName {
    Resolved { syntax: NameNode, value: SyntaxName },
    Missing { syntax: NameNode },
    Invalid { syntax: NameNode },
}

/// Header semantics shared by retained declaration producers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedRetainedHeader {
    syntax: AstNode<DeclarationHeaderKind>,
    public_id: AttachedDeclarationPublicId,
    name: AttachedRetainedName,
}

impl AttachedRetainedHeader {
    pub const fn syntax(&self) -> &AstNode<DeclarationHeaderKind> {
        &self.syntax
    }

    pub const fn public_id(&self) -> &AttachedDeclarationPublicId {
        &self.public_id
    }

    pub const fn name(&self) -> &AttachedRetainedName {
        &self.name
    }
}

/// Optional Character surface alias with typed missing-name recovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedCharacterSurfaceAlias {
    Absent,
    Resolved {
        syntax: AstNode<SurfaceAliasKind>,
        name: NameNode,
        value: SyntaxName,
    },
    Missing {
        syntax: AstNode<SurfaceAliasKind>,
        name: NameNode,
    },
}

/// Exact authored assignment token or its parser-owned insertion site.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedCharacterAssignment {
    Authored(SourceSpan),
    Missing(SourceSpan),
}

impl AttachedCharacterAssignment {
    pub const fn source_span(&self) -> &SourceSpan {
        match self {
            Self::Authored(source) | Self::Missing(source) => source,
        }
    }

    pub const fn is_missing(&self) -> bool {
        matches!(self, Self::Missing(_))
    }
}

/// Display-name initializer retained without source rediscovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedCharacterInitializer {
    Authored(AttachedExpressionNode),
    Missing(AstNode<MissingMemberValueKind>),
}

/// One typed Character display-name member.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedCharacterDisplayNameMember {
    syntax: AstNode<CharacterDisplayNameMemberKind>,
    source_ordinal: u16,
    name: SourceSpan,
    duplicate: bool,
    assignment: AttachedCharacterAssignment,
    initializer: AttachedCharacterInitializer,
}

impl AttachedCharacterDisplayNameMember {
    pub const fn syntax(&self) -> &AstNode<CharacterDisplayNameMemberKind> {
        &self.syntax
    }

    pub const fn source_ordinal(&self) -> u16 {
        self.source_ordinal
    }

    pub const fn name(&self) -> &SourceSpan {
        &self.name
    }

    pub const fn is_duplicate(&self) -> bool {
        self.duplicate
    }

    pub const fn assignment(&self) -> &AttachedCharacterAssignment {
        &self.assignment
    }

    pub const fn initializer(&self) -> &AttachedCharacterInitializer {
        &self.initializer
    }
}

/// Closed Character member inventory in exact source order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedCharacterMember {
    DisplayName(AttachedCharacterDisplayNameMember),
    Recovery {
        source_ordinal: u16,
        syntax: AstNode<ErrorDeclarationMemberKind>,
    },
}

impl AttachedCharacterMember {
    pub const fn source_ordinal(&self) -> u16 {
        match self {
            Self::DisplayName(member) => member.source_ordinal(),
            Self::Recovery { source_ordinal, .. } => *source_ordinal,
        }
    }
}

/// Missing or authored Character body with exact close/member state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedCharacterBody {
    Missing(AstNode<MissingBodyKind>),
    Braced {
        syntax: AstNode<CharacterBodyKind>,
        close: AstNode<CloseBraceKind>,
        members: Box<[AttachedCharacterMember]>,
    },
}

impl AttachedCharacterBody {
    pub fn members(&self) -> &[AttachedCharacterMember] {
        match self {
            Self::Missing(_) => &[],
            Self::Braced { members, .. } => members,
        }
    }

    pub fn is_missing_or_unclosed(&self) -> bool {
        match self {
            Self::Missing(_) => true,
            Self::Braced { close, .. } => {
                matches!(close.delimiter_state(), AttachedDelimiterState::Missing(_))
            }
        }
    }
}

/// Fully bound Character declaration selected by one parser transaction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedCharacterDeclaration {
    syntax: AstNode<CharacterDeclarationItemKind>,
    prefix: AttachedItemPrefix,
    header: AttachedRetainedHeader,
    surface_alias: AttachedCharacterSurfaceAlias,
    body: AttachedCharacterBody,
    unexpected_header: bool,
    trailing_syntax: bool,
}

impl AttachedCharacterDeclaration {
    pub const fn syntax(&self) -> &AstNode<CharacterDeclarationItemKind> {
        &self.syntax
    }

    pub const fn prefix(&self) -> &AttachedItemPrefix {
        &self.prefix
    }

    pub const fn header(&self) -> &AttachedRetainedHeader {
        &self.header
    }

    pub const fn surface_alias(&self) -> &AttachedCharacterSurfaceAlias {
        &self.surface_alias
    }

    pub const fn body(&self) -> &AttachedCharacterBody {
        &self.body
    }

    pub const fn has_unexpected_header(&self) -> bool {
        self.unexpected_header
    }

    pub const fn has_trailing_syntax(&self) -> bool {
        self.trailing_syntax
    }
}

impl AstNode<CharacterDeclarationItemKind> {
    /// Binds the sole parser-owned Character projection to exact descendants.
    pub fn semantics(&self) -> Result<AttachedCharacterDeclaration, SyntaxAccessError> {
        let item = TypedItemNode::Character(self.clone());
        let pending = self
            .syntax()
            .character_projection()
            .cloned()
            .ok_or(SyntaxAccessError::MissingCharacterProjection { id: self.id() })?;
        let header_syntax =
            self.required_exact_child::<DeclarationHeaderKind>(SyntaxRole::Element(0))?;
        let header = header_syntax.retained_semantics()?;
        let alias_node =
            header_syntax.optional_exact_child::<SurfaceAliasKind>(SyntaxRole::Alias)?;
        let surface_alias = attach_alias(self.id(), alias_node, pending.surface_alias())?;
        let body_node = self
            .syntax()
            .optional_unique_child(SyntaxRole::Body)?
            .ok_or(SyntaxAccessError::InvalidCharacterProjection { id: self.id() })?;
        let body = attach_body(self.id(), body_node, pending.body())?;
        let recoveries = self.ordered_exact_children::<ErrorNodeKind>(SyntaxRoleClass::Recovery)?;
        let expected_recoveries = if pending.has_unexpected_header() {
            1
        } else {
            0
        } + if pending.has_trailing_syntax() { 1 } else { 0 };
        if recoveries.len() != expected_recoveries {
            return Err(SyntaxAccessError::InvalidCharacterProjection { id: self.id() });
        }

        Ok(AttachedCharacterDeclaration {
            syntax: self.clone(),
            prefix: item.attached_prefix()?,
            header,
            surface_alias,
            body,
            unexpected_header: pending.has_unexpected_header(),
            trailing_syntax: pending.has_trailing_syntax(),
        })
    }
}

impl AstNode<DeclarationHeaderKind> {
    /// Binds the parser-selected retained identity to this exact header.
    pub fn retained_semantics(&self) -> Result<AttachedRetainedHeader, SyntaxAccessError> {
        let pending = self
            .syntax()
            .declaration_header_projection()
            .cloned()
            .ok_or(SyntaxAccessError::MissingDeclarationHeaderProjection { id: self.id() })?;
        let public_id = attach_public_id(
            self.id(),
            self.optional_exact_child::<DeclarationPublicIdKind>(SyntaxRole::PublicId)?,
            pending.public_id(),
        )?;
        let name_node = self
            .optional_family_child::<NameFamily>(SyntaxRole::Name)?
            .ok_or(SyntaxAccessError::InvalidDeclarationHeaderProjection { id: self.id() })?;
        let name = attach_name(self.id(), self, name_node, pending.name())?;
        Ok(AttachedRetainedHeader {
            syntax: self.clone(),
            public_id,
            name,
        })
    }
}

pub(super) fn attach_declaration_identity(
    owner: &SyntaxNodeHandle,
    pending: &PendingDeclarationHeaderProjection,
) -> Result<AttachedDeclarationIdentity, SyntaxAccessError> {
    let public_id = attach_public_id(
        owner.id(),
        owner
            .optional_unique_child(SyntaxRole::PublicId)?
            .map(|syntax| syntax.cast())
            .transpose()?,
        pending.public_id(),
    )?;
    Ok(AttachedDeclarationIdentity { public_id })
}

fn attach_public_id(
    owner: SyntaxNodeId,
    syntax: Option<AstNode<DeclarationPublicIdKind>>,
    pending: &PendingDeclarationPublicId,
) -> Result<AttachedDeclarationPublicId, SyntaxAccessError> {
    match (pending, syntax) {
        (PendingDeclarationPublicId::Derived, None) => Ok(AttachedDeclarationPublicId::Derived),
        (PendingDeclarationPublicId::Explicit { value, source }, Some(syntax)) => {
            validate_retained_range(owner, syntax.range(), *source)?;
            if !syntax.syntax().children().is_empty() {
                return Err(SyntaxAccessError::InvalidDeclarationHeaderProjection { id: owner });
            }
            Ok(AttachedDeclarationPublicId::Explicit {
                syntax,
                value: value.clone(),
            })
        }
        (PendingDeclarationPublicId::Recovered { issue, source }, Some(syntax)) => {
            validate_retained_range(owner, syntax.range(), *source)?;
            let issue = match issue {
                PendingDeclarationPublicIdIssue::WrongFamily(value) => {
                    let child = syntax.required_exact_child::<WrongFamilyReferenceKind>(
                        SyntaxRole::Reference(0),
                    )?;
                    validate_retained_range(owner, child.range(), *source)?;
                    if syntax.syntax().children().len() != 1 {
                        return Err(SyntaxAccessError::InvalidDeclarationHeaderProjection {
                            id: owner,
                        });
                    }
                    AttachedDeclarationPublicIdIssue::WrongFamily(value.clone())
                }
                PendingDeclarationPublicIdIssue::Malformed => {
                    let child =
                        syntax.required_exact_child::<ErrorNodeKind>(SyntaxRole::Recovery(0))?;
                    validate_retained_range(owner, child.range(), *source)?;
                    if syntax.syntax().children().len() != 1 {
                        return Err(SyntaxAccessError::InvalidDeclarationHeaderProjection {
                            id: owner,
                        });
                    }
                    AttachedDeclarationPublicIdIssue::Malformed
                }
                PendingDeclarationPublicIdIssue::Missing => {
                    let child = syntax.required_exact_child::<MissingDeclarationIdKind>(
                        SyntaxRole::Recovery(0),
                    )?;
                    validate_retained_range(owner, child.range(), *source)?;
                    if syntax.syntax().children().len() != 1 {
                        return Err(SyntaxAccessError::InvalidDeclarationHeaderProjection {
                            id: owner,
                        });
                    }
                    AttachedDeclarationPublicIdIssue::Missing
                }
            };
            Ok(AttachedDeclarationPublicId::Recovered { syntax, issue })
        }
        _ => Err(SyntaxAccessError::InvalidDeclarationHeaderProjection { id: owner }),
    }
}

fn validate_exact_range(
    owner: SyntaxNodeId,
    actual: arcweft_source::SourceRange,
    expected: arcweft_source::SourceRange,
) -> Result<(), SyntaxAccessError> {
    (actual == expected)
        .then_some(())
        .ok_or(SyntaxAccessError::InvalidCharacterProjection { id: owner })
}

fn validate_retained_range(
    owner: SyntaxNodeId,
    actual: arcweft_source::SourceRange,
    expected: arcweft_source::SourceRange,
) -> Result<(), SyntaxAccessError> {
    (actual == expected)
        .then_some(())
        .ok_or(SyntaxAccessError::InvalidDeclarationHeaderProjection { id: owner })
}

fn attach_name(
    owner: SyntaxNodeId,
    header: &AstNode<DeclarationHeaderKind>,
    syntax: NameNode,
    pending: &PendingDeclarationName,
) -> Result<AttachedRetainedName, SyntaxAccessError> {
    match pending {
        PendingDeclarationName::Resolved { value, source }
            if syntax.kind() == SyntaxKind::NameDefinition =>
        {
            validate_retained_range(owner, syntax.range(), *source)?;
            if header
                .optional_exact_child::<ErrorNodeKind>(SyntaxRole::Recovery(0))?
                .is_some()
            {
                return Err(SyntaxAccessError::InvalidDeclarationHeaderProjection { id: owner });
            }
            Ok(AttachedRetainedName::Resolved {
                syntax,
                value: value.clone(),
            })
        }
        PendingDeclarationName::Missing { insertion }
            if syntax.kind() == SyntaxKind::MissingName =>
        {
            validate_retained_range(owner, syntax.range(), *insertion)?;
            if header
                .optional_exact_child::<ErrorNodeKind>(SyntaxRole::Recovery(0))?
                .is_some()
            {
                return Err(SyntaxAccessError::InvalidDeclarationHeaderProjection { id: owner });
            }
            Ok(AttachedRetainedName::Missing { syntax })
        }
        PendingDeclarationName::Invalid {
            insertion,
            recovery,
        } if syntax.kind() == SyntaxKind::MissingName => {
            validate_retained_range(owner, syntax.range(), *insertion)?;
            let error = header.required_exact_child::<ErrorNodeKind>(SyntaxRole::Recovery(0))?;
            validate_retained_range(owner, error.range(), *recovery)?;
            Ok(AttachedRetainedName::Invalid { syntax })
        }
        _ => Err(SyntaxAccessError::InvalidDeclarationHeaderProjection { id: owner }),
    }
}

fn attach_alias(
    owner: SyntaxNodeId,
    syntax: Option<AstNode<SurfaceAliasKind>>,
    pending: &PendingCharacterSurfaceAlias,
) -> Result<AttachedCharacterSurfaceAlias, SyntaxAccessError> {
    match (pending, syntax) {
        (PendingCharacterSurfaceAlias::Absent, None) => Ok(AttachedCharacterSurfaceAlias::Absent),
        (PendingCharacterSurfaceAlias::Resolved { value, source }, Some(syntax)) => {
            let name = syntax.required_family_child::<NameFamily>(SyntaxRole::Name)?;
            if name.kind() != SyntaxKind::NameDefinition {
                return Err(SyntaxAccessError::InvalidCharacterProjection { id: owner });
            }
            validate_exact_range(owner, name.range(), *source)?;
            Ok(AttachedCharacterSurfaceAlias::Resolved {
                syntax,
                name,
                value: value.clone(),
            })
        }
        (PendingCharacterSurfaceAlias::Missing { insertion }, Some(syntax)) => {
            let name = syntax.required_family_child::<NameFamily>(SyntaxRole::Name)?;
            if name.kind() != SyntaxKind::MissingName {
                return Err(SyntaxAccessError::InvalidCharacterProjection { id: owner });
            }
            validate_exact_range(owner, name.range(), *insertion)?;
            Ok(AttachedCharacterSurfaceAlias::Missing { syntax, name })
        }
        _ => Err(SyntaxAccessError::InvalidCharacterProjection { id: owner }),
    }
}

fn attach_body(
    owner: SyntaxNodeId,
    syntax: super::SyntaxNodeHandle,
    pending: &PendingCharacterBodyProjection,
) -> Result<AttachedCharacterBody, SyntaxAccessError> {
    match pending {
        PendingCharacterBodyProjection::Missing if syntax.kind() == SyntaxKind::MissingBody => {
            Ok(AttachedCharacterBody::Missing(syntax.cast()?))
        }
        PendingCharacterBodyProjection::Braced { closed, members }
            if syntax.kind() == SyntaxKind::CharacterBody =>
        {
            let body = syntax.cast::<CharacterBodyKind>()?;
            let close = body.required_exact_child::<CloseBraceKind>(SyntaxRole::CloseDelimiter)?;
            if *closed != matches!(close.delimiter_state(), AttachedDelimiterState::Authored(_)) {
                return Err(SyntaxAccessError::InvalidCharacterProjection { id: owner });
            }
            let syntax_members = body.syntax().ordered_children(SyntaxRoleClass::Member)?;
            if syntax_members.len() != members.len() {
                return Err(SyntaxAccessError::InvalidCharacterProjection { id: owner });
            }
            let members = syntax_members
                .into_iter()
                .zip(members)
                .map(|(syntax, projection)| attach_member(owner, syntax, projection))
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice();
            Ok(AttachedCharacterBody::Braced {
                syntax: body,
                close,
                members,
            })
        }
        _ => Err(SyntaxAccessError::InvalidCharacterProjection { id: owner }),
    }
}

fn attach_member(
    owner: SyntaxNodeId,
    syntax: super::SyntaxNodeHandle,
    pending: &PendingCharacterMemberProjection,
) -> Result<AttachedCharacterMember, SyntaxAccessError> {
    if syntax.role() != SyntaxRole::Member(pending.source_ordinal()) {
        return Err(SyntaxAccessError::InvalidCharacterProjection { id: owner });
    }
    match pending {
        PendingCharacterMemberProjection::Recovery { source_ordinal }
            if syntax.kind() == SyntaxKind::ErrorDeclarationMember =>
        {
            Ok(AttachedCharacterMember::Recovery {
                source_ordinal: *source_ordinal,
                syntax: syntax.cast()?,
            })
        }
        PendingCharacterMemberProjection::DisplayName {
            source_ordinal,
            name,
            duplicate,
            assignment,
            initializer,
        } if syntax.kind() == SyntaxKind::CharacterDisplayNameMember => {
            let member = syntax.cast::<CharacterDisplayNameMemberKind>()?;
            if name.start() < member.range().start() || name.end() > member.range().end() {
                return Err(SyntaxAccessError::InvalidCharacterProjection { id: owner });
            }
            let name = member.syntax().source_span_for_range(*name);
            let duplicate_recovery = member
                .optional_exact_child::<ErrorDeclarationMemberKind>(SyntaxRole::Recovery(0))?
                .is_some();
            if duplicate_recovery != *duplicate {
                return Err(SyntaxAccessError::InvalidCharacterProjection { id: owner });
            }
            let assignment = attach_assignment(owner, &member, *assignment)?;
            let initializer_node = member
                .syntax()
                .optional_unique_child(SyntaxRole::Initializer)?
                .ok_or(SyntaxAccessError::InvalidCharacterProjection { id: owner })?;
            let initializer = match initializer {
                PendingCharacterInitializer::Authored
                    if initializer_node.kind().is_expression() =>
                {
                    AttachedCharacterInitializer::Authored(AttachedExpressionNode::from_syntax(
                        initializer_node,
                    )?)
                }
                PendingCharacterInitializer::Missing
                    if initializer_node.kind() == SyntaxKind::MissingMemberValue =>
                {
                    AttachedCharacterInitializer::Missing(initializer_node.cast()?)
                }
                _ => return Err(SyntaxAccessError::InvalidCharacterProjection { id: owner }),
            };
            Ok(AttachedCharacterMember::DisplayName(
                AttachedCharacterDisplayNameMember {
                    syntax: member,
                    source_ordinal: *source_ordinal,
                    name,
                    duplicate: *duplicate,
                    assignment,
                    initializer,
                },
            ))
        }
        _ => Err(SyntaxAccessError::InvalidCharacterProjection { id: owner }),
    }
}

fn attach_assignment(
    owner: SyntaxNodeId,
    member: &AstNode<CharacterDisplayNameMemberKind>,
    pending: PendingCharacterAssignment,
) -> Result<AttachedCharacterAssignment, SyntaxAccessError> {
    let range = pending.range();
    if range.start() < member.range().start() || range.end() > member.range().end() {
        return Err(SyntaxAccessError::InvalidCharacterProjection { id: owner });
    }
    let source = member.syntax().source_span_for_range(range);
    match pending {
        PendingCharacterAssignment::Authored(_) if !source.range().is_empty() => {
            Ok(AttachedCharacterAssignment::Authored(source))
        }
        PendingCharacterAssignment::Missing(_) if source.range().is_empty() => {
            Ok(AttachedCharacterAssignment::Missing(source))
        }
        _ => Err(SyntaxAccessError::InvalidCharacterProjection { id: owner }),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::num::NonZeroU64;
    use std::sync::Arc;

    use arcweft_source::identity::SourceSnapshotId;
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange};

    use super::{
        AstNode, AttachedCharacterBody, AttachedCharacterInitializer, AttachedCharacterMember,
        AttachedCharacterSurfaceAlias, AttachedDeclarationPublicId,
        AttachedDeclarationPublicIdIssue, AttachedRetainedName, CharacterDeclarationItemKind,
    };
    use crate::attachment::{
        GrammarIdentityMap, SyntaxDatabaseId, SyntaxLineageId, SyntaxNodeId, SyntaxSnapshotData,
        SyntaxSnapshotId, attach_typed_tree,
    };
    use crate::grammar::kinds::SyntaxKind;
    use crate::parser::{ParseOptions, parse_shadow_document};

    fn attach(text: &str) -> Arc<SyntaxSnapshotData> {
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcw:/character-declaration-owner-test").unwrap(),
                SourceName::path("character-declaration-owner-test.arcw"),
                text,
            )
            .unwrap(),
        );
        let build = parse_shadow_document(&document, ParseOptions::default()).unwrap();
        let database = SyntaxDatabaseId::from_raw_for_test(NonZeroU64::new(93).unwrap());
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

    fn characters(
        snapshot: &Arc<SyntaxSnapshotData>,
    ) -> Vec<AstNode<CharacterDeclarationItemKind>> {
        snapshot
            .nodes()
            .filter(|node| node.kind() == SyntaxKind::CharacterDeclarationItem)
            .map(|node| node.cast().unwrap())
            .collect()
    }

    #[test]
    fn attached_character_semantics_bind_canonical_header_alias_and_member() {
        let source = concat!(
            "/// Alice\n",
            "#[tool.fixture]\n",
            "pub character @character.alice Alice as alice {\n",
            "    display_name = \"Alice\"\n",
            "}\n",
        );
        let snapshot = attach(source);
        let declarations = characters(&snapshot);
        let declaration = declarations[0].semantics().unwrap();

        assert_eq!(
            declaration
                .prefix()
                .documentation()
                .expect("Character documentation")
                .markdown(),
            "Alice"
        );
        assert_eq!(declaration.prefix().attributes().len(), 1);
        assert!(matches!(
            declaration
                .prefix()
                .visibility()
                .map(|visibility| visibility.kind()),
            Some(crate::attachment::source_file::AttachedVisibilityKind::Public)
        ));
        match declaration.header().public_id() {
            AttachedDeclarationPublicId::Explicit { syntax, value } => {
                assert_eq!(value.as_str(), "character.alice");
                let start = source.find("@character.alice").unwrap();
                assert_eq!(
                    syntax.range(),
                    SourceRange::new(start, start + "@character.alice".len())
                );
            }
            actual => panic!("unexpected public-ID state: {actual:?}"),
        }
        assert!(matches!(
            declaration.header().name(),
            AttachedRetainedName::Resolved { value, .. } if value.as_str() == "Alice"
        ));
        assert!(matches!(
            declaration.surface_alias(),
            AttachedCharacterSurfaceAlias::Resolved { value, .. } if value.as_str() == "alice"
        ));
        let members = declaration.body().members();
        assert_eq!(members.len(), 1);
        let AttachedCharacterMember::DisplayName(member) = &members[0] else {
            panic!("expected display-name member")
        };
        assert_eq!(member.source_ordinal(), 0);
        assert!(!member.is_duplicate());
        assert!(!member.assignment().is_missing());
        assert!(matches!(
            member.initializer(),
            AttachedCharacterInitializer::Authored(_)
        ));
        assert!(!declaration.body().is_missing_or_unclosed());
    }

    #[test]
    fn attached_character_semantics_distinguish_header_recovery_without_text_reparse() {
        let source = concat!(
            "character @view.alice WrongFamily {}\n",
            "character @.relative Relative {}\n",
            "character @character:malformed Malformed {}\n",
            "character @ MissingId {}\n",
            "character {}\n",
            "character Bad.Name {}\n",
            "character Alias as {}\n",
        );
        let snapshot = attach(source);
        let declarations = characters(&snapshot);
        assert_eq!(declarations.len(), 7);
        let semantics = declarations
            .iter()
            .map(AstNode::<CharacterDeclarationItemKind>::semantics)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert!(matches!(
            semantics[0].header().public_id(),
            AttachedDeclarationPublicId::Recovered {
                issue: AttachedDeclarationPublicIdIssue::WrongFamily(value),
                ..
            } if value.as_str() == "view.alice"
        ));
        assert!(matches!(
            semantics[1].header().public_id(),
            AttachedDeclarationPublicId::Explicit { value, .. }
                if value.as_str() == "character.relative"
        ));
        assert!(matches!(
            semantics[2].header().public_id(),
            AttachedDeclarationPublicId::Recovered {
                issue: AttachedDeclarationPublicIdIssue::Malformed,
                ..
            }
        ));
        assert!(matches!(
            semantics[3].header().public_id(),
            AttachedDeclarationPublicId::Recovered {
                issue: AttachedDeclarationPublicIdIssue::Missing,
                ..
            }
        ));
        assert!(matches!(
            semantics[4].header().name(),
            AttachedRetainedName::Missing { .. }
        ));
        assert!(matches!(
            semantics[5].header().name(),
            AttachedRetainedName::Invalid { .. }
        ));
        assert!(matches!(
            semantics[6].surface_alias(),
            AttachedCharacterSurfaceAlias::Missing { .. }
        ));
    }

    #[test]
    fn attached_family_relative_wrong_family_keeps_authored_family_value() {
        let snapshot = attach("character @view:.alice WrongFamily {}\n");
        let declarations = characters(&snapshot);
        let [declaration] = declarations.as_slice() else {
            panic!("expected one character declaration")
        };
        let semantics = declaration
            .semantics()
            .expect("attached character semantics");
        assert!(matches!(
            semantics.header().public_id(),
            AttachedDeclarationPublicId::Recovered {
                issue: AttachedDeclarationPublicIdIssue::WrongFamily(value),
                ..
            } if value.as_str() == "view.alice"
        ));
    }

    #[test]
    fn attached_character_semantics_preserve_recovered_member_ordinals_and_body_state() {
        let source = concat!(
            "character Alice {\n",
            "    display_name \"Alice\"\n",
            "    display_name =\n",
            "    voice = @res.voice\n",
            "}\n",
            "character MissingBody\n",
            "character Unclosed {\n",
            "    display_name = \"Open\"\n",
        );
        let snapshot = attach(source);
        let declarations = characters(&snapshot);
        assert_eq!(declarations.len(), 3);

        let recovered = declarations[0].semantics().unwrap();
        let members = recovered.body().members();
        assert_eq!(members.len(), 3);
        assert_eq!(
            members
                .iter()
                .map(AttachedCharacterMember::source_ordinal)
                .collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
        let AttachedCharacterMember::DisplayName(first) = &members[0] else {
            panic!("expected first display-name member")
        };
        assert!(first.assignment().is_missing());
        assert!(matches!(
            first.initializer(),
            AttachedCharacterInitializer::Authored(_)
        ));
        let AttachedCharacterMember::DisplayName(second) = &members[1] else {
            panic!("expected duplicate display-name member")
        };
        assert!(second.is_duplicate());
        assert!(matches!(
            second.initializer(),
            AttachedCharacterInitializer::Missing(_)
        ));
        assert!(matches!(
            members[2],
            AttachedCharacterMember::Recovery { .. }
        ));

        assert!(matches!(
            declarations[1].semantics().unwrap().body(),
            AttachedCharacterBody::Missing(_)
        ));
        assert!(
            declarations[2]
                .semantics()
                .unwrap()
                .body()
                .is_missing_or_unclosed()
        );
    }
}
