//! Exact source freeze for E34 candidate expressions and their type preorder.
//!
//! Candidate nodes deliberately have no syntax identity. Their only authority
//! is the source-backed outer postfix expression, the interpretation role, and
//! the recursive grammar traversal reproduced here. Nested E34 graphs inherit
//! that outer owner and role and continue the same expression/type counters.

mod block;
mod control;
mod pattern;
mod payload;
mod type_expectation;

#[cfg(test)]
mod tests;

use payload::{
    binary_operator_matches, candidate_node_root_site, candidate_path_matches,
    candidate_record_fields_match, candidate_resolved_path_matches, candidate_role_map, child_ids,
    child_poison, dialogue_content_matches, dialogue_coordinates_match,
    dialogue_intrinsic_recovery, dialogue_slot_role, outer_root_site, recovered_child,
};
pub(in crate::source_index) use type_expectation::{
    CandidateTypeExpectation, candidate_type_expectations,
};

use std::collections::{BTreeMap, BTreeSet};

use arcweft_lang_syntax::attachment::node::MissingExpressionKind;
use arcweft_lang_syntax::attachment::{
    AttachedCandidateDialogueOwner, AttachedCandidateExpressionChild, AttachedCandidateGraph,
    AttachedCandidateNode, AttachedCandidatePathExpression, AttachedCandidateTypeRoot,
    AttachedExpressionNode, SyntaxNodeId,
};
use arcweft_lang_syntax::expressions::{
    ExpressionComponentRole, ExpressionProjection, SyntaxAssociatedReceiver,
    SyntaxCallArgumentPart, SyntaxCallCalleeProjection, SyntaxCallProjection,
    SyntaxCallTypeArgumentProjection, SyntaxCallTypeChildRole, SyntaxDialogueApplicationForm,
    SyntaxDialogueContentProjection, SyntaxExpressionSlot, SyntaxPlaceholderKind,
    SyntaxPostfixBracketProjection, SyntaxSelectedMember,
};
use arcweft_lang_syntax::incremental::ParsedSource;

use super::ExpressionManifestRows;
use super::call::call_projection_matches;
use super::leaf::{
    lifetime_projection_matches, numeric_sequence_matches, short_variant_projection_matches,
};
use super::projection::poison_state_matches;
use crate::arena::ArenaSnapshot;
use crate::dialogue_application::HirPostfixBracketCandidates;
use crate::expr::{
    HirAssociatedReceiver, HirBorrowKind, HirCallArgument, HirCallCallee, HirCallChildPoison,
    HirCallChildStates, HirCallTypeApplication, HirCallTypeArgument, HirCallValue,
    HirComputationBlockKind, HirExpr, HirExprKind, HirExpressionRecoveryIssue, HirGenericExprIssue,
    HirNamedBlockName, HirPlaceholderKind, HirPoisonState, HirRecoveryIssue, HirSelectedMember,
    HirUnaryOp, literal_recovery_issue,
};
use crate::identity::{
    ExprId, LocalId, PatternId, ScopeId, StmtId, SyntheticKey, SyntheticOwner, SyntheticRole,
    TypeId,
};
use crate::leaf::HirPathValue;
use crate::pattern::HirPattern;
use crate::scope::{HirLocal, HirScope};
use crate::slot::{HirOrigin, SlotSnapshot};
use crate::source_index::{
    HirExprSourceRole, HirInsertionPoint, HirSourceIndex, HirSourceSite, expression_component_role,
};
use crate::stmt::HirStmt;
use crate::type_ref::HirType;

#[derive(Clone, Copy)]
struct CandidateChild {
    id: ExprId,
    missing: bool,
    poisoned: bool,
    role: HirExprSourceRole,
}

#[derive(Clone, Copy)]
struct CandidateTypeChild {
    id: TypeId,
    poisoned: bool,
}

#[derive(Default)]
struct CandidateExpectedDescendants {
    expressions: BTreeSet<ExprId>,
    statements: BTreeSet<StmtId>,
    types: BTreeSet<TypeId>,
    patterns: BTreeSet<PatternId>,
    scopes: BTreeSet<ScopeId>,
    locals: BTreeSet<LocalId>,
    scope_children: BTreeMap<ScopeId, Vec<ScopeId>>,
}

