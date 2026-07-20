//! Crate-private typed attachment over the staged grammar tree.
//!
//! This module deliberately exposes no public reader. The existing public CST
//! remains the only source-backed compiler input until the atomic syntax switch.

mod error;
mod snapshot;

use core::marker::PhantomData;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use arcweft_source::{SourceDocument, SourceRange};

pub(crate) use error::{AttachmentFailure, SyntaxLookupError};
pub(crate) use snapshot::{
    GrammarSyntaxNode, SyntaxDatabaseId, SyntaxLineageId, SyntaxNodeHandle, SyntaxNodeId,
    SyntaxSnapshotData, SyntaxSnapshotId,
};

use crate::grammar::build::{GrammarBuild, GrammarEventPath};
use crate::grammar::kinds::SyntaxKind;

/// Stable grammar identities indexed by exact event path.
#[derive(Clone, Debug, Default)]
pub(crate) struct GrammarIdentityMap {
    by_path: HashMap<GrammarEventPath, SyntaxNodeId>,
}

impl GrammarIdentityMap {
    pub(crate) fn new(by_path: HashMap<GrammarEventPath, SyntaxNodeId>) -> Self {
        Self { by_path }
    }

    pub(crate) fn id_for_path(&self, path: &GrammarEventPath) -> Option<SyntaxNodeId> {
        self.by_path.get(path).copied()
    }

    pub(crate) fn len(&self) -> usize {
        self.by_path.len()
    }

    #[cfg(test)]
    pub(crate) fn remove_path(&mut self, path: &GrammarEventPath) {
        self.by_path.remove(path);
    }
}

/// Syntax-owned exact-kind marker for an attached grammar node.
pub(crate) trait AstKind: Copy + 'static {
    const KIND: SyntaxKind;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SourceFileKind;

impl AstKind for SourceFileKind {
    const KIND: SyntaxKind = SyntaxKind::SourceFile;
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PredicateItemKind;

#[cfg(test)]
impl AstKind for PredicateItemKind {
    const KIND: SyntaxKind = SyntaxKind::PredicateItem;
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProofItemKind;

#[cfg(test)]
impl AstKind for ProofItemKind {
    const KIND: SyntaxKind = SyntaxKind::ProofItem;
}

/// Typed handle that cannot detach from its immutable grammar snapshot.
pub(crate) struct AstNode<K: AstKind> {
    syntax: SyntaxNodeHandle,
    marker: PhantomData<fn() -> K>,
}

impl<K: AstKind> Clone for AstNode<K> {
    fn clone(&self) -> Self {
        Self {
            syntax: self.syntax.clone(),
            marker: PhantomData,
        }
    }
}

impl<K: AstKind> core::fmt::Debug for AstNode<K> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("AstNode")
            .field("kind", &K::KIND)
            .field("id", &self.id())
            .field("snapshot", self.snapshot_id())
            .finish()
    }
}

impl<K: AstKind> PartialEq for AstNode<K> {
    fn eq(&self, other: &Self) -> bool {
        self.syntax == other.syntax
    }
}

impl<K: AstKind> Eq for AstNode<K> {}

impl<K: AstKind> AstNode<K> {
    fn new(syntax: SyntaxNodeHandle) -> Result<Self, SyntaxLookupError> {
        if syntax.kind() != K::KIND {
            return Err(SyntaxLookupError::KindMismatch {
                id: syntax.id(),
                expected: K::KIND,
                actual: syntax.kind(),
            });
        }
        Ok(Self {
            syntax,
            marker: PhantomData,
        })
    }

    pub(crate) fn id(&self) -> SyntaxNodeId {
        self.syntax.id()
    }

    pub(crate) fn snapshot_id(&self) -> &SyntaxSnapshotId {
        self.syntax.snapshot_id()
    }

    pub(crate) fn syntax(&self) -> SyntaxNodeHandle {
        self.syntax.clone()
    }

    pub(crate) fn range(&self) -> SourceRange {
        self.syntax.range()
    }

    pub(crate) fn is_same_reconciled_node(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

/// Builds the immutable node/path/ID attachment for one staged snapshot.
pub(crate) fn attach_typed_tree(
    build: &GrammarBuild,
    identities: &GrammarIdentityMap,
    snapshot: SyntaxSnapshotId,
    document: Arc<SourceDocument>,
) -> Result<Arc<SyntaxSnapshotData>, AttachmentFailure> {
    let root = GrammarSyntaxNode::new_root(build.green().clone());
    let mut records = HashMap::with_capacity(build.index().entries().len());
    let mut by_path = BTreeMap::new();
    let mut by_node = HashMap::with_capacity(build.index().entries().len());

    for entry in build.index().entries() {
        let id = identities.id_for_path(entry.path()).ok_or_else(|| {
            AttachmentFailure::MissingIdentity {
                path: entry.path().clone(),
            }
        })?;
        let node = grammar_node_at_path(&root, entry.path())
            .ok_or(AttachmentFailure::MissingAttachment { id })?;
        let actual = node.kind();
        let expected = rowan::SyntaxKind(entry.kind() as u16);
        if actual != expected {
            return Err(AttachmentFailure::GrammarKindMismatch {
                id,
                expected: entry.kind(),
                actual,
            });
        }
        let record = snapshot::AttachedNodeRecord::new(
            id,
            entry.kind(),
            entry.role(),
            entry.path().clone(),
            node.clone(),
        );
        if records.insert(id, record).is_some()
            || by_path.insert(entry.path().clone(), id).is_some()
            || by_node.insert(node, id).is_some()
        {
            return Err(AttachmentFailure::DuplicateAttachment { id });
        }
    }

    if records.len() != identities.len() {
        return Err(AttachmentFailure::IdentityMapMismatch {
            expected: build.index().entries().len(),
            actual: identities.len(),
        });
    }

    let root_id = identities
        .id_for_path(
            build
                .index()
                .entries()
                .first()
                .ok_or(AttachmentFailure::MissingRoot)?
                .path(),
        )
        .ok_or(AttachmentFailure::MissingRoot)?;
    if records
        .get(&root_id)
        .is_none_or(|record| record.kind() != SyntaxKind::SourceFile)
    {
        return Err(AttachmentFailure::MissingRoot);
    }

    #[expect(
        clippy::arc_with_non_send_sync,
        reason = "immutable snapshot ownership is shared while Rowan red nodes remain session-thread-affine"
    )]
    let attached = Arc::new(SyntaxSnapshotData::new(
        snapshot, document, root, root_id, records, by_path, by_node,
    ));
    validate_snapshot(&attached)?;
    Ok(attached)
}

