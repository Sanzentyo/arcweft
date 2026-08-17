//! Cross-arena validation for source-backed value blocks and lexical locals.

mod assignment;
mod keyword;
mod required_operand;
mod thread_control;

use std::collections::{BTreeMap, BTreeSet};

use arcweft_lang_syntax::attachment::node::{
    AssertionStatementKind, BlockKind, ExpressionStatementKind, IfStatementKind, LetStatementKind,
    MatchStatementKind, PredicateBlockKind, ProofBlockKind, ProofCallStatementKind,
    UnsafeLifetimeStatementKind,
};
use arcweft_lang_syntax::attachment::{
    AstNode, AttachedAssertionMode, AttachedExpressionNode, AttachedRequiredNestedThreadFlowBody,
    BlockTailNode, IfStatementElseNode, IfStatementHeadNode, LetInitializerNode,
    MatchStatementArmBodyNode, MatchStatementBodyNode, MatchStatementExpressionNode, StatementNode,
    SyntaxNodeId, UnsafeAuditBodyNode, UnsafeAuditIdNode, UnsafeAuditReasonNode,
};
use arcweft_lang_syntax::expressions::ExpressionProjection;
use arcweft_lang_syntax::grammar::SyntaxKind;
use arcweft_lang_syntax::incremental::ParsedSource;

use super::control_projection::canonical_pattern_locals;
use super::pattern_projection::{BindingLocalValidation, binding_locals_match};
use super::{HirSourceSite, HirStmtRecoveryOperandSlot};
use crate::arena::ArenaSnapshot;
use crate::expr::{
    HirBlockExpr, HirComputationBlockExpr, HirComputationBlockKind, HirExpr, HirExprKind,
    HirExpressionRecoveryIssue, HirGenericExprIssue, HirLoopExpr, HirNamedBlockExpr,
    HirPoisonState, HirRecoveryIssue,
};
use crate::identity::{
    ExprId, HirTypedId, ItemId, LocalGeneration, LocalId, PatternId, ScopeId, StmtId, SyntheticKey,
    SyntheticOwner, SyntheticRole,
};
use crate::leaf::{HirIdRefIssue, HirIdRefRecovery, HirIdRefShape, HirIdRefValue, HirName};
use crate::pattern::HirPattern;
use crate::proof_return::HirProofReturnSemanticClass;
use crate::scope::{
    HirLocal, HirLocalKind, HirPatternBindingPolicy, HirScope, HirScopeKind, HirScopeOwner,
};
use crate::slot::{HirOrigin, SlotSnapshot};
use crate::stmt::{
    HirAssertionMode, HirConditionalElseBranch, HirIfLetStmt, HirIfStmt, HirMatchStmt,
    HirSelectStmt, HirStatementContext, HirStmt, HirStmtChildRole, HirStmtKind,
    HirStmtMatchArmBody, HirStmtPoisonState, HirStmtRecoveryIssue, HirThreadStmtBodyRole,
    HirThreadStmtRecoveryIssue, HirUnsafeLifetimeBody,
};

use assignment::assignment_statement_evidence;
use keyword::keyword_statement_evidence;
use required_operand::required_operand_statement_evidence;

pub(super) use thread_control::root_thread_body_graph_matches;

pub(super) struct BlockValidationArenas<'arena> {
    pub(super) expressions: &'arena ArenaSnapshot<HirExpr, ExprId>,
    pub(super) statements: &'arena ArenaSnapshot<HirStmt, StmtId>,
    pub(super) scopes: &'arena ArenaSnapshot<HirScope, ScopeId>,
    pub(super) locals: &'arena ArenaSnapshot<HirLocal, LocalId>,
    pub(super) patterns: &'arena ArenaSnapshot<HirPattern, PatternId>,
}

/// Re-derives a Block's scope, statement, local, and tail graph from the
/// accepted attached node. Qualified IDs may not be substituted merely
/// because their individual arena records are otherwise valid.
pub(super) fn block_expression_matches(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
    owner: ExprId,
    payload: &HirExpr,
    block: &HirBlockExpr,
    attached: &AttachedExpressionNode,
) -> bool {
    value_block_expression_matches(
        parsed,
        slots,
        arenas,
        owner,
        payload,
        ValueBlockGraph {
            scope: block.scope(),
            statements: block.statements(),
            tail: block.tail(),
            omitted_tail: OmittedTailExpectation::ImplicitUnit,
            initial_recovery: None,
        },
        attached,
    )
}

pub(super) fn computation_block_expression_matches(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
    owner: ExprId,
    payload: &HirExpr,
    block: &HirComputationBlockExpr,
    attached: &AttachedExpressionNode,
) -> bool {
    let omitted_tail = match block.kind() {
        HirComputationBlockKind::Result | HirComputationBlockKind::Task => {
            OmittedTailExpectation::MissingRequired
        }
        HirComputationBlockKind::Seq | HirComputationBlockKind::Stream => {
            OmittedTailExpectation::ImplicitUnit
        }
    };
    value_block_expression_matches(
        parsed,
        slots,
        arenas,
        owner,
        payload,
        ValueBlockGraph {
            scope: block.scope(),
            statements: block.statements(),
            tail: block.tail(),
            omitted_tail,
            initial_recovery: None,
        },
        attached,
    )
}

pub(super) fn named_block_expression_matches(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
    owner: ExprId,
    payload: &HirExpr,
    block: &HirNamedBlockExpr,
    attached: &AttachedExpressionNode,
) -> bool {
    value_block_expression_matches(
        parsed,
        slots,
        arenas,
        owner,
        payload,
        ValueBlockGraph {
            scope: block.scope(),
            statements: block.statements(),
            tail: block.tail(),
            omitted_tail: OmittedTailExpectation::ImplicitUnit,
            initial_recovery: block
                .name()
                .recovery_issue()
                .map(HirRecoveryIssue::InvalidName),
        },
        attached,
    )
}

pub(super) fn loop_expression_matches(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
    owner: ExprId,
    payload: &HirExpr,
    block: &HirLoopExpr,
    attached: &AttachedExpressionNode,
) -> bool {
    value_block_expression_matches(
        parsed,
        slots,
        arenas,
        owner,
        payload,
        ValueBlockGraph {
            scope: block.scope(),
            statements: block.statements(),
            tail: block.tail(),
            omitted_tail: OmittedTailExpectation::ImplicitUnit,
            initial_recovery: None,
        },
        attached,
    )
}

/// Re-derives an item-owned Predicate block without introducing a second
/// expression owner for the block surface.
#[derive(Clone, Copy)]
pub(super) struct ItemValueBlockRetained<'a> {
    pub owner: ItemId,
    pub callable_scope: ScopeId,
    pub scope: ScopeId,
    pub statements: &'a [StmtId],
    pub tail: ExprId,
}

pub(super) fn predicate_block_matches(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
    retained: ItemValueBlockRetained<'_>,
    body_id: SyntaxNodeId,
    body_source: arcweft_source::SourceSpan,
    attached: &AstNode<PredicateBlockKind>,
) -> Option<bool> {
    item_owned_value_block_matches(
        parsed,
        slots,
        arenas,
        ItemValueBlockGraph {
            retained,
            attached_id: body_id,
            attached_source: body_source,
            attached_statements: attached.statements().ok()?,
            attached_tail: attached.tail().ok()?,
            scope_kind: HirScopeKind::Predicate,
            omitted_tail: OmittedTailExpectation::MissingRequired,
            statement_context: HirStatementContext::Predicate,
        },
    )
}

/// Re-derives an item-owned Proof block. Unit-returning Proofs own a clean
/// scope-keyed Unit tail; all other omitted tails own typed recovery.
#[allow(
    clippy::too_many_arguments,
    reason = "the Proof block validator compares one typed owner against its exact body, return class, source, and arena context"
)]
pub(super) fn proof_block_matches(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
    retained: ItemValueBlockRetained<'_>,
    return_semantic_class: HirProofReturnSemanticClass,
    body_id: SyntaxNodeId,
    body_source: arcweft_source::SourceSpan,
    attached: &AstNode<ProofBlockKind>,
) -> Option<bool> {
    item_owned_value_block_matches(
        parsed,
        slots,
        arenas,
        ItemValueBlockGraph {
            retained,
            attached_id: body_id,
            attached_source: body_source,
            attached_statements: attached.statements().ok()?,
            attached_tail: attached.tail().ok()?,
            scope_kind: HirScopeKind::Proof,
            omitted_tail: if return_semantic_class.admits_implicit_unit_tail() {
                OmittedTailExpectation::ImplicitUnit
            } else {
                OmittedTailExpectation::MissingRequired
            },
            statement_context: HirStatementContext::Proof,
        },
    )
}

