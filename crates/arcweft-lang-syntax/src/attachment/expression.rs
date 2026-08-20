//! Parser-owned semantic expression projections bound to attached identities.

use std::collections::BTreeSet;

mod candidate_block;
mod candidate_control;
mod candidate_path;
mod match_expression;
mod structure;

pub use candidate_block::{
    AttachedCandidateAssertion, AttachedCandidateAssignment, AttachedCandidateBlockTail,
    AttachedCandidateControlLabel, AttachedCandidateIf, AttachedCandidateIfElse,
    AttachedCandidateIfHead, AttachedCandidateKeywordStatement, AttachedCandidateMatchArmBody,
    AttachedCandidateMatchArmStatement, AttachedCandidateMatchBody,
    AttachedCandidateMatchStatement, AttachedCandidateRequiredOperand, AttachedCandidateStatement,
    AttachedCandidateStatementBlock, AttachedCandidateStatementExpression,
    AttachedCandidateUnsafeAuditId, AttachedCandidateUnsafeBody, AttachedCandidateUnsafeLifetime,
    AttachedCandidateValueBlock,
};
pub use candidate_control::{
    AttachedCandidateClosure, AttachedCandidateClosureParameter, AttachedCandidateIfLet,
    AttachedCandidateMatch, AttachedCandidateMatchArm, AttachedCandidatePatternChild,
};
pub use candidate_path::{AttachedCandidateNominalTypeRoot, AttachedCandidatePathExpression};
pub use match_expression::{
    AttachedMatchArm, AttachedMatchArmComponent, AttachedMatchArmExpression,
};

use match_expression::attached_match_arms;
use structure::{
    attached_block, attached_call_type_children, attached_closure_children,
    attached_composite_children, attached_path, attached_path_projection, attached_pattern,
    validate_short_variant_shape,
};

use arcweft_source::{SourceRange, SourceSpan};

use super::choice::AttachedChoiceExpression;
use super::family::{
    ExprNode, ExpressionFamily, FamilyNode, FamilySpec, PatternFamily, RecoveryFamily, RecoveryNode,
};
use super::node::{
    AstNode, AwaitExpressionKind, AwaitWithBranchKind, BlockKind, ChoiceExpressionKind,
    CloseBraceKind, ErrorNodeKind, ExpressionFragmentRootKind, MissingBodyKind, OpenBraceKind,
    PathKind, ThreadExpressionKind,
};
use super::source_file::{
    AttachedDelimiterState, AttachedPath, AttachedPathRoot, AttachedPathSegmentKind,
};
use super::thread_body::{AttachedRequiredNestedThreadFlowBody, required_nested_thread_flow_body};
use super::{
    AttachedPatternNode, AttachedTypeRefNode, SyntaxAccessError, SyntaxNodeHandle, SyntaxNodeId,
    SyntaxSnapshotId,
};
use crate::assertion::AssertionMode;
use crate::expressions::{
    CandidateNodeIndex, ExpressionComponentRole, ExpressionProjection, ExpressionRecordFieldPart,
    PendingCandidateGraph, PendingCandidateSemantic, SyntaxAssociatedReceiver,
    SyntaxCallArgumentPart, SyntaxCallCalleeProjection, SyntaxCallProjection,
    SyntaxCallTypeApplicationComponentRole, SyntaxCallTypeArgumentPart,
    SyntaxCallTypeArgumentProjection, SyntaxCallTypeChildRole, SyntaxClosureParameterPart,
    SyntaxExpressionSlot, SyntaxRecordField,
};
use crate::grammar::keyword_statement_projection::PendingKeywordStatementProjection;
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};
use crate::grammar::source_projection::{
    PendingPathProjection, PendingPathRoot, PendingPathSegmentKind,
};
use crate::grammar::{SyntaxAwaitBranchKind, SyntaxRoleClass};
use crate::name::SyntaxNameIssue;
use crate::patterns::{PatternNodePath, PatternSyntaxNode, PatternSyntaxState};
use crate::types::{TypeRef, TypeRefNodePath, TypeRefNodeStep};

/// One exact revision-bound source component of an attached expression leaf.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedExpressionComponent {
    role: ExpressionComponentRole,
    source: SourceSpan,
}

impl AttachedExpressionComponent {
    pub const fn role(&self) -> ExpressionComponentRole {
        self.role
    }

    pub const fn source_span(&self) -> &SourceSpan {
        &self.source
    }
}

/// Revision-bound, read-only view of one retained ambiguous parse candidate.
///
/// Candidate nodes deliberately have no public syntax identity. Their local
/// indices remain an implementation detail of the parser-owned graph.
#[derive(Clone, Copy)]
pub struct AttachedCandidateGraph<'a> {
    owner: &'a ExprNode,
    graph: &'a PendingCandidateGraph,
    primary: Option<CandidateNodeIndex>,
    dialogue: Option<&'a crate::expressions::SyntaxPostfixDialogueCandidate>,
}

impl<'a> AttachedCandidateGraph<'a> {
    fn new(
        owner: &'a ExprNode,
        graph: &'a PendingCandidateGraph,
        primary: Option<CandidateNodeIndex>,
        dialogue: Option<&'a crate::expressions::SyntaxPostfixDialogueCandidate>,
    ) -> Self {
        Self {
            owner,
            graph,
            primary,
            dialogue,
        }
    }

    /// Primary candidate node when the candidate grammar selected one.
    ///
    /// The ordinary-index candidate has one primary index expression. A
    /// dialogue candidate is an ordered graph and therefore has no synthetic
    /// primary node.
    pub fn primary(self) -> Option<AttachedCandidateNode<'a>> {
        self.primary
            .map(|index| AttachedCandidateNode::new(self.owner, self.graph, index))
    }

    /// Typed dialogue content when this is the retained dialogue candidate.
    pub fn dialogue_content(
        self,
    ) -> Option<&'a crate::expressions::SyntaxDialogueContentProjection> {
        self.dialogue
            .map(crate::expressions::SyntaxPostfixDialogueCandidate::content)
    }

    /// Exact source components retained by the Dialogue interpretation.
    ///
    /// The candidate has no independent syntax identity. Every returned span
    /// remains attached to the source-backed outer postfix expression while
    /// the component role describes the selected Dialogue interpretation.
    pub fn dialogue_components(
        self,
    ) -> Option<impl ExactSizeIterator<Item = AttachedExpressionComponent> + 'a> {
        let candidate = self.dialogue?;
        Some(
            candidate
                .components()
                .iter()
                .map(move |component| AttachedExpressionComponent {
                    role: component.role(),
                    source: self.owner.syntax().source_span_for_range(component.range()),
                }),
        )
    }

    fn expression_roots(self) -> Vec<AttachedCandidateNode<'a>> {
        let mut roots = Vec::new();
        for root in self.graph.roots() {
            collect_candidate_expression_roots(self.owner, self.graph, *root, &mut roots);
        }
        roots
    }

    /// Nested expression slots owned by this retained Dialogue candidate.
    ///
    /// The typed content inventory supplies owner identity and slot state;
    /// candidate graph preorder supplies the unique attached expression node.
    /// Structural Dialogue wrappers never appear in the result.
    /// # Panics
    ///
    /// Panics if a validated Dialogue component is absent from its candidate graph.
    pub fn dialogue_expression_slots(
        self,
    ) -> Option<impl ExactSizeIterator<Item = AttachedCandidateDialogueExpression<'a>> + 'a> {
        let candidate = self.dialogue?;
        let crate::expressions::SyntaxDialogueContentProjection::Present(content) =
            candidate.content()
        else {
            return Some(Vec::new().into_iter());
        };
        let expected = dialogue_expression_specs(content);
        for spec in &expected {
            assert!(
                candidate
                    .components()
                    .iter()
                    .any(|component| component.role() == spec.component_role),
                "Dialogue candidate expression slots retain typed source components"
            );
        }

        let mut seen = BTreeSet::new();
        let bindings = self
            .expression_roots()
            .into_iter()
            .map(|attached| {
                let owner = attached
                    .nearest_dialogue_owner(attached.index)
                    .expect("Dialogue expression roots retain one typed content owner edge");
                let spec = expected
                    .iter()
                    .find(|spec| spec.owner == owner)
                    .expect("Dialogue typed content inventory owns every expression edge");
                assert!(
                    seen.insert(owner),
                    "Dialogue expression owner edges remain unique"
                );
                assert_eq!(
                    attached.role(),
                    spec.syntax_role,
                    "Dialogue expression roots retain their typed owner relation"
                );
                assert!(
                    matches!(
                        (spec.slot, attached.kind()),
                        (SyntaxExpressionSlot::Missing, SyntaxKind::MissingExpression)
                    ) || spec.slot == SyntaxExpressionSlot::Authored
                        && attached.kind() != SyntaxKind::MissingExpression,
                    "Dialogue candidate slot state must match its expression node"
                );
                AttachedCandidateDialogueExpression {
                    owner: spec.owner,
                    slot: spec.slot,
                    source: attached.source_span(),
                    node: attached,
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(
            seen.len(),
            expected.len(),
            "Dialogue candidate must bind every typed expression slot exactly once"
        );
        Some(bindings.into_iter())
    }
}

/// Content-local identity of one Dialogue expression owner.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AttachedCandidateDialogueOwner {
    /// One Dialogue content node identified by its typed content ordinal.
    Node { ordinal: u32 },
    /// One `RichText` tag identified by its typed content tag ordinal.
    Tag { ordinal: u32 },
}

/// One typed Dialogue expression slot bound to a candidate graph node.
#[derive(Clone)]
pub struct AttachedCandidateDialogueExpression<'a> {
    owner: AttachedCandidateDialogueOwner,
    slot: SyntaxExpressionSlot,
    source: SourceSpan,
    node: AttachedCandidateNode<'a>,
}