#[allow(clippy::too_many_arguments)]
#[allow(
    clippy::too_many_lines,
    reason = "one candidate expression pass validates the complete attached-to-HIR identity and source projection"
)]
pub(super) fn validate_candidate_expressions(
    index: &HirSourceIndex,
    expression_rows: &ExpressionManifestRows<'_>,
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    expressions: &ArenaSnapshot<HirExpr, ExprId>,
    statements: &ArenaSnapshot<HirStmt, StmtId>,
    types: &ArenaSnapshot<HirType, TypeId>,
    scopes: &ArenaSnapshot<HirScope, ScopeId>,
    locals: &ArenaSnapshot<HirLocal, crate::identity::LocalId>,
    patterns: &ArenaSnapshot<HirPattern, PatternId>,
    local_resolver: &crate::module::HirLocalResolver<'_>,
    retained_style_expressions: &BTreeSet<ExprId>,
) -> Option<BTreeSet<ExprId>> {
    let type_expectations = candidate_type_expectations(parsed, slots, retained_style_expressions)?;
    let mut expected = CandidateExpectedDescendants::default();
    for outer in slots.prepared_live_ids::<ExprId>() {
        let metadata = slots.resolve_prepared(outer).ok()?;
        let HirOrigin::Source(source) = metadata.origin() else {
            continue;
        };
        let Some(attached) = candidate_attached_expression(
            parsed,
            source.syntax(),
            outer,
            retained_style_expressions,
        )?
        else {
            continue;
        };
        let (Some(index_graph), Some(dialogue_graph)) = (
            attached.ambiguous_index_candidate(),
            attached.ambiguous_dialogue_candidate(),
        ) else {
            continue;
        };
        let payload = expressions.resolve_prepared(slots, outer).ok()?;
        let HirExprKind::PostfixBracket(postfix) = payload.kind() else {
            return None;
        };
        let HirPostfixBracketCandidates::Ambiguous {
            index: index_root,
            dialogue: dialogue_root,
        } = postfix.candidates()
        else {
            return None;
        };
        let root_site = outer_root_site(parsed, &attached)?;
        let mut index_cursor = CandidateValidationCursor::new(
            expression_rows,
            parsed,
            slots,
            expressions,
            statements,
            types,
            scopes,
            locals,
            patterns,
            local_resolver,
            &type_expectations,
            outer,
            SyntheticRole::PostfixIndexCandidateExpression,
            &mut expected,
        );
        let actual_index = index_cursor.validate_index_root(
            postfix.target(),
            index_graph,
            &root_site,
            payload.scope(),
            0,
        )?;
        if actual_index != *index_root {
            return None;
        }
        let mut dialogue_cursor = CandidateValidationCursor::new(
            expression_rows,
            parsed,
            slots,
            expressions,
            statements,
            types,
            scopes,
            locals,
            patterns,
            local_resolver,
            &type_expectations,
            outer,
            SyntheticRole::DialogueContentCandidateExpression,
            &mut expected,
        );
        let actual_dialogue = dialogue_cursor.validate_dialogue_root(
            postfix.target(),
            dialogue_graph,
            &root_site,
            payload.scope(),
            0,
        )?;
        if actual_dialogue != *dialogue_root {
            return None;
        }
        let dialogue_payload = expressions.resolve_prepared(slots, actual_dialogue).ok()?;
        let HirExprKind::DialogueContentApplication(application) = dialogue_payload.kind() else {
            return None;
        };
        if !super::candidate_dialogue_manifest_matches(
            index,
            parsed,
            actual_dialogue,
            &attached,
            dialogue_graph,
            application,
        ) {
            return None;
        }
    }
    candidate_descendant_slots_match(slots, scopes, &expected)?;
    Some(expected.expressions)
}

#[allow(
    clippy::option_option,
    reason = "the lookup returns a deliberate tri-state: absent candidate, missing expression, or one attached expression"
)]
fn candidate_attached_expression(
    parsed: &ParsedSource,
    syntax: SyntaxNodeId,
    owner: ExprId,
    retained_style_expressions: &BTreeSet<ExprId>,
) -> Option<Option<AttachedExpressionNode>> {
    match parsed.attached_expression(syntax) {
        Ok(attached) => Some(Some(attached)),
        Err(_)
            if retained_style_expressions.contains(&owner)
                && parsed.typed_node::<MissingExpressionKind>(syntax).is_ok() =>
        {
            Some(None)
        }
        Err(_) => None,
    }
}

#[cfg(test)]
pub(in crate::source_index) fn candidate_expression_slots_match(
    slots: &SlotSnapshot,
    expected: &BTreeSet<ExprId>,
) -> bool {
    candidate_slot_ids::<ExprId>(slots).is_some_and(|actual| &actual == expected)
}

fn candidate_descendant_slots_match(
    slots: &SlotSnapshot,
    scopes: &ArenaSnapshot<HirScope, ScopeId>,
    expected: &CandidateExpectedDescendants,
) -> Option<()> {
    if candidate_slot_ids::<ExprId>(slots)? != expected.expressions
        || candidate_slot_ids::<TypeId>(slots)? != expected.types
        || candidate_slot_ids::<StmtId>(slots)? != expected.statements
        || candidate_slot_ids::<PatternId>(slots)? != expected.patterns
        || candidate_slot_ids::<ScopeId>(slots)? != expected.scopes
        || candidate_slot_ids::<LocalId>(slots)? != expected.locals
    {
        return None;
    }
    for (parent, expected_children) in &expected.scope_children {
        let payload = scopes.resolve_prepared(slots, *parent).ok()?;
        let actual_children = payload
            .children()
            .iter()
            .copied()
            .filter(|child| {
                slots.resolve_prepared(*child).is_ok_and(|metadata| {
                    matches!(
                        metadata.origin(),
                        HirOrigin::Synthetic(key) if is_candidate_role(key.role())
                    )
                })
            })
            .collect::<Vec<_>>();
        if &actual_children != expected_children {
            return None;
        }
    }
    Some(())
}

fn candidate_slot_ids<I>(slots: &SlotSnapshot) -> Option<BTreeSet<I>>
where
    I: crate::identity::HirTypedId + Ord,
{
    let mut actual = BTreeSet::new();
    for id in slots.prepared_live_ids::<I>() {
        let metadata = slots.resolve_prepared(id).ok()?;
        if matches!(
            metadata.origin(),
            HirOrigin::Synthetic(key) if is_candidate_role(key.role())
        ) && !actual.insert(id)
        {
            return None;
        }
    }
    Some(actual)
}

const fn is_candidate_role(role: SyntheticRole) -> bool {
    matches!(
        role,
        SyntheticRole::PostfixIndexCandidateExpression
            | SyntheticRole::DialogueContentCandidateExpression
    )
}

struct CandidateValidationCursor<'a> {
    expression_rows: &'a ExpressionManifestRows<'a>,
    parsed: &'a ParsedSource,
    slots: &'a SlotSnapshot,
    expressions: &'a ArenaSnapshot<HirExpr, ExprId>,
    statements: &'a ArenaSnapshot<HirStmt, StmtId>,
    types: &'a ArenaSnapshot<HirType, TypeId>,
    scopes: &'a ArenaSnapshot<HirScope, ScopeId>,
    locals: &'a ArenaSnapshot<HirLocal, LocalId>,
    patterns: &'a ArenaSnapshot<HirPattern, PatternId>,
    local_resolver: &'a crate::module::HirLocalResolver<'a>,
    type_expectations: &'a BTreeMap<TypeId, CandidateTypeExpectation>,
    outer: ExprId,
    role: SyntheticRole,
    next_expression: u32,
    next_statement: u32,
    next_type: u32,
    next_pattern: u32,
    next_scope: u32,
    next_local: u32,
    expected: &'a mut CandidateExpectedDescendants,
}

