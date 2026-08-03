//! Direct final-HIR lowering for attached Thread/Flow control statements.

use arcweft_lang_syntax::attachment::node::{
    AwaitWithStatementKind, ForStatementKind, LoopStatementKind, SelectStatementKind,
    WhileLetStatementKind, WhileStatementKind,
};
use arcweft_lang_syntax::attachment::source_file::AttachedDelimiterState;
use arcweft_lang_syntax::attachment::{
    AttachedRequiredAwaitWithBranchBody, AttachedRequiredNestedThreadFlowBody,
    AttachedSelectBindingName, AttachedSelectBranch, AttachedSelectStatementForm, StatementNode,
};
use arcweft_lang_syntax::expressions::SyntaxAwaitPropagation as AttachedAwaitPropagation;
use arcweft_lang_syntax::grammar::{SyntaxAwaitBranchKind, SyntaxKind};

use crate::expr::{
    HirExpr, HirExprKind, HirExpressionRecoveryIssue, HirForSyntheticExpr, HirPoisonState,
    HirRecoveryIssue, HirThreadIssue,
};
use crate::identity::{
    ExprId, LocalId, ScopeId, StmtId, SyntheticKey, SyntheticOwner, SyntheticRole,
};
use crate::lower::{HirInvariantFailure, HirLowerFailure};
use crate::scope::{HirLocal, HirLocalKind, HirPatternBindingPolicy, HirScopeKind, HirScopeOwner};
use crate::source_index::{HirExprSourceRole, HirInsertionPoint, HirSourceSite};
use crate::stmt::{
    HirAwaitPropagation, HirAwaitWithBranch, HirAwaitWithBranchKind, HirAwaitWithStmt,
    HirContextualStmtBody, HirForStmt, HirLoopStmt, HirSelectBindingLocal, HirSelectBranch,
    HirSelectBranchHead, HirSelectStmt, HirStatementContext, HirStmtChildRole, HirStmtKind,
    HirStmtRecoveryIssue, HirThreadStmtBodyRole, HirThreadStmtChildRole,
    HirThreadStmtRecoveryIssue, HirWhileLetStmt, HirWhileStmt,
};

use super::super::name_projection::{name, name_issue, require_attempted_name_limit};
use super::super::{LocalGenerationLedgerEntry, StagedHirModuleTransaction};
use super::nested_thread_body_recovery;