impl<'a> AttachedCandidateDialogueExpression<'a> {
    /// Content-local Dialogue node or tag identity.
    pub const fn owner(&self) -> AttachedCandidateDialogueOwner {
        self.owner
    }

    /// Authored or source-owned missing slot state.
    pub const fn slot(&self) -> SyntaxExpressionSlot {
        self.slot
    }

    /// Exact expression node span in the accepted outer revision.
    pub const fn source_span(&self) -> &SourceSpan {
        &self.source
    }

    /// Candidate-local typed expression node.
    pub const fn node(&self) -> AttachedCandidateNode<'a> {
        self.node
    }
}

#[derive(Clone, Copy)]
struct CandidateDialogueExpressionSpec {
    owner: AttachedCandidateDialogueOwner,
    slot: SyntaxExpressionSlot,
    component_role: ExpressionComponentRole,
    syntax_role: SyntaxRole,
}

fn dialogue_expression_specs(
    content: &crate::expressions::SyntaxDialogueContent,
) -> Vec<CandidateDialogueExpressionSpec> {
    let mut specs = Vec::new();
    for (ordinal, node) in content.nodes().iter().enumerate() {
        let crate::expressions::SyntaxDialogueNodeProjection::Interpolation(slot) = node else {
            continue;
        };
        let ordinal =
            u32::try_from(ordinal).expect("Dialogue node counts are bounded by syntax limits");
        specs.push(CandidateDialogueExpressionSpec {
            owner: AttachedCandidateDialogueOwner::Node { ordinal },
            slot: *slot,
            component_role: ExpressionComponentRole::DialogueNode {
                ordinal,
                part: crate::expressions::SyntaxDialogueNodeSourcePart::Interpolation,
            },
            syntax_role: SyntaxRole::Operand,
        });
    }
    for (ordinal, tag) in content.tags().iter().enumerate() {
        let (slot, syntax_role) = match tag.payload() {
            crate::expressions::SyntaxRichTextTagPayloadProjection::FxCall(slot)
            | crate::expressions::SyntaxRichTextTagPayloadProjection::DialogueCall(slot) => {
                (*slot, SyntaxRole::Operand)
            }
            crate::expressions::SyntaxRichTextTagPayloadProjection::Condition(slot) => {
                (*slot, SyntaxRole::Condition)
            }
            crate::expressions::SyntaxRichTextTagPayloadProjection::Arguments
            | crate::expressions::SyntaxRichTextTagPayloadProjection::None => continue,
        };
        let ordinal =
            u32::try_from(ordinal).expect("Rich-text tag counts are bounded by syntax limits");
        specs.push(CandidateDialogueExpressionSpec {
            owner: AttachedCandidateDialogueOwner::Tag { ordinal },
            slot,
            component_role: ExpressionComponentRole::RichTextTag {
                tag: ordinal,
                part: crate::expressions::SyntaxRichTextTagSourcePart::Payload,
            },
            syntax_role,
        });
    }
    specs
}

fn collect_candidate_expression_roots<'a>(
    owner: &'a ExprNode,
    graph: &'a PendingCandidateGraph,
    index: CandidateNodeIndex,
    roots: &mut Vec<AttachedCandidateNode<'a>>,
) {
    let node = graph
        .node(index)
        .expect("candidate traversal follows retained typed child edges");
    if ExpressionFamily::accepts(node.kind()) {
        roots.push(AttachedCandidateNode::new(owner, graph, index));
        return;
    }
    for child in graph
        .children(index)
        .expect("candidate traversal follows retained typed child edges")
    {
        collect_candidate_expression_roots(owner, graph, *child, roots);
    }
}

/// One revision-bound node borrowed from a retained candidate graph.
#[derive(Clone, Copy)]
pub struct AttachedCandidateNode<'a> {
    owner: &'a ExprNode,
    graph: &'a PendingCandidateGraph,
    index: CandidateNodeIndex,
}

impl<'a> AttachedCandidateNode<'a> {
    fn new(
        owner: &'a ExprNode,
        graph: &'a PendingCandidateGraph,
        index: CandidateNodeIndex,
    ) -> Self {
        Self {
            owner,
            graph,
            index,
        }
    }

