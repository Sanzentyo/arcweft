//! Crate-private typed attachment over the staged grammar tree.
//!
//! This module deliberately exposes no public reader. The existing public CST
//! remains the only source-backed compiler input until the atomic syntax switch.

mod error;
mod node;
mod snapshot;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use arcweft_source::SourceDocument;

pub(crate) use error::{AttachmentFailure, SyntaxLookupError};
pub(crate) use node::{AstKind, AstNode, SourceFileKind};
#[cfg(test)]
pub(crate) use node::{PredicateItemKind, ProofItemKind};
pub(crate) use snapshot::{
    GrammarSyntaxNode, SyntaxDatabaseId, SyntaxLineageId, SyntaxNodeHandle, SyntaxNodeId,
    SyntaxSnapshotData, SyntaxSnapshotId,
};

use crate::grammar::build::{GrammarBuild, GrammarEventPath, UnattachedGrammarEntry};
use crate::grammar::kinds::{AstTag, SyntaxKind, SyntaxRole};

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

/// Builds the immutable node/path/ID attachment for one staged snapshot.
pub(crate) fn attach_typed_tree(
    build: &GrammarBuild,
    identities: &GrammarIdentityMap,
    snapshot: SyntaxSnapshotId,
    document: Arc<SourceDocument>,
) -> Result<Arc<SyntaxSnapshotData>, AttachmentFailure> {
    let root = GrammarSyntaxNode::new_root(build.green().clone());
    let inventory =
        AttachmentInventoryBuilder::new(&root, identities, build.index().entries().len())
            .collect(build.index().entries())?;

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
    if inventory
        .records
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
        snapshot,
        document,
        root,
        root_id,
        inventory.records,
        inventory.by_path,
        inventory.by_node,
    ));
    validate_snapshot(&attached)?;
    Ok(attached)
}

#[derive(Debug)]
struct AttachmentInventory {
    records: HashMap<SyntaxNodeId, snapshot::AttachedNodeRecord>,
    by_path: BTreeMap<GrammarEventPath, SyntaxNodeId>,
    by_node: HashMap<GrammarSyntaxNode, SyntaxNodeId>,
}

#[derive(Debug)]
struct AttachmentInventoryBuilder<'a> {
    root: &'a GrammarSyntaxNode,
    identities: &'a GrammarIdentityMap,
    expected_count: usize,
    by_path: BTreeMap<GrammarEventPath, SyntaxNodeId>,
    by_node: HashMap<GrammarSyntaxNode, SyntaxNodeId>,
    seen_ids: HashSet<SyntaxNodeId>,
    ancestry: Vec<(GrammarEventPath, SyntaxNodeId)>,
    pending: Vec<PendingAttachment>,
    children: HashMap<SyntaxNodeId, Vec<SyntaxNodeId>>,
    children_by_role: HashMap<SyntaxNodeId, BTreeMap<SyntaxRole, Vec<SyntaxNodeId>>>,
}

impl<'a> AttachmentInventoryBuilder<'a> {
    fn new(
        root: &'a GrammarSyntaxNode,
        identities: &'a GrammarIdentityMap,
        expected_count: usize,
    ) -> Self {
        Self {
            root,
            identities,
            expected_count,
            by_path: BTreeMap::new(),
            by_node: HashMap::with_capacity(expected_count),
            seen_ids: HashSet::with_capacity(expected_count),
            ancestry: Vec::new(),
            pending: Vec::with_capacity(expected_count),
            children: HashMap::new(),
            children_by_role: HashMap::new(),
        }
    }

    fn collect(
        mut self,
        entries: &[UnattachedGrammarEntry],
    ) -> Result<AttachmentInventory, AttachmentFailure> {
        for entry in entries {
            self.attach(entry)?;
        }
        self.finish()
    }

    fn attach(&mut self, entry: &UnattachedGrammarEntry) -> Result<(), AttachmentFailure> {
        let id = self.identities.id_for_path(entry.path()).ok_or_else(|| {
            AttachmentFailure::MissingIdentity {
                path: entry.path().clone(),
            }
        })?;
        let node = grammar_node_at_path(self.root, entry.path())
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
        let tag = entry
            .kind()
            .ast_tag()
            .ok_or(AttachmentFailure::MissingAstTag {
                id,
                kind: entry.kind(),
            })?;
        while self.ancestry.last().is_some_and(|(candidate, _)| {
            !strict_path_prefix(candidate.elements(), entry.path().elements())
        }) {
            self.ancestry.pop();
        }
        let parent = self.ancestry.last().map(|(_, id)| *id);
        if let Some(parent) = parent {
            self.children.entry(parent).or_default().push(id);
            self.children_by_role
                .entry(parent)
                .or_default()
                .entry(entry.role())
                .or_default()
                .push(id);
        }
        if !self.seen_ids.insert(id)
            || self.by_path.insert(entry.path().clone(), id).is_some()
            || self.by_node.insert(node.clone(), id).is_some()
        {
            return Err(AttachmentFailure::DuplicateAttachment { id });
        }
        self.pending.push(PendingAttachment {
            id,
            kind: entry.kind(),
            tag,
            role: entry.role(),
            path: entry.path().clone(),
            node,
            parent,
        });
        self.ancestry.push((entry.path().clone(), id));
        Ok(())
    }