/// Re-derives an ordinary function's block-only body and implicit Unit tail.
pub(super) fn function_block_matches(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
    retained: ItemValueBlockRetained<'_>,
    body_id: SyntaxNodeId,
    body_source: arcweft_source::SourceSpan,
    attached: &AstNode<BlockKind>,
) -> Option<bool> {
    item_owned_value_block_matches(
        parsed,
        slots,
        arenas,
        ItemValueBlockGraph {
            retained,
            attached_id: body_id,
            attached_source: body_source,
            attached_statements: attached.statements().ok()?,
            attached_tail: attached.tail().ok()?,
            scope_kind: HirScopeKind::Block,
            omitted_tail: OmittedTailExpectation::ImplicitUnit,
            statement_context: HirStatementContext::Ordinary,
        },
    )
}

/// Source-ordered body payload of a Trait/Impl method. Unlike an ordinary
/// Function, the method body deliberately reuses the callable scope so its
/// parameters and body locals have one lexical owner.
#[derive(Clone, Copy)]
pub(super) struct MethodValueBlockRetained<'a> {
    pub owner: ItemId,
    pub scope: ScopeId,
    pub parameter_locals: &'a [LocalId],
    pub statements: &'a [StmtId],
    pub tail: ExprId,
}

/// Re-derives a method block without admitting a second body scope.
pub(super) fn method_block_matches(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
    retained: MethodValueBlockRetained<'_>,
    attached: &AstNode<BlockKind>,
    generations: &mut BTreeMap<HirName, LocalGeneration>,
) -> Option<bool> {
    let attached_statements = attached.statements().ok()?;
    if attached_statements.len() != retained.statements.len()
        || slots
            .prepared_source_owner::<ScopeId>(attached.id())
            .is_some()
    {
        return None;
    }
    let scope = arenas.scopes.resolve_prepared(slots, retained.scope).ok()?;
    if scope.kind() != HirScopeKind::Callable
        || scope.owner() != &HirScopeOwner::Item(retained.owner)
    {
        return None;
    }

    let mut expected_locals = retained.parameter_locals.to_vec();
    if expected_locals
        .iter()
        .copied()
        .collect::<BTreeSet<_>>()
        .len()
        != expected_locals.len()
        || expected_locals.iter().copied().any(|local| {
            !arenas
                .locals
                .resolve_prepared(slots, local)
                .is_ok_and(|payload| payload.scope() == retained.scope)
        })
    {
        return None;
    }

    let mut recovered = false;
    for (statement, attached_statement) in retained
        .statements
        .iter()
        .copied()
        .zip(&attached_statements)
    {
        let evidence = statement_matches(
            parsed,
            slots,
            arenas,
            statement,
            attached_statement,
            retained.scope,
            generations,
            HirStatementContext::Ordinary,
        )?;
        recovered |= evidence.is_poisoned();
        expected_locals.extend(evidence.locals);
    }
    if scope.locals() != expected_locals
        || !scope_locals_are_exact(retained.scope, &expected_locals, slots, arenas)
    {
        return None;
    }

    let expected_statements = retained.statements.iter().copied().collect::<BTreeSet<_>>();
    let actual_statements = arenas
        .statements
        .try_iter_prepared(slots)
        .ok()?
        .filter_map(|(owner, payload)| (payload.scope() == retained.scope).then_some(owner))
        .collect::<BTreeSet<_>>();
    if actual_statements != expected_statements {
        return None;
    }

    match attached.tail().ok()? {
        BlockTailNode::Expression(expression) => {
            let expression = expression.semantic().ok()?;
            if !source_expression_matches(
                slots,
                arenas.expressions,
                retained.tail,
                &expression,
                retained.scope,
            ) {
                return None;
            }
            recovered |= arenas
                .expressions
                .resolve_prepared(slots, retained.tail)
                .is_ok_and(HirExpr::is_poisoned);
        }
        BlockTailNode::Omitted(omitted) => {
            if !implicit_scope_unit_tail_matches(
                parsed,
                slots,
                arenas.expressions,
                retained.scope,
                retained.tail,
                omitted.source_span(),
            ) {
                return None;
            }
        }
    }
    Some(recovered)
}

/// Re-derives an item-owned statement-only block without accepting a hidden
/// value tail or a second expression owner for the plan body.
#[derive(Clone, Copy)]
pub(super) struct ItemStatementBlockRetained<'a> {
    pub owner: ItemId,
    pub parent_scope: ScopeId,
    pub scope: ScopeId,
    pub statements: &'a [StmtId],
}

pub(super) struct AttachedStatementBlock<'a> {
    pub id: SyntaxNodeId,
    pub source: arcweft_source::SourceSpan,
    pub statements: &'a [StatementNode],
}

pub(super) fn item_statement_block_matches(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
    retained: ItemStatementBlockRetained<'_>,
    attached: AttachedStatementBlock<'_>,
) -> Option<bool> {
    item_statement_block_matches_with_prefix(
        parsed,
        slots,
        arenas,
        retained,
        attached,
        &[],
        &mut BTreeMap::new(),
    )
}

/// Re-derives an item-owned statement body whose lexical scope already owns
/// source-ordered Pattern bindings. The caller validates those bindings and
/// supplies the resulting name-generation ledger; statement locals must then
/// follow the prefix exactly.
pub(super) fn item_statement_block_matches_with_prefix(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
    retained: ItemStatementBlockRetained<'_>,
    attached: AttachedStatementBlock<'_>,
    prefix_locals: &[LocalId],
    generations: &mut BTreeMap<HirName, LocalGeneration>,
) -> Option<bool> {
    if retained.statements.len() != attached.statements.len()
        || !source_owner_matches(
            slots,
            retained.scope,
            attached.id,
            &HirSourceSite::Span(attached.source),
        )
    {
        return None;
    }
    let scope = arenas.scopes.resolve_prepared(slots, retained.scope).ok()?;
    let parent = arenas
        .scopes
        .resolve_prepared(slots, retained.parent_scope)
        .ok()?;
    if scope.kind() != HirScopeKind::Block
        || scope.parent() != Some(retained.parent_scope)
        || scope.owner() != &HirScopeOwner::Item(retained.owner)
        || !parent.children().contains(&retained.scope)
    {
        return None;
    }

    let mut expected_locals = prefix_locals.to_vec();
    if expected_locals
        .iter()
        .copied()
        .collect::<BTreeSet<_>>()
        .len()
        != expected_locals.len()
        || expected_locals.iter().copied().any(|local| {
            !arenas
                .locals
                .resolve_prepared(slots, local)
                .is_ok_and(|payload| payload.scope() == retained.scope)
        })
    {
        return None;
    }
    let mut recovered = false;
    for (statement, attached) in retained.statements.iter().copied().zip(attached.statements) {
        let evidence = statement_matches(
            parsed,
            slots,
            arenas,
            statement,
            attached,
            retained.scope,
            generations,
            HirStatementContext::Ordinary,
        )?;
        recovered |= evidence.is_poisoned();
        expected_locals.extend(evidence.locals);
    }
    if scope.locals() != expected_locals
        || !scope_locals_are_exact(retained.scope, &expected_locals, slots, arenas)
    {
        return None;
    }

    let expected_statements = retained.statements.iter().copied().collect::<BTreeSet<_>>();
    let actual_statements = arenas
        .statements
        .try_iter_prepared(slots)
        .ok()?
        .filter_map(|(owner, payload)| (payload.scope() == retained.scope).then_some(owner))
        .collect::<BTreeSet<_>>();
    if actual_statements != expected_statements {
        return None;
    }

    let owns_tail = slots.prepared_live_ids::<ExprId>().any(|expression| {
        slots.resolve_prepared(expression).is_ok_and(|metadata| {
            matches!(
                metadata.origin(),
                HirOrigin::Synthetic(key)
                    if key.owner() == SyntheticOwner::Scope(retained.scope)
                        && matches!(
                            key.role(),
                            SyntheticRole::ImplicitUnitTail | SyntheticRole::MissingRequiredTail
                        )
            )
        })
    });
    (!owns_tail).then_some(recovered)
}

struct ItemValueBlockGraph<'a> {
    retained: ItemValueBlockRetained<'a>,
    attached_id: SyntaxNodeId,
    attached_source: arcweft_source::SourceSpan,
    attached_statements: Vec<StatementNode>,
    attached_tail: BlockTailNode,
    scope_kind: HirScopeKind,
    omitted_tail: OmittedTailExpectation,
    statement_context: HirStatementContext,
}