    fn pending(self) -> &'a crate::expressions::PendingCandidateNode {
        self.graph
            .node(self.index)
            .expect("attached candidate nodes originate from their retained graph")
    }

    /// Exact parser-selected syntax family.
    pub fn kind(self) -> SyntaxKind {
        self.pending().kind()
    }

    /// Exact grammar relation within this candidate.
    pub fn role(self) -> SyntaxRole {
        self.pending().role()
    }

    /// Exact source span bound to the accepted outer expression revision.
    pub fn source_span(self) -> SourceSpan {
        self.owner
            .syntax()
            .source_span_for_range(self.pending().source())
    }

    /// Direct candidate-local children in authored order.
    /// # Panics
    ///
    /// Panics if the validated candidate graph contains a dangling child edge.
    pub fn children(self) -> impl ExactSizeIterator<Item = Self> + 'a {
        self.graph
            .children(self.index)
            .expect("attached candidate nodes retain validated child edges")
            .iter()
            .copied()
            .map(move |index| Self::new(self.owner, self.graph, index))
    }

    /// Direct semantic expression children, excluding structural wrappers.
    ///
    /// Each child keeps its candidate-local node view and receives the same
    /// stable grammar ordinal used by ordinary attached expression children.
    /// # Panics
    ///
    /// Panics if the projection and retained semantic-child inventory disagree.
    pub fn semantic_expression_children(
        self,
    ) -> impl ExactSizeIterator<Item = AttachedCandidateExpressionChild<'a>> + 'a {
        let Some(projection) = self.expression_projection() else {
            return Vec::new().into_iter();
        };
        let PendingCandidateSemantic::Expression(pending) = self.pending().semantic() else {
            unreachable!("expression projection is backed by expression semantics");
        };
        let nodes = self.direct_semantic_expression_nodes();
        let mut specs =
            candidate_semantic_child_specs(self, projection, pending.components(), &nodes);
        if matches!(projection, ExpressionProjection::Error) && specs.is_empty() {
            specs.extend(nodes.first().map(|node| {
                CandidateSemanticChildSpec {
                    ordinal: 0,
                    slot: SyntaxExpressionSlot::Authored,
                    component_role: ExpressionComponentRole::Recovery,
                    syntax_role: node.role(),
                    source: pending
                        .components()
                        .iter()
                        .find(|component| component.role() == ExpressionComponentRole::Recovery)
                        .map_or_else(|| node.pending().source(), |component| component.range()),
                }
            }));
        }
        assert_eq!(
            nodes.len(),
            specs.len(),
            "candidate expression projection must declare every semantic child"
        );
        let children = specs
            .into_iter()
            .zip(nodes)
            .map(|(spec, node)| {
                assert_eq!(
                    node.role(),
                    spec.syntax_role,
                    "candidate semantic child retains its parser-owned typed edge"
                );
                let source = self.owner.syntax().source_span_for_range(spec.source);
                match (spec.slot, node.kind(), node.expression_projection()) {
                    (SyntaxExpressionSlot::Missing, SyntaxKind::MissingExpression, _) => {
                        AttachedCandidateExpressionChild::Missing {
                            ordinal: spec.ordinal,
                            component_role: spec.component_role,
                            source,
                            node,
                        }
                    }
                    (SyntaxExpressionSlot::Authored, kind, Some(ExpressionProjection::Error))
                        if kind != SyntaxKind::MissingExpression =>
                    {
                        AttachedCandidateExpressionChild::Recovered {
                            ordinal: spec.ordinal,
                            component_role: spec.component_role,
                            source,
                            node,
                        }
                    }
                    (SyntaxExpressionSlot::Authored, kind, Some(_))
                        if kind != SyntaxKind::MissingExpression =>
                    {
                        AttachedCandidateExpressionChild::Authored {
                            ordinal: spec.ordinal,
                            component_role: spec.component_role,
                            source,
                            node,
                        }
                    }
                    _ => panic!("candidate semantic slot and expression recovery disagree"),
                }
            })
            .collect::<Vec<_>>();
        children.into_iter()
    }

    fn direct_semantic_expression_nodes(self) -> Vec<Self> {
        let mut nodes = Vec::new();
        for child in self.children() {
            self.collect_direct_semantic_expression_nodes(child, &mut nodes);
        }
        nodes
    }

    fn collect_direct_semantic_expression_nodes(self, node: Self, nodes: &mut Vec<Self>) {
        if matches!(
            self.expression_projection(),
            Some(ExpressionProjection::Match(_))
        ) && node.kind() == SyntaxKind::MatchArm
        {
            // MatchArm is a typed semantic scope boundary. Its guard and value
            // are read through `match_view()` and remain outside the Match
            // expression's direct semantic-child inventory.
            return;
        }
        if ExpressionFamily::accepts(node.kind()) {
            nodes.push(node);
            return;
        }
        for child in node.children() {
            self.collect_direct_semantic_expression_nodes(child, nodes);
        }
    }

    /// Exact source components retained by this expression payload.
    pub fn expression_components(
        self,
    ) -> Option<impl ExactSizeIterator<Item = AttachedExpressionComponent> + 'a> {
        let PendingCandidateSemantic::Expression(projection) = self.pending().semantic() else {
            return None;
        };
        Some(
            projection
                .components()
                .iter()
                .map(move |component| AttachedExpressionComponent {
                    role: component.role(),
                    source: self.owner.syntax().source_span_for_range(component.range()),
                }),
        )
    }

    fn require_expression_component(self, role: ExpressionComponentRole) {
        let PendingCandidateSemantic::Expression(projection) = self.pending().semantic() else {
            panic!("expression component lookups require expression semantics");
        };
        assert!(
            projection
                .components()
                .iter()
                .any(|component| component.role() == role),
            "candidate Call type roots retain typed source components"
        );
    }

    /// Direct type roots semantically owned by this candidate Call.
    ///
    /// Structural `TypeArgument` and delimiter nodes are skipped. The returned
    /// roots remain in candidate preorder and retain the same typed Call
    /// relation as ordinary attached Call type children.
    ///
    /// # Panics
    ///
    /// Panics if the parser-owned candidate graph does not retain the exact
    /// type roots declared by its typed Call projection.
    #[allow(
        clippy::too_many_lines,
        reason = "the closed Call type-root projection validates every typed receiver and argument edge together"
    )]
    pub fn direct_semantic_type_roots(
        self,
    ) -> impl ExactSizeIterator<Item = AttachedCandidateTypeRoot<'a>> + 'a {
        let Some(ExpressionProjection::Call(SyntaxCallProjection::Parenthesized(call))) =
            self.expression_projection()
        else {
            return Vec::new().into_iter();
        };
        let mut expected = Vec::new();
        match call.callee() {
            SyntaxCallCalleeProjection::Ordinary => {}
            SyntaxCallCalleeProjection::UnresolvedDot { .. } => {
                let callees = self
                    .children()
                    .filter(|child| child.role() == SyntaxRole::Callee)
                    .collect::<Vec<_>>();
                let [callee] = callees.as_slice() else {
                    panic!("candidate unresolved-dot Call must retain one value callee")
                };
                let AttachedCandidatePathExpression::NominalType(receiver) = callee
                    .path_expression_view()
                    .expect("candidate unresolved-dot Call retains one typed Path interpretation")
                else {
                    panic!("candidate unresolved-dot Call requires a nominal Path interpretation")
                };
                self.require_expression_component(ExpressionComponentRole::CallAssociatedReceiver);
                expected.push((
                    SyntaxCallTypeChildRole::DotNominalReceiver,
                    callee.index,
                    Some(receiver.node().index),
                ));
            }
            SyntaxCallCalleeProjection::Associated {
                receiver: SyntaxAssociatedReceiver::Present,
                ..
            } => {
                self.require_expression_component(ExpressionComponentRole::CallAssociatedReceiver);
                expected.push((
                    SyntaxCallTypeChildRole::AssociatedReceiver,
                    self.index,
                    None,
                ));
            }
        }
        if let Some(application) = call.explicit_type_application() {
            for (argument, projection) in application.arguments().iter().enumerate() {
                if matches!(projection, SyntaxCallTypeArgumentProjection::Missing) {
                    continue;
                }
                let argument = u16::try_from(argument)
                    .expect("candidate Call type arguments are bounded by syntax limits");
                self.require_expression_component(ExpressionComponentRole::CallTypeApplication(
                    SyntaxCallTypeApplicationComponentRole::Argument {
                        argument,
                        part: SyntaxCallTypeArgumentPart::Type,
                    },
                ));
                expected.push((
                    SyntaxCallTypeChildRole::ExplicitCallTypeArgument { ordinal: argument },
                    self.index,
                    None,
                ));
            }
        }
        let roots = self
            .graph
            .nodes()
            .iter()
            .enumerate()
            .filter_map(|(position, node)| {
                let PendingCandidateSemantic::Type(projection) = node.semantic() else {
                    return None;
                };
                if !projection.path().steps().is_empty() {
                    return None;
                }
                let index = CandidateNodeIndex::try_new(position)
                    .expect("candidate graph node counts are bounded by their local index");
                let semantic_parent = self.nearest_expression_or_type_parent(index)?;
                expected
                    .iter()
                    .any(|(_, owner, exact)| {
                        *owner == semantic_parent && exact.is_none_or(|expected| expected == index)
                    })
                    .then(|| {
                        (
                            semantic_parent,
                            index,
                            Self::new(self.owner, self.graph, index),
                        )
                    })
            })
            .collect::<Vec<_>>();
        assert_eq!(
            roots.len(),
            expected.len(),
            "candidate Call must retain every declared direct type root"
        );
        expected
            .into_iter()
            .zip(roots)
            .map(|((role, owner, exact), (semantic_parent, index, node))| {
                assert_eq!(
                    semantic_parent, owner,
                    "candidate Call type roots retain their typed semantic parent edge"
                );
                assert!(
                    exact.is_none_or(|expected| expected == index),
                    "candidate Call type roots retain their exact typed root edge"
                );
                AttachedCandidateTypeRoot {
                    role,
                    source: node.source_span(),
                    node,
                }
            })
            .collect::<Vec<_>>()
            .into_iter()
    }

    /// Direct semantic type children of this candidate type node.
    ///
    /// Child paths must extend the parent path by exactly one typed step and
    /// the nearest expression-or-type semantic ancestor must be this node.
    /// # Panics
    ///
    /// Panics if a retained type child no longer resolves in its authored tree.
    pub fn direct_semantic_type_children(
        self,
    ) -> impl ExactSizeIterator<Item = AttachedCandidateTypeChild<'a>> + 'a {
        let PendingCandidateSemantic::Type(parent) = self.pending().semantic() else {
            return Vec::new().into_iter();
        };
        let parent_steps = parent.path().steps();
        self.graph
            .nodes()
            .iter()
            .enumerate()
            .filter_map(move |(position, node)| {
                let PendingCandidateSemantic::Type(child) = node.semantic() else {
                    return None;
                };
                let steps = child.path().steps();
                if child.tree() != parent.tree()
                    || steps.len() != parent_steps.len() + 1
                    || !steps.starts_with(parent_steps)
                {
                    return None;
                }
                let index = CandidateNodeIndex::try_new(position)
                    .expect("candidate graph node counts are bounded by their local index");
                if self.nearest_expression_or_type_parent(index) != Some(self.index) {
                    return None;
                }
                let node = Self::new(self.owner, self.graph, index);
                Some(AttachedCandidateTypeChild {
                    step: *steps
                        .last()
                        .expect("direct candidate type child paths add one step"),
                    source: node.source_span(),
                    node,
                })
            })
            .collect::<Vec<_>>()
            .into_iter()
    }

    fn nearest_expression_or_type_parent(
        self,
        candidate: CandidateNodeIndex,
    ) -> Option<CandidateNodeIndex> {
        let mut parent = self.graph.node(candidate)?.parent();
        while let Some(index) = parent {
            let ancestor = self.graph.node(index)?;
            if matches!(
                ancestor.semantic(),
                PendingCandidateSemantic::Expression(_) | PendingCandidateSemantic::Type(_)
            ) {
                return Some(index);
            }
            parent = ancestor.parent();
        }
        None
    }

    fn nearest_dialogue_owner(
        self,
        candidate: CandidateNodeIndex,
    ) -> Option<AttachedCandidateDialogueOwner> {
        let mut parent = self.graph.node(candidate)?.parent();
        while let Some(index) = parent {
            let ancestor = self.graph.node(index)?;
            match ancestor.role() {
                SyntaxRole::DialogueNode(ordinal) => {
                    return Some(AttachedCandidateDialogueOwner::Node { ordinal });
                }
                SyntaxRole::RichTextTag(ordinal) => {
                    return Some(AttachedCandidateDialogueOwner::Tag { ordinal });
                }
                _ => parent = ancestor.parent(),
            }
        }
        None
    }

    /// Expression payload, when this is an expression semantic node.
    pub fn expression_projection(self) -> Option<&'a ExpressionProjection> {
        let PendingCandidateSemantic::Expression(projection) = self.pending().semantic() else {
            return None;
        };
        Some(projection.projection())
    }

    /// Assertion payload, when this is an assertion semantic node.
    pub fn assertion_projection(self) -> Option<AttachedCandidateAssertionProjection> {
        let PendingCandidateSemantic::Assertion(projection) = self.pending().semantic() else {
            return None;
        };
        Some(AttachedCandidateAssertionProjection {
            mode: projection.mode(),
        })
    }

    pub(crate) fn keyword_statement_projection(
        self,
    ) -> Option<&'a PendingKeywordStatementProjection> {
        let PendingCandidateSemantic::KeywordStatement(projection) = self.pending().semantic()
        else {
            return None;
        };
        Some(projection)
    }

    /// Type payload, when this is a type semantic node.
    /// # Panics
    ///
    /// Panics if a validated type projection no longer resolves in its authored tree.
    pub fn type_projection(self) -> Option<AttachedCandidateTypeProjection<'a>> {
        let PendingCandidateSemantic::Type(projection) = self.pending().semantic() else {
            return None;
        };
        Some(AttachedCandidateTypeProjection {
            path: projection.path(),
            value: projection
                .authored()
                .value_at(projection.path())
                .expect("candidate graph validates typed projections"),
        })
    }

    /// Pattern payload, when this is a pattern semantic node.
    pub fn pattern_projection(self) -> Option<AttachedCandidatePatternProjection<'a>> {
        let PendingCandidateSemantic::Pattern(projection) = self.pending().semantic() else {
            return None;
        };
        Some(AttachedCandidatePatternProjection {
            owner: self.owner,
            graph: self.graph,
            index: self.index,
            projection,
        })
    }

    /// Path payload, when this is a path semantic node.
    pub fn path_projection(self) -> Option<AttachedCandidatePathProjection<'a>> {
        let PendingCandidateSemantic::Path(projection) = self.pending().semantic() else {
            return None;
        };
        Some(AttachedCandidatePathProjection {
            owner: self.owner,
            graph: self.graph,
            index: self.index,
            projection,
        })
    }

    /// Nested retained ordinary-index interpretation, when this candidate node
    /// is itself an ambiguous postfix bracket.
    pub fn ambiguous_index_candidate(self) -> Option<AttachedCandidateGraph<'a>> {
        let Some(ExpressionProjection::PostfixBracket(
            crate::expressions::SyntaxPostfixBracketProjection::Ambiguous { index, .. },
        )) = self.expression_projection()
        else {
            return None;
        };
        Some(AttachedCandidateGraph::new(
            self.owner,
            index.graph(),
            Some(index.index()),
            None,
        ))
    }

    /// Nested retained dialogue interpretation, when this candidate node is
    /// itself an ambiguous postfix bracket.
    pub fn ambiguous_dialogue_candidate(self) -> Option<AttachedCandidateGraph<'a>> {
        let Some(ExpressionProjection::PostfixBracket(
            crate::expressions::SyntaxPostfixBracketProjection::Ambiguous { dialogue, .. },
        )) = self.expression_projection()
        else {
            return None;
        };
        Some(AttachedCandidateGraph::new(
            self.owner,
            dialogue.graph(),
            None,
            Some(dialogue),
        ))
    }
}

