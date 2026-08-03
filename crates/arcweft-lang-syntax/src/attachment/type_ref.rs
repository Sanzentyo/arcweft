//! Semantic type projection bound to attached syntax identity.

use std::sync::Arc;

use arcweft_source::SourceSpan;

use super::family::{FamilyNode, TypeFamily};
use super::{
    AstNode, SyntaxAccessError, SyntaxNodeHandle, SyntaxNodeId, SyntaxSnapshotId,
    TypeFragmentRootKind,
};
use crate::types::{
    AuthoredTypeRef, TypeRef, TypeRefComponentRole, TypeRefNodePath, TypeRefNodeStep,
};

/// Final semantic family of one attached type node.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AttachedTypeFamily {
    Never,
    ConstInt,
    Path,
    Tuple,
    Function,
    Choice,
    Generic,
    TraitBound,
    Projection,
    Reference,
    Slice,
    Recovery,
}

/// One structural child and its snapshot-bound semantic type node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedTypeChild {
    step: TypeRefNodeStep,
    node: AttachedTypeRefNode,
}

/// One typed semantic component projected from the exact attached type node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedTypeComponent {
    role: TypeRefComponentRole,
    source: SourceSpan,
}

impl AttachedTypeComponent {
    /// Semantic source role fixed by the authoritative type grammar.
    pub const fn role(&self) -> TypeRefComponentRole {
        self.role
    }

    /// Exact revision-bound source for this component.
    pub const fn source_span(&self) -> &SourceSpan {
        &self.source
    }
}

impl AttachedTypeChild {
    pub const fn step(&self) -> TypeRefNodeStep {
        self.step
    }

    pub const fn node(&self) -> &AttachedTypeRefNode {
        &self.node
    }
}

/// Typed semantic projection retained by one attached type identity.
#[derive(Clone, Debug)]
pub struct AttachedTypeRefNode {
    syntax: SyntaxNodeHandle,
    authored: Arc<AuthoredTypeRef>,
    path: TypeRefNodePath,
    tree: u64,
}

impl PartialEq for AttachedTypeRefNode {
    fn eq(&self, other: &Self) -> bool {
        self.syntax == other.syntax && self.path == other.path && self.tree == other.tree
    }
}

impl Eq for AttachedTypeRefNode {}