fn item_owned_value_block_matches(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
    graph: ItemValueBlockGraph<'_>,
) -> Option<bool> {
    let ItemValueBlockGraph {
        retained:
            ItemValueBlockRetained {
                owner,
                callable_scope,
                scope,
                statements,
                tail,
            },
        attached_id,
        attached_source,
        attached_statements,
        attached_tail,
        scope_kind,
        omitted_tail,
        statement_context,
    } = graph;
    if attached_statements.len() != statements.len()
        || !source_owner_matches(
            slots,
            scope,
            attached_id,
            &HirSourceSite::Span(attached_source),
        )
    {
        return None;
    }
    let block_scope = arenas.scopes.resolve_prepared(slots, scope).ok()?;
    let callable = arenas.scopes.resolve_prepared(slots, callable_scope).ok()?;
    if block_scope.kind() != scope_kind
        || block_scope.parent() != Some(callable_scope)
        || block_scope.owner() != &HirScopeOwner::Item(owner)
        || !callable.children().contains(&scope)
    {
        return None;
    }

    let mut expected_locals = Vec::new();
    let mut recovered = false;
    let mut generations = BTreeMap::new();
    for (statement, attached_statement) in statements.iter().copied().zip(&attached_statements) {
        let evidence = statement_matches(
            parsed,
            slots,
            arenas,
            statement,
            attached_statement,
            scope,
            &mut generations,
            statement_context,
        )?;
        recovered |= evidence.is_poisoned();
        expected_locals.extend(evidence.locals);
    }
    if block_scope.locals() != expected_locals
        || !scope_locals_are_exact(scope, &expected_locals, slots, arenas)
    {
        return None;
    }

    match attached_tail {
        BlockTailNode::Expression(expression) => {
            let expression = expression.semantic().ok()?;
            if !source_expression_matches(slots, arenas.expressions, tail, &expression, scope) {
                return None;
            }
            recovered |= arenas
                .expressions
                .resolve_prepared(slots, tail)
                .is_ok_and(HirExpr::is_poisoned);
        }
        BlockTailNode::Omitted(omitted) => {
            let matches = match omitted_tail {
                OmittedTailExpectation::ImplicitUnit => implicit_scope_unit_tail_matches(
                    parsed,
                    slots,
                    arenas.expressions,
                    scope,
                    tail,
                    omitted.source_span(),
                ),
                OmittedTailExpectation::MissingRequired => missing_scope_tail_matches(
                    parsed,
                    slots,
                    arenas.expressions,
                    scope,
                    tail,
                    omitted.source_span(),
                ),
            };
            if !matches {
                return None;
            }
            recovered |= matches!(omitted_tail, OmittedTailExpectation::MissingRequired);
        }
    }
    Some(recovered)
}

fn scope_locals_are_exact(
    scope: ScopeId,
    expected: &[LocalId],
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
) -> bool {
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    let Ok(locals) = arenas.locals.try_iter_prepared(slots) else {
        return false;
    };
    let actual = locals
        .filter_map(|(local, payload)| (payload.scope() == scope).then_some(local))
        .collect::<BTreeSet<_>>();
    actual == expected
}

#[derive(Clone, Copy)]
enum OmittedTailExpectation {
    ImplicitUnit,
    MissingRequired,
}

struct ValueBlockGraph<'a> {
    scope: ScopeId,
    statements: &'a [StmtId],
    tail: ExprId,
    omitted_tail: OmittedTailExpectation,
    initial_recovery: Option<HirRecoveryIssue>,
}

#[allow(
    clippy::too_many_lines,
    reason = "one value-block projection proves statement order, lexical generations, tail ownership, and recovery together"
)]
fn value_block_expression_matches(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
    owner: ExprId,
    payload: &HirExpr,
    graph: ValueBlockGraph<'_>,
    attached: &AttachedExpressionNode,
) -> bool {
    let Some(attached_block) = attached.block() else {
        return false;
    };
    let Ok(attached_statements) = attached_block.statements() else {
        return false;
    };
    if attached_statements.len() != graph.statements.len()
        || !source_owner_matches(
            slots,
            graph.scope,
            attached_block.id(),
            &HirSourceSite::Span(attached_block.source_span()),
        )
    {
        return false;
    }

    let Ok(block_scope) = arenas.scopes.resolve_prepared(slots, graph.scope) else {
        return false;
    };
    if block_scope.kind() != HirScopeKind::Block
        || block_scope.parent() != Some(payload.scope())
        || block_scope.owner() != &HirScopeOwner::Expr(owner)
        || !arenas
            .scopes
            .resolve_prepared(slots, payload.scope())
            .is_ok_and(|parent| parent.children().contains(&graph.scope))
    {
        return false;
    }

    let mut expected_locals = Vec::new();
    let mut expected_recovery = graph.initial_recovery;
    let mut generations = BTreeMap::new();
    for (ordinal, (statement, attached_statement)) in graph
        .statements
        .iter()
        .copied()
        .zip(&attached_statements)
        .enumerate()
    {
        let Some(evidence) = statement_matches(
            parsed,
            slots,
            arenas,
            statement,
            attached_statement,
            graph.scope,
            &mut generations,
            HirStatementContext::Ordinary,
        ) else {
            return false;
        };
        let poisoned = evidence.is_poisoned();
        expected_locals.extend(evidence.locals);
        if poisoned && expected_recovery.is_none() {
            let Ok(ordinal) = u32::try_from(ordinal) else {
                return false;
            };
            expected_recovery = Some(HirRecoveryIssue::InvalidExpression(
                HirExpressionRecoveryIssue::RecoveredChild {
                    role: super::HirExprSourceRole::Statement { ordinal },
                },
            ));
        }
    }
    if block_scope.locals() != expected_locals {
        return false;
    }

    match attached_block.tail() {
        Ok(BlockTailNode::Expression(expression)) => {
            let Ok(attached_tail) = expression.semantic() else {
                return false;
            };
            if !source_expression_matches(
                slots,
                arenas.expressions,
                graph.tail,
                &attached_tail,
                graph.scope,
            ) {
                return false;
            }
            if arenas
                .expressions
                .resolve_prepared(slots, graph.tail)
                .is_ok_and(HirExpr::is_poisoned)
            {
                expected_recovery.get_or_insert(HirRecoveryIssue::InvalidExpression(
                    HirExpressionRecoveryIssue::RecoveredChild {
                        role: super::HirExprSourceRole::Tail,
                    },
                ));
            }
        }
        Ok(BlockTailNode::Omitted(omitted)) => {
            let matches = match graph.omitted_tail {
                OmittedTailExpectation::ImplicitUnit => implicit_unit_tail_matches(
                    parsed,
                    slots,
                    arenas.expressions,
                    owner,
                    graph.tail,
                    graph.scope,
                    omitted.source_span(),
                ),
                OmittedTailExpectation::MissingRequired => missing_expr_tail_matches(
                    parsed,
                    slots,
                    arenas.expressions,
                    owner,
                    graph.tail,
                    graph.scope,
                    omitted.source_span(),
                ),
            };
            if !matches {
                return false;
            }
            if matches!(graph.omitted_tail, OmittedTailExpectation::MissingRequired) {
                expected_recovery.get_or_insert(HirRecoveryIssue::MissingRequiredTail);
            }
        }
        Err(_) => return false,
    }
    let expected_state = expected_recovery.map_or(HirPoisonState::Clean, HirPoisonState::Poisoned);
    payload.state() == &expected_state
}

pub(super) struct StatementEvidence {
    locals: Box<[LocalId]>,
    state: HirStmtPoisonState,
}