impl<'a> CandidateValidationCursor<'a> {
    fn source_index_has_typed_owner(&self, owner: SyntheticOwner) -> bool {
        self.expression_rows.has_typed_owner(owner)
    }

    #[allow(clippy::too_many_arguments)]
    fn new(
        expression_rows: &'a ExpressionManifestRows<'a>,
        parsed: &'a ParsedSource,
        slots: &'a SlotSnapshot,
        expressions: &'a ArenaSnapshot<HirExpr, ExprId>,
        statements: &'a ArenaSnapshot<HirStmt, StmtId>,
        types: &'a ArenaSnapshot<HirType, TypeId>,
        scopes: &'a ArenaSnapshot<HirScope, ScopeId>,
        locals: &'a ArenaSnapshot<HirLocal, LocalId>,
        patterns: &'a ArenaSnapshot<HirPattern, PatternId>,
        local_resolver: &'a crate::module::HirLocalResolver<'a>,
        type_expectations: &'a BTreeMap<TypeId, CandidateTypeExpectation>,
        outer: ExprId,
        role: SyntheticRole,
        expected: &'a mut CandidateExpectedDescendants,
    ) -> Self {
        Self {
            expression_rows,
            parsed,
            slots,
            expressions,
            statements,
            types,
            scopes,
            locals,
            patterns,
            local_resolver,
            type_expectations,
            outer,
            role,
            next_expression: 1,
            next_statement: 0,
            next_type: 0,
            next_pattern: 0,
            next_scope: 0,
            next_local: 0,
            expected,
        }
    }

    fn resolve_expression(
        &mut self,
        ordinal: u32,
        source_site: &HirSourceSite,
        scope: ScopeId,
    ) -> Option<ExprId> {
        let key =
            SyntheticKey::try_new(SyntheticOwner::Expr(self.outer), self.role, ordinal).ok()?;
        let id = self
            .slots
            .resolve_prepared_synthetic::<ExprId>(key)
            .ok()??;
        let metadata = self.slots.resolve_prepared(id).ok()?;
        let payload = self.expressions.resolve_prepared(self.slots, id).ok()?;
        let admits_dialogue_source_manifest =
            self.role == SyntheticRole::DialogueContentCandidateExpression && ordinal == 0;
        if metadata.origin() != &HirOrigin::Synthetic(key)
            || metadata.source_site() != source_site
            || payload.scope() != scope
            || (self
                .expression_rows
                .has_typed_owner(SyntheticOwner::Expr(id))
                && !admits_dialogue_source_manifest)
            || !self.expected.expressions.insert(id)
        {
            return None;
        }
        Some(id)
    }

    fn take_expression(&mut self, source_site: &HirSourceSite, scope: ScopeId) -> Option<ExprId> {
        let ordinal = self.next_expression;
        self.next_expression = ordinal.checked_add(1)?;
        self.resolve_expression(ordinal, source_site, scope)
    }

    fn validate_index_root(
        &mut self,
        target: ExprId,
        graph: AttachedCandidateGraph<'_>,
        source_site: &HirSourceSite,
        scope: ScopeId,
        ordinal: u32,
    ) -> Option<ExprId> {
        let root = self.resolve_expression(ordinal, source_site, scope)?;
        let primary = graph.primary()?;
        let index = self.validate_expression(primary, scope)?;
        let payload = self.expressions.resolve_prepared(self.slots, root).ok()?;
        let HirExprKind::Index(actual) = payload.kind() else {
            return None;
        };
        if actual.target() != target || actual.index() != index.id {
            return None;
        }
        let target_poisoned = self.expression_is_poisoned(target)?;
        let recovery = if target_poisoned {
            Some(recovered_child(HirExprSourceRole::Target))
        } else if index.poisoned {
            Some(recovered_child(HirExprSourceRole::Index))
        } else {
            None
        };
        poison_state_matches(payload.state(), recovery).then_some(root)
    }

    fn validate_dialogue_root(
        &mut self,
        target: ExprId,
        graph: AttachedCandidateGraph<'_>,
        source_site: &HirSourceSite,
        scope: ScopeId,
        ordinal: u32,
    ) -> Option<ExprId> {
        let root = self.resolve_expression(ordinal, source_site, scope)?;
        let mut recovery = self
            .expression_is_poisoned(target)?
            .then_some(recovered_child(HirExprSourceRole::Target));
        let content = graph.dialogue_content()?;
        let mut node_values = BTreeMap::new();
        let mut tag_values = BTreeMap::new();
        if matches!(content, SyntaxDialogueContentProjection::Present(_)) {
            for slot in graph.dialogue_expression_slots()? {
                let role = dialogue_slot_role(slot.owner());
                let child = match slot.slot() {
                    SyntaxExpressionSlot::Authored => {
                        self.validate_expression(slot.node(), scope)?
                    }
                    SyntaxExpressionSlot::Missing => {
                        self.validate_missing(role, slot.source_span(), scope)?
                    }
                };
                if child.poisoned && recovery.is_none() {
                    recovery = Some(recovered_child(role));
                }
                let destination = match slot.owner() {
                    AttachedCandidateDialogueOwner::Node { ordinal } => {
                        node_values.insert(ordinal, child.id)
                    }
                    AttachedCandidateDialogueOwner::Tag { ordinal } => {
                        tag_values.insert(ordinal, child.id)
                    }
                };
                if destination.is_some() {
                    return None;
                }
            }
        } else {
            recovery = Some(HirRecoveryIssue::MissingOperand {
                role: HirExprSourceRole::Content,
            });
        }
        let payload = self.expressions.resolve_prepared(self.slots, root).ok()?;
        let HirExprKind::DialogueContentApplication(application) = payload.kind() else {
            return None;
        };
        let content_matches =
            dialogue_content_matches(application.content(), content, &node_values, &tag_values);
        let coordinates_match = dialogue_coordinates_match(
            application.coordinates(),
            self.expressions.resolve_prepared(self.slots, target).ok()?,
        );
        if application.target() != target
            || application.plan().is_some()
            || application.content().id().owner() != root
            || !content_matches
            || !coordinates_match
        {
            return None;
        }
        dialogue_intrinsic_recovery(
            content,
            application,
            self.slots,
            self.expressions,
            &mut recovery,
        )?;
        poison_state_matches(payload.state(), recovery).then_some(root)
    }

