//! Typed Dialogue line-plan lowering through the existing HIR arenas.

use arcweft_lang_syntax::attachment::AttachedDialogueLinePlan;

use crate::dialogue_application::{HirLinePlan, HirLinePlanItem};
use crate::identity::{ExprId, LocalId, ScopeId};
use crate::lowering::{HirInvariantFailure, HirLowerFailure};
use crate::stmt::HirStmtKind;

use super::super::super::StagedHirModuleTransaction;

impl StagedHirModuleTransaction<'_> {
    pub(super) fn lower_dialogue_line_plan(
        &mut self,
        attached: &AttachedDialogueLinePlan,
        owner: ExprId,
        parent_scope: ScopeId,
    ) -> Result<HirLinePlan, HirLowerFailure> {
        let scope = self.allocate_expression_owned_block_scope(
            attached.body().syntax(),
            owner,
            parent_scope,
        )?;
        let mut items = Vec::with_capacity(attached.body().items().len());
        let mut locals = Vec::<LocalId>::new();
        for statement in attached.body().items() {
            let lowered = self.lower_attached_thread_flow_statement(statement, scope)?;
            locals.extend_from_slice(&lowered.locals);
            let retained = self
                .arenas
                .statements()
                .resolve_staged(&self.slots, lowered.owner)?;
            let item = if lowered.poisoned {
                HirLinePlanItem::Error(lowered.owner)
            } else {
                match retained.kind() {
                    HirStmtKind::Let {
                        pattern,
                        initializer,
                        ..
                    } => HirLinePlanItem::Let {
                        pattern: *pattern,
                        value: *initializer,
                        statement: lowered.owner,
                    },
                    HirStmtKind::Out { label: None, value } => HirLinePlanItem::Out {
                        value: *value,
                        statement: lowered.owner,
                    },
                    HirStmtKind::Expression { expression } => {
                        HirLinePlanItem::Expression(*expression)
                    }
                    _ => HirLinePlanItem::Statement(lowered.owner),
                }
            };
            items.push(item);
        }
        self.close_scope_members(scope, locals.into_boxed_slice())?;
        HirLinePlan::try_new(scope, None, items.into_boxed_slice())
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit.into())
    }
}
