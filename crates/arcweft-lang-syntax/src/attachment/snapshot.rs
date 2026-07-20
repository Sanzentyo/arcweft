//! Immutable grammar snapshot data and qualified session identities.

use core::num::NonZeroU64;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceRange};

use super::error::SyntaxLookupError;
use super::{AstKind, AstNode};
use crate::grammar::build::GrammarEventPath;
use crate::grammar::kinds::{AstTag, SyntaxKind, SyntaxRole};

static NEXT_DATABASE_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct SyntaxDatabaseId(NonZeroU64);

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
pub(crate) struct SyntaxLineageId {
    database: SyntaxDatabaseId,
    ordinal: NonZeroU64,
}

impl SyntaxLineageId {
    pub(crate) const fn new(database: SyntaxDatabaseId, ordinal: NonZeroU64) -> Self {
        Self { database, ordinal }
    }

    pub(crate) const fn database(self) -> SyntaxDatabaseId {
        self.database
    }

    #[cfg(test)]
    pub(crate) const fn from_raw_for_test(database: SyntaxDatabaseId, ordinal: NonZeroU64) -> Self {
        Self::new(database, ordinal)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct SyntaxSnapshotId {
    lineage: SyntaxLineageId,
    source: SourceSnapshotId,
}

impl SyntaxSnapshotId {
    pub(crate) const fn new(lineage: SyntaxLineageId, source: SourceSnapshotId) -> Self {
        Self { lineage, source }
    }

    pub(crate) const fn lineage(&self) -> SyntaxLineageId {
        self.lineage
    }

    pub(crate) const fn source(&self) -> &SourceSnapshotId {
        &self.source
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct SyntaxNodeId {
    lineage: SyntaxLineageId,
    slot: NonZeroU64,
}

impl SyntaxNodeId {
    pub(crate) const fn new(lineage: SyntaxLineageId, slot: NonZeroU64) -> Self {
        Self { lineage, slot }
    }

    pub(crate) const fn lineage(self) -> SyntaxLineageId {
        self.lineage
    }

    pub(crate) const fn slot(self) -> NonZeroU64 {
        self.slot
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum GrammarLanguage {}

impl rowan::Language for GrammarLanguage {
    type Kind = rowan::SyntaxKind;

    fn kind_from_raw(raw: rowan::SyntaxKind) -> Self::Kind {
        raw
    }

    fn kind_to_raw(kind: Self::Kind) -> rowan::SyntaxKind {
        kind
    }
}

pub(crate) type GrammarSyntaxNode = rowan::SyntaxNode<GrammarLanguage>;

#[derive(Clone, Debug)]
pub(crate) struct AttachedNodeRecord {
    kind: SyntaxKind,
    tag: AstTag,
    role: SyntaxRole,
    path: GrammarEventPath,
    node: GrammarSyntaxNode,
    parent: Option<SyntaxNodeId>,
    children: Box<[SyntaxNodeId]>,
    children_by_role: BTreeMap<SyntaxRole, Box<[SyntaxNodeId]>>,
}

#[derive(Clone, Debug)]
pub(super) struct AttachedNodeRecordParts {
    pub(super) kind: SyntaxKind,
    pub(super) tag: AstTag,
    pub(super) role: SyntaxRole,
    pub(super) path: GrammarEventPath,
    pub(super) node: GrammarSyntaxNode,
    pub(super) parent: Option<SyntaxNodeId>,
    pub(super) children: Box<[SyntaxNodeId]>,
    pub(super) children_by_role: BTreeMap<SyntaxRole, Box<[SyntaxNodeId]>>,
}

impl AttachedNodeRecord {
    pub(super) fn from_parts(parts: AttachedNodeRecordParts) -> Self {
        Self {
            kind: parts.kind,
            tag: parts.tag,
            role: parts.role,
            path: parts.path,
            node: parts.node,
            parent: parts.parent,
            children: parts.children,
            children_by_role: parts.children_by_role,
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
    root: GrammarSyntaxNode,
    root_id: SyntaxNodeId,
    records: HashMap<SyntaxNodeId, AttachedNodeRecord>,
    by_path: BTreeMap<GrammarEventPath, SyntaxNodeId>,
    by_node: HashMap<GrammarSyntaxNode, SyntaxNodeId>,
}

impl SyntaxSnapshotData {
    pub(crate) const fn new(
        snapshot: SyntaxSnapshotId,
        document: Arc<SourceDocument>,
        root: GrammarSyntaxNode,
        root_id: SyntaxNodeId,
        records: HashMap<SyntaxNodeId, AttachedNodeRecord>,
        by_path: BTreeMap<GrammarEventPath, SyntaxNodeId>,
        by_node: HashMap<GrammarSyntaxNode, SyntaxNodeId>,
    ) -> Self {
        Self {
            snapshot,
            document,
            root,
            root_id,
            records,
            by_path,
            by_node,
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
        node: &GrammarSyntaxNode,
    ) -> Result<SyntaxNodeHandle, SyntaxLookupError> {
        if node.ancestors().last().as_ref() != Some(&self.root) {
            return Err(SyntaxLookupError::ForeignRowanRoot {
                expected: self.snapshot.clone(),
            });
        }
        let id =
            self.by_node
                .get(node)
                .copied()
                .ok_or_else(|| SyntaxLookupError::ForeignRowanRoot {
                    expected: self.snapshot.clone(),
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
}

#[derive(Clone, Debug)]
pub(crate) struct SyntaxNodeHandle {
    snapshot: Arc<SyntaxSnapshotData>,
    id: SyntaxNodeId,
}

impl SyntaxNodeHandle {
    const fn new(snapshot: Arc<SyntaxSnapshotData>, id: SyntaxNodeId) -> Self {
        Self { snapshot, id }
    }

    pub(crate) const fn id(&self) -> SyntaxNodeId {
        self.id
    }

    pub(crate) fn snapshot_id(&self) -> &SyntaxSnapshotId {
        self.snapshot.snapshot_id()
    }

    pub(crate) fn kind(&self) -> SyntaxKind {
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

    pub(crate) fn rowan(&self) -> &GrammarSyntaxNode {
        &self.snapshot.record(self.id).node
    }

    pub(crate) fn range(&self) -> SourceRange {
        let range = self.rowan().text_range();
        SourceRange::new(usize::from(range.start()), usize::from(range.end()))
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

    pub(crate) fn cast<K: AstKind>(&self) -> Result<AstNode<K>, SyntaxLookupError> {
        AstNode::new(self.clone())
    }
}

impl PartialEq for SyntaxNodeHandle {
    fn eq(&self, other: &Self) -> bool {
        self.snapshot.snapshot == other.snapshot.snapshot && self.id == other.id
    }
}

impl Eq for SyntaxNodeHandle {}