impl StatementEvidence {
    fn is_poisoned(&self) -> bool {
        self.state.is_poisoned()
    }
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "one exhaustive statement projection dispatches the closed typed family with shared lexical-generation state"
)]
pub(super) fn statement_matches(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
    owner: StmtId,
    attached: &StatementNode,
    scope: ScopeId,
    generations: &mut BTreeMap<HirName, LocalGeneration>,
    context: HirStatementContext,
) -> Option<StatementEvidence> {
    if !source_owner_matches(
        slots,
        owner,
        attached.id(),
        &HirSourceSite::Span(attached.source_span()),
    ) {
        return None;
    }
    let statement = arenas.statements.resolve_prepared(slots, owner).ok()?;
    if statement.scope() != scope {
        return None;
    }

    let evidence = match (attached.kind(), statement.kind()) {
        (
            SyntaxKind::LetStatement,
            HirStmtKind::Let {
                pattern,
                annotation,
                initializer,
                locals,
            },
        ) => {
            let attached = attached.cast::<LetStatementKind>().ok()?;
            if annotation.is_some()
                || !let_initializer_matches(
                    parsed,
                    slots,
                    arenas.expressions,
                    owner,
                    *initializer,
                    attached.initializer().ok()?,
                    scope,
                    attached.range().end(),
                )
            {
                return None;
            }
            let attached_pattern = attached.pattern().ok()?.semantic().ok()?;
            if !source_owner_matches(
                slots,
                *pattern,
                attached_pattern.id(),
                &HirSourceSite::Span(attached_pattern.whole_source_span()),
            ) {
                return None;
            }
            let pattern_payload = arenas.patterns.resolve_prepared(slots, *pattern).ok()?;
            let expected_locals =
                canonical_pattern_locals(slots, arenas, *pattern, *pattern, scope)?;
            let expected_local_ids = expected_locals
                .iter()
                .map(|expected| expected.local)
                .collect::<Vec<_>>();
            let mut local_validation = BindingLocalValidation::new(
                scope,
                context.let_binding_policy(),
                generations,
                slots,
                arenas.patterns,
                arenas.locals,
            );
            if pattern_payload.scope() != scope
                || expected_local_ids.as_slice() != locals.as_ref()
                || expected_local_ids
                    .iter()
                    .copied()
                    .collect::<BTreeSet<_>>()
                    .len()
                    != expected_local_ids.len()
                || !binding_locals_match(&attached_pattern, &expected_locals, &mut local_validation)
                || expected_locals.iter().any(|expected| {
                    !arenas
                        .locals
                        .resolve_prepared(slots, expected.local)
                        .is_ok_and(|payload| {
                            payload.scope() == scope
                                && payload.kind() == HirLocalKind::LetBinding
                                && payload.pattern() == Some(expected.pattern)
                        })
                })
            {
                return None;
            }
            let initializer_poisoned = arenas
                .expressions
                .resolve_prepared(slots, *initializer)
                .is_ok_and(HirExpr::is_poisoned);
            Some(StatementEvidence {
                locals: locals.clone(),
                state: if pattern_payload.is_poisoned() || local_validation.is_poisoned() {
                    HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::RecoveredChild {
                        role: HirStmtChildRole::Pattern,
                    })
                } else if initializer_poisoned {
                    HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::RecoveredChild {
                        role: HirStmtChildRole::Initializer,
                    })
                } else {
                    HirStmtPoisonState::Clean
                },
            })
        }
        (SyntaxKind::AssignmentStatement, HirStmtKind::Assign { target, value })
        | (SyntaxKind::LifetimeSetStatement, HirStmtKind::LifetimeSet { target, value }) => {
            assignment_statement_evidence(
                parsed,
                slots,
                arenas.expressions,
                owner,
                attached,
                *target,
                *value,
                scope,
            )
        }
        (
            SyntaxKind::ReturnStatement
            | SyntaxKind::YieldStatement
            | SyntaxKind::WaitStatement
            | SyntaxKind::CloseStatement
            | SyntaxKind::SelectStatement,
            kind @ (HirStmtKind::Return { .. }
            | HirStmtKind::Yield { .. }
            | HirStmtKind::Wait { .. }
            | HirStmtKind::Close { .. }
            | HirStmtKind::Select(HirSelectStmt::Operand(_))),
        ) => required_operand_statement_evidence(
            parsed,
            slots,
            arenas.expressions,
            owner,
            attached,
            kind,
            scope,
        ),
        (SyntaxKind::ExpressionStatement, HirStmtKind::Expression { expression }) => {
            let attached = attached.cast::<ExpressionStatementKind>().ok()?;
            let attached_expression = attached.expression().ok()?.semantic().ok()?;
            if !source_expression_matches(
                slots,
                arenas.expressions,
                *expression,
                &attached_expression,
                scope,
            ) {
                return None;
            }
            Some(StatementEvidence {
                locals: Box::new([]),
                state: if arenas
                    .expressions
                    .resolve_prepared(slots, *expression)
                    .is_ok_and(HirExpr::is_poisoned)
                {
                    HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::RecoveredChild {
                        role: HirStmtChildRole::Expression,
                    })
                } else {
                    HirStmtPoisonState::Clean
                },
            })
        }
        (
            SyntaxKind::OutStatement
            | SyntaxKind::GotoStatement
            | SyntaxKind::DeferStatement
            | SyntaxKind::SignalStatement
            | SyntaxKind::BreakStatement
            | SyntaxKind::ContinueStatement,
            kind @ (HirStmtKind::Out { .. }
            | HirStmtKind::Goto { .. }
            | HirStmtKind::Defer { .. }
            | HirStmtKind::Signal { .. }
            | HirStmtKind::Break { .. }
            | HirStmtKind::Continue { .. }),
        ) => keyword_statement_evidence(
            parsed,
            slots,
            arenas.expressions,
            owner,
            attached,
            kind,
            scope,
        ),
        (SyntaxKind::ProofCallStatement, HirStmtKind::ProofCall { call }) => {
            let attached = attached.cast::<ProofCallStatementKind>().ok()?;
            let attached_call = attached.callee().ok()?.semantic().ok()?;
            if !source_expression_matches(slots, arenas.expressions, *call, &attached_call, scope) {
                return None;
            }
            Some(StatementEvidence {
                locals: Box::new([]),
                state: if arenas
                    .expressions
                    .resolve_prepared(slots, *call)
                    .is_ok_and(HirExpr::is_poisoned)
                {
                    HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::RecoveredChild {
                        role: HirStmtChildRole::Expression,
                    })
                } else {
                    HirStmtPoisonState::Clean
                },
            })
        }
        (SyntaxKind::AssertionStatement, HirStmtKind::Assertion { mode, conditions }) => {
            let assertion = attached
                .cast::<AssertionStatementKind>()
                .ok()?
                .semantics()
                .ok()?;
            let expected_mode = match assertion.mode() {
                AttachedAssertionMode::Resolved { value, .. } => HirAssertionMode::Resolved(*value),
                AttachedAssertionMode::Recovered { .. } => HirAssertionMode::Recovered,
            };
            if *mode != expected_mode || conditions.len() != assertion.conditions().len() {
                return None;
            }
            let mut condition_recovery = false;
            for (condition, attached_condition) in
                conditions.iter().copied().zip(assertion.conditions())
            {
                if !source_expression_matches(
                    slots,
                    arenas.expressions,
                    condition,
                    attached_condition,
                    scope,
                ) {
                    return None;
                }
                condition_recovery |= arenas
                    .expressions
                    .resolve_prepared(slots, condition)
                    .is_ok_and(HirExpr::is_poisoned);
            }
            let state = if matches!(expected_mode, HirAssertionMode::Recovered) {
                HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::InvalidAssertionMode)
            } else if conditions.is_empty() {
                HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::MissingAssertionCondition)
            } else if condition_recovery {
                HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::RecoveredChild {
                    role: HirStmtChildRole::Condition,
                })
            } else if assertion.has_recovery() {
                HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::MalformedAssertion)
            } else {
                HirStmtPoisonState::Clean
            };
            Some(StatementEvidence {
                locals: Box::new([]),
                state,
            })
        }
        (SyntaxKind::ErrorStatement, HirStmtKind::Error) => Some(StatementEvidence {
            locals: Box::new([]),
            state: HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::UnclassifiedSyntax),
        }),
        (SyntaxKind::UnsafeLifetimeStatement, HirStmtKind::UnsafeLifetime { audit, body }) => {
            unsafe_lifetime_statement_matches(
                parsed, slots, arenas, owner, attached, scope, context, audit, body,
            )
        }
        (SyntaxKind::IfStatement, HirStmtKind::IfLet(statement)) => if_let_statement_matches(
            parsed, slots, arenas, owner, attached, scope, context, statement,
        ),
        (SyntaxKind::MatchStatement, HirStmtKind::Match(statement)) => match_statement_matches(
            parsed, slots, arenas, owner, attached, scope, context, statement,
        ),
        (SyntaxKind::IfStatement, HirStmtKind::If(statement)) => ordinary_if_statement_matches(
            parsed, slots, arenas, owner, attached, scope, context, statement,
        ),
        (
            SyntaxKind::WhileStatement
            | SyntaxKind::WhileLetStatement
            | SyntaxKind::ForStatement
            | SyntaxKind::SelectStatement,
            kind @ (HirStmtKind::While(_)
            | HirStmtKind::WhileLet(_)
            | HirStmtKind::For(_)
            | HirStmtKind::Select(HirSelectStmt::Branches { .. })),
        ) => thread_control::thread_control_statement_evidence(
            parsed, slots, arenas, owner, attached, scope, context, kind,
        ),
        _ => None,
    }?;
    (statement.state() == &evidence.state).then_some(evidence)
}

