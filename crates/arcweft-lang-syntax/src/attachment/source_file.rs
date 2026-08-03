//! Source-file headers and import trees over parser-owned attached projections.

use arcweft_source::{SourceRange, SourceSpan};

use super::access::TypedSyntaxTree;
use super::family::{
    AttributeFamily, AttributeNode, FamilyNode, NameFamily, NameNode, RecoveryFamily, RecoveryNode,
};
use super::item::TypedItemNode;
use super::node::{
    AstNode, CloseBraceKind, ModuleDeclarationKind, NameReferenceKind, OpenBraceKind, PathKind,
    SourceFileKind, UseDeclarationKind, VisibilityKind,
};
use super::{SyntaxAccessError, SyntaxNodeHandle, SyntaxNodeId};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole, SyntaxRoleClass};
use crate::grammar::source_projection::{
    PendingPathRoot, PendingPathSegmentKind, PendingUseGroupMember, PendingUseTreeKind,
    PendingVisibilityKind,
};

/// One identity-bearing child owned directly by a source-file root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceFileEntryNode {
    /// A source-file attribute.
    Attribute(AttributeNode),
    /// One exact source-item family, including Module and Use.
    Item(TypedItemNode),
}

impl SourceFileEntryNode {
    /// Returns the exact attached identity of this source-file child.
    pub fn id(&self) -> SyntaxNodeId {
        match self {
            Self::Attribute(node) => node.id(),
            Self::Item(node) => node.id(),
        }
    }

    /// Returns the exact source revision and range owned by this child.
    pub fn source_span(&self) -> SourceSpan {
        match self {
            Self::Attribute(node) => node.source_span(),
            Self::Item(node) => node.source_span(),
        }
    }
}

impl TypedSyntaxTree {
    /// Returns every identity-bearing source-file child in authored order.
    pub fn entries(&self) -> Result<Vec<SourceFileEntryNode>, SyntaxAccessError> {
        self.root().entries()
    }

    /// Returns source-file attributes in authored order.
    pub fn attributes(&self) -> Result<Vec<AttributeNode>, SyntaxAccessError> {
        self.root().attributes()
    }

    /// Returns every exact source-item family in authored order.
    pub fn items(&self) -> Result<Vec<TypedItemNode>, SyntaxAccessError> {
        self.root().items()
    }
}

impl AstNode<SourceFileKind> {
    /// Source attributes, module declarations, imports, and ordinary items in
    /// authored order. Exact root roles are the classification authority.
    pub fn entries(&self) -> Result<Vec<SourceFileEntryNode>, SyntaxAccessError> {
        self.syntax()
            .children()
            .into_iter()
            .map(|syntax| match syntax.role() {
                SyntaxRole::Attribute(_) => {
                    Ok(SourceFileEntryNode::Attribute(
                        FamilyNode::<AttributeFamily>::new(syntax)?,
                    ))
                }
                role @ (SyntaxRole::Target | SyntaxRole::Reference(_) | SyntaxRole::Element(_)) => {
                    let item = TypedItemNode::from_syntax(syntax)?;
                    let valid_role = match item.kind() {
                        SyntaxKind::ModuleDeclaration => matches!(role, SyntaxRole::Target),
                        SyntaxKind::UseDeclaration => matches!(role, SyntaxRole::Reference(_)),
                        _ => matches!(role, SyntaxRole::Element(_)),
                    };
                    if !valid_role {
                        return Err(SyntaxAccessError::InvalidSourceFileChildRole {
                            parent: self.id(),
                            role,
                        });
                    }
                    Ok(SourceFileEntryNode::Item(item))
                }
                role => Err(SyntaxAccessError::InvalidSourceFileChildRole {
                    parent: self.id(),
                    role,
                }),
            })
            .collect()
    }

    /// Returns direct source-file attributes in authored order.
    pub fn attributes(&self) -> Result<Vec<AttributeNode>, SyntaxAccessError> {
        self.ordered_family_children::<AttributeFamily>(SyntaxRoleClass::Attribute)
    }

    /// Returns direct source items, including Module and Use, in authored order.
    pub fn items(&self) -> Result<Vec<TypedItemNode>, SyntaxAccessError> {
        self.entries()?
            .into_iter()
            .filter_map(|entry| match entry {
                SourceFileEntryNode::Attribute(_) => None,
                SourceFileEntryNode::Item(item) => Some(Ok(item)),
            })
            .collect()
    }
}

