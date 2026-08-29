//! Choice lifecycle-plan and trigger lowering.

use arcweft_lang_syntax::attachment::{
    AttachedChoicePlan, AttachedChoicePlanItem, AttachedPatternNode,
    AttachedRequiredChoicePlanBody, AttachedSignalTrigger, AttachedTriggerPattern,
};

use crate::expr::{HirChoicePlan, HirChoicePlanError, HirChoicePlanItem};
use crate::identity::{LocalId, PatternId, ScopeId};
use crate::lowering::HirLowerFailure;
use crate::scope::{HirPatternBindingPolicy, HirScopeOwner};
use crate::stmt::HirTrigger;

use super::super::super::StagedHirModuleTransaction;
use super::super::super::name_projection::{name, require_attempted_name_limit};
use super::ChoiceLoweringState;

impl StagedHirModuleTransaction<'_> {
    pub(super) fn lower_choice_plan(
        &mut self,
        attached: &AttachedChoicePlan,
        scope: ScopeId,
        state: &mut ChoiceLoweringState,
    ) -> Result<HirChoicePlan, HirLowerFailure> {
        let items = match attached.body() {
            AttachedRequiredChoicePlanBody::Present(body) => {
                let mut items = Vec::with_capacity(body.items().len());
                for item in body.items() {
                    items.push(self.lower_choice_plan_item(item, scope, state)?);
                }
                if body.has_recovery() {
                    state.mark_recovered();
                }
                items.into_boxed_slice()
            }
            AttachedRequiredChoicePlanBody::Missing(_) => {
                state.mark_recovered();
                Box::new([])
            }
        };
        if attached.has_recovery() {
            state.mark_recovered();
        }
        Ok(HirChoicePlan::new(items))
    }

    fn lower_choice_plan_item(
        &mut self,
        attached: &AttachedChoicePlanItem,
        scope: ScopeId,
        state: &mut ChoiceLoweringState,
    ) -> Result<HirChoicePlanItem, HirLowerFailure> {
        match attached {
            AttachedChoicePlanItem::Assignment(assignment) => {
                let key = match assignment.key().value() {
                    Ok(key) => name(key)?,
                    Err(issue) => {
                        require_attempted_name_limit(issue)?;
                        // The assignment value is still a declared required
                        // expression slot even though this recovered carrier
                        // cannot retain it. Keep later recovery identities
                        // independent of whether the key was accepted.
                        state.skip_required_expression()?;
                        state.mark_recovered();
                        return Ok(HirChoicePlanItem::Error(
                            HirChoicePlanError::InvalidAssignmentKey,
                        ));
                    }
                };
                let value =
                    self.lower_choice_required_expression(assignment.value(), scope, state)?;
                if assignment.has_recovery() {
                    state.mark_recovered();
                }
                Ok(HirChoicePlanItem::Assignment { key, value })
            }
            AttachedChoicePlanItem::Timeout(timeout) => {
                let duration =
                    self.lower_choice_required_expression(timeout.duration(), scope, state)?;
                let body = self.lower_attached_nested_thread_body(
                    timeout.body(),
                    HirScopeOwner::Expr(state.owner()),
                    scope,
                )?;
                if body.recovery.is_some() || timeout.has_recovery() {
                    state.mark_recovered();
                }
                Ok(HirChoicePlanItem::Timeout {
                    duration,
                    body: body.body,
                })
            }
            AttachedChoicePlanItem::Cancel(cancel) => {
                let prepared = self.prepare_attached_nested_thread_body(
                    cancel.body(),
                    HirScopeOwner::Expr(state.owner()),
                    scope,
                )?;
                let (trigger, locals) =
                    self.lower_choice_trigger(cancel.trigger(), prepared.scope(), state)?;
                let body = self.finish_attached_nested_thread_body(prepared, locals)?;
                if body.recovery.is_some() || cancel.has_recovery() {
                    state.mark_recovered();
                }
                Ok(HirChoicePlanItem::Cancel {
                    trigger,
                    body: body.body,
                })
            }
            AttachedChoicePlanItem::OnSelect(on_select) => {
                let prepared = self.prepare_attached_nested_thread_body(
                    on_select.body(),
                    HirScopeOwner::Expr(state.owner()),
                    scope,
                )?;
                let pattern = self.lower_attached_pattern_binding(
                    on_select.pattern(),
                    prepared.scope(),
                    HirPatternBindingPolicy::PatternBinding,
                )?;
                if pattern.poisoned {
                    state.mark_recovered();
                }
                let locals = pattern.locals.clone();
                let body = self.finish_attached_nested_thread_body(prepared, pattern.locals)?;
                if body.recovery.is_some() || on_select.has_recovery() {
                    state.mark_recovered();
                }
                Ok(HirChoicePlanItem::OnSelect {
                    pattern: pattern.owner,
                    locals,
                    body: body.body,
                })
            }
            AttachedChoicePlanItem::Recovered(_) => {
                state.mark_recovered();
                Ok(HirChoicePlanItem::Error(
                    HirChoicePlanError::RecoveredSyntax,
                ))
            }
        }
    }

    fn lower_choice_trigger(
        &mut self,
        attached: &AttachedTriggerPattern,
        scope: ScopeId,
        state: &mut ChoiceLoweringState,
    ) -> Result<(HirTrigger, Box<[LocalId]>), HirLowerFailure> {
        let (trigger, locals) = match attached {
            AttachedTriggerPattern::Input(pattern) => {
                let (pattern, locals) =
                    self.lower_choice_trigger_pattern(pattern.pattern(), scope, state)?;
                (HirTrigger::Input(pattern), locals)
            }
            AttachedTriggerPattern::Event(pattern) => {
                let (pattern, locals) =
                    self.lower_choice_trigger_pattern(pattern.pattern(), scope, state)?;
                (HirTrigger::Event(pattern), locals)
            }
            AttachedTriggerPattern::Mark(mark) => {
                state.mark_recovered();
                let issue = if mark.selector().has_recovery() {
                    crate::stmt::HirTriggerIssue::Malformed
                } else {
                    crate::stmt::HirTriggerIssue::MarkOutsideDialogueApplication
                };
                (HirTrigger::Recovered(issue), Box::<[LocalId]>::from([]))
            }
            AttachedTriggerPattern::Select(pattern) => {
                let (pattern, locals) =
                    self.lower_choice_trigger_pattern(pattern.pattern(), scope, state)?;
                (HirTrigger::Select(pattern), locals)
            }
            AttachedTriggerPattern::Task(pattern) => {
                let (pattern, locals) =
                    self.lower_choice_trigger_pattern(pattern.pattern(), scope, state)?;
                (HirTrigger::Task(pattern), locals)
            }
            AttachedTriggerPattern::Scope(pattern) => {
                let (pattern, locals) =
                    self.lower_choice_trigger_pattern(pattern.pattern(), scope, state)?;
                (HirTrigger::Scope(pattern), locals)
            }
            AttachedTriggerPattern::Signal(signal) => {
                self.lower_choice_signal_trigger(signal, scope, state)?
            }
            AttachedTriggerPattern::Timeout(timeout) => {
                let expression =
                    self.lower_choice_required_expression(timeout.expression(), scope, state)?;
                (HirTrigger::Timeout(expression), Box::<[LocalId]>::from([]))
            }
            AttachedTriggerPattern::Expr(expression) => {
                let expression = self.lower_attached_expression(expression, scope)?;
                self.mark_choice_expression_recovery(expression, state)?;
                (
                    HirTrigger::Expression(expression),
                    Box::<[LocalId]>::from([]),
                )
            }
        };
        if attached.has_recovery() {
            state.mark_recovered();
        }
        Ok((trigger, locals))
    }

    fn lower_choice_trigger_pattern(
        &mut self,
        attached: &AttachedPatternNode,
        scope: ScopeId,
        state: &mut ChoiceLoweringState,
    ) -> Result<(PatternId, Box<[LocalId]>), HirLowerFailure> {
        let pattern = self.lower_attached_pattern_binding(
            attached,
            scope,
            HirPatternBindingPolicy::PatternBinding,
        )?;
        if pattern.poisoned {
            state.mark_recovered();
        }
        Ok((pattern.owner, pattern.locals))
    }

    fn lower_choice_signal_trigger(
        &mut self,
        attached: &AttachedSignalTrigger,
        scope: ScopeId,
        state: &mut ChoiceLoweringState,
    ) -> Result<(HirTrigger, Box<[LocalId]>), HirLowerFailure> {
        let target = self.lower_choice_required_expression(attached.target(), scope, state)?;
        let (value, locals) = attached
            .value()
            .map(|value| {
                let pattern = self.lower_attached_pattern_binding(
                    value,
                    scope,
                    HirPatternBindingPolicy::PatternBinding,
                )?;
                if pattern.poisoned {
                    state.mark_recovered();
                }
                Ok::<_, HirLowerFailure>((Some(pattern.owner), pattern.locals))
            })
            .transpose()?
            .unwrap_or((None, Box::new([])));
        Ok((HirTrigger::Signal { target, value }, locals))
    }
}