#[allow(clippy::too_many_arguments)]
#[allow(
    clippy::too_many_lines,
    reason = "one Match projection proves scrutinee, per-arm scopes, bindings, bodies, and recovery together"
)]
fn match_statement_matches(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
    owner: StmtId,
    attached: &StatementNode,
    outer_scope: ScopeId,
    context: HirStatementContext,
    statement: &HirMatchStmt,
) -> Option<StatementEvidence> {
    let attached = attached.cast::<MatchStatementKind>().ok()?;
    let scrutinee_poisoned = match attached.scrutinee().ok()? {
        MatchStatementExpressionNode::Expression(attached_scrutinee) => {
            let attached_scrutinee = attached_scrutinee.semantic().ok()?;
            if !source_expression_matches(
                slots,
                arenas.expressions,
                statement.scrutinee(),
                &attached_scrutinee,
                outer_scope,
            ) {
                return None;
            }
            arenas
                .expressions
                .resolve_prepared(slots, statement.scrutinee())
                .is_ok_and(HirExpr::is_poisoned)
        }
        MatchStatementExpressionNode::Missing(missing) => {
            if !missing_statement_expression_matches(
                parsed,
                slots,
                arenas.expressions,
                owner,
                statement.scrutinee(),
                outer_scope,
                HirStmtRecoveryOperandSlot::MatchScrutinee {
                    insertion: missing.range().start(),
                },
            ) {
                return None;
            }
            true
        }
    };
    let mut recovery = scrutinee_poisoned.then_some(HirStmtRecoveryIssue::RecoveredChild {
        role: HirStmtChildRole::Scrutinee,
    });

    let attached_body = attached.body_or_missing().ok()?;
    let attached_arms = attached_body.arms().ok()?;
    let body_unclosed = match &attached_body {
        MatchStatementBodyNode::Missing(_) => {
            recovery.get_or_insert(HirStmtRecoveryIssue::Thread(
                HirThreadStmtRecoveryIssue::MissingBody {
                    role: HirThreadStmtBodyRole::Match,
                },
            ));
            false
        }
        MatchStatementBodyNode::Block(body) => body.close_delimiter().ok()?.range().is_empty(),
    };
    if attached_arms.len() != statement.arms().len() {
        return None;
    }
    if attached_arms.is_empty() && !matches!(attached_body, MatchStatementBodyNode::Missing(_)) {
        recovery.get_or_insert(HirStmtRecoveryIssue::Thread(
            HirThreadStmtRecoveryIssue::EmptyMatch,
        ));
    }
    let parent_scope = arenas.scopes.resolve_prepared(slots, outer_scope).ok()?;
    for (arm_ordinal, (attached_arm, arm)) in attached_arms.iter().zip(statement.arms()).enumerate()
    {
        let arm_ordinal = u32::try_from(arm_ordinal).ok()?;
        let attached_arm_body = attached_arm.body().ok()?;
        let thread_block_arm = matches!(
            (&context, &attached_arm_body),
            (
                HirStatementContext::Thread,
                MatchStatementArmBodyNode::Block(_)
            )
        );
        let arm_scope = arenas.scopes.resolve_prepared(slots, arm.scope()).ok()?;
        if (!thread_block_arm
            && (!source_owner_matches(
                slots,
                arm.scope(),
                attached_arm.id(),
                &HirSourceSite::Span(attached_arm.source_span()),
            ) || arm_scope.kind() != HirScopeKind::MatchArm))
            || (thread_block_arm && arm_scope.kind() != HirScopeKind::Block)
            || arm_scope.parent() != Some(outer_scope)
            || arm_scope.owner() != &HirScopeOwner::Stmt(owner)
            || !parent_scope.children().contains(&arm.scope())
        {
            return None;
        }

        let attached_pattern = attached_arm.pattern().ok()?.semantic().ok()?;
        if !source_owner_matches(
            slots,
            arm.pattern(),
            attached_pattern.id(),
            &HirSourceSite::Span(attached_pattern.whole_source_span()),
        ) {
            return None;
        }
        let pattern = arenas
            .patterns
            .resolve_prepared(slots, arm.pattern())
            .ok()?;
        if pattern.scope() != arm.scope() {
            return None;
        }
        let expected_pattern_locals =
            canonical_pattern_locals(slots, arenas, arm.pattern(), arm.pattern(), arm.scope())?;
        let expected_pattern_ids = expected_pattern_locals
            .iter()
            .map(|expected| expected.local)
            .collect::<Vec<_>>();
        let mut arm_generations = BTreeMap::new();
        let mut local_validation = BindingLocalValidation::new(
            arm.scope(),
            HirPatternBindingPolicy::MatchBinding,
            &mut arm_generations,
            slots,
            arenas.patterns,
            arenas.locals,
        );
        if expected_pattern_ids != arm.locals()
            || expected_pattern_ids
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                .len()
                != expected_pattern_ids.len()
            || !binding_locals_match(
                &attached_pattern,
                &expected_pattern_locals,
                &mut local_validation,
            )
            || !expected_pattern_locals.iter().all(|expected| {
                arenas
                    .locals
                    .resolve_prepared(slots, expected.local)
                    .is_ok_and(|local| {
                        local.scope() == arm.scope()
                            && local.kind() == HirLocalKind::MatchBinding
                            && local.pattern() == Some(expected.pattern)
                    })
            })
        {
            return None;
        }
        if pattern.is_poisoned() || local_validation.is_poisoned() {
            recovery.get_or_insert(HirStmtRecoveryIssue::RecoveredChild {
                role: HirStmtChildRole::MatchArmPattern { arm: arm_ordinal },
            });
        }

        match (attached_arm.guard().ok()?, arm.guard()) {
            (None, None) => {}
            (Some(MatchStatementExpressionNode::Expression(attached_guard)), Some(guard)) => {
                let attached_guard = attached_guard.semantic().ok()?;
                if !source_expression_matches(
                    slots,
                    arenas.expressions,
                    guard,
                    &attached_guard,
                    arm.scope(),
                ) {
                    return None;
                }
                if arenas
                    .expressions
                    .resolve_prepared(slots, guard)
                    .is_ok_and(HirExpr::is_poisoned)
                {
                    recovery.get_or_insert(HirStmtRecoveryIssue::RecoveredChild {
                        role: HirStmtChildRole::MatchArmGuard { arm: arm_ordinal },
                    });
                }
            }
            (Some(MatchStatementExpressionNode::Missing(missing)), Some(guard)) => {
                if !missing_statement_expression_matches(
                    parsed,
                    slots,
                    arenas.expressions,
                    owner,
                    guard,
                    arm.scope(),
                    HirStmtRecoveryOperandSlot::MatchArmGuard {
                        insertion: missing.range().start(),
                        arm: arm_ordinal,
                    },
                ) {
                    return None;
                }
                recovery.get_or_insert(HirStmtRecoveryIssue::RecoveredChild {
                    role: HirStmtChildRole::MatchArmGuard { arm: arm_ordinal },
                });
            }
            _ => return None,
        }

        match (attached_arm_body, arm.body()) {
            (
                MatchStatementArmBodyNode::Expression(attached_body),
                HirStmtMatchArmBody::Expression(body),
            ) => {
                let attached_body = attached_body.semantic().ok()?;
                if !source_expression_matches(
                    slots,
                    arenas.expressions,
                    *body,
                    &attached_body,
                    arm.scope(),
                ) || arm_scope.locals() != arm.locals()
                {
                    return None;
                }
                if arenas
                    .expressions
                    .resolve_prepared(slots, *body)
                    .is_ok_and(HirExpr::is_poisoned)
                {
                    recovery.get_or_insert(HirStmtRecoveryIssue::RecoveredChild {
                        role: HirStmtChildRole::MatchArmBody { arm: arm_ordinal },
                    });
                }
            }
            (
                MatchStatementArmBodyNode::Missing(missing),
                HirStmtMatchArmBody::Expression(body),
            ) => {
                if !missing_scope_tail_matches(
                    parsed,
                    slots,
                    arenas.expressions,
                    arm.scope(),
                    *body,
                    missing.source_span(),
                ) || arm_scope.locals() != arm.locals()
                {
                    return None;
                }
                recovery.get_or_insert(HirStmtRecoveryIssue::RecoveredChild {
                    role: HirStmtChildRole::MatchArmBody { arm: arm_ordinal },
                });
            }
            (
                MatchStatementArmBodyNode::Statement(attached_statement),
                HirStmtMatchArmBody::Body(body),
            ) => {
                let [statement] = body.ordinary_statements()? else {
                    return None;
                };
                let evidence = statement_matches(
                    parsed,
                    slots,
                    arenas,
                    *statement,
                    &attached_statement,
                    arm.scope(),
                    &mut arm_generations,
                    context,
                )?;
                let expected_locals = arm
                    .locals()
                    .iter()
                    .copied()
                    .chain(evidence.locals.iter().copied())
                    .collect::<Vec<_>>();
                if arm_scope.locals() != expected_locals {
                    return None;
                }
                if evidence.is_poisoned() {
                    recovery.get_or_insert(HirStmtRecoveryIssue::RecoveredChild {
                        role: HirStmtChildRole::MatchArmBody { arm: arm_ordinal },
                    });
                }
            }
            (MatchStatementArmBodyNode::Block(attached_body), HirStmtMatchArmBody::Body(body))
                if context == HirStatementContext::Thread =>
            {
                let attached_body = AttachedRequiredNestedThreadFlowBody::Present(
                    attached_body.thread_flow_body().ok()?,
                );
                if thread_control::nested_match_arm_body_is_poisoned(
                    parsed,
                    slots,
                    arenas,
                    owner,
                    outer_scope,
                    body,
                    &attached_body,
                    arm.locals(),
                    &mut arm_generations,
                )? {
                    recovery.get_or_insert(HirStmtRecoveryIssue::RecoveredChild {
                        role: HirStmtChildRole::MatchArmBody { arm: arm_ordinal },
                    });
                }
            }
            (MatchStatementArmBodyNode::Block(attached_body), HirStmtMatchArmBody::Body(body)) => {
                let statements = body.ordinary_statements()?;
                let evidence = statement_block_contents_match(
                    parsed,
                    slots,
                    arenas,
                    &attached_body,
                    arm.scope(),
                    statements,
                    arm.locals(),
                    context,
                    &mut arm_generations,
                )?;
                if evidence.poisoned {
                    recovery.get_or_insert(HirStmtRecoveryIssue::RecoveredChild {
                        role: HirStmtChildRole::MatchArmBody { arm: arm_ordinal },
                    });
                }
            }
            _ => return None,
        }
    }
    if body_unclosed {
        recovery.get_or_insert(HirStmtRecoveryIssue::Thread(
            HirThreadStmtRecoveryIssue::UnclosedBody {
                role: HirThreadStmtBodyRole::Match,
            },
        ));
    }

    Some(StatementEvidence {
        locals: Box::new([]),
        state: recovery.map_or(HirStmtPoisonState::Clean, HirStmtPoisonState::Poisoned),
    })
}