/// Root semantics preserved by one complete attached path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedPathRoot {
    /// An unqualified path rooted implicitly at the current crate context.
    ImplicitCrate,
    /// An explicit `crate` root.
    Crate {
        /// Exact `crate` token span.
        source: SourceSpan,
    },
    /// An explicit `self` module root.
    SelfModule {
        /// Exact `self` token span.
        source: SourceSpan,
    },
    /// One or more authored `super` levels.
    Super {
        /// Exact `super` token spans in authored order.
        levels: Box<[SourceSpan]>,
    },
}

impl AttachedPathRoot {
    /// Returns the authored `super` depth when this is a `super` root.
    pub const fn super_depth(&self) -> Option<usize> {
        match self {
            Self::Super { levels } => Some(levels.len()),
            Self::ImplicitCrate | Self::Crate { .. } | Self::SelfModule { .. } => None,
        }
    }
}

/// Parser-validated token family of one ID-less path segment.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AttachedPathSegmentKind {
    /// An ordinary identifier segment.
    Identifier,
    /// A keyword accepted by the path grammar.
    Keyword,
    /// A lifetime-shaped segment accepted by the path grammar.
    Lifetime,
}

/// One source-backed ID-less path segment owned by a complete `Path` identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedPathSegment {
    syntax: SyntaxNodeHandle,
    kind: AttachedPathSegmentKind,
    source: SourceRange,
}

impl AttachedPathSegment {
    /// Returns the parser-validated token family for this segment.
    pub const fn kind(&self) -> AttachedPathSegmentKind {
        self.kind
    }

    /// Returns this segment's exact source span.
    pub fn source_span(&self) -> SourceSpan {
        self.syntax.source_span_for_range(self.source)
    }

    /// Returns this segment's exact authored text without reparsing it.
    pub fn source_text(&self) -> &str {
        self.syntax.source_text_for_range(self.source)
    }
}

/// Typed path projection retained by one immutable `Path` syntax identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedPath {
    syntax: AstNode<PathKind>,
    root: AttachedPathRoot,
    segments: Box<[AttachedPathSegment]>,
    missing_name: Option<NameNode>,
}

impl AttachedPath {
    pub(crate) fn from_syntax(syntax: AstNode<PathKind>) -> Result<Self, SyntaxAccessError> {
        let projection = syntax
            .syntax()
            .path_projection()
            .cloned()
            .ok_or(SyntaxAccessError::MissingPathProjection { id: syntax.id() })?;
        let owner = syntax.range();
        let root = match projection.root() {
            PendingPathRoot::ImplicitCrate => AttachedPathRoot::ImplicitCrate,
            PendingPathRoot::Crate(source) if token_belongs_to(owner, *source) => {
                AttachedPathRoot::Crate {
                    source: syntax.syntax().source_span_for_range(*source),
                }
            }
            PendingPathRoot::SelfModule(source) if token_belongs_to(owner, *source) => {
                AttachedPathRoot::SelfModule {
                    source: syntax.syntax().source_span_for_range(*source),
                }
            }
            PendingPathRoot::Super(levels)
                if !levels.is_empty()
                    && levels.iter().all(|source| token_belongs_to(owner, *source))
                    && levels
                        .windows(2)
                        .all(|pair| pair[0].end() <= pair[1].start()) =>
            {
                AttachedPathRoot::Super {
                    levels: levels
                        .iter()
                        .map(|source| syntax.syntax().source_span_for_range(*source))
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                }
            }
            PendingPathRoot::Crate(_)
            | PendingPathRoot::SelfModule(_)
            | PendingPathRoot::Super(_) => {
                return Err(SyntaxAccessError::InvalidPathProjection { id: syntax.id() });
            }
        };
        let mut previous_end = root_end(projection.root()).unwrap_or(owner.start());
        let mut segments = Vec::with_capacity(projection.segments().len());
        for segment in projection.segments() {
            if !token_belongs_to(owner, segment.source()) || segment.source().start() < previous_end
            {
                return Err(SyntaxAccessError::InvalidPathProjection { id: syntax.id() });
            }
            previous_end = segment.source().end();
            segments.push(AttachedPathSegment {
                syntax: syntax.syntax(),
                kind: match segment.kind() {
                    PendingPathSegmentKind::Identifier => AttachedPathSegmentKind::Identifier,
                    PendingPathSegmentKind::Keyword => AttachedPathSegmentKind::Keyword,
                    PendingPathSegmentKind::Lifetime => AttachedPathSegmentKind::Lifetime,
                },
                source: segment.source(),
            });
        }
        let missing_name = syntax.optional_family_child::<NameFamily>(SyntaxRole::Name)?;
        Ok(Self {
            syntax,
            root,
            segments: segments.into_boxed_slice(),
            missing_name,
        })
    }

