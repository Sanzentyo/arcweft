//! Immutable grammar snapshot data and qualified session identities.

use core::num::NonZeroU64;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceRange, SourceSpan};

use super::error::SyntaxLookupError;
use super::{AstKind, AstNode, ExactAstKind};
use crate::ast::common::TextRange;
use crate::expressions::PendingExpressionProjection;
use crate::grammar::assertion_projection::PendingAssertionProjection;
use crate::grammar::attribute_projection::PendingOuterAttributeProjection;
use crate::grammar::build::GrammarEventPath;
use crate::grammar::callable_projection::PendingMethodReceiverProjection;
use crate::grammar::contract_projection::PendingFlowContractClauseProjection;
use crate::grammar::declaration_projection::{
    PendingCharacterDeclarationProjection, PendingLayerDeclarationProjection,
    PendingRetainedHeaderProjection,
};
use crate::grammar::entry_projection::PendingEntryDeclarationProjection;
use crate::grammar::event::{PendingPatternProjection, PendingTypeProjection};
use crate::grammar::flow_projection::PendingFlowDeclarationProjection;
use crate::grammar::keyword_statement_projection::PendingKeywordStatementProjection;
use crate::grammar::kinds::{AstTag, SyntaxKind, SyntaxRole};
use crate::grammar::source_declaration_projection::PendingSourceDeclarationProjection;
use crate::grammar::source_projection::{
    PendingPathProjection, PendingUseProjection, PendingVisibilityKind,
};
use crate::grammar::style_projection::PendingStyleDeclarationProjection;
use crate::grammar::test_projection::PendingTestKindProjection;
use crate::grammar::view_projection::PendingViewExportProjection;
use crate::patterns::PatternNodePath;
use crate::types::TypeRefNodePath;

static NEXT_DATABASE_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SyntaxDatabaseId(NonZeroU64);

impl SyntaxDatabaseId {
    pub(crate) fn allocate() -> Option<Self> {
        NEXT_DATABASE_ID
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .ok()
            .and_then(NonZeroU64::new)
            .map(Self)
    }