impl AttachedTypeRefNode {
    pub(crate) fn from_syntax(syntax: SyntaxNodeHandle) -> Result<Self, SyntaxAccessError> {
        let projection = syntax
            .type_projection()
            .ok_or(SyntaxAccessError::MissingTypeProjection { id: syntax.id() })?;
        if projection.authored().value_at(projection.path()).is_none() {
            return Err(SyntaxAccessError::InvalidTypeProjection { id: syntax.id() });
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

    pub const fn path(&self) -> &TypeRefNodePath {
        &self.path
    }

    /// Semantic type payload selected by this node.
    ///
    /// # Panics
    ///
    /// Panics only if crate-internal attachment construction bypasses the
    /// validated projection invariant.
    pub fn value(&self) -> &TypeRef {
        self.authored
            .value_at(&self.path)
            .expect("attached semantic type paths are validated at construction")
    }

    pub fn family(&self) -> AttachedTypeFamily {
        match self.value() {
            TypeRef::Never => AttachedTypeFamily::Never,
            TypeRef::ConstInt(_) => AttachedTypeFamily::ConstInt,
            TypeRef::Path(_) => AttachedTypeFamily::Path,
            TypeRef::Tuple(_) => AttachedTypeFamily::Tuple,
            TypeRef::Function { .. } => AttachedTypeFamily::Function,
            TypeRef::Choice(_) => AttachedTypeFamily::Choice,
            TypeRef::Generic { .. } => AttachedTypeFamily::Generic,
            TypeRef::TraitBound(_) => AttachedTypeFamily::TraitBound,
            TypeRef::Projection { .. } => AttachedTypeFamily::Projection,
            TypeRef::Reference(_) => AttachedTypeFamily::Reference,
            TypeRef::Slice(_) => AttachedTypeFamily::Slice,
            TypeRef::Recovery(_) => AttachedTypeFamily::Recovery,
        }
    }

    /// Exact revision-bound source occupied by this semantic node.
    ///
    /// # Panics
    ///
    /// Panics only if crate-internal source-map construction bypasses the
    /// required whole-component invariant.
    pub fn whole_source_span(&self) -> SourceSpan {
        self.component(TypeRefComponentRole::Whole)
            .expect("every validated semantic type node owns its whole source")
    }

    pub fn component(&self, role: TypeRefComponentRole) -> Option<SourceSpan> {
        let range = *self.authored.source().component_at(&self.path, role)?;
        Some(self.syntax.source_span_for_text_range(range))
    }

    /// Complete typed component inventory for this structural type node.
    ///
    /// Entries retain the grammar transaction's canonical role order. No
    /// source text is rescanned and child-node components remain owned by
    /// their own attached projections.
    pub fn components(&self) -> Vec<AttachedTypeComponent> {
        self.authored
            .source()
            .components()
            .iter()
            .filter(|component| component.owner() == &self.path)
            .map(|component| AttachedTypeComponent {
                role: component.role(),
                source: self.syntax.source_span_for_text_range(*component.range()),
            })
            .collect()
    }

    pub fn children(&self) -> Result<Vec<AttachedTypeChild>, SyntaxAccessError> {
        let mut steps = immediate_steps(self.value());
        steps.sort_by_key(|step| {
            let path = self.path.child(*step);
            self.authored
                .source_at(&path)
                .map_or(usize::MAX, |source| source.whole().start())
        });
        steps
            .into_iter()
            .map(|step| {
                let path = self.path.child(step);
                let syntax = self
                    .syntax
                    .type_node_for_projection(self.tree, &path)
                    .ok_or_else(|| SyntaxAccessError::MissingTypeChildProjection {
                        parent: self.id(),
                        step,
                    })?;
                Ok(AttachedTypeChild {
                    step,
                    node: Self::from_syntax(syntax)?,
                })
            })
            .collect()
    }
}

impl FamilyNode<TypeFamily> {
    /// Returns the final semantic type projection owned by this attached node.
    pub fn semantic(&self) -> Result<AttachedTypeRefNode, SyntaxAccessError> {
        AttachedTypeRefNode::from_syntax(self.syntax())
    }
}

impl AstNode<TypeFragmentRootKind> {
    /// Returns the semantic projection of an attached standalone type fragment.
    pub fn semantic(&self) -> Result<AttachedTypeRefNode, SyntaxAccessError> {
        AttachedTypeRefNode::from_syntax(self.syntax())
    }
}

fn immediate_steps(value: &TypeRef) -> Vec<TypeRefNodeStep> {
    match value {
        TypeRef::Tuple(items) => indexed(items.len(), TypeRefNodeStep::TupleItem),
        TypeRef::Function { params, .. } => {
            indexed(params.len(), TypeRefNodeStep::FunctionParameter)
                .into_iter()
                .chain([TypeRefNodeStep::FunctionReturn])
                .collect()
        }
        TypeRef::Choice(items) => indexed(items.len(), TypeRefNodeStep::ChoiceAlternative),
        TypeRef::Generic { args, .. } => indexed(args.len(), TypeRefNodeStep::GenericArgument),
        TypeRef::TraitBound(bound) => indexed(bound.args().len(), TypeRefNodeStep::TraitArgument)
            .into_iter()
            .chain(indexed(
                bound.associated().len(),
                TypeRefNodeStep::AssociatedBinding,
            ))
            .collect(),
        TypeRef::Projection { .. } => vec![TypeRefNodeStep::ProjectionSubject],
        TypeRef::Reference(_) => vec![TypeRefNodeStep::ReferenceReferent],
        TypeRef::Slice(_) => vec![TypeRefNodeStep::SliceItem],
        TypeRef::Never | TypeRef::ConstInt(_) | TypeRef::Path(_) | TypeRef::Recovery(_) => {
            Vec::new()
        }
    }
}

fn indexed(len: usize, step: fn(u16) -> TypeRefNodeStep) -> Vec<TypeRefNodeStep> {
    (0..len)
        .map(|index| {
            step(u16::try_from(index).expect("validated type limits fit structural ordinals"))
        })
        .collect()
}