    // This is the single exhaustive payload switch for the candidate grammar.
    #[allow(clippy::too_many_lines)]
    fn validate_expression(
        &mut self,
        node: AttachedCandidateNode<'_>,
        scope: ScopeId,
    ) -> Option<CandidateChild> {
        let projection = node.expression_projection()?;
        let site =
            HirSourceSite::from_attached_span(self.parsed.document(), &node.source_span()).ok()?;
        let id = self.take_expression(&site, scope)?;
        let payload = self.expressions.resolve_prepared(self.slots, id).ok()?;

        let (children, mut recovery) = if matches!(
            projection,
            ExpressionProjection::Call(_)
                | ExpressionProjection::Closure(_)
                | ExpressionProjection::IfLet { .. }
                | ExpressionProjection::Match(_)
                | ExpressionProjection::Block
                | ExpressionProjection::ComputationBlock(_)
                | ExpressionProjection::NamedBlock(_)
        ) {
            (Vec::new(), None)
        } else {
            self.validate_expression_children(node, projection, scope)?
        };
        let payload_matches = match (payload.kind(), projection) {
            (HirExprKind::Unit, ExpressionProjection::Unit) => true,
            (HirExprKind::Literal(actual), ExpressionProjection::Literal(expected)) => {
                crate::final_lowering::literal_projection::literal(expected)
                    .is_ok_and(|expected| &expected == actual)
                    && {
                        recovery =
                            literal_recovery_issue(actual).map(HirRecoveryIssue::MalformedLiteral);
                        true
                    }
            }
            (
                HirExprKind::EntityReference(actual),
                ExpressionProjection::EntityReference(expected),
            ) => {
                crate::final_lowering::id_ref_projection::id_ref(expected)
                    .is_ok_and(|expected| &expected == actual)
                    && {
                        recovery = actual.recovery_issue().map(HirRecoveryIssue::InvalidId);
                        true
                    }
            }
            (HirExprKind::LifetimePath(actual), ExpressionProjection::LifetimePath(expected)) => {
                lifetime_projection_matches(actual, expected) && {
                    recovery = actual
                        .recovery()
                        .map(|value| HirRecoveryIssue::InvalidLifetimeRegistry(value.issue()));
                    true
                }
            }
            (HirExprKind::Path(actual), ExpressionProjection::Path) => {
                let exact = match node.path_expression_view()? {
                    AttachedCandidatePathExpression::Value(expected) => {
                        candidate_path_matches(actual, expected)
                    }
                    AttachedCandidatePathExpression::NominalType(expected) => matches!(
                        actual,
                        HirPathValue::Resolved(actual)
                            if expected
                                .projection()
                                .value()
                                .nominal_path()
                                .is_some_and(|expected| {
                                    super::super::type_projection::hir_path_matches_type_path(
                                        actual, expected,
                                    )
                                })
                    ),
                };
                recovery = match actual {
                    HirPathValue::Resolved(_) => None,
                    HirPathValue::Recovered(actual) => {
                        Some(HirRecoveryIssue::InvalidPath(actual.issue().clone()))
                    }
                };
                exact
            }
            (HirExprKind::ShortVariant(actual), ExpressionProjection::ShortVariant(expected)) => {
                short_variant_projection_matches(actual, expected) && {
                    recovery = actual.recovery_issue().map(HirRecoveryIssue::InvalidName);
                    true
                }
            }
            (HirExprKind::Placeholder(actual), ExpressionProjection::Placeholder(expected)) => {
                matches!(
                    (actual, expected),
                    (
                        HirPlaceholderKind::PartialApplication,
                        SyntaxPlaceholderKind::PartialApplication
                    ) | (
                        HirPlaceholderKind::PipeLeft,
                        SyntaxPlaceholderKind::PipeLeft
                    )
                )
            }
            (HirExprKind::Tuple(actual), ExpressionProjection::Tuple(_)) => {
                actual.elements() == child_ids(&children)
            }
            (HirExprKind::BracketSequence(actual), ExpressionProjection::BracketSequence(_)) => {
                actual.elements() == child_ids(&children)
            }
            (
                HirExprKind::NumericBracketSequence(actual),
                ExpressionProjection::NumericBracketSequence(expected),
            ) => {
                recovery = expected
                    .has_recovery()
                    .then_some(HirRecoveryIssue::InvalidNumericSequence);
                children.is_empty() && numeric_sequence_matches(actual, expected)
            }
            (HirExprKind::ArrayRepeat(actual), ExpressionProjection::ArrayRepeat(_)) => {
                matches!(children.as_slice(), [value, length]
                    if actual.value() == value.id && actual.length() == length.id)
            }
            (HirExprKind::Call(actual), ExpressionProjection::Call(expected)) => {
                let expected_recovery =
                    self.validate_call(node, projection, actual, expected, scope)?;
                recovery = expected_recovery;
                call_projection_matches(actual, expected)
            }
            (HirExprKind::Select(actual), ExpressionProjection::Select(expected)) => {
                if matches!(expected, SyntaxSelectedMember::Missing) && recovery.is_none() {
                    recovery = Some(HirRecoveryIssue::MissingOperand {
                        role: HirExprSourceRole::SelectedMember,
                    });
                }
                matches!(children.as_slice(), [target] if actual.target() == target.id)
                    && (matches!((actual.member(), expected),
                        (HirSelectedMember::Name(actual), SyntaxSelectedMember::Name(expected))
                            if actual.as_str() == expected.as_str())
                        || matches!(
                            (actual.member(), expected),
                            (HirSelectedMember::Missing, SyntaxSelectedMember::Missing)
                        ))
            }
            (HirExprKind::Index(actual), ExpressionProjection::Index(_)) => {
                matches!(children.as_slice(), [target, index]
                    if actual.target() == target.id && actual.index() == index.id)
            }
            (
                HirExprKind::DialogueContentApplication(actual),
                ExpressionProjection::DialogueContentApplication(expected),
            ) => {
                let (target, remaining) = children.split_first()?;
                if target.role != HirExprSourceRole::Target
                    || actual.target() != target.id
                    || actual.plan().is_some()
                    || expected.has_plan()
                    || actual.content().id().owner() != id
                {
                    return None;
                }
                recovery = target
                    .poisoned
                    .then_some(recovered_child(HirExprSourceRole::Target));
                if matches!(
                    expected.form(),
                    SyntaxDialogueApplicationForm::Bracket {
                        terminator:
                            arcweft_lang_syntax::expressions::SyntaxBracketTerminator::RecoveredMissing(_)
                    }
                ) {
                    recovery.get_or_insert(HirRecoveryIssue::MissingOperand {
                        role: HirExprSourceRole::CloseBracket,
                    });
                }
                let mut node_values = BTreeMap::new();
                let mut tag_values = BTreeMap::new();
                match expected.content() {
                    SyntaxDialogueContentProjection::Missing { .. } => {
                        if !remaining.is_empty() {
                            return None;
                        }
                        recovery.get_or_insert(HirRecoveryIssue::MissingOperand {
                            role: HirExprSourceRole::Content,
                        });
                    }
                    SyntaxDialogueContentProjection::Present(_) => {
                        for child in remaining {
                            if child.missing {
                                recovery.get_or_insert(HirRecoveryIssue::MissingOperand {
                                    role: child.role,
                                });
                            } else if child.poisoned {
                                recovery.get_or_insert(recovered_child(child.role));
                            }
                            let previous = match child.role {
                                HirExprSourceRole::DialogueNode {
                                    ordinal,
                                    part:
                                        crate::source_index::HirDialogueNodeSourcePart::Interpolation,
                                } => node_values.insert(ordinal, child.id),
                                HirExprSourceRole::RichTextTag {
                                    tag,
                                    part: crate::source_index::HirRichTextTagSourcePart::Payload,
                                } => tag_values.insert(tag, child.id),
                                _ => return None,
                            };
                            if previous.is_some() {
                                return None;
                            }
                        }
                    }
                }
                dialogue_intrinsic_recovery(
                    expected.content(),
                    actual,
                    self.slots,
                    self.expressions,
                    &mut recovery,
                )?;
                let content_matches = dialogue_content_matches(
                    actual.content(),
                    expected.content(),
                    &node_values,
                    &tag_values,
                );
                let coordinates_match = dialogue_coordinates_match(
                    actual.coordinates(),
                    self.expressions
                        .resolve_prepared(self.slots, target.id)
                        .ok()?,
                );
                content_matches && coordinates_match
            }
            (
                HirExprKind::PostfixBracket(actual),
                ExpressionProjection::PostfixBracket(expected),
            ) => {
                let [target] = children.as_slice() else {
                    return None;
                };
                if actual.target() != target.id {
                    return None;
                }
                match (actual.candidates(), expected) {
                    (
                        HirPostfixBracketCandidates::Invalid { .. },
                        SyntaxPostfixBracketProjection::Invalid { .. },
                    ) => {
                        if recovery.is_none() {
                            recovery = Some(HirRecoveryIssue::InvalidExpression(
                                HirExpressionRecoveryIssue::Generic(
                                    HirGenericExprIssue::TransactionalChildFailure,
                                ),
                            ));
                        }
                        true
                    }
                    (
                        HirPostfixBracketCandidates::Ambiguous { index, dialogue },
                        SyntaxPostfixBracketProjection::Ambiguous { .. },
                    ) => {
                        let site = candidate_node_root_site(self.parsed, node)?;
                        let index_ordinal = self.take_expression_ordinal()?;
                        let actual_index = self.validate_index_root(
                            target.id,
                            node.ambiguous_index_candidate()?,
                            &site,
                            scope,
                            index_ordinal,
                        )?;
                        let dialogue_ordinal = self.take_expression_ordinal()?;
                        let actual_dialogue = self.validate_dialogue_root(
                            target.id,
                            node.ambiguous_dialogue_candidate()?,
                            &site,
                            scope,
                            dialogue_ordinal,
                        )?;
                        *index == actual_index && *dialogue == actual_dialogue
                    }
                    _ => false,
                }
            }
            (HirExprKind::Pipe(actual), ExpressionProjection::Pipe(_)) => {
                matches!(children.as_slice(), [left, right]
                    if actual.left() == left.id && actual.right() == right.id)
            }
            (HirExprKind::Try(actual), ExpressionProjection::Try { .. }) => {
                matches!(children.as_slice(), [operand] if actual.operand() == operand.id)
            }
            (HirExprKind::Await(actual), ExpressionProjection::Await { .. }) => {
                matches!(children.as_slice(), [operand] if actual.operand() == operand.id)
            }
            (
                HirExprKind::Range(actual),
                ExpressionProjection::Range {
                    start,
                    end,
                    inclusive,
                },
            ) => {
                let mut children = children.iter();
                let expected_start = if start.is_some() {
                    Some(children.next()?.id)
                } else {
                    None
                };
                let expected_end = if end.is_some() {
                    Some(children.next()?.id)
                } else {
                    None
                };
                children.next().is_none()
                    && actual.start() == expected_start
                    && actual.end() == expected_end
                    && actual.inclusive() == *inclusive
            }
            (HirExprKind::Record(actual), ExpressionProjection::Record(fields)) => {
                let mut paths = node
                    .children()
                    .filter_map(AttachedCandidateNode::path_projection);
                let path_matches = paths.next().is_some_and(|expected| {
                    candidate_resolved_path_matches(actual.path(), expected)
                }) && paths.next().is_none();
                path_matches
                    && candidate_record_fields_match(
                        actual.fields(),
                        fields,
                        &children,
                        node,
                        self.local_resolver,
                        scope,
                        &mut recovery,
                    )
            }
            (HirExprKind::RecordLiteral(actual), ExpressionProjection::RecordLiteral(fields)) => {
                candidate_record_fields_match(
                    actual.fields(),
                    fields,
                    &children,
                    node,
                    self.local_resolver,
                    scope,
                    &mut recovery,
                )
            }
            (HirExprKind::Binary(actual), ExpressionProjection::Binary { operator, .. }) => {
                matches!(children.as_slice(), [left, right]
                    if actual.left() == left.id && actual.right() == right.id)
                    && binary_operator_matches(actual.operator(), *operator)
            }
            (HirExprKind::If(actual), ExpressionProjection::If { else_branch, .. }) => {
                let (condition, then_branch, else_id) = match (children.as_slice(), else_branch) {
                    ([condition, then_branch, else_branch], Some(_)) => {
                        (condition.id, then_branch.id, else_branch.id)
                    }
                    ([condition, then_branch], None) => {
                        let source = node.expression_components()?.find(|component| {
                            component.role() == ExpressionComponentRole::ElseBranch
                        })?;
                        let unit = self.validate_implicit_unit(source.source_span(), scope)?;
                        (condition.id, then_branch.id, unit)
                    }
                    _ => return None,
                };
                actual.condition() == condition
                    && actual.then_branch() == then_branch
                    && actual.else_branch() == else_id
            }
            (HirExprKind::Borrow(actual), ExpressionProjection::Borrow { kind, .. }) => {
                matches!(children.as_slice(), [operand] if actual.operand() == operand.id)
                    && matches!(
                        (actual.kind(), kind),
                        (
                            HirBorrowKind::Shared,
                            arcweft_lang_syntax::expressions::SyntaxBorrowKind::Shared
                        ) | (
                            HirBorrowKind::Mutable,
                            arcweft_lang_syntax::expressions::SyntaxBorrowKind::Mutable
                        )
                    )
            }
            (HirExprKind::Dereference(actual), ExpressionProjection::Dereference { .. }) => {
                matches!(children.as_slice(), [operand] if actual.operand() == operand.id)
            }
            (HirExprKind::Unary(actual), ExpressionProjection::Unary { operator, .. }) => {
                matches!(children.as_slice(), [operand] if actual.operand() == operand.id)
                    && matches!(
                        (actual.operator(), operator),
                        (
                            HirUnaryOp::Not,
                            arcweft_lang_syntax::expressions::SyntaxUnaryOperator::Not
                        ) | (
                            HirUnaryOp::Negate,
                            arcweft_lang_syntax::expressions::SyntaxUnaryOperator::Negate
                        )
                    )
            }
            (HirExprKind::Closure(actual), ExpressionProjection::Closure(_)) => {
                recovery = self.validate_closure(id, node, actual, scope)?;
                true
            }
            (HirExprKind::IfLet(actual), ExpressionProjection::IfLet { .. }) => {
                recovery = self.validate_if_let(id, node, actual, scope)?;
                true
            }
            (HirExprKind::Match(actual), ExpressionProjection::Match(_)) => {
                recovery = self.validate_match(id, node, actual, scope)?;
                true
            }
            (HirExprKind::Block(actual), ExpressionProjection::Block) => {
                recovery = self.validate_value_block(
                    id,
                    node,
                    scope,
                    actual.scope(),
                    actual.statements(),
                    actual.tail(),
                    block::CandidateTailPolicy::ImplicitUnit,
                )?;
                true
            }
            (
                HirExprKind::ComputationBlock(actual),
                ExpressionProjection::ComputationBlock(expected),
            ) => {
                let (expected_kind, tail_policy) = match expected {
                    arcweft_lang_syntax::expressions::SyntaxComputationBlockKind::Result => (
                        HirComputationBlockKind::Result,
                        block::CandidateTailPolicy::MissingRequired,
                    ),
                    arcweft_lang_syntax::expressions::SyntaxComputationBlockKind::Option => (
                        HirComputationBlockKind::Option,
                        block::CandidateTailPolicy::MissingRequired,
                    ),
                    arcweft_lang_syntax::expressions::SyntaxComputationBlockKind::Seq => (
                        HirComputationBlockKind::Seq,
                        block::CandidateTailPolicy::ImplicitUnit,
                    ),
                    arcweft_lang_syntax::expressions::SyntaxComputationBlockKind::Stream => (
                        HirComputationBlockKind::Stream,
                        block::CandidateTailPolicy::ImplicitUnit,
                    ),
                };
                recovery = self.validate_value_block(
                    id,
                    node,
                    scope,
                    actual.scope(),
                    actual.statements(),
                    actual.tail(),
                    tail_policy,
                )?;
                actual.kind() == expected_kind
            }
            (HirExprKind::NamedBlock(actual), ExpressionProjection::NamedBlock(expected)) => {
                let name_recovery = match expected {
                    Ok(expected) => {
                        let expected =
                            crate::final_lowering::name_projection::name(expected).ok()?;
                        if actual.name() != &HirNamedBlockName::Resolved(expected) {
                            return None;
                        }
                        None
                    }
                    Err(arcweft_lang_syntax::name::SyntaxNameIssue::Missing) => return None,
                    Err(issue) => {
                        let issue = crate::final_lowering::name_projection::name_issue(issue);
                        if actual.name() != &HirNamedBlockName::InvalidPresent(issue) {
                            return None;
                        }
                        Some(HirRecoveryIssue::InvalidName(issue))
                    }
                };
                let block_recovery = self.validate_value_block(
                    id,
                    node,
                    scope,
                    actual.scope(),
                    actual.statements(),
                    actual.tail(),
                    block::CandidateTailPolicy::ImplicitUnit,
                )?;
                recovery = name_recovery.or(block_recovery);
                true
            }
            (HirExprKind::Error(actual), ExpressionProjection::Error) => {
                if recovery.is_none() {
                    recovery = Some(HirRecoveryIssue::InvalidExpression(
                        HirExpressionRecoveryIssue::Generic(
                            HirGenericExprIssue::UnclassifiedSyntax,
                        ),
                    ));
                }
                actual.issue() == HirGenericExprIssue::UnclassifiedSyntax
            }
            _ => false,
        };
        if !payload_matches || !poison_state_matches(payload.state(), recovery) {
            return None;
        }
        Some(CandidateChild {
            id,
            missing: false,
            poisoned: payload.is_poisoned(),
            role: HirExprSourceRole::Recovery,
        })
    }

