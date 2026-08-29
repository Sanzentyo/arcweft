//! Typed Dialogue line-plan lowering through the existing HIR arenas.

use arcweft_lang_syntax::attachment::AttachedDialogueLinePlan;

use crate::dialogue_application::{HirDialogueContent, HirLinePlan, HirLinePlanItem};
use crate::identity::{ExprId, LocalId, ScopeId};
use crate::lowering::{HirInvariantFailure, HirLowerFailure};

use super::super::super::StagedHirModuleTransaction;

impl StagedHirModuleTransaction<'_> {
    pub(super) fn lower_dialogue_line_plan(
        &mut self,
        attached: &AttachedDialogueLinePlan,
        owner: ExprId,
        parent_scope: ScopeId,
        content: &HirDialogueContent,
    ) -> Result<HirLinePlan, HirLowerFailure> {
        let scope = self.allocate_expression_owned_block_scope(
            attached.body().syntax(),
            owner,
            parent_scope,
        )?;
        let mut items = Vec::with_capacity(attached.body().items().len());
        let mut locals = Vec::<LocalId>::new();
        for statement in attached.body().items() {
            let lowered =
                self.lower_attached_dialogue_line_plan_statement(statement, scope, content)?;
            locals.extend_from_slice(&lowered.locals);
            let item = if lowered.poisoned {
                HirLinePlanItem::Error(lowered.owner)
            } else {
                HirLinePlanItem::Statement(lowered.owner)
            };
            items.push(item);
        }
        self.close_scope_members(scope, locals.into_boxed_slice())?;
        HirLinePlan::try_new(scope, None, items.into_boxed_slice())
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit.into())
    }
}
