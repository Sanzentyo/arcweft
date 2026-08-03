//! Parser-owned semantic Pattern projections bound to attached syntax identity.

use std::sync::Arc;

use arcweft_source::SourceSpan;

use super::family::{FamilyNode, PatternFamily};
use super::{
    AstNode, AttachedTypeRefNode, PatternFragmentRootKind, SyntaxAccessError, SyntaxNodeHandle,
    SyntaxNodeId, SyntaxSnapshotId,
};
use crate::grammar::SyntaxKind;
use crate::patterns::{
    AuthoredPattern, PatternBindingSite, PatternComponentRole, PatternNodePath, PatternNodeStep,
    PatternSyntaxFamily, PatternSyntaxNode, PatternSyntaxState, PatternTypeChildRelation,
};

/// One exact revision-bound component of an attached Pattern node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedPatternComponent {
    role: PatternComponentRole,
    source: SourceSpan,
}

impl AttachedPatternComponent {
    pub const fn role(&self) -> PatternComponentRole {
        self.role
    }

    pub const fn source_span(&self) -> &SourceSpan {
        &self.source
    }
}

/// One typed semantic child projected from an attached Pattern owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedPatternChild {
    Pattern {
        step: PatternNodeStep,
        node: AttachedPatternNode,
    },
    Type {
        relation: PatternTypeChildRelation,
        node: AttachedTypeRefNode,
    },
}

impl AttachedPatternChild {
    pub const fn pattern_step(&self) -> Option<PatternNodeStep> {
        match self {
            Self::Pattern { step, .. } => Some(*step),
            Self::Type { .. } => None,
        }
    }

    pub const fn type_relation(&self) -> Option<PatternTypeChildRelation> {
        match self {
            Self::Pattern { .. } => None,
            Self::Type { relation, .. } => Some(*relation),
        }
    }

    pub const fn pattern(&self) -> Option<&AttachedPatternNode> {
        match self {
            Self::Pattern { node, .. } => Some(node),
            Self::Type { .. } => None,
        }
    }

    pub const fn type_ref(&self) -> Option<&AttachedTypeRefNode> {
        match self {
            Self::Pattern { .. } => None,
            Self::Type { node, .. } => Some(node),
        }
    }
}

/// Typed semantic projection retained by one attached Pattern identity.
#[derive(Clone, Debug)]
pub struct AttachedPatternNode {
    syntax: SyntaxNodeHandle,
    authored: Arc<AuthoredPattern>,
    path: PatternNodePath,
    tree: u64,
}

impl PartialEq for AttachedPatternNode {
    fn eq(&self, other: &Self) -> bool {
        self.syntax == other.syntax && self.path == other.path && self.tree == other.tree
    }
}

impl Eq for AttachedPatternNode {}

impl AttachedPatternNode {
    pub(crate) fn from_syntax(syntax: SyntaxNodeHandle) -> Result<Self, SyntaxAccessError> {
        let projection = syntax
            .pattern_projection()
            .ok_or(SyntaxAccessError::MissingPatternProjection { id: syntax.id() })?;
        let value = projection
            .authored()
            .value_at(projection.path())
            .ok_or(SyntaxAccessError::InvalidPatternProjection { id: syntax.id() })?;
        if !family_accepts_kind(value.family(), syntax.kind()) {
            return Err(SyntaxAccessError::InvalidPatternProjection { id: syntax.id() });
        }
        let authored = Arc::clone(projection.authored());
        let path = projection.path().clone();
        let tree = projection.tree();
        Ok(Self {
            syntax,
            authored,
            path,
            tree,
        })
    }

    pub fn id(&self) -> SyntaxNodeId {
        self.syntax.id()
    }

    pub fn snapshot_id(&self) -> &SyntaxSnapshotId {
        self.syntax.snapshot_id()
    }

    pub fn syntax(&self) -> SyntaxNodeHandle {
        self.syntax.clone()
    }

    pub const fn path(&self) -> &PatternNodePath {
        &self.path
    }

    /// Semantic Pattern payload selected by the authoritative grammar transaction.
    ///
    /// # Panics
    ///
    /// Panics only if the already-validated attached projection is internally
    /// corrupted and its semantic node path no longer exists.
    pub fn value(&self) -> &PatternSyntaxNode {
        self.authored
            .value_at(&self.path)
            .expect("attached semantic Pattern paths are validated at construction")
    }

    pub fn state(&self) -> &PatternSyntaxState {
        self.value().state()
    }

    pub fn family(&self) -> PatternSyntaxFamily {
        self.value().family()
    }

    /// Complete parser-owned binding inventory in deterministic authored
    /// preorder. Reusers validate Local payloads against this one authority.
    pub fn binding_sites(&self) -> &[PatternBindingSite] {
        self.authored.binding_sites()
    }

