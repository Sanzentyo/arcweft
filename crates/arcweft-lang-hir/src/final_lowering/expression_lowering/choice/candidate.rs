//! Choice option, View, and compact-arm lowering.

use arcweft_lang_syntax::attachment::{
    AttachedChoiceCompactAction, AttachedChoiceCompactArm, AttachedChoiceOption,
    AttachedChoiceOptionBody, AttachedChoiceOptionField, AttachedChoiceOptionFor,
    AttachedChoiceView, AttachedChoiceViewEntry, AttachedRequiredChoiceOptionBody,
    AttachedRequiredChoiceViewBody,
};

use crate::expr::{
    HirChoiceCompactAction, HirChoiceCompactArm, HirChoiceOption, HirChoiceOptionBody,
    HirChoiceOptionField, HirChoiceOptionFor, HirChoiceView, HirChoiceViewEntry,
};
use crate::identity::{HirLimit, LocalId, ScopeId};
use crate::lowering::HirLowerFailure;
use crate::scope::{HirPatternBindingPolicy, HirScopeOwner};

use super::super::super::id_ref_projection::id_ref;
use super::super::super::{StagedHirModuleTransaction, require_limit};
use super::{ChoiceLoweringState, project_choice_entity_reference};

struct PreparedChoiceOptionBody<'attached> {
    scope: ScopeId,
    fields: &'attached [AttachedChoiceOptionField],
    source_recovered: bool,
}