pub(crate) fn grammar_node_at_path(
    root: &GrammarSyntaxNode,
    path: &GrammarEventPath,
) -> Option<GrammarSyntaxNode> {
    let mut current = root.clone();
    for &element in path.elements() {
        let index = usize::try_from(element).ok()?;
        current = current.children_with_tokens().nth(index)?.into_node()?;
    }
    Some(current)
}

fn validate_snapshot(snapshot: &Arc<SyntaxSnapshotData>) -> Result<(), AttachmentFailure> {
    let root = snapshot.root_handle();
    let typed_root = snapshot
        .typed_node::<SourceFileKind>(root.id())
        .map_err(|_| AttachmentFailure::SnapshotInvariant)?;
    if typed_root.id() != root.id()
        || typed_root.snapshot_id() != snapshot.snapshot_id()
        || typed_root.syntax() != root
        || typed_root.range() != root.range()
        || !typed_root.is_same_reconciled_node(&typed_root.clone())
        || root
            .cast::<SourceFileKind>()
            .map_err(|_| AttachmentFailure::SnapshotInvariant)?
            != typed_root
        || root.rowan().text() != snapshot.document().text()
    {
        return Err(AttachmentFailure::SnapshotInvariant);
    }

    for node in snapshot.nodes() {
        if snapshot
            .syntax_node(node.id())
            .map_err(|_| AttachmentFailure::SnapshotInvariant)?
            != node
            || snapshot
                .node_for_path(node.path())
                .is_none_or(|by_path| by_path != node)
            || snapshot
                .bind_rowan(node.rowan())
                .map_err(|_| AttachmentFailure::SnapshotInvariant)?
                != node
            || snapshot
                .resolve_exact(&node)
                .map_err(|_| AttachmentFailure::SnapshotInvariant)?
                != node
            || node.range().end() > snapshot.document().text().len()
        {
            return Err(AttachmentFailure::SnapshotInvariant);
        }
        let _ = node.role().class();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use arcweft_source::identity::SourceSnapshotId;
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
    use core::num::NonZeroU64;

    use super::{
        AstNode, GrammarIdentityMap, PredicateItemKind, ProofItemKind, SyntaxDatabaseId,
        SyntaxLineageId, SyntaxLookupError, SyntaxNodeId, SyntaxSnapshotId, attach_typed_tree,
    };
    use crate::parser::parse_shadow_document;

    fn document(text: &str) -> Arc<SourceDocument> {
        Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcw:/attachment-test").unwrap(),
                SourceName::path("attachment-test.arcw"),
                text,
            )
            .unwrap(),
        )
    }

    fn attach(text: &str) -> Arc<super::SyntaxSnapshotData> {
        let document = document(text);
        let build = parse_shadow_document(&document).unwrap();
        let database = SyntaxDatabaseId::from_raw_for_test(NonZeroU64::new(1).unwrap());
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

    #[test]
    fn typed_and_rowan_handles_round_trip_without_range_search() {
        let snapshot = attach("predicate ready() = true\nproof valid() = ()\n");
        let predicate = snapshot
            .nodes()
            .find(|node| node.kind() == crate::grammar::kinds::SyntaxKind::PredicateItem)
            .unwrap();
        let typed = AstNode::<PredicateItemKind>::new(predicate.clone()).unwrap();
        let rebound = snapshot.bind_rowan(typed.syntax().rowan()).unwrap();
        assert_eq!(rebound, predicate);
        assert_eq!(
            snapshot
                .typed_node::<PredicateItemKind>(typed.id())
                .unwrap(),
            typed
        );

        assert!(matches!(
            snapshot.typed_node::<ProofItemKind>(typed.id()),
            Err(SyntaxLookupError::KindMismatch { .. })
        ));
    }

    #[test]
    fn structurally_equal_foreign_rowan_root_is_rejected() {
        let first = attach("proof valid() = ()\n");
        let second = attach("proof valid() = ()\n");
        let foreign = second.root_handle();
        assert!(matches!(
            first.bind_rowan(foreign.rowan()),
            Err(SyntaxLookupError::ForeignRowanRoot { .. })
        ));
    }
}
