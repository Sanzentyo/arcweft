//! Cross-arena freeze for Thread/Flow control statements.
//!
//! The ordinary block validator remains the sole statement-graph authority.
//! This module only supplies the contextual rules that cannot occur in an
//! ordinary value block: no-tail Thread bodies, branch-local scopes, and the
//! two statement-owned synthetic values introduced by `for`.

use std::collections::{BTreeMap, BTreeSet};

use arcweft_lang_syntax::attachment::node::{
    AwaitWithStatementKind, ForStatementKind, LoopStatementKind, SelectStatementKind,
    WhileLetStatementKind, WhileStatementKind,
};
use arcweft_lang_syntax::attachment::source_file::AttachedDelimiterState;
use arcweft_lang_syntax::attachment::{
    AttachedAwaitWithBranch, AttachedPatternNode, AttachedRequiredAwaitWithBranchBody,
    AttachedRequiredNestedThreadFlowBody, AttachedSelectBindingName, AttachedSelectBranch,
    AttachedSelectStatementForm, AttachedThreadFlowItem, AttachedThreadFlowItemFamily,
    RequiredStatementExpressionNode, StatementNode, SyntaxNodeId,
};
use arcweft_lang_syntax::expressions::SyntaxAwaitPropagation;
use arcweft_lang_syntax::grammar::SyntaxAwaitBranchKind;
use arcweft_lang_syntax::incremental::ParsedSource;
use arcweft_source::SourceSpan;

use super::{
    BlockValidationArenas, StatementEvidence, missing_statement_expression_matches,
    source_expression_matches, source_owner_matches, statement_matches,
};
use crate::expr::{
    HirExpr, HirExprKind, HirExpressionRecoveryIssue, HirForSyntheticExpr, HirPoisonState,
    HirRecoveryIssue, HirThreadBody, HirThreadFlowItem, HirThreadIssue,
};
use crate::final_lowering::name_projection::{name, name_issue};
use crate::identity::{
    ExprId, LocalGeneration, LocalId, ScopeId, StmtId, SyntheticKey, SyntheticOwner, SyntheticRole,
};
use crate::leaf::HirName;
use crate::scope::{HirLocalKind, HirPatternBindingPolicy, HirScopeKind, HirScopeOwner};
use crate::slot::{HirOrigin, SlotSnapshot};
use crate::source_index::control_projection::canonical_pattern_locals;
use crate::source_index::pattern_projection::{BindingLocalValidation, binding_locals_match};
use crate::source_index::{HirExprSourceRole, HirSourceSite};
use crate::stmt::{
    HirAwaitPropagation, HirAwaitWithBranchKind, HirContextualStmtBody, HirForStmt, HirLoopStmt,
    HirSelectBindingLocal, HirSelectBranchHead, HirSelectStmt, HirStatementContext,
    HirStmtChildRole, HirStmtKind, HirStmtPoisonState, HirStmtRecoveryIssue, HirThreadStmtBodyRole,
    HirThreadStmtChildRole, HirThreadStmtRecoveryIssue, HirWhileLetStmt, HirWhileStmt,
};

struct ThreadBodyGraphEvidence {
    recovery: Option<HirThreadIssue>,
}