/// One projection-declared semantic expression child inside a candidate graph.
#[derive(Clone)]
pub enum AttachedCandidateExpressionChild<'a> {
    Authored {
        ordinal: u32,
        component_role: ExpressionComponentRole,
        source: SourceSpan,
        node: AttachedCandidateNode<'a>,
    },
    Recovered {
        ordinal: u32,
        component_role: ExpressionComponentRole,
        source: SourceSpan,
        node: AttachedCandidateNode<'a>,
    },
    Missing {
        ordinal: u32,
        component_role: ExpressionComponentRole,
        source: SourceSpan,
        node: AttachedCandidateNode<'a>,
    },
}

impl<'a> AttachedCandidateExpressionChild<'a> {
    /// Projection-declared semantic-child ordinal.
    pub const fn ordinal(&self) -> u32 {
        match self {
            Self::Authored { ordinal, .. }
            | Self::Recovered { ordinal, .. }
            | Self::Missing { ordinal, .. } => *ordinal,
        }
    }

    /// Projection slot state retained independently of node family.
    pub const fn slot(&self) -> SyntaxExpressionSlot {
        match self {
            Self::Authored { .. } | Self::Recovered { .. } => SyntaxExpressionSlot::Authored,
            Self::Missing { .. } => SyntaxExpressionSlot::Missing,
        }
    }

    /// Exact source-component relation declared by the projection.
    pub const fn component_role(&self) -> ExpressionComponentRole {
        match self {
            Self::Authored { component_role, .. }
            | Self::Recovered { component_role, .. }
            | Self::Missing { component_role, .. } => *component_role,
        }
    }

    /// Exact component span in the accepted outer revision.
    pub const fn source_span(&self) -> &SourceSpan {
        match self {
            Self::Authored { source, .. }
            | Self::Recovered { source, .. }
            | Self::Missing { source, .. } => source,
        }
    }

    /// Candidate-local node carrying the typed projection or recovery.
    pub const fn node(&self) -> AttachedCandidateNode<'a> {
        match self {
            Self::Authored { node, .. }
            | Self::Recovered { node, .. }
            | Self::Missing { node, .. } => *node,
        }
    }
}

#[derive(Clone, Copy)]
struct CandidateSemanticChildSpec {
    ordinal: u32,
    slot: SyntaxExpressionSlot,
    component_role: ExpressionComponentRole,
    syntax_role: SyntaxRole,
    source: SourceRange,
}

#[derive(Clone, Copy)]
struct CandidateSemanticSpecBuilder<'a> {
    components: &'a [crate::expressions::PendingExpressionComponent],
}