#[allow(clippy::too_many_arguments)]
#[allow(
    clippy::too_many_lines,
    reason = "one UnsafeLifetime projection proves audit insertion, body scope, statements, and recovery together"
)]
fn unsafe_lifetime_statement_matches(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
    owner: StmtId,
    attached: &StatementNode,
    outer_scope: ScopeId,
    context: HirStatementContext,
    audit: &crate::stmt::HirUnsafeAudit,
    body: &HirUnsafeLifetimeBody,
) -> Option<StatementEvidence> {
    let attached = attached.cast::<UnsafeLifetimeStatementKind>().ok()?;
    let expected_id = match attached.audit_id().ok()? {
        UnsafeAuditIdNode::Reference(reference) => {
            let semantic = reference.semantic().ok()?;
            let ExpressionProjection::EntityReference(reference) = semantic.projection() else {
                return None;
            };
            crate::final_lowering::id_ref_projection::id_ref(reference).ok()?
        }
        UnsafeAuditIdNode::Missing(_) => HirIdRefValue::Recovered(HirIdRefRecovery::new(
            HirIdRefShape::Missing,
            HirIdRefIssue::Missing,
        )),
    };
    if audit.id() != &expected_id {
        return None;
    }
    let mut recovery = expected_id
        .recovery_issue()
        .map(HirStmtRecoveryIssue::InvalidAuditId);

    match (attached.reason().ok()?, audit.reason()) {
        (None, None) => {}
        (Some(UnsafeAuditReasonNode::Expression(reason)), Some(owner)) => {
            let reason = reason.semantic().ok()?;
            if !source_expression_matches(slots, arenas.expressions, owner, &reason, outer_scope) {
                return None;
            }
            if arenas
                .expressions
                .resolve_prepared(slots, owner)
                .is_ok_and(HirExpr::is_poisoned)
            {
                recovery.get_or_insert(HirStmtRecoveryIssue::RecoveredChild {
                    role: HirStmtChildRole::Reason,
                });
            }
        }
        (Some(UnsafeAuditReasonNode::Missing(missing)), Some(reason)) => {
            if !missing_statement_expression_matches(
                parsed,
                slots,
                arenas.expressions,
                owner,
                reason,
                outer_scope,
                HirStmtRecoveryOperandSlot::UnsafeAuditReason {
                    insertion: missing.range().start(),
                },
            ) {
                return None;
            }
            recovery.get_or_insert(HirStmtRecoveryIssue::RecoveredChild {
                role: HirStmtChildRole::Reason,
            });
        }
        _ => return None,
    }

    let expected_safety_doc;
    match (attached.body_or_missing().ok()?, body) {
        (UnsafeAuditBodyNode::Missing(_), HirUnsafeLifetimeBody::Missing) => {
            expected_safety_doc = false;
            recovery.get_or_insert(HirStmtRecoveryIssue::MissingBody);
        }
        (
            UnsafeAuditBodyNode::Block(attached_body),
            HirUnsafeLifetimeBody::Block { scope, statements },
        ) => {
            let mut generations = BTreeMap::new();
            let evidence = statement_block_matches(
                parsed,
                slots,
                arenas,
                owner,
                &attached_body,
                *scope,
                outer_scope,
                statements,
                &[],
                HirScopeKind::Block,
                context,
                &mut generations,
            )?;
            if let Some(ordinal) = evidence.first_poisoned {
                recovery.get_or_insert(HirStmtRecoveryIssue::RecoveredChild {
                    role: HirStmtChildRole::BodyStatement { ordinal },
                });
            }
            if attached_body.close_delimiter().ok()?.range().is_empty() {
                recovery.get_or_insert(HirStmtRecoveryIssue::UnclosedBody);
            }
            expected_safety_doc = !attached.safety_documentation().ok()?.is_empty();
        }
        _ => return None,
    }
    if audit.has_safety_doc() != expected_safety_doc {
        return None;
    }

    Some(StatementEvidence {
        locals: Box::new([]),
        state: recovery.map_or(HirStmtPoisonState::Clean, HirStmtPoisonState::Poisoned),
    })
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "one IfLet projection proves asymmetric bindings, branch scopes, bodies, source roles, and recovery"
)]
fn if_let_statement_matches(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
    owner: StmtId,
    attached: &StatementNode,
    outer_scope: ScopeId,
    context: HirStatementContext,
    statement: &HirIfLetStmt,
) -> Option<StatementEvidence> {
    let attached = attached.cast::<IfStatementKind>().ok()?;
    let IfStatementHeadNode::Let {
        pattern,
        scrutinee,
        guard,
    } = attached.head().ok()?
    else {
        return None;
    };
    let attached_scrutinee = scrutinee.semantic().ok()?;
    if !source_expression_matches(
        slots,
        arenas.expressions,
        statement.scrutinee(),
        &attached_scrutinee,
        outer_scope,
    ) {
        return None;
    }

    let attached_pattern = pattern.semantic().ok()?;
    if !source_owner_matches(
        slots,
        statement.pattern(),
        attached_pattern.id(),
        &HirSourceSite::Span(attached_pattern.whole_source_span()),
    ) {
        return None;
    }
    let pattern_payload = arenas
        .patterns
        .resolve_prepared(slots, statement.pattern())
        .ok()?;
    if pattern_payload.scope() != statement.then_scope() {
        return None;
    }

    let expected_pattern_locals = canonical_pattern_locals(
        slots,
        arenas,
        statement.pattern(),
        statement.pattern(),
        statement.then_scope(),
    )?;
    let expected_pattern_ids = expected_pattern_locals
        .iter()
        .map(|expected| expected.local)
        .collect::<Vec<_>>();
    let mut then_generations = BTreeMap::new();
    let pattern_binding_poisoned = {
        let mut local_validation = BindingLocalValidation::new(
            statement.then_scope(),
            HirPatternBindingPolicy::PatternBinding,
            &mut then_generations,
            slots,
            arenas.patterns,
            arenas.locals,
        );
        if expected_pattern_ids != statement.locals()
            || expected_pattern_ids
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                .len()
                != expected_pattern_ids.len()
            || !binding_locals_match(
                &attached_pattern,
                &expected_pattern_locals,
                &mut local_validation,
            )
            || !expected_pattern_locals.iter().all(|expected| {
                arenas
                    .locals
                    .resolve_prepared(slots, expected.local)
                    .is_ok_and(|local| {
                        local.scope() == statement.then_scope()
                            && local.kind() == HirLocalKind::PatternBinding
                            && local.pattern() == Some(expected.pattern)
                    })
            })
        {
            return None;
        }
        local_validation.is_poisoned()
    };

    let guard_poisoned = match (guard, statement.guard()) {
        (None, None) => false,
        (Some(attached_guard), Some(guard)) => {
            let attached_guard = attached_guard.semantic().ok()?;
            if !source_expression_matches(
                slots,
                arenas.expressions,
                guard,
                &attached_guard,
                statement.then_scope(),
            ) {
                return None;
            }
            arenas
                .expressions
                .resolve_prepared(slots, guard)
                .is_ok_and(HirExpr::is_poisoned)
        }
        _ => return None,
    };

    let then_block = attached.then_branch().ok()?;
    let then_statements = statement.then_body().ordinary_statements()?;
    let then_poisoned = statement_block_matches(
        parsed,
        slots,
        arenas,
        owner,
        &then_block,
        statement.then_scope(),
        outer_scope,
        then_statements,
        statement.locals(),
        HirScopeKind::Conditional,
        context,
        &mut then_generations,
    )?
    .poisoned;

    let else_poisoned = match (attached.else_branch().ok()?, statement.else_branch()) {
        (None, None) => false,
        (Some(IfStatementElseNode::Block(block)), Some(HirConditionalElseBranch::Body(body))) => {
            let mut generations = BTreeMap::new();
            let statements = body.ordinary_statements()?;
            statement_block_matches(
                parsed,
                slots,
                arenas,
                owner,
                &block,
                body.scope(),
                outer_scope,
                statements,
                &[],
                HirScopeKind::Conditional,
                context,
                &mut generations,
            )?
            .poisoned
        }
        (
            Some(IfStatementElseNode::If(nested)),
            Some(HirConditionalElseBranch::ElseIf(nested_owner)),
        ) => {
            let mut generations = BTreeMap::new();
            let evidence = statement_matches(
                parsed,
                slots,
                arenas,
                *nested_owner,
                &nested,
                outer_scope,
                &mut generations,
                context,
            )?;
            if !evidence.locals.is_empty() {
                return None;
            }
            evidence.is_poisoned()
        }
        _ => return None,
    };

    let scrutinee_poisoned = arenas
        .expressions
        .resolve_prepared(slots, statement.scrutinee())
        .is_ok_and(HirExpr::is_poisoned);
    Some(StatementEvidence {
        locals: Box::new([]),
        state: if pattern_payload.is_poisoned() || pattern_binding_poisoned {
            HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::RecoveredChild {
                role: HirStmtChildRole::Pattern,
            })
        } else if scrutinee_poisoned {
            HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::RecoveredChild {
                role: HirStmtChildRole::Scrutinee,
            })
        } else if guard_poisoned {
            HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::RecoveredChild {
                role: HirStmtChildRole::Guard,
            })
        } else if then_poisoned {
            HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::RecoveredChild {
                role: HirStmtChildRole::ThenBranch,
            })
        } else if else_poisoned {
            HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::RecoveredChild {
                role: HirStmtChildRole::ElseBranch,
            })
        } else {
            HirStmtPoisonState::Clean
        },
    })
}