    fn validate_expression_children(
        &mut self,
        node: AttachedCandidateNode<'_>,
        projection: &ExpressionProjection,
        scope: ScopeId,
    ) -> Option<(Vec<CandidateChild>, Option<HirRecoveryIssue>)> {
        let mut children = Vec::with_capacity(node.semantic_expression_children().len());
        let mut recovery = None;
        for child in node.semantic_expression_children() {
            let role = expression_component_role(projection, child.component_role())?;
            let value = match child {
                AttachedCandidateExpressionChild::Authored { node, .. }
                | AttachedCandidateExpressionChild::Recovered { node, .. } => {
                    self.validate_expression(node, scope)?
                }
                AttachedCandidateExpressionChild::Missing { source, .. } => {
                    self.validate_missing(role, &source, scope)?
                }
            };
            if recovery.is_none() {
                recovery = if value.missing {
                    Some(HirRecoveryIssue::MissingOperand { role })
                } else if value.poisoned {
                    Some(recovered_child(role))
                } else {
                    None
                };
            }
            children.push(CandidateChild { role, ..value });
        }
        Some((children, recovery))
    }

    fn validate_missing(
        &mut self,
        role: HirExprSourceRole,
        source: &arcweft_source::SourceSpan,
        scope: ScopeId,
    ) -> Option<CandidateChild> {
        let site = HirSourceSite::from_attached_span(self.parsed.document(), source).ok()?;
        if !matches!(site, HirSourceSite::Insertion(_)) {
            return None;
        }
        let id = self.take_expression(&site, scope)?;
        let payload = self.expressions.resolve_prepared(self.slots, id).ok()?;
        matches!(
            (payload.kind(), payload.state()),
            (
                HirExprKind::Error(error),
                HirPoisonState::Poisoned(HirRecoveryIssue::MissingOperand { role: actual })
            ) if error.issue() == HirGenericExprIssue::TransactionalChildFailure && *actual == role
        )
        .then_some(CandidateChild {
            id,
            missing: true,
            poisoned: true,
            role,
        })
    }