impl CandidateSemanticSpecBuilder<'_> {
    fn source(self, role: ExpressionComponentRole) -> SourceRange {
        self.components
            .iter()
            .find(|component| component.role() == role)
            .map(|component| component.range())
            .expect("candidate expression slots retain exact source components")
    }

    fn spec(
        self,
        ordinal: u32,
        slot: SyntaxExpressionSlot,
        component_role: ExpressionComponentRole,
        syntax_role: SyntaxRole,
    ) -> CandidateSemanticChildSpec {
        CandidateSemanticChildSpec {
            ordinal,
            slot,
            component_role,
            syntax_role,
            source: self.source(component_role),
        }
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "the wildcard-free expression-family child table is one auditable projection authority"
)]
fn candidate_semantic_child_specs(
    owner: AttachedCandidateNode<'_>,
    projection: &ExpressionProjection,
    components: &[crate::expressions::PendingExpressionComponent],
    nodes: &[AttachedCandidateNode<'_>],
) -> Vec<CandidateSemanticChildSpec> {
    let builder = CandidateSemanticSpecBuilder { components };
    if let Some(children) = candidate_branch_child_specs(projection, builder) {
        return children;
    }
    match projection {
        ExpressionProjection::Tuple(slots) | ExpressionProjection::BracketSequence(slots) => slots
            .iter()
            .copied()
            .enumerate()
            .map(|(ordinal, slot)| {
                let ordinal = u32::try_from(ordinal)
                    .expect("candidate expression child counts are bounded by syntax limits");
                builder.spec(
                    ordinal,
                    slot,
                    ExpressionComponentRole::Element { ordinal },
                    SyntaxRole::Element(ordinal),
                )
            })
            .collect(),
        ExpressionProjection::ArrayRepeat([value, length]) => vec![
            builder.spec(
                0,
                *value,
                ExpressionComponentRole::RepeatValue,
                SyntaxRole::Element(0),
            ),
            builder.spec(
                1,
                *length,
                ExpressionComponentRole::RepeatLength,
                SyntaxRole::Element(1),
            ),
        ],
        ExpressionProjection::Call(call) => candidate_call_child_specs(call, builder),
        ExpressionProjection::Select(_) | ExpressionProjection::PostfixBracket(_) => {
            vec![builder.spec(
                0,
                SyntaxExpressionSlot::Authored,
                ExpressionComponentRole::Target,
                SyntaxRole::Target,
            )]
        }
        ExpressionProjection::Index(index) => vec![
            builder.spec(
                0,
                index.target(),
                ExpressionComponentRole::Target,
                SyntaxRole::Target,
            ),
            builder.spec(
                1,
                index.index(),
                ExpressionComponentRole::Index,
                SyntaxRole::Argument(0),
            ),
        ],
        ExpressionProjection::DialogueContentApplication(application) => {
            candidate_dialogue_child_specs(owner, application, builder, nodes)
        }
        ExpressionProjection::Pipe([left, right])
        | ExpressionProjection::Binary { left, right, .. } => vec![
            builder.spec(
                0,
                *left,
                ExpressionComponentRole::LeftOperand,
                SyntaxRole::LeftOperand,
            ),
            builder.spec(
                1,
                *right,
                ExpressionComponentRole::RightOperand,
                SyntaxRole::RightOperand,
            ),
        ],
        ExpressionProjection::Try { operand, .. }
        | ExpressionProjection::Await { operand, .. }
        | ExpressionProjection::Borrow { operand, .. }
        | ExpressionProjection::Dereference { operand }
        | ExpressionProjection::Unary { operand, .. } => {
            vec![builder.spec(
                0,
                *operand,
                ExpressionComponentRole::Operand,
                SyntaxRole::Operand,
            )]
        }
        ExpressionProjection::Range { start, end, .. } => {
            let mut children = Vec::with_capacity(2);
            if let Some(start) = start {
                children.push(builder.spec(
                    0,
                    *start,
                    ExpressionComponentRole::RangeStart,
                    SyntaxRole::LeftOperand,
                ));
            }
            if let Some(end) = end {
                children.push(builder.spec(
                    1,
                    *end,
                    ExpressionComponentRole::RangeEnd,
                    SyntaxRole::RightOperand,
                ));
            }
            children
        }
        ExpressionProjection::Record(fields) | ExpressionProjection::RecordLiteral(fields) => {
            fields
                .iter()
                .enumerate()
                .filter_map(|(field, projection)| {
                    let slot = projection.value()?;
                    let field = u32::try_from(field)
                        .expect("candidate record fields are bounded by syntax limits");
                    Some(builder.spec(
                        field,
                        slot,
                        ExpressionComponentRole::RecordField {
                            field,
                            part: ExpressionRecordFieldPart::Value,
                        },
                        SyntaxRole::Initializer,
                    ))
                })
                .collect()
        }
        ExpressionProjection::Closure(closure) => {
            vec![builder.spec(
                0,
                closure.body(),
                ExpressionComponentRole::Body,
                SyntaxRole::Body,
            )]
        }
        ExpressionProjection::If { .. } | ExpressionProjection::IfLet { .. } => {
            unreachable!("branch projections return before generic candidate projection")
        }
        ExpressionProjection::Match(projection) => {
            vec![builder.spec(
                0,
                projection.scrutinee(),
                ExpressionComponentRole::Scrutinee,
                SyntaxRole::Scrutinee,
            )]
        }
        ExpressionProjection::Unit
        | ExpressionProjection::Literal(_)
        | ExpressionProjection::EntityReference(_)
        | ExpressionProjection::LifetimePath(_)
        | ExpressionProjection::Path
        | ExpressionProjection::ShortVariant(_)
        | ExpressionProjection::Placeholder(_)
        | ExpressionProjection::NumericBracketSequence(_)
        | ExpressionProjection::Block
        | ExpressionProjection::ComputationBlock(_)
        | ExpressionProjection::NamedBlock(_)
        | ExpressionProjection::Loop
        | ExpressionProjection::Thread(_)
        | ExpressionProjection::Choice
        | ExpressionProjection::Error => Vec::new(),
    }
}

fn candidate_branch_child_specs(
    projection: &ExpressionProjection,
    builder: CandidateSemanticSpecBuilder<'_>,
) -> Option<Vec<CandidateSemanticChildSpec>> {
    match projection {
        ExpressionProjection::If {
            condition,
            then_branch,
            else_branch,
        } => {
            let mut children = vec![
                builder.spec(
                    0,
                    *condition,
                    ExpressionComponentRole::Condition,
                    SyntaxRole::Condition,
                ),
                builder.spec(
                    1,
                    *then_branch,
                    ExpressionComponentRole::ThenBranch,
                    SyntaxRole::ThenBranch,
                ),
            ];
            children.extend(else_branch.map(|slot| {
                builder.spec(
                    2,
                    slot,
                    ExpressionComponentRole::ElseBranch,
                    SyntaxRole::ElseBranch,
                )
            }));
            Some(children)
        }
        ExpressionProjection::IfLet {
            scrutinee,
            guard,
            then_branch,
            else_branch,
        } => {
            let mut children = vec![builder.spec(
                0,
                *scrutinee,
                ExpressionComponentRole::Scrutinee,
                SyntaxRole::Scrutinee,
            )];
            children.extend(guard.map(|slot| {
                builder.spec(1, slot, ExpressionComponentRole::Guard, SyntaxRole::Guard)
            }));
            children.push(builder.spec(
                2,
                *then_branch,
                ExpressionComponentRole::ThenBranch,
                SyntaxRole::ThenBranch,
            ));
            children.extend(else_branch.map(|slot| {
                builder.spec(
                    3,
                    slot,
                    ExpressionComponentRole::ElseBranch,
                    SyntaxRole::ElseBranch,
                )
            }));
            Some(children)
        }
        _ => None,
    }
}

fn candidate_call_child_specs(
    call: &SyntaxCallProjection,
    builder: CandidateSemanticSpecBuilder<'_>,
) -> Vec<CandidateSemanticChildSpec> {
    match call {
        SyntaxCallProjection::CallbackBlock(callback) => vec![
            builder.spec(
                0,
                SyntaxExpressionSlot::Authored,
                ExpressionComponentRole::CallCallee,
                SyntaxRole::Callee,
            ),
            builder.spec(
                1,
                callback.callback(),
                ExpressionComponentRole::CallArgument {
                    argument: 0,
                    part: SyntaxCallArgumentPart::Value,
                },
                SyntaxRole::Argument(0),
            ),
        ],
        SyntaxCallProjection::Parenthesized(call) => {
            let mut children = Vec::with_capacity(call.arguments().len() + 1);
            match call.callee() {
                SyntaxCallCalleeProjection::Ordinary => children.push(builder.spec(
                    0,
                    SyntaxExpressionSlot::Authored,
                    ExpressionComponentRole::CallCallee,
                    SyntaxRole::Callee,
                )),
                SyntaxCallCalleeProjection::UnresolvedDot { .. } => children.push(builder.spec(
                    0,
                    SyntaxExpressionSlot::Authored,
                    ExpressionComponentRole::CallAssociatedReceiver,
                    SyntaxRole::Callee,
                )),
                SyntaxCallCalleeProjection::Associated { .. } => {}
            }
            children.extend(
                call.arguments()
                    .iter()
                    .enumerate()
                    .map(|(argument, value)| {
                        let argument = u16::try_from(argument)
                            .expect("candidate call arguments are bounded by syntax limits");
                        builder.spec(
                            u32::from(argument) + 1,
                            value.value(),
                            ExpressionComponentRole::CallArgument {
                                argument,
                                part: SyntaxCallArgumentPart::Value,
                            },
                            SyntaxRole::Operand,
                        )
                    }),
            );
            children
        }
    }
}

fn candidate_dialogue_child_specs(
    owner: AttachedCandidateNode<'_>,
    application: &crate::expressions::SyntaxDialogueApplicationProjection,
    builder: CandidateSemanticSpecBuilder<'_>,
    nodes: &[AttachedCandidateNode<'_>],
) -> Vec<CandidateSemanticChildSpec> {
    let mut children = vec![builder.spec(
        0,
        SyntaxExpressionSlot::Authored,
        ExpressionComponentRole::Target,
        SyntaxRole::Target,
    )];
    let crate::expressions::SyntaxDialogueContentProjection::Present(content) =
        application.content()
    else {
        return children;
    };
    let expected = dialogue_expression_specs(content);
    let mut seen = BTreeSet::new();
    children.extend(nodes.iter().enumerate().skip(1).map(|(position, node)| {
        let dialogue_owner = owner
            .nearest_dialogue_owner(node.index)
            .expect("Dialogue semantic children retain one typed content owner edge");
        let spec = expected
            .iter()
            .find(|spec| spec.owner == dialogue_owner)
            .expect("Dialogue typed content inventory owns every semantic child edge");
        assert!(
            seen.insert(dialogue_owner),
            "Dialogue semantic child owner edges remain unique"
        );
        builder.spec(
            u32::try_from(position)
                .expect("dialogue expression counts are bounded by syntax limits"),
            spec.slot,
            spec.component_role,
            spec.syntax_role,
        )
    }));
    assert_eq!(
        seen.len(),
        expected.len(),
        "Dialogue semantic children bind every typed expression slot exactly once"
    );
    children
}

/// Typed assertion payload retained by a candidate node.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AttachedCandidateAssertionProjection {
    mode: Option<AssertionMode>,
}

impl AttachedCandidateAssertionProjection {
    /// Canonical assertion mode, absent only for parser-owned recovery.
    pub const fn mode(self) -> Option<AssertionMode> {
        self.mode
    }
}

/// Typed type value and structural path retained by a candidate node.
#[derive(Clone, Copy)]
pub struct AttachedCandidateTypeProjection<'a> {
    path: &'a TypeRefNodePath,
    value: &'a TypeRef,
}

