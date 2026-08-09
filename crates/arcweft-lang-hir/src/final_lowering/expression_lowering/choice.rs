//! Direct attached Choice lowering into the sole final expression owner.
//!
//! Candidate suites keep lexical scopes and typed arena IDs. This module does
//! not reopen source text, reconstruct the detached Choice model, or route a
//! recognized Choice through the generic Error expression family.

mod candidate;
mod plan;

use arcweft_lang_syntax::attachment::{
    AttachedChoiceBody, AttachedChoiceIf, AttachedChoiceItem, AttachedChoiceMatchArm,
    AttachedChoiceMatchArmBody, AttachedExpressionNode, AttachedRequiredChoiceBody,
    AttachedRequiredChoiceEntityReference, AttachedRequiredChoiceMatchBody,
    RequiredStatementExpressionNode,
};
use arcweft_source::SourceSpan;

use crate::expr::{
    HirChoiceBody, HirChoiceExpr, HirChoiceFor, HirChoiceIf, HirChoiceIfBranch, HirChoiceItem,
    HirChoiceMatch, HirChoiceMatchArm, HirExpressionRecoveryIssue, HirGenericExprIssue,
    HirRecoveryIssue,
};
use crate::identity::{ExprId, HirLimit, LocalId, ScopeId, SyntheticOwner};
use crate::leaf::{HirIdRefIssue, HirIdRefRecovery, HirIdRefShape, HirIdRefValue};
use crate::lowering::{HirInvariantFailure, HirLowerFailure};
use crate::scope::{HirPatternBindingPolicy, HirScope, HirScopeKind, HirScopeOwner};
use crate::source_index::{HirExprSourceRole, HirSourceSite};

use super::super::id_ref_projection::id_ref;
use super::super::{StagedHirModuleTransaction, require_limit};

pub(super) struct ChoiceLoweringState {
    owner: ExprId,
    next_required_expression: u32,
    recovered: bool,
}

impl ChoiceLoweringState {
    const fn new(owner: ExprId) -> Self {
        Self {
            owner,
            next_required_expression: 0,
            recovered: false,
        }
    }

    fn take_required_expression_ordinal(&mut self) -> Result<u32, HirLowerFailure> {
        let ordinal = self.next_required_expression;
        let observed = usize::try_from(ordinal)
            .map_err(|_| HirInvariantFailure::InvalidSlotCommit)?
            .checked_add(1)
            .ok_or(HirInvariantFailure::InvalidSlotCommit)?;
        require_limit(HirLimit::SyntheticDescendantsPerOwner, observed)?;
        self.next_required_expression = ordinal
            .checked_add(1)
            .ok_or(HirInvariantFailure::InvalidSlotCommit)?;
        Ok(ordinal)
    }

    fn skip_required_expression(&mut self) -> Result<(), HirLowerFailure> {
        self.take_required_expression_ordinal().map(|_| ())
    }

    fn required_expression_count(&self) -> Result<usize, HirLowerFailure> {
        usize::try_from(self.next_required_expression)
            .map_err(|_| HirInvariantFailure::InvalidSlotCommit.into())
    }

    pub(super) const fn owner(&self) -> ExprId {
        self.owner
    }

    pub(super) fn mark_recovered(&mut self) {
        self.recovered = true;
    }
}

struct PreparedChoiceBody<'attached> {
    scope: ScopeId,
    items: Vec<&'attached AttachedChoiceItem>,
    source_recovered: bool,
}

struct ChoiceBodyFrame<'attached> {
    scope: ScopeId,
    items: Vec<&'attached AttachedChoiceItem>,
    next_item: usize,
    source_recovered: bool,
    lowered_items: Vec<HirChoiceItem>,
    locals: Vec<LocalId>,
}

impl<'attached> ChoiceBodyFrame<'attached> {
    fn new(prepared: PreparedChoiceBody<'attached>, prefix_locals: Box<[LocalId]>) -> Self {
        Self {
            scope: prepared.scope,
            lowered_items: Vec::with_capacity(prepared.items.len()),
            items: prepared.items,
            next_item: 0,
            source_recovered: prepared.source_recovered,
            locals: Vec::from(prefix_locals),
        }
    }
}