    /// Returns the complete identity-bearing `Path` owner.
    pub const fn syntax(&self) -> &AstNode<PathKind> {
        &self.syntax
    }

    /// Returns the root semantics selected by the parser transaction.
    pub const fn root(&self) -> &AttachedPathRoot {
        &self.root
    }

    /// Returns the exact authored root span, or `None` for an implicit crate root.
    pub fn root_source_span(&self) -> Option<SourceSpan> {
        match &self.root {
            AttachedPathRoot::ImplicitCrate => None,
            AttachedPathRoot::Crate { source } | AttachedPathRoot::SelfModule { source } => {
                Some(source.clone())
            }
            AttachedPathRoot::Super { levels } => {
                levels.first().zip(levels.last()).map(|(first, last)| {
                    self.syntax.syntax().source_span_for_range(SourceRange::new(
                        first.range().start(),
                        last.range().end(),
                    ))
                })
            }
        }
    }

    /// Returns ID-less path segments in authored order.
    pub fn segments(&self) -> &[AttachedPathSegment] {
        &self.segments
    }

    /// Returns the typed missing-name recovery child, when present.
    pub const fn missing_name(&self) -> Option<&NameNode> {
        self.missing_name.as_ref()
    }

    /// Returns whether the parser-owned path projection contains recovery.
    pub fn has_recovery(&self) -> bool {
        self.missing_name.is_some()
            || self
                .segments
                .iter()
                .any(|segment| matches!(segment.kind(), AttachedPathSegmentKind::Lifetime))
    }
}

impl AstNode<ModuleDeclarationKind> {
    /// Returns the module declaration's parser-owned attached path.
    pub fn path(&self) -> Result<AttachedPath, SyntaxAccessError> {
        AttachedPath::from_syntax(self.required_exact_child(SyntaxRole::Target)?)
    }
}

/// One structural alias and its identity-bearing name owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedUseAlias {
    source: SourceSpan,
    name: NameNode,
}

impl AttachedUseAlias {
    /// Returns the alias clause's exact source span.
    pub const fn source_span(&self) -> &SourceSpan {
        &self.source
    }

    /// Returns the alias name or typed missing-name recovery owner.
    pub const fn name(&self) -> &NameNode {
        &self.name
    }
}

/// One valid name selection in an attached grouped import.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedUseBinding {
    source: SourceSpan,
    name: AstNode<NameReferenceKind>,
    kind: AttachedPathSegmentKind,
    alias: Option<AttachedUseAlias>,
    recovery: Option<RecoveryNode>,
}

impl AttachedUseBinding {
    /// Returns the grouped binding's exact source span.
    pub const fn source_span(&self) -> &SourceSpan {
        &self.source
    }

    /// Returns the identity-bearing imported name.
    pub const fn name(&self) -> &AstNode<NameReferenceKind> {
        &self.name
    }

    /// Returns the parser-selected token family of the imported name.
    pub const fn kind(&self) -> AttachedPathSegmentKind {
        self.kind
    }

    /// Returns the optional alias owned by this binding.
    pub const fn alias(&self) -> Option<&AttachedUseAlias> {
        self.alias.as_ref()
    }

    /// Returns trailing recovery attached to this binding, when present.
    pub const fn recovery(&self) -> Option<&RecoveryNode> {
        self.recovery.as_ref()
    }
}

/// Ordered grouped-import child, including ordinary grammar recovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedUseGroupChild {
    /// A valid grouped import binding.
    Binding(AttachedUseBinding),
    /// An ordinary grammar recovery member.
    Recovery {
        /// Exact source extent attributed to the recovery member.
        source: SourceSpan,
        /// Identity-bearing recovery node.
        node: RecoveryNode,
    },
}