    #[cfg(test)]
    pub(crate) const fn from_raw_for_test(raw: NonZeroU64) -> Self {
        Self(raw)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SyntaxLineageId {
    database: SyntaxDatabaseId,
    ordinal: NonZeroU64,
}

impl SyntaxLineageId {
    pub(crate) const fn new(database: SyntaxDatabaseId, ordinal: NonZeroU64) -> Self {
        Self { database, ordinal }
    }

    pub const fn database(self) -> SyntaxDatabaseId {
        self.database
    }

    #[cfg(test)]
    pub(crate) const fn from_raw_for_test(database: SyntaxDatabaseId, ordinal: NonZeroU64) -> Self {
        Self::new(database, ordinal)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SyntaxSnapshotId {
    lineage: SyntaxLineageId,
    source: SourceSnapshotId,
}

impl SyntaxSnapshotId {
    pub(crate) const fn new(lineage: SyntaxLineageId, source: SourceSnapshotId) -> Self {
        Self { lineage, source }
    }

    pub const fn lineage(&self) -> SyntaxLineageId {
        self.lineage
    }

    pub const fn source(&self) -> &SourceSnapshotId {
        &self.source
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SyntaxNodeId {
    lineage: SyntaxLineageId,
    slot: NonZeroU64,
}

impl SyntaxNodeId {
    pub(crate) const fn new(lineage: SyntaxLineageId, slot: NonZeroU64) -> Self {
        Self { lineage, slot }
    }

    pub const fn lineage(self) -> SyntaxLineageId {
        self.lineage
    }

    pub(crate) const fn slot(self) -> NonZeroU64 {
        self.slot
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxLanguage {}

impl rowan::Language for SyntaxLanguage {
    type Kind = rowan::SyntaxKind;

    fn kind_from_raw(raw: rowan::SyntaxKind) -> Self::Kind {
        raw
    }

    fn kind_to_raw(kind: Self::Kind) -> rowan::SyntaxKind {
        kind
    }
}

/// Raw Rowan node retained by one attached syntax snapshot.
pub type SyntaxNode = rowan::SyntaxNode<SyntaxLanguage>;

#[derive(Clone, Debug)]
pub(crate) struct AttachedNodeRecord {
    kind: SyntaxKind,
    tag: AstTag,
    role: SyntaxRole,
    path: GrammarEventPath,
    range: SourceRange,
    parent: Option<SyntaxNodeId>,
    children: Box<[SyntaxNodeId]>,
    children_by_role: BTreeMap<SyntaxRole, Box<[SyntaxNodeId]>>,
    expression_projection: Option<PendingExpressionProjection>,
    assertion_projection: Option<PendingAssertionProjection>,
    keyword_statement_projection: Option<PendingKeywordStatementProjection>,
    type_projection: Option<PendingTypeProjection>,
    pattern_projection: Option<PendingPatternProjection>,
    path_projection: Option<PendingPathProjection>,
    use_projection: Option<PendingUseProjection>,
    visibility_projection: Option<PendingVisibilityKind>,
    attribute_projection: Option<PendingOuterAttributeProjection>,
    retained_header_projection: Option<PendingRetainedHeaderProjection>,
    character_projection: Option<PendingCharacterDeclarationProjection>,
    test_kind_projection: Option<PendingTestKindProjection>,
    layer_projection: Option<PendingLayerDeclarationProjection>,
    entry_projection: Option<PendingEntryDeclarationProjection>,
    style_projection: Option<PendingStyleDeclarationProjection>,
    source_declaration_projection: Option<PendingSourceDeclarationProjection>,
    method_receiver_projection: Option<PendingMethodReceiverProjection>,
    contract_clause_projection: Option<PendingFlowContractClauseProjection>,
    flow_declaration_projection: Option<PendingFlowDeclarationProjection>,
    view_export_projection: Option<PendingViewExportProjection>,
}

#[derive(Clone, Debug)]
pub(super) struct AttachedNodeRecordParts {
    pub(super) kind: SyntaxKind,
    pub(super) tag: AstTag,
    pub(super) role: SyntaxRole,
    pub(super) path: GrammarEventPath,
    pub(super) range: SourceRange,
    pub(super) parent: Option<SyntaxNodeId>,
    pub(super) children: Box<[SyntaxNodeId]>,
    pub(super) children_by_role: BTreeMap<SyntaxRole, Box<[SyntaxNodeId]>>,
    pub(super) expression_projection: Option<PendingExpressionProjection>,
    pub(super) assertion_projection: Option<PendingAssertionProjection>,
    pub(super) keyword_statement_projection: Option<PendingKeywordStatementProjection>,
    pub(super) type_projection: Option<PendingTypeProjection>,
    pub(super) pattern_projection: Option<PendingPatternProjection>,
    pub(super) path_projection: Option<PendingPathProjection>,
    pub(super) use_projection: Option<PendingUseProjection>,
    pub(super) visibility_projection: Option<PendingVisibilityKind>,
    pub(super) attribute_projection: Option<PendingOuterAttributeProjection>,
    pub(super) retained_header_projection: Option<PendingRetainedHeaderProjection>,
    pub(super) character_projection: Option<PendingCharacterDeclarationProjection>,
    pub(super) test_kind_projection: Option<PendingTestKindProjection>,
    pub(super) layer_projection: Option<PendingLayerDeclarationProjection>,
    pub(super) entry_projection: Option<PendingEntryDeclarationProjection>,
    pub(super) style_projection: Option<PendingStyleDeclarationProjection>,
    pub(super) source_declaration_projection: Option<PendingSourceDeclarationProjection>,
    pub(super) method_receiver_projection: Option<PendingMethodReceiverProjection>,
    pub(super) contract_clause_projection: Option<PendingFlowContractClauseProjection>,
    pub(super) flow_declaration_projection: Option<PendingFlowDeclarationProjection>,
    pub(super) view_export_projection: Option<PendingViewExportProjection>,
}

impl AttachedNodeRecord {
    pub(super) fn from_parts(parts: AttachedNodeRecordParts) -> Self {
        Self {
            kind: parts.kind,
            tag: parts.tag,
            role: parts.role,
            path: parts.path,
            range: parts.range,
            parent: parts.parent,
            children: parts.children,
            children_by_role: parts.children_by_role,
            expression_projection: parts.expression_projection,
            assertion_projection: parts.assertion_projection,
            keyword_statement_projection: parts.keyword_statement_projection,
            type_projection: parts.type_projection,
            pattern_projection: parts.pattern_projection,
            path_projection: parts.path_projection,
            use_projection: parts.use_projection,
            visibility_projection: parts.visibility_projection,
            attribute_projection: parts.attribute_projection,
            retained_header_projection: parts.retained_header_projection,
            character_projection: parts.character_projection,
            test_kind_projection: parts.test_kind_projection,
            layer_projection: parts.layer_projection,
            entry_projection: parts.entry_projection,
            style_projection: parts.style_projection,
            source_declaration_projection: parts.source_declaration_projection,
            method_receiver_projection: parts.method_receiver_projection,
            contract_clause_projection: parts.contract_clause_projection,
            flow_declaration_projection: parts.flow_declaration_projection,
            view_export_projection: parts.view_export_projection,
        }
    }

    pub(crate) const fn kind(&self) -> SyntaxKind {
        self.kind
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SyntaxSnapshotData {
    snapshot: SyntaxSnapshotId,
    document: Arc<SourceDocument>,
    root: rowan::GreenNode,
    root_id: SyntaxNodeId,
    records: HashMap<SyntaxNodeId, AttachedNodeRecord>,
    by_path: BTreeMap<GrammarEventPath, SyntaxNodeId>,
    type_projections: HashMap<(u64, TypeRefNodePath), SyntaxNodeId>,
    pattern_projections: HashMap<(u64, PatternNodePath), SyntaxNodeId>,
}

impl SyntaxSnapshotData {
    pub(crate) fn new(
        snapshot: SyntaxSnapshotId,
        document: Arc<SourceDocument>,
        root: rowan::GreenNode,
        root_id: SyntaxNodeId,
        records: HashMap<SyntaxNodeId, AttachedNodeRecord>,
        by_path: BTreeMap<GrammarEventPath, SyntaxNodeId>,
    ) -> Self {
        let type_projections = records
            .iter()
            .filter_map(|(id, record)| {
                record
                    .type_projection
                    .as_ref()
                    .map(|projection| ((projection.tree(), projection.path().clone()), *id))
            })
            .collect();
        let pattern_projections = records
            .iter()
            .filter_map(|(id, record)| {
                record
                    .pattern_projection
                    .as_ref()
                    .map(|projection| ((projection.tree(), projection.path().clone()), *id))
            })
            .collect();
        Self {
            snapshot,
            document,
            root,
            root_id,
            records,
            by_path,
            type_projections,
            pattern_projections,
        }
    }

    pub(crate) const fn snapshot_id(&self) -> &SyntaxSnapshotId {
        &self.snapshot
    }

    pub(crate) const fn document(&self) -> &Arc<SourceDocument> {
        &self.document
    }

    pub(crate) fn root_handle(self: &Arc<Self>) -> SyntaxNodeHandle {
        SyntaxNodeHandle::new(Arc::clone(self), self.root_id)
    }

    pub(crate) fn node_for_path(
        self: &Arc<Self>,
        path: &GrammarEventPath,
    ) -> Option<SyntaxNodeHandle> {
        self.by_path
            .get(path)
            .copied()
            .map(|id| SyntaxNodeHandle::new(Arc::clone(self), id))
    }

    pub(crate) fn nodes(self: &Arc<Self>) -> impl Iterator<Item = SyntaxNodeHandle> + '_ {
        self.by_path
            .values()
            .copied()
            .map(|id| SyntaxNodeHandle::new(Arc::clone(self), id))
    }

    pub(crate) fn syntax_node(
        self: &Arc<Self>,
        id: SyntaxNodeId,
    ) -> Result<SyntaxNodeHandle, SyntaxLookupError> {
        self.validate_lineage(id)?;
        if !self.records.contains_key(&id) {
            return Err(SyntaxLookupError::MissingNode { id });
        }
        Ok(SyntaxNodeHandle::new(Arc::clone(self), id))
    }

    pub(crate) fn typed_node<K: AstKind>(
        self: &Arc<Self>,
        id: SyntaxNodeId,
    ) -> Result<AstNode<K>, SyntaxLookupError> {
        AstNode::new(self.syntax_node(id)?)
    }

    pub(crate) fn bind_rowan(
        self: &Arc<Self>,
        node: &SyntaxNode,
    ) -> Result<SyntaxNodeHandle, SyntaxLookupError> {
        // Rowan equality cannot distinguish same-kind zero-width recovery
        // nodes at one offset. Their exact child-index event paths can.
        let mut current = node.clone();
        let mut elements = Vec::new();
        while let Some(parent) = current.parent() {
            elements.push(u32::try_from(current.index()).map_err(|_| {
                SyntaxLookupError::ForeignRowanRoot {
                    expected: self.snapshot.clone(),
                }
            })?);
            current = parent;
        }
        let current_green = current.green();
        let retained_green = std::borrow::Borrow::<rowan::GreenNodeData>::borrow(&self.root);
        if !std::ptr::eq(current_green.as_ref(), retained_green) {
            return Err(SyntaxLookupError::ForeignRowanRoot {
                expected: self.snapshot.clone(),
            });
        }
        elements.reverse();
        let path = GrammarEventPath::from_elements(elements.into_boxed_slice());
        let id = self.by_path.get(&path).copied().ok_or_else(|| {
            SyntaxLookupError::ForeignRowanRoot {
                expected: self.snapshot.clone(),
            }
        })?;
        self.syntax_node(id)
    }

    pub(crate) fn resolve_exact(
        self: &Arc<Self>,
        handle: &SyntaxNodeHandle,
    ) -> Result<SyntaxNodeHandle, SyntaxLookupError> {
        let expected = &self.snapshot;
        let actual = handle.snapshot_id();
        if expected != actual {
            return Err(SyntaxLookupError::WrongSnapshot {
                expected: expected.clone(),
                actual: actual.clone(),
            });
        }
        self.syntax_node(handle.id())
    }

    fn validate_lineage(&self, id: SyntaxNodeId) -> Result<(), SyntaxLookupError> {
        let expected = self.snapshot.lineage();
        let actual = id.lineage();
        if expected.database() != actual.database() {
            return Err(SyntaxLookupError::WrongDatabase {
                expected: expected.database(),
                actual: actual.database(),
            });
        }
        if expected != actual {
            return Err(SyntaxLookupError::WrongLineage { expected, actual });
        }
        Ok(())
    }

    fn record(&self, id: SyntaxNodeId) -> &AttachedNodeRecord {
        &self.records[&id]
    }

    pub(crate) fn type_projection(
        self: &Arc<Self>,
        id: SyntaxNodeId,
    ) -> Option<&PendingTypeProjection> {
        self.record(id).type_projection.as_ref()
    }

    pub(crate) fn expression_projection(
        self: &Arc<Self>,
        id: SyntaxNodeId,
    ) -> Option<&PendingExpressionProjection> {
        self.record(id).expression_projection.as_ref()
    }

    pub(crate) fn assertion_projection(
        self: &Arc<Self>,
        id: SyntaxNodeId,
    ) -> Option<PendingAssertionProjection> {
        self.record(id).assertion_projection
    }

    pub(crate) fn keyword_statement_projection(
        self: &Arc<Self>,
        id: SyntaxNodeId,
    ) -> Option<&PendingKeywordStatementProjection> {
        self.record(id).keyword_statement_projection.as_ref()
    }

    pub(crate) fn pattern_projection(
        self: &Arc<Self>,
        id: SyntaxNodeId,
    ) -> Option<&PendingPatternProjection> {
        self.record(id).pattern_projection.as_ref()
    }

    pub(crate) fn path_projection(
        self: &Arc<Self>,
        id: SyntaxNodeId,
    ) -> Option<&PendingPathProjection> {
        self.record(id).path_projection.as_ref()
    }

    pub(crate) fn use_projection(
        self: &Arc<Self>,
        id: SyntaxNodeId,
    ) -> Option<&PendingUseProjection> {
        self.record(id).use_projection.as_ref()
    }

    pub(crate) fn visibility_projection(
        self: &Arc<Self>,
        id: SyntaxNodeId,
    ) -> Option<PendingVisibilityKind> {
        self.record(id).visibility_projection
    }

    pub(crate) fn character_projection(
        self: &Arc<Self>,
        id: SyntaxNodeId,
    ) -> Option<&PendingCharacterDeclarationProjection> {
        self.record(id).character_projection.as_ref()
    }

    pub(crate) fn test_kind_projection(
        self: &Arc<Self>,
        id: SyntaxNodeId,
    ) -> Option<&PendingTestKindProjection> {
        self.record(id).test_kind_projection.as_ref()
    }

    pub(crate) fn layer_projection(
        self: &Arc<Self>,
        id: SyntaxNodeId,
    ) -> Option<&PendingLayerDeclarationProjection> {
        self.record(id).layer_projection.as_ref()
    }

    pub(crate) fn entry_projection(
        self: &Arc<Self>,
        id: SyntaxNodeId,
    ) -> Option<&PendingEntryDeclarationProjection> {
        self.record(id).entry_projection.as_ref()
    }

    pub(crate) fn style_projection(
        self: &Arc<Self>,
        id: SyntaxNodeId,
    ) -> Option<&PendingStyleDeclarationProjection> {
        self.record(id).style_projection.as_ref()
    }

    pub(crate) fn source_declaration_projection(
        self: &Arc<Self>,
        id: SyntaxNodeId,
    ) -> Option<&PendingSourceDeclarationProjection> {
        self.record(id).source_declaration_projection.as_ref()
    }

    pub(crate) fn view_export_projection(
        self: &Arc<Self>,
        id: SyntaxNodeId,
    ) -> Option<&PendingViewExportProjection> {
        self.record(id).view_export_projection.as_ref()
    }

    pub(crate) fn method_receiver_projection(
        self: &Arc<Self>,
        id: SyntaxNodeId,
    ) -> Option<&PendingMethodReceiverProjection> {
        self.record(id).method_receiver_projection.as_ref()
    }

    pub(crate) fn contract_clause_projection(
        self: &Arc<Self>,
        id: SyntaxNodeId,
    ) -> Option<&PendingFlowContractClauseProjection> {
        self.record(id).contract_clause_projection.as_ref()
    }

    pub(crate) fn flow_declaration_projection(
        self: &Arc<Self>,
        id: SyntaxNodeId,
    ) -> Option<&PendingFlowDeclarationProjection> {
        self.record(id).flow_declaration_projection.as_ref()
    }

    pub(crate) fn retained_header_projection(
        self: &Arc<Self>,
        id: SyntaxNodeId,
    ) -> Option<&PendingRetainedHeaderProjection> {
        self.record(id).retained_header_projection.as_ref()
    }

    pub(crate) fn attribute_projection(
        &self,
        id: SyntaxNodeId,
    ) -> Option<&PendingOuterAttributeProjection> {
        self.record(id).attribute_projection.as_ref()
    }

    pub(crate) fn type_node_for_projection(
        self: &Arc<Self>,
        tree: u64,
        path: &TypeRefNodePath,
    ) -> Option<SyntaxNodeHandle> {
        self.type_projections
            .get(&(tree, path.clone()))
            .copied()
            .map(|id| SyntaxNodeHandle::new(Arc::clone(self), id))
    }

    pub(crate) fn pattern_node_for_projection(
        self: &Arc<Self>,
        tree: u64,
        path: &PatternNodePath,
    ) -> Option<SyntaxNodeHandle> {
        self.pattern_projections
            .get(&(tree, path.clone()))
            .copied()
            .map(|id| SyntaxNodeHandle::new(Arc::clone(self), id))
    }

    fn rowan_node(&self, id: SyntaxNodeId) -> SyntaxNode {
        let root = SyntaxNode::new_root(self.root.clone());
        super::grammar_node_at_path(&root, &self.record(id).path)
            .expect("committed syntax paths resolve in their retained green tree")
    }
}

#[derive(Clone, Debug)]
pub struct SyntaxNodeHandle {
    snapshot: Arc<SyntaxSnapshotData>,
    id: SyntaxNodeId,
    node: SyntaxNode,
}

impl SyntaxNodeHandle {
    fn new(snapshot: Arc<SyntaxSnapshotData>, id: SyntaxNodeId) -> Self {
        let node = snapshot.rowan_node(id);
        Self { snapshot, id, node }
    }

    pub const fn id(&self) -> SyntaxNodeId {
        self.id
    }

    pub fn snapshot_id(&self) -> &SyntaxSnapshotId {
        self.snapshot.snapshot_id()
    }

    pub fn kind(&self) -> SyntaxKind {
        self.snapshot.record(self.id).kind
    }

    pub(crate) fn tag(&self) -> AstTag {
        self.snapshot.record(self.id).tag
    }

    pub(crate) fn role(&self) -> SyntaxRole {
        self.snapshot.record(self.id).role
    }

    pub(crate) fn path(&self) -> &GrammarEventPath {
        &self.snapshot.record(self.id).path
    }

    pub fn rowan(&self) -> &SyntaxNode {
        &self.node
    }

    pub fn range(&self) -> SourceRange {
        self.snapshot.record(self.id).range
    }

    /// Exact revision-bound source span occupied by this node.
    ///
    /// # Panics
    ///
    /// Panics only if crate-internal construction bypasses the committed
    /// snapshot invariant that binds every attached range to its document.
    pub fn source_span(&self) -> SourceSpan {
        self.snapshot
            .document()
            .span(self.range())
            .expect("attached syntax ranges belong to their retained source document")
    }

    pub(crate) fn source_span_for_text_range(&self, range: TextRange) -> SourceSpan {
        self.snapshot
            .document()
            .span(SourceRange::new(range.start(), range.end()))
            .expect("typed type-component ranges belong to their retained source document")
    }

    pub(crate) fn source_span_for_range(&self, range: SourceRange) -> SourceSpan {
        self.snapshot
            .document()
            .span(range)
            .expect("typed Pattern-component ranges belong to their retained source document")
    }

    pub(crate) fn source_text_for_range(&self, range: SourceRange) -> &str {
        self.snapshot
            .document()
            .text()
            .get(range.as_range())
            .expect("typed source-component ranges belong to their retained source document")
    }

    /// Exact UTF-8 source slice retained by this immutable syntax snapshot.
    ///
    /// # Panics
    ///
    /// Panics only if crate-internal construction bypasses the committed
    /// snapshot invariant that validates every attached UTF-8 range.
    pub fn source_text(&self) -> &str {
        self.snapshot
            .document()
            .text()
            .get(self.range().as_range())
            .expect("attached syntax ranges are UTF-8 boundaries in their retained document")
    }

    pub(crate) fn parent(&self) -> Option<Self> {
        self.snapshot
            .record(self.id)
            .parent
            .map(|id| Self::new(Arc::clone(&self.snapshot), id))
    }

    pub(crate) fn children(&self) -> Vec<Self> {
        self.snapshot
            .record(self.id)
            .children
            .iter()
            .copied()
            .map(|id| Self::new(Arc::clone(&self.snapshot), id))
            .collect()
    }

    pub(crate) fn child(&self, role: SyntaxRole) -> Option<Self> {
        let ids = self.snapshot.record(self.id).children_by_role.get(&role)?;
        let [id] = ids.as_ref() else {
            return None;
        };
        Some(Self::new(Arc::clone(&self.snapshot), *id))
    }

    pub(crate) fn children_with_role(&self, role: SyntaxRole) -> Vec<Self> {
        self.snapshot
            .record(self.id)
            .children_by_role
            .get(&role)
            .into_iter()
            .flatten()
            .copied()
            .map(|id| Self::new(Arc::clone(&self.snapshot), id))
            .collect()
    }

    pub(crate) fn type_projection(&self) -> Option<&PendingTypeProjection> {
        self.snapshot.type_projection(self.id)
    }

    pub(crate) fn expression_projection(&self) -> Option<&PendingExpressionProjection> {
        self.snapshot.expression_projection(self.id)
    }

    pub(crate) fn assertion_projection(&self) -> Option<PendingAssertionProjection> {
        self.snapshot.assertion_projection(self.id)
    }

    pub(crate) fn keyword_statement_projection(
        &self,
    ) -> Option<&PendingKeywordStatementProjection> {
        self.snapshot.keyword_statement_projection(self.id)
    }

    pub(crate) fn pattern_projection(&self) -> Option<&PendingPatternProjection> {
        self.snapshot.pattern_projection(self.id)
    }

    pub(crate) fn path_projection(&self) -> Option<&PendingPathProjection> {
        self.snapshot.path_projection(self.id)
    }

    pub(crate) fn use_projection(&self) -> Option<&PendingUseProjection> {
        self.snapshot.use_projection(self.id)
    }

    pub(crate) fn visibility_projection(&self) -> Option<PendingVisibilityKind> {
        self.snapshot.visibility_projection(self.id)
    }

    pub(crate) fn character_projection(&self) -> Option<&PendingCharacterDeclarationProjection> {
        self.snapshot.character_projection(self.id)
    }

    pub(crate) fn test_kind_projection(&self) -> Option<&PendingTestKindProjection> {
        self.snapshot.test_kind_projection(self.id)
    }

    pub(crate) fn layer_projection(&self) -> Option<&PendingLayerDeclarationProjection> {
        self.snapshot.layer_projection(self.id)
    }

    pub(crate) fn entry_projection(&self) -> Option<&PendingEntryDeclarationProjection> {
        self.snapshot.entry_projection(self.id)
    }

    pub(crate) fn style_projection(&self) -> Option<&PendingStyleDeclarationProjection> {
        self.snapshot.style_projection(self.id)
    }

    pub(crate) fn source_declaration_projection(
        &self,
    ) -> Option<&PendingSourceDeclarationProjection> {
        self.snapshot.source_declaration_projection(self.id)
    }

    pub(crate) fn view_export_projection(&self) -> Option<&PendingViewExportProjection> {
        self.snapshot.view_export_projection(self.id)
    }

    pub(crate) fn method_receiver_projection(&self) -> Option<&PendingMethodReceiverProjection> {
        self.snapshot.method_receiver_projection(self.id)
    }

    pub(crate) fn contract_clause_projection(
        &self,
    ) -> Option<&PendingFlowContractClauseProjection> {
        self.snapshot.contract_clause_projection(self.id)
    }

    pub(crate) fn flow_declaration_projection(&self) -> Option<&PendingFlowDeclarationProjection> {
        self.snapshot.flow_declaration_projection(self.id)
    }

    pub(crate) fn retained_header_projection(&self) -> Option<&PendingRetainedHeaderProjection> {
        self.snapshot.retained_header_projection(self.id)
    }

    pub(crate) fn attribute_projection(&self) -> Option<&PendingOuterAttributeProjection> {
        self.snapshot.attribute_projection(self.id)
    }

    pub(crate) fn type_node_for_projection(
        &self,
        tree: u64,
        path: &TypeRefNodePath,
    ) -> Option<Self> {
        self.snapshot.type_node_for_projection(tree, path)
    }

    pub(crate) fn pattern_node_for_projection(
        &self,
        tree: u64,
        path: &PatternNodePath,
    ) -> Option<Self> {
        self.snapshot.pattern_node_for_projection(tree, path)
    }

    pub fn cast<K: ExactAstKind>(&self) -> Result<AstNode<K>, SyntaxLookupError> {
        AstNode::new(self.clone())
    }
}

impl PartialEq for SyntaxNodeHandle {
    fn eq(&self, other: &Self) -> bool {
        self.snapshot.snapshot == other.snapshot.snapshot && self.id == other.id
    }
}

impl Eq for SyntaxNodeHandle {}
