//! Crate-private typed attachment over the staged grammar tree.
//!
//! This module deliberately exposes no public reader. The existing public CST
//! remains the only source-backed compiler input until the atomic syntax switch.

mod access;
mod error;
mod family;
mod node;
mod snapshot;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use arcweft_source::SourceDocument;

pub(crate) use error::{AttachmentFailure, SyntaxAccessError, SyntaxLookupError};
pub(crate) use node::{
    AstKind, AstNode, ExactAstKind, ExpressionFragmentRootKind, PatternFragmentRootKind,
    SourceFileKind, StatementFragmentRootKind, TypeFragmentRootKind,
};
#[cfg(test)]
pub(crate) use node::{
    BlockKind, ExpressionStatementKind, PredicateItemKind, ProofItemKind, SourceItemKind,
};
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
    ));
    validate_snapshot(&attached)?;
    Ok(attached)
}

#[derive(Debug)]
struct AttachmentInventory {
    records: HashMap<SyntaxNodeId, snapshot::AttachedNodeRecord>,
    by_path: BTreeMap<GrammarEventPath, SyntaxNodeId>,
}

#[derive(Debug)]
struct AttachmentInventoryBuilder<'a> {
    root: &'a GrammarSyntaxNode,
    identities: &'a GrammarIdentityMap,
    expected_count: usize,
    by_path: BTreeMap<GrammarEventPath, SyntaxNodeId>,
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
        if !self.seen_ids.insert(id) || self.by_path.insert(entry.path().clone(), id).is_some() {
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
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange};
    use core::num::NonZeroU64;

    use super::access::{
        BlockTailNode, DeclarationBodyNode, IfStatementElseNode, IfStatementHeadNode,
    };
    use super::family::{DelimiterFamily, ExpressionFamily, FamilyNode, RichTextNode, TypeFamily};
    use super::node::{
        AssertionStatementKind, BinaryExpressionKind, CallExpressionKind, CharacterBodyKind,
        CharacterDeclarationItemKind, DialogueCallExpressionKind, ExpressionBodyKind,
        FixedParameterGroupKind, FunctionTypeKind, GenericApplicationTypeKind, IfStatementKind,
        LetStatementKind, PredicateBodyKind, ProofBlockKind, ProofBodyKind, ProofCallStatementKind,
        RecordPatternKind, RichTextArgumentPayloadKind, RichTextArgumentValueKind,
        RichTextConditionPayloadKind, RichTextDialogueCallPayloadKind, RichTextEndTagKind,
        RichTextFxCallPayloadKind, RichTextInvalidArgumentKind, RichTextNamedArgumentKind,
        RichTextTagKind, UnsafeLifetimeStatementKind, WholeBindingPatternKind,
    };
    use super::{
        AstNode, GrammarIdentityMap, PredicateItemKind, ProofItemKind, SyntaxDatabaseId,
        SyntaxLineageId, SyntaxLookupError, SyntaxNodeId, SyntaxSnapshotId, attach_typed_tree,
    };
    use crate::ast::dialogue::DialogueToken;
    use crate::grammar::kinds::{AstTag, SyntaxKind, SyntaxRole, SyntaxRoleClass};
    use crate::parser::parse_shadow_document;
    use crate::text::parse_dialogue_text;

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
        attach_at(text, 1, 1)
    }

    fn attach_at(
        text: &str,
        database_ordinal: u64,
        lineage_ordinal: u64,
    ) -> Arc<super::SyntaxSnapshotData> {
        let document = document(text);
        let build = parse_shadow_document(&document).unwrap();
        let database =
            SyntaxDatabaseId::from_raw_for_test(NonZeroU64::new(database_ordinal).unwrap());
        let lineage =
            SyntaxLineageId::from_raw_for_test(database, NonZeroU64::new(lineage_ordinal).unwrap());
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
    fn typed_handle_cannot_cross_an_immutable_snapshot_lineage() {
        let first = attach_at("proof valid() = ()\n", 1, 1);
        let second = attach_at("proof valid() = ()\n", 1, 2);
        let first_item = first.typed_tree().unwrap().items().unwrap().remove(0);
        assert!(matches!(
            second.resolve_exact(&first_item.syntax()),
            Err(SyntaxLookupError::WrongSnapshot { .. })
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

    #[test]
    fn statement_if_let_and_unsafe_audit_anchor_remain_typed_and_snapshot_bound() {
        let snapshot = attach(concat!(
            "fn choose(input: Option<Int>, ready: Bool) {\n",
            "    if let .Some(value) = input when ready { value; } else if ready { 1; } else { 0; };\n",
            "    unsafe lifetime @unsafe.audit { value; };\n",
            "}\n",
        ));

        let conditional = snapshot
            .nodes()
            .find(|node| node.kind() == SyntaxKind::IfStatement)
            .expect("if statement")
            .cast::<IfStatementKind>()
            .unwrap();
        let IfStatementHeadNode::Let {
            pattern,
            scrutinee,
            guard,
        } = conditional.head().unwrap()
        else {
            panic!("statement-form if let must keep its pattern head");
        };
        assert_eq!(pattern.syntax().rowan().text().to_string(), ".Some(value)");
        assert_eq!(scrutinee.syntax().rowan().text().to_string(), "input");
        assert_eq!(guard.unwrap().syntax().rowan().text().to_string(), "ready");
        assert_eq!(
            conditional
                .then_branch()
                .unwrap()
                .statements()
                .unwrap()
                .len(),
            1
        );
        let Some(IfStatementElseNode::If(nested)) = conditional.else_branch().unwrap() else {
            panic!("else if must keep its nested statement identity");
        };
        assert!(matches!(
            nested.head().unwrap(),
            IfStatementHeadNode::Condition(condition)
                if condition.syntax().rowan().text() == "ready"
        ));
        assert!(matches!(
            nested.else_branch().unwrap(),
            Some(IfStatementElseNode::Block(_))
        ));

        let audit = snapshot
            .nodes()
            .find(|node| node.kind() == SyntaxKind::UnsafeLifetimeStatement)
            .expect("unsafe lifetime statement")
            .cast::<UnsafeLifetimeStatementKind>()
            .unwrap();
        let anchor = audit.audit_insertion_anchor().unwrap();
        assert_eq!(anchor.syntax().rowan().text().to_string(), "{");
        assert_eq!(anchor.snapshot_id(), snapshot.snapshot_id());
        assert_eq!(
            anchor.syntax().parent(),
            Some(audit.body().unwrap().syntax().clone())
        );
        assert_eq!(
            anchor.id(),
            audit.body().unwrap().open_delimiter().unwrap().id()
        );
        assert_eq!(
            audit
                .body()
                .unwrap()
                .close_delimiter()
                .unwrap()
                .syntax()
                .rowan()
                .text()
                .to_string(),
            "}"
        );
    }

    #[test]
    fn missing_if_let_equals_retains_a_typed_missing_scrutinee() {
        let snapshot = attach(concat!(
            "fn choose(input: Option<Int>) {\n",
            "    if let .Some(value) input { value; };\n",
            "}\n",
        ));
        let conditional = snapshot
            .nodes()
            .find(|node| node.kind() == SyntaxKind::IfStatement)
            .expect("recovered if statement")
            .cast::<IfStatementKind>()
            .unwrap();
        let IfStatementHeadNode::Let { scrutinee, .. } = conditional.head().unwrap() else {
            panic!("recovered if let must keep its pattern head");
        };
        assert_eq!(scrutinee.kind(), SyntaxKind::MissingExpression);
        assert!(scrutinee.range().is_empty());
        assert_eq!(scrutinee.snapshot_id(), snapshot.snapshot_id());
    }

    #[test]
    fn unsafe_audit_body_recovery_never_fabricates_an_authored_anchor() {
        let missing_body = attach(concat!(
            "fn audit() {\n",
            "    unsafe lifetime @unsafe.audit value;\n",
            "}\n",
        ));
        let audit = missing_body
            .nodes()
            .find(|node| node.kind() == SyntaxKind::UnsafeLifetimeStatement)
            .expect("unsafe lifetime statement")
            .cast::<UnsafeLifetimeStatementKind>()
            .unwrap();
        assert!(audit.body().is_err());
        assert!(audit.audit_insertion_anchor().is_err());

        let unclosed_body = attach(concat!(
            "fn audit() {\n",
            "    unsafe lifetime @unsafe.audit { value;\n",
        ));
        let audit = unclosed_body
            .nodes()
            .find(|node| node.kind() == SyntaxKind::UnsafeLifetimeStatement)
            .expect("recovered unsafe lifetime statement")
            .cast::<UnsafeLifetimeStatementKind>()
            .unwrap();
        let body = audit.body().unwrap();
        assert_eq!(
            audit
                .audit_insertion_anchor()
                .unwrap()
                .syntax()
                .rowan()
                .text()
                .to_string(),
            "{"
        );
        assert!(body.close_delimiter().unwrap().range().is_empty());
    }

    #[test]
    fn typed_tree_navigates_declaration_prefixes_parameters_and_expression_body() {
        let source = concat!(
            "/// externally reviewed\n",
            "#[verify.trusted(reason = \"reviewed\")]\n",
            "pub proof ordered<'a, T>((left, right): (T, T), cmp: Comparator<T>) ",
            "-> Bool where T: Ord requires cmp.ready() ensures result = left == right\n",
        );
        let snapshot = attach(source);
        let tree = snapshot.typed_tree().unwrap();
        assert_eq!(tree.root().range(), SourceRange::new(0, source.len()));
        let items = tree.items().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].role(), SyntaxRole::Element(0));

        let proof = items[0].cast::<ProofItemKind>().unwrap();
        assert_eq!(
            items[0]
                .documentation()
                .unwrap()
                .unwrap()
                .syntax()
                .rowan()
                .text()
                .to_string(),
            "/// externally reviewed\n"
        );
        assert_eq!(items[0].attributes().unwrap().len(), 1);
        assert_eq!(
            items[0]
                .visibility()
                .unwrap()
                .unwrap()
                .syntax()
                .rowan()
                .text()
                .to_string(),
            "pub"
        );
        assert_eq!(
            items[0]
                .name()
                .unwrap()
                .unwrap()
                .syntax()
                .rowan()
                .text()
                .to_string(),
            "ordered"
        );
        assert!(items[0].declaration_header().unwrap().is_none());

        let parameters = proof
            .required_exact_child::<FixedParameterGroupKind>(SyntaxRole::ParameterGroup)
            .unwrap()
            .parameters()
            .unwrap();
        assert_eq!(parameters.len(), 2);
        assert_eq!(
            parameters[0].pattern().unwrap().kind(),
            SyntaxKind::TuplePattern
        );
        assert_eq!(parameters[0].ty().unwrap().kind(), SyntaxKind::TupleType);
        assert_eq!(
            parameters[1].ty().unwrap().kind(),
            SyntaxKind::GenericApplicationType
        );

        let DeclarationBodyNode::Body(proof_body) = items[0].body().unwrap().unwrap() else {
            panic!("proof has an authored body");
        };
        let proof_body = proof_body.cast::<ProofBodyKind>().unwrap();
        let DeclarationBodyNode::Body(expression_body) = proof_body.content().unwrap() else {
            panic!("proof has an expression body");
        };
        let expression_body = expression_body.cast::<ExpressionBodyKind>().unwrap();
        let expression = expression_body.expression().unwrap();
        let binary = expression.cast::<BinaryExpressionKind>().unwrap();
        assert_eq!(
            binary.left().unwrap().syntax().rowan().text().to_string(),
            "left"
        );
        assert_eq!(
            binary.right().unwrap().syntax().rowan().text().to_string(),
            "right"
        );
    }

    #[test]
    fn ordinary_call_accessors_keep_named_and_positional_argument_order() {
        let snapshot = attach("predicate next(value: Int) = outer(named = inner(value), value)\n");
        let item = snapshot.typed_tree().unwrap().items().unwrap().remove(0);
        let DeclarationBodyNode::Body(predicate_body) = item.body().unwrap().unwrap() else {
            panic!("predicate has an authored body");
        };
        let predicate_body = predicate_body.cast::<PredicateBodyKind>().unwrap();
        let DeclarationBodyNode::Body(expression_body) = predicate_body.content().unwrap() else {
            panic!("predicate has an expression body");
        };
        let call = expression_body
            .cast::<ExpressionBodyKind>()
            .unwrap()
            .expression()
            .unwrap()
            .cast::<CallExpressionKind>()
            .unwrap();
        assert_eq!(
            call.callee().unwrap().syntax().rowan().text().to_string(),
            "outer"
        );
        let arguments = call.arguments().unwrap();
        assert_eq!(arguments.len(), 2);
        assert_eq!(arguments[0].role(), SyntaxRole::Argument(0));
        assert_eq!(arguments[1].role(), SyntaxRole::Argument(1));
        assert_eq!(
            arguments[0]
                .name()
                .unwrap()
                .unwrap()
                .syntax()
                .rowan()
                .text()
                .to_string(),
            "named"
        );
        assert_eq!(
            arguments[0]
                .operand()
                .unwrap()
                .syntax()
                .rowan()
                .text()
                .to_string(),
            "inner(value)"
        );
        assert_eq!(
            arguments[1]
                .operand()
                .unwrap()
                .syntax()
                .rowan()
                .text()
                .to_string(),
            "value"
        );
        assert!(arguments[0].range().end() <= arguments[1].range().start());
    }

    #[test]
    fn retained_declaration_header_owns_prefixes_name_and_body() {
        let source = concat!(
            "/// authored character\n",
            "#[authoring]\n",
            "pub character @character.alice Alice as alice {\n",
            "    display_name = \"Alice\"\n",
            "}\n",
        );
        let snapshot = attach(source);
        let item = snapshot.typed_tree().unwrap().items().unwrap().remove(0);
        let character = item.cast::<CharacterDeclarationItemKind>().unwrap();
        let header = item
            .declaration_header()
            .unwrap()
            .expect("retained declaration owns an exact header");
        assert_eq!(
            header
                .documentation()
                .unwrap()
                .unwrap()
                .syntax()
                .rowan()
                .text()
                .to_string(),
            "/// authored character\n"
        );
        assert_eq!(header.attributes().unwrap().len(), 1);
        assert_eq!(
            header
                .name()
                .unwrap()
                .unwrap()
                .syntax()
                .rowan()
                .text()
                .to_string(),
            "Alice"
        );
        assert!(header.visibility().unwrap().is_some());
        assert_eq!(
            character
                .required_exact_child::<CharacterBodyKind>(SyntaxRole::Body)
                .unwrap()
                .kind(),
            SyntaxKind::CharacterBody
        );
    }

    #[test]
    fn proof_block_accessors_preserve_statement_pattern_type_and_tail_identity() {
        let source = concat!(
            "proof verify(value: Result<Int>) {\n",
            "    let current: Int = unwrap(value);\n",
            "    assert.check(current > 0, is_valid(current));\n",
            "    verify_nested(current);\n",
            "}\n",
        );
        let snapshot = attach(source);
        let proof = snapshot
            .typed_tree()
            .unwrap()
            .items()
            .unwrap()
            .remove(0)
            .cast::<ProofItemKind>()
            .unwrap();
        let proof_body = proof
            .required_exact_child::<ProofBodyKind>(SyntaxRole::Body)
            .unwrap();
        let DeclarationBodyNode::Body(block) = proof_body.content().unwrap() else {
            panic!("proof has a block body");
        };
        let block = block.cast::<ProofBlockKind>().unwrap();
        assert_eq!(
            block.open_delimiter().unwrap().kind(),
            SyntaxKind::OpenBraceNode
        );
        assert_eq!(
            block.close_delimiter().unwrap().kind(),
            SyntaxKind::CloseBraceNode
        );

        let statements = block.statements().unwrap();
        assert_eq!(statements.len(), 3);
        assert_eq!(statements[0].role(), SyntaxRole::Statement(0));
        assert_eq!(statements[1].role(), SyntaxRole::Statement(1));
        assert_eq!(statements[2].role(), SyntaxRole::Statement(2));

        let binding = statements[0].cast::<LetStatementKind>().unwrap();
        assert_eq!(
            binding.pattern().unwrap().kind(),
            SyntaxKind::BindingPattern
        );
        assert_eq!(
            binding.annotation().unwrap().unwrap().kind(),
            SyntaxKind::PrimitiveType
        );
        assert_eq!(
            binding.initializer().unwrap().unwrap().kind(),
            SyntaxKind::CallExpression
        );

        let assertion = statements[1].cast::<AssertionStatementKind>().unwrap();
        let conditions = assertion.conditions().unwrap();
        assert_eq!(conditions.len(), 2);
        assert_eq!(
            conditions
                .iter()
                .map(|condition| condition.syntax().rowan().text().to_string())
                .collect::<Vec<_>>(),
            ["current > 0", "is_valid(current)"]
        );
        assert!(matches!(
            assertion.required_family_child::<ExpressionFamily>(SyntaxRole::Condition),
            Err(super::SyntaxAccessError::AmbiguousChild { count: 2, .. })
        ));

        let proof_call = statements[2].cast::<ProofCallStatementKind>().unwrap();
        assert_eq!(
            proof_call
                .callee()
                .unwrap()
                .syntax()
                .rowan()
                .text()
                .to_string(),
            "verify_nested(current)"
        );
        let BlockTailNode::Omitted(tail) = block.tail().unwrap() else {
            panic!("semicolon-terminated block has an omitted tail");
        };
        assert_eq!(tail.range().start(), tail.range().end());
    }

    #[test]
    fn missing_and_wrong_kind_paths_fail_without_range_or_text_lookup() {
        let snapshot = attach("proof ()() \n");
        let item = snapshot.typed_tree().unwrap().items().unwrap().remove(0);
        let proof = item.cast::<ProofItemKind>().unwrap();
        assert_eq!(
            item.name().unwrap().unwrap().kind(),
            SyntaxKind::MissingName
        );
        assert_eq!(item.recovery().unwrap().len(), 1);
        assert_eq!(item.recovery().unwrap()[0].kind(), SyntaxKind::ErrorNode);

        let proof_body = proof
            .required_exact_child::<ProofBodyKind>(SyntaxRole::Body)
            .unwrap();
        let DeclarationBodyNode::Missing(missing) = proof_body.content().unwrap() else {
            panic!("missing proof body remains an exact recovery node");
        };
        assert_eq!(missing.kind(), SyntaxKind::MissingBody);
        assert_eq!(missing.range().start(), missing.range().end());
        assert!(matches!(
            item.cast::<PredicateItemKind>(),
            Err(SyntaxLookupError::KindMismatch { .. })
        ));
        assert!(matches!(
            proof.required_exact_child::<PredicateBodyKind>(SyntaxRole::Body),
            Err(super::SyntaxAccessError::Lookup(
                SyntaxLookupError::KindMismatch { .. }
            ))
        ));
        assert!(matches!(
            FamilyNode::<TypeFamily>::new(item.syntax()),
            Err(super::SyntaxAccessError::FamilyMismatch { .. })
        ));

        let call_snapshot = attach("predicate broken() = outer(value\n");
        let predicate = call_snapshot
            .typed_tree()
            .unwrap()
            .items()
            .unwrap()
            .remove(0)
            .cast::<PredicateItemKind>()
            .unwrap();
        let body = predicate
            .required_exact_child::<PredicateBodyKind>(SyntaxRole::Body)
            .unwrap();
        let DeclarationBodyNode::Body(expression_body) = body.content().unwrap() else {
            panic!("predicate retains expression body");
        };
        let call = expression_body
            .cast::<ExpressionBodyKind>()
            .unwrap()
            .expression()
            .unwrap()
            .cast::<CallExpressionKind>()
            .unwrap();
        let close = call
            .required_family_child::<DelimiterFamily>(SyntaxRole::CloseDelimiter)
            .unwrap();
        assert_eq!(close.kind(), SyntaxKind::CloseParenNode);
        assert_eq!(close.range().start(), close.range().end());
    }

    #[test]
    fn dialogue_rich_text_owns_ordered_ranged_attached_descendants() {
        let source = concat!(
            "flow @flow.opening opening {\n",
            "    let line = alice[本文。",
            "[transform .offset x=4px pattern==value label='二 px' missing= bad=\\q]",
            "[fx warning(accent=\"urgent\")]",
            "[call flash(level=2)]",
            "[! blink(level=3)]",
            "[if player.ready]",
            "[.sparkle]",
            "[/]]\n",
            "}\n",
        );
        let snapshot = attach(source);
        let dialogue = snapshot
            .nodes()
            .find(|node| node.kind() == SyntaxKind::DialogueCallExpression)
            .expect("dialogue expression")
            .cast::<DialogueCallExpressionKind>()
            .unwrap();
        assert_eq!(snapshot.root_handle().rowan().text().to_string(), source);

        let tags = dialogue.rich_text_tags().unwrap();
        assert_eq!(tags.len(), 7);
        for (ordinal, tag) in tags.iter().enumerate() {
            assert_eq!(
                tag.role(),
                SyntaxRole::RichTextTag(u32::try_from(ordinal).unwrap())
            );
        }

        assert_rich_text_argument_descendants(source, &tags[0]);
        assert_rich_text_expression_payloads(&tags[1..5]);
        let sparkle = tags[5].cast::<RichTextTagKind>().unwrap();
        assert_eq!(
            sparkle.name().unwrap().syntax().rowan().text().to_string(),
            ".sparkle"
        );
        assert!(sparkle.payload().unwrap().is_none());
        assert!(
            tags[6]
                .cast::<RichTextEndTagKind>()
                .unwrap()
                .name()
                .unwrap()
                .is_none()
        );
    }

    fn assert_rich_text_argument_descendants(source: &str, tag: &RichTextNode) {
        let tag = tag.cast::<RichTextTagKind>().unwrap();
        assert_eq!(
            tag.name().unwrap().syntax().rowan().text().to_string(),
            "transform"
        );
        let payload = tag
            .payload()
            .unwrap()
            .unwrap()
            .cast::<RichTextArgumentPayloadKind>()
            .unwrap();
        let arguments = payload.arguments().unwrap();
        assert_eq!(arguments.len(), 6);
        for (ordinal, argument) in arguments.iter().enumerate() {
            assert_eq!(
                argument.role(),
                SyntaxRole::Argument(u16::try_from(ordinal).unwrap())
            );
        }

        assert_split_equals_argument(&arguments[2]);
        assert_quoted_argument(source, &arguments[3]);
        assert_missing_and_invalid_arguments(source, &arguments[4], &arguments[5]);
    }

    fn assert_split_equals_argument(argument: &RichTextNode) {
        let split_equals = argument.cast::<RichTextNamedArgumentKind>().unwrap();
        let equals = split_equals.equals().unwrap();
        assert_eq!(equals.syntax().rowan().text().to_string(), "=");
        assert_eq!(
            equals.syntax().rowan().first_token().unwrap().kind().0,
            SyntaxKind::PunctuationToken as u16
        );
        assert_eq!(
            split_equals
                .value()
                .unwrap()
                .cast::<RichTextArgumentValueKind>()
                .unwrap()
                .token()
                .unwrap()
                .content()
                .unwrap()
                .syntax()
                .rowan()
                .text()
                .to_string(),
            "=value"
        );
    }

    #[test]
    fn dialogue_non_tag_surfaces_do_not_gain_rich_text_identity() {
        let source = concat!(
            "flow @flow.opening opening {\n",
            "    let line = alice[本文。",
            "\\[effect .wave]",
            "#[score]",
            "$([effect .wave])",
            "|[base](ruby)",
            "[raw]literal [p][/raw]",
            "[raw: [p]x]",
            "[em:夢]",
            "[color #a8:night]",
            "[ruby rt=x]base[/ruby]",
            "[effect .wave]",
            "]\n",
            "}\n",
        );
        let snapshot = attach(source);
        let dialogue = snapshot
            .nodes()
            .find(|node| node.kind() == SyntaxKind::DialogueCallExpression)
            .expect("dialogue expression")
            .cast::<DialogueCallExpressionKind>()
            .unwrap();
        let tags = dialogue.rich_text_tags().unwrap();

        assert_eq!(tags.len(), 1);
        assert_eq!(
            tags[0]
                .cast::<RichTextTagKind>()
                .unwrap()
                .name()
                .unwrap()
                .syntax()
                .rowan()
                .text()
                .to_string(),
            "effect"
        );
        assert_eq!(tags[0].role(), SyntaxRole::RichTextTag(0));
        assert_eq!(snapshot.root_handle().rowan().text().to_string(), source);
    }

    fn assert_quoted_argument(source: &str, argument: &RichTextNode) {
        let quoted = argument.cast::<RichTextNamedArgumentKind>().unwrap();
        assert_eq!(
            quoted.key().unwrap().syntax().rowan().text().to_string(),
            "label"
        );
        assert_eq!(
            quoted.equals().unwrap().syntax().rowan().text().to_string(),
            "="
        );
        let value = quoted
            .value()
            .unwrap()
            .cast::<RichTextArgumentValueKind>()
            .unwrap();
        assert_eq!(&source[value.range().as_range()], "'二 px'");
        let token = value.token().unwrap();
        assert_eq!(
            token.content().unwrap().syntax().rowan().text().to_string(),
            "二 px"
        );
        assert_eq!(
            token
                .opening_quote()
                .unwrap()
                .unwrap()
                .syntax()
                .rowan()
                .text()
                .to_string(),
            "'"
        );
        assert_eq!(
            token
                .closing_quote()
                .unwrap()
                .unwrap()
                .syntax()
                .rowan()
                .text()
                .to_string(),
            "'"
        );
    }

    fn assert_missing_and_invalid_arguments(
        source: &str,
        missing: &RichTextNode,
        invalid: &RichTextNode,
    ) {
        let missing = missing.cast::<RichTextNamedArgumentKind>().unwrap();
        let missing_value = missing.value().unwrap();
        assert_eq!(
            missing_value.kind(),
            SyntaxKind::RichTextMissingArgumentValue
        );
        assert_eq!(missing_value.range().start(), missing_value.range().end());
        assert_eq!(missing_value.range().start(), missing.range().end());

        let invalid = invalid.cast::<RichTextInvalidArgumentKind>().unwrap();
        assert_eq!(&source[invalid.range().as_range()], "bad=\\q");
        assert_eq!(
            invalid.issue().unwrap().syntax().rowan().text().to_string(),
            "\\q"
        );
    }

    fn assert_rich_text_expression_payloads(tags: &[RichTextNode]) {
        let fx = tags[0]
            .cast::<RichTextTagKind>()
            .unwrap()
            .payload()
            .unwrap()
            .unwrap()
            .cast::<RichTextFxCallPayloadKind>()
            .unwrap();
        assert_eq!(
            fx.expression().unwrap().syntax().rowan().text().to_string(),
            "warning(accent=\"urgent\")"
        );
        let call = tags[1]
            .cast::<RichTextTagKind>()
            .unwrap()
            .payload()
            .unwrap()
            .unwrap()
            .cast::<RichTextDialogueCallPayloadKind>()
            .unwrap();
        assert_eq!(
            call.expression()
                .unwrap()
                .syntax()
                .rowan()
                .text()
                .to_string(),
            "flash(level=2)"
        );
        let bang = tags[2].cast::<RichTextTagKind>().unwrap();
        assert_eq!(
            bang.name().unwrap().syntax().rowan().text().to_string(),
            "!"
        );
        let bang = bang
            .payload()
            .unwrap()
            .unwrap()
            .cast::<RichTextDialogueCallPayloadKind>()
            .unwrap();
        assert_eq!(
            bang.expression()
                .unwrap()
                .syntax()
                .rowan()
                .text()
                .to_string(),
            "blink(level=3)"
        );
        let condition = tags[3]
            .cast::<RichTextTagKind>()
            .unwrap()
            .payload()
            .unwrap()
            .unwrap()
            .cast::<RichTextConditionPayloadKind>()
            .unwrap();
        assert_eq!(
            condition
                .expression()
                .unwrap()
                .syntax()
                .rowan()
                .text()
                .to_string(),
            "player.ready"
        );
    }

    #[test]
    fn attached_rich_text_ranges_match_the_public_lossless_scan() {
        let tag_source = "[effect .wave\u{3000}amp=2 label=\"游 ゴシック\"]";
        let source = format!(
            "flow @flow.opening opening {{\r\n    let line = alice[本文。{tag_source}]\r\n}}\r\n"
        );
        let snapshot = attach(&source);
        assert_eq!(snapshot.root_handle().rowan().text().to_string(), source);
        let dialogue = snapshot
            .nodes()
            .find(|node| node.kind() == SyntaxKind::DialogueCallExpression)
            .unwrap()
            .cast::<DialogueCallExpressionKind>()
            .unwrap();
        let private_tag = dialogue.rich_text_tags().unwrap().remove(0);
        let private_tag = private_tag.cast::<RichTextTagKind>().unwrap();
        let payload = private_tag
            .payload()
            .unwrap()
            .unwrap()
            .cast::<RichTextArgumentPayloadKind>()
            .unwrap();
        let private_arguments = payload.arguments().unwrap();

        let parsed = parse_dialogue_text(tag_source);
        assert!(
            parsed.diagnostics().is_empty(),
            "{:?}",
            parsed.diagnostics()
        );
        let public_tag = parsed
            .tokens()
            .iter()
            .find_map(|token| match token {
                DialogueToken::Tag(tag) => Some(tag),
                _ => None,
            })
            .unwrap();
        let base = source.find(tag_source).unwrap();
        assert_eq!(
            private_tag.range(),
            SourceRange::new(
                base + public_tag.range().start(),
                base + public_tag.range().end(),
            )
        );
        assert_eq!(private_arguments.len(), public_tag.arguments().len());
        for (private, public) in private_arguments.iter().zip(public_tag.arguments()) {
            assert_eq!(
                private.range(),
                SourceRange::new(base + public.range().start(), base + public.range().end())
            );
        }

        let private_label = private_arguments[2]
            .cast::<RichTextNamedArgumentKind>()
            .unwrap();
        let public_label = &public_tag.arguments()[2];
        let public_value = public_label.value().unwrap();
        assert_eq!(
            private_label.key().unwrap().range(),
            shifted(base, public_label.name_range().unwrap())
        );
        assert_eq!(
            private_label.equals().unwrap().range(),
            shifted(base, public_label.equals_range().unwrap())
        );
        let private_value = private_label
            .value()
            .unwrap()
            .cast::<RichTextArgumentValueKind>()
            .unwrap();
        let private_token = private_value.token().unwrap();
        assert_eq!(
            private_token.range(),
            shifted(base, public_value.token_range())
        );
        assert_eq!(
            private_token.content().unwrap().range(),
            shifted(base, public_value.content_range())
        );
        assert_eq!(
            private_token.opening_quote().unwrap().unwrap().range(),
            shifted(base, public_value.opening_quote_range().unwrap())
        );
        assert_eq!(
            private_token.closing_quote().unwrap().unwrap().range(),
            shifted(base, public_value.closing_quote_range().unwrap())
        );
    }

    fn shifted(base: usize, range: crate::ast::common::TextRange) -> SourceRange {
        SourceRange::new(base + range.start(), base + range.end())
    }

    #[test]
    fn equal_range_rich_text_recovery_nodes_keep_distinct_path_identity() {
        let source = concat!(
            "flow @flow.opening opening {\n",
            "    let line = alice[本文。[effect \\q]]\n",
            "}\n",
        );
        let snapshot = attach(source);
        let invalid = snapshot
            .nodes()
            .find(|node| node.kind() == SyntaxKind::RichTextInvalidArgument)
            .unwrap()
            .cast::<RichTextInvalidArgumentKind>()
            .unwrap();
        let issue = invalid.issue().unwrap();

        assert_eq!(invalid.range(), issue.range());
        assert_ne!(invalid.id(), issue.id());
        assert_eq!(
            snapshot.bind_rowan(invalid.syntax().rowan()).unwrap().id(),
            invalid.id()
        );
        assert_eq!(
            snapshot.bind_rowan(issue.syntax().rowan()).unwrap().id(),
            issue.id()
        );
        assert_eq!(snapshot.root_handle().rowan().text().to_string(), source);
    }

    #[test]
    fn nested_pattern_and_type_accessors_keep_exact_child_roles() {
        let source = "proof nested((head, [first, ..rest], TruckResult { score, rank: mut r, .. }, ev .Choice(value)): (&'a mut Comparator<Option<(Int, String)> | [U8; 32]>) -> Result<Bool, Error>, .Some(left) | .None: Option<Int>) where Comparator<Option<Int>>: Callable<(Int, String)> + Send = true\n";
        let snapshot = attach(source);

        let whole = snapshot
            .nodes()
            .find(|node| node.kind() == SyntaxKind::WholeBindingPattern)
            .unwrap()
            .cast::<WholeBindingPatternKind>()
            .unwrap();
        assert_eq!(whole.pattern().unwrap().kind(), SyntaxKind::VariantPattern);

        let record = snapshot
            .nodes()
            .find(|node| node.kind() == SyntaxKind::RecordPattern)
            .unwrap()
            .cast::<RecordPatternKind>()
            .unwrap();
        let fields = record.fields().unwrap();
        assert_eq!(fields.len(), 3);
        assert_eq!(fields[0].role(), SyntaxRole::Field(0));
        assert_eq!(fields[1].role(), SyntaxRole::Field(1));
        assert_eq!(fields[2].role(), SyntaxRole::Field(2));
        let shorthand = fields[0]
            .cast::<super::node::RecordPatternFieldKind>()
            .unwrap();
        let named = fields[1]
            .cast::<super::node::RecordPatternFieldKind>()
            .unwrap();
        assert_eq!(
            shorthand.pattern().unwrap().unwrap().kind(),
            SyntaxKind::BindingPattern
        );
        assert_eq!(
            named.pattern().unwrap().unwrap().kind(),
            SyntaxKind::MutableBindingPattern
        );
        assert_eq!(fields[2].kind(), SyntaxKind::RestPattern);

        let function = snapshot
            .nodes()
            .find(|node| node.kind() == SyntaxKind::FunctionType)
            .unwrap()
            .cast::<FunctionTypeKind>()
            .unwrap();
        assert_eq!(function.parameters().unwrap().len(), 1);
        assert_eq!(
            function.parameters().unwrap()[0].kind(),
            SyntaxKind::ReferenceType
        );
        assert_eq!(
            function.result().unwrap().kind(),
            SyntaxKind::GenericApplicationType
        );

        let generic = snapshot
            .nodes()
            .find(|node| node.kind() == SyntaxKind::GenericApplicationType)
            .unwrap()
            .cast::<GenericApplicationTypeKind>()
            .unwrap();
        let arguments = generic.arguments().unwrap();
        assert_eq!(arguments.len(), 1);
        assert_eq!(arguments[0].role(), SyntaxRole::Argument(0));
        assert_eq!(arguments[0].ty().unwrap().kind(), SyntaxKind::SumType);
    }
}