    fn finish(mut self) -> Result<AttachmentInventory, AttachmentFailure> {
        let mut records = HashMap::with_capacity(self.expected_count);
        for node in self.pending {
            let record =
                snapshot::AttachedNodeRecord::from_parts(snapshot::AttachedNodeRecordParts {
                    kind: node.kind,
                    tag: node.tag,
                    role: node.role,
                    path: node.path,
                    node: node.node,
                    parent: node.parent,
                    children: self
                        .children
                        .remove(&node.id)
                        .unwrap_or_default()
                        .into_boxed_slice(),
                    children_by_role: self
                        .children_by_role
                        .remove(&node.id)
                        .unwrap_or_default()
                        .into_iter()
                        .map(|(role, children)| (role, children.into_boxed_slice()))
                        .collect(),
                });
            if records.insert(node.id, record).is_some() {
                return Err(AttachmentFailure::DuplicateAttachment { id: node.id });
            }
        }
        if records.len() != self.identities.len() {
            return Err(AttachmentFailure::IdentityMapMismatch {
                expected: self.expected_count,
                actual: self.identities.len(),
            });
        }
        Ok(AttachmentInventory {
            records,
            by_path: self.by_path,
            by_node: self.by_node,
        })
    }
}

#[derive(Debug)]
struct PendingAttachment {
    id: SyntaxNodeId,
    kind: SyntaxKind,
    tag: AstTag,
    role: SyntaxRole,
    path: GrammarEventPath,
    node: GrammarSyntaxNode,
    parent: Option<SyntaxNodeId>,
}

fn strict_path_prefix(parent: &[u32], child: &[u32]) -> bool {
    parent.len() < child.len() && child.starts_with(parent)
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
        || root.parent().is_some()
        || root.tag() != AstTag::SourceFile
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
            || node.kind().ast_tag() != Some(node.tag())
            || node.range().end() > snapshot.document().text().len()
        {
            return Err(AttachmentFailure::SnapshotInvariant);
        }
        if node.id() != root.id() && node.parent().is_none() {
            return Err(AttachmentFailure::SnapshotInvariant);
        }
        for child in node.children() {
            let same_role = node.children_with_role(child.role());
            if child.parent().as_ref() != Some(&node)
                || !same_role.contains(&child)
                || (same_role.len() == 1 && node.child(child.role()).as_ref() != Some(&child))
                || !strict_path_prefix(node.path().elements(), child.path().elements())
            {
                return Err(AttachmentFailure::SnapshotInvariant);
            }
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
    use crate::grammar::kinds::{AstTag, SyntaxKind, SyntaxRole, SyntaxRoleClass};
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

    #[test]
    fn exact_roles_index_nearest_identity_parent_without_structural_wrappers() {
        let snapshot = attach("predicate ready(value: Bool) = check(value)\n");
        let root = snapshot.root_handle();
        let predicate = root
            .child(SyntaxRole::Element(0))
            .expect("item-list wrapper does not become semantic parent");
        assert_eq!(predicate.kind(), SyntaxKind::PredicateItem);
        assert_eq!(predicate.tag(), AstTag::Item);
        assert_eq!(predicate.parent(), Some(root.clone()));
        let element_children = root
            .children()
            .into_iter()
            .filter(|child| child.role().class() == SyntaxRoleClass::Element)
            .collect::<Vec<_>>();
        assert_eq!(
            element_children.as_slice(),
            std::slice::from_ref(&predicate)
        );

        let name = predicate
            .child(SyntaxRole::Name)
            .expect("declaration name is indexed by exact role");
        assert_eq!(name.kind(), SyntaxKind::NameDefinition);
        assert_eq!(name.tag(), AstTag::Name);
        assert_eq!(name.parent(), Some(predicate.clone()));

        let body = predicate
            .child(SyntaxRole::Body)
            .expect("predicate body is indexed by exact role");
        assert_eq!(body.kind(), SyntaxKind::PredicateBody);
        assert_eq!(body.tag(), AstTag::Body);
        let expression_body = body
            .child(SyntaxRole::Body)
            .expect("expression body remains a distinct identity owner");
        let call = expression_body
            .child(SyntaxRole::Body)
            .expect("ordinary expression is attached below its authored body");
        assert_eq!(call.kind(), SyntaxKind::CallExpression);
        assert_eq!(call.tag(), AstTag::Expression);
        assert_eq!(
            call.child(SyntaxRole::Callee)
                .expect("call callee role")
                .kind(),
            SyntaxKind::PathExpression
        );
    }

    #[test]
    fn repeated_exact_roles_remain_ordered_without_claiming_unique_child_access() {
        let snapshot = attach("flow checks {\n    assert.check(true, false)\n}\n");
        let assertion = snapshot
            .nodes()
            .find(|node| node.kind() == SyntaxKind::AssertionStatement)
            .expect("assertion node");
        let conditions = assertion.children_with_role(SyntaxRole::Condition);
        assert_eq!(conditions.len(), 2);
        assert!(
            conditions
                .iter()
                .all(|condition| condition.tag() == AstTag::Expression)
        );
        assert_eq!(
            conditions
                .iter()
                .map(|condition| condition.rowan().text().to_string())
                .collect::<Vec<_>>(),
            ["true", "false"]
        );
        assert_eq!(assertion.child(SyntaxRole::Condition), None);
    }
}