enum ChoiceBodyContinuation<'attached> {
    IfBranch {
        parent: ChoiceBodyFrame<'attached>,
        conditional: &'attached AttachedChoiceIf,
        next_branch: usize,
        branches: Vec<HirChoiceIfBranch>,
        condition: ExprId,
    },
    IfElse {
        parent: ChoiceBodyFrame<'attached>,
        branches: Vec<HirChoiceIfBranch>,
    },
    For {
        parent: ChoiceBodyFrame<'attached>,
        pattern: crate::identity::PatternId,
        source: ExprId,
        locals: Box<[LocalId]>,
    },
    MatchArm {
        parent: ChoiceBodyFrame<'attached>,
        arms: &'attached [AttachedChoiceMatchArm],
        next_arm: usize,
        scrutinee: ExprId,
        lowered_arms: Vec<HirChoiceMatchArm>,
        pattern: crate::identity::PatternId,
        guard: Option<ExprId>,
        locals: Box<[LocalId]>,
    },
}

impl StagedHirModuleTransaction<'_> {
    pub(super) fn lower_attached_choice_expression(
        &mut self,
        attached: &AttachedExpressionNode,
        owner: ExprId,
        parent_scope: ScopeId,
    ) -> Result<(HirChoiceExpr, Option<HirRecoveryIssue>), HirLowerFailure> {
        let syntax = attached
            .choice()
            .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
        if syntax.syntax().id() != attached.id() {
            return Err(HirInvariantFailure::InvalidArenaCommit.into());
        }

        let expected_required_expressions = syntax.required_expression_slots().len();
        require_limit(
            HirLimit::SyntheticDescendantsPerOwner,
            expected_required_expressions,
        )?;
        let mut state = ChoiceLoweringState::new(owner);
        let id = syntax
            .id()
            .map(|reference| id_ref(reference.value()))
            .transpose()?;
        if id
            .as_ref()
            .is_some_and(crate::leaf::HirIdRefValue::is_recovered)
        {
            state.mark_recovered();
        }
        let body = self.lower_required_choice_body(
            syntax.body(),
            owner,
            parent_scope,
            Box::new([]),
            &mut state,
        )?;
        let plan = syntax
            .plan()
            .map(|plan| self.lower_choice_plan(plan, parent_scope, &mut state))
            .transpose()?;
        if syntax.has_recovery() {
            state.mark_recovered();
        }
        if state.required_expression_count()? != expected_required_expressions {
            return Err(HirInvariantFailure::InvalidArenaCommit.into());
        }

        let choice = HirChoiceExpr::new(id, body, plan);
        let recovery = state
            .recovered
            .then_some(HirRecoveryIssue::InvalidExpression(
                HirExpressionRecoveryIssue::Generic(HirGenericExprIssue::TransactionalChildFailure),
            ));
        Ok((choice, recovery))
    }

    fn lower_required_choice_body(
        &mut self,
        attached: &AttachedRequiredChoiceBody,
        owner: ExprId,
        parent_scope: ScopeId,
        prefix_locals: Box<[LocalId]>,
        state: &mut ChoiceLoweringState,
    ) -> Result<HirChoiceBody, HirLowerFailure> {
        let prepared = self.prepare_required_choice_body(attached, owner, parent_scope)?;
        self.finish_choice_body(prepared, prefix_locals, state)
    }

    fn prepare_required_choice_body<'attached>(
        &mut self,
        attached: &'attached AttachedRequiredChoiceBody,
        owner: ExprId,
        parent_scope: ScopeId,
    ) -> Result<PreparedChoiceBody<'attached>, HirLowerFailure> {
        match attached {
            AttachedRequiredChoiceBody::Present(body) => {
                self.prepare_choice_body(body, owner, parent_scope)
            }
            AttachedRequiredChoiceBody::Missing(missing) => {
                let scope = self.allocate_choice_scope(
                    missing.id(),
                    &missing.source_span(),
                    owner,
                    parent_scope,
                )?;
                Ok(PreparedChoiceBody {
                    scope,
                    items: Vec::new(),
                    source_recovered: true,
                })
            }
        }
    }

    fn prepare_choice_body<'attached>(
        &mut self,
        attached: &'attached AttachedChoiceBody,
        owner: ExprId,
        parent_scope: ScopeId,
    ) -> Result<PreparedChoiceBody<'attached>, HirLowerFailure> {
        let scope = self.allocate_choice_scope(
            attached.syntax().id(),
            &attached.syntax().source_span(),
            owner,
            parent_scope,
        )?;
        Ok(PreparedChoiceBody {
            scope,
            items: attached.items().iter().collect(),
            source_recovered: attached.has_recovery(),
        })
    }

    fn prepare_choice_match_arm_body<'attached>(
        &mut self,
        attached: &'attached AttachedChoiceMatchArmBody,
        arm: &'attached AttachedChoiceMatchArm,
        owner: ExprId,
        parent_scope: ScopeId,
    ) -> Result<PreparedChoiceBody<'attached>, HirLowerFailure> {
        match attached {
            AttachedChoiceMatchArmBody::Block(body) => {
                self.prepare_choice_body(body, owner, parent_scope)
            }
            AttachedChoiceMatchArmBody::Single(item) => {
                let scope = self.allocate_choice_scope(
                    arm.syntax().id(),
                    &arm.syntax().source_span(),
                    owner,
                    parent_scope,
                )?;
                Ok(PreparedChoiceBody {
                    scope,
                    items: vec![item.as_ref()],
                    source_recovered: item.has_recovery(),
                })
            }
            AttachedChoiceMatchArmBody::Missing(missing) => {
                let scope = self.allocate_choice_scope(
                    missing.id(),
                    &missing.source_span(),
                    owner,
                    parent_scope,
                )?;
                Ok(PreparedChoiceBody {
                    scope,
                    items: Vec::new(),
                    source_recovered: true,
                })
            }
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the explicit stack is one non-recursive source-order traversal over the complete nested Choice body grammar"
    )]
    fn finish_choice_body(
        &mut self,
        prepared: PreparedChoiceBody<'_>,
        prefix_locals: Box<[LocalId]>,
        state: &mut ChoiceLoweringState,
    ) -> Result<HirChoiceBody, HirLowerFailure> {
        let mut current = ChoiceBodyFrame::new(prepared, prefix_locals);
        let mut continuations = Vec::<ChoiceBodyContinuation<'_>>::new();

        loop {
            if let Some(attached) = current.items.get(current.next_item).copied() {
                current.next_item += 1;
                match attached {
                    AttachedChoiceItem::Let(statement) => {
                        let lowered =
                            self.lower_attached_thread_flow_statement(statement, current.scope)?;
                        if lowered.poisoned {
                            state.mark_recovered();
                        }
                        current
                            .lowered_items
                            .push(HirChoiceItem::Let(lowered.owner));
                        current.locals.extend(lowered.locals);
                    }
                    AttachedChoiceItem::If(conditional) => {
                        let branches = Vec::with_capacity(conditional.branches().len());
                        if let Some(branch) = conditional.branches().first() {
                            let condition = self.lower_choice_required_expression(
                                branch.condition(),
                                current.scope,
                                state,
                            )?;
                            let prepared = self.prepare_required_choice_body(
                                branch.then_body(),
                                state.owner(),
                                current.scope,
                            )?;
                            continuations.push(ChoiceBodyContinuation::IfBranch {
                                parent: current,
                                conditional,
                                next_branch: 1,
                                branches,
                                condition,
                            });
                            current = ChoiceBodyFrame::new(prepared, Box::new([]));
                        } else if let Some(else_body) = conditional.else_body() {
                            let prepared = self.prepare_required_choice_body(
                                else_body,
                                state.owner(),
                                current.scope,
                            )?;
                            continuations.push(ChoiceBodyContinuation::IfElse {
                                parent: current,
                                branches,
                            });
                            current = ChoiceBodyFrame::new(prepared, Box::new([]));
                        } else {
                            current
                                .lowered_items
                                .push(HirChoiceItem::If(HirChoiceIf::new(
                                    branches.into_boxed_slice(),
                                    None,
                                )));
                        }
                    }
                    AttachedChoiceItem::For(loop_item) => {
                        let source = self.lower_choice_required_expression(
                            loop_item.source(),
                            current.scope,
                            state,
                        )?;
                        let prepared = self.prepare_required_choice_body(
                            loop_item.body(),
                            state.owner(),
                            current.scope,
                        )?;
                        let pattern = self.lower_attached_pattern_binding(
                            loop_item.pattern(),
                            prepared.scope,
                            HirPatternBindingPolicy::PatternBinding,
                        )?;
                        if pattern.poisoned {
                            state.mark_recovered();
                        }
                        let locals = pattern.locals.clone();
                        continuations.push(ChoiceBodyContinuation::For {
                            parent: current,
                            pattern: pattern.owner,
                            source,
                            locals,
                        });
                        current = ChoiceBodyFrame::new(prepared, pattern.locals);
                    }
                    AttachedChoiceItem::Match(match_item) => {
                        let scrutinee = self.lower_choice_required_expression(
                            match_item.scrutinee(),
                            current.scope,
                            state,
                        )?;
                        match match_item.body() {
                            AttachedRequiredChoiceMatchBody::Missing(_) => {
                                state.mark_recovered();
                                current.lowered_items.push(HirChoiceItem::Match(
                                    HirChoiceMatch::new(scrutinee, Box::new([])),
                                ));
                            }
                            AttachedRequiredChoiceMatchBody::Present(body) => {
                                if body.has_recovery() {
                                    state.mark_recovered();
                                }
                                let arms = body.arms();
                                if let Some(arm) = arms.first() {
                                    let prepared = self.prepare_choice_match_arm_body(
                                        arm.body(),
                                        arm,
                                        state.owner(),
                                        current.scope,
                                    )?;
                                    let pattern = self.lower_attached_pattern_binding(
                                        arm.pattern(),
                                        prepared.scope,
                                        HirPatternBindingPolicy::MatchBinding,
                                    )?;
                                    if pattern.poisoned {
                                        state.mark_recovered();
                                    }
                                    let guard = arm
                                        .guard()
                                        .map(|guard| {
                                            let guard = guard.semantic().map_err(|_| {
                                                HirInvariantFailure::InvalidArenaCommit
                                            })?;
                                            let guard = self.lower_attached_expression(
                                                &guard,
                                                prepared.scope,
                                            )?;
                                            self.mark_choice_expression_recovery(guard, state)?;
                                            Ok::<_, HirLowerFailure>(guard)
                                        })
                                        .transpose()?;
                                    let locals = pattern.locals.clone();
                                    continuations.push(ChoiceBodyContinuation::MatchArm {
                                        parent: current,
                                        arms,
                                        next_arm: 1,
                                        scrutinee,
                                        lowered_arms: Vec::with_capacity(arms.len()),
                                        pattern: pattern.owner,
                                        guard,
                                        locals,
                                    });
                                    current = ChoiceBodyFrame::new(prepared, pattern.locals);
                                } else {
                                    current.lowered_items.push(HirChoiceItem::Match(
                                        HirChoiceMatch::new(scrutinee, Box::new([])),
                                    ));
                                }
                            }
                        }
                    }
                    AttachedChoiceItem::Option(option) => {
                        let option = self.lower_choice_option(option, current.scope, state)?;
                        current.lowered_items.push(HirChoiceItem::Option(option));
                    }
                    AttachedChoiceItem::OptionFor(option) => {
                        let option = self.lower_choice_option_for(option, current.scope, state)?;
                        current.lowered_items.push(HirChoiceItem::OptionFor(option));
                    }
                    AttachedChoiceItem::CompactArm(arm) => {
                        let arm = self.lower_choice_compact_arm(arm, current.scope, state)?;
                        current.lowered_items.push(HirChoiceItem::CompactArm(arm));
                    }
                    AttachedChoiceItem::Recovered(_) => {
                        state.mark_recovered();
                        current.lowered_items.push(HirChoiceItem::Error);
                    }
                }
                continue;
            }

            require_limit(HirLimit::LocalsPerScope, current.locals.len())?;
            self.close_scope_members(current.scope, current.locals.into_boxed_slice())?;
            if current.source_recovered {
                state.mark_recovered();
            }
            let completed =
                HirChoiceBody::new(current.scope, current.lowered_items.into_boxed_slice());
            let Some(continuation) = continuations.pop() else {
                return Ok(completed);
            };

            match continuation {
                ChoiceBodyContinuation::IfBranch {
                    mut parent,
                    conditional,
                    next_branch,
                    mut branches,
                    condition,
                } => {
                    branches.push(HirChoiceIfBranch::new(condition, completed));
                    if let Some(branch) = conditional.branches().get(next_branch) {
                        let condition = self.lower_choice_required_expression(
                            branch.condition(),
                            parent.scope,
                            state,
                        )?;
                        let prepared = self.prepare_required_choice_body(
                            branch.then_body(),
                            state.owner(),
                            parent.scope,
                        )?;
                        continuations.push(ChoiceBodyContinuation::IfBranch {
                            parent,
                            conditional,
                            next_branch: next_branch + 1,
                            branches,
                            condition,
                        });
                        current = ChoiceBodyFrame::new(prepared, Box::new([]));
                    } else if let Some(else_body) = conditional.else_body() {
                        let prepared = self.prepare_required_choice_body(
                            else_body,
                            state.owner(),
                            parent.scope,
                        )?;
                        continuations.push(ChoiceBodyContinuation::IfElse { parent, branches });
                        current = ChoiceBodyFrame::new(prepared, Box::new([]));
                    } else {
                        parent
                            .lowered_items
                            .push(HirChoiceItem::If(HirChoiceIf::new(
                                branches.into_boxed_slice(),
                                None,
                            )));
                        current = parent;
                    }
                }
                ChoiceBodyContinuation::IfElse {
                    mut parent,
                    branches,
                } => {
                    parent
                        .lowered_items
                        .push(HirChoiceItem::If(HirChoiceIf::new(
                            branches.into_boxed_slice(),
                            Some(completed),
                        )));
                    current = parent;
                }
                ChoiceBodyContinuation::For {
                    mut parent,
                    pattern,
                    source,
                    locals,
                } => {
                    parent
                        .lowered_items
                        .push(HirChoiceItem::For(HirChoiceFor::new(
                            pattern, source, completed, locals,
                        )));
                    current = parent;
                }
                ChoiceBodyContinuation::MatchArm {
                    mut parent,
                    arms,
                    next_arm,
                    scrutinee,
                    mut lowered_arms,
                    pattern,
                    guard,
                    locals,
                } => {
                    lowered_arms.push(HirChoiceMatchArm::new(pattern, guard, completed, locals));
                    if let Some(arm) = arms.get(next_arm) {
                        let prepared = self.prepare_choice_match_arm_body(
                            arm.body(),
                            arm,
                            state.owner(),
                            parent.scope,
                        )?;
                        let pattern = self.lower_attached_pattern_binding(
                            arm.pattern(),
                            prepared.scope,
                            HirPatternBindingPolicy::MatchBinding,
                        )?;
                        if pattern.poisoned {
                            state.mark_recovered();
                        }
                        let guard = arm
                            .guard()
                            .map(|guard| {
                                let guard = guard
                                    .semantic()
                                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                                let guard =
                                    self.lower_attached_expression(&guard, prepared.scope)?;
                                self.mark_choice_expression_recovery(guard, state)?;
                                Ok::<_, HirLowerFailure>(guard)
                            })
                            .transpose()?;
                        let locals = pattern.locals.clone();
                        continuations.push(ChoiceBodyContinuation::MatchArm {
                            parent,
                            arms,
                            next_arm: next_arm + 1,
                            scrutinee,
                            lowered_arms,
                            pattern: pattern.owner,
                            guard,
                            locals,
                        });
                        current = ChoiceBodyFrame::new(prepared, pattern.locals);
                    } else {
                        parent
                            .lowered_items
                            .push(HirChoiceItem::Match(HirChoiceMatch::new(
                                scrutinee,
                                lowered_arms.into_boxed_slice(),
                            )));
                        current = parent;
                    }
                }
            }
        }
    }

    pub(super) fn lower_choice_required_expression(
        &mut self,
        attached: &RequiredStatementExpressionNode,
        scope: ScopeId,
        state: &mut ChoiceLoweringState,
    ) -> Result<ExprId, HirLowerFailure> {
        // Every declared required-expression slot advances the same semantic
        // preorder. Authored versus missing recovery is deliberately not an
        // input to later RecoveryOperand identities.
        let ordinal = state.take_required_expression_ordinal()?;
        let expression = match attached {
            RequiredStatementExpressionNode::Expression(expression) => {
                let expression = expression
                    .semantic()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                self.lower_attached_expression(&expression, scope)?
            }
            RequiredStatementExpressionNode::Missing(missing) => {
                state.mark_recovered();
                let site = HirSourceSite::from_attached_span(
                    self.request.source().document(),
                    &missing.source_span(),
                )
                .map_err(|_| HirInvariantFailure::InvalidSourceSpan)?;
                self.lower_missing_owned_expression(
                    SyntheticOwner::Expr(state.owner()),
                    scope,
                    site,
                    ordinal,
                    HirExprSourceRole::Recovery,
                )?
            }
        };
        self.mark_choice_expression_recovery(expression, state)?;
        Ok(expression)
    }

    pub(super) fn mark_choice_expression_recovery(
        &mut self,
        expression: ExprId,
        state: &mut ChoiceLoweringState,
    ) -> Result<(), HirLowerFailure> {
        if self.staged_expression_is_poisoned(expression)? {
            state.mark_recovered();
        }
        Ok(())
    }

    fn allocate_choice_scope(
        &mut self,
        syntax: arcweft_lang_syntax::attachment::SyntaxNodeId,
        source: &SourceSpan,
        owner: ExprId,
        parent: ScopeId,
    ) -> Result<ScopeId, HirLowerFailure> {
        let source = HirSourceSite::from_attached_span(self.request.source().document(), source)
            .map_err(|_| HirInvariantFailure::InvalidSourceSpan)?;
        let reservation = self
            .arenas
            .scopes()
            .reserve_source(&mut self.slots, syntax, source)?;
        let scope = reservation.id();
        if reservation.is_first_touch() {
            let payload = HirScope::try_new(
                scope.module(),
                HirScopeKind::Block,
                Some(parent),
                HirScopeOwner::Expr(owner),
                Box::new([]),
                Box::new([]),
            )
            .map_err(|_| HirInvariantFailure::InvalidScopeParent)?;
            self.arenas
                .scopes()
                .finalize(&mut self.slots, reservation, payload)?;
            self.append_scope_child(parent, scope)?;
            return Ok(scope);
        }
        let retained = self.arenas.scopes().resolve_staged(&self.slots, scope)?;
        if retained.kind() == HirScopeKind::Block
            && retained.parent() == Some(parent)
            && retained.owner() == &HirScopeOwner::Expr(owner)
        {
            Ok(scope)
        } else {
            Err(HirInvariantFailure::InvalidScopeParent.into())
        }
    }
}

pub(super) fn project_choice_entity_reference(
    attached: &AttachedRequiredChoiceEntityReference,
) -> Result<HirIdRefValue, HirLowerFailure> {
    match attached {
        AttachedRequiredChoiceEntityReference::Reference(reference) => id_ref(reference.value()),
        AttachedRequiredChoiceEntityReference::Missing(_) => Ok(HirIdRefValue::Recovered(
            HirIdRefRecovery::new(HirIdRefShape::Missing, HirIdRefIssue::Missing),
        )),
    }
}
