//! Continuation typing through the sole HIR evaluation/transfer algebra.
//! This bounded source inventory query publishes no execution edge or proof.
//! The same Unit/Never continuation algebra is used by the ownership CFG fold.
use super::{
    Analyzer, AnalyzerExpressionError, ExprId, FinalSemanticAnalysisError, HirExprKind, HirModule,
    PreparedExpressionFact,
};
use crate::final_analysis::completion::NormalContinuation;
use arcweft_lang_hir::body_edges::HirBodyChild;
use arcweft_lang_hir::expr::{
    HirComputationBlockKind, HirExpressionChildOwnership, HirExpressionOwnedBodyRole,
    HirExpressionOwnedChild,
};
use arcweft_lang_hir::identity::StmtId;
use arcweft_lang_hir::project::{
    HirControlTransferTarget, HirExpressionEvaluationEdge, HirLoopTargetFamily, HirReturnContext,
    HirSelectedCallExpressionDisposition, HirSelectedExpressionInventoryError,
    HirSelectedSelectTargetDisposition, HirSemanticBodyOwner,
};
use arcweft_lang_hir::stmt::{
    HirConditionalElseBranch, HirContextualStmtBody, HirStatementBodyRole, HirStmtEvaluationPlan,
    HirStmtMatchArmBody, HirStmtSelectEvaluationPlan, HirStmtValuePlanKind,
};
use std::collections::BTreeSet;