/// Re-derives a top-level Flow/Thread no-tail body from its accepted attached
/// children. Nested bodies use the same private graph validator below.
#[allow(clippy::too_many_arguments)]
pub(in crate::source_index) fn root_thread_body_graph_matches(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
    body: &HirThreadBody,
    syntax: SyntaxNodeId,
    source: &SourceSpan,
    items: &[AttachedThreadFlowItem],
    close_missing: bool,
    missing: bool,
    scope_owner: &HirScopeOwner,
    parent_scope: ScopeId,
    scope_kind: HirScopeKind,
) -> Option<Option<HirThreadIssue>> {
    let mut generations = BTreeMap::new();
    thread_body_graph_evidence(
        parsed,
        slots,
        arenas,
        body,
        syntax,
        source,
        items,
        close_missing,
        missing,
        scope_owner,
        parent_scope,
        scope_kind,
        &[],
        &mut generations,
    )
    .map(|evidence| evidence.recovery)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn thread_control_statement_evidence(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
    owner: StmtId,
    attached: &StatementNode,
    outer_scope: ScopeId,
    context: HirStatementContext,
    kind: &HirStmtKind,
) -> Option<StatementEvidence> {
    if context != HirStatementContext::Thread {
        return None;
    }
    match kind {
        HirStmtKind::Loop(statement) => loop_evidence(
            parsed,
            slots,
            arenas,
            owner,
            attached,
            outer_scope,
            statement,
        ),
        HirStmtKind::While(statement) => while_evidence(
            parsed,
            slots,
            arenas,
            owner,
            attached,
            outer_scope,
            statement,
        ),
        HirStmtKind::WhileLet(statement) => while_let_evidence(
            parsed,
            slots,
            arenas,
            owner,
            attached,
            outer_scope,
            statement,
        ),
        HirStmtKind::For(statement) => for_evidence(
            parsed,
            slots,
            arenas,
            owner,
            attached,
            outer_scope,
            statement,
        ),
        HirStmtKind::Select(statement @ HirSelectStmt::Branches { .. }) => select_evidence(
            parsed,
            slots,
            arenas,
            owner,
            attached,
            outer_scope,
            statement,
        ),
        HirStmtKind::AwaitWith(statement) => await_with_evidence(
            parsed,
            slots,
            arenas,
            owner,
            attached,
            outer_scope,
            statement,
        ),
        _ => None,
    }
}

fn loop_evidence(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
    owner: StmtId,
    attached: &StatementNode,
    outer_scope: ScopeId,
    statement: &HirLoopStmt,
) -> Option<StatementEvidence> {
    let attached = attached
        .cast::<LoopStatementKind>()
        .ok()?
        .semantics()
        .ok()?;
    if statement.label().is_some() {
        return None;
    }
    let mut generations = BTreeMap::new();
    let body = nested_body_evidence(
        parsed,
        slots,
        arenas,
        owner,
        outer_scope,
        statement.body(),
        attached.body(),
        &[],
        &mut generations,
    )?;
    exact_owned_child_scopes(
        slots,
        arenas,
        outer_scope,
        owner,
        &[statement.body().scope()],
    )?;
    exact_statement_scope_inventory(slots, arenas, owner, &[statement.body().scope()])?;
    exact_statement_synthetic_expressions(slots, arenas, owner, &[])?;
    Some(empty_statement(nested_body_recovery(
        body.recovery,
        HirThreadStmtBodyRole::Loop,
    )))
}

fn while_evidence(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
    owner: StmtId,
    attached: &StatementNode,
    outer_scope: ScopeId,
    statement: &HirWhileStmt,
) -> Option<StatementEvidence> {
    let attached = attached
        .cast::<WhileStatementKind>()
        .ok()?
        .semantics()
        .ok()?;
    let condition_poisoned = required_expression_matches(
        parsed,
        slots,
        arenas,
        owner,
        statement.condition(),
        attached.condition(),
        outer_scope,
        0,
        HirExprSourceRole::Condition,
    )?;
    let mut generations = BTreeMap::new();
    let body = nested_body_evidence(
        parsed,
        slots,
        arenas,
        owner,
        outer_scope,
        statement.body(),
        attached.body(),
        &[],
        &mut generations,
    )?;
    exact_owned_child_scopes(
        slots,
        arenas,
        outer_scope,
        owner,
        &[statement.body().scope()],
    )?;
    exact_statement_scope_inventory(slots, arenas, owner, &[statement.body().scope()])?;
    let expected_synthetics = missing_operand_key(attached.condition(), 0, statement.condition())
        .into_iter()
        .collect::<Vec<_>>();
    exact_statement_synthetic_expressions(slots, arenas, owner, &expected_synthetics)?;
    let recovery = condition_poisoned
        .then_some(thread_child(HirThreadStmtChildRole::Condition))
        .or_else(|| nested_body_recovery(body.recovery, HirThreadStmtBodyRole::While));
    Some(empty_statement(recovery))
}

fn while_let_evidence(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
    owner: StmtId,
    attached: &StatementNode,
    outer_scope: ScopeId,
    statement: &HirWhileLetStmt,
) -> Option<StatementEvidence> {
    let attached = attached
        .cast::<WhileLetStatementKind>()
        .ok()?
        .semantics()
        .ok()?;
    let body_scope = statement.body().scope();
    let scrutinee_poisoned = required_expression_matches(
        parsed,
        slots,
        arenas,
        owner,
        statement.scrutinee(),
        attached.scrutinee(),
        outer_scope,
        0,
        HirExprSourceRole::Scrutinee,
    )?;
    let mut generations = BTreeMap::new();
    let pattern_poisoned = pattern_binding_matches(
        slots,
        arenas,
        attached.pattern(),
        statement.pattern(),
        statement.locals(),
        body_scope,
        &mut generations,
    )?;
    let guard_poisoned = match (attached.guard(), statement.guard()) {
        (None, None) => false,
        (Some(attached), Some(guard)) => required_expression_matches(
            parsed,
            slots,
            arenas,
            owner,
            guard,
            attached,
            body_scope,
            1,
            HirExprSourceRole::Guard,
        )?,
        _ => return None,
    };
    let body = nested_body_evidence(
        parsed,
        slots,
        arenas,
        owner,
        outer_scope,
        statement.body(),
        attached.body(),
        statement.locals(),
        &mut generations,
    )?;
    exact_owned_child_scopes(slots, arenas, outer_scope, owner, &[body_scope])?;
    exact_statement_scope_inventory(slots, arenas, owner, &[body_scope])?;
    let mut expected_synthetics =
        missing_operand_key(attached.scrutinee(), 0, statement.scrutinee())
            .into_iter()
            .collect::<Vec<_>>();
    if let (Some(attached), Some(guard)) = (attached.guard(), statement.guard())
        && let Some(expected) = missing_operand_key(attached, 1, guard)
    {
        expected_synthetics.push(expected);
    }
    exact_statement_synthetic_expressions(slots, arenas, owner, &expected_synthetics)?;
    let recovery = pattern_poisoned
        .then_some(thread_child(HirThreadStmtChildRole::Pattern))
        .or_else(|| scrutinee_poisoned.then_some(thread_child(HirThreadStmtChildRole::Scrutinee)))
        .or_else(|| guard_poisoned.then_some(thread_child(HirThreadStmtChildRole::Guard)))
        .or_else(|| nested_body_recovery(body.recovery, HirThreadStmtBodyRole::WhileLet));
    Some(empty_statement(recovery))
}

fn for_evidence(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
    owner: StmtId,
    attached: &StatementNode,
    outer_scope: ScopeId,
    statement: &HirForStmt,
) -> Option<StatementEvidence> {
    let attached = attached.cast::<ForStatementKind>().ok()?.semantics().ok()?;
    let source_poisoned = required_expression_matches(
        parsed,
        slots,
        arenas,
        owner,
        statement.source(),
        attached.source(),
        outer_scope,
        0,
        HirExprSourceRole::Scrutinee,
    )?;
    let iterator_poisoned = for_synthetic_matches(
        parsed,
        slots,
        arenas,
        owner,
        statement.iterator(),
        outer_scope,
        SyntheticRole::ForIterator,
        attached.in_keyword().range().end(),
        statement.source(),
        true,
        source_poisoned,
    )?;
    let next_offset = match attached.body() {
        AttachedRequiredNestedThreadFlowBody::Present(body) => body.open().range().start(),
        AttachedRequiredNestedThreadFlowBody::Missing(missing) => missing.range().start(),
    };
    let next_poisoned = for_synthetic_matches(
        parsed,
        slots,
        arenas,
        owner,
        statement.next_value(),
        outer_scope,
        SyntheticRole::ForNextValue,
        next_offset,
        statement.iterator(),
        false,
        iterator_poisoned,
    )?;
    let body_scope = statement.body().scope();
    let mut generations = BTreeMap::new();
    let pattern_poisoned = pattern_binding_matches(
        slots,
        arenas,
        attached.pattern(),
        statement.pattern(),
        statement.locals(),
        body_scope,
        &mut generations,
    )?;
    let body = nested_body_evidence(
        parsed,
        slots,
        arenas,
        owner,
        outer_scope,
        statement.body(),
        attached.body(),
        statement.locals(),
        &mut generations,
    )?;
    exact_owned_child_scopes(slots, arenas, outer_scope, owner, &[body_scope])?;
    exact_statement_scope_inventory(slots, arenas, owner, &[body_scope])?;
    let mut expected_synthetics = vec![
        (SyntheticRole::ForIterator, 0, statement.iterator()),
        (SyntheticRole::ForNextValue, 0, statement.next_value()),
    ];
    if let Some(expected) = missing_operand_key(attached.source(), 0, statement.source()) {
        expected_synthetics.push(expected);
    }
    exact_statement_synthetic_expressions(slots, arenas, owner, &expected_synthetics)?;
    let recovery = pattern_poisoned
        .then_some(thread_child(HirThreadStmtChildRole::Pattern))
        .or_else(|| source_poisoned.then_some(thread_child(HirThreadStmtChildRole::Source)))
        .or_else(|| iterator_poisoned.then_some(thread_child(HirThreadStmtChildRole::Iterator)))
        .or_else(|| next_poisoned.then_some(thread_child(HirThreadStmtChildRole::NextValue)))
        .or_else(|| nested_body_recovery(body.recovery, HirThreadStmtBodyRole::For));
    Some(empty_statement(recovery))
}

fn select_evidence(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
    owner: StmtId,
    attached: &StatementNode,
    outer_scope: ScopeId,
    statement: &HirSelectStmt,
) -> Option<StatementEvidence> {
    let attached = attached
        .cast::<SelectStatementKind>()
        .ok()?
        .semantics()
        .ok()?;
    let AttachedSelectStatementForm::Branches(attached) = attached.form() else {
        return None;
    };
    let HirSelectStmt::Branches { scope, branches } = statement else {
        return None;
    };
    if branches.len() != attached.branches().len()
        || !scope_matches_source_block(
            slots,
            arenas,
            *scope,
            owner,
            outer_scope,
            attached.syntax().id(),
            &attached.syntax().source_span(),
        )
        || !arenas
            .scopes
            .resolve_prepared(slots, *scope)
            .is_ok_and(|payload| payload.locals().is_empty())
    {
        return None;
    }
    exact_owned_child_scopes(slots, arenas, outer_scope, owner, &[*scope])?;

    let expected_branch_scopes = branches
        .iter()
        .map(|branch| branch.body().scope())
        .collect::<Vec<_>>();
    exact_owned_child_scopes(slots, arenas, *scope, owner, &expected_branch_scopes)?;
    let mut expected_scopes = Vec::with_capacity(expected_branch_scopes.len() + 1);
    expected_scopes.push(*scope);
    expected_scopes.extend_from_slice(&expected_branch_scopes);
    exact_statement_scope_inventory(slots, arenas, owner, &expected_scopes)?;

    let mut expected_synthetics = Vec::new();
    let mut recovery = branches.is_empty().then_some(HirStmtRecoveryIssue::Thread(
        HirThreadStmtRecoveryIssue::EmptySelect,
    ));
    for (ordinal, (attached_branch, branch)) in
        attached.branches().iter().zip(branches.iter()).enumerate()
    {
        let ordinal = u32::try_from(ordinal).ok()?;
        let branch_recovery = select_branch_evidence(
            parsed,
            slots,
            arenas,
            owner,
            *scope,
            ordinal,
            attached_branch,
            branch,
        )?;
        if let Some(branch_recovery) = branch_recovery {
            recovery.get_or_insert(branch_recovery);
        }
        if let (
            AttachedSelectBranch::Bind { source, .. },
            HirSelectBranchHead::Bind {
                source: semantic_source,
                ..
            },
        ) = (attached_branch, branch.head())
            && let Some(expected) = missing_operand_key(source, ordinal, *semantic_source)
        {
            expected_synthetics.push(expected);
        }
    }
    exact_statement_synthetic_expressions(slots, arenas, owner, &expected_synthetics)?;
    if matches!(attached.close_state(), AttachedDelimiterState::Missing(_)) {
        recovery.get_or_insert(HirStmtRecoveryIssue::Thread(
            HirThreadStmtRecoveryIssue::UnclosedBody {
                role: HirThreadStmtBodyRole::Select,
            },
        ));
    }
    Some(empty_statement(recovery))
}

#[allow(clippy::too_many_arguments)]
fn select_branch_evidence(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
    owner: StmtId,
    select_scope: ScopeId,
    ordinal: u32,
    attached: &AttachedSelectBranch,
    branch: &crate::stmt::HirSelectBranch,
) -> Option<Option<HirStmtRecoveryIssue>> {
    let body_scope = branch.body().scope();
    let mut generations = BTreeMap::new();
    let (prefix, head_recovery) = match (attached, branch.head()) {
        (
            AttachedSelectBranch::Bind {
                name: attached_name,
                source,
                propagates_error,
                ..
            },
            HirSelectBranchHead::Bind {
                binding,
                source: semantic_source,
                propagates_error: semantic_propagation,
            },
        ) if propagates_error == semantic_propagation => {
            let (prefix, binding_poisoned) = select_binding_matches(
                slots,
                arenas,
                attached_name,
                binding,
                body_scope,
                &mut generations,
            )?;
            let source_poisoned = required_expression_matches(
                parsed,
                slots,
                arenas,
                owner,
                *semantic_source,
                source,
                select_scope,
                ordinal,
                HirExprSourceRole::Operand,
            )?;
            (
                prefix,
                binding_poisoned
                    .then_some(thread_child(HirThreadStmtChildRole::SelectBinding {
                        branch: ordinal,
                    }))
                    .or_else(|| {
                        source_poisoned.then_some(thread_child(
                            HirThreadStmtChildRole::SelectSource { branch: ordinal },
                        ))
                    }),
            )
        }
        (
            AttachedSelectBranch::Frame { pattern, .. },
            HirSelectBranchHead::Frame {
                pattern: semantic_pattern,
                locals,
            },
        )
        | (
            AttachedSelectBranch::Event { pattern, .. },
            HirSelectBranchHead::Event {
                pattern: semantic_pattern,
                locals,
            },
        ) => {
            let pattern_poisoned = pattern_binding_matches(
                slots,
                arenas,
                pattern,
                *semantic_pattern,
                locals,
                body_scope,
                &mut generations,
            )?;
            (
                locals.to_vec(),
                pattern_poisoned.then_some(thread_child(HirThreadStmtChildRole::SelectPattern {
                    branch: ordinal,
                })),
            )
        }
        (AttachedSelectBranch::Recovered { .. }, HirSelectBranchHead::Recovered) => (
            Vec::new(),
            Some(HirStmtRecoveryIssue::Thread(
                HirThreadStmtRecoveryIssue::RecoveredSelectBranch { ordinal },
            )),
        ),
        _ => return None,
    };
    let body = nested_body_evidence(
        parsed,
        slots,
        arenas,
        owner,
        select_scope,
        branch.body(),
        attached.body(),
        &prefix,
        &mut generations,
    )?;
    Some(head_recovery.or_else(|| select_branch_body_recovery(body.recovery, ordinal)))
}

fn await_with_evidence(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
    owner: StmtId,
    attached: &StatementNode,
    outer_scope: ScopeId,
    statement: &crate::stmt::HirAwaitWithStmt,
) -> Option<StatementEvidence> {
    let attached = attached
        .cast::<AwaitWithStatementKind>()
        .ok()?
        .semantics()
        .ok()?;
    let operand_poisoned = required_expression_matches(
        parsed,
        slots,
        arenas,
        owner,
        statement.operand(),
        attached.operand(),
        outer_scope,
        0,
        HirExprSourceRole::Operand,
    )?;
    let propagation_matches = matches!(
        (attached.propagation(), statement.propagation()),
        (
            SyntaxAwaitPropagation::PreserveResult,
            HirAwaitPropagation::PreserveResult
        ) | (
            SyntaxAwaitPropagation::PropagateError,
            HirAwaitPropagation::PropagateError
        )
    );
    if !propagation_matches {
        return None;
    }

    let mut recovery =
        operand_poisoned.then_some(thread_child(HirThreadStmtChildRole::AwaitOperand));
    let expected_branch_scopes = statement
        .branches()
        .iter()
        .map(|branch| branch.body().scope())
        .collect::<Vec<_>>();
    exact_owned_child_scopes(slots, arenas, outer_scope, owner, &expected_branch_scopes)?;
    exact_statement_scope_inventory(slots, arenas, owner, &expected_branch_scopes)?;
    let expected_synthetics = missing_operand_key(attached.operand(), 0, statement.operand())
        .into_iter()
        .collect::<Vec<_>>();
    exact_statement_synthetic_expressions(slots, arenas, owner, &expected_synthetics)?;
    match attached.body() {
        AttachedRequiredAwaitWithBranchBody::Missing(_) => {
            if !statement.branches().is_empty() {
                return None;
            }
            recovery.get_or_insert(HirStmtRecoveryIssue::Thread(
                HirThreadStmtRecoveryIssue::MissingBody {
                    role: HirThreadStmtBodyRole::AwaitWith,
                },
            ));
        }
        AttachedRequiredAwaitWithBranchBody::Present(block) => {
            if block.branches().len() != statement.branches().len() {
                return None;
            }
            if block.branches().is_empty() {
                recovery.get_or_insert(HirStmtRecoveryIssue::Thread(
                    HirThreadStmtRecoveryIssue::EmptyAwaitWith,
                ));
            }
            for (ordinal, (attached_branch, branch)) in block
                .branches()
                .iter()
                .zip(statement.branches())
                .enumerate()
            {
                let ordinal = u32::try_from(ordinal).ok()?;
                let branch_recovery = await_branch_evidence(
                    parsed,
                    slots,
                    arenas,
                    owner,
                    outer_scope,
                    ordinal,
                    attached_branch,
                    branch,
                )?;
                if let Some(branch_recovery) = branch_recovery {
                    recovery.get_or_insert(branch_recovery);
                }
            }
            if matches!(block.close_state(), AttachedDelimiterState::Missing(_)) {
                recovery.get_or_insert(HirStmtRecoveryIssue::Thread(
                    HirThreadStmtRecoveryIssue::UnclosedBody {
                        role: HirThreadStmtBodyRole::AwaitWith,
                    },
                ));
            }
        }
    }
    Some(empty_statement(recovery))
}

#[allow(clippy::too_many_arguments)]
fn await_branch_evidence(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
    owner: StmtId,
    outer_scope: ScopeId,
    ordinal: u32,
    attached: &AttachedAwaitWithBranch,
    branch: &crate::stmt::HirAwaitWithBranch,
) -> Option<Option<HirStmtRecoveryIssue>> {
    let body_scope = branch.body().scope();
    let mut generations = BTreeMap::new();
    let (prefix, head_recovery) = match (attached.kind(), branch.kind()) {
        (Some(actual), expected) if await_branch_kind_matches(actual, expected) => {
            let attached_pattern = attached.pattern()?;
            let semantic_pattern = branch.pattern()?;
            let pattern_poisoned = pattern_binding_matches(
                slots,
                arenas,
                attached_pattern,
                semantic_pattern,
                branch.locals(),
                body_scope,
                &mut generations,
            )?;
            (
                branch.locals().to_vec(),
                pattern_poisoned.then_some(thread_child(HirThreadStmtChildRole::AwaitPattern {
                    branch: ordinal,
                })),
            )
        }
        (None, HirAwaitWithBranchKind::Recovered)
            if attached.pattern().is_none() && attached.recovery().is_some() =>
        {
            (
                Vec::new(),
                Some(HirStmtRecoveryIssue::Thread(
                    HirThreadStmtRecoveryIssue::RecoveredAwaitWithBranch { ordinal },
                )),
            )
        }
        _ => return None,
    };
    let body = nested_body_evidence(
        parsed,
        slots,
        arenas,
        owner,
        outer_scope,
        branch.body(),
        attached.body(),
        &prefix,
        &mut generations,
    )?;
    Some(head_recovery.or_else(|| await_branch_body_recovery(body.recovery, ordinal)))
}

const fn await_branch_kind_matches(
    attached: SyntaxAwaitBranchKind,
    semantic: HirAwaitWithBranchKind,
) -> bool {
    matches!(
        (attached, semantic),
        (
            SyntaxAwaitBranchKind::Pending,
            HirAwaitWithBranchKind::Pending
        ) | (SyntaxAwaitBranchKind::Ready, HirAwaitWithBranchKind::Ready)
            | (SyntaxAwaitBranchKind::Error, HirAwaitWithBranchKind::Error)
            | (
                SyntaxAwaitBranchKind::Denied,
                HirAwaitWithBranchKind::Denied
            )
    )
}

#[allow(clippy::too_many_arguments)]
fn nested_body_evidence(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
    owner: StmtId,
    parent_scope: ScopeId,
    semantic: &HirContextualStmtBody,
    attached: &AttachedRequiredNestedThreadFlowBody,
    prefix_locals: &[LocalId],
    generations: &mut BTreeMap<HirName, LocalGeneration>,
) -> Option<ThreadBodyGraphEvidence> {
    let body = semantic.thread_body()?;
    match attached {
        AttachedRequiredNestedThreadFlowBody::Present(attached) => thread_body_graph_evidence(
            parsed,
            slots,
            arenas,
            body,
            attached.syntax().id(),
            &attached.syntax().source_span(),
            attached.items(),
            matches!(attached.close_state(), AttachedDelimiterState::Missing(_)),
            false,
            &HirScopeOwner::Stmt(owner),
            parent_scope,
            HirScopeKind::Block,
            prefix_locals,
            generations,
        ),
        AttachedRequiredNestedThreadFlowBody::Missing(missing) => thread_body_graph_evidence(
            parsed,
            slots,
            arenas,
            body,
            missing.id(),
            &missing.source_span(),
            &[],
            false,
            true,
            &HirScopeOwner::Stmt(owner),
            parent_scope,
            HirScopeKind::Block,
            prefix_locals,
            generations,
        ),
    }
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn thread_body_graph_evidence(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
    body: &HirThreadBody,
    syntax: SyntaxNodeId,
    source: &SourceSpan,
    attached_items: &[AttachedThreadFlowItem],
    close_missing: bool,
    missing: bool,
    scope_owner: &HirScopeOwner,
    parent_scope: ScopeId,
    scope_kind: HirScopeKind,
    prefix_locals: &[LocalId],
    generations: &mut BTreeMap<HirName, LocalGeneration>,
) -> Option<ThreadBodyGraphEvidence> {
    let source_site = HirSourceSite::from_attached_span(parsed.document(), source).ok()?;
    if body.items().len() != attached_items.len()
        || (missing && (!body.items().is_empty() || close_missing))
        || !source_owner_matches(slots, body.scope(), syntax, &source_site)
    {
        return None;
    }
    let scope = arenas.scopes.resolve_prepared(slots, body.scope()).ok()?;
    let parent = arenas.scopes.resolve_prepared(slots, parent_scope).ok()?;
    if scope.kind() != scope_kind
        || scope.parent() != Some(parent_scope)
        || scope.owner() != scope_owner
        || !parent.children().contains(&body.scope())
    {
        return None;
    }
    let mut expected_locals = prefix_locals.to_vec();
    let mut first_recovered = None;
    for (ordinal, (attached, semantic)) in attached_items.iter().zip(body.items()).enumerate() {
        let ordinal = u32::try_from(ordinal).ok()?;
        let evidence = thread_flow_item_evidence(
            parsed,
            slots,
            arenas,
            body.scope(),
            attached,
            semantic,
            generations,
        )?;
        expected_locals.extend_from_slice(&evidence.locals);
        if (evidence.poisoned || attached.has_recovery()) && first_recovered.is_none() {
            first_recovered = Some(ordinal);
        }
    }
    if scope.locals() != expected_locals {
        return None;
    }
    let recovery = if missing {
        Some(HirThreadIssue::MissingBody)
    } else if let Some(ordinal) = first_recovered {
        Some(HirThreadIssue::RecoveredBodyChild { ordinal })
    } else if close_missing {
        Some(HirThreadIssue::UnclosedBody)
    } else {
        None
    };
    Some(ThreadBodyGraphEvidence { recovery })
}

struct ThreadFlowItemEvidence {
    locals: Box<[LocalId]>,
    poisoned: bool,
}

#[allow(clippy::too_many_arguments)]
fn thread_flow_item_evidence(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
    scope: ScopeId,
    attached: &AttachedThreadFlowItem,
    semantic: &HirThreadFlowItem,
    generations: &mut BTreeMap<HirName, LocalGeneration>,
) -> Option<ThreadFlowItemEvidence> {
    if let (
        AttachedThreadFlowItem::DialogueApplication(_),
        HirThreadFlowItem::DialogueApplication(owner),
    ) = (attached, semantic)
    {
        let attached = attached.dialogue_application()?;
        if !matches!(
            arenas
                .expressions
                .resolve_prepared(slots, *owner)
                .ok()?
                .kind(),
            HirExprKind::DialogueContentApplication(_)
        ) {
            return None;
        }
        if !source_expression_matches(slots, arenas.expressions, *owner, &attached, scope) {
            return None;
        }
        return Some(ThreadFlowItemEvidence {
            locals: Box::new([]),
            poisoned: arenas
                .expressions
                .resolve_prepared(slots, *owner)
                .is_ok_and(HirExpr::is_poisoned),
        });
    }

    let attached_statement = attached.statement()?;
    let owner = semantic_statement_owner(slots, arenas, attached.family(), semantic)?;
    let statement = arenas.statements.resolve_prepared(slots, owner).ok()?;
    let exact = match statement.kind() {
        HirStmtKind::Choice { .. }
        | HirStmtKind::If(_)
        | HirStmtKind::IfLet(_)
        | HirStmtKind::Match(_)
        | HirStmtKind::SourceLocale(_)
        | HirStmtKind::Scope(_)
        | HirStmtKind::Include(_)
        | HirStmtKind::Error => None,
        _ => statement_matches(
            parsed,
            slots,
            arenas,
            owner,
            &attached_statement,
            scope,
            generations,
            HirStatementContext::Thread,
        ),
    };
    let evidence = if let Some(evidence) = exact {
        evidence
    } else {
        if !matches!(
            statement.kind(),
            HirStmtKind::Choice { .. }
                | HirStmtKind::If(_)
                | HirStmtKind::IfLet(_)
                | HirStmtKind::Match(_)
                | HirStmtKind::SourceLocale(_)
                | HirStmtKind::Scope(_)
                | HirStmtKind::Include(_)
                | HirStmtKind::Error
        ) || !source_owner_matches(
            slots,
            owner,
            attached_statement.id(),
            &HirSourceSite::Span(attached_statement.source_span()),
        ) || statement.scope() != scope
        {
            return None;
        }
        StatementEvidence {
            locals: statement.kind().post_statement_locals().into(),
            state: statement.state().clone(),
        }
    };
    Some(ThreadFlowItemEvidence {
        poisoned: evidence.is_poisoned(),
        locals: evidence.locals,
    })
}

fn semantic_statement_owner(
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
    family: AttachedThreadFlowItemFamily,
    semantic: &HirThreadFlowItem,
) -> Option<StmtId> {
    let owner = match semantic {
        HirThreadFlowItem::DialogueApplication(_) => return None,
        HirThreadFlowItem::Statement(owner)
        | HirThreadFlowItem::Choice(owner)
        | HirThreadFlowItem::If(owner)
        | HirThreadFlowItem::IfLet(owner)
        | HirThreadFlowItem::Match(owner)
        | HirThreadFlowItem::Loop(owner)
        | HirThreadFlowItem::While(owner)
        | HirThreadFlowItem::WhileLet(owner)
        | HirThreadFlowItem::For(owner)
        | HirThreadFlowItem::Select(owner)
        | HirThreadFlowItem::SourceLocale(owner)
        | HirThreadFlowItem::Scope(owner)
        | HirThreadFlowItem::Include(owner)
        | HirThreadFlowItem::AwaitWith(owner)
        | HirThreadFlowItem::Error(owner) => *owner,
    };
    let statement = arenas.statements.resolve_prepared(slots, owner).ok()?;
    let exact = matches!(
        (family, semantic, statement.kind()),
        (
            AttachedThreadFlowItemFamily::Statement,
            HirThreadFlowItem::Statement(_),
            kind
        ) if ordinary_thread_statement(kind)
    ) || matches!(
        (family, semantic, statement.kind()),
        (
            AttachedThreadFlowItemFamily::Choice,
            HirThreadFlowItem::Choice(_),
            HirStmtKind::Choice { .. }
        ) | (
            AttachedThreadFlowItemFamily::If,
            HirThreadFlowItem::If(_),
            HirStmtKind::If(_)
        ) | (
            AttachedThreadFlowItemFamily::IfLet,
            HirThreadFlowItem::IfLet(_),
            HirStmtKind::IfLet(_)
        ) | (
            AttachedThreadFlowItemFamily::Match,
            HirThreadFlowItem::Match(_),
            HirStmtKind::Match(_)
        ) | (
            AttachedThreadFlowItemFamily::Loop,
            HirThreadFlowItem::Loop(_),
            HirStmtKind::Loop(_)
        ) | (
            AttachedThreadFlowItemFamily::While,
            HirThreadFlowItem::While(_),
            HirStmtKind::While(_)
        ) | (
            AttachedThreadFlowItemFamily::WhileLet,
            HirThreadFlowItem::WhileLet(_),
            HirStmtKind::WhileLet(_)
        ) | (
            AttachedThreadFlowItemFamily::For,
            HirThreadFlowItem::For(_),
            HirStmtKind::For(_)
        ) | (
            AttachedThreadFlowItemFamily::Select,
            HirThreadFlowItem::Select(_),
            HirStmtKind::Select(_)
        ) | (
            AttachedThreadFlowItemFamily::SourceLocale,
            HirThreadFlowItem::SourceLocale(_),
            HirStmtKind::SourceLocale(_)
        ) | (
            AttachedThreadFlowItemFamily::Scope,
            HirThreadFlowItem::Scope(_),
            HirStmtKind::Scope(_)
        ) | (
            AttachedThreadFlowItemFamily::Include,
            HirThreadFlowItem::Include(_),
            HirStmtKind::Include(_)
        ) | (
            AttachedThreadFlowItemFamily::AwaitWith,
            HirThreadFlowItem::AwaitWith(_),
            HirStmtKind::AwaitWith(_)
        ) | (
            AttachedThreadFlowItemFamily::Error,
            HirThreadFlowItem::Error(_),
            HirStmtKind::Error
        )
    );
    exact.then_some(owner)
}

fn ordinary_thread_statement(kind: &HirStmtKind) -> bool {
    !matches!(
        kind,
        HirStmtKind::Choice { .. }
            | HirStmtKind::If(_)
            | HirStmtKind::IfLet(_)
            | HirStmtKind::Match(_)
            | HirStmtKind::Loop(_)
            | HirStmtKind::While(_)
            | HirStmtKind::WhileLet(_)
            | HirStmtKind::For(_)
            | HirStmtKind::Select(_)
            | HirStmtKind::SourceLocale(_)
            | HirStmtKind::Scope(_)
            | HirStmtKind::Include(_)
            | HirStmtKind::AwaitWith(_)
            | HirStmtKind::Error
    )
}

#[allow(clippy::too_many_arguments)]
fn required_expression_matches(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
    statement: StmtId,
    owner: ExprId,
    attached: &RequiredStatementExpressionNode,
    scope: ScopeId,
    ordinal: u32,
    role: HirExprSourceRole,
) -> Option<bool> {
    match attached {
        RequiredStatementExpressionNode::Expression(attached) => {
            let attached = attached.semantic().ok()?;
            source_expression_matches(slots, arenas.expressions, owner, &attached, scope).then(
                || {
                    arenas
                        .expressions
                        .resolve_prepared(slots, owner)
                        .is_ok_and(HirExpr::is_poisoned)
                },
            )
        }
        RequiredStatementExpressionNode::Missing(missing) => missing_statement_expression_matches(
            parsed,
            slots,
            arenas.expressions,
            statement,
            owner,
            scope,
            missing.range().start(),
            ordinal,
            role,
        )
        .then_some(true),
    }
}

#[allow(clippy::too_many_arguments)]
fn for_synthetic_matches(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
    statement: StmtId,
    owner: ExprId,
    scope: ScopeId,
    role: SyntheticRole,
    insertion: usize,
    input: ExprId,
    iterator: bool,
    poisoned: bool,
) -> Option<bool> {
    let key = SyntheticKey::try_new(SyntheticOwner::Stmt(statement), role, 0).ok()?;
    let metadata = slots.resolve_prepared(owner).ok()?;
    let payload = arenas.expressions.resolve_prepared(slots, owner).ok()?;
    let expected_state = if poisoned {
        HirPoisonState::Poisoned(HirRecoveryIssue::InvalidExpression(
            HirExpressionRecoveryIssue::RecoveredChild {
                role: HirExprSourceRole::Operand,
            },
        ))
    } else {
        HirPoisonState::Clean
    };
    let payload_matches = match payload.kind() {
        HirExprKind::ForSynthetic(HirForSyntheticExpr::Iterator { source }) if iterator => {
            *source == input
        }
        HirExprKind::ForSynthetic(HirForSyntheticExpr::NextValue { iterator: actual })
            if !iterator =>
        {
            *actual == input
        }
        _ => false,
    };
    (matches!(metadata.origin(), HirOrigin::Synthetic(actual) if *actual == key)
        && matches!(metadata.source_site(), HirSourceSite::Insertion(point)
            if point.source_identity() == parsed.document().identity()
                && point.offset() == insertion)
        && payload.scope() == scope
        && payload_matches
        && payload.state() == &expected_state)
        .then_some(poisoned)
}

fn pattern_binding_matches(
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
    attached: &AttachedPatternNode,
    owner: crate::identity::PatternId,
    locals: &[LocalId],
    scope: ScopeId,
    generations: &mut BTreeMap<HirName, LocalGeneration>,
) -> Option<bool> {
    if !source_owner_matches(
        slots,
        owner,
        attached.id(),
        &HirSourceSite::Span(attached.whole_source_span()),
    ) {
        return None;
    }
    let pattern = arenas.patterns.resolve_prepared(slots, owner).ok()?;
    if pattern.scope() != scope {
        return None;
    }
    let expected = canonical_pattern_locals(slots, arenas, owner, owner, scope)?;
    let expected_ids = expected.iter().map(|entry| entry.local).collect::<Vec<_>>();
    let mut validation = BindingLocalValidation::new(
        scope,
        HirPatternBindingPolicy::PatternBinding,
        generations,
        slots,
        arenas.patterns,
        arenas.locals,
    );
    if expected_ids != locals
        || expected_ids.iter().copied().collect::<BTreeSet<_>>().len() != expected_ids.len()
        || !binding_locals_match(attached, &expected, &mut validation)
        || expected.iter().any(|entry| {
            !arenas
                .locals
                .resolve_prepared(slots, entry.local)
                .is_ok_and(|local| {
                    local.scope() == scope
                        && local.kind() == HirLocalKind::PatternBinding
                        && local.pattern() == Some(entry.pattern)
                })
        })
    {
        return None;
    }
    Some(pattern.is_poisoned() || validation.is_poisoned())
}

fn select_binding_matches(
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
    attached: &AttachedSelectBindingName,
    semantic: &HirSelectBindingLocal,
    scope: ScopeId,
    generations: &mut BTreeMap<HirName, LocalGeneration>,
) -> Option<(Vec<LocalId>, bool)> {
    match (attached, semantic) {
        (AttachedSelectBindingName::Missing(_), HirSelectBindingLocal::Missing) => {
            Some((Vec::new(), true))
        }
        (
            AttachedSelectBindingName::Authored {
                syntax,
                value: Ok(value),
            },
            HirSelectBindingLocal::Resolved(owner),
        ) => {
            let expected_name = name(value).ok()?;
            let local = arenas.locals.resolve_prepared(slots, *owner).ok()?;
            let expected_generation = generations
                .get(&expected_name)
                .copied()
                .map_or(Some(LocalGeneration::FIRST), LocalGeneration::checked_next)?;
            if !source_owner_matches(
                slots,
                *owner,
                syntax.id(),
                &HirSourceSite::Span(syntax.source_span()),
            ) || local.scope() != scope
                || local.kind() != HirLocalKind::LetBinding
                || local.name() != &expected_name
                || local.generation() != expected_generation
                || local.pattern().is_some()
                || local.annotation().is_some()
            {
                return None;
            }
            generations.insert(expected_name, expected_generation);
            Some((vec![*owner], false))
        }
        (
            AttachedSelectBindingName::Authored {
                value: Err(issue), ..
            },
            HirSelectBindingLocal::Invalid(actual),
        ) if *actual == name_issue(issue) => Some((Vec::new(), true)),
        _ => None,
    }
}

fn scope_matches_source_block(
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
    scope: ScopeId,
    owner: StmtId,
    parent: ScopeId,
    syntax: SyntaxNodeId,
    source: &SourceSpan,
) -> bool {
    source_owner_matches(slots, scope, syntax, &HirSourceSite::Span(source.clone()))
        && arenas
            .scopes
            .resolve_prepared(slots, scope)
            .is_ok_and(|payload| {
                payload.kind() == HirScopeKind::Block
                    && payload.parent() == Some(parent)
                    && payload.owner() == &HirScopeOwner::Stmt(owner)
                    && arenas
                        .scopes
                        .resolve_prepared(slots, parent)
                        .is_ok_and(|parent| parent.children().contains(&scope))
            })
}

fn exact_owned_child_scopes(
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
    parent: ScopeId,
    owner: StmtId,
    expected: &[ScopeId],
) -> Option<()> {
    let parent_id = parent;
    let parent = arenas.scopes.resolve_prepared(slots, parent_id).ok()?;
    let actual = parent
        .children()
        .iter()
        .copied()
        .filter(|child| {
            arenas
                .scopes
                .resolve_prepared(slots, *child)
                .is_ok_and(|scope| {
                    scope.parent() == Some(parent_id)
                        && scope.owner() == &HirScopeOwner::Stmt(owner)
                })
        })
        .collect::<Vec<_>>();
    (actual == expected).then_some(())
}

fn exact_statement_scope_inventory(
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
    owner: StmtId,
    expected: &[ScopeId],
) -> Option<()> {
    let actual = arenas
        .scopes
        .try_iter_prepared(slots)
        .ok()?
        .filter_map(|(scope, payload)| {
            (payload.owner() == &HirScopeOwner::Stmt(owner)).then_some(scope)
        })
        .collect::<BTreeSet<_>>();
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    (actual == expected).then_some(())
}

fn missing_operand_key(
    attached: &RequiredStatementExpressionNode,
    ordinal: u32,
    owner: ExprId,
) -> Option<(SyntheticRole, u32, ExprId)> {
    matches!(attached, RequiredStatementExpressionNode::Missing(_)).then_some((
        SyntheticRole::RecoveryOperand,
        ordinal,
        owner,
    ))
}

fn exact_statement_synthetic_expressions(
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
    owner: StmtId,
    expected: &[(SyntheticRole, u32, ExprId)],
) -> Option<()> {
    let mut expected_by_key = BTreeMap::new();
    for (role, ordinal, expression) in expected {
        let key = SyntheticKey::try_new(SyntheticOwner::Stmt(owner), *role, *ordinal).ok()?;
        if expected_by_key.insert(key, *expression).is_some() {
            return None;
        }
    }

    let mut actual_by_key = BTreeMap::new();
    for (expression, _) in arenas.expressions.try_iter_prepared(slots).ok()? {
        let metadata = slots.resolve_prepared(expression).ok()?;
        let HirOrigin::Synthetic(key) = metadata.origin() else {
            continue;
        };
        if key.owner() != SyntheticOwner::Stmt(owner) {
            continue;
        }
        if actual_by_key.insert(*key, expression).is_some() {
            return None;
        }
    }

    (actual_by_key == expected_by_key).then_some(())
}

fn empty_statement(recovery: Option<HirStmtRecoveryIssue>) -> StatementEvidence {
    StatementEvidence {
        locals: Box::new([]),
        state: recovery.map_or(HirStmtPoisonState::Clean, HirStmtPoisonState::Poisoned),
    }
}

const fn thread_child(role: HirThreadStmtChildRole) -> HirStmtRecoveryIssue {
    HirStmtRecoveryIssue::Thread(HirThreadStmtRecoveryIssue::RecoveredChild { role })
}

fn nested_body_recovery(
    recovery: Option<HirThreadIssue>,
    role: HirThreadStmtBodyRole,
) -> Option<HirStmtRecoveryIssue> {
    match recovery {
        None => None,
        Some(HirThreadIssue::MissingBody) => Some(HirStmtRecoveryIssue::Thread(
            HirThreadStmtRecoveryIssue::MissingBody { role },
        )),
        Some(HirThreadIssue::UnclosedBody) => Some(HirStmtRecoveryIssue::Thread(
            HirThreadStmtRecoveryIssue::UnclosedBody { role },
        )),
        Some(HirThreadIssue::RecoveredBodyChild { ordinal }) => {
            Some(HirStmtRecoveryIssue::RecoveredChild {
                role: HirStmtChildRole::BodyStatement { ordinal },
            })
        }
        Some(
            HirThreadIssue::InvalidName
            | HirThreadIssue::DetachedBorrowedCapture { .. }
            | HirThreadIssue::DetachedEphemeralRegistryAccess,
        ) => None,
    }
}

fn select_branch_body_recovery(
    recovery: Option<HirThreadIssue>,
    branch: u32,
) -> Option<HirStmtRecoveryIssue> {
    branch_body_recovery(
        recovery,
        HirThreadStmtBodyRole::SelectBranch { ordinal: branch },
        |statement| HirThreadStmtChildRole::SelectBranchStatement { branch, statement },
    )
}

fn await_branch_body_recovery(
    recovery: Option<HirThreadIssue>,
    branch: u32,
) -> Option<HirStmtRecoveryIssue> {
    branch_body_recovery(
        recovery,
        HirThreadStmtBodyRole::AwaitBranch { ordinal: branch },
        |statement| HirThreadStmtChildRole::AwaitBranchStatement { branch, statement },
    )
}

fn branch_body_recovery(
    recovery: Option<HirThreadIssue>,
    body_role: HirThreadStmtBodyRole,
    child_role: impl FnOnce(u32) -> HirThreadStmtChildRole,
) -> Option<HirStmtRecoveryIssue> {
    match recovery {
        None => None,
        Some(HirThreadIssue::MissingBody) => Some(HirStmtRecoveryIssue::Thread(
            HirThreadStmtRecoveryIssue::MissingBody { role: body_role },
        )),
        Some(HirThreadIssue::UnclosedBody) => Some(HirStmtRecoveryIssue::Thread(
            HirThreadStmtRecoveryIssue::UnclosedBody { role: body_role },
        )),
        Some(HirThreadIssue::RecoveredBodyChild { ordinal }) => {
            Some(thread_child(child_role(ordinal)))
        }
        Some(
            HirThreadIssue::InvalidName
            | HirThreadIssue::DetachedBorrowedCapture { .. }
            | HirThreadIssue::DetachedEphemeralRegistryAccess,
        ) => None,
    }
}
