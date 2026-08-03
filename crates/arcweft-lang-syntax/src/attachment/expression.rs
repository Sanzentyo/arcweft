//! Parser-owned semantic expression projections bound to attached identities.

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
use super::node::{AstNode, BlockKind, ChoiceExpressionKind, ExpressionFragmentRootKind, PathKind};
use super::source_file::{AttachedPath, AttachedPathRoot, AttachedPathSegmentKind};
use super::{
    AttachedPatternNode, AttachedTypeRefNode, SyntaxAccessError, SyntaxNodeHandle, SyntaxNodeId,
    SyntaxSnapshotId,
};
use crate::assertion::AssertionMode;
use crate::expressions::{
    CandidateNodeIndex, ExpressionComponentRole, ExpressionProjection, ExpressionRecordFieldPart,
    PendingCandidateGraph, PendingCandidateSemantic, SyntaxCallArgumentPart,
    SyntaxCallCalleeProjection, SyntaxCallProjection, SyntaxCallTypeApplicationComponentRole,
    SyntaxCallTypeArgumentPart, SyntaxCallTypeArgumentProjection, SyntaxCallTypeChildRole,
    SyntaxClosureParameterPart, SyntaxExpressionSlot, SyntaxRecordField,
};
use crate::grammar::keyword_statement_projection::PendingKeywordStatementProjection;
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};
use crate::grammar::source_projection::{
    PendingPathProjection, PendingPathRoot, PendingPathSegmentKind,
};
use crate::name::SyntaxNameIssue;
use crate::patterns::{PatternNodePath, PatternSyntaxNode};
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
        self.dialogue.map(|candidate| candidate.content())
    }

    /// Nested expression slots owned by this retained Dialogue candidate.
    ///
    /// The typed content inventory supplies owner identity and slot state;
    /// candidate graph preorder supplies the unique attached expression node.
    /// Structural Dialogue wrappers never appear in the result.
    pub fn dialogue_expression_slots(
        self,
    ) -> Option<impl ExactSizeIterator<Item = AttachedCandidateDialogueExpression<'a>> + 'a> {
        let candidate = self.dialogue?;
        let crate::expressions::SyntaxDialogueContentProjection::Present(content) =
            candidate.content()
        else {
            return Some(Vec::new().into_iter());
        };
        let component_source = |role| {
            candidate
                .components()
                .iter()
                .find(|component| component.role() == role)
                .map(|component| component.range())
                .expect("Dialogue candidate expression slots retain source components")
        };
        let mut expected = Vec::new();
        for (ordinal, node) in content.nodes().iter().enumerate() {
            let crate::expressions::SyntaxDialogueNodeProjection::Interpolation(slot) = node else {
                continue;
            };
            let ordinal =
                u32::try_from(ordinal).expect("Dialogue node counts are bounded by syntax limits");
            let component_role = ExpressionComponentRole::DialogueNode {
                ordinal,
                part: crate::expressions::SyntaxDialogueNodeSourcePart::Interpolation,
            };
            expected.push(CandidateDialogueExpressionSpec {
                owner: AttachedCandidateDialogueOwner::Node { ordinal },
                slot: *slot,
                source: component_source(component_role),
            });
        }
        for (ordinal, tag) in content.tags().iter().enumerate() {
            let slot = match tag.payload() {
                crate::expressions::SyntaxRichTextTagPayloadProjection::FxCall(slot) => *slot,
                crate::expressions::SyntaxRichTextTagPayloadProjection::DialogueCall(slot) => *slot,
                crate::expressions::SyntaxRichTextTagPayloadProjection::Condition(slot) => *slot,
                crate::expressions::SyntaxRichTextTagPayloadProjection::Arguments
                | crate::expressions::SyntaxRichTextTagPayloadProjection::None => continue,
            };
            let ordinal =
                u32::try_from(ordinal).expect("Rich-text tag counts are bounded by syntax limits");
            let component_role = ExpressionComponentRole::RichTextTag {
                tag: ordinal,
                part: crate::expressions::SyntaxRichTextTagSourcePart::Payload,
            };
            expected.push(CandidateDialogueExpressionSpec {
                owner: AttachedCandidateDialogueOwner::Tag { ordinal },
                slot,
                source: component_source(component_role),
            });
        }
        expected.sort_by_key(|spec| (spec.source.start(), spec.source.end()));

        let bindings = self
            .graph
            .nodes()
            .iter()
            .enumerate()
            .filter_map(|(position, node)| {
                if !matches!(node.semantic(), PendingCandidateSemantic::Expression(_)) {
                    return None;
                }
                let index = CandidateNodeIndex::try_new(position)
                    .expect("candidate graph node counts are bounded by their local index");
                let attached = AttachedCandidateNode::new(self.owner, self.graph, index);
                if attached.nearest_expression_parent(index).is_some() {
                    return None;
                }
                let owner = attached.nearest_dialogue_owner(index)?;
                let spec = expected.iter().find(|spec| spec.owner == owner)?;
                assert!(
                    source_contains(spec.source, node.source()),
                    "Dialogue candidate owner payload must contain its expression node"
                );
                assert!(
                    matches!(
                        (spec.slot, node.kind()),
                        (SyntaxExpressionSlot::Missing, SyntaxKind::MissingExpression)
                    ) || spec.slot == SyntaxExpressionSlot::Authored
                        && node.kind() != SyntaxKind::MissingExpression,
                    "Dialogue candidate slot state must match its expression node"
                );
                Some(AttachedCandidateDialogueExpression {
                    owner: spec.owner,
                    slot: spec.slot,
                    source: attached.source_span(),
                    node: attached,
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(
            bindings.len(),
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
    /// One RichText tag identified by its typed content tag ordinal.
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
    source: SourceRange,
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
    pub fn semantic_expression_children(
        self,
    ) -> impl ExactSizeIterator<Item = AttachedCandidateExpressionChild<'a>> + 'a {
        let Some(projection) = self.expression_projection() else {
            return Vec::new().into_iter();
        };
        let PendingCandidateSemantic::Expression(pending) = self.pending().semantic() else {
            unreachable!("expression projection is backed by expression semantics");
        };
        let mut nodes = self.direct_semantic_expression_nodes();
        let mut specs = candidate_semantic_child_specs(projection, pending.components());
        if matches!(projection, ExpressionProjection::Error) && specs.is_empty() {
            specs.extend(nodes.first().map(|node| {
                CandidateSemanticChildSpec {
                    ordinal: 0,
                    slot: SyntaxExpressionSlot::Authored,
                    component_role: ExpressionComponentRole::Recovery,
                    source: pending
                        .components()
                        .iter()
                        .find(|component| component.role() == ExpressionComponentRole::Recovery)
                        .map_or_else(|| node.pending().source(), |component| component.range()),
                }
            }));
        }
        let children = specs
            .into_iter()
            .map(|spec| {
                let position = nodes
                    .iter()
                    .position(|node| source_contains(spec.source, node.pending().source()))
                    .expect("candidate semantic slots retain one expression node");
                let node = nodes.remove(position);
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
        assert!(
            nodes.is_empty(),
            "candidate expression projection must declare every semantic child"
        );
        children.into_iter()
    }

    fn direct_semantic_expression_nodes(self) -> Vec<Self> {
        self.graph
            .nodes()
            .iter()
            .enumerate()
            .filter_map(|(position, node)| {
                let candidate = CandidateNodeIndex::try_new(position)
                    .expect("candidate graph node counts are bounded by their local index");
                if candidate == self.index || !ExpressionFamily::accepts(node.kind()) {
                    return None;
                }
                let mut parent = node.parent();
                while let Some(index) = parent {
                    let ancestor = self
                        .graph
                        .node(index)
                        .expect("candidate parent edges stay inside their graph");
                    if matches!(
                        self.expression_projection(),
                        Some(ExpressionProjection::Match(_))
                    ) && ancestor.kind() == SyntaxKind::MatchArm
                    {
                        // MatchArm is a typed semantic scope boundary. Its
                        // guard and value are read through `match_view()` and
                        // must not be flattened into the outer Match scope.
                        return None;
                    }
                    if matches!(ancestor.semantic(), PendingCandidateSemantic::Expression(_)) {
                        return (index == self.index)
                            .then(|| Self::new(self.owner, self.graph, candidate));
                    }
                    parent = ancestor.parent();
                }
                None
            })
            .collect()
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
    pub fn direct_semantic_type_roots(
        self,
    ) -> impl ExactSizeIterator<Item = AttachedCandidateTypeRoot<'a>> + 'a {
        let Some(ExpressionProjection::Call(SyntaxCallProjection::Parenthesized(call))) =
            self.expression_projection()
        else {
            return Vec::new().into_iter();
        };
        let PendingCandidateSemantic::Expression(pending) = self.pending().semantic() else {
            unreachable!("Call projection is backed by expression semantics");
        };
        let component = |role| {
            pending
                .components()
                .iter()
                .find(|component| component.role() == role)
                .map(|component| component.range())
                .expect("candidate Call type roots retain source components")
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
                expected.push((
                    SyntaxCallTypeChildRole::DotNominalReceiver,
                    component(ExpressionComponentRole::CallAssociatedReceiver),
                    callee.index,
                    Some(receiver.node().index),
                ));
            }
            SyntaxCallCalleeProjection::Associated { .. } => expected.push((
                SyntaxCallTypeChildRole::AssociatedReceiver,
                component(ExpressionComponentRole::CallAssociatedReceiver),
                self.index,
                None,
            )),
        }
        if let Some(application) = call.explicit_type_application() {
            for (argument, projection) in application.arguments().iter().enumerate() {
                if matches!(projection, SyntaxCallTypeArgumentProjection::Missing) {
                    continue;
                }
                let argument = u16::try_from(argument)
                    .expect("candidate Call type arguments are bounded by syntax limits");
                expected.push((
                    SyntaxCallTypeChildRole::ExplicitCallTypeArgument { ordinal: argument },
                    component(ExpressionComponentRole::CallTypeApplication(
                        SyntaxCallTypeApplicationComponentRole::Argument {
                            argument,
                            part: SyntaxCallTypeArgumentPart::Type,
                        },
                    )),
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
                let (role, _, _, _) = expected.iter().find(|(_, source, owner, exact)| {
                    *owner == semantic_parent
                        && exact.is_none_or(|expected| expected == index)
                        && source_contains(*source, node.source())
                })?;
                let node = Self::new(self.owner, self.graph, index);
                Some(AttachedCandidateTypeRoot {
                    role: *role,
                    source: node.source_span(),
                    node,
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(
            roots.len(),
            expected.len(),
            "candidate Call must retain every declared direct type root"
        );
        roots.into_iter()
    }

    /// Direct semantic type children of this candidate type node.
    ///
    /// Child paths must extend the parent path by exactly one typed step and
    /// the nearest expression-or-type semantic ancestor must be this node.
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

    fn nearest_expression_parent(
        self,
        candidate: CandidateNodeIndex,
    ) -> Option<CandidateNodeIndex> {
        let mut parent = self.graph.node(candidate)?.parent();
        while let Some(index) = parent {
            let ancestor = self.graph.node(index)?;
            if matches!(ancestor.semantic(), PendingCandidateSemantic::Expression(_)) {
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
    source: SourceRange,
}

fn candidate_semantic_child_specs(
    projection: &ExpressionProjection,
    components: &[crate::expressions::PendingExpressionComponent],
) -> Vec<CandidateSemanticChildSpec> {
    let source = |role| {
        components
            .iter()
            .find(|component| component.role() == role)
            .map(|component| component.range())
            .expect("candidate expression slots retain exact source components")
    };
    let spec = |ordinal, slot, component_role| CandidateSemanticChildSpec {
        ordinal,
        slot,
        component_role,
        source: source(component_role),
    };
    match projection {
        ExpressionProjection::Tuple(slots) | ExpressionProjection::BracketSequence(slots) => slots
            .iter()
            .copied()
            .enumerate()
            .map(|(ordinal, slot)| {
                let ordinal = u32::try_from(ordinal)
                    .expect("candidate expression child counts are bounded by syntax limits");
                spec(ordinal, slot, ExpressionComponentRole::Element { ordinal })
            })
            .collect(),
        ExpressionProjection::ArrayRepeat([value, length]) => vec![
            spec(0, *value, ExpressionComponentRole::RepeatValue),
            spec(1, *length, ExpressionComponentRole::RepeatLength),
        ],
        ExpressionProjection::Call(SyntaxCallProjection::CallbackBlock(callback)) => vec![
            spec(
                0,
                SyntaxExpressionSlot::Authored,
                ExpressionComponentRole::CallCallee,
            ),
            spec(
                1,
                callback.callback(),
                ExpressionComponentRole::CallArgument {
                    argument: 0,
                    part: SyntaxCallArgumentPart::Value,
                },
            ),
        ],
        ExpressionProjection::Call(SyntaxCallProjection::Parenthesized(call)) => {
            let mut children = Vec::with_capacity(call.arguments().len() + 1);
            match call.callee() {
                SyntaxCallCalleeProjection::Ordinary => children.push(spec(
                    0,
                    SyntaxExpressionSlot::Authored,
                    ExpressionComponentRole::CallCallee,
                )),
                SyntaxCallCalleeProjection::UnresolvedDot { .. } => children.push(spec(
                    0,
                    SyntaxExpressionSlot::Authored,
                    ExpressionComponentRole::CallAssociatedReceiver,
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
                        spec(
                            u32::from(argument) + 1,
                            value.value(),
                            ExpressionComponentRole::CallArgument {
                                argument,
                                part: SyntaxCallArgumentPart::Value,
                            },
                        )
                    }),
            );
            children
        }
        ExpressionProjection::Select(_) => vec![spec(
            0,
            SyntaxExpressionSlot::Authored,
            ExpressionComponentRole::Target,
        )],
        ExpressionProjection::Index(index) => vec![
            spec(0, index.target(), ExpressionComponentRole::Target),
            spec(1, index.index(), ExpressionComponentRole::Index),
        ],
        ExpressionProjection::DialogueContentApplication(application) => {
            let mut children = vec![spec(
                0,
                SyntaxExpressionSlot::Authored,
                ExpressionComponentRole::Target,
            )];
            let crate::expressions::SyntaxDialogueContentProjection::Present(content) =
                application.content()
            else {
                return children;
            };
            let mut nested = Vec::new();
            for (ordinal, node) in content.nodes().iter().enumerate() {
                let crate::expressions::SyntaxDialogueNodeProjection::Interpolation(slot) = node
                else {
                    continue;
                };
                let ordinal = u32::try_from(ordinal)
                    .expect("dialogue node counts are bounded by syntax limits");
                let role = ExpressionComponentRole::DialogueNode {
                    ordinal,
                    part: crate::expressions::SyntaxDialogueNodeSourcePart::Interpolation,
                };
                nested.push((*slot, role, source(role)));
            }
            for (tag, projection) in content.tags().iter().enumerate() {
                let slot = match projection.payload() {
                    crate::expressions::SyntaxRichTextTagPayloadProjection::FxCall(slot)
                    | crate::expressions::SyntaxRichTextTagPayloadProjection::DialogueCall(slot)
                    | crate::expressions::SyntaxRichTextTagPayloadProjection::Condition(slot) => {
                        *slot
                    }
                    crate::expressions::SyntaxRichTextTagPayloadProjection::Arguments
                    | crate::expressions::SyntaxRichTextTagPayloadProjection::None => continue,
                };
                let tag =
                    u32::try_from(tag).expect("rich-text tag counts are bounded by syntax limits");
                let role = ExpressionComponentRole::RichTextTag {
                    tag,
                    part: crate::expressions::SyntaxRichTextTagSourcePart::Payload,
                };
                nested.push((slot, role, source(role)));
            }
            nested.sort_by_key(|(_, _, range)| (range.start(), range.end()));
            children.extend(nested.into_iter().enumerate().map(
                |(position, (slot, component_role, source))| {
                    CandidateSemanticChildSpec {
                        ordinal: u32::try_from(position)
                            .expect("dialogue expression counts are bounded by syntax limits")
                            + 1,
                        slot,
                        component_role,
                        source,
                    }
                },
            ));
            children
        }
        ExpressionProjection::PostfixBracket(_) => vec![spec(
            0,
            SyntaxExpressionSlot::Authored,
            ExpressionComponentRole::Target,
        )],
        ExpressionProjection::Pipe([left, right]) => vec![
            spec(0, *left, ExpressionComponentRole::LeftOperand),
            spec(1, *right, ExpressionComponentRole::RightOperand),
        ],
        ExpressionProjection::Try { operand, .. }
        | ExpressionProjection::Await { operand, .. }
        | ExpressionProjection::Borrow { operand, .. }
        | ExpressionProjection::Dereference { operand }
        | ExpressionProjection::Unary { operand, .. } => {
            vec![spec(0, *operand, ExpressionComponentRole::Operand)]
        }
        ExpressionProjection::Range { start, end, .. } => {
            let mut children = Vec::with_capacity(2);
            if let Some(start) = start {
                children.push(spec(0, *start, ExpressionComponentRole::RangeStart));
            }
            if let Some(end) = end {
                children.push(spec(1, *end, ExpressionComponentRole::RangeEnd));
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
                    Some(spec(
                        field,
                        slot,
                        ExpressionComponentRole::RecordField {
                            field,
                            part: ExpressionRecordFieldPart::Value,
                        },
                    ))
                })
                .collect()
        }
        ExpressionProjection::Binary { left, right, .. } => vec![
            spec(0, *left, ExpressionComponentRole::LeftOperand),
            spec(1, *right, ExpressionComponentRole::RightOperand),
        ],
        ExpressionProjection::Closure(closure) => {
            vec![spec(0, closure.body(), ExpressionComponentRole::Body)]
        }
        ExpressionProjection::If {
            condition,
            then_branch,
            else_branch,
        } => {
            let mut children = vec![
                spec(0, *condition, ExpressionComponentRole::Condition),
                spec(1, *then_branch, ExpressionComponentRole::ThenBranch),
            ];
            if let Some(else_branch) = else_branch {
                children.push(spec(2, *else_branch, ExpressionComponentRole::ElseBranch));
            }
            children
        }
        ExpressionProjection::IfLet {
            scrutinee,
            guard,
            then_branch,
            else_branch,
        } => {
            let mut children = vec![spec(0, *scrutinee, ExpressionComponentRole::Scrutinee)];
            if let Some(guard) = guard {
                children.push(spec(1, *guard, ExpressionComponentRole::Guard));
            }
            children.push(spec(2, *then_branch, ExpressionComponentRole::ThenBranch));
            if let Some(else_branch) = else_branch {
                children.push(spec(3, *else_branch, ExpressionComponentRole::ElseBranch));
            }
            children
        }
        ExpressionProjection::Match(projection) => {
            vec![spec(
                0,
                projection.scrutinee(),
                ExpressionComponentRole::Scrutinee,
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
        | ExpressionProjection::Thread(_)
        | ExpressionProjection::Choice
        | ExpressionProjection::Error => Vec::new(),
    }
}

const fn source_contains(owner: SourceRange, nested: SourceRange) -> bool {
    owner.start() <= nested.start() && nested.end() <= owner.end()
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
    choice: Option<Box<AttachedChoiceExpression>>,
    children: Box<[AttachedExpressionChild]>,
    match_arms: Box<[AttachedMatchArm]>,
    call_type_children: Box<[AttachedCallTypeChild]>,
    closure_parameters: Box<[AttachedClosureParameter]>,
    closure_result_type: Option<AttachedTypeRefNode>,
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
        expression: ExprNode,
        source: SourceSpan,
    },
    /// One exact zero-width `MissingExpression` insertion.
    Missing {
        ordinal: u32,
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
            | ExpressionProjection::NamedBlock(_) => Some(attached_block(&syntax)?),
            _ => None,
        };
        let choice = match pending.projection() {
            ExpressionProjection::Choice => Some(Box::new(
                syntax.clone().cast::<ChoiceExpressionKind>()?.semantics()?,
            )),
            _ => None,
        };
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
            choice,
            children,
            match_arms,
            call_type_children,
            closure_parameters,
            closure_result_type,
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
    pub fn projection(&self) -> &ExpressionProjection {
        self.syntax
            .syntax_handle()
            .expression_projection()
            .expect("attached expression families retain their validated projection")
            .projection()
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