pub(super) struct BodyCompletion {
    normal: NormalContinuation,
    returns: BTreeSet<StmtId>,
    // Exact source rows, not copied targets. Only a matching receiving boundary
    // may consume a reachable transfer. Dead operands are still type checked.
    escaping: BTreeSet<StmtId>,
}
impl BodyCompletion {
    fn unit() -> Self {
        Self {
            normal: NormalContinuation::Unit,
            returns: BTreeSet::new(),
            escaping: BTreeSet::new(),
        }
    }
    fn never() -> Self {
        Self {
            normal: NormalContinuation::Never,
            returns: BTreeSet::new(),
            escaping: BTreeSet::new(),
        }
    }
    pub(super) const fn continues(&self) -> bool {
        self.normal.is_unit()
    }
    pub(super) fn returns(&self) -> &BTreeSet<StmtId> {
        &self.returns
    }
    fn sequence(&mut self, child: Self) {
        if self.continues() {
            self.escaping.extend(child.escaping);
        }
        self.normal = self.normal.then(child.normal);
        self.returns.extend(child.returns);
    }
    fn branch(&mut self, child: Self) {
        self.normal = self.normal.join(child.normal);
        self.returns.extend(child.returns);
        self.escaping.extend(child.escaping);
    }
    fn evidence(&mut self, child: Self) {
        self.returns.extend(child.returns);
        self.escaping.extend(child.escaping);
    }
}
#[derive(Clone, Copy)]
struct CompletionQuery<'a> {
    callable: Option<ExprId>,
    // The checker owns this exact prepared result before fact publication.
    // Recursive child reads still require their own existing prepared facts.
    current_expression: Option<(ExprId, &'a PreparedExpressionFact)>,
}
impl Analyzer<'_, '_, '_> {
    pub(super) fn block_statements_completion(
        &self,
        module: &HirModule,
        statements: &[StmtId],
        callable: Option<ExprId>,
    ) -> Result<BodyCompletion, AnalyzerExpressionError> {
        self.block_statements_completion_in(
            module,
            statements,
            CompletionQuery {
                callable,
                current_expression: None,
            },
        )
    }
    pub(super) fn expression_body_completion(
        &self,
        module: &HirModule,
        body: ExprId,
        callable: ExprId,
        body_fact: &PreparedExpressionFact,
    ) -> Result<BodyCompletion, AnalyzerExpressionError> {
        self.expression_body_completion_in(
            module,
            body,
            CompletionQuery {
                callable: Some(callable),
                current_expression: Some((body, body_fact)),
            },
        )
    }

    /// Borrows the same exact live mapper/callee inventory as final publication.
    /// A raw owning callee edge may be a static namespace/type spelling that
    /// the selected application intentionally does not publish as a value.
    fn completion_call_disposition(
        &self,
        owner: ExprId,
        query: CompletionQuery<'_>,
    ) -> Result<HirSelectedCallExpressionDisposition, AnalyzerExpressionError> {
        let site = crate::callable::CheckedCallSite::HirCall(owner);
        let checked_site = self
            .completion_expression_fact(owner, query)?
            .checked_call_site(owner);
        let graph = self
            .facts
            .prepared_calls()
            .map_err(AnalyzerExpressionError::fact)?;
        if let Some(disposition) = graph.project_site_payload(
            site,
            |prefix| {
                prefix
                    .selected_expression_inventory()
                    .map(HirSelectedCallExpressionDisposition::Callable)
            },
            |unselected| Ok(unselected.source_expression_disposition()),
        ) {
            if checked_site != Some(site) {
                return Err(AnalyzerExpressionError::fatal(
                    FinalSemanticAnalysisError::CallSeal(
                        crate::final_analysis::FinalCallSealFailure::new(
                            crate::final_analysis::FinalCallSealLocation::Site(site),
                            crate::callable::CallConstraintInvariant::PreparedCallSiteMismatch,
                        ),
                    ),
                ));
            }
            return disposition.map_err(|failure| {
                AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::CallSeal(
                    crate::final_analysis::FinalCallSealFailure::new(
                        crate::final_analysis::FinalCallSealLocation::Site(site),
                        failure,
                    ),
                ))
            });
        }
        // Only an exact checked structural resolution grants ordinary Call
        // traversal. Missing or stale callable state remains fatal.
        if checked_site.is_none() {
            return Ok(HirSelectedCallExpressionDisposition::Structural);
        }
        Err(AnalyzerExpressionError::fatal(
            FinalSemanticAnalysisError::CallSeal(crate::final_analysis::FinalCallSealFailure::new(
                crate::final_analysis::FinalCallSealLocation::Site(site),
                crate::callable::CallConstraintInvariant::MissingOrStalePreparedNode,
            )),
        ))
    }

    fn completion_expression_fact<'a>(
        &'a self,
        owner: ExprId,
        query: CompletionQuery<'a>,
    ) -> Result<&'a PreparedExpressionFact, AnalyzerExpressionError> {
        query
            .current_expression
            .filter(|(current, _)| *current == owner)
            .map(|(_, fact)| fact)
            .or_else(|| self.facts.expressions().get(&owner))
            .ok_or_else(|| {
                AnalyzerExpressionError::fatal(
                    FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner },
                )
            })
    }

    /// Reads the same selected owning edges as final semantic publication.
    /// Alternative candidates and static qualifiers are omitted by HIR's
    /// exact checked projection, never by absent child facts or Unit defaults.
    fn completion_expression_edges(
        &self,
        module: &HirModule,
        owner: ExprId,
        kind: &HirExprKind,
        query: CompletionQuery<'_>,
    ) -> Result<Box<[HirExpressionEvaluationEdge]>, AnalyzerExpressionError> {
        let fact = self.completion_expression_fact(owner, query)?;
        let call = if matches!(kind, HirExprKind::Call(_)) {
            Some(self.completion_call_disposition(owner, query)?)
        } else {
            None
        };
        self.topology
            .selected_expression_child_edges(
                module,
                owner,
                fact.selected_postfix_candidate(),
                call,
                fact.is_variant_expression()
                    .then_some(HirSelectedSelectTargetDisposition::StaticVariantQualifier),
            )
            .map_err(|error| {
                AnalyzerExpressionError::fatal(match error {
                    HirSelectedExpressionInventoryError::TopologyMismatch
                    | HirSelectedExpressionInventoryError::UnknownModule { .. }
                    | HirSelectedExpressionInventoryError::UnresolvedExpression { .. } => {
                        FinalSemanticAnalysisError::InvalidOwner
                    }
                    HirSelectedExpressionInventoryError::RecoveredOwner { .. } => {
                        FinalSemanticAnalysisError::RecoveredOwner
                    }
                    HirSelectedExpressionInventoryError::MissingSelectedCallEdges {
                        expression,
                    } => FinalSemanticAnalysisError::CallSeal(
                        crate::final_analysis::FinalCallSealFailure::new(
                            crate::final_analysis::FinalCallSealLocation::Site(
                                crate::callable::CheckedCallSite::HirCall(expression),
                            ),
                            crate::callable::CallConstraintInvariant::MissingOrStalePreparedNode,
                        ),
                    ),
                    HirSelectedExpressionInventoryError::InvalidSelectedCallCallee {
                        expression,
                        ..
                    } => FinalSemanticAnalysisError::CallSeal(
                        crate::final_analysis::FinalCallSealFailure::new(
                            crate::final_analysis::FinalCallSealLocation::Site(
                                crate::callable::CheckedCallSite::HirCall(expression),
                            ),
                            crate::callable::CallConstraintInvariant::PreparedCallSiteMismatch,
                        ),
                    ),
                    HirSelectedExpressionInventoryError::InvalidSelectedCallArguments {
                        expression,
                    } => FinalSemanticAnalysisError::CallSeal(
                        crate::final_analysis::FinalCallSealFailure::new(
                            crate::final_analysis::FinalCallSealLocation::Site(
                                crate::callable::CheckedCallSite::HirCall(expression),
                            ),
                            crate::callable::CallConstraintInvariant::MalformedMapperSeal,
                        ),
                    ),
                    _ => FinalSemanticAnalysisError::WrongPayloadFamily,
                })
            })
    }

    fn completion_transfer_target(
        &self,
        owner: StmtId,
    ) -> Result<&HirControlTransferTarget, AnalyzerExpressionError> {
        self.topology
            .control_transfer_row(owner)
            .map_err(|_| AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner))?
            .target()
            .map_err(|error| {
                AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::ControlTransfer(*error))
            })
    }
    fn receive_completion_transfers(
        &self,
        module: &HirModule,
        completion: &mut BodyCompletion,
        receives: impl Fn(&HirControlTransferTarget) -> bool,
    ) -> Result<bool, AnalyzerExpressionError> {
        let mut resumes = false;
        let mut received = Vec::new();
        for statement in &completion.escaping {
            let kind = module
                .resolve_stmt(*statement)
                .map_err(|_| {
                    AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner)
                })?
                .kind()
                .evaluation_plan();
            // Goto has its own Flow target owner; no expression/loop boundary
            // consumes it and there is no HIR Return/Output/Loop row to invent.
            if matches!(
                kind,
                HirStmtEvaluationPlan::Value {
                    kind: HirStmtValuePlanKind::Goto,
                    ..
                }
            ) {
                continue;
            }
            if receives(self.completion_transfer_target(*statement)?) {
                received.push(*statement);
                resumes |= !matches!(kind, HirStmtEvaluationPlan::Continue { .. });
            }
        }
        for statement in received {
            completion.escaping.remove(&statement);
        }
        Ok(resumes)
    }
    fn receive_loop_completion(
        &self,
        module: &HirModule,
        completion: &mut BodyCompletion,
        family: HirLoopTargetFamily,
        body: HirSemanticBodyOwner,
    ) -> Result<(), AnalyzerExpressionError> {
        if self.receive_completion_transfers(module, completion, |target| matches!(target, HirControlTransferTarget::Loop { family: target_family, body_owner } if *target_family == family && *body_owner == body))? {
            completion.normal = completion.normal.join(NormalContinuation::Unit);
        }
        Ok(())
    }
    fn block_statements_completion_in(
        &self,
        module: &HirModule,
        statements: &[StmtId],
        query: CompletionQuery<'_>,
    ) -> Result<BodyCompletion, AnalyzerExpressionError> {
        let mut result = BodyCompletion::unit();
        // Dead statements remain checked and constrain Return values, while a
        // preceding Never continuation cannot be revived by their completion.
        for statement in statements {
            result.sequence(self.statement_body_completion(module, *statement, query)?);
        }
        Ok(result)
    }
    fn contextual_body_completion(
        &self,
        module: &HirModule,
        body: &HirContextualStmtBody,
        query: CompletionQuery<'_>,
    ) -> Result<BodyCompletion, AnalyzerExpressionError> {
        let mut result = BodyCompletion::unit();
        for edge in body.try_child_edges().map_err(|_| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::RecoveredOwner)
        })? {
            result.sequence(match edge.child() {
                HirBodyChild::Statement(statement) => {
                    self.statement_body_completion(module, statement, query)?
                }
                HirBodyChild::Expression(expression) => {
                    self.expression_body_completion_in(module, expression, query)?
                }
            });
        }
        Ok(result)
    }
    fn else_body_completion(
        &self,
        module: &HirModule,
        branch: Option<&HirConditionalElseBranch>,
        query: CompletionQuery<'_>,
    ) -> Result<BodyCompletion, AnalyzerExpressionError> {
        match branch {
            Some(HirConditionalElseBranch::Body(body)) => {
                self.contextual_body_completion(module, body, query)
            }
            Some(HirConditionalElseBranch::ElseIf(statement)) => {
                self.statement_body_completion(module, *statement, query)
            }
            None => Ok(BodyCompletion::unit()),
        }
    }
    fn block_body_completion(
        &self,
        module: &HirModule,
        statements: &[StmtId],
        tail: ExprId,
        query: CompletionQuery<'_>,
    ) -> Result<BodyCompletion, AnalyzerExpressionError> {
        let mut result = self.block_statements_completion_in(module, statements, query)?;
        result.sequence(self.expression_body_completion_in(module, tail, query)?);
        Ok(result)
    }
    fn expression_body_completion_in(
        &self,
        module: &HirModule,
        owner: ExprId,
        query: CompletionQuery<'_>,
    ) -> Result<BodyCompletion, AnalyzerExpressionError> {
        self.control
            .check()
            .map_err(AnalyzerExpressionError::fatal)?;
        let expression = module.resolve_expr(owner).map_err(|_| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner)
        })?;
        if matches!(
            expression.kind(),
            HirExprKind::Closure(_) | HirExprKind::Thread(_)
        ) || (Some(owner) != query.callable
            && self
                .facts
                .expressions()
                .get(&owner)
                .is_some_and(PreparedExpressionFact::creates_implicit_callable))
            || matches!(expression.kind(), HirExprKind::ComputationBlock(block) if matches!(block.kind(), HirComputationBlockKind::Seq | HirComputationBlockKind::Stream))
        {
            // Creating a callable, generator or child does not run its body.
            return Ok(BodyCompletion::unit());
        }
        let mut result = match expression.kind() {
            HirExprKind::Block(block) => {
                self.block_body_completion(module, block.statements(), block.tail(), query)?
            }
            HirExprKind::NamedBlock(block) => {
                self.block_body_completion(module, block.statements(), block.tail(), query)?
            }
            HirExprKind::ComputationBlock(block) => {
                self.block_body_completion(module, block.statements(), block.tail(), query)?
            }
            HirExprKind::Loop(block) => {
                let mut result =
                    self.block_body_completion(module, block.statements(), block.tail(), query)?;
                self.receive_loop_completion(
                    module,
                    &mut result,
                    HirLoopTargetFamily::LoopExpression,
                    HirSemanticBodyOwner::direct_expression(owner),
                )?;
                result
            }
            _ => {
                let mut result = BodyCompletion::unit();
                // Existing checked expression typing owns conditional normal
                // completion. HIR selects only the accepted owning operands;
                // semantic reference edges remain outside active evaluation.
                for edge in
                    self.completion_expression_edges(module, owner, expression.kind(), query)?
                {
                    if let HirExpressionEvaluationEdge::Expression {
                        ownership: HirExpressionChildOwnership::Owning,
                        child,
                        ..
                    } = edge
                    {
                        result.evidence(self.expression_body_completion_in(module, child, query)?);
                    }
                }

                for edge in expression
                    .kind()
                    .expression_owned_child_edges()
                    .map_err(|_| {
                        AnalyzerExpressionError::fatal(
                            FinalSemanticAnalysisError::AccountingOverflow,
                        )
                    })?
                {
                    let child = match edge.child() {
                        HirExpressionOwnedChild::Statement(statement) => {
                            Some(self.statement_body_completion(module, statement, query)?)
                        }
                        HirExpressionOwnedChild::Body(edge) => Some(match edge.child() {
                            HirBodyChild::Statement(statement) => {
                                self.statement_body_completion(module, statement, query)?
                            }
                            HirBodyChild::Expression(expression) => {
                                self.expression_body_completion_in(module, expression, query)?
                            }
                        }),
                        HirExpressionOwnedChild::Pattern(_) => None,
                    };
                    if let Some(child) = child {
                        if matches!(
                            edge.role(),
                            HirExpressionOwnedBodyRole::DialogueLinePlanStatement { .. }
                        ) {
                            // A line plan runs its admitted source-order body.
                            // Only the exact Output receiver can resume a
                            // reached Out; Return/Goto still escape this body.
                            result.sequence(child);
                        } else {
                            result.evidence(child);
                        }
                    }
                }
                result
            }
        };
        if matches!(expression.kind(), HirExprKind::AttachedContentApplication(_))
            && self.receive_completion_transfers(module, &mut result, |target| matches!(target, HirControlTransferTarget::Output { application } if *application == owner))?
        {
            result.normal = result.normal.join(NormalContinuation::Unit);
        }
        let normal = self
            .completion_expression_fact(owner, query)?
            .normal_continuation()
            .ok_or_else(|| {
                AnalyzerExpressionError::fatal(
                    FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner },
                )
            })?;
        result.normal = result.normal.then(normal);
        Ok(result)
    }
    fn statement_body_completion(
        &self,
        module: &HirModule,
        owner: StmtId,
        query: CompletionQuery<'_>,
    ) -> Result<BodyCompletion, AnalyzerExpressionError> {
        self.control
            .check()
            .map_err(AnalyzerExpressionError::fatal)?;
        let statement = module.resolve_stmt(owner).map_err(|_| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner)
        })?;
        let result = match statement.kind().evaluation_plan() {
            HirStmtEvaluationPlan::Value {
                kind, expression, ..
            } => {
                if kind == HirStmtValuePlanKind::Defer {
                    return Ok(BodyCompletion::unit());
                }
                let mut result = match expression {
                    Some(expression) => {
                        self.expression_body_completion_in(module, expression, query)?
                    }
                    None if kind == HirStmtValuePlanKind::Return => {
                        return Err(AnalyzerExpressionError::fatal(
                            FinalSemanticAnalysisError::WrongPayloadFamily,
                        ));
                    }
                    None => BodyCompletion::unit(),
                };
                if kind == HirStmtValuePlanKind::Return {
                    if let Some(callable) = query.callable {
                        let target = self.completion_transfer_target(owner)?;
                        let receives = match target {
                            HirControlTransferTarget::Return {
                                context: HirReturnContext::FunctionSite(site),
                            } => *site == callable,
                            HirControlTransferTarget::Return {
                                context: HirReturnContext::Item(_),
                            } => {
                                // An implicit callable retains the typed lexical
                                // Item return context until its sema boundary is
                                // selected. Nested callable bodies were excluded
                                // above; no source parent/name resolver is used.
                                !matches!(
                                    module
                                        .resolve_expr(callable)
                                        .map_err(|_| AnalyzerExpressionError::fatal(
                                            FinalSemanticAnalysisError::InvalidOwner
                                        ))?
                                        .kind(),
                                    HirExprKind::Closure(_)
                                )
                            }
                            _ => false,
                        };
                        if receives {
                            result.returns.insert(owner);
                        }
                    }
                }
                if matches!(
                    kind,
                    HirStmtValuePlanKind::Return
                        | HirStmtValuePlanKind::Out
                        | HirStmtValuePlanKind::Goto
                        | HirStmtValuePlanKind::Break
                ) {
                    if kind != HirStmtValuePlanKind::Goto {
                        self.completion_transfer_target(owner)?;
                    }
                    if result.continues() {
                        result.escaping.insert(owner);
                    }
                    result.normal = NormalContinuation::Never;
                }
                result
            }
            HirStmtEvaluationPlan::Binding { input, .. } => {
                self.expression_body_completion_in(module, input, query)?
            }
            HirStmtEvaluationPlan::OrderedPair { first, second, .. } => {
                let mut result = self.expression_body_completion_in(module, first, query)?;
                result.sequence(self.expression_body_completion_in(module, second, query)?);
                result
            }
            HirStmtEvaluationPlan::Scope { body, .. }
            | HirStmtEvaluationPlan::SourceLocale { body, .. } => {
                self.contextual_body_completion(module, body, query)?
            }
            HirStmtEvaluationPlan::UnsafeLifetime { body, .. } => {
                self.block_statements_completion_in(module, body.statements(), query)?
            }
            HirStmtEvaluationPlan::If {
                condition,
                then_body,
                else_branch,
            } => {
                let mut result = self.expression_body_completion_in(module, condition, query)?;
                let mut branches = self.contextual_body_completion(module, then_body, query)?;
                branches.branch(self.else_body_completion(module, else_branch, query)?);
                result.sequence(branches);
                result
            }
            HirStmtEvaluationPlan::IfLet {
                scrutinee,
                guard,
                then_body,
                else_branch,
                ..
            } => {
                let mut result = self.expression_body_completion_in(module, scrutinee, query)?;
                let mut branches = guard
                    .map(|guard| self.expression_body_completion_in(module, guard, query))
                    .transpose()?
                    .unwrap_or_else(BodyCompletion::unit);
                branches.sequence(self.contextual_body_completion(module, then_body, query)?);
                branches.branch(self.else_body_completion(module, else_branch, query)?);
                result.sequence(branches);
                result
            }
            HirStmtEvaluationPlan::Match { scrutinee, arms } => {
                let mut result = self.expression_body_completion_in(module, scrutinee, query)?;
                // Join identity is Never: an empty exhaustive/uninhabited Match
                // cannot produce a normal continuation. Coverage owns whether
                // the source domain admits that empty arm inventory.
                let mut branches = BodyCompletion::never();
                for arm in arms {
                    let mut branch = arm
                        .guard()
                        .map(|guard| self.expression_body_completion_in(module, guard, query))
                        .transpose()?
                        .unwrap_or_else(BodyCompletion::unit);
                    branch.sequence(match arm.body() {
                        HirStmtMatchArmBody::Expression(expression) => {
                            self.expression_body_completion_in(module, *expression, query)?
                        }
                        HirStmtMatchArmBody::Body(body) => {
                            self.contextual_body_completion(module, body, query)?
                        }
                    });
                    branches.branch(branch);
                }
                result.sequence(branches);
                result
            }
            HirStmtEvaluationPlan::LetElse {
                initializer,
                else_body,
                ..
            } => {
                let mut result = self.expression_body_completion_in(module, initializer, query)?;
                // The existing local-use seal enforces the failed branch Never
                // rule; its invalid continuation is not an acceptance here.
                result.evidence(self.block_statements_completion_in(module, else_body, query)?);
                result
            }
            HirStmtEvaluationPlan::While { condition, body } => {
                let mut result = self.expression_body_completion_in(module, condition, query)?;
                // A loop can run zero times; body Return operands still constrain
                // its callable. Actual loop-exit availability remains CFG-owned.
                let mut child = self.contextual_body_completion(module, body, query)?;
                self.receive_loop_completion(
                    module,
                    &mut child,
                    HirLoopTargetFamily::WhileStatement,
                    HirSemanticBodyOwner::statement_body(owner, HirStatementBodyRole::While),
                )?;
                if result.continues() {
                    result.evidence(child);
                } else {
                    result.returns.extend(child.returns);
                }
                result
            }
            HirStmtEvaluationPlan::WhileLet {
                scrutinee,
                guard,
                body,
                ..
            } => {
                let mut result = self.expression_body_completion_in(module, scrutinee, query)?;
                let mut child = guard
                    .map(|guard| self.expression_body_completion_in(module, guard, query))
                    .transpose()?
                    .unwrap_or_else(BodyCompletion::unit);
                child.sequence(self.contextual_body_completion(module, body, query)?);
                self.receive_loop_completion(
                    module,
                    &mut child,
                    HirLoopTargetFamily::WhileLetStatement,
                    HirSemanticBodyOwner::statement_body(owner, HirStatementBodyRole::WhileLet),
                )?;
                if result.continues() {
                    result.evidence(child);
                } else {
                    result.returns.extend(child.returns);
                }
                result
            }
            HirStmtEvaluationPlan::For {
                source,
                iterator,
                next_value,
                body,
                key,
                ..
            } => {
                let mut result = self.expression_body_completion_in(module, source, query)?;
                result.sequence(self.expression_body_completion_in(module, iterator, query)?);
                let mut child = self.expression_body_completion_in(module, next_value, query)?;
                if let Some(key) = key {
                    child.sequence(self.expression_body_completion_in(module, key, query)?);
                }
                child.sequence(self.contextual_body_completion(module, body, query)?);
                self.receive_loop_completion(
                    module,
                    &mut child,
                    HirLoopTargetFamily::ForStatement,
                    HirSemanticBodyOwner::statement_body(owner, HirStatementBodyRole::For),
                )?;
                if result.continues() {
                    result.evidence(child);
                } else {
                    result.returns.extend(child.returns);
                }
                result
            }
            HirStmtEvaluationPlan::Select { plan, .. } => match plan {
                HirStmtSelectEvaluationPlan::Operand { expression } => {
                    self.expression_body_completion_in(module, expression, query)?
                }
                HirStmtSelectEvaluationPlan::Branches { branches } => {
                    let mut result = if branches.is_empty() {
                        BodyCompletion::unit()
                    } else {
                        BodyCompletion::never()
                    };
                    for branch in branches.entries() {
                        result.branch(self.contextual_body_completion(
                            module,
                            branch.body(),
                            query,
                        )?);
                    }
                    result
                }
            },
            HirStmtEvaluationPlan::Continue { .. } => {
                self.completion_transfer_target(owner)?;
                let mut result = BodyCompletion::never();
                result.escaping.insert(owner);
                result
            }
            HirStmtEvaluationPlan::Assertion { conditions, .. } => {
                let mut result = BodyCompletion::unit();
                for expression in conditions {
                    result.sequence(self.expression_body_completion_in(
                        module,
                        *expression,
                        query,
                    )?);
                }
                result
            }
            HirStmtEvaluationPlan::EventBody { .. }
            | HirStmtEvaluationPlan::CancelRule { .. }
            | HirStmtEvaluationPlan::Include { .. } => BodyCompletion::unit(),
            HirStmtEvaluationPlan::Recovered => {
                return Err(AnalyzerExpressionError::fatal(
                    FinalSemanticAnalysisError::RecoveredOwner,
                ));
            }
        };
        Ok(result)
    }
}