    /// Returns the required whole-node source component.
    ///
    /// # Panics
    ///
    /// Panics only if the already-validated attached projection is internally
    /// corrupted and no longer owns its required whole source component.
    pub fn whole_source_span(&self) -> SourceSpan {
        self.component(PatternComponentRole::Whole)
            .expect("every attached semantic Pattern owns its whole source")
    }

    pub fn component(&self, role: PatternComponentRole) -> Option<SourceSpan> {
        let range = *self.authored.source().component_at(&self.path, role)?;
        Some(self.syntax.source_span_for_range(range))
    }

    pub fn components(&self) -> Vec<AttachedPatternComponent> {
        self.authored
            .source()
            .components()
            .iter()
            .filter(|component| component.owner() == &self.path)
            .map(|component| AttachedPatternComponent {
                role: component.role(),
                source: self.syntax.source_span_for_range(*component.range()),
            })
            .collect()
    }

    /// Immediate semantic Pattern and typed-binding type children.
    pub fn children(&self) -> Result<Vec<AttachedPatternChild>, SyntaxAccessError> {
        let mut children = self
            .value()
            .immediate_child_steps()
            .into_iter()
            .map(|step| {
                let path = self.path.child(step);
                let syntax = self
                    .syntax
                    .pattern_node_for_projection(self.tree, &path)
                    .ok_or_else(|| SyntaxAccessError::MissingPatternChildProjection {
                        parent: self.id(),
                        step,
                    })?;
                Ok(AttachedPatternChild::Pattern {
                    step,
                    node: Self::from_syntax(syntax)?,
                })
            })
            .collect::<Result<Vec<_>, SyntaxAccessError>>()?;

        if let Some(type_child) = self
            .authored
            .source()
            .type_child_at(&self.path, PatternTypeChildRelation::TypedBinding)
        {
            let syntax = self
                .syntax
                .type_node_for_projection(type_child.tree(), type_child.path())
                .ok_or_else(|| SyntaxAccessError::MissingPatternTypeChildProjection {
                    parent: self.id(),
                    relation: type_child.relation(),
                })?;
            let projection = syntax
                .type_projection()
                .ok_or(SyntaxAccessError::InvalidPatternProjection { id: self.id() })?;
            if projection.tree() != type_child.tree()
                || projection.path() != type_child.path()
                || !Arc::ptr_eq(projection.authored(), type_child.authored())
            {
                return Err(SyntaxAccessError::InvalidPatternProjection { id: self.id() });
            }
            children.push(AttachedPatternChild::Type {
                relation: type_child.relation(),
                node: AttachedTypeRefNode::from_syntax(syntax)?,
            });
        }
        Ok(children)
    }
}

impl FamilyNode<PatternFamily> {
    /// Returns the final semantic Pattern projection owned by this node.
    pub fn semantic(&self) -> Result<AttachedPatternNode, SyntaxAccessError> {
        AttachedPatternNode::from_syntax(self.syntax())
    }
}

impl AstNode<PatternFragmentRootKind> {
    /// Returns the semantic projection of an attached standalone Pattern fragment.
    pub fn semantic(&self) -> Result<AttachedPatternNode, SyntaxAccessError> {
        AttachedPatternNode::from_syntax(self.syntax())
    }
}

pub(crate) const fn family_accepts_kind(family: PatternSyntaxFamily, kind: SyntaxKind) -> bool {
    matches!(
        (family, kind),
        (PatternSyntaxFamily::Binding, SyntaxKind::BindingPattern)
            | (
                PatternSyntaxFamily::MutableBinding,
                SyntaxKind::MutableBindingPattern
            )
            | (PatternSyntaxFamily::Literal, SyntaxKind::LiteralPattern)
            | (
                PatternSyntaxFamily::EntityReference,
                SyntaxKind::EntityReferencePattern
            )
            | (PatternSyntaxFamily::Variant, SyntaxKind::VariantPattern)
            | (PatternSyntaxFamily::Discard, SyntaxKind::WildcardPattern)
            | (PatternSyntaxFamily::Tuple, SyntaxKind::TuplePattern)
            | (PatternSyntaxFamily::Record, SyntaxKind::RecordPattern)
            | (
                PatternSyntaxFamily::BracketSequence,
                SyntaxKind::SequencePattern
            )
            | (
                PatternSyntaxFamily::WholeBinding,
                SyntaxKind::WholeBindingPattern
            )
            | (PatternSyntaxFamily::Or, SyntaxKind::OrPattern)
            | (
                PatternSyntaxFamily::TypedBinding,
                SyntaxKind::TypedBindingPattern
            )
            | (
                PatternSyntaxFamily::Error,
                SyntaxKind::MissingPattern | SyntaxKind::ErrorPattern | SyntaxKind::RestPattern
            )
    )
}