impl StagedHirModuleTransaction<'_> {
    pub(super) fn lower_choice_option(
        &mut self,
        attached: &AttachedChoiceOption,
        scope: ScopeId,
        state: &mut ChoiceLoweringState,
    ) -> Result<HirChoiceOption, HirLowerFailure> {
        let id = self.lower_choice_required_expression(attached.id(), scope, state)?;
        let body =
            self.lower_required_choice_option_body(attached.body(), scope, Box::new([]), state)?;
        Ok(HirChoiceOption::new(id, body))
    }

    pub(super) fn lower_choice_option_for(
        &mut self,
        attached: &AttachedChoiceOptionFor,
        scope: ScopeId,
        state: &mut ChoiceLoweringState,
    ) -> Result<HirChoiceOptionFor, HirLowerFailure> {
        let source = self.lower_choice_required_expression(attached.source(), scope, state)?;
        let prepared = self.prepare_required_choice_option_body(attached.body(), scope, state)?;
        let pattern = self.lower_attached_pattern_binding(
            attached.pattern(),
            prepared.scope,
            HirPatternBindingPolicy::PatternBinding,
        )?;
        if pattern.poisoned {
            state.mark_recovered();
        }
        let locals = pattern.locals.clone();
        let body = self.finish_choice_option_body(&prepared, pattern.locals, state)?;
        Ok(HirChoiceOptionFor::new(pattern.owner, source, body, locals))
    }

    fn lower_required_choice_option_body(
        &mut self,
        attached: &AttachedRequiredChoiceOptionBody,
        parent_scope: ScopeId,
        prefix_locals: Box<[LocalId]>,
        state: &mut ChoiceLoweringState,
    ) -> Result<HirChoiceOptionBody, HirLowerFailure> {
        let prepared = self.prepare_required_choice_option_body(attached, parent_scope, state)?;
        self.finish_choice_option_body(&prepared, prefix_locals, state)
    }

    fn prepare_required_choice_option_body<'attached>(
        &mut self,
        attached: &'attached AttachedRequiredChoiceOptionBody,
        parent_scope: ScopeId,
        state: &ChoiceLoweringState,
    ) -> Result<PreparedChoiceOptionBody<'attached>, HirLowerFailure> {
        match attached {
            AttachedRequiredChoiceOptionBody::Present(body) => {
                self.prepare_choice_option_body(body, parent_scope, state)
            }
            AttachedRequiredChoiceOptionBody::Missing(missing) => {
                let scope = self.allocate_choice_scope(
                    missing.id(),
                    &missing.source_span(),
                    state.owner(),
                    parent_scope,
                )?;
                Ok(PreparedChoiceOptionBody {
                    scope,
                    fields: &[],
                    source_recovered: true,
                })
            }
        }
    }

    fn prepare_choice_option_body<'attached>(
        &mut self,
        attached: &'attached AttachedChoiceOptionBody,
        parent_scope: ScopeId,
        state: &ChoiceLoweringState,
    ) -> Result<PreparedChoiceOptionBody<'attached>, HirLowerFailure> {
        let scope = self.allocate_choice_scope(
            attached.syntax().id(),
            &attached.syntax().source_span(),
            state.owner(),
            parent_scope,
        )?;
        Ok(PreparedChoiceOptionBody {
            scope,
            fields: attached.fields(),
            source_recovered: attached.has_recovery(),
        })
    }

    fn finish_choice_option_body(
        &mut self,
        prepared: &PreparedChoiceOptionBody<'_>,
        prefix_locals: Box<[LocalId]>,
        state: &mut ChoiceLoweringState,
    ) -> Result<HirChoiceOptionBody, HirLowerFailure> {
        let mut fields = Vec::with_capacity(prepared.fields.len());
        let mut locals = Vec::from(prefix_locals);
        for field in prepared.fields {
            let (field, field_locals) =
                self.lower_choice_option_field(field, prepared.scope, state)?;
            fields.push(field);
            locals.extend(field_locals);
        }
        require_limit(HirLimit::LocalsPerScope, locals.len())?;
        self.close_scope_members(prepared.scope, locals.into_boxed_slice())?;
        if prepared.source_recovered {
            state.mark_recovered();
        }
        Ok(HirChoiceOptionBody::new(
            prepared.scope,
            fields.into_boxed_slice(),
        ))
    }

    fn lower_choice_option_field(
        &mut self,
        attached: &AttachedChoiceOptionField,
        scope: ScopeId,
        state: &mut ChoiceLoweringState,
    ) -> Result<(HirChoiceOptionField, Box<[LocalId]>), HirLowerFailure> {
        let field = match attached {
            AttachedChoiceOptionField::Label {
                text_key, value, ..
            } => {
                let text_key = text_key
                    .as_ref()
                    .as_ref()
                    .map(project_choice_entity_reference)
                    .transpose()?;
                if text_key
                    .as_ref()
                    .is_some_and(crate::leaf::HirIdRefValue::is_recovered)
                {
                    state.mark_recovered();
                }
                let value = self.lower_choice_required_expression(value, scope, state)?;
                HirChoiceOptionField::Label { text_key, value }
            }
            AttachedChoiceOptionField::Id { value, .. } => HirChoiceOptionField::Id(
                self.lower_choice_required_expression(value, scope, state)?,
            ),
            AttachedChoiceOptionField::Value { value, .. } => HirChoiceOptionField::Value(
                self.lower_choice_required_expression(value, scope, state)?,
            ),
            AttachedChoiceOptionField::Visible { value, .. } => HirChoiceOptionField::Visible(
                self.lower_choice_required_expression(value, scope, state)?,
            ),
            AttachedChoiceOptionField::Enabled { value, .. } => HirChoiceOptionField::Enabled(
                self.lower_choice_required_expression(value, scope, state)?,
            ),
            AttachedChoiceOptionField::Order { value, .. } => HirChoiceOptionField::Order(
                self.lower_choice_required_expression(value, scope, state)?,
            ),
            AttachedChoiceOptionField::Hotkey { value, .. } => HirChoiceOptionField::Hotkey(
                self.lower_choice_required_expression(value, scope, state)?,
            ),
            AttachedChoiceOptionField::View(view) => {
                HirChoiceOptionField::View(self.lower_choice_view(view, scope, state)?)
            }
            AttachedChoiceOptionField::Select(select) => {
                let lowered = self.lower_attached_nested_thread_body(
                    select.body(),
                    HirScopeOwner::Expr(state.owner()),
                    scope,
                )?;
                if lowered.recovery.is_some() {
                    state.mark_recovered();
                }
                HirChoiceOptionField::Select(lowered.body)
            }
            AttachedChoiceOptionField::Let(statement) => {
                let lowered = self.lower_attached_thread_flow_statement(statement, scope)?;
                if lowered.poisoned {
                    state.mark_recovered();
                }
                return Ok((HirChoiceOptionField::Let(lowered.owner), lowered.locals));
            }
            AttachedChoiceOptionField::Recovered(_) => {
                state.mark_recovered();
                HirChoiceOptionField::Error
            }
        };
        Ok((field, Box::new([])))
    }

    fn lower_choice_view(
        &mut self,
        attached: &AttachedChoiceView,
        scope: ScopeId,
        state: &mut ChoiceLoweringState,
    ) -> Result<HirChoiceView, HirLowerFailure> {
        let entries = match attached.body() {
            AttachedRequiredChoiceViewBody::Present(body) => {
                let mut entries = Vec::with_capacity(body.fields().len());
                for entry in body.fields() {
                    entries.push(self.lower_choice_view_entry(entry, scope, state)?);
                }
                if body.has_recovery() {
                    state.mark_recovered();
                }
                entries.into_boxed_slice()
            }
            AttachedRequiredChoiceViewBody::Missing(_) => {
                state.mark_recovered();
                Box::new([])
            }
        };
        Ok(HirChoiceView::new(entries))
    }

    fn lower_choice_view_entry(
        &mut self,
        attached: &AttachedChoiceViewEntry,
        scope: ScopeId,
        state: &mut ChoiceLoweringState,
    ) -> Result<HirChoiceViewEntry, HirLowerFailure> {
        let key = self.lower_choice_required_expression(attached.key(), scope, state)?;
        let value = self.lower_choice_required_expression(attached.value(), scope, state)?;
        Ok(HirChoiceViewEntry::new(key, value))
    }

    pub(super) fn lower_choice_compact_arm(
        &mut self,
        attached: &AttachedChoiceCompactArm,
        scope: ScopeId,
        state: &mut ChoiceLoweringState,
    ) -> Result<HirChoiceCompactArm, HirLowerFailure> {
        let id = id_ref(attached.id().value())?;
        if id.is_recovered() {
            state.mark_recovered();
        }
        let label = self.lower_choice_required_expression(attached.label(), scope, state)?;
        let condition = attached
            .condition()
            .map(|condition| self.lower_choice_required_expression(condition, scope, state))
            .transpose()?;
        let action = match attached.action() {
            AttachedChoiceCompactAction::Goto { target, .. } => {
                let target = project_choice_entity_reference(target)?;
                if target.is_recovered() {
                    state.mark_recovered();
                }
                HirChoiceCompactAction::Goto(target)
            }
            AttachedChoiceCompactAction::Out { value, .. } => HirChoiceCompactAction::Out(
                self.lower_choice_required_expression(value, scope, state)?,
            ),
            AttachedChoiceCompactAction::Missing(_) => {
                state.mark_recovered();
                HirChoiceCompactAction::Missing
            }
        };
        Ok(HirChoiceCompactArm::new(id, label, condition, action))
    }
}