/// Typed import tree projected from its `UseDeclaration` identity owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedUseTree {
    /// A direct path import with an optional alias.
    Path {
        /// Imported path.
        path: AttachedPath,
        /// Optional alias clause.
        alias: Option<AttachedUseAlias>,
    },
    /// A module glob import with an optional alias.
    Glob {
        /// Module path preceding the glob marker.
        module: AttachedPath,
        /// Exact `*` marker span.
        marker: SourceSpan,
        /// Optional alias clause.
        alias: Option<AttachedUseAlias>,
    },
    /// A grouped import whose children retain authored order.
    Group {
        /// Module path preceding the group.
        module: AttachedPath,
        /// Identity-bearing opening delimiter.
        open: AstNode<OpenBraceKind>,
        /// Imported bindings and recovery members in authored order.
        children: Box<[AttachedUseGroupChild]>,
        /// Identity-bearing closing or missing delimiter.
        close: AstNode<CloseBraceKind>,
    },
}

/// Parser-selected visibility semantics of one attached declaration header.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AttachedVisibilityKind {
    /// Unrestricted `pub` visibility.
    Public,
    /// Crate-scoped `pub(crate)` visibility.
    Crate,
    /// Parent-module `pub(super)` visibility.
    Super,
    /// Invalid scoped visibility retained through ordinary recovery.
    Recovery,
}

/// Typed visibility projection retained by one immutable syntax identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedVisibility {
    syntax: AstNode<VisibilityKind>,
    kind: AttachedVisibilityKind,
}

impl AttachedVisibility {
    /// Returns the identity-bearing visibility syntax owner.
    pub const fn syntax(&self) -> &AstNode<VisibilityKind> {
        &self.syntax
    }

    /// Returns the visibility semantics selected by the parser transaction.
    pub const fn kind(&self) -> AttachedVisibilityKind {
        self.kind
    }
}

impl AstNode<VisibilityKind> {
    /// Returns this visibility's parser-owned semantic projection.
    pub fn semantics(&self) -> Result<AttachedVisibility, SyntaxAccessError> {
        let kind = self
            .syntax()
            .visibility_projection()
            .ok_or(SyntaxAccessError::MissingVisibilityProjection { id: self.id() })?;
        Ok(AttachedVisibility {
            syntax: self.clone(),
            kind: match kind {
                PendingVisibilityKind::Public => AttachedVisibilityKind::Public,
                PendingVisibilityKind::Crate => AttachedVisibilityKind::Crate,
                PendingVisibilityKind::Super => AttachedVisibilityKind::Super,
                PendingVisibilityKind::Recovery => AttachedVisibilityKind::Recovery,
            },
        })
    }
}

/// Source-backed or missing state of an attached closing delimiter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedDelimiterState {
    /// A closing delimiter authored in the accepted source revision.
    Authored(SourceSpan),
    /// A zero-width insertion site retained by ordinary parser recovery.
    Missing(SourceSpan),
}

impl AttachedDelimiterState {
    /// Returns the exact authored extent or missing insertion site.
    pub const fn source_span(&self) -> &SourceSpan {
        match self {
            Self::Authored(source) | Self::Missing(source) => source,
        }
    }
}

impl AstNode<CloseBraceKind> {
    /// Returns whether this group close was authored or recovered as missing.
    pub fn delimiter_state(&self) -> AttachedDelimiterState {
        let source = self.source_span();
        if self.range().is_empty() {
            AttachedDelimiterState::Missing(source)
        } else {
            AttachedDelimiterState::Authored(source)
        }
    }
}

impl AstNode<UseDeclarationKind> {
    /// Returns the optional visibility owned by this use declaration.
    pub fn visibility(&self) -> Result<Option<AttachedVisibility>, SyntaxAccessError> {
        self.optional_exact_child::<VisibilityKind>(SyntaxRole::Visibility)?
            .map(|visibility| visibility.semantics())
            .transpose()
    }