#[allow(clippy::too_many_arguments)]
fn ordinary_if_statement_matches(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
    owner: StmtId,
    attached: &StatementNode,
    outer_scope: ScopeId,
    context: HirStatementContext,
    statement: &HirIfStmt,
) -> Option<StatementEvidence> {
    let attached = attached.cast::<IfStatementKind>().ok()?;
    let IfStatementHeadNode::Condition(attached_condition) = attached.head().ok()? else {
        return None;
    };
    let attached_condition = attached_condition.semantic().ok()?;
    if !source_expression_matches(
        slots,
        arenas.expressions,
        statement.condition(),
        &attached_condition,
        outer_scope,
    ) {
        return None;
    }
    let then_block = attached.then_branch().ok()?;
    let mut then_generations = BTreeMap::new();
    let then_statements = statement.then_body().ordinary_statements()?;
    let then_poisoned = statement_block_matches(
        parsed,
        slots,
        arenas,
        owner,
        &then_block,
        statement.then_scope(),
        outer_scope,
        then_statements,
        &[],
        HirScopeKind::Conditional,
        context,
        &mut then_generations,
    )?
    .poisoned;
    let else_poisoned = match (attached.else_branch().ok()?, statement.else_branch()) {
        (None, None) => false,
        (Some(IfStatementElseNode::Block(block)), Some(HirConditionalElseBranch::Body(body))) => {
            let mut generations = BTreeMap::new();
            let statements = body.ordinary_statements()?;
            statement_block_matches(
                parsed,
                slots,
                arenas,
                owner,
                &block,
                body.scope(),
                outer_scope,
                statements,
                &[],
                HirScopeKind::Conditional,
                context,
                &mut generations,
            )?
            .poisoned
        }
        (
            Some(IfStatementElseNode::If(nested)),
            Some(HirConditionalElseBranch::ElseIf(nested_owner)),
        ) => {
            let mut generations = BTreeMap::new();
            let evidence = statement_matches(
                parsed,
                slots,
                arenas,
                *nested_owner,
                &nested,
                outer_scope,
                &mut generations,
                context,
            )?;
            if !evidence.locals.is_empty() {
                return None;
            }
            evidence.is_poisoned()
        }
        _ => return None,
    };
    Some(StatementEvidence {
        locals: Box::new([]),
        state: if arenas
            .expressions
            .resolve_prepared(slots, statement.condition())
            .is_ok_and(HirExpr::is_poisoned)
        {
            HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::RecoveredChild {
                role: HirStmtChildRole::Condition,
            })
        } else if then_poisoned {
            HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::RecoveredChild {
                role: HirStmtChildRole::ThenBranch,
            })
        } else if else_poisoned {
            HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::RecoveredChild {
                role: HirStmtChildRole::ElseBranch,
            })
        } else {
            HirStmtPoisonState::Clean
        },
    })
}

#[allow(clippy::too_many_arguments)]
struct StatementBlockEvidence {
    poisoned: bool,
    first_poisoned: Option<u32>,
}

#[allow(
    clippy::too_many_arguments,
    reason = "the block validator carries one exact typed owner, scope pair, body, prefix-local set, context, and generation ledger"
)]
fn statement_block_matches(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
    owner: StmtId,
    attached: &AstNode<BlockKind>,
    scope: ScopeId,
    parent_scope: ScopeId,
    body: &[StmtId],
    prefix_locals: &[LocalId],
    expected_kind: HirScopeKind,
    context: HirStatementContext,
    generations: &mut BTreeMap<HirName, LocalGeneration>,
) -> Option<StatementBlockEvidence> {
    if attached.optional_tail().ok()?.is_some()
        || !source_owner_matches(
            slots,
            scope,
            attached.id(),
            &HirSourceSite::Span(attached.source_span()),
        )
    {
        return None;
    }
    let scope_payload = arenas.scopes.resolve_prepared(slots, scope).ok()?;
    let parent = arenas.scopes.resolve_prepared(slots, parent_scope).ok()?;
    if scope_payload.kind() != expected_kind
        || scope_payload.parent() != Some(parent_scope)
        || scope_payload.owner() != &HirScopeOwner::Stmt(owner)
        || !parent.children().contains(&scope)
    {
        return None;
    }
    statement_block_contents_match(
        parsed,
        slots,
        arenas,
        attached,
        scope,
        body,
        prefix_locals,
        context,
        generations,
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "the block-content validator compares one attached body with its exact typed scope, locals, context, and generation ledger"
)]
fn statement_block_contents_match(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
    attached: &AstNode<BlockKind>,
    scope: ScopeId,
    body: &[StmtId],
    prefix_locals: &[LocalId],
    context: HirStatementContext,
    generations: &mut BTreeMap<HirName, LocalGeneration>,
) -> Option<StatementBlockEvidence> {
    if attached.optional_tail().ok()?.is_some() {
        return None;
    }
    let scope_payload = arenas.scopes.resolve_prepared(slots, scope).ok()?;
    let attached_statements = attached.statements().ok()?;
    if attached_statements.len() != body.len() {
        return None;
    }
    let mut expected_locals = prefix_locals.to_vec();
    let mut poisoned = false;
    let mut first_poisoned = None;
    for (ordinal, (statement, attached_statement)) in
        body.iter().copied().zip(&attached_statements).enumerate()
    {
        let evidence = statement_matches(
            parsed,
            slots,
            arenas,
            statement,
            attached_statement,
            scope,
            generations,
            context,
        )?;
        let statement_poisoned = evidence.is_poisoned();
        expected_locals.extend(evidence.locals);
        if statement_poisoned {
            poisoned = true;
            if first_poisoned.is_none() {
                first_poisoned = Some(u32::try_from(ordinal).ok()?);
            }
        }
    }
    if scope_payload.locals() != expected_locals {
        return None;
    }
    Some(StatementBlockEvidence {
        poisoned,
        first_poisoned,
    })
}