/// One direct type root owned by a candidate Call expression.
#[derive(Clone)]
pub struct AttachedCandidateTypeRoot<'a> {
    role: SyntaxCallTypeChildRole,
    source: SourceSpan,
    node: AttachedCandidateNode<'a>,
}

impl<'a> AttachedCandidateTypeRoot<'a> {
    /// Typed Call relation retained from the parser transaction.
    pub const fn role(&self) -> SyntaxCallTypeChildRole {
        self.role
    }

    /// Candidate-local type root node.
    pub const fn node(&self) -> AttachedCandidateNode<'a> {
        self.node
    }

    /// Exact source occupied by the type root in the accepted revision.
    pub const fn source_span(&self) -> &SourceSpan {
        &self.source
    }
}

/// One direct typed edge and child node inside a candidate type tree.
#[derive(Clone)]
pub struct AttachedCandidateTypeChild<'a> {
    step: TypeRefNodeStep,
    source: SourceSpan,
    node: AttachedCandidateNode<'a>,
}

impl<'a> AttachedCandidateTypeChild<'a> {
    /// Exact structural type edge from the semantic parent.
    pub const fn step(&self) -> TypeRefNodeStep {
        self.step
    }

    /// Candidate-local node selected by this typed edge.
    pub const fn node(&self) -> AttachedCandidateNode<'a> {
        self.node
    }

    /// Exact child type source in the accepted revision.
    pub const fn source_span(&self) -> &SourceSpan {
        &self.source
    }
}

impl<'a> AttachedCandidateTypeProjection<'a> {
    pub const fn path(self) -> &'a TypeRefNodePath {
        self.path
    }

    pub const fn value(self) -> &'a TypeRef {
        self.value
    }
}

/// Typed pattern value and structural path retained by a candidate node.
#[derive(Clone, Copy)]
pub struct AttachedCandidatePatternProjection<'a> {
    owner: &'a ExprNode,
    graph: &'a PendingCandidateGraph,
    index: CandidateNodeIndex,
    projection: &'a crate::grammar::event::PendingPatternProjection,
}

impl<'a> AttachedCandidatePatternProjection<'a> {
    pub const fn path(self) -> &'a PatternNodePath {
        self.projection.path()
    }

    /// # Panics
    ///
    /// Panics if the validated pattern path no longer resolves in its authored tree.
    pub fn value(self) -> &'a PatternSyntaxNode {
        self.projection
            .authored()
            .value_at(self.projection.path())
            .expect("candidate graph validates pattern projections")
    }
}

/// Parser-validated path payload retained without source-text reconstruction.
#[derive(Clone, Copy)]
pub struct AttachedCandidatePathProjection<'a> {
    owner: &'a ExprNode,
    graph: &'a PendingCandidateGraph,
    index: CandidateNodeIndex,
    projection: &'a PendingPathProjection,
}

impl<'a> AttachedCandidatePathProjection<'a> {
    /// Root semantics and exact revision-bound root spans.
    pub fn root(self) -> AttachedPathRoot {
        match self.projection.root() {
            PendingPathRoot::ImplicitCrate => AttachedPathRoot::ImplicitCrate,
            PendingPathRoot::Crate(source) => AttachedPathRoot::Crate {
                source: self.owner.syntax().source_span_for_range(*source),
            },
            PendingPathRoot::SelfModule(source) => AttachedPathRoot::SelfModule {
                source: self.owner.syntax().source_span_for_range(*source),
            },
            PendingPathRoot::Super(levels) => AttachedPathRoot::Super {
                levels: levels
                    .iter()
                    .map(|source| self.owner.syntax().source_span_for_range(*source))
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            },
        }
    }

    /// Ordered parser-validated path segments.
    pub fn segments(self) -> impl ExactSizeIterator<Item = AttachedCandidatePathSegment<'a>> + 'a {
        self.projection
            .segments()
            .iter()
            .map(move |segment| AttachedCandidatePathSegment {
                owner: self.owner,
                kind: match segment.kind() {
                    PendingPathSegmentKind::Identifier => AttachedPathSegmentKind::Identifier,
                    PendingPathSegmentKind::Keyword => AttachedPathSegmentKind::Keyword,
                    PendingPathSegmentKind::ProjectSymbol => AttachedPathSegmentKind::ProjectSymbol,
                    PendingPathSegmentKind::Lifetime => AttachedPathSegmentKind::Lifetime,
                },
                source: segment.source(),
            })
    }

    /// Source-owned missing terminal name, when path recovery inserted one.
    pub fn missing_name(self) -> Option<AttachedCandidateNode<'a>> {
        self.graph
            .children(self.index)?
            .iter()
            .copied()
            .map(|index| AttachedCandidateNode::new(self.owner, self.graph, index))
            .find(|node| node.kind() == SyntaxKind::MissingName)
    }

    /// Whether this path retains typed missing-name recovery.
    pub fn has_recovery(self) -> bool {
        self.missing_name().is_some()
    }
}

/// One revision-bound segment in a retained candidate path.
#[derive(Clone, Copy)]
pub struct AttachedCandidatePathSegment<'a> {
    owner: &'a ExprNode,
    kind: AttachedPathSegmentKind,
    source: SourceRange,
}

impl<'a> AttachedCandidatePathSegment<'a> {
    pub const fn kind(self) -> AttachedPathSegmentKind {
        self.kind
    }

    pub fn source_span(self) -> SourceSpan {
        self.owner.syntax().source_span_for_range(self.source)
    }

    /// Exact parser-validated token spelling in the accepted outer revision.
    /// # Panics
    ///
    /// Panics if a validated path segment lies outside its accepted expression.
    pub fn source_text(self) -> &'a str {
        let owner = self.owner.range();
        self.owner
            .source_text()
            .get(self.source.start() - owner.start()..self.source.end() - owner.start())
            .expect("candidate path segments stay within their accepted outer expression")
    }
}

/// Typed semantic projection retained by one attached expression identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedExpressionNode {
    syntax: ExprNode,
    components: Box<[AttachedExpressionComponent]>,
    path: Option<AttachedPath>,
    nominal_path_type: Option<AttachedTypeRefNode>,
    pattern: Option<AttachedPatternNode>,
    block: Option<AstNode<BlockKind>>,
    thread: Option<AstNode<ThreadExpressionKind>>,
    choice: Option<Box<AttachedChoiceExpression>>,
    children: Box<[AttachedExpressionChild]>,
    match_arms: Box<[AttachedMatchArm]>,
    call_type_children: Box<[AttachedCallTypeChild]>,
    closure_parameters: Box<[AttachedClosureParameter]>,
    closure_result_type: Option<AttachedTypeRefNode>,
    await_branches: Option<AttachedAwaitBranchBody>,
}

/// Present `await` branch body or its exact missing-body insertion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedAwaitBranchBody {
    Present(AttachedAwaitBranchBlock),
    Missing(AstNode<MissingBodyKind>),
}

impl AttachedAwaitBranchBody {
    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Present(body) => body.has_recovery(),
            Self::Missing(_) => true,
        }
    }
}

/// Source-ordered `await` branch container with no ordinary value tail.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedAwaitBranchBlock {
    syntax: AstNode<BlockKind>,
    open: AstNode<OpenBraceKind>,
    branches: Box<[AttachedAwaitBranch]>,
    close: AstNode<CloseBraceKind>,
}

impl AttachedAwaitBranchBlock {
    pub const fn syntax(&self) -> &AstNode<BlockKind> {
        &self.syntax
    }

    pub const fn open(&self) -> &AstNode<OpenBraceKind> {
        &self.open
    }

    pub fn branches(&self) -> &[AttachedAwaitBranch] {
        &self.branches
    }

    pub const fn close(&self) -> &AstNode<CloseBraceKind> {
        &self.close
    }

    pub fn close_state(&self) -> AttachedDelimiterState {
        self.close.delimiter_state()
    }