    /// Returns the typed path, glob, or grouped import tree.
    pub fn tree(&self) -> Result<AttachedUseTree, SyntaxAccessError> {
        let projection = self
            .syntax()
            .use_projection()
            .cloned()
            .ok_or(SyntaxAccessError::MissingUseProjection { id: self.id() })?;
        let path = AttachedPath::from_syntax(self.required_exact_child(SyntaxRole::Target)?)?;
        match projection.kind() {
            PendingUseTreeKind::Path => Ok(AttachedUseTree::Path {
                path,
                alias: self.single_alias(projection.aliases())?,
            }),
            PendingUseTreeKind::Glob { marker } if token_belongs_to(self.range(), *marker) => {
                Ok(AttachedUseTree::Glob {
                    module: path,
                    marker: self.syntax().source_span_for_range(*marker),
                    alias: self.single_alias(projection.aliases())?,
                })
            }
            PendingUseTreeKind::Glob { .. } => {
                Err(SyntaxAccessError::InvalidUseProjection { id: self.id() })
            }
            PendingUseTreeKind::Group(members) => {
                if projection.aliases().len()
                    != self
                        .syntax()
                        .ordered_children(SyntaxRoleClass::Field)?
                        .len()
                {
                    return Err(SyntaxAccessError::InvalidUseProjection { id: self.id() });
                }
                Ok(AttachedUseTree::Group {
                    module: path,
                    open: self.required_exact_child(SyntaxRole::OpenDelimiter)?,
                    children: self.group_children(members, projection.aliases())?,
                    close: self.required_exact_child(SyntaxRole::CloseDelimiter)?,
                })
            }
        }
    }

    /// Returns import recovery nodes in parser-authored order.
    pub fn recoveries(&self) -> Result<Vec<RecoveryNode>, SyntaxAccessError> {
        self.ordered_family_children::<RecoveryFamily>(SyntaxRoleClass::Recovery)
    }

    fn single_alias(
        &self,
        aliases: &[crate::grammar::source_projection::PendingUseAlias],
    ) -> Result<Option<AttachedUseAlias>, SyntaxAccessError> {
        let name = self.optional_family_child::<NameFamily>(SyntaxRole::Name)?;
        match (aliases, name) {
            ([], None) => Ok(None),
            ([alias], Some(name)) if component_belongs_to(self.range(), alias.source()) => {
                Ok(Some(AttachedUseAlias {
                    source: self.syntax().source_span_for_range(alias.source()),
                    name,
                }))
            }
            _ => Err(SyntaxAccessError::InvalidUseProjection { id: self.id() }),
        }
    }

    fn group_children(
        &self,
        members: &[PendingUseGroupMember],
        aliases: &[crate::grammar::source_projection::PendingUseAlias],
    ) -> Result<Box<[AttachedUseGroupChild]>, SyntaxAccessError> {
        let names = self.ordered_exact_children::<NameReferenceKind>(SyntaxRoleClass::Element)?;
        let alias_names = self.ordered_family_children::<NameFamily>(SyntaxRoleClass::Field)?;
        let recoveries = self.recoveries()?;
        let mut used_names = vec![false; names.len()];
        let mut used_aliases = vec![false; alias_names.len()];
        let mut children = Vec::with_capacity(members.len());

        for member in members {
            if !component_belongs_to(self.range(), member.source()) {
                return Err(SyntaxAccessError::InvalidUseProjection { id: self.id() });
            }
            if let Some(name_ordinal) = member.name_ordinal() {
                let name_index = usize::try_from(name_ordinal)
                    .map_err(|_| SyntaxAccessError::InvalidUseProjection { id: self.id() })?;
                let Some(name) = names.get(name_index).cloned() else {
                    return Err(SyntaxAccessError::InvalidUseProjection { id: self.id() });
                };
                if core::mem::replace(&mut used_names[name_index], true) {
                    return Err(SyntaxAccessError::InvalidUseProjection { id: self.id() });
                }
                let alias = member
                    .alias_ordinal()
                    .map(|ordinal| {
                        let index = usize::from(ordinal);
                        let projection = aliases
                            .get(index)
                            .ok_or(SyntaxAccessError::InvalidUseProjection { id: self.id() })?;
                        let name = alias_names
                            .get(index)
                            .cloned()
                            .ok_or(SyntaxAccessError::InvalidUseProjection { id: self.id() })?;
                        if core::mem::replace(&mut used_aliases[index], true)
                            || !component_belongs_to(self.range(), projection.source())
                        {
                            return Err(SyntaxAccessError::InvalidUseProjection { id: self.id() });
                        }
                        Ok(AttachedUseAlias {
                            source: self.syntax().source_span_for_range(projection.source()),
                            name,
                        })
                    })
                    .transpose()?;
                let recovery = member
                    .recovery_ordinal()
                    .map(|ordinal| {
                        let index = usize::try_from(ordinal).map_err(|_| {
                            SyntaxAccessError::InvalidUseProjection { id: self.id() }
                        })?;
                        recoveries
                            .get(index)
                            .cloned()
                            .ok_or(SyntaxAccessError::InvalidUseProjection { id: self.id() })
                    })
                    .transpose()?;
                children.push(AttachedUseGroupChild::Binding(AttachedUseBinding {
                    source: self.syntax().source_span_for_range(member.source()),
                    name,
                    kind: match member
                        .name_kind()
                        .ok_or(SyntaxAccessError::InvalidUseProjection { id: self.id() })?
                    {
                        PendingPathSegmentKind::Identifier => AttachedPathSegmentKind::Identifier,
                        PendingPathSegmentKind::Keyword => AttachedPathSegmentKind::Keyword,
                        PendingPathSegmentKind::Lifetime => AttachedPathSegmentKind::Lifetime,
                    },
                    alias,
                    recovery,
                }));
            } else {
                let Some(ordinal) = member.recovery_ordinal() else {
                    return Err(SyntaxAccessError::InvalidUseProjection { id: self.id() });
                };
                let index = usize::try_from(ordinal)
                    .map_err(|_| SyntaxAccessError::InvalidUseProjection { id: self.id() })?;
                let node = recoveries
                    .get(index)
                    .cloned()
                    .ok_or(SyntaxAccessError::InvalidUseProjection { id: self.id() })?;
                children.push(AttachedUseGroupChild::Recovery {
                    source: self.syntax().source_span_for_range(member.source()),
                    node,
                });
            }
        }
        if used_names.iter().any(|used| !used) || used_aliases.iter().any(|used| !used) {
            return Err(SyntaxAccessError::InvalidUseProjection { id: self.id() });
        }
        Ok(children.into_boxed_slice())
    }
}