impl StagedHirModuleTransaction<'_> {
    pub(super) fn lower_attached_thread_control_statement(
        &mut self,
        attached: &StatementNode,
        owner: StmtId,
        outer_scope: ScopeId,
        context: HirStatementContext,
    ) -> Result<(HirStmtKind, Option<HirStmtRecoveryIssue>), HirLowerFailure> {
        match attached.kind() {
            SyntaxKind::LoopStatement => {
                self.require_thread_statement_context(context)?;
                let attached = attached
                    .cast::<LoopStatementKind>()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
                    .semantics()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                let lowered = self.lower_attached_nested_thread_body(
                    attached.body(),
                    HirScopeOwner::Stmt(owner),
                    outer_scope,
                )?;
                let recovery =
                    nested_thread_body_recovery(lowered.recovery, HirThreadStmtBodyRole::Loop)?;
                let body = HirContextualStmtBody::try_thread(lowered.body)
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                let statement = HirLoopStmt::try_new(None, body)
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                Ok((HirStmtKind::Loop(statement), recovery))
            }
            SyntaxKind::WhileStatement => {
                self.require_thread_statement_context(context)?;
                let attached = attached
                    .cast::<WhileStatementKind>()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
                    .semantics()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                let condition = self.lower_required_statement_operand(
                    owner,
                    attached.condition(),
                    outer_scope,
                    HirExprSourceRole::Condition,
                    0,
                )?;
                let condition_poisoned = self.staged_expression_is_poisoned(condition)?;
                let lowered = self.lower_attached_nested_thread_body(
                    attached.body(),
                    HirScopeOwner::Stmt(owner),
                    outer_scope,
                )?;
                let body_recovery =
                    nested_thread_body_recovery(lowered.recovery, HirThreadStmtBodyRole::While)?;
                let body = HirContextualStmtBody::try_thread(lowered.body)
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                let statement = HirWhileStmt::try_new(condition, body)
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                let recovery = condition_poisoned
                    .then_some(thread_child(HirThreadStmtChildRole::Condition))
                    .or(body_recovery);
                Ok((HirStmtKind::While(statement), recovery))
            }
            SyntaxKind::WhileLetStatement => {
                self.require_thread_statement_context(context)?;
                let attached = attached
                    .cast::<WhileLetStatementKind>()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
                    .semantics()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                let scrutinee = self.lower_required_statement_operand(
                    owner,
                    attached.scrutinee(),
                    outer_scope,
                    HirExprSourceRole::Scrutinee,
                    0,
                )?;
                let scrutinee_poisoned = self.staged_expression_is_poisoned(scrutinee)?;
                let prepared = self.prepare_attached_nested_thread_body(
                    attached.body(),
                    HirScopeOwner::Stmt(owner),
                    outer_scope,
                )?;
                let body_scope = prepared.scope();
                let pattern = self.lower_attached_pattern_binding(
                    attached.pattern(),
                    body_scope,
                    HirPatternBindingPolicy::PatternBinding,
                )?;
                let guard = attached
                    .guard()
                    .map(|guard| {
                        self.lower_required_statement_operand(
                            owner,
                            guard,
                            body_scope,
                            HirExprSourceRole::Guard,
                            1,
                        )
                    })
                    .transpose()?;
                let guard_poisoned = guard
                    .map(|guard| self.staged_expression_is_poisoned(guard))
                    .transpose()?
                    .unwrap_or(false);
                let lowered =
                    self.finish_attached_nested_thread_body(prepared, pattern.locals.clone())?;
                let body_recovery =
                    nested_thread_body_recovery(lowered.recovery, HirThreadStmtBodyRole::WhileLet)?;
                let body = HirContextualStmtBody::try_thread(lowered.body)
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                let statement =
                    HirWhileLetStmt::try_new(pattern.owner, scrutinee, guard, pattern.locals, body)
                        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                let recovery = pattern
                    .poisoned
                    .then_some(thread_child(HirThreadStmtChildRole::Pattern))
                    .or_else(|| {
                        scrutinee_poisoned
                            .then_some(thread_child(HirThreadStmtChildRole::Scrutinee))
                    })
                    .or_else(|| {
                        guard_poisoned.then_some(thread_child(HirThreadStmtChildRole::Guard))
                    })
                    .or(body_recovery);
                Ok((HirStmtKind::WhileLet(statement), recovery))
            }
            SyntaxKind::ForStatement => {
                self.require_thread_statement_context(context)?;
                let attached = attached
                    .cast::<ForStatementKind>()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
                    .semantics()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                let source = self.lower_required_statement_operand(
                    owner,
                    attached.source(),
                    outer_scope,
                    HirExprSourceRole::Scrutinee,
                    0,
                )?;
                let source_poisoned = self.staged_expression_is_poisoned(source)?;
                let iterator_site =
                    self.statement_insertion(attached.in_keyword().range().end())?;
                let iterator = self.lower_for_synthetic_expression(
                    owner,
                    outer_scope,
                    SyntheticRole::ForIterator,
                    iterator_site,
                    HirForSyntheticExpr::iterator(source),
                    source_poisoned,
                )?;
                let iterator_poisoned = self.staged_expression_is_poisoned(iterator)?;
                let next_site = match attached.body() {
                    AttachedRequiredNestedThreadFlowBody::Present(body) => {
                        self.statement_insertion(body.open().range().start())?
                    }
                    AttachedRequiredNestedThreadFlowBody::Missing(missing) => {
                        self.statement_insertion(missing.range().start())?
                    }
                };
                let next_value = self.lower_for_synthetic_expression(
                    owner,
                    outer_scope,
                    SyntheticRole::ForNextValue,
                    next_site,
                    HirForSyntheticExpr::next_value(iterator),
                    iterator_poisoned,
                )?;
                let next_poisoned = self.staged_expression_is_poisoned(next_value)?;
                let prepared = self.prepare_attached_nested_thread_body(
                    attached.body(),
                    HirScopeOwner::Stmt(owner),
                    outer_scope,
                )?;
                let body_scope = prepared.scope();
                let pattern = self.lower_attached_pattern_binding(
                    attached.pattern(),
                    body_scope,
                    HirPatternBindingPolicy::PatternBinding,
                )?;
                let lowered =
                    self.finish_attached_nested_thread_body(prepared, pattern.locals.clone())?;
                let body_recovery =
                    nested_thread_body_recovery(lowered.recovery, HirThreadStmtBodyRole::For)?;
                let body = HirContextualStmtBody::try_thread(lowered.body)
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                let statement = HirForStmt::try_new(
                    source,
                    iterator,
                    next_value,
                    pattern.owner,
                    pattern.locals,
                    body,
                )
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                let recovery = pattern
                    .poisoned
                    .then_some(thread_child(HirThreadStmtChildRole::Pattern))
                    .or_else(|| {
                        source_poisoned.then_some(thread_child(HirThreadStmtChildRole::Source))
                    })
                    .or_else(|| {
                        iterator_poisoned.then_some(thread_child(HirThreadStmtChildRole::Iterator))
                    })
                    .or_else(|| {
                        next_poisoned.then_some(thread_child(HirThreadStmtChildRole::NextValue))
                    })
                    .or(body_recovery);
                Ok((HirStmtKind::For(statement), recovery))
            }
            SyntaxKind::SelectStatement => {
                let attached = attached
                    .cast::<SelectStatementKind>()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
                    .semantics()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                match attached.form() {
                    AttachedSelectStatementForm::Operand(operand) => {
                        let expression = self.lower_required_statement_operand(
                            owner,
                            operand,
                            outer_scope,
                            HirExprSourceRole::Operand,
                            0,
                        )?;
                        let recovery = self.staged_expression_is_poisoned(expression)?.then_some(
                            HirStmtRecoveryIssue::RecoveredChild {
                                role: HirStmtChildRole::Expression,
                            },
                        );
                        Ok((
                            HirStmtKind::Select(HirSelectStmt::operand(expression)),
                            recovery,
                        ))
                    }
                    AttachedSelectStatementForm::Branches(block) => {
                        self.require_thread_statement_context(context)?;
                        let select_scope = self.allocate_statement_scope(
                            block.syntax(),
                            owner,
                            outer_scope,
                            HirScopeKind::Block,
                        )?;
                        let mut branches = Vec::with_capacity(block.branches().len());
                        let mut recovery =
                            block
                                .branches()
                                .is_empty()
                                .then_some(HirStmtRecoveryIssue::Thread(
                                    HirThreadStmtRecoveryIssue::EmptySelect,
                                ));
                        for (branch, attached_branch) in block.branches().iter().enumerate() {
                            let branch = u32::try_from(branch)
                                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                            let (lowered, branch_recovery) = self.lower_select_branch(
                                owner,
                                select_scope,
                                branch,
                                attached_branch,
                            )?;
                            if let Some(branch_recovery) = branch_recovery {
                                recovery.get_or_insert(branch_recovery);
                            }
                            branches.push(lowered);
                        }
                        if matches!(block.close_state(), AttachedDelimiterState::Missing(_)) {
                            recovery.get_or_insert(HirStmtRecoveryIssue::Thread(
                                HirThreadStmtRecoveryIssue::UnclosedBody {
                                    role: HirThreadStmtBodyRole::Select,
                                },
                            ));
                        }
                        self.close_scope_members(select_scope, Box::new([]))?;
                        let statement =
                            HirSelectStmt::try_branches(select_scope, branches.into_boxed_slice())
                                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                        Ok((HirStmtKind::Select(statement), recovery))
                    }
                }
            }
            SyntaxKind::AwaitWithStatement => {
                self.require_thread_statement_context(context)?;
                let attached = attached
                    .cast::<AwaitWithStatementKind>()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
                    .semantics()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                let operand = self.lower_required_statement_operand(
                    owner,
                    attached.operand(),
                    outer_scope,
                    HirExprSourceRole::Operand,
                    0,
                )?;
                let mut recovery = self
                    .staged_expression_is_poisoned(operand)?
                    .then_some(thread_child(HirThreadStmtChildRole::AwaitOperand));
                let mut branches = Vec::new();
                match attached.body() {
                    AttachedRequiredAwaitWithBranchBody::Missing(_) => {
                        recovery.get_or_insert(HirStmtRecoveryIssue::Thread(
                            HirThreadStmtRecoveryIssue::MissingBody {
                                role: HirThreadStmtBodyRole::AwaitWith,
                            },
                        ));
                    }
                    AttachedRequiredAwaitWithBranchBody::Present(block) => {
                        if block.branches().is_empty() {
                            recovery.get_or_insert(HirStmtRecoveryIssue::Thread(
                                HirThreadStmtRecoveryIssue::EmptyAwaitWith,
                            ));
                        }
                        for (branch, attached_branch) in block.branches().iter().enumerate() {
                            let branch = u32::try_from(branch)
                                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                            let (lowered, branch_recovery) = self.lower_await_branch(
                                owner,
                                outer_scope,
                                branch,
                                attached_branch,
                            )?;
                            if let Some(branch_recovery) = branch_recovery {
                                recovery.get_or_insert(branch_recovery);
                            }
                            branches.push(lowered);
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
                let propagation = match attached.propagation() {
                    AttachedAwaitPropagation::PreserveResult => HirAwaitPropagation::PreserveResult,
                    AttachedAwaitPropagation::PropagateError => HirAwaitPropagation::PropagateError,
                };
                let statement =
                    HirAwaitWithStmt::try_new(operand, propagation, branches.into_boxed_slice())
                        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                Ok((HirStmtKind::AwaitWith(statement), recovery))
            }
            _ => Err(HirInvariantFailure::InvalidArenaCommit.into()),
        }
    }

    fn lower_select_branch(
        &mut self,
        owner: StmtId,
        select_scope: ScopeId,
        branch: u32,
        attached: &AttachedSelectBranch,
    ) -> Result<(HirSelectBranch, Option<HirStmtRecoveryIssue>), HirLowerFailure> {
        let (head, body, recovery) = match attached {
            AttachedSelectBranch::Bind {
                name: attached_name,
                source,
                propagates_error,
                body,
                ..
            } => {
                let source = self.lower_required_statement_operand(
                    owner,
                    source,
                    select_scope,
                    HirExprSourceRole::Operand,
                    branch,
                )?;
                let source_poisoned = self.staged_expression_is_poisoned(source)?;
                let prepared = self.prepare_attached_nested_thread_body(
                    body,
                    HirScopeOwner::Stmt(owner),
                    select_scope,
                )?;
                let branch_scope = prepared.scope();
                let (binding, local, binding_poisoned) = match attached_name {
                    AttachedSelectBindingName::Missing(_) => {
                        (HirSelectBindingLocal::Missing, None, true)
                    }
                    AttachedSelectBindingName::Authored { syntax, value } => match value {
                        Ok(value) => {
                            let name = name(value)?;
                            let start = syntax.range().start();
                            let generation =
                                self.next_sequential_local_generation(branch_scope, &name, start)?;
                            let reservation = self.arenas.locals().reserve_source(
                                &mut self.slots,
                                syntax.id(),
                                HirSourceSite::Span(syntax.source_span()),
                            )?;
                            let payload = HirLocal::try_new(
                                branch_scope,
                                HirLocalKind::LetBinding,
                                name.clone(),
                                generation,
                                None,
                                None,
                                false,
                                false,
                            )
                            .map_err(|_| HirInvariantFailure::InvalidLocalTimeline)?;
                            let local = self.arenas.locals().finalize(
                                &mut self.slots,
                                reservation,
                                payload,
                            )?;
                            self.local_timelines
                                .entry((branch_scope, name))
                                .or_default()
                                .publish(LocalGenerationLedgerEntry::new(
                                    local, generation, start,
                                ))?;
                            (HirSelectBindingLocal::Resolved(local), Some(local), false)
                        }
                        Err(issue) => {
                            require_attempted_name_limit(issue)?;
                            (
                                HirSelectBindingLocal::Invalid(name_issue(issue)),
                                None,
                                true,
                            )
                        }
                    },
                };
                let prefix: Box<[LocalId]> =
                    local.map_or_else(|| Box::from([]), |local| Box::from([local]));
                let lowered = self.finish_attached_nested_thread_body(prepared, prefix)?;
                let body_recovery = select_branch_body_recovery(lowered.recovery, branch)?;
                let body = HirContextualStmtBody::try_thread(lowered.body)
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                let recovery = binding_poisoned
                    .then_some(thread_child(HirThreadStmtChildRole::SelectBinding {
                        branch,
                    }))
                    .or_else(|| {
                        source_poisoned.then_some(thread_child(
                            HirThreadStmtChildRole::SelectSource { branch },
                        ))
                    })
                    .or(body_recovery);
                (
                    HirSelectBranchHead::Bind {
                        binding,
                        source,
                        propagates_error: *propagates_error,
                    },
                    body,
                    recovery,
                )
            }
            AttachedSelectBranch::Frame { pattern, body, .. }
            | AttachedSelectBranch::Event { pattern, body, .. } => {
                let prepared = self.prepare_attached_nested_thread_body(
                    body,
                    HirScopeOwner::Stmt(owner),
                    select_scope,
                )?;
                let branch_scope = prepared.scope();
                let pattern = self.lower_attached_pattern_binding(
                    pattern,
                    branch_scope,
                    HirPatternBindingPolicy::PatternBinding,
                )?;
                let lowered =
                    self.finish_attached_nested_thread_body(prepared, pattern.locals.clone())?;
                let body_recovery = select_branch_body_recovery(lowered.recovery, branch)?;
                let body = HirContextualStmtBody::try_thread(lowered.body)
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                let recovery = pattern
                    .poisoned
                    .then_some(thread_child(HirThreadStmtChildRole::SelectPattern {
                        branch,
                    }))
                    .or(body_recovery);
                let head = if matches!(attached, AttachedSelectBranch::Frame { .. }) {
                    HirSelectBranchHead::Frame {
                        pattern: pattern.owner,
                        locals: pattern.locals,
                    }
                } else {
                    HirSelectBranchHead::Event {
                        pattern: pattern.owner,
                        locals: pattern.locals,
                    }
                };
                (head, body, recovery)
            }
            AttachedSelectBranch::Recovered { body, .. } => {
                let lowered = self.lower_attached_nested_thread_body(
                    body,
                    HirScopeOwner::Stmt(owner),
                    select_scope,
                )?;
                let body_recovery = select_branch_body_recovery(lowered.recovery, branch)?;
                let body = HirContextualStmtBody::try_thread(lowered.body)
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                (
                    HirSelectBranchHead::Recovered,
                    body,
                    Some(HirStmtRecoveryIssue::Thread(
                        HirThreadStmtRecoveryIssue::RecoveredSelectBranch { ordinal: branch },
                    ))
                    .or(body_recovery),
                )
            }
        };
        let branch = HirSelectBranch::try_new(head, body)
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        Ok((branch, recovery))
    }

    fn lower_await_branch(
        &mut self,
        owner: StmtId,
        outer_scope: ScopeId,
        branch: u32,
        attached: &arcweft_lang_syntax::attachment::AttachedAwaitWithBranch,
    ) -> Result<(HirAwaitWithBranch, Option<HirStmtRecoveryIssue>), HirLowerFailure> {
        let prepared = self.prepare_attached_nested_thread_body(
            attached.body(),
            HirScopeOwner::Stmt(owner),
            outer_scope,
        )?;
        let branch_scope = prepared.scope();
        let (kind, pattern, locals, pattern_poisoned, head_recovery) = match attached.kind() {
            Some(kind) => {
                let pattern = attached
                    .pattern()
                    .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
                let pattern = self.lower_attached_pattern_binding(
                    pattern,
                    branch_scope,
                    HirPatternBindingPolicy::PatternBinding,
                )?;
                let kind = match kind {
                    SyntaxAwaitBranchKind::Pending => HirAwaitWithBranchKind::Pending,
                    SyntaxAwaitBranchKind::Ready => HirAwaitWithBranchKind::Ready,
                    SyntaxAwaitBranchKind::Error => HirAwaitWithBranchKind::Error,
                    SyntaxAwaitBranchKind::Denied => HirAwaitWithBranchKind::Denied,
                };
                (
                    kind,
                    Some(pattern.owner),
                    pattern.locals,
                    pattern.poisoned,
                    false,
                )
            }
            None => {
                if attached.recovery().is_none() || attached.pattern().is_some() {
                    return Err(HirInvariantFailure::InvalidArenaCommit.into());
                }
                (
                    HirAwaitWithBranchKind::Recovered,
                    None,
                    Box::<[LocalId]>::from([]),
                    false,
                    true,
                )
            }
        };
        let lowered = self.finish_attached_nested_thread_body(prepared, locals.clone())?;
        let body_recovery = await_branch_body_recovery(lowered.recovery, branch)?;
        let body = HirContextualStmtBody::try_thread(lowered.body)
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let payload = HirAwaitWithBranch::try_new(kind, pattern, locals, body)
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let recovery = head_recovery
            .then_some(HirStmtRecoveryIssue::Thread(
                HirThreadStmtRecoveryIssue::RecoveredAwaitWithBranch { ordinal: branch },
            ))
            .or_else(|| {
                pattern_poisoned.then_some(thread_child(HirThreadStmtChildRole::AwaitPattern {
                    branch,
                }))
            })
            .or(body_recovery);
        Ok((payload, recovery))
    }

    fn lower_for_synthetic_expression(
        &mut self,
        owner: StmtId,
        scope: ScopeId,
        role: SyntheticRole,
        site: HirSourceSite,
        expression: HirForSyntheticExpr,
        poisoned: bool,
    ) -> Result<ExprId, HirLowerFailure> {
        let key = SyntheticKey::try_new(SyntheticOwner::Stmt(owner), role, 0)
            .map_err(|_| HirInvariantFailure::InvalidSlotCommit)?;
        let reservation =
            self.arenas
                .expressions()
                .reserve_synthetic(&mut self.slots, key, site)?;
        let state = if poisoned {
            HirPoisonState::Poisoned(HirRecoveryIssue::InvalidExpression(
                HirExpressionRecoveryIssue::RecoveredChild {
                    role: HirExprSourceRole::Operand,
                },
            ))
        } else {
            HirPoisonState::Clean
        };
        let payload = HirExpr::try_new(scope, HirExprKind::ForSynthetic(expression), state)
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        self.arenas
            .expressions()
            .finalize(&mut self.slots, reservation, payload)
            .map_err(Into::into)
    }

    fn statement_insertion(&self, offset: usize) -> Result<HirSourceSite, HirLowerFailure> {
        HirInsertionPoint::try_new(self.request.source().document(), offset)
            .map(HirSourceSite::Insertion)
            .map_err(|_| HirInvariantFailure::InvalidSourceSpan.into())
    }
}

const fn thread_child(role: HirThreadStmtChildRole) -> HirStmtRecoveryIssue {
    HirStmtRecoveryIssue::Thread(HirThreadStmtRecoveryIssue::RecoveredChild { role })
}

fn select_branch_body_recovery(
    recovery: Option<HirThreadIssue>,
    branch: u32,
) -> Result<Option<HirStmtRecoveryIssue>, HirLowerFailure> {
    branch_body_recovery(
        recovery,
        HirThreadStmtBodyRole::SelectBranch { ordinal: branch },
        |statement| HirThreadStmtChildRole::SelectBranchStatement { branch, statement },
    )
}

fn await_branch_body_recovery(
    recovery: Option<HirThreadIssue>,
    branch: u32,
) -> Result<Option<HirStmtRecoveryIssue>, HirLowerFailure> {
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
) -> Result<Option<HirStmtRecoveryIssue>, HirLowerFailure> {
    match recovery {
        None => Ok(None),
        Some(HirThreadIssue::MissingBody) => Ok(Some(HirStmtRecoveryIssue::Thread(
            HirThreadStmtRecoveryIssue::MissingBody { role: body_role },
        ))),
        Some(HirThreadIssue::UnclosedBody) => Ok(Some(HirStmtRecoveryIssue::Thread(
            HirThreadStmtRecoveryIssue::UnclosedBody { role: body_role },
        ))),
        Some(HirThreadIssue::RecoveredBodyChild { ordinal }) => {
            Ok(Some(thread_child(child_role(ordinal))))
        }
        Some(
            HirThreadIssue::InvalidName
            | HirThreadIssue::DetachedBorrowedCapture { .. }
            | HirThreadIssue::DetachedEphemeralRegistryAccess,
        ) => Err(HirInvariantFailure::InvalidArenaCommit.into()),
    }
}