    pub fn is_unclosed(&self) -> bool {
        !self.open.range().is_empty()
            && matches!(self.close_state(), AttachedDelimiterState::Missing(_))
    }

    pub fn has_recovery(&self) -> bool {
        self.is_unclosed() || self.branches.iter().any(AttachedAwaitBranch::has_recovery)
    }
}

/// One typed wait-view branch and its nested Thread/Flow body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedAwaitBranch {
    syntax: AstNode<AwaitWithBranchKind>,
    kind: Option<SyntaxAwaitBranchKind>,
    pattern: Option<AttachedPatternNode>,
    recovery: Option<AstNode<ErrorNodeKind>>,
    body: AttachedRequiredNestedThreadFlowBody,
}

impl AttachedAwaitBranch {
    pub const fn syntax(&self) -> &AstNode<AwaitWithBranchKind> {
        &self.syntax
    }

    pub const fn kind(&self) -> Option<SyntaxAwaitBranchKind> {
        self.kind
    }

    pub const fn pattern(&self) -> Option<&AttachedPatternNode> {
        self.pattern.as_ref()
    }

    pub const fn recovery(&self) -> Option<&AstNode<ErrorNodeKind>> {
        self.recovery.as_ref()
    }

    pub const fn body(&self) -> &AttachedRequiredNestedThreadFlowBody {
        &self.body
    }

    pub fn has_recovery(&self) -> bool {
        self.kind.is_none()
            || self.pattern.as_ref().is_some_and(|pattern| {
                matches!(pattern.value().state(), PatternSyntaxState::Recovered(_))
            })
            || self.recovery.is_some()
            || self.body.has_recovery()
    }
}

/// One exact type child retained by an attached Call projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedCallTypeChild {
    role: SyntaxCallTypeChildRole,
    node: AttachedTypeRefNode,
}

impl AttachedCallTypeChild {
    /// Grammar relation between the Call and this attached type identity.
    pub const fn role(&self) -> SyntaxCallTypeChildRole {
        self.role
    }

    /// Snapshot-bound semantic type node selected by the parser transaction.
    pub const fn node(&self) -> &AttachedTypeRefNode {
        &self.node
    }
}

/// One ordered closure parameter with attached Pattern and optional Type
/// identities from the same source snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedClosureParameter {
    pattern: AttachedPatternNode,
    ty: Option<AttachedTypeRefNode>,
}

impl AttachedClosureParameter {
    pub const fn pattern(&self) -> &AttachedPatternNode {
        &self.pattern
    }

    pub const fn ty(&self) -> Option<&AttachedTypeRefNode> {
        self.ty.as_ref()
    }
}

/// One exact ordered structural child of a composite expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedExpressionChild {
    /// One authored expression identity, including a typed Error family.
    Authored {
        ordinal: u32,
        component_role: ExpressionComponentRole,
        expression: ExprNode,
        source: SourceSpan,
    },
    /// One exact zero-width `MissingExpression` insertion.
    Missing {
        ordinal: u32,
        component_role: ExpressionComponentRole,
        recovery: RecoveryNode,
    },
}

impl AttachedExpressionChild {
    /// Zero-based grammar ordinal retained independently of vector position.
    pub const fn ordinal(&self) -> u32 {
        match self {
            Self::Authored { ordinal, .. } | Self::Missing { ordinal, .. } => *ordinal,
        }
    }

    /// Exact source-component relation declared by the expression projection.
    pub const fn component_role(&self) -> ExpressionComponentRole {
        match self {
            Self::Authored { component_role, .. } | Self::Missing { component_role, .. } => {
                *component_role
            }
        }
    }

    /// Exact revision-bound source span or insertion owned by this slot.
    pub fn source_span(&self) -> SourceSpan {
        match self {
            Self::Authored { source, .. } => source.clone(),
            Self::Missing { recovery, .. } => recovery.source_span(),
        }
    }

    /// Authored child identity, absent only for a missing recovery slot.
    pub const fn authored(&self) -> Option<&ExprNode> {
        match self {
            Self::Authored { expression, .. } => Some(expression),
            Self::Missing { .. } => None,
        }
    }

    /// Missing-expression recovery identity, absent for authored slots.
    pub const fn missing(&self) -> Option<&RecoveryNode> {
        match self {
            Self::Missing { recovery, .. } => Some(recovery),
            Self::Authored { .. } => None,
        }
    }

    /// Returns the authored semantic expression selected through transparent
    /// parenthesized groups, or `None` for an omitted recovery slot.
    ///
    /// Grouping remains part of the parent's exact source component, but it is
    /// not a second semantic HIR expression. Attachment navigation has already
    /// selected the inner identity-bearing expression without source lookup.
    pub fn authored_semantic(&self) -> Result<Option<AttachedExpressionNode>, SyntaxAccessError> {
        match self {
            Self::Authored { expression, .. } => {
                AttachedExpressionNode::from_syntax(expression.syntax()).map(Some)
            }
            Self::Missing { .. } => Ok(None),
        }
    }
}

impl AttachedExpressionNode {
    pub(crate) fn from_syntax(syntax: SyntaxNodeHandle) -> Result<Self, SyntaxAccessError> {
        let pending = syntax
            .expression_projection()
            .ok_or(SyntaxAccessError::MissingExpressionProjection { id: syntax.id() })?;
        if !pending.accepts_kind(syntax.kind()) || !pending.validates_components(syntax.range()) {
            return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
        }

        let children =
            attached_composite_children(&syntax, pending.projection(), pending.components())?;
        let match_arms = attached_match_arms(&syntax, pending.projection(), pending.components())?;
        let call_type_children =
            attached_call_type_children(&syntax, pending.projection(), pending.components())?;
        let (closure_parameters, closure_result_type) =
            attached_closure_children(&syntax, pending.projection(), pending.components())?;
        validate_short_variant_shape(&syntax, pending.projection(), pending.components())?;
        let (path, nominal_path_type) = match pending.projection() {
            ExpressionProjection::Path => attached_path_projection(&syntax)?,
            ExpressionProjection::Record(_) => (Some(attached_path(&syntax)?), None),
            _ => (None, None),
        };
        let pattern = match pending.projection() {
            ExpressionProjection::IfLet { .. } => Some(attached_pattern(&syntax)?),
            _ => None,
        };
        let block = match pending.projection() {
            ExpressionProjection::Block
            | ExpressionProjection::ComputationBlock(_)
            | ExpressionProjection::NamedBlock(_)
            | ExpressionProjection::Loop => Some(attached_block(&syntax)?),
            _ => None,
        };
        let thread = match pending.projection() {
            ExpressionProjection::Thread(_) => Some(syntax.clone().cast::<ThreadExpressionKind>()?),
            _ => None,
        };
        let choice = match pending.projection() {
            ExpressionProjection::Choice => Some(Box::new(
                syntax.clone().cast::<ChoiceExpressionKind>()?.semantics()?,
            )),
            _ => None,
        };
        let await_branches = attached_await_branches(&syntax, pending.projection())?;
        let mut components = pending
            .components()
            .iter()
            .map(|component| AttachedExpressionComponent {
                role: component.role(),
                source: syntax.source_span_for_range(component.range()),
            })
            .collect::<Vec<_>>();
        if let Some(block) = &block {
            for (ordinal, statement) in block.statements()?.into_iter().enumerate() {
                components.push(AttachedExpressionComponent {
                    role: ExpressionComponentRole::Statement {
                        ordinal: u32::try_from(ordinal).map_err(|_| {
                            SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() }
                        })?,
                    },
                    source: statement.source_span(),
                });
            }
            components.push(AttachedExpressionComponent {
                role: ExpressionComponentRole::Tail,
                source: block.tail()?.source_span(),
            });
        }
        let components = components.into_boxed_slice();
        if pattern.as_ref().is_some_and(|pattern| {
            pending
                .components()
                .iter()
                .find(|component| component.role() == ExpressionComponentRole::Pattern)
                .is_none_or(|component| component.range() != pattern.whole_source_span().range())
        }) {
            return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
        }