#[allow(
    clippy::too_many_arguments,
    reason = "the Let initializer validator compares one typed owner with its exact statement, scope, attachment, and insertion site"
)]
fn let_initializer_matches(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    expressions: &ArenaSnapshot<HirExpr, ExprId>,
    statement: StmtId,
    owner: ExprId,
    attached: Option<LetInitializerNode>,
    scope: ScopeId,
    missing_offset: usize,
) -> bool {
    match attached {
        Some(LetInitializerNode::Expression(attached)) => {
            attached.semantic().is_ok_and(|attached| {
                source_expression_matches(slots, expressions, owner, &attached, scope)
            })
        }
        Some(LetInitializerNode::Missing(missing)) => missing_statement_expression_matches(
            parsed,
            slots,
            expressions,
            statement,
            owner,
            scope,
            HirStmtRecoveryOperandSlot::LetInitializer {
                insertion: missing.range().start(),
            },
        ),
        None => missing_statement_expression_matches(
            parsed,
            slots,
            expressions,
            statement,
            owner,
            scope,
            HirStmtRecoveryOperandSlot::LetInitializer {
                insertion: missing_offset,
            },
        ),
    }
}

pub(super) fn missing_statement_expression_matches(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    expressions: &ArenaSnapshot<HirExpr, ExprId>,
    statement: StmtId,
    owner: ExprId,
    scope: ScopeId,
    slot: HirStmtRecoveryOperandSlot,
) -> bool {
    let Some(ordinal) = slot.ordinal() else {
        return false;
    };
    let Ok(key) = SyntheticKey::try_new(
        SyntheticOwner::Stmt(statement),
        SyntheticRole::RecoveryOperand,
        ordinal,
    ) else {
        return false;
    };
    let Ok(metadata) = slots.resolve_prepared(owner) else {
        return false;
    };
    let Ok(payload) = expressions.resolve_prepared(slots, owner) else {
        return false;
    };
    matches!(metadata.origin(), HirOrigin::Synthetic(actual) if *actual == key)
        && matches!(metadata.source_site(), HirSourceSite::Insertion(point)
                    if point.source_identity() == parsed.document().identity()
                        && point.offset() == slot.insertion())
        && payload.scope() == scope
        && matches!(
            payload.kind(),
            HirExprKind::Error(error)
                if error.issue() == HirGenericExprIssue::TransactionalChildFailure
        )
        && payload.state()
            == &HirPoisonState::Poisoned(HirRecoveryIssue::MissingOperand {
                role: slot.source_role(),
            })
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "the synthetic-tail validator consumes one exact source span for identity comparison"
)]
pub(super) fn missing_scope_tail_matches(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    expressions: &ArenaSnapshot<HirExpr, ExprId>,
    scope: ScopeId,
    owner: ExprId,
    source: arcweft_source::SourceSpan,
) -> bool {
    let Ok(key) = SyntheticKey::try_new(
        SyntheticOwner::Scope(scope),
        SyntheticRole::MissingRequiredTail,
        0,
    ) else {
        return false;
    };
    let Ok(expected_site) = HirSourceSite::from_attached_span(parsed.document(), &source) else {
        return false;
    };
    let Ok(metadata) = slots.resolve_prepared(owner) else {
        return false;
    };
    let Ok(payload) = expressions.resolve_prepared(slots, owner) else {
        return false;
    };
    matches!(metadata.origin(), HirOrigin::Synthetic(actual) if *actual == key)
        && metadata.source_site() == &expected_site
        && payload.scope() == scope
        && matches!(
            (payload.kind(), payload.state()),
            (
                HirExprKind::Error(error),
                HirPoisonState::Poisoned(HirRecoveryIssue::MissingRequiredTail)
            ) if error.issue() == HirGenericExprIssue::TransactionalChildFailure
        )
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "the synthetic-tail validator consumes one exact source span for identity comparison"
)]
fn missing_expr_tail_matches(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    expressions: &ArenaSnapshot<HirExpr, ExprId>,
    parent: ExprId,
    owner: ExprId,
    scope: ScopeId,
    source: arcweft_source::SourceSpan,
) -> bool {
    let Ok(key) = SyntheticKey::try_new(
        SyntheticOwner::Expr(parent),
        SyntheticRole::MissingRequiredTail,
        0,
    ) else {
        return false;
    };
    let Ok(expected_site) = HirSourceSite::from_attached_span(parsed.document(), &source) else {
        return false;
    };
    let Ok(metadata) = slots.resolve_prepared(owner) else {
        return false;
    };
    let Ok(payload) = expressions.resolve_prepared(slots, owner) else {
        return false;
    };
    matches!(metadata.origin(), HirOrigin::Synthetic(actual) if *actual == key)
        && metadata.source_site() == &expected_site
        && payload.scope() == scope
        && matches!(
            (payload.kind(), payload.state()),
            (
                HirExprKind::Error(error),
                HirPoisonState::Poisoned(HirRecoveryIssue::MissingRequiredTail)
            ) if error.issue() == HirGenericExprIssue::TransactionalChildFailure
        )
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "the synthetic-tail validator consumes one exact source span for identity comparison"
)]
pub(super) fn implicit_unit_tail_matches(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    expressions: &ArenaSnapshot<HirExpr, ExprId>,
    parent: ExprId,
    owner: ExprId,
    scope: ScopeId,
    source: arcweft_source::SourceSpan,
) -> bool {
    let Ok(key) = SyntheticKey::try_new(
        SyntheticOwner::Expr(parent),
        SyntheticRole::ImplicitUnitTail,
        0,
    ) else {
        return false;
    };
    let Ok(expected_site) = HirSourceSite::from_attached_span(parsed.document(), &source) else {
        return false;
    };
    let Ok(metadata) = slots.resolve_prepared(owner) else {
        return false;
    };
    let Ok(payload) = expressions.resolve_prepared(slots, owner) else {
        return false;
    };
    matches!(metadata.origin(), HirOrigin::Synthetic(actual) if *actual == key)
        && metadata.source_site() == &expected_site
        && payload.scope() == scope
        && matches!(payload.kind(), HirExprKind::Unit)
        && matches!(payload.state(), HirPoisonState::Clean)
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "the synthetic-tail validator consumes one exact source span for identity comparison"
)]
fn implicit_scope_unit_tail_matches(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    expressions: &ArenaSnapshot<HirExpr, ExprId>,
    scope: ScopeId,
    owner: ExprId,
    source: arcweft_source::SourceSpan,
) -> bool {
    let Ok(key) = SyntheticKey::try_new(
        SyntheticOwner::Scope(scope),
        SyntheticRole::ImplicitUnitTail,
        0,
    ) else {
        return false;
    };
    let Ok(expected_site) = HirSourceSite::from_attached_span(parsed.document(), &source) else {
        return false;
    };
    let Ok(metadata) = slots.resolve_prepared(owner) else {
        return false;
    };
    let Ok(payload) = expressions.resolve_prepared(slots, owner) else {
        return false;
    };
    matches!(metadata.origin(), HirOrigin::Synthetic(actual) if *actual == key)
        && metadata.source_site() == &expected_site
        && payload.scope() == scope
        && matches!(payload.kind(), HirExprKind::Unit)
        && matches!(payload.state(), HirPoisonState::Clean)
}

pub(super) fn source_expression_matches(
    slots: &SlotSnapshot,
    expressions: &ArenaSnapshot<HirExpr, ExprId>,
    owner: ExprId,
    attached: &AttachedExpressionNode,
    scope: ScopeId,
) -> bool {
    source_owner_matches(
        slots,
        owner,
        attached.id(),
        &HirSourceSite::Span(attached.whole_source_span()),
    ) && expressions
        .resolve_prepared(slots, owner)
        .is_ok_and(|payload| payload.scope() == scope)
}

pub(super) fn source_owner_matches<I: HirTypedId>(
    slots: &SlotSnapshot,
    owner: I,
    syntax: SyntaxNodeId,
    site: &HirSourceSite,
) -> bool {
    slots.resolve_prepared(owner).is_ok_and(|metadata| {
        matches!(metadata.origin(), HirOrigin::Source(source) if source.syntax() == syntax)
            && metadata.source_site() == site
    })
}