    fn validate_implicit_unit(
        &mut self,
        source: &arcweft_source::SourceSpan,
        scope: ScopeId,
    ) -> Option<ExprId> {
        let site = HirInsertionPoint::try_new(self.parsed.document(), source.range().start())
            .ok()
            .map(HirSourceSite::Insertion)?;
        let id = self.take_expression(&site, scope)?;
        let payload = self.expressions.resolve_prepared(self.slots, id).ok()?;
        matches!(
            (payload.kind(), payload.state()),
            (HirExprKind::Unit, HirPoisonState::Clean)
        )
        .then_some(id)
    }

    fn take_expression_ordinal(&mut self) -> Option<u32> {
        let ordinal = self.next_expression;
        self.next_expression = ordinal.checked_add(1)?;
        Some(ordinal)
    }

    // The outer `Option` aborts source freeze; the inner one is the call recovery payload.
    #[allow(clippy::option_option, clippy::too_many_lines)]
    fn validate_call(
        &mut self,
        node: AttachedCandidateNode<'_>,
        expression_projection: &ExpressionProjection,
        actual: &crate::expr::HirCallExpr,
        projection: &SyntaxCallProjection,
        scope: ScopeId,
    ) -> Option<Option<HirRecoveryIssue>> {
        let mut children = candidate_role_map(
            node.semantic_expression_children()
                .map(|child| (child.component_role(), child)),
        )?;
        let mut types = candidate_role_map(
            node.direct_semantic_type_roots()
                .map(|root| (root.role(), root)),
        )?;
        let mut argument_states = Vec::new();
        let mut type_argument_states = Vec::new();

        let callee_state = match projection {
            SyntaxCallProjection::CallbackBlock(_) => {
                let callee = self.validate_call_child(
                    expression_projection,
                    &mut children,
                    ExpressionComponentRole::CallCallee,
                    scope,
                )?;
                let argument = self.validate_call_child(
                    expression_projection,
                    &mut children,
                    ExpressionComponentRole::CallArgument {
                        argument: 0,
                        part: SyntaxCallArgumentPart::Value,
                    },
                    scope,
                )?;
                if callee.missing
                    || actual.callee().value_expression() != Some(callee.id)
                    || actual.arguments().first().map(HirCallArgument::value) != Some(argument.id)
                {
                    return None;
                }
                argument_states.push(child_poison(argument.poisoned));
                child_poison(callee.poisoned)
            }
            SyntaxCallProjection::Parenthesized(call) => {
                let callee_state = match call.callee() {
                    SyntaxCallCalleeProjection::Ordinary => {
                        let child = self.validate_call_child(
                            expression_projection,
                            &mut children,
                            ExpressionComponentRole::CallCallee,
                            scope,
                        )?;
                        if child.missing || actual.callee().value_expression() != Some(child.id) {
                            return None;
                        }
                        child_poison(child.poisoned)
                    }
                    SyntaxCallCalleeProjection::UnresolvedDot { .. } => {
                        let value = self.validate_call_child(
                            expression_projection,
                            &mut children,
                            ExpressionComponentRole::CallAssociatedReceiver,
                            scope,
                        )?;
                        let nominal = self.validate_call_type(
                            &mut types,
                            SyntaxCallTypeChildRole::DotNominalReceiver,
                            scope,
                        )?;
                        let HirCallCallee::UnresolvedDot {
                            value_receiver,
                            nominal_receiver,
                            ..
                        } = actual.callee()
                        else {
                            return None;
                        };
                        if value.missing
                            || *value_receiver != value.id
                            || nominal_receiver.type_id() != Some(nominal.id)
                            || matches!(
                                nominal_receiver,
                                HirAssociatedReceiver::InvalidPresent { .. }
                            ) != nominal.poisoned
                        {
                            return None;
                        }
                        child_poison(value.poisoned)
                    }
                    SyntaxCallCalleeProjection::Associated {
                        receiver: SyntaxAssociatedReceiver::Present,
                        ..
                    } => {
                        let receiver = self.validate_call_type(
                            &mut types,
                            SyntaxCallTypeChildRole::AssociatedReceiver,
                            scope,
                        )?;
                        let HirCallCallee::Associated {
                            receiver: actual_receiver,
                            ..
                        } = actual.callee()
                        else {
                            return None;
                        };
                        if actual_receiver.type_id() != Some(receiver.id)
                            || matches!(
                                actual_receiver,
                                HirAssociatedReceiver::InvalidPresent { .. }
                            ) != receiver.poisoned
                        {
                            return None;
                        }
                        HirCallChildPoison::Clean
                    }
                };
                if let Some(application) = call.explicit_type_application() {
                    let HirCallTypeApplication::Present { arguments, .. } =
                        actual.explicit_type_application()
                    else {
                        return None;
                    };
                    for (position, source_argument) in application.arguments().iter().enumerate() {
                        let actual_argument = arguments.get(position)?;
                        if matches!(source_argument, SyntaxCallTypeArgumentProjection::Missing) {
                            if !matches!(actual_argument, HirCallTypeArgument::Missing) {
                                return None;
                            }
                            continue;
                        }
                        let child = self.validate_call_type(
                            &mut types,
                            SyntaxCallTypeChildRole::ExplicitCallTypeArgument {
                                ordinal: u16::try_from(position).ok()?,
                            },
                            scope,
                        )?;
                        if actual_argument.type_id() != Some(child.id)
                            || matches!(actual_argument, HirCallTypeArgument::InvalidPresent { .. })
                                != child.poisoned
                        {
                            return None;
                        }
                        type_argument_states.push(child_poison(child.poisoned));
                    }
                }
                for (position, actual_argument) in actual.arguments().iter().enumerate() {
                    let child = self.validate_call_child(
                        expression_projection,
                        &mut children,
                        ExpressionComponentRole::CallArgument {
                            argument: u16::try_from(position).ok()?,
                            part: SyntaxCallArgumentPart::Value,
                        },
                        scope,
                    )?;
                    if actual_argument.value() != child.id
                        || matches!(actual_argument.value_state(), HirCallValue::Missing { .. })
                            != child.missing
                    {
                        return None;
                    }
                    argument_states.push(child_poison(child.poisoned));
                }
                callee_state
            }
        };
        if !children.is_empty() || !types.is_empty() {
            return None;
        }
        Some(
            actual
                .primary_issue(HirCallChildStates::new(
                    callee_state,
                    &argument_states,
                    &type_argument_states,
                ))
                .map(HirRecoveryIssue::InvalidCall),
        )
    }