        Ok(Self {
            syntax: FamilyNode::<ExpressionFamily>::new(syntax)?,
            components,
            path,
            nominal_path_type,
            pattern,
            block,
            thread,
            choice,
            children,
            match_arms,
            call_type_children,
            closure_parameters,
            closure_result_type,
            await_branches,
        })
    }

    pub fn id(&self) -> SyntaxNodeId {
        self.syntax.id()
    }

    pub fn snapshot_id(&self) -> &SyntaxSnapshotId {
        self.syntax.snapshot_id()
    }

    pub const fn syntax(&self) -> &ExprNode {
        &self.syntax
    }

    /// Exact revision-bound source span occupied by this expression leaf.
    pub fn whole_source_span(&self) -> SourceSpan {
        self.syntax.source_span()
    }

    /// Parser-selected leaf semantic payload.
    /// # Panics
    ///
    /// Panics if an attached expression has lost its parser-validated projection.
    pub fn projection(&self) -> &ExpressionProjection {
        self.syntax
            .syntax_handle()
            .expression_projection()
            .expect("attached expression families retain their validated projection")
            .projection()
    }

    /// Typed Dialogue line plan owned by this application, when present.
    pub fn dialogue_line_plan(
        &self,
    ) -> Result<Option<super::AttachedDialogueLinePlan>, SyntaxAccessError> {
        super::dialogue_plan::attached_dialogue_line_plan(self)
    }

    /// Retained ordinary-index candidate for an ambiguous postfix bracket.
    ///
    /// Selected and invalid postfix brackets have no candidate graph and
    /// therefore return `None`.
    pub fn ambiguous_index_candidate(&self) -> Option<AttachedCandidateGraph<'_>> {
        let ExpressionProjection::PostfixBracket(
            crate::expressions::SyntaxPostfixBracketProjection::Ambiguous { index, .. },
        ) = self.projection()
        else {
            return None;
        };
        Some(AttachedCandidateGraph::new(
            &self.syntax,
            index.graph(),
            Some(index.index()),
            None,
        ))
    }

    /// Retained dialogue-content candidate for an ambiguous postfix bracket.
    ///
    /// The shared target remains owned by this outer expression and is not
    /// duplicated in the returned graph.
    pub fn ambiguous_dialogue_candidate(&self) -> Option<AttachedCandidateGraph<'_>> {
        let ExpressionProjection::PostfixBracket(
            crate::expressions::SyntaxPostfixBracketProjection::Ambiguous { dialogue, .. },
        ) = self.projection()
        else {
            return None;
        };
        Some(AttachedCandidateGraph::new(
            &self.syntax,
            dialogue.graph(),
            None,
            Some(dialogue),
        ))
    }

    /// Existing attached path owner selected by a `Path` expression marker.
    pub const fn path(&self) -> Option<&AttachedPath> {
        self.path.as_ref()
    }

    /// Attached nominal type path retained for value-first/type-second lookup.
    ///
    /// This is present only for a path expression produced as the dot receiver
    /// of an unresolved Call. It is the parser's original type projection, not
    /// a path reconstructed from source text.
    pub const fn nominal_path_type(&self) -> Option<&AttachedTypeRefNode> {
        self.nominal_path_type.as_ref()
    }

    /// Exact attached binding pattern selected by an `if let` projection.
    pub const fn pattern(&self) -> Option<&AttachedPatternNode> {
        self.pattern.as_ref()
    }

    /// Exact attached value block selected by a Block expression projection.
    pub const fn block(&self) -> Option<&AstNode<BlockKind>> {
        self.block.as_ref()
    }

    /// Exact attached Thread owner selected by a Thread expression projection.
    pub const fn thread(&self) -> Option<&AstNode<ThreadExpressionKind>> {
        self.thread.as_ref()
    }

    /// Exact typed Choice relation selected by the shared expression owner.
    pub fn choice(&self) -> Option<&AttachedChoiceExpression> {
        self.choice.as_deref()
    }

    pub fn component(&self, role: ExpressionComponentRole) -> Option<SourceSpan> {
        self.components
            .iter()
            .find(|component| component.role == role)
            .map(|component| component.source.clone())
    }

    pub fn components(&self) -> &[AttachedExpressionComponent] {
        &self.components
    }

    /// Exact ordered structural children of a composite expression.
    pub fn children(&self) -> &[AttachedExpressionChild] {
        &self.children
    }

    /// Exact source-ordered arms owned by a Match expression.
    pub fn match_arms(&self) -> &[AttachedMatchArm] {
        &self.match_arms
    }

    /// Exact type children owned by a parenthesized Call projection.
    pub fn call_type_children(&self) -> &[AttachedCallTypeChild] {
        &self.call_type_children
    }

    /// Ordered closure parameter identities, empty for non-Closure families.
    pub fn closure_parameters(&self) -> &[AttachedClosureParameter] {
        &self.closure_parameters
    }

    /// Optional result type attached to one Closure expression.
    pub const fn closure_result_type(&self) -> Option<&AttachedTypeRefNode> {
        self.closure_result_type.as_ref()
    }

    /// Exact source-ordered `with` branches owned by this Await expression.
    pub const fn await_branches(&self) -> Option<&AttachedAwaitBranchBody> {
        self.await_branches.as_ref()
    }
}

fn attached_await_branches(
    syntax: &SyntaxNodeHandle,
    projection: &ExpressionProjection,
) -> Result<Option<AttachedAwaitBranchBody>, SyntaxAccessError> {
    let ExpressionProjection::Await {
        branches: Some(branches),
        ..
    } = projection
    else {
        return Ok(None);
    };
    let owner = syntax.clone().cast::<AwaitExpressionKind>()?;
    attach_await_branch_body(&owner, branches).map(Some)
}

fn attach_await_branch_body(
    owner: &AstNode<AwaitExpressionKind>,
    projections: &[Option<SyntaxAwaitBranchKind>],
) -> Result<AttachedAwaitBranchBody, SyntaxAccessError> {
    let body = owner
        .syntax()
        .optional_unique_child(SyntaxRole::Body)?
        .ok_or_else(|| SyntaxAccessError::InvalidExpressionProjection { id: owner.id() })?;
    match body.kind() {
        SyntaxKind::Block => Ok(AttachedAwaitBranchBody::Present(attach_await_branch_block(
            owner,
            projections,
        )?)),
        SyntaxKind::MissingBody if projections.is_empty() && body.range().is_empty() => {
            Ok(AttachedAwaitBranchBody::Missing(body.cast()?))
        }
        _ => Err(SyntaxAccessError::InvalidExpressionProjection { id: owner.id() }),
    }
}

fn attach_await_branch_block(
    owner: &AstNode<AwaitExpressionKind>,
    projections: &[Option<SyntaxAwaitBranchKind>],
) -> Result<AttachedAwaitBranchBlock, SyntaxAccessError> {
    let syntax = owner.required_exact_child::<BlockKind>(SyntaxRole::Body)?;
    let open = syntax.required_exact_child::<OpenBraceKind>(SyntaxRole::OpenDelimiter)?;
    let close = syntax.required_exact_child::<CloseBraceKind>(SyntaxRole::CloseDelimiter)?;
    let branches = syntax.ordered_exact_children::<AwaitWithBranchKind>(SyntaxRoleClass::Branch)?;
    if branches.len() != projections.len()
        || syntax.syntax().children().iter().any(|child| {
            !matches!(
                child.role(),
                SyntaxRole::OpenDelimiter | SyntaxRole::CloseDelimiter | SyntaxRole::Branch(_)
            )
        })
    {
        return Err(SyntaxAccessError::InvalidExpressionProjection { id: owner.id() });
    }
    let branches = branches
        .into_iter()
        .zip(projections.iter().copied())
        .map(|(syntax, kind)| {
            let body = required_nested_thread_flow_body(&syntax)?;
            if kind.is_some() {
                let pattern = syntax
                    .required_family_child::<PatternFamily>(SyntaxRole::Pattern)?
                    .semantic()?;
                if syntax
                    .syntax()
                    .optional_unique_child(SyntaxRole::Recovery(0))?
                    .is_some()
                {
                    return Err(SyntaxAccessError::InvalidExpressionProjection { id: owner.id() });
                }
                Ok(AttachedAwaitBranch {
                    pattern: Some(pattern),
                    syntax,
                    kind,
                    recovery: None,
                    body,
                })
            } else {
                let recovery =
                    syntax.required_exact_child::<ErrorNodeKind>(SyntaxRole::Recovery(0))?;
                Ok(AttachedAwaitBranch {
                    recovery: Some(recovery),
                    syntax,
                    kind: None,
                    pattern: None,
                    body,
                })
            }
        })
        .collect::<Result<Vec<_>, SyntaxAccessError>>()?
        .into_boxed_slice();
    Ok(AttachedAwaitBranchBlock {
        syntax,
        open,
        branches,
        close,
    })
}

impl FamilyNode<ExpressionFamily> {
    /// Returns the parser-selected semantic projection owned by this leaf.
    pub fn semantic(&self) -> Result<AttachedExpressionNode, SyntaxAccessError> {
        AttachedExpressionNode::from_syntax(self.syntax())
    }
}

impl AstNode<ExpressionFragmentRootKind> {
    /// Returns the semantic projection of an attached standalone expression leaf.
    pub fn semantic(&self) -> Result<AttachedExpressionNode, SyntaxAccessError> {
        AttachedExpressionNode::from_syntax(self.syntax())
    }
}

#[cfg(test)]
mod tests;