fn component_belongs_to(owner: SourceRange, component: SourceRange) -> bool {
    component.start() >= owner.start()
        && component.end() <= owner.end()
        && component.start() <= component.end()
}

fn token_belongs_to(owner: SourceRange, token: SourceRange) -> bool {
    component_belongs_to(owner, token) && token.start() < token.end()
}

fn root_end(root: &PendingPathRoot) -> Option<usize> {
    match root {
        PendingPathRoot::ImplicitCrate => None,
        PendingPathRoot::Crate(source) | PendingPathRoot::SelfModule(source) => Some(source.end()),
        PendingPathRoot::Super(levels) => levels.last().map(|source| source.end()),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::num::NonZeroU64;
    use std::sync::Arc;

    use arcweft_source::identity::SourceSnapshotId;
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

    use super::{
        AstNode, AttachedDelimiterState, AttachedPathRoot, AttachedPathSegmentKind,
        AttachedUseGroupChild, AttachedUseTree, AttachedVisibilityKind, ModuleDeclarationKind,
        SourceFileEntryNode, TypedItemNode, TypedSyntaxTree, UseDeclarationKind,
    };
    use crate::attachment::{
        GrammarIdentityMap, SyntaxDatabaseId, SyntaxLineageId, SyntaxNodeId, SyntaxSnapshotId,
        attach_typed_tree,
    };
    use crate::grammar::kinds::{SyntaxKind, SyntaxRole};
    use crate::parser::{ParseOptions, parse_shadow_document};

    fn attach(text: &str) -> Arc<crate::attachment::SyntaxSnapshotData> {
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcw:/source-file-owner-test").unwrap(),
                SourceName::path("source-file-owner-test.arcw"),
                text,
            )
            .unwrap(),
        );
        let build = parse_shadow_document(&document, ParseOptions::default()).unwrap();
        let database = SyntaxDatabaseId::from_raw_for_test(NonZeroU64::new(91).unwrap());
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

    fn module_item(tree: &TypedSyntaxTree) -> AstNode<ModuleDeclarationKind> {
        let mut modules = tree.items().unwrap().into_iter().filter_map(|item| {
            if let TypedItemNode::Module(module) = item {
                Some(module)
            } else {
                None
            }
        });
        let module = modules.next().expect("source file owns one module item");
        assert!(modules.next().is_none(), "source file owns one module item");
        module
    }

    fn use_items(tree: &TypedSyntaxTree) -> Vec<AstNode<UseDeclarationKind>> {
        tree.items()
            .unwrap()
            .into_iter()
            .filter_map(|item| {
                if let TypedItemNode::Use(import) = item {
                    Some(import)
                } else {
                    None
                }
            })
            .collect()
    }

    #[test]
    fn source_file_roles_separate_headers_from_ordinary_items_in_authored_order() {
        let snapshot = attach(concat!(
            "#![allow(unused)]\n",
            "mod crate.game.story\n",
            "pub use self.characters.{alice, bob as narrator}\n",
            "use super.common.route_gate as gate\n",
            "fn next() {}\n",
        ));
        let tree = snapshot.typed_tree().unwrap();
        let entries = tree.entries().unwrap();
        assert!(matches!(
            entries.as_slice(),
            [
                SourceFileEntryNode::Attribute(_),
                SourceFileEntryNode::Item(TypedItemNode::Module(_)),
                SourceFileEntryNode::Item(TypedItemNode::Use(_)),
                SourceFileEntryNode::Item(TypedItemNode::Use(_)),
                SourceFileEntryNode::Item(TypedItemNode::Function(_)),
            ]
        ));
        assert_eq!(entries[0].id().lineage(), entries[4].id().lineage());
        assert_eq!(
            tree.attributes().unwrap()[0].role(),
            SyntaxRole::Attribute(0)
        );
        assert_eq!(tree.items().unwrap()[0].role(), SyntaxRole::Target);
        assert_eq!(tree.items().unwrap()[2].role(), SyntaxRole::Reference(1));
        assert_eq!(tree.items().unwrap()[3].role(), SyntaxRole::Element(0));
        assert_eq!(tree.items().unwrap()[3].kind(), SyntaxKind::FunctionItem);
    }

    #[test]
    fn paths_preserve_parser_selected_roots_and_idless_segments() {
        let snapshot = attach(concat!(
            "mod crate.game.story\n",
            "use self.characters.alice\n",
            "use super.super.common.route_gate\n",
            "use local.item\n",
        ));
        let tree = snapshot.typed_tree().unwrap();
        let module = module_item(&tree).path().unwrap();
        assert!(matches!(module.root(), AttachedPathRoot::Crate { .. }));
        assert_eq!(
            module
                .segments()
                .iter()
                .map(super::AttachedPathSegment::source_text)
                .collect::<Vec<_>>(),
            ["game", "story"]
        );

        let uses = use_items(&tree);
        let paths = uses
            .iter()
            .map(|import| match import.tree().unwrap() {
                AttachedUseTree::Path { path, .. } => path,
                AttachedUseTree::Glob { .. } | AttachedUseTree::Group { .. } => {
                    panic!("path import")
                }
            })
            .collect::<Vec<_>>();
        assert!(matches!(
            paths[0].root(),
            AttachedPathRoot::SelfModule { .. }
        ));
        assert_eq!(paths[1].root().super_depth(), Some(2));
        assert!(matches!(paths[2].root(), AttachedPathRoot::ImplicitCrate));
        assert_eq!(paths[1].segments()[0].source_text(), "common");
    }

    #[test]
    fn use_projection_owns_glob_group_alias_and_recovery_without_range_pairing() {
        let snapshot = attach(concat!(
            "use crate.game.prelude.*\n",
            "use self.characters.{alice, bob as narrator, , carol as}\n",
        ));
        let imports = use_items(&snapshot.typed_tree().unwrap());
        let AttachedUseTree::Glob { marker, alias, .. } = imports[0].tree().unwrap() else {
            panic!("glob import");
        };
        assert_eq!(&snapshot.document().text()[marker.range().as_range()], "*");
        assert!(alias.is_none());

        let AttachedUseTree::Group {
            children,
            open,
            close,
            ..
        } = imports[1].tree().unwrap()
        else {
            panic!("group import");
        };
        assert_eq!(open.source_text(), "{");
        assert_eq!(close.source_text(), "}");
        assert!(matches!(
            close.delimiter_state(),
            AttachedDelimiterState::Authored(_)
        ));
        assert_eq!(children.len(), 4);
        let AttachedUseGroupChild::Binding(bob) = &children[1] else {
            panic!("bob binding");
        };
        assert_eq!(bob.name().source_text(), "bob");
        assert_eq!(bob.alias().unwrap().name().source_text(), "narrator");
        assert!(matches!(
            children[2],
            AttachedUseGroupChild::Recovery { .. }
        ));
        let AttachedUseGroupChild::Binding(carol) = &children[3] else {
            panic!("carol binding");
        };
        assert_eq!(
            carol.alias().unwrap().name().kind(),
            SyntaxKind::MissingName
        );
    }

    #[test]
    fn missing_module_path_keeps_one_path_identity_and_typed_missing_name() {
        let snapshot = attach("mod\n");
        let path = module_item(&snapshot.typed_tree().unwrap()).path().unwrap();
        assert!(matches!(path.root(), AttachedPathRoot::ImplicitCrate));
        assert!(path.segments().is_empty());
        assert_eq!(path.missing_name().unwrap().kind(), SyntaxKind::MissingName);
    }

    #[test]
    fn explicit_root_only_paths_keep_one_owner_and_parent_normalizes_once() {
        let snapshot = attach(concat!(
            "mod crate\n",
            "use self\n",
            "use super\n",
            "use parent\n",
            "use parent.parent\n",
        ));
        let tree = snapshot.typed_tree().unwrap();
        let module = module_item(&tree).path().unwrap();
        assert!(matches!(module.root(), AttachedPathRoot::Crate { .. }));
        assert!(module.segments().is_empty());
        assert!(module.missing_name().is_some());

        let uses = use_items(&tree);
        let paths = uses
            .iter()
            .map(|import| match import.tree().unwrap() {
                AttachedUseTree::Path { path, .. } => path,
                AttachedUseTree::Glob { .. } | AttachedUseTree::Group { .. } => {
                    panic!("path import")
                }
            })
            .collect::<Vec<_>>();
        assert!(matches!(
            paths[0].root(),
            AttachedPathRoot::SelfModule { .. }
        ));
        assert_eq!(paths[1].root().super_depth(), Some(1));
        assert_eq!(paths[2].root().super_depth(), Some(1));
        assert!(paths[..3].iter().all(|path| path.missing_name().is_some()));
        assert_eq!(paths[3].root().super_depth(), Some(1));
        assert_eq!(paths[3].segments().len(), 1);
        assert_eq!(paths[3].segments()[0].source_text(), "parent");
        assert_ne!(paths[2].syntax().id(), paths[3].syntax().id());
    }

    #[test]
    fn visibility_group_member_kind_and_missing_close_are_direct_typed_state() {
        let snapshot = attach(concat!(
            "pub use crate.public\n",
            "pub(crate) use crate.internal\n",
            "pub(super) use crate.parent\n",
            "pub(other) use crate.invalid\n",
            "use crate.members.{alice, self, 'scope}\n",
        ));
        let imports = use_items(&snapshot.typed_tree().unwrap());
        assert_eq!(
            imports[0].visibility().unwrap().unwrap().kind(),
            AttachedVisibilityKind::Public
        );
        assert_eq!(
            imports[1].visibility().unwrap().unwrap().kind(),
            AttachedVisibilityKind::Crate
        );
        assert_eq!(
            imports[2].visibility().unwrap().unwrap().kind(),
            AttachedVisibilityKind::Super
        );
        assert_eq!(
            imports[3].visibility().unwrap().unwrap().kind(),
            AttachedVisibilityKind::Recovery
        );
        assert!(
            imports[0].visibility().unwrap().unwrap().syntax().id()
                != imports[1].visibility().unwrap().unwrap().syntax().id()
        );

        let AttachedUseTree::Group {
            children, close, ..
        } = imports[4].tree().unwrap()
        else {
            panic!("group import")
        };
        let kinds = children
            .iter()
            .map(|child| match child {
                AttachedUseGroupChild::Binding(binding) => binding.kind(),
                AttachedUseGroupChild::Recovery { .. } => panic!("binding"),
            })
            .collect::<Vec<_>>();
        assert_eq!(
            kinds,
            [
                AttachedPathSegmentKind::Identifier,
                AttachedPathSegmentKind::Keyword,
                AttachedPathSegmentKind::Lifetime,
            ]
        );
        assert!(matches!(
            close.delimiter_state(),
            AttachedDelimiterState::Authored(_)
        ));

        let missing = attach("use crate.members.{alice\n");
        let import = use_items(&missing.typed_tree().unwrap()).remove(0);
        let AttachedUseTree::Group { close, .. } = import.tree().unwrap() else {
            panic!("group import")
        };
        let state = close.delimiter_state();
        assert!(matches!(state, AttachedDelimiterState::Missing(_)));
        assert!(state.source_span().range().is_empty());
    }
}