    fn validate_call_child(
        &mut self,
        projection: &ExpressionProjection,
        children: &mut BTreeMap<ExpressionComponentRole, AttachedCandidateExpressionChild<'_>>,
        role: ExpressionComponentRole,
        scope: ScopeId,
    ) -> Option<CandidateChild> {
        let child = children.remove(&role)?;
        let source_role = expression_component_role(projection, role)?;
        match child {
            AttachedCandidateExpressionChild::Authored { node, .. }
            | AttachedCandidateExpressionChild::Recovered { node, .. } => {
                self.validate_expression(node, scope)
            }
            AttachedCandidateExpressionChild::Missing { source, .. } => {
                self.validate_missing(source_role, &source, scope)
            }
        }
    }

    fn validate_call_type(
        &mut self,
        types: &mut BTreeMap<SyntaxCallTypeChildRole, AttachedCandidateTypeRoot<'_>>,
        role: SyntaxCallTypeChildRole,
        scope: ScopeId,
    ) -> Option<CandidateTypeChild> {
        let root = types.remove(&role)?;
        self.validate_type(root.node(), scope)
    }

    fn expression_is_poisoned(&self, id: ExprId) -> Option<bool> {
        Some(
            self.expressions
                .resolve_prepared(self.slots, id)
                .ok()?
                .is_poisoned(),
        )
    }
}
